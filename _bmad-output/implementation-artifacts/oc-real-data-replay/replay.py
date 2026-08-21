#!/usr/bin/env python3
"""Privacy-preserving OC-0.5 replay over a Hermes state.db snapshot.

The harness never writes message text, user/session IDs, paths, URLs, tool
arguments, reasoning, or credentials. Outputs are aggregate metrics and a
reproduction manifest only. It uses Python stdlib and performs no network I/O.
"""
from __future__ import annotations

import argparse
import collections
import datetime as dt
import hashlib
import json
import math
import os
import random
import re
import sqlite3
import statistics
import sys
from pathlib import Path
from typing import Iterable

SEED = 20260820
M0_VERSION = "oc-prototype-m0-v1-compatible"
M1_VERSION = "oc-prototype-m1n-v1-compatible"
ENTITY_VERSION = "oc-real-replay-entity-v1"
SPLIT_POLICY = "temporal-family-60-20-20"
PPR_ITERS = 20
PPR_DAMPING = 0.85
ALPHA = 2.0
BETA = 0.6
BUDGETS = (6, 12)

HEX_RE = re.compile(r"(?<![A-Za-z0-9_])[0-9a-f]{8,}(?![A-Za-z0-9_])")
INT_RE = re.compile(r"(?<![A-Za-z0-9_])-?\d{3,}(?![A-Za-z0-9_])")
DATE_RE = re.compile(r"(?<!\d)(?:20\d{2})[-/.](?:0?[1-9]|1[0-2])[-/.](?:0?[1-9]|[12]\d|3[01])(?!\d)")
HUMAN_RE = re.compile(r"(?<![A-Za-z0-9_])(-?\d+(?:\.\d+)?)\s*([kKmMgGtT])(?![A-Za-z0-9_])")
URL_RE = re.compile(r"https?://[^\s<>\]\[\"']+")
PATH_RE = re.compile(r"(?<![A-Za-z0-9_])(?:~?/|\./|\.\./)[A-Za-z0-9_./@+~:-]{3,}")
SYMBOL_RE = re.compile(r"(?<![A-Za-z0-9_])(?:[A-Za-z_][A-Za-z0-9_]{2,}::)+[A-Za-z_][A-Za-z0-9_]*(?![A-Za-z0-9_])")
WORD_RE = re.compile(r"[A-Za-z0-9]+")
ERROR_RE = re.compile(r"(?i)(?:\berror\b|\bfailed\b|traceback|exception|exit[_ ]?code\s*[:=]?\s*[1-9]|not found|permission denied|timed out)")


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def stable_hash(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8", "replace")).hexdigest()


def snapshot(source: Path, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.exists():
        dest.unlink()
    src = sqlite3.connect(f"file:{source}?mode=ro", uri=True)
    dst = sqlite3.connect(dest)
    try:
        src.backup(dst)
        row = dst.execute("PRAGMA integrity_check").fetchone()
        if not row or row[0] != "ok":
            raise RuntimeError(f"snapshot integrity_check failed: {row}")
    finally:
        dst.close()
        src.close()
    os.chmod(dest, 0o600)
    print(json.dumps({"snapshot": str(dest), "sha256": sha256_file(dest), "integrity": "ok"}, indent=2))


def raw_values(text: str) -> set[str]:
    text = text or ""
    out = {m.group(0).lower() for m in HEX_RE.finditer(text)}
    out.update(m.group(0) for m in INT_RE.finditer(text))
    out.update(m.group(0).replace("/", "-").replace(".", "-") for m in DATE_RE.finditer(text))
    return out


def normalized_values(text: str) -> set[str]:
    out = raw_values(text)
    for m in HUMAN_RE.finditer(text or ""):
        num = float(m.group(1))
        factor = {"k": 1_000, "m": 1_000_000, "g": 1_000_000_000, "t": 1_000_000_000_000}[m.group(2).lower()]
        val = num * factor
        out.add(str(int(val)) if val.is_integer() else format(val, ".12g"))
    return out


def tf_tokens(text: str) -> list[str]:
    return [x.lower() for x in WORD_RE.findall(text or "")]


def entity_keys(text: str, tool_name: str | None) -> set[str]:
    """Return opaque local fingerprints; caller never serializes raw entities."""
    vals: set[str] = set()
    if tool_name:
        vals.add("tool:" + tool_name.lower())
    for u in URL_RE.findall(text or ""):
        vals.add("url:" + u.rstrip(".,);"))
    for p in PATH_RE.findall(text or ""):
        vals.add("path:" + p.rstrip(".,);"))
    for s in SYMBOL_RE.findall(text or ""):
        vals.add("symbol:" + s)
    for v in normalized_values(text or ""):
        if len(v) >= 4:
            vals.add("value:" + v)
    return {stable_hash(v) for v in vals}


def family_map(con: sqlite3.Connection) -> dict[str, str]:
    rows = con.execute("SELECT id,parent_session_id FROM sessions").fetchall()
    parent = {r[0]: r[1] for r in rows}
    memo: dict[str, str] = {}
    visiting: set[str] = set()
    def root(sid: str) -> str:
        if sid in memo:
            return memo[sid]
        if sid in visiting:
            memo[sid] = sid
            return sid
        visiting.add(sid)
        p = parent.get(sid)
        result = root(p) if p in parent else sid
        visiting.remove(sid)
        memo[sid] = result
        return result
    return {sid: root(sid) for sid in parent}


def load_episodes(con: sqlite3.Connection, fam: dict[str, str]) -> tuple[list[dict], list[dict]]:
    rows = con.execute(
        "SELECT id,session_id,role,content,tool_call_id,tool_name,timestamp,finish_reason "
        "FROM messages WHERE active=1 ORDER BY session_id,timestamp,id"
    ).fetchall()
    by_session: dict[str, list] = collections.defaultdict(list)
    for r in rows:
        by_session[r[1]].append(r)
    episodes: list[dict] = []
    tools_all: list[dict] = []
    for sid, msgs in by_session.items():
        start = 0
        terminal_no = 0
        for i, m in enumerate(msgs):
            if m[2] == "assistant" and m[7] == "stop" and (m[3] or "").strip():
                segment = msgs[start:i+1]
                users = [x for x in segment if x[2] == "user" and (x[3] or "").strip()]
                tools = [x for x in segment if x[2] == "tool" and (x[3] or "").strip()]
                if users:
                    task = "\n".join(x[3] for x in users)
                    answer = m[3]
                    ep_tools = []
                    for x in tools:
                        item = {
                            "id": x[0], "session": sid, "family": fam.get(sid, sid),
                            "timestamp": float(x[6]), "content": x[3], "tool_name": x[5],
                            "entities": entity_keys(x[3], x[5]),
                        }
                        ep_tools.append(item)
                        tools_all.append(item)
                    episodes.append({
                        "session": sid, "family": fam.get(sid, sid), "terminal_no": terminal_no,
                        "timestamp": float(m[6]), "task": task, "answer": answer,
                        "tools": ep_tools,
                    })
                    terminal_no += 1
                start = i + 1
    # A tool can occur in malformed/missing-user segments; replay only uses eligible episodes.
    seen = set()
    dedup_tools = []
    for t in sorted(tools_all, key=lambda x: (x["timestamp"], x["id"])):
        if t["id"] not in seen:
            seen.add(t["id"])
            dedup_tools.append(t)
    return sorted(episodes, key=lambda x: x["timestamp"]), dedup_tools


def assign_splits(episodes: list[dict]) -> dict[str, str]:
    first: dict[str, float] = {}
    for e in episodes:
        first[e["family"]] = min(first.get(e["family"], e["timestamp"]), e["timestamp"])
    families = sorted(first, key=lambda f: (first[f], f))
    n = len(families)
    train_end = math.floor(n * 0.60)
    valid_end = math.floor(n * 0.80)
    out = {}
    for i, f in enumerate(families):
        out[f] = "train" if i < train_end else ("validation" if i < valid_end else "test")
    return out


def nomination_labels(ep: dict) -> dict:
    ans_raw = raw_values(ep["answer"])
    ans_norm = normalized_values(ep["answer"])
    task_norm = normalized_values(ep["task"])
    m0, m1, unique = set(), set(), set()
    occurrence: dict[str, list[int]] = collections.defaultdict(list)
    for t in ep["tools"]:
        tid = t["id"]
        rv = raw_values(t["content"])
        nv = normalized_values(t["content"])
        if rv & ans_raw:
            m0.add(tid)
        if nv & ans_norm:
            m1.add(tid)
        for v in (nv & ans_norm) - task_norm:
            occurrence[v].append(tid)
    for ids in occurrence.values():
        if len(set(ids)) == 1:
            unique.add(ids[0])
    return {"m0": m0, "m1": m1, "unique": unique}


def annotate_tools(episodes: list[dict]) -> tuple[set[int], set[int], dict[int, int]]:
    positive: set[int] = set()
    thorn: set[int] = set()
    episode_of: dict[int, int] = {}
    for ei, ep in enumerate(episodes):
        labels = nomination_labels(ep)
        positive.update(labels["m1"])
        for i, t in enumerate(ep["tools"]):
            episode_of[t["id"]] = ei
            if t["id"] in positive:
                continue
            retry_same_tool = any(
                u.get("tool_name") and u.get("tool_name") == t.get("tool_name")
                for u in ep["tools"][i+1:]
            )
            if ERROR_RE.search(t["content"] or "") and retry_same_tool:
                thorn.add(t["id"])
    return positive, thorn, episode_of


def build_adj(tools: list[dict]) -> list[set[int]]:
    idx = {t["id"]: i for i, t in enumerate(tools)}
    adj = [set() for _ in tools]
    last_session: dict[str, int] = {}
    seen_entity: dict[str, list[int]] = collections.defaultdict(list)
    for i, t in enumerate(tools):
        prev = last_session.get(t["session"])
        if prev is not None:
            adj[i].add(prev); adj[prev].add(i)
        last_session[t["session"]] = i
        for ent in sorted(t["entities"]):
            for j in seen_entity[ent][-7:]:
                adj[i].add(j); adj[j].add(i)
            seen_entity[ent].append(i)
    return adj


def ppr(adj: list[set[int]], seeds: set[int], id_to_idx: dict[int, int]) -> list[float]:
    n = len(adj)
    if n == 0 or not seeds:
        return [0.0] * n
    seed_idx = sorted({id_to_idx[x] for x in seeds if x in id_to_idx})
    if not seed_idx:
        return [0.0] * n
    pers = [0.0] * n
    for i in seed_idx:
        pers[i] = 1.0 / len(seed_idx)
    rank = pers[:]
    d = PPR_DAMPING
    for _ in range(PPR_ITERS):
        nxt = [(1.0 - d) * p for p in pers]
        dangling = 0.0
        for i, neighbors in enumerate(adj):
            if neighbors:
                share = d * rank[i] / len(neighbors)
                for j in sorted(neighbors):
                    nxt[j] += share
            else:
                dangling += d * rank[i]
        if dangling:
            for i, p in enumerate(pers):
                nxt[i] += dangling * p
        rank = nxt
    return rank


def normalize_scores(values: list[float]) -> list[float]:
    m = max(values, default=0.0)
    return [v / m if m else 0.0 for v in values]


def source_tf(task: str, content: str) -> float:
    terms = set(tf_tokens(task))
    counts = collections.Counter(tf_tokens(content))
    return float(sum(counts[t] for t in terms))


def proxy_gold(ep: dict, candidates: list[dict]) -> set[int]:
    # Current-episode tools define future-used entities; answer narrows to entities
    # visibly carried through. This is a retrieval proxy, not causal gold.
    current_entities = set().union(*(t["entities"] for t in ep["tools"])) if ep["tools"] else set()
    answer_entities = entity_keys(ep["answer"], None)
    target = current_entities | answer_entities
    if not target:
        return set()
    entity_freq = collections.Counter(ent for c in candidates for ent in c["entities"])
    rare_target = {e for e in target if entity_freq[e] <= 20}
    return {c["id"] for c in candidates if c["entities"] & rare_target}


def metric_at(ranked: list[int], gold: set[int], k: int) -> dict[str, float]:
    chosen = ranked[:k]
    hit = sum(x in gold for x in chosen)
    precision = hit / k
    recall = hit / len(gold) if gold else 0.0
    dcg = sum((1.0 if x in gold else 0.0) / math.log2(i + 2) for i, x in enumerate(chosen))
    ideal = sum(1.0 / math.log2(i + 2) for i in range(min(k, len(gold))))
    return {"precision": precision, "recall": recall, "ndcg": dcg / ideal if ideal else 0.0, "hit": 1.0 if hit else 0.0}


def mean_dict(rows: list[dict[str, float]]) -> dict[str, float]:
    if not rows:
        return {"precision": 0.0, "recall": 0.0, "ndcg": 0.0, "hit": 0.0}
    return {k: statistics.fmean(r[k] for r in rows) for k in rows[0]}


def run_replay(db: Path, repo_commit: str) -> dict:
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    try:
        schema = con.execute("SELECT version FROM schema_version").fetchone()[0]
        fam = family_map(con)
        episodes, tools = load_episodes(con, fam)
        split = assign_splits(episodes)
        positive, thorns, _ = annotate_tools(episodes)

        nomination = collections.Counter()
        for ep in episodes:
            lab = nomination_labels(ep)
            nomination["episodes"] += 1
            nomination["candidate_tools"] += len(ep["tools"])
            nomination["m0_nominations"] += len(lab["m0"])
            nomination["m1_nominations"] += len(lab["m1"])
            nomination["m1_incremental"] += len(lab["m1"] - lab["m0"])
            nomination["unique_value_silver"] += len(lab["unique"])
            nomination["m0_unique_hits"] += len(lab["m0"] & lab["unique"])
            nomination["m1_unique_hits"] += len(lab["m1"] & lab["unique"])
            nomination["no_m0"] += int(not lab["m0"])
            nomination["no_m1"] += int(not lab["m1"])

        test_eps = [e for e in episodes if split.get(e["family"]) == "test"]
        eval_rows: dict[str, dict[int, list[dict[str, float]]]] = {
            name: {k: [] for k in BUDGETS}
            for name in ("lexical", "random", "prior_only_multiplicative", "thorn_only_multiplicative", "full_multiplicative", "exploratory_additive")
        }
        eligible = zero_tf_gold = no_gold = 0
        candidate_counts = []
        gold_counts = []
        rng = random.Random(SEED)

        for ep in test_eps:
            candidates = [t for t in tools if t["timestamp"] < ep["timestamp"] and t["family"] != ep["family"]]
            if len(candidates) < max(BUDGETS):
                continue
            gold = proxy_gold(ep, candidates)
            if not gold:
                no_gold += 1
                continue
            eligible += 1
            candidate_counts.append(len(candidates)); gold_counts.append(len(gold))
            id_to_idx = {t["id"]: i for i, t in enumerate(candidates)}
            adj = build_adj(candidates)
            pos_prior = normalize_scores(ppr(adj, positive, id_to_idx))
            neg_prior = normalize_scores(ppr(adj, thorns, id_to_idx))
            tf = [source_tf(ep["task"], t["content"]) for t in candidates]
            zero_tf_gold += int(all(tf[i] == 0.0 for i, t in enumerate(candidates) if t["id"] in gold))
            ids = [t["id"] for t in candidates]
            scores = {
                "lexical": tf,
                "prior_only_multiplicative": [tf[i] * (1 + ALPHA * pos_prior[i]) for i in range(len(ids))],
                "thorn_only_multiplicative": [tf[i] * (1 - BETA * neg_prior[i]) for i in range(len(ids))],
                "full_multiplicative": [tf[i] * (1 + ALPHA * pos_prior[i]) * (1 - BETA * neg_prior[i]) for i in range(len(ids))],
                # Contract-external diagnostic: can resurrect a zero-TF event.
                "exploratory_additive": [tf[i] + ALPHA * pos_prior[i] - BETA * neg_prior[i] for i in range(len(ids))],
            }
            ranked: dict[str, list[int]] = {}
            for name, vals in scores.items():
                ranked[name] = [ids[i] for i in sorted(range(len(ids)), key=lambda i: (-vals[i], ids[i]))]
            random_ids = ids[:]
            rng.shuffle(random_ids)
            ranked["random"] = random_ids
            for name, order in ranked.items():
                for k in BUDGETS:
                    eval_rows[name][k].append(metric_at(order, gold, k))

        selection = {}
        for name, by_k in eval_rows.items():
            selection[name] = {str(k): {m: round(v, 6) for m, v in mean_dict(rows).items()} for k, rows in by_k.items()}

        family_counts = collections.Counter(split.values())
        episode_split_counts = collections.Counter(split.get(e["family"], "unknown") for e in episodes)
        sessions = con.execute("SELECT COUNT(*) FROM sessions").fetchone()[0]
        messages = con.execute("SELECT COUNT(*) FROM messages WHERE active=1").fetchone()[0]
        tool_messages = con.execute("SELECT COUNT(*) FROM messages WHERE active=1 AND role='tool'").fetchone()[0]
        token_nonnull = con.execute("SELECT COUNT(*) FROM messages WHERE token_count IS NOT NULL").fetchone()[0]
        duplicate_rows = con.execute("""
          WITH x AS (SELECT role,hex(sha3(content,256)) h,COUNT(*) n FROM messages
                     WHERE active=1 AND LENGTH(content)>0 GROUP BY role,h HAVING n>1)
          SELECT COALESCE(SUM(n),0) FROM x
        """).fetchone()[0] if any(r[0] == "sha3" for r in con.execute("SELECT name FROM pragma_function_list")) else None

        result = {
            "manifest": {
                "generated_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
                "db_sha256": sha256_file(db), "schema_version": schema,
                "repo_commit": repo_commit, "seed": SEED,
                "m0_version": M0_VERSION, "m1_version": M1_VERSION,
                "entity_version": ENTITY_VERSION, "split_policy": SPLIT_POLICY,
                "alpha": ALPHA, "beta": BETA, "ppr_iterations": PPR_ITERS,
                "ppr_damping": PPR_DAMPING, "budgets": list(BUDGETS),
                "network_used": False, "raw_content_emitted": False,
            },
            "corpus": {
                "sessions": sessions, "active_messages": messages,
                "tool_messages": tool_messages, "eligible_terminal_episodes": len(episodes),
                "unique_families": sum(family_counts.values()), "family_split": dict(family_counts),
                "episode_split": dict(episode_split_counts), "eligible_tool_events": len(tools),
                "message_token_count_nonnull": token_nonnull,
                "duplicate_nonempty_rows_if_supported": duplicate_rows,
            },
            "nomination_proxy": dict(nomination),
            "selection_proxy": {
                "test_episodes_total": len(test_eps), "evaluated_episodes": eligible,
                "episodes_without_proxy_gold": no_gold,
                "episodes_all_proxy_gold_zero_tf": zero_tf_gold,
                "candidate_events_mean": round(statistics.fmean(candidate_counts), 3) if candidate_counts else 0,
                "proxy_gold_mean": round(statistics.fmean(gold_counts), 3) if gold_counts else 0,
                "metrics": selection,
            },
            "validity": {
                "label_status": "silver proxies only; no causal or human gold",
                "m0_m1_precision_claim_allowed": False,
                "selection_usefulness_claim_allowed": False,
                "finish_reason_is_not_task_success": True,
                "state_db_is_not_option_a_signed_dag": True,
                "multiplicative_formula_cannot_resurrect_tf_zero": True,
                "tokens_unavailable_if_nonnull_zero": token_nonnull == 0,
            },
        }
        result["manifest"]["aggregate_fingerprint"] = stable_hash(json.dumps({
            "corpus": result["corpus"],
            "nomination_proxy": result["nomination_proxy"],
            "selection_proxy": result["selection_proxy"],
            "validity": result["validity"],
        }, sort_keys=True, separators=(",", ":")))
        return result
    finally:
        con.close()


def write_result(path: Path, result: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def verify(db: Path, results: Path) -> None:
    data = json.loads(results.read_text(encoding="utf-8"))
    errors = []
    if data["manifest"]["db_sha256"] != sha256_file(db):
        errors.append("database SHA-256 mismatch")
    if data["manifest"].get("network_used") is not False:
        errors.append("network_used must be false")
    if data["manifest"].get("raw_content_emitted") is not False:
        errors.append("raw_content_emitted must be false")
    forbidden_keys = {"content", "task", "answer", "session_id", "user_id", "chat_id", "reasoning"}
    def walk(x):
        if isinstance(x, dict):
            for k, v in x.items():
                if k in forbidden_keys:
                    errors.append(f"forbidden output key: {k}")
                walk(v)
        elif isinstance(x, list):
            for v in x: walk(v)
    walk(data)
    expected_fingerprint = stable_hash(json.dumps({
        "corpus": data["corpus"],
        "nomination_proxy": data["nomination_proxy"],
        "selection_proxy": data["selection_proxy"],
        "validity": data["validity"],
    }, sort_keys=True, separators=(",", ":")))
    if data["manifest"].get("aggregate_fingerprint") != expected_fingerprint:
        errors.append("aggregate fingerprint mismatch")
    if errors:
        print("REPLAY VERIFY FAILED")
        for e in sorted(set(errors)): print("-", e)
        raise SystemExit(1)
    print("REPLAY VERIFIED: snapshot hash exact; aggregate-only output; no raw content keys; network=false")


def repo_head(repo: Path) -> str:
    import subprocess
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()


def main() -> None:
    p = argparse.ArgumentParser()
    sp = p.add_subparsers(dest="cmd", required=True)
    s = sp.add_parser("snapshot"); s.add_argument("--source", type=Path, required=True); s.add_argument("--dest", type=Path, required=True)
    r = sp.add_parser("run"); r.add_argument("--db", type=Path, required=True); r.add_argument("--out", type=Path, required=True); r.add_argument("--repo", type=Path, default=Path.cwd())
    v = sp.add_parser("verify"); v.add_argument("--db", type=Path, required=True); v.add_argument("--results", type=Path, required=True)
    a = p.parse_args()
    if a.cmd == "snapshot": snapshot(a.source, a.dest)
    elif a.cmd == "run":
        result = run_replay(a.db, repo_head(a.repo))
        write_result(a.out, result)
        print(json.dumps({"results": str(a.out), "evaluated_episodes": result["selection_proxy"]["evaluated_episodes"], "raw_content_emitted": False}, indent=2))
    else: verify(a.db, a.results)

if __name__ == "__main__":
    main()
