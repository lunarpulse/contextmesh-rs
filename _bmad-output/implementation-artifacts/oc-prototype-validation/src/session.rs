//! Agent-session generator. Mirrors prototype.gen_session *call for call*:
//! every PyRng consumption happens at exactly the same point in the stream,
//! and every content string is byte-identical to the Python output.

use crate::parse::py_round;
use crate::rng::PyRng;
use crate::world::{FileMeta, KWS, TASKS};
use std::collections::HashSet;

#[derive(Clone)]
pub struct Event {
    pub eid: String,
    pub kind: &'static str,
    pub content: String,
    pub dur_ms: i64,
    pub parents: Vec<String>,
    pub ents: Vec<String>,
}

pub struct Session {
    pub sid: usize,
    pub task: String,
    pub mode: &'static str,
    pub events: Vec<Event>,
    /// Raw value strings the answer depends on (judge's requirement set).
    pub judge_required: HashSet<String>,
    /// Ground-truth load-bearing eids.
    pub lb_true: HashSet<String>,
    /// Ground-truth dead-end eids.
    pub dead_ends: HashSet<String>,
    pub answer: Event,
    /// size+cksum of critical files (precise identity for E2 'useful').
    pub crit_identity: HashSet<String>,
}

fn push(
    evs: &mut Vec<Event>,
    sid: usize,
    kind: &'static str,
    content: String,
    dur_ms: i64,
    ents: Vec<String>,
) -> Event {
    let eid = format!("s{sid}e{}", evs.len());
    let parents: Vec<String> = evs.last().map(|e| vec![e.eid.clone()]).unwrap_or_default();
    let e = Event { eid, kind, content, dur_ms, parents, ents };
    evs.push(e.clone());
    e
}

/// prototype.typo: delete one char (50%) or insert a vowel.
fn typo(rng: &mut PyRng, w: &str) -> String {
    let chars: Vec<char> = w.chars().collect();
    if chars.len() > 3 && rng.random() < 0.5 {
        let i = rng.randrange(1, chars.len() as u64) as usize;
        let mut s: String = chars[..i].iter().collect();
        s.extend(&chars[i + 1..]);
        return s;
    }
    let i = rng.randrange(1, chars.len() as u64) as usize;
    let vowels = ['a', 'e', 'i', 'o', 'u'];
    let c = rng.choice(&vowels);
    let mut s: String = chars[..i].iter().collect();
    s.push(c);
    s.extend(&chars[i..]);
    s
}

pub fn gen_session(sid: usize, rng: &mut PyRng, files: &[FileMeta]) -> Session {
    let (tmpl, mode) = TASKS[rng.randrange(0, 2) as usize];
    let kw: String = rng.choice(&KWS).to_string();
    let d: String = rng.choice(&crate::world::DIRS).to_string();
    let task = tmpl.replace("{kw}", &kw).replace("{d}", &d);

    let mut evs: Vec<Event> = Vec::new();

    if rng.random() < 0.5 {
        let t = typo(rng, &kw);
        let fp = format!("typo:{t}");
        let dur = rng.randint(200, 600) as i64;
        push(
            &mut evs, sid, "fail",
            format!("search '{t}' under {d}: 0 hits"), dur,
            vec![fp],
        );
    }

    let k = rng.randint(3, 6) as usize;
    let hits: Vec<FileMeta> = rng.sample(files, k);

    {
        let dur = rng.randint(80, 400) as i64;
        let content = format!(
            "search ok: {}",
            hits.iter().map(|f| f.path.as_str()).collect::<Vec<_>>().join("; ")
        );
        push(&mut evs, sid, "search", content, dur, vec![d.clone()]);
    }

    if rng.random() < 0.3 {
        let dur = rng.randint(300, 900) as i64;
        push(
            &mut evs, sid, "fail",
            "read /etc/secure/vault.key: permission denied".to_string(), dur,
            vec!["perm:denied".to_string()],
        );
    }

    let mut crit: Vec<FileMeta> = hits
        .iter()
        .filter(|_| rng.random() < 0.6)
        .cloned()
        .collect();
    if crit.is_empty() {
        crit.push(hits[0].clone());
    }

    for f in &hits {
        let dur = rng.randint(100, 700) as i64;
        push(
            &mut evs, sid, "read",
            format!("read {}: size={} mtime={} cksum={}", f.path, f.size, f.mtime, f.cksum),
            dur, vec![f.path.clone()],
        );
    }

    let inc_total = rng.random() < 0.75;
    let total: i64 = {
        let s: i64 = crit.iter().map(|f| f.size).sum();
        (py_round(s as f64 / 100_000.0) * 100_000.0) as i64
    };
    if inc_total {
        let dur = rng.randint(150, 900) as i64;
        push(
            &mut evs, sid, "compute",
            format!("du -h {d}: total {total} bytes across {} files", crit.len()), dur,
            vec![d.clone()],
        );
        if rng.random() < 0.35 {
            let dur = rng.randint(100, 400) as i64;
            push(
                &mut evs, sid, "verify",
                format!("verify footprint: du total {total} bytes confirmed"), dur,
                vec![d.clone()],
            );
        }
    }

    let human = inc_total && rng.random() < 0.5;

    let mut parts: Vec<String> = crit
        .iter()
        .map(|f| format!("{} size={} mtime={}", f.path, f.size, f.mtime))
        .collect();
    if inc_total {
        let tstr = if human {
            format!("{:.1}M", total as f64 / 1e6)
        } else {
            total.to_string()
        };
        parts.push(format!("total={tstr}"));
    }
    let dur = rng.randint(200, 800) as i64;
    let answer = push(
        &mut evs, sid, "answer",
        format!("ANSWER: {}", parts.join("; ")), dur,
        vec![d.clone()],
    );

    let mut required: HashSet<String> = HashSet::new();
    let mut lb: HashSet<String> = HashSet::new();
    let mut crit_identity: HashSet<String> = HashSet::new();
    for f in &crit {
        required.insert(f.size.to_string());
        required.insert(f.mtime.to_string());
        required.insert(f.cksum.clone());
        crit_identity.insert(f.size.to_string());
        crit_identity.insert(f.cksum.clone());
        for e in &evs {
            if e.kind == "read" && e.content.contains(&f.path) {
                lb.insert(e.eid.clone());
            }
        }
    }
    if inc_total {
        required.insert(total.to_string());
        for e in &evs {
            if e.kind == "compute" || e.kind == "verify" {
                lb.insert(e.eid.clone());
            }
        }
    }
    let dead: HashSet<String> = evs
        .iter()
        .filter(|e| e.kind == "fail")
        .map(|e| e.eid.clone())
        .collect();

    Session {
        sid,
        task,
        mode,
        events: evs,
        judge_required: required,
        lb_true: lb,
        dead_ends: dead,
        answer,
        crit_identity,
    }
}
