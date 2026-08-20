//! Static world (same constants as the Python prototype) + file universe.

use crate::rng::PyRng;

pub const SEED: u64 = 20260820;

pub const DIRS: [&str; 4] = ["/srv/app", "/srv/db", "/srv/core", "/opt/edge"];
pub const FNAMES: [&str; 15] = [
    "models.py", "cache.py", "router.py", "ledger.db", "config.yaml", "index.ts", "queue.rs",
    "metrics.py", "auth.py", "dump.json", "sync.rs", "vault.md", "traces.log", "tokens.txt",
    "hooks.rs",
];
pub const MTIMES: [&str; 28] = [
    "2026-08-01", "2026-08-02", "2026-08-03", "2026-08-04", "2026-08-05", "2026-08-06",
    "2026-08-07", "2026-08-08", "2026-08-09", "2026-08-10", "2026-08-11", "2026-08-12",
    "2026-08-13", "2026-08-14", "2026-08-15", "2026-08-16", "2026-08-17", "2026-08-18",
    "2026-08-19", "2026-08-20", "2026-08-21", "2026-08-22", "2026-08-23", "2026-08-24",
    "2026-08-25", "2026-08-26", "2026-08-27", "2026-08-28",
];
pub const KWS: [&str; 5] = ["cache", "tokens", "traces", "ledger", "queue"];

/// (template, mode): 'syn' wording mismatches event vocabulary; 'lit' shares it.
pub const TASKS: [(&str, &str); 2] = [
    (
        "locate files for {kw} under {d} and report storage footprint and modified date",
        "syn",
    ),
    ("find {kw} files under {d} and report size and mtime", "lit"),
];

#[derive(Clone)]
pub struct FileMeta {
    pub path: String,
    pub size: i64,
    pub mtime: &'static str,
    pub cksum: String,
}

/// Mirrors prototype._mkfile: one private Random(SEED + 31*i) per file, calls in
/// path/size/mtime/cksum evaluation order (choice, choice, choice, randint,
/// choice, getrandbits(48)).
pub fn files() -> Vec<FileMeta> {
    let mut out = Vec::with_capacity(120);
    for i in 0..120u64 {
        let mut r = PyRng::new(SEED + 31 * i);
        let d: &str = r.choice(&DIRS);
        let f1: &str = r.choice(&FNAMES);
        let f2: &str = r.choice(&FNAMES);
        let size = r.randint(1_000, 9_500_000) as i64;
        let mtime: &'static str = r.choice(&MTIMES);
        let ck = r.getrandbits(48);
        let base = f1.split('.').next().unwrap();
        let ext = f2.rsplit('.').next().unwrap();
        out.push(FileMeta {
            path: format!("{d}/{base}{i}.{ext}"),
            size,
            mtime,
            cksum: format!("{ck:012x}"),
        });
    }
    assert_eq!(out.iter().map(|f| f.path.clone()).collect::<std::collections::HashSet<_>>().len(), 120);
    out
}
