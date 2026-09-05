#!/usr/bin/env python3
"""inspect_families.py — frame-level parent-family distribution (read-only)."""
import json
from collections import defaultdict

doc = json.load(open(
    "_bmad-output/implementation-artifacts/oc-real-data-replay/private/oc04-frame-snapshot.json"
))
CUT = doc["cutoff_epoch"]
rows = [s for s in doc["sessions"]
        if s["started_at"] >= CUT and s["message_count"] >= 4]

toks = sorted(s["input_tokens"] for s in rows if s["terminated"] and s["input_tokens"] > 0)
n = len(toks)
med = toks[n // 2] if n % 2 else (toks[n // 2 - 1] + toks[n // 2]) / 2

fam = defaultdict(lambda: {"n": 0, "high": 0, "low": 0})
for s in rows:
    f = s["parent_session_id"] or s["id"]
    fam[f]["n"] += 1
    if s["terminated"]:
        st = "high" if s["input_tokens"] > med else "low"
        fam[f][st] += 1

print(f"eligible={len(rows)} median={med:.0f} distinct_families={len(fam)}")
for f, d in sorted(fam.items(), key=lambda kv: -kv[1]["n"]):
    print(f"family {f[:18]:<18} n={d['n']:<4} high={d['high']:<4} low={d['low']}")
