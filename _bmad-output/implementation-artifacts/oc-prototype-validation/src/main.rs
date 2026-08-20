//! contextmesh-salience — Option C (Attention Ledger) validation prototype,
//! Rust port of prototype.py. Zero dependencies; reproduces the Python run
//! bit-for-bit on E1/E3 (E2 within float ulps of set-iteration order).

mod e1;
mod e2;
mod e3;
mod parse;
mod rng;
mod session;
mod world;

use parse::round_dp;
use rng::PyRng;
use std::time::Instant;

fn main() {
    let t0 = Instant::now();
    let s_count = 48usize;

    let mut rng = PyRng::new(world::SEED);
    let files = world::files();
    let sessions: Vec<session::Session> = (0..s_count)
        .map(|i| session::gen_session(i, &mut rng, &files))
        .collect();
    let events_total: usize = sessions.iter().map(|s| s.events.len()).sum();

    let rows = e1::run_e1(&sessions, &mut rng);
    let e2 = e2::run_e2(&sessions, &mut rng, 12);
    let e3 = e3::run_e3(&sessions);

    // ---- JSON emit (shape mirrors prototype.py output) ----
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"seed\": {},\n", world::SEED));
    out.push_str(&format!("  \"sessions\": {},\n", s_count));
    out.push_str(&format!("  \"events_total\": {},\n", events_total));
    out.push_str("  \"E1_attribution_ladder\": [\n");
    for (i, r) in rows.iter().enumerate() {
        let tp = r.tp as f64;
        let p = tp / (tp + r.fp as f64).max(1.0);
        let rc = tp / (tp + r.fn_ as f64).max(1.0);
        let f1 = 2.0 * p * rc / (p + rc).max(1e-9);
        let jc = r.judge_calls as f64 / s_count as f64;
        out.push_str(&format!(
            "    {{\"mech\": \"{}\", \"precision\": {}, \"recall\": {}, \"f1\": {}, \"judge_calls_per_session\": {}}}{}\n",
            r.mech,
            round_dp(p, 3),
            round_dp(rc, 3),
            round_dp(f1, 3),
            round_dp(jc, 1),
            if i + 1 < rows.len() { "," } else { "" }
        ));
    }
    out.push_str("  ],\n");
    out.push_str("  \"E2_propagation_selection\": {\n");
    out.push_str(&format!(
        "    \"recall@6_base\": {}, \"recall@6_enh\": {},\n    \"recall@12_base\": {}, \"recall@12_enh\": {},\n    \"precision@12_base\": {}, \"precision@12_enh\": {},\n    \"expected_random_precision\": {},\n    \"fail_share_top12_base\": {}, \"fail_share_top12_enh\": {},\n    \"mean_useful_pool\": {}, \"probes\": {}\n  }},\n",
        round_dp(e2.recall6_base, 3),
        round_dp(e2.recall6_enh, 3),
        round_dp(e2.recall12_base, 3),
        round_dp(e2.recall12_enh, 3),
        round_dp(e2.prec12_base, 3),
        round_dp(e2.prec12_enh, 3),
        round_dp(e2.exp_random, 3),
        round_dp(e2.fail12_base, 3),
        round_dp(e2.fail12_enh, 3),
        round_dp(e2.mean_useful, 3),
        e2.probes
    ));
    out.push_str(&format!(
        "  \"E3_cost_ledger\": {{\"useful_ms\": {}, \"dead_ms\": {}, \"noise_ms\": {}, \"wasted_pct\": {}}},\n",
        round_dp(e3.useful_ms / 1000.0, 1),
        round_dp(e3.dead_ms / 1000.0, 1),
        round_dp(e3.noise_ms / 1000.0, 1),
        round_dp(e3.wasted_pct, 1)
    ));
    out.push_str(&format!("  \"wall_s\": {}\n", round_dp(t0.elapsed().as_secs_f64(), 2)));
    out.push_str("}\n");

    print!("{out}");
    std::fs::write("results-rust.json", &out).expect("write results-rust.json");
}
