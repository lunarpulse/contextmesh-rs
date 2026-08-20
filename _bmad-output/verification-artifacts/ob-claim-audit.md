# OB Claim Audit

Scope: every substantive Option B claim, classified as demonstrated (exact
proving artifact named), limited (true within a stated bound), or
removed/absent (correctly not claimed). Verifiers are the per-gate scripts
`scripts/verify-ob01.sh`..`verify-ob13.sh`; each gate ran green when its
package committed.

## Demonstrated

| Claim | Proof |
|---|---|
| B1 receipts: signed, self-contained, content-addressed, DAG-verifiable; omissions + uncertainty fields | tests/ob01_receipts.rs; verify-ob01.sh |
| B2 selection core: task intake (free text + structured), budget enforcement, baseline lexical selector, recorded provenance | tests/ob02_selection.rs; verify-ob02.sh; ob02-selection-golden.json |
| B3 dependency closure + critical-risk coverage; added-critical reporting | tests/ob03_closure.rs; verify-ob03.sh |
| B4 recipient-known-history delta; cold-start + at-head states | tests/ob04_delta.rs; verify-ob04.sh |
| B5 state-bound handoff validity; stale handoff never applied | tests/ob05_validity.rs; verify-ob05.sh |
| B6 omission challenge + uncertainty; challenged omission re-included with the challenge recorded; no omission hidden | tests/ob06_omission.rs; verify-ob06.sh |
| B7 progressive context repair: bounded repair loop, JSON-lines history file distinct from Option A, convergence/non-convergence reported, original handoff intact | tests/ob07_repair.rs; verify-ob07.sh |
| B8 comprehension + task-performance evaluation: frozen offline manifest, challenge probes + task benchmarks, withheld fails / repaired passes | tests/ob08_eval.rs; tests/fixtures/ob08-eval-manifest.json; verify-ob08.sh |
| B9 hierarchical and project summaries: content-addressed, reference exactly covered events, tampered/drifted rejected | tests/ob09_summaries.rs; verify-ob09.sh |
| B10 sufficiency/minimality claims backed by the B8 evaluation and the recorded metric; beyond-metric claims refused | tests/ob10_sufficient.rs; verify-ob10.sh |
| B11 recipient capability modeling: recorded + versioned model; mismatches flagged in the omission/uncertainty list, never assumed | tests/ob11_capability.rs; verify-ob11.sh |
| B2 semantic mechanisms resolved with a recorded non-adoption decision + baseline evidence | ob-12-semantic-mechanisms-audit.md; verify-ob12.sh |
| Phase success signal: recipient fails with withheld context, succeeds only after repair | src/bin/demo_ob.rs; scripts/demo-ob.sh; tests/ob13_demo.rs; verify-ob13.sh |
| Determinism on the structural path (no network, no wall clock, fixed seeds) | determinism tests in ob07/ob08/ob09/ob10/ob11/ob13; every gate offline |
| No omission hidden: omission list present on every handoff, never names a delivered source | ob06_omission + ob13_demo tests |
| Additive-only over Option A and prior OB packages (no module, wire, or schema change) | every gate Step 2 diff check |

## Limited (true within a stated bound)

| Claim | Bound stated |
|---|---|
| Determinism is guaranteed only on the structural and verification path | spec resolved decision 3; model inference is never in the path |
| Sufficiency/minimality claims hold only where a recorded metric proves them | OB-10 claim discipline; global minimality refused |
| Selection is task- and recipient-conditioned, never "the" relevant context | spec Never list; claim language throughout |
| Semantic mechanisms are not adopted; the baseline is the lexical selector | OB-12 decision; re-evaluable via the Ask-First gate |
| Comprehension is measured by the frozen in-repo task suite, not external benchmarks | spec resolved decision 5; external benchmarks advisory only |
| Repair-history is a distinct JSON-lines file, never Option A's DB and not a second embedded database | OB-07 scope; plan §3.2 out-of-scope bullet |

## Removed / absent (correctly not claimed)

- No claim that a selection is objective or "the" relevant context.
- No claim of minimal-sufficient context or comprehension beyond a recorded
  metric.
- No claim that semantic ranking is in use (non-adoption recorded).
- No consensus, blockchain, or A2A/ACP compliance claims.
- No claim that receipts enter Option A's store.
- No claim of a second embedded database process alongside Option A's Turso
  file.
- No claim of private chain-of-thought storage.

## Evidence owners

- Amelia (engineer) — claim audit for the Option B layer.
- Dr. Quinn (evaluation design) — eval-backed claim audit.
- Winston (architecture) — dependency and capability claim audit.
- Lunarpulse — final approval.
