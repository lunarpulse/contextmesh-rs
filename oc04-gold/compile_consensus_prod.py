#!/usr/bin/env python3
"""OC-04 Phase 2: production consensus compiler (v5.5 protocol, plan v3).

Rule: per (session, event_index), 4 votes from A'(GLM), A''(GPT), B(GPT), C(DeepSeek).
- uncertain votes do not count toward any majority (uncertain-majority adoption prohibited).
- Adopt label if >=3 agreeing votes AND agreeing set spans >=2 model-family camps
  (camps: GLM={A'}, GPT={A'',B}, DS={C}) — F-04 both-camps requirement.
- 2-2 / no >=3 majority / cross-family fail -> excluded (fail-closed).
Outputs labels_prod.jsonl (session_hmac, event_index, label, votes) + consensus_report.json.
"""
import json
from pathlib import Path
from collections import Counter

BASE = Path("/home/cosmo/contextmesh-rs/oc04-gold/packets_prod")
ANN = {"Aprime": "GLM", "A2sol": "GPT", "B": "GPT", "C": "DS"}

def load(name):
    d = {}
    for f in sorted((BASE / f"labels_{name}").glob("*.jsonl")):
        sess = f.name.split(".")[0]
        for line in f.read_text().splitlines():
            line = line.strip()
            if not line:
                continue
            r = json.loads(line)
            d[(sess, int(r["event_index"]))] = r["label"]
    return d

labels = {n: load(n) for n in ANN}
keys = sorted(set().union(*[set(v) for v in labels.values()]))
assert all(set(v) == set(keys) for v in labels.values()), "key set mismatch"

out, excluded, ambiguous = [], [], []
vote_patterns = Counter()
for k in keys:
    votes = {n: labels[n][k] for n in ANN}
    decisive = {n: v for n, v in votes.items() if v != "uncertain"}
    cnt = Counter(decisive.values())
    label, n_agree = (cnt.most_common(1)[0] if cnt else (None, 0))
    camps = {ANN[n] for n, v in decisive.items() if v == label}
    if n_agree >= 3 and len(camps) >= 2:
        out.append({"session_hmac": k[0], "event_index": k[1], "label": label,
                    "votes": votes})
    else:
        excluded.append({"session_hmac": k[0], "event_index": k[1],
                         "votes": votes, "reason": "no_3of4_cross_family"})
        # ambiguity candidates for founder: 2-2 decisive split with 4 decisive votes
        if len(decisive) == 4 and len(cnt) == 2:
            ambiguous.append(out and excluded[-1])
    pat = "".join("U" if votes[n] == "uncertain" else votes[n][0].upper()
                  for n in ["Aprime", "A2sol", "B", "C"]) + "->" + (label or "EXCL")
    vote_patterns[pat] += 1

decisive_total = len(out) + len(excluded)
dist = Counter(r["label"] for r in out)
report = {
    "adopted": len(out), "excluded": len(excluded),
    "exclusion_rate": round(len(excluded) / decisive_total, 4),
    "exclusion_ceiling": 0.15,
    "ceiling_pass": len(excluded) / decisive_total <= 0.15,
    "adopted_dist": dict(dist),
    "vote_patterns_top": vote_patterns.most_common(15),
}

# pairwise kappa (Cohen, unweighted, 5-label) + binary (required vs rest)
LAB5 = ["required", "supporting", "irrelevant", "dead_end", "uncertain"]
def kappa(a, b, labels_list):
    ka = [labels[a][k] for k in keys]; kb = [labels[b][k] for k in keys]
    cats = labels_list
    po = sum(1 for x, y in zip(ka, kb) if x == y) / len(ka)
    ca = Counter(ka); cb = Counter(kb)
    pe = sum((ca[c] / len(ka)) * (cb[c] / len(kb)) for c in cats)
    return round((po - pe) / (1 - pe), 4) if pe < 1 else 1.0

pairs = {}
for i, a in enumerate(ANN):
    for b in list(ANN)[i+1:]:
        pairs[f"{a}<->{b}"] = {"kappa5": kappa(a, b, LAB5)}
        bin_a = {k: ("required" if labels[a][k] == "required" else "other") for k in keys}
        bin_b = {k: ("required" if labels[b][k] == "required" else "other") for k in keys}
        ka = [bin_a[k] for k in keys]; kb = [bin_b[k] for k in keys]
        po = sum(1 for x, y in zip(ka, kb) if x == y) / len(ka)
        ca = Counter(ka); cb = Counter(kb)
        pe = sum((ca[c]/len(ka))*(cb[c]/len(kb)) for c in ["required", "other"])
        pairs[f"{a}<->{b}"]["kappa_bin"] = round((po-pe)/(1-pe), 4)
report["pairwise_kappa"] = pairs
report["primary_axis"] = "Aprime<->B (GLM<->GPT cross-family)"

(BASE / "labels_prod.jsonl").write_text(
    "\n".join(json.dumps(r, ensure_ascii=False) for r in out) + "\n")
(BASE / "consensus_report.json").write_text(json.dumps(report, indent=2))
print(json.dumps({k: report[k] for k in
                  ["adopted", "excluded", "exclusion_rate", "ceiling_pass",
                   "adopted_dist", "primary_axis"]}, indent=2))
print(json.dumps(pairs, indent=2))
