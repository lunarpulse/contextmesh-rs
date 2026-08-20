//! CPython-compatible MT19937 so the Rust port reproduces the Python
//! prototype's random streams exactly: same seed + same call sequence
//! => identical data. Verified against captured Python draws in tests.

pub struct PyRng {
    mt: [u32; 624],
    idx: usize,
}

impl PyRng {
    /// Mirrors CPython `random.Random(int_seed)`: init_by_array over the
    /// 32-bit little-endian words of the (absolute) integer seed.
    pub fn new(seed: u64) -> Self {
        let mut key: Vec<u32> = Vec::new();
        if seed == 0 {
            key.push(0);
        } else {
            let mut n = seed;
            while n != 0 {
                key.push((n & 0xFFFF_FFFF) as u32);
                n >>= 32;
            }
        }
        let mut r = PyRng { mt: [0; 624], idx: 624 };
        r.init_by_array(&key);
        r
    }

    fn init_by_array(&mut self, key: &[u32]) {
        let mt = &mut self.mt;
        mt[0] = 19650218;
        for i in 1..624 {
            mt[i] = 1812433253u32
                .wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        let mut i = 1usize;
        let mut j = 0usize;
        let mut k = std::cmp::max(624, key.len());
        while k > 0 {
            mt[i] = (mt[i] ^ ((mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_mul(1664525)))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= 624 {
                mt[0] = mt[623];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
            k -= 1;
        }
        let mut k = 623;
        while k > 0 {
            mt[i] = (mt[i] ^ ((mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_mul(1566083941)))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= 624 {
                mt[0] = mt[623];
                i = 1;
            }
            k -= 1;
        }
        mt[0] = 0x8000_0000;
        self.idx = 624;
    }

    fn genrand(&mut self) -> u32 {
        if self.idx >= 624 {
            for i in 0..624 {
                let y = (self.mt[i] & 0x8000_0000) | (self.mt[(i + 1) % 624] & 0x7FFF_FFFF);
                self.mt[i] = self.mt[(i + 397) % 624] ^ (y >> 1) ^ (if y & 1 != 0 { 0x9908_B0DF } else { 0 });
            }
            self.idx = 0;
        }
        let mut y = self.mt[self.idx];
        self.idx += 1;
        y ^= y >> 11;
        y ^= y.wrapping_shl(7) & 0x9D2C_5680;
        y ^= y.wrapping_shl(15) & 0xEFC6_0000;
        y ^= y >> 18;
        y
    }

    /// CPython random.random(): 53-bit float from two 32-bit draws.
    pub fn random(&mut self) -> f64 {
        let a = (self.genrand() >> 5) as f64;
        let b = (self.genrand() >> 6) as f64;
        (a * 67108864.0 + b) * (1.0 / 9007199254740992.0)
    }

    /// CPython getrandbits(k): little-endian 32-bit words; top word is
    /// shifted down when the remaining bit count is under 32.
    pub fn getrandbits(&mut self, k: u32) -> u64 {
        assert!((1..=64).contains(&k));
        let words = ((k - 1) / 32 + 1) as usize;
        let mut out: u64 = 0;
        let mut rem = k;
        for i in 0..words {
            let mut r = self.genrand();
            if rem < 32 {
                r >>= 32 - rem;
            }
            out |= (r as u64) << (32 * i as u32);
            rem = rem.saturating_sub(32);
        }
        out
    }

    /// CPython _randbelow(n): draw k = n.bit_length() bits, reject >= n.
    pub fn randbelow(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        let k = 64 - n.leading_zeros();
        loop {
            let r = self.getrandbits(k);
            if r < n {
                return r;
            }
        }
    }

    /// CPython randrange(start, stop) = start + _randbelow(stop - start).
    pub fn randrange(&mut self, start: u64, stop: u64) -> u64 {
        start + self.randbelow(stop - start)
    }

    /// CPython randint(a, b) inclusive.
    pub fn randint(&mut self, a: u64, b: u64) -> u64 {
        self.randrange(a, b + 1)
    }

    pub fn choice<'a, T: Clone>(&mut self, items: &'a [T]) -> T {
        items[self.randbelow(items.len() as u64) as usize].clone()
    }

    /// CPython random.sample (both n<=setsize pool branch and set branch).
    pub fn sample<T: Clone>(&mut self, items: &[T], k: usize) -> Vec<T> {
        let n = items.len();
        assert!(k <= n, "sample larger than population");
        let mut result: Vec<T> = items[..k].to_vec();
        let mut setsize = 21usize;
        if k > 5 {
            setsize += 4usize.pow(((3.0 * k as f64).ln() / 4f64.ln()).ceil() as u32);
        }
        if n <= setsize {
            let mut pool: Vec<T> = items.to_vec();
            for i in 0..k {
                let j = self.randbelow((n - i) as u64) as usize;
                result[i] = pool[j].clone();
                pool[j] = pool[n - i - 1].clone();
            }
        } else {
            let mut selected: Vec<u64> = Vec::with_capacity(k);
            for i in 0..k {
                let mut j = self.randbelow(n as u64);
                while selected.contains(&j) {
                    j = self.randbelow(n as u64);
                }
                selected.push(j);
                result[i] = items[j as usize].clone();
            }
        }
        result
    }

    /// CPython random.shuffle (Fisher-Yates, descending).
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.randbelow((i + 1) as u64) as usize;
            v.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Captured from CPython 3.11: python3 - with random.Random(20260820).
    #[test]
    fn matches_cpython_stream() {
        let mut r = PyRng::new(20260820);
        assert_eq!(r.random(), 0.9141398216526289);
        assert_eq!(r.random(), 0.49992523027434044);
        assert_eq!(r.randrange(0, 2), 0);
        assert_eq!(r.randint(3, 6), 6);
        assert_eq!(r.randint(1000, 9500000), 7226044);
        let idx: Vec<usize> = (0..10).collect();
        assert_eq!(r.sample(&idx, 4), vec![7, 6, 1, 3]);
        let mut v: Vec<usize> = (0..8).collect();
        r.shuffle(&mut v);
        assert_eq!(v, vec![5, 0, 1, 6, 2, 4, 7, 3]);
    }

    #[test]
    fn matches_cpython_mkfile_stream() {
        let mut r = PyRng::new(20260820 + 31 * 3);
        let dirs = ["/srv/app", "/srv/db", "/srv/core", "/opt/edge"];
        assert_eq!(r.choice(&dirs), "/srv/app");
        assert_eq!(r.randint(1000, 9500000), 3185042);
        assert_eq!(r.getrandbits(48), 65510736306010u64);
        let mut r2 = PyRng::new(20260820);
        let vowels = ['a', 'e', 'i', 'o', 'u'];
        assert_eq!(r2.choice(&vowels), 'o');
    }
}
