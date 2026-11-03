#!/usr/bin/env python3
"""OC-04 post-mortem: why did cross-family consensus fail?
Read-only analysis over the 4 production label sets (local, HMAC-only)."""
import json
from pathlib import Path
from collections import Counter, defaultdict

BASE = Path("/home/cosmo/contextmesh-rs/oc04-gold/packets_prod")
ANN = ["Aprime", "A2sol", "B", "C"]  # GLM, GPT-6.1, GPT-5.6, DS
FAM = {"Aprime": "GLM", "A2sol": "GPT", "B": "GPT", "C": "DS"}
LAB = ["required", "supporting", "irrelevant", "dead_end", "uncertain"]

labels = {n: {} for n in ANN}
sess_events = defaultdict(list)
for n in ANN:
    for f in sorted((BASE / f"labels_{n}").glob("*.jsonl")):
        s = f.name.split(".")[0]
        for line in f.read_text().splitlines():
            if line.strip():
                r = json.loads(line)
                labels[n][(s, r["event_index"])] = r["label"]
                if n == "Aprime":
                    sess_events[s].append(r["event_index"])

keys = sorted(labels["Aprime"])

# 1. pairwise confusion A' vs B (row=A', col=B)
def confusion(a, b):
    m = {l: Counter() for l in LAB}
    for k in keys:
        m[labels[a][k]][labels[b][k]] += 1
    return m

def show(cm, a, b):
    print(f"\n== {a} (row) vs {b} (col) ==")
    print("            " + " ".join(f"{l[:6]:>7}" for l in LAB))
    for l in LAB:
        row = cm[l]
        tot = sum(row.values())
        cells = " ".join(f"{row[l2]:>7}" for l2 in LAB)
        print(f"{l[:10]:>11} {cells}   (n={tot})")

cm_AB = confusion("Aprime", "B")
show(cm_AB, "Aprime", "B")
show(confusion("Aprime", "A2sol"), "Aprime", "A2sol")

# 2. 2-2 split camp structure
pair_count = Counter()
for k in keys:
    v = {n: labels[n][k] for n in ANN}
    d = {n: x for n, x in v.items() if x != "uncertain"}
    c = Counter(d.values())
    if len(d) == 4 and len(c) == 2 and c.most_common(1)[0][1] == 2:
        top = {l for l, n_ in c.items() if n_ == 2}
        # which annotator pairs share a label
        camps = []
        ns = list(ANN)
        for i in range(4):
            for j in range(i+1, 4):
                if d[ns[i]] == d[ns[j]]:
                    camps.append(f"{ns[i]}+{ns[j]}")
        for cc in camps:
            pair_count[cc] += 1
print("\n== 2-2 split: sharing pairs (top 10) ==")
for p, n_ in pair_count.most_common(10):
    print(f"  {p}: {n_}")

# 3. session-level agreement A' vs B (percent) distribution
sess_agree = []
for s in sorted(sess_events):
    evs = sess_events[s]
    agree = sum(1 for e in evs if labels["Aprime"][(s, e)] == labels["B"][(s, e)])
    sess_agree.append((s[:12], agree / len(evs), len(evs)))
sess_agree.sort(key=lambda x: x[1])
print("\n== session-level A'<->B raw agreement: worst 8 / best 3 ==")
for s, a, n_ in sess_agree[:8] + sess_agree[-3:]:
    print(f"  {s}… agree={a:.2f} n={n_}")
import statistics
print(f"  median={statistics.median(a for _,a,_ in sess_agree):.3f}")

# 4. label-transition profile: where A' diverges from GPT consensus
# GPT-pair agreement as pseudo-reference
div = Counter()
for k in keys:
    a2, b = labels["A2sol"][k], labels["B"][k]
    if a2 == b and a2 != "uncertain":
        ap = labels["Aprime"][k]
        if ap != a2:
            div[f"GPT:{a2[:4]} <- GLM:{ap[:4]}"] += 1
print("\n== A' divergence when A\"==B (GPT pair agrees), top 12 ==")
for t, n_ in div.most_common(12):
    print(f"  {t}: {n_}")

# 5. uncertain usage per annotator on the 64-event session
big = max(sess_events, key=lambda s: len(sess_events[s]))
print(f"\n== biggest session {big[:12]}… n={len(sess_events[big])} ==")
for n in ANN:
    c = Counter(labels[n][(big, e)] for e in sess_events[big])
    print(f"  {n}: {dict(c)}")

# 6. binary (required vs rest) agreement matrix
print("\n== binary required-vs-rest agreement matrix ==")
print("            " + " ".join(f"{n[:7]:>8}" for n in ANN))
for a in ANN:
    row = []
    for b in ANN:
        ag = sum(1 for k in keys
                 if (labels[a][k] == "required") == (labels[b][k] == "required"))
        row.append(ag / len(keys))
    print(f"{a[:10]:>11} " + " ".join(f"{x:>8.3f}" for x in row))
