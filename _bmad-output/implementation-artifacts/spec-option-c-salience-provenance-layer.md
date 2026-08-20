---
title: 'Option C — Salience Provenance Layer (Attention Ledger)'
type: 'feature'
created: '2026-08-20'
status: 'draft'
approved_by: 'pending founder review'
phase: 'Option C — Salience Provenance Layer'
review_loop_iteration: 0
context:
  - '../implementation-artifacts/spec-signed-agent-context-dag.md'
  - '../implementation-artifacts/spec-option-b-source-grounded-context-handoff.md'
charter: '2026-08-20 founder session — frozen dependency discipline opened for Option C'
delivery_plan: 'pending — to be authored after founder approval of this spec'
---

# Option C — Salience Provenance Layer (Attention Ledger)

> DRAFT awaiting founder review. On approval, the Intent and Boundaries
> section below becomes `<frozen-after-approval>` human-owned intent, exactly
> as in the Option A and Option B specifications.

## Charter (recorded 2026-08-20)

The founder opened Option A's frozen dependency discipline for this Option C
design session: embeddings, vector search, rerankers, LLM judges, wall-clock
time, and native runtimes are permitted mechanisms. This spec codifies that
opening while preserving what the founder identified as structural lessons,
not constraints:

1. Option C artifacts are **signed, content-addressed records referencing
   Option A EventIds**, verified fail-closed against the DAG (B1 discipline).
2. Selector/attribution provenance (identity, version, configuration hash) is
   recorded in every artifact.
3. Option A history is never mutated; Option C only adds derived layers.

## Intent

**Problem:** Option A proves *what happened*; Option B selects *what to hand
over* — but under OB-12 non-adoption its baseline selector is lexical-TF
only, and no layer records *why events mattered*: which events were
load-bearing for an outcome, which attempts failed and how, and what they
cost in time and calls. Lexical selection collapses to random-level
precision when task wording mismatches event vocabulary (measured:
precision@12 = 0.014 = random expectation on the OC probe set), and
failure records are scattered rather than reused as negative knowledge.

**Founder's asks, formalized:** (a) terminal outcome nodes should carry what
failed and what the strongest clue was, (b) how long the work took, (c)
these relations should power a context assembler that points to the few
blocks that matter, blockchain-explorer style, instead of compacting by
destruction.

**Approach:** an Attention Ledger above Option A/B. Attribution is measured
by **intervention, not correlation**: cheap deterministic mechanisms
nominate candidate load-bearing events (string-overlap backtracking,
normalized value matching, citation analysis) and expensive causal
mechanisms verify them (leave-one-out counterfactual ablation,
Shapley-sampling coalition attribution over shortlists). Outcomes are
recorded as Outcome Ledgers (cost, attempts, dead ends, load-bearing set
with mechanism provenance). Dead ends accumulate into a Thorn Index
(negative knowledge); annotations propagate as a versioned Salience Prior
over DAG-parent and entity edges (personalized PageRank). Selection fuses
the lexical baseline with priors and thorn suppression, gated by a B8-style
evaluation before any deployment claim.

**Phase success signal:** on the frozen vocabulary-mismatch probe set,
prior-fused selection beats both the lexical baseline and random
expectation at fixed budget with all influences stated explicitly; the
validation gate reproduces deterministically offline from a clean
checkout, Python ground truth and Rust port agreeing.

## Boundaries & Constraints

**Always:** Verify every ledger/receipt reference against the Option A DAG
(exists, authorized, same context); reject — never sanitize — an
unverifiable reference. Record mechanism provenance (identity, version,
configuration hash) for every attribution and selection. Record wall-clock
and call counts when available and mark them `clock: unavailable` when not;
never fabricate times. State thorn influence and omissions explicitly in
every selection's uncertainty list. Gate every selector/prior version with
the frozen evaluation before any sufficiency claim; claim nothing beyond
what the metric demonstrates. Guarantee determinism only on structural and
verification paths; model-inference mechanisms are recorded, never
re-derived.

**Ask First:** Any change to Option A wire, bounds, or claim discipline.
Adopting any heavy dependency into the Option A core closure (which stays
frozen at 320 — the charter opens Option C's own budget, not the core's).
Extending the charter opening to Options A/B artifacts themselves.

**Never:** Mutate, reorder, or rewrite Option A history. Store Option C
artifacts inside the Option A store. Claim objective salience, minimality,
sufficiency, or recipient comprehension beyond recorded metrics. Hide a
dead end or filter negatives silently. Emit an artifact whose references do
not verify against the DAG.

## Completion Gates (C1–C5)

### C1 — Outcome Ledger artifacts
**Intent:** Per task termination, a signed, content-addressed
`OutcomeLedgerV1` referencing Option A EventIds: outcome and quality, cost
ledger (wall-clock, tool calls, retries, tokens when available), attempt
tree with error fingerprints, load-bearing set with per-event attribution
and mechanism tags, dead-end list. Ledger artifacts live outside the
Option A store (B1 receipt discipline: exported artifacts, not events).
**Success:** A ledger verifies against the DAG and rejects a tampered or
cross-context EventId reference; provenance of the generating run is
recorded; a task without a terminal answer event yields an explicit
`unterminated` marker, never a fabricated ledger.

### C2 — Attribution mechanism ladder
**Intent:** Mechanisms M0 (raw string-overlap backtracking), M1
(normalized-value nomination), M2 (citation analysis), M3 (single-event
counterfactual ablation), M4 (Shapley-sampling coalition attribution).
Composition rule: **cheap mechanisms nominate, expensive mechanisms verify
causally** — M3/M4 run only on M0–M2 shortlists, never the whole DAG.
Every attribution carries its mechanism tag, version, and configuration
hash.
**Success:** The frozen E1-class evaluation demonstrates the ladder's
precision/recall/cost trade-off and that nominate-verify strictly
dominates naive union (measured: union F1 0.819 vs M4 F1 0.997 at 314.5
judge calls/session on the OC set); redundant carriers are credited by M4
where M3 under-marks by design, both recorded.

### C3 — Thorn Index and Salience Prior
**Intent:** Dead-end fingerprints (failure mode × entity × cost) accumulate
into a versioned `ThornIndexV1`; annotations (load-bearing marks, thorns)
propagate as a versioned `SaliencePriorV1` over DAG-parent and bounded
entity edges via personalized PageRank (thorns as negative seeds).
**Success:** Prior and thorn versions are recorded; thorn proximity
suppresses dead-end recurrence in selection with the influence stated in
the uncertainty list; an all-failure history produces explicit thorn
warnings rather than silent filtering.

### C4 — Prior-fused selection
**Intent:** Fuse the lexical-TF baseline with salience prior and thorn
suppression (`score = tf × (1 + α·prior) × (1 − β·thorn)`), with α/β
recorded per selector version; every deployment claim is gated by the
frozen evaluation.
**Success:** On the frozen vocabulary-mismatch probe set, prior-fused
selection beats the lexical baseline and random expectation at fixed
budget (measured direction: precision@12 0.042 vs 0.014 baseline =
random); no claim beyond the demonstrated budget regime.

### C5 — Deterministic validation gate
**Intent:** Freeze the prototype validation (E1 attribution ladder, E2
propagation/selection, E3 cost ledger) as a seeded, offline, deterministic
test: Python ground truth and Rust port must agree — E1/E3 exactly, E2
within a declared float tolerance for set-iteration order.
**Success:** The gate passes from a clean checkout with the pinned
toolchain; results are recorded in
`_bmad-output/verification-artifacts/oc-00-prototype-validation.md`.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Behavior | Error Handling |
|---|---|---|---|
| Unterminated task | No terminal answer event | Explicit `unterminated` marker | No fabricated ledger |
| Empty shortlist | M0–M2 nominate nothing | M3/M4 skipped, recorded as `no nominations` | Not an error |
| Judge unavailable | Causal tier cannot run | Fail closed for M3/M4; M0/M1 marks recorded with uncertainty marker | No causal claim made |
| Redundant carriers | Two events carry the same fact | M3 under-marks by design; M4 splits credit | Both outcomes recorded |
| Thorn-only history | All prior sessions failed | Empty prior, explicit thorn warnings, lexical baseline proceeds | No silent negative filter |
| Cross-context reference | Ledger cites event of another context | Rejected as unverifiable | Typed error, no partial artifact |
| Timeless environment | Wall-clock unavailable | Cost ledger records `clock: unavailable` | Never fabricate times |
| Vocabulary mismatch | Task wording ≠ event wording | Prior carries selection; uncertainty list states the regime | No relevance claim beyond probe set |

## Feature-completeness map

| Founder ask (2026-08-20 session) | Owner |
|---|---|
| "무엇이 실패했는지" (what failed) | C1 dead-end list + C3 Thorn Index |
| "제일 큰 단서" (strongest clue) | C2 load-bearing set |
| "얼마나 시간이 걸렸는지" (elapsed time) | C1 cost ledger |
| Context assembler pointing at the few blocks that matter | C3 warnings + C4 fusion (over B2/B4/B6/B9 mechanisms) |
| Cross-session 연결관계 (relations between sessions) | C3 propagation over entity + parent edges |

## Dependency policy (amnesty clause)

OB-12's NON-ADOPTION remains true **for the Option A core closure** (320
crates, offline). Option C lives as a separate crate/feature
(`contextmesh-salience`) with its own dependency budget; heavy mechanisms
(embedding models, judge sidecars) sit behind the attribution/selector
traits as external adapters — the "thin mapping onto stable library types"
pattern the Option A spec reserves for adapters. This charter supersedes
OB-12's scope for Option C artifacts only, by founder authority recorded
above; OB-12's method (recorded decision with audit evidence) is kept.

## Code map (validated prototype)

- `_bmad-output/implementation-artifacts/oc-prototype-validation/` — the
  session's validated prototype, recorded verbatim:
  - `prototype.py` — Python ground truth (stdlib only, seeded).
  - `src/` — Rust port, zero external dependencies, including
    `src/rng.rs` (CPython-compatible MT19937 verified against captured
    draws) so the port reproduces the Python streams exactly.
  - `DESIGN.md`, `results.json`, `results-rust.json`, `compare.py` —
    design note, both result sets, and the port-verification gate.
- Future Option C packages (OC-01..OC-05) implement C1–C5 in the
  `contextmesh-salience` crate per the delivery plan to be authored after
  approval.

## Approval Record

- **Charter:** 2026-08-20 — founder opened the frozen dependency
  discipline for Option C design (this session).
- **Spec approval:** PENDING. This document is a draft for founder review;
  nothing in it is frozen until approved. On approval, wrap Intent and
  Boundaries in `<frozen-after-approval>`, set `status: 'approved'`, and
  author `_bmad-output/planning-artifacts/option-c-delivery-plan.md`
  mapping OC-01..OC-05 to gates C1–C5.
