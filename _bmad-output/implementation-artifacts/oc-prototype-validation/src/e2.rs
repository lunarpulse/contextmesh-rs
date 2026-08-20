//! E2 — cross-session salience propagation and prior-boosted selection vs the
//! ob-baseline-lexical-tf reimplementation. PPR runs over DAG-parent and
//! bounded entity edges; adjacency is iterated in sorted order (the Python
//! prototype iterates sets, which only perturbs float sums at the ulp level —
//! verified stable across hash seeds at 3-decimal rounding).

use crate::e1::{m0, m1n};
use crate::parse::{tf_score, vals_norm, vals_raw};
use crate::rng::PyRng;
use crate::session::{Event, Session};
use std::collections::HashMap;
use std::collections::HashSet;

fn build_adj(pool: &[&Event]) -> Vec<Vec<usize>> {
    let idx: HashMap<&str, usize> = pool
        .iter()
        .enumerate()
        .map(|(i, e)| (e.eid.as_str(), i))
        .collect();
    let mut adj: Vec<HashSet<usize>> = vec![HashSet::new(); pool.len()];
    for (i, e) in pool.iter().enumerate() {
        for p in &e.parents {
            if let Some(&j) = idx.get(p.as_str()) {
                adj[i].insert(j);
                adj[j].insert(i);
            }
        }
    }
    // Bounded entity edges in first-seen order (insertion order of Python dict).
    let mut ent_map: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut ent_order: Vec<&str> = Vec::new();
    for (i, e) in pool.iter().enumerate() {
        for x in &e.ents {
            if !ent_map.contains_key(x.as_str()) {
                ent_order.push(x.as_str());
            }
            ent_map.entry(x.as_str()).or_default().push(i);
        }
    }
    for key in ent_order {
        let eids = &ent_map[key];
        for i in 0..eids.len() {
            for j in (i + 1)..std::cmp::min(i + 8, eids.len()) {
                adj[eids[i]].insert(eids[j]);
                adj[eids[j]].insert(eids[i]);
            }
        }
    }
    adj.into_iter()
        .map(|s| {
            let mut v: Vec<usize> = s.into_iter().collect();
            v.sort_unstable();
            v
        })
        .collect()
}

fn ppr(adj: &[Vec<usize>], seeds: &HashSet<usize>, iters: usize, d: f64) -> Vec<f64> {
    let n = adj.len();
    if seeds.is_empty() {
        return vec![0.0; n];
    }
    let mut r = vec![0.0f64; n];
    let inv = 1.0 / seeds.len() as f64;
    for s in seeds {
        r[*s] = inv;
    }
    let mut cur = r.clone();
    for _ in 0..iters {
        let mut nxt: Vec<f64> = r.iter().map(|v| (1.0 - d) * v).collect();
        for i in 0..n {
            if !adj[i].is_empty() {
                let share = d * cur[i] / adj[i].len() as f64;
                for &nb in &adj[i] {
                    nxt[nb] += share;
                }
            }
        }
        cur = nxt;
    }
    cur
}

pub struct E2Out {
    pub recall6_base: f64,
    pub recall6_enh: f64,
    pub recall12_base: f64,
    pub recall12_enh: f64,
    pub prec12_base: f64,
    pub prec12_enh: f64,
    pub exp_random: f64,
    pub fail12_base: f64,
    pub fail12_enh: f64,
    pub mean_useful: f64,
    pub probes: usize,
}

pub fn run_e2(sessions: &[Session], rng: &mut PyRng, probes_n: usize) -> E2Out {
    let order: Vec<usize> = (0..sessions.len()).collect();
    let probes = rng.sample(&order, probes_n);

    let mut r6b = Vec::new();
    let mut r6e = Vec::new();
    let mut r12b = Vec::new();
    let mut r12e = Vec::new();
    let mut p12b = Vec::new();
    let mut p12e = Vec::new();
    let mut f12b = Vec::new();
    let mut f12e = Vec::new();
    let mut useful_sizes = Vec::new();
    let mut pool_sizes = Vec::new();

    for &sid in &probes {
        let s = &sessions[sid];
        let others: Vec<&Session> = sessions.iter().filter(|x| x.sid != sid).collect();
        let pool: Vec<&Event> = others
            .iter()
            .flat_map(|x| x.events.iter().filter(|e| e.kind != "answer"))
            .collect();
        let pool_idx: HashMap<&str, usize> = pool
            .iter()
            .enumerate()
            .map(|(i, e)| (e.eid.as_str(), i))
            .collect();

        // Positive seeds: load-bearing marks from completed sessions.
        let mut seeds_pos: HashSet<usize> = HashSet::new();
        for x in &others {
            let ar = vals_raw(&x.answer.content);
            let an = vals_norm(&x.answer.content);
            for e in x.events.iter().filter(|e| e.kind != "answer") {
                if m0(e, &ar) || m1n(e, &an) {
                    if let Some(&i) = pool_idx.get(e.eid.as_str()) {
                        seeds_pos.insert(i);
                    }
                }
            }
        }
        // Negative seeds: dead ends (thorns).
        let seeds_neg: HashSet<usize> = pool
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == "fail")
            .map(|(i, _)| i)
            .collect();
        // Useful for THIS task: precise identity overlap (ground truth).
        let useful: HashSet<usize> = pool
            .iter()
            .enumerate()
            .filter(|(_, e)| !vals_raw(&e.content).is_disjoint(&s.crit_identity))
            .map(|(i, _)| i)
            .collect();
        useful_sizes.push(useful.len());
        pool_sizes.push(pool.len());

        let adj = build_adj(&pool);
        let pos = ppr(&adj, &seeds_pos, 20, 0.85);
        let neg = ppr(&adj, &seeds_neg, 20, 0.85);
        let pmax = {
            let m = pos.iter().cloned().fold(0.0f64, f64::max);
            if m == 0.0 { 1.0 } else { m }
        };
        let nmax = {
            let m = neg.iter().cloned().fold(0.0f64, f64::max);
            if m == 0.0 { 1.0 } else { m }
        };

        // Baseline: lexical-TF (stable sort, descending).
        let tf: Vec<i64> = pool.iter().map(|e| tf_score(&s.task, &e.content)).collect();
        let mut base: Vec<usize> = (0..pool.len()).collect();
        base.sort_by(|&a, &b| tf[a].cmp(&tf[b]).reverse());

        // Enhanced: tf × (1 + 2·prior) × (1 − 0.6·thorn) — same association
        // order as the Python expression.
        let score = |i: usize| {
            (tf[i] as f64 * (1.0 + 2.0 * pos[i] / pmax)) * (1.0 - 0.6 * neg[i] / nmax)
        };
        let mut enh: Vec<usize> = (0..pool.len()).collect();
        enh.sort_by(|&a, &b| score(b).partial_cmp(&score(a)).unwrap());

        let fail: HashSet<usize> = pool
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == "fail")
            .map(|(i, _)| i)
            .collect();

        let den = useful.len().max(1) as f64;
        r6b.push(base[..6].iter().filter(|i| useful.contains(i)).count() as f64 / den);
        r6e.push(enh[..6].iter().filter(|i| useful.contains(i)).count() as f64 / den);
        r12b.push(base[..12].iter().filter(|i| useful.contains(i)).count() as f64 / den);
        r12e.push(enh[..12].iter().filter(|i| useful.contains(i)).count() as f64 / den);
        p12b.push(base[..12].iter().filter(|i| useful.contains(i)).count() as f64 / 12.0);
        p12e.push(enh[..12].iter().filter(|i| useful.contains(i)).count() as f64 / 12.0);
        f12b.push(base[..12].iter().filter(|i| fail.contains(i)).count() as f64 / 12.0);
        f12e.push(enh[..12].iter().filter(|i| fail.contains(i)).count() as f64 / 12.0);
    }

    let mean = |v: &Vec<f64>| v.iter().sum::<f64>() / v.len() as f64;
    let exp_rand = useful_sizes
        .iter()
        .zip(&pool_sizes)
        .map(|(u, p)| *u as f64 / *p as f64)
        .collect::<Vec<_>>();
    E2Out {
        recall6_base: mean(&r6b),
        recall6_enh: mean(&r6e),
        recall12_base: mean(&r12b),
        recall12_enh: mean(&r12e),
        prec12_base: mean(&p12b),
        prec12_enh: mean(&p12e),
        exp_random: mean(&exp_rand),
        fail12_base: mean(&f12b),
        fail12_enh: mean(&f12e),
        mean_useful: mean(&useful_sizes.iter().map(|u| *u as f64).collect::<Vec<_>>()),
        probes: probes_n,
    }
}
