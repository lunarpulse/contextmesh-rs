#!/usr/bin/env python3
"""
Option C — Salience Provenance Layer ("Attention Ledger") for contextmesh-rs.
Python validation prototype. Pure stdlib, seeded, offline, deterministic.

  E1  Attribution mechanism ladder vs ground truth:
      M0  raw string-overlap backtracking
      M1n numeric-normalizing nomination (humanized formats)
      M3  single-event counterfactual ablation   (logical judge calls counted)
      M4  Shapley-sampling coalition attribution (logical judge calls counted)
  E2  Cross-session salience propagation: recall@budget,
      ob-baseline-lexical-tf (reimplemented) vs prior-boosted selection.
  E3  Cost ledger: useful vs wasted effort; judge-call economics.
"""
from __future__ import annotations

import json
import random
import re
import time
from dataclasses import dataclass, field

SEED = 20260820
rng = random.Random(SEED)

# ---------------------------------------------------------------- 1. world --

DIRS = ["/srv/app", "/srv/db", "/srv/core", "/opt/edge"]
FNAMES = ["models.py", "cache.py", "router.py", "ledger.db", "config.yaml",
          "index.ts", "queue.rs", "metrics.py", "auth.py", "dump.json",
          "sync.rs", "vault.md", "traces.log", "tokens.txt", "hooks.rs"]
MTIMES = [f"2026-08-{d:02d}" for d in range(1, 29)]
KWS = ["cache", "tokens", "traces", "ledger", "queue"]

def _mkfile(i: int) -> dict:
    r = random.Random(SEED + 31 * i)
    return {
        "path": f"{r.choice(DIRS)}/{r.choice(FNAMES).rsplit('.', 1)[0]}{i}.{r.choice(FNAMES).rsplit('.', 1)[1]}",
        "size": r.randint(1_000, 9_500_000),
        "mtime": r.choice(MTIMES),
        "cksum": f"{r.getrandbits(48):012x}",
    }

FILES = [_mkfile(i) for i in range(120)]
assert len({f["path"] for f in FILES}) == 120  # unique world paths

# 'syn' phrasing deliberately mismatches event vocabulary (no literal overlap);
# 'lit' phrasing shares tokens (size, mtime) with event contents.
TASKS = [
    ("locate files for {kw} under {d} and report storage footprint and modified date", "syn"),
    ("find {kw} files under {d} and report size and mtime", "lit"),
]

# ------------------------------------------------------------- 2. sessions --

@dataclass
class Event:
    eid: str
    kind: str            # search | read | compute | verify | fail | answer
    content: str
    dur_ms: int
    parents: list = field(default_factory=list)
    ents: set = field(default_factory=set)
    fp: str | None = None  # failure fingerprint

@dataclass
class Session:
    sid: int
    task: str
    mode: str
    events: list
    judge_required: set   # raw value strings the answer depends on
    lb_true: set          # ground-truth load-bearing eids
    dead_ends: set        # ground-truth dead-end eids
    answer: Event
    crit_identity: set = field(default_factory=set)  # size+cksum of critical files

def typo(w: str) -> str:
    if len(w) > 3 and rng.random() < 0.5:
        i = rng.randrange(1, len(w)); return w[:i] + w[i + 1:]
    i = rng.randrange(1, len(w)); return w[:i] + rng.choice("aeiou") + w[i:]

def gen_session(sid: int) -> Session:
    tmpl, mode = TASKS[rng.randrange(2)]
    kw, d = rng.choice(KWS), rng.choice(DIRS)
    task = tmpl.format(kw=kw, d=d)
    evs: list[Event] = []

    def add(kind, content, dur, ents, fp=None):
        eid = f"s{sid}e{len(evs)}"
        e = Event(eid, kind, content, dur, [evs[-1].eid] if evs else [], set(ents), fp)
        evs.append(e); return e

    if rng.random() < 0.5:  # dead end: typo'd query
        t = typo(kw)
        add("fail", f"search '{t}' under {d}: 0 hits", rng.randint(200, 600), [f"typo:{t}"], f"typo:{t}")
    hits = rng.sample(FILES, rng.randint(3, 6))
    add("search", "search ok: " + "; ".join(f["path"] for f in hits), rng.randint(80, 400), [d])
    if rng.random() < 0.3:  # dead end: locked path
        add("fail", "read /etc/secure/vault.key: permission denied", rng.randint(300, 900),
            ["perm:denied"], "perm:denied")

    crit = [f for f in hits if rng.random() < 0.6] or [hits[0]]
    for f in hits:
        add("read", f"read {f['path']}: size={f['size']} mtime={f['mtime']} cksum={f['cksum']}",
            rng.randint(100, 700), [f["path"]])

    inc_total = rng.random() < 0.75
    total = int(round(sum(f["size"] for f in crit) / 100_000) * 100_000)
    if inc_total:
        add("compute", f"du -h {d}: total {total} bytes across {len(crit)} files",
            rng.randint(150, 900), [d])
        if rng.random() < 0.35:  # redundant second carrier of the same fact
            add("verify", f"verify footprint: du total {total} bytes confirmed",
                rng.randint(100, 400), [d])

    human = inc_total and rng.random() < 0.5  # answer reformats the total
    parts = [f"{f['path']} size={f['size']} mtime={f['mtime']}" for f in crit]
    if inc_total:
        parts.append(f"total={'%.1fM' % (total / 1e6) if human else str(total)}")
    answer = add("answer", "ANSWER: " + "; ".join(parts), rng.randint(200, 800), [d])

    required, lb = set(), set()
    crit_identity = set()  # precise file identity: size + cksum of critical files
    for f in crit:
        required |= {str(f["size"]), f["mtime"], f["cksum"]}
        crit_identity |= {str(f["size"]), f["cksum"]}
        lb |= {e.eid for e in evs if e.kind == "read" and f["path"] in e.content}
    if inc_total:
        required.add(str(total))
        lb |= {e.eid for e in evs if e.kind in ("compute", "verify")}
    dead = {e.eid for e in evs if e.kind == "fail"}
    return Session(sid, task, mode, evs, required, lb, dead, answer, crit_identity)

# ------------------------------------------------------- 3. value parsing --

RE_HEX = re.compile(r"\b[0-9a-f]{8,}\b")
RE_INT = re.compile(r"\b\d{3,}\b")
RE_DATE = re.compile(r"\b\d{4}-\d{2}-\d{2}\b")
RE_HUM = re.compile(r"\b(\d+(?:\.\d+)?)([KMGT])\b")
SUF = {"K": 1e3, "M": 1e6, "G": 1e9, "T": 1e12}

def vals_raw(s: str) -> set:
    return set(RE_HEX.findall(s)) | set(RE_INT.findall(s)) | set(RE_DATE.findall(s))

def vals_norm(s: str) -> set:
    v = set(RE_INT.findall(s)) | set(RE_DATE.findall(s)) | set(RE_HEX.findall(s))
    v |= {str(int(round(float(n) * SUF[su]))) for n, su in RE_HUM.findall(s)}
    return v

def toks(s: str) -> list:
    return re.findall(r"[a-z0-9]+", s.lower())

# ------------------------------------------------- 4. mechanism ladder (E1) --

JUDGE_CALLS = 0

def judge(events: list, required: set) -> bool:
    """Deterministic simulated recipient: succeeds iff every required raw
    value is present in some context event. Counts logical judge calls."""
    global JUDGE_CALLS
    JUDGE_CALLS += 1
    have = set()
    for e in events:
        have |= vals_raw(e.content)
    return required <= have

def m0(ev: Event, answer_vals: set) -> bool:
    return ev.kind != "answer" and bool(vals_raw(ev.content) & answer_vals)

def m1n(ev: Event, answer_nvals: set) -> bool:
    return ev.kind != "answer" and bool(vals_norm(ev.content) & answer_nvals)

def run_e1(sessions):
    global JUDGE_CALLS
    rows = {k: {"tp": 0, "fp": 0, "fn": 0} for k in ("M0", "M1n", "M3", "M4", "M0+M1n+M4")}
    judge_calls = {k: 0 for k in ("M3", "M4")}
    for s in sessions:
        cands = [e for e in s.events if e.kind != "answer"]
        a_raw = vals_raw(s.answer.content)
        a_norm = vals_norm(s.answer.content)
        marks = {}
        marks["M0"] = {e.eid for e in cands if m0(e, a_raw)}
        marks["M1n"] = {e.eid for e in cands if m1n(e, a_norm)}
        # M3: leave-one-out counterfactual over ALL candidates
        JUDGE_CALLS = 0
        m3 = {e.eid for e in cands if not judge([x for x in cands if x is not e], s.judge_required)}
        judge_calls["M3"] += JUDGE_CALLS
        marks["M3"] = m3
        # M4: Shapley sampling over the nominated shortlist (M0 ∪ M1n ∪ M3)
        shortlist = [e for e in cands if e.eid in (marks["M0"] | marks["M1n"] | m3)]
        JUDGE_CALLS = 0
        phi = {e.eid: 0.0 for e in shortlist}
        m = 64
        if shortlist:
            for _ in range(m):
                perm = shortlist[:]; rng.shuffle(perm)
                ctx: list = []
                ok = judge(ctx, s.judge_required) if False else False
                for e in perm:
                    prev = ok
                    ctx.append(e)
                    ok = judge(ctx, s.judge_required)
                    if not prev and ok:
                        phi[e.eid] += 1.0 / m
                        break  # coverage game: no further flips matter
        judge_calls["M4"] += JUDGE_CALLS
        marks["M4"] = {eid for eid, p in phi.items() if p > 0.015}
        marks["M0+M1n+M4"] = marks["M0"] | marks["M1n"] | marks["M4"]
        for k, mk in marks.items():
            rows[k]["tp"] += len(mk & s.lb_true)
            rows[k]["fp"] += len(mk - s.lb_true)
            rows[k]["fn"] += len(s.lb_true - mk)
    out = []
    for k, r in rows.items():
        p = r["tp"] / max(1, r["tp"] + r["fp"]); rc = r["tp"] / max(1, r["tp"] + r["fn"])
        f1 = 2 * p * rc / max(1e-9, p + rc)
        out.append({"mech": k, "precision": round(p, 3), "recall": round(rc, 3),
                    "f1": round(f1, 3),
                    "judge_calls_per_session": round(judge_calls.get(k, 0) / len(sessions), 1)})
    return out

# ------------------------------------------- 5. propagation + selection (E2) --

def tf_score(task: str, content: str) -> int:
    terms = toks(task)
    ct = toks(content)
    return sum(1 for t in ct if t in terms)

def build_graph(pool: list) -> dict:
    adj: dict[str, set] = {}
    by_eid = {e.eid: e for e in pool}
    for e in pool:
        adj.setdefault(e.eid, set())
    for e in pool:
        for p in e.parents:
            if p in by_eid:
                adj[e.eid].add(p); adj[p].add(e.eid)
    ent_map: dict[str, list] = {}
    for e in pool:
        for x in e.ents:
            ent_map.setdefault(x, []).append(e.eid)
    for x, eids in ent_map.items():
        for i in range(len(eids)):
            for j in range(i + 1, min(i + 8, len(eids))):  # bounded entity edges
                adj[eids[i]].add(eids[j]); adj[eids[j]].add(eids[i])
    return adj

def ppr(adj: dict, seeds: set, iters=20, d=0.85) -> dict:
    if not seeds:
        return {n: 0.0 for n in adj}
    r = {n: 0.0 for n in adj}
    for s in seeds:
        r[s] = 1.0 / len(seeds)
    cur = dict(r)
    for _ in range(iters):
        nxt = {n: (1 - d) * r.get(n, 0.0) for n in adj}
        for n in adj:
            if adj[n]:
                share = d * cur[n] / len(adj[n])
                for nb in adj[n]:
                    nxt[nb] += share
        cur = nxt
    return cur

def run_e2(sessions, probes_n=12, budgets=(6, 12)):
    probes = rng.sample(range(len(sessions)), probes_n)
    res = {"baseline": {b: [] for b in budgets}, "enhanced": {b: [] for b in budgets},
           "fail_share_base": [], "fail_share_enh": [], "useful_size": [], "pool_size": [],
           "prec_base": [], "prec_enh": []}
    for sid in probes:
        s = sessions[sid]
        others = [x for x in sessions if x.sid != sid]
        pool = [e for x in others for e in x.events if e.kind != "answer"]
        seeds_pos = {e.eid for x in others for e in x.events if e.kind != "answer"
                     and (m0(e, vals_raw(x.answer.content)) or m1n(e, vals_norm(x.answer.content)))}
        seeds_neg = {e.eid for e in pool if e.kind == "fail"}
        useful = {e.eid for e in pool if vals_raw(e.content) & s.crit_identity}
        res["useful_size"].append(len(useful))
        res["pool_size"].append(len(pool))
        adj = build_graph(pool)
        pos = ppr(adj, seeds_pos); neg = ppr(adj, seeds_neg)
        pmax = max(pos.values()) or 1.0; nmax = max(neg.values()) or 1.0
        base = sorted(pool, key=lambda e: -tf_score(s.task, e.content))
        enh = sorted(pool, key=lambda e: -(tf_score(s.task, e.content)
                                           * (1 + 2.0 * pos[e.eid] / pmax)
                                           * (1 - 0.6 * neg[e.eid] / nmax)))
        fail_eids = {e.eid for e in pool if e.kind == "fail"}
        for b in budgets:
            res["baseline"][b].append(len({e.eid for e in base[:b]} & useful) / max(1, len(useful)))
            res["enhanced"][b].append(len({e.eid for e in enh[:b]} & useful) / max(1, len(useful)))
        res["fail_share_base"].append(len({e.eid for e in base[:12]} & fail_eids) / 12)
        res["fail_share_enh"].append(len({e.eid for e in enh[:12]} & fail_eids) / 12)
        res["prec_base"].append(len({e.eid for e in base[:12]} & useful) / 12)
        res["prec_enh"].append(len({e.eid for e in enh[:12]} & useful) / 12)
    def mean(v): return round(sum(v) / len(v), 3)
    exp_rand = mean([u / p for u, p in zip(res["useful_size"], res["pool_size"])])
    return {"recall@6_base": mean(res["baseline"][6]), "recall@6_enh": mean(res["enhanced"][6]),
            "recall@12_base": mean(res["baseline"][12]), "recall@12_enh": mean(res["enhanced"][12]),
            "precision@12_base": mean(res["prec_base"]), "precision@12_enh": mean(res["prec_enh"]),
            "expected_random_precision": exp_rand,
            "fail_share_top12_base": mean(res["fail_share_base"]),
            "fail_share_top12_enh": mean(res["fail_share_enh"]),
            "mean_useful_pool": mean(res["useful_size"]), "probes": probes_n}

# ------------------------------------------------------- 6. cost ledger (E3) --

def run_e3(sessions):
    agg = {"useful_ms": 0, "dead_ms": 0, "noise_ms": 0}
    for s in sessions:
        for e in s.events:
            if e.eid in s.lb_true: agg["useful_ms"] += e.dur_ms
            elif e.eid in s.dead_ends: agg["dead_ms"] += e.dur_ms
            else: agg["noise_ms"] += e.dur_ms
    tot = sum(agg.values())
    return {k: round(v / 1000, 1) for k, v in agg.items()} | {
        "wasted_pct": round(100 * (agg["dead_ms"] + agg["noise_ms"]) / tot, 1)}

# ------------------------------------------------------------------- main --

def main():
    t0 = time.time()
    S = 48
    sessions = [gen_session(i) for i in range(S)]
    e1 = run_e1(sessions)
    e2 = run_e2(sessions)
    e3 = run_e3(sessions)
    out = {"seed": SEED, "sessions": S, "events_total": sum(len(s.events) for s in sessions),
           "E1_attribution_ladder": e1, "E2_propagation_selection": e2, "E3_cost_ledger": e3,
           "wall_s": round(time.time() - t0, 2)}
    print(json.dumps(out, indent=2))
    with open("results.json", "w") as f:
        json.dump(out, f, indent=2)

if __name__ == "__main__":
    main()
