//! E3 — cost ledger: useful vs wasted (dead ends + noise) wall-clock effort.

use crate::session::Session;

pub struct E3Out {
    pub useful_ms: f64,
    pub dead_ms: f64,
    pub noise_ms: f64,
    pub wasted_pct: f64,
}

pub fn run_e3(sessions: &[Session]) -> E3Out {
    let mut useful = 0i64;
    let mut dead = 0i64;
    let mut noise = 0i64;
    for s in sessions {
        for e in &s.events {
            if s.lb_true.contains(&e.eid) {
                useful += e.dur_ms;
            } else if s.dead_ends.contains(&e.eid) {
                dead += e.dur_ms;
            } else {
                noise += e.dur_ms;
            }
        }
    }
    let total = useful + dead + noise;
    E3Out {
        useful_ms: useful as f64,
        dead_ms: dead as f64,
        noise_ms: noise as f64,
        wasted_pct: 100.0 * (dead + noise) as f64 / total as f64,
    }
}
