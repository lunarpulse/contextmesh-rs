//! E1 — attribution mechanism ladder vs ground truth.
//! M0 raw string-overlap, M1n normalized nomination, M3 leave-one-out
//! counterfactual, M4 Shapley-sampling coalition attribution.

use crate::parse::{vals_norm, vals_raw};
use crate::rng::PyRng;
use crate::session::{Event, Session};
use std::collections::HashMap;
use std::collections::HashSet;

pub struct Row {
    pub mech: &'static str,
    pub tp: u64,
    pub fp: u64,
    pub fn_: u64,
    pub judge_calls: u64,
}

pub fn judge(events: &[&Event], required: &HashSet<String>, calls: &mut u64) -> bool {
    *calls += 1;
    let mut have: HashSet<String> = HashSet::new();
    for e in events {
        have.extend(vals_raw(&e.content));
    }
    required.is_subset(&have)
}

pub fn m0(e: &Event, answer_vals: &HashSet<String>) -> bool {
    e.kind != "answer" && !vals_raw(&e.content).is_disjoint(answer_vals)
}

pub fn m1n(e: &Event, answer_nvals: &HashSet<String>) -> bool {
    e.kind != "answer" && !vals_norm(&e.content).is_disjoint(answer_nvals)
}

pub fn run_e1(sessions: &[Session], rng: &mut PyRng) -> Vec<Row> {
    const MECHS: [&str; 5] = ["M0", "M1n", "M3", "M4", "M0+M1n+M4"];
    let mut rows: Vec<Row> = MECHS
        .iter()
        .map(|m| Row { mech: m, tp: 0, fp: 0, fn_: 0, judge_calls: 0 })
        .collect();

    for s in sessions {
        let cands: Vec<&Event> = s.events.iter().filter(|e| e.kind != "answer").collect();
        let a_raw = vals_raw(&s.answer.content);
        let a_norm = vals_norm(&s.answer.content);

        let mut marks: Vec<HashSet<String>> = vec![HashSet::new(); MECHS.len()];
        marks[0] = cands.iter().filter(|e| m0(e, &a_raw)).map(|e| e.eid.clone()).collect();
        marks[1] = cands.iter().filter(|e| m1n(e, &a_norm)).map(|e| e.eid.clone()).collect();

        // M3: leave-one-out counterfactual over all candidates.
        let mut calls3 = 0u64;
        let mut m3 = HashSet::new();
        for (i, _) in cands.iter().enumerate() {
            let others: Vec<&Event> = cands
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, e)| *e)
                .collect();
            if !judge(&others, &s.judge_required, &mut calls3) {
                m3.insert(cands[i].eid.clone());
            }
        }
        marks[2] = m3;
        rows[2].judge_calls += calls3;

        // M4: Shapley sampling over the nominated shortlist (M0 ∪ M1n ∪ M3).
        let nominated: HashSet<&str> = marks[0]
            .union(&marks[1])
            .chain(marks[2].iter())
            .map(|s| s.as_str())
            .collect();
        let shortlist: Vec<&Event> = cands
            .iter()
            .filter(|e| nominated.contains(e.eid.as_str()))
            .copied()
            .collect();
        let mut calls4 = 0u64;
        let mut phi: HashMap<&str, f64> = shortlist.iter().map(|e| (e.eid.as_str(), 0.0f64)).collect();
        if !shortlist.is_empty() {
            for _ in 0..64 {
                let mut perm = shortlist.clone();
                rng.shuffle(&mut perm);
                let mut ctx: Vec<&Event> = Vec::new();
                let mut ok = false;
                for e in perm {
                    let prev = ok;
                    ctx.push(e);
                    ok = judge(&ctx, &s.judge_required, &mut calls4);
                    if !prev && ok {
                        *phi.get_mut(e.eid.as_str()).unwrap() += 1.0 / 64.0;
                        break; // coverage game: no further flips matter
                    }
                }
            }
        }
        marks[3] = phi
            .into_iter()
            .filter(|(_, p)| *p > 0.015)
            .map(|(k, _)| k.to_string())
            .collect();
        rows[3].judge_calls += calls4;

        let union_row: HashSet<String> = marks[0]
            .union(&marks[1])
            .cloned()
            .chain(marks[3].iter().cloned())
            .collect();
        marks[4] = union_row;

        for (i, mk) in marks.iter().enumerate() {
            rows[i].tp += mk.intersection(&s.lb_true).count() as u64;
            rows[i].fp += mk.difference(&s.lb_true).count() as u64;
            rows[i].fn_ += s.lb_true.difference(mk).count() as u64;
        }
    }
    rows
}
