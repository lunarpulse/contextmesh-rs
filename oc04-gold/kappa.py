#!/usr/bin/env python3
"""kappa.py — OC-04 pilot reliability statistics (§5.3, plan v5.4).

Computes, from two annotator label files (JSONL: {session_hmac, event_index,
label, reason_code?}):
  - kappa (binary): relevant (required|supporting) vs non-relevant
  - kappa (5-label): required / supporting / dead_end / irrelevant / uncertain
  - positive agreement per label: required, supporting, dead_end
  - per-annotator `uncertain` rate (pilot gate: > 15% triggers revision)

All statistics run on RAW independent labels BEFORE adjudication.
κ = (P_o - P_e) / (1 - P_e); P_e by the joint marginal distribution.

Usage:
  python3 kappa.py --a annotA.jsonl --b annotB.jsonl [--stratified]

Exit codes: 0 = gates pass (κbin>=0.6, κ5>=0.5, PA>=0.60 each decisive
label with >=20 occurrences, uncertain<=15%); 1 = gates fail; 2 = usage.
"""
import argparse
import json
import sys
from collections import Counter

LABELS_5 = ("required", "supporting", "dead_end", "irrelevant", "uncertain")
DECISIVE = ("required", "supporting", "dead_end")
KAPPA_BINARY_MIN = 0.60
KAPPA_5_MIN = 0.50
POSITIVE_AGREEMENT_MIN = 0.60
DECISIVE_MIN_OCCURRENCES = 20
UNCERTAIN_RATE_MAX = 0.15


def load(path):
    rows = {}
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            r = json.loads(line)
            key = (r["session_hmac"], r["event_index"])
            if key in rows:
                sys.exit(f"usage-error: duplicate key {key} in {path}")
            if r["label"] not in LABELS_5:
                sys.exit(f"usage-error: unknown label {r['label']!r} in {path}")
            rows[key] = r["label"]
    return rows


def paired(a, b):
    keys = sorted(set(a) & set(b))
    missing_a = set(b) - set(a)
    missing_b = set(a) - set(b)
    if missing_a or missing_b:
        print(f"WARN: unpaired items (attrition) a-missing={len(missing_b)} b-missing={len(missing_a)}")
    return [(a[k], b[k]) for k in keys]


def kappa(pairs, classes):
    n = len(pairs)
    if n == 0:
        return float("nan"), 0.0
    po = sum(1 for x, y in pairs if x == y) / n
    ca = Counter(x for x, _ in pairs)
    cb = Counter(y for _, y in pairs)
    pe = sum((ca.get(c, 0) / n) * (cb.get(c, 0) / n) for c in classes)
    if pe == 1.0:
        return float("nan"), po
    return (po - pe) / (1.0 - pe), po


def positive_agreement(pairs, label):
    # PA = P(b says label | a says label) symmetrized: (P(b|a)+P(a|b))/2
    ab = [(x, y) for x, y in pairs if x == label]
    ba = [(x, y) for x, y in pairs if y == label]
    pa = sum(1 for _, y in ab if y == label) / len(ab) if ab else float("nan")
    pb = sum(1 for x, _ in ba if x == label) / len(ba) if ba else float("nan")
    vals = [v for v in (pa, pb) if v == v]
    return (sum(vals) / len(vals)) if vals else float("nan"), len(ab) + len(ba)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--a", required=True)
    ap.add_argument("--b", required=True)
    args = ap.parse_args()

    a, b = load(args.a), load(args.b)
    pairs = paired(a, b)
    n = len(pairs)
    if n == 0:
        print("no paired labels"); sys.exit(2)

    bin_pairs = [("relevant" if x in ("required", "supporting") else "non", "relevant" if y in ("required", "supporting") else "non") for x, y in pairs]
    k_bin, po_bin = kappa(bin_pairs, ("relevant", "non"))
    k5, po5 = kappa(pairs, LABELS_5)

    unc_a = sum(1 for x, _ in pairs if x == "uncertain") / n
    unc_b = sum(1 for _, y in pairs if y == "uncertain") / n

    print(f"paired items: {n}")
    print(f"kappa(binary) = {k_bin:.4f}  (P_o={po_bin:.4f})  gate >= {KAPPA_BINARY_MIN}")
    print(f"kappa(5-label)= {k5:.4f}  (P_o={po5:.4f})  gate >= {KAPPA_5_MIN}")
    print(f"uncertain rate: A={unc_a:.3f} B={unc_b:.3f}  gate <= {UNCERTAIN_RATE_MAX}")
    for label in DECISIVE:
        pa, occ = positive_agreement(pairs, label)
        gate = f"gate >= {POSITIVE_AGREEMENT_MIN} (n>={DECISIVE_MIN_OCCURRENCES})" if occ >= DECISIVE_MIN_OCCURRENCES else f"n={occ} < {DECISIVE_MIN_OCCURRENCES} → adjudication audit sampling"
        print(f"PA({label:<10})= {pa:.4f}  occ={occ:<4} {gate}")

    failures = []
    if not (k_bin == k_bin and k_bin >= KAPPA_BINARY_MIN):
        failures.append("kappa(binary)")
    if not (k5 == k5 and k5 >= KAPPA_5_MIN):
        failures.append("kappa(5-label)")
    for label in DECISIVE:
        pa, occ = positive_agreement(pairs, label)
        if occ >= DECISIVE_MIN_OCCURRENCES and not (pa == pa and pa >= POSITIVE_AGREEMENT_MIN):
            failures.append(f"PA({label})")
    if unc_a > UNCERTAIN_RATE_MAX or unc_b > UNCERTAIN_RATE_MAX:
        failures.append("uncertain-rate (codebook revision + full re-label per §5.2b)")
    if failures:
        print(f"GATE: FAIL — {', '.join(failures)}")
        sys.exit(1)
    print("GATE: PASS — proceed to production labeling (codebook frozen)")


if __name__ == "__main__":
    main()
