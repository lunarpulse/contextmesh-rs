//! Regex-equivalent value/token extraction, hand-rolled so the crate stays
//! dependency-free. Semantics mirror the Python prototype's patterns:
//!   RE_HEX  \b[0-9a-f]{8,}\b   -> whole word-tokens, all hex, len >= 8
//!   RE_INT  \b\d{3,}\b         -> whole word-tokens, all digits, len >= 3
//!   RE_DATE \b\d{4}-\d{2}-\d{2}\b -> positional scan
//!   RE_HUM  \b(\d+(\.\d+)?)([KMGT])\b -> positional scan, expands to integers
//! Word chars follow Python \w: [A-Za-z0-9_].

use std::collections::HashSet;

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn whole_tokens(s: &str) -> Vec<&str> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        if is_word(b[i]) {
            let st = i;
            while i < b.len() && is_word(b[i]) {
                i += 1;
            }
            out.push(&s[st..i]);
        } else {
            i += 1;
        }
    }
    out
}

fn date_at(b: &[u8], i: usize) -> bool {
    let d = |j: usize| j < b.len() && b[j].is_ascii_digit();
    if i == 0 || !is_word(b[i - 1]) {
        // \d{4} '-' \d{2} '-' \d{2} \b
        if d(i) && d(i + 1) && d(i + 2) && d(i + 3)
            && i + 4 < b.len() && b[i + 4] == b'-'
            && d(i + 5) && d(i + 6)
            && i + 7 < b.len() && b[i + 7] == b'-'
            && d(i + 8) && d(i + 9)
        {
            let after = i + 10;
            return after >= b.len() || !is_word(b[after]);
        }
    }
    false
}

pub fn vals_raw(s: &str) -> HashSet<String> {
    let mut out: HashSet<String> = HashSet::new();
    for tok in whole_tokens(s) {
        let bytes = tok.as_bytes();
        let all_hex = bytes.iter().all(|c| c.is_ascii_digit() || c.is_ascii_lowercase() && *c <= b'f');
        if bytes.len() >= 8 && all_hex {
            out.insert(tok.to_string()); // RE_HEX (digits are a subset of hex)
        } else if bytes.len() >= 3 && bytes.iter().all(|c| c.is_ascii_digit()) {
            out.insert(tok.to_string()); // RE_INT
        }
    }
    let b = s.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        if date_at(b, i) {
            out.insert(s[i..i + 10].to_string());
            i += 10; // non-overlapping
        } else {
            i += 1;
        }
    }
    out
}

/// One humanized-number attempt at position i: returns (match_end, value).
fn hum_at(b: &[u8], i: usize) -> Option<(usize, f64, u8)> {
    if !(i == 0 || !is_word(b[i - 1])) || !b[i].is_ascii_digit() {
        return None;
    }
    let mut j = i;
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
    }
    let mut num_end = j;
    if j + 1 < b.len() && b[j] == b'.' && b[j + 1].is_ascii_digit() {
        j += 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        num_end = j;
    }
    if j < b.len() && matches!(b[j], b'K' | b'M' | b'G' | b'T') {
        let after = j + 1;
        if after >= b.len() || !is_word(b[after]) {
            let val: f64 = std::str::from_utf8(&b[i..num_end]).unwrap().parse().unwrap();
            return Some((after, val, b[j]));
        }
    }
    None
}

const SUF: [(u8, f64); 4] = [(b'K', 1e3), (b'M', 1e6), (b'G', 1e9), (b'T', 1e12)];

pub fn vals_norm(s: &str) -> HashSet<String> {
    let mut out = vals_raw(s);
    let b = s.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        match hum_at(b, i) {
            Some((end, val, suf)) => {
                let mult = SUF.iter().find(|(c, _)| *c == suf).unwrap().1;
                let v = py_round(val * mult) as i64;
                out.insert(v.to_string());
                i = end; // non-overlapping
            }
            None => i += 1,
        }
    }
    out
}

pub fn toks(s: &str) -> Vec<String> {
    let low = s.to_lowercase();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in low.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn tf_score(task: &str, content: &str) -> i64 {
    let terms = toks(task);
    toks(content).iter().filter(|t| terms.contains(t)).count() as i64
}

/// Python round() to integer: half-to-even (differs from Rust f64::round).
pub fn py_round(x: f64) -> f64 {
    let t = x.trunc();
    if (x - t).abs() == 0.5 {
        if (t as i64) % 2 == 0 {
            t
        } else if x > 0.0 {
            t + 1.0
        } else {
            t - 1.0
        }
    } else {
        x.round()
    }
}

/// Python round(x, dp) via correctly-rounded decimal formatting.
pub fn round_dp(x: f64, dp: usize) -> f64 {
    format!("{x:.dp$}").parse().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_int_date_extraction() {
        let s = "read /srv/app/models3.py: size=4123769 mtime=2026-08-19 cksum=a1b2c3d4e5f6 total=4.1M 9M x9M";
        let raw = vals_raw(s);
        assert!(raw.contains("4123769")); // int
        assert!(raw.contains("2026")); // int from date
        assert!(raw.contains("2026-08-19")); // date
        assert!(raw.contains("a1b2c3d4e5f6")); // hex
        assert!(!raw.contains("4.1M"));
        let norm = vals_norm(s);
        assert!(norm.contains("4100000")); // 4.1M
        assert!(norm.contains("9000000")); // 9M (space-delimited)
        assert!(!norm.contains(&"x9M".to_string()));
        assert!(norm.contains("4123769"));
    }

    #[test]
    fn tokens() {
        assert_eq!(toks("Total: 4.1M x_K"), vec!["total", "4", "1m", "x", "k"]);
    }

    #[test]
    fn round_half_even() {
        assert_eq!(py_round(270.5), 270.0);
        assert_eq!(py_round(271.5), 272.0);
        assert_eq!(py_round(2.4), 2.0);
    }
}
