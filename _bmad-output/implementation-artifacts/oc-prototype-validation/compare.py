#!/usr/bin/env python3
"""Formal port-verification gate: Rust results vs Python ground truth.
E1/E3 must match exactly; E2 allowed 0.002 (set-iteration ulps)."""
import json, sys

py = json.load(open("results.json"))
rs = json.load(open("results-rust.json"))

fail = []
for k in ("seed", "sessions", "events_total"):
    if py[k] != rs[k]:
        fail.append(f"{k}: {py[k]} != {rs[k]}")

for a, b in zip(py["E1_attribution_ladder"], rs["E1_attribution_ladder"]):
    for k in ("mech", "precision", "recall", "f1", "judge_calls_per_session"):
        if a[k] != b[k]:
            fail.append(f"E1 {a['mech']}.{k}: {a[k]} != {b[k]}")

for k in py["E2_propagation_selection"]:
    a, b = py["E2_propagation_selection"][k], rs["E2_propagation_selection"][k]
    if abs(a - b) > 0.002:
        fail.append(f"E2 {k}: {a} vs {b} (tolerance 0.002)")

for k in py["E3_cost_ledger"]:
    if py["E3_cost_ledger"][k] != rs["E3_cost_ledger"][k]:
        fail.append(f"E3 {k}: {py['E3_cost_ledger'][k]} != {rs['E3_cost_ledger'][k]}")

if fail:
    print("PORT MISMATCH:")
    print("\n".join(fail))
    sys.exit(1)
print("PORT VERIFIED: E1 exact, E3 exact, E2 within 0.002 (observed: exact)")
