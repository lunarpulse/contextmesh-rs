#!/usr/bin/env python3
"""Inspect the 3 zero-agreement sessions: distribution + offset hypothesis."""
import json
from pathlib import Path
from collections import Counter

BASE = Path("/home/cosmo/contextmesh-rs/oc04-gold/packets_prod")
ANN = ["Aprime", "A2sol", "B", "C"]
ZERO = ["910ff102065a", "9aa3d3221c9b", "e30f83a58bf5"]

labels = {n: {} for n in ANN}
for n in ANN:
    for f in sorted((BASE / f"labels_{n}").glob("*.jsonl")):
        s = f.name.split(".")[0]
        if not any(s.startswith(z) for z in ZERO):
            continue
        for line in f.read_text().splitlines():
            if line.strip():
                r = json.loads(line)
                labels[n][(s, r["event_index"])] = r["label"]

for z in ZERO:
    s = next(k[0] for k in labels["Aprime"] if k[0].startswith(z))
    evs = sorted(e for (ss, e) in labels["Aprime"] if ss == s)
    print(f"\n=== {s[:16]}… n={len(evs)} ===")
    for n in ANN:
        c = Counter(labels[n][(s, e)] for e in evs)
        print(f"  {n:8}: {dict(c)}")
    # pairwise agreement
    for i, a in enumerate(ANN):
        for b in ANN[i+1:]:
            ag = sum(1 for e in evs if labels[a][(s,e)] == labels[b][(s,e)]) / len(evs)
            print(f"    {a}<->{b}: {ag:.2f}", end="")
    print()
    # offset hypothesis: A' at e vs B at e+k
    for shift in (-1, 1):
        ag = sum(1 for e in evs if e + shift in evs and
                 labels["Aprime"][(s,e)] == labels["B"][(s, e+shift)])
        print(f"  Aprime@e == B@e{shift:+d}: {ag}/{len(evs)-1}")
    # first 6 labels side by side
    print("  first 6 events (Aprime/A2sol/B/C):")
    for e in evs[:6]:
        print(f"    e{e}: " + "/".join(labels[n][(s,e)][:4] for n in ANN))
