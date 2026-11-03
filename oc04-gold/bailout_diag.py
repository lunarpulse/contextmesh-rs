#!/usr/bin/env python3
"""Quantify mass-uncertain bailout sessions; diagnostic recompute without them."""
import json
from pathlib import Path
from collections import Counter

BASE = Path("/home/cosmo/contextmesh-rs/oc04-gold/packets_prod")
ANN = {"Aprime": "GLM", "A2sol": "GPT", "B": "GPT", "C": "DS"}
ORDER = ["Aprime", "A2sol", "B", "C"]

labels = {n: {} for n in ORDER}
sess_events = {}
for n in ORDER:
    for f in sorted((BASE / f"labels_{n}").glob("*.jsonl")):
        s = f.name.split(".")[0]
        for line in f.read_text().splitlines():
            if line.strip():
                r = json.loads(line)
                labels[n][(s, r["event_index"])] = r["label"]
                sess_events.setdefault(s, set()).add(r["event_index"])

# mass-bailout: any annotator with >=50% uncertain in a session
bail = []
for s in sorted(sess_events):
    evs = sorted(sess_events[s])
    who = [n for n in ORDER
           if sum(1 for e in evs if labels[n][(s,e)] == "uncertain") / len(evs) >= 0.5]
    if who:
        bail.append((s, len(evs), who))
print(f"mass-bailout sessions (>=50% uncertain by >=1 annotator): {len(bail)}")
tot_b = sum(n for _, n, _ in bail)
for s, n, who in bail:
    print(f"  {s[:14]}… n={n} bailed={who}")
print(f"events affected: {tot_b} ({tot_b/3979:.1%} of corpus)")

# diagnostic: consensus + kappa on clean sessions only
clean = {s for s in sess_events if s not in {b[0] for b in bail}}
ck = [k for k in labels["Aprime"] if k[0] in clean]
print(f"\nclean keys: {len(ck)}")

adopted = excl = 0
for k in ck:
    v = {n: labels[n][k] for n in ORDER}
    d = {n: x for n, x in v.items() if x != "uncertain"}
    c = Counter(d.values())
    if not c:
        excl += 1; continue
    lab, na = c.most_common(1)[0]
    camps = {ANN[n] for n, x in d.items() if x == lab}
    if na >= 3 and len(camps) >= 2:
        adopted += 1
    else:
        excl += 1
print(f"clean-only: adopted={adopted} excluded={excl} "
      f"rate={excl/(adopted+excl):.4f} (ceiling 0.15)")

def kappa(a, b, keys, binary=False):
    ka, kb = [], []
    for k in keys:
        x, y = labels[a][k], labels[b][k]
        if binary:
            x = "required" if x == "required" else "other"
            y = "required" if y == "required" else "other"
        ka.append(x); kb.append(y)
    po = sum(1 for x, y in zip(ka, kb) if x == y) / len(ka)
    ca, cb = Counter(ka), Counter(kb)
    cats = set(ka) | set(kb)
    pe = sum((ca[c]/len(ka)) * (cb[c]/len(kb)) for c in cats)
    return (po - pe) / (1 - pe) if pe < 1 else 1.0

for pair in [("Aprime","B"), ("Aprime","A2sol"), ("A2sol","B")]:
    print(f"clean-only κ {pair[0]}<->{pair[1]}: "
          f"5label={kappa(*pair, ck):.4f} binary={kappa(*pair, ck, True):.4f}")
