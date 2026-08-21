---
title: 'Option C — Salience Provenance Layer (Attention Ledger)'
type: 'feature'
created: '2026-08-20'
status: 'approved'
approved: '2026-08-21'
approved_by: 'Lunarpulse'
approval_source: 'Discord message 1540302757649457254'
phase: 'Option C — Salience Provenance Layer'
review_loop_iteration: 2
context:
  - '../implementation-artifacts/spec-signed-agent-context-dag.md'
  - '../implementation-artifacts/spec-option-b-source-grounded-context-handoff.md'
charter: '2026-08-20 founder session — frozen dependency discipline opened for Option C'
delivery_plan: '../planning-artifacts/option-c-priority-and-gate-plan.md'
priority_plan: '../planning-artifacts/option-c-priority-and-gate-plan.md'
integration_decision_record: '../planning-artifacts/oc-00-5-founder-decision-record.md'
---

# Option C — Salience Provenance Layer (Attention Ledger)

> Approved and frozen by Lunarpulse on 2026-08-21 after OC-0.5 integration
> audit, Hermes real-data replay, and two independent documentation reviews.
> D-C-00 through D-C-10 govern execution and change control.

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

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

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
with mechanism provenance). Positive annotations propagate as a versioned
nonnegative Salience Prior over DAG-parent and entity edges. Eligible,
conditional dead ends separately propagate as nonnegative Thorn proximity;
the channels are never collapsed into signed or negative PPR seeds. Selection
forms a bounded lexical/prior candidate union before deterministic reranking.
Thorn suppression remains off until its later independent gate. Every
deployment claim is gated by preregistered human-gold and B8-style evaluation.

**Phase success signal:** on a preregistered temporal-family human-gold test
including a strict all-gold-TF=0 stratum, positive-prior candidate union beats
the lexical baseline on the frozen primary metric at fixed budget, while the
OC-to-Option-B execution binding and structural validation reproduce exactly
offline from a clean checkout. Existing OC-00 synthetic and Hermes silver
replays are directional baselines, not the phase-completion gate.

## Boundaries & Constraints

**Always:** Verify every ledger/receipt reference against the Option A DAG
(exists, authorized, same context); reject — never sanitize — an
unverifiable reference. Record mechanism provenance (identity, version,
configuration hash) for every attribution and selection. Record wall-clock
and call counts when available and mark them `clock: unavailable` when not;
never fabricate times. State prior influence and omissions explicitly; when
Thorn is separately approved and enabled, state its influence explicitly in
the selection uncertainty. Gate every selector/prior version with
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

</frozen-after-approval>

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
**Intent:** Positive load-bearing annotations propagate into a versioned
nonnegative fixed-point `SaliencePriorV1`. Conditional dead-end fingerprints
(failure mode × entity × cost × task/recipient/world state × expiry) accumulate
separately in a bounded `ThornIndexV1` and propagate into a nonnegative
fixed-point Thorn-proximity channel. Both use deterministic bounded graphs and
separate provenance; production does not use negative PPR seeds.
**Success:** Positive-prior exact vectors pass before any Thorn rollout. Prior
and Thorn versions/channels are recorded independently; Thorn is disabled until
human gold demonstrates incremental benefit over positive-only selection and
false suppression below the frozen threshold. An all-failure history produces
explicit warnings rather than silent filtering.

### C4 — Prior-assisted selection and execution binding
**Intent:** Generate bounded lexical and positive-prior candidate arms, take a
deterministic EventId-deduplicated union, then rerank under the preregistered
normalization/formula/caps/tie-break/overflow configuration. A TF=0 event can
enter through the prior arm. `SelectionInfluenceV1` records ordered influence;
`SelectionExecutionV1` binds the exact pre-closure IDs and budget through B3
closure, B4 delta, B5 state verification, B6 uncertainty, and final handoff.
Thorn is a separate later reranker input and is off by default.
**Success:** On preregistered human gold, positive-prior selection passes the
frozen nDCG@12/Any-hit@12, family-bootstrap, strict TF=0, and Option B regression
gates. Influence/execution mismatch or stale state fails closed with neither a
deliverable handoff nor execution artifact. No claim extends beyond the tested
budget and label regime.

### C5 — Deterministic validation and claim gate
**Intent:** Preserve OC-00 (E1 attribution ladder, E2 propagation/selection,
E3 cost ledger) as seeded directional prototype evidence, then add production
vectors for strict OC artifacts, corrected shortlist attribution, separate
fixed-point prior/Thorn channels, and OC-to-Option-B execution binding.
Production content-addressed paths require exact canonical-byte equality; float
tolerance remains prototype-only.
**Success:** A clean checkout on the pinned toolchain passes exact artifact,
fixed-point, integration, dependency-closure, privacy, real-replay, B8, and claim
audit gates. OC-00 Python/Rust evidence remains recorded at
`_bmad-output/verification-artifacts/oc-00-prototype-validation.md` but does not
alone complete C5.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Behavior | Error Handling |
|---|---|---|---|
| Unterminated task | No terminal answer event | Explicit `unterminated` marker | No fabricated ledger |
| Empty shortlist | M0–M2 nominate nothing | M3/M4 skipped, recorded as `no nominations` | Not an error |
| Judge unavailable | Causal tier cannot run | Fail closed for M3/M4; M0/M1 marks recorded with uncertainty marker | No causal claim made |
| Redundant carriers | Two events carry the same fact | M3 under-marks by design; M4 splits credit | Both outcomes recorded |
| Thorn-only history | All prior sessions failed | Empty positive prior, explicit Thorn warnings, lexical baseline proceeds; suppression disabled until approved | No silent negative filter |
| Cross-context reference | Ledger cites event of another context | Rejected as unverifiable | Typed error, no partial artifact |
| Timeless environment | Wall-clock unavailable | Cost ledger records `clock: unavailable` | Never fabricate times |
| Vocabulary mismatch | Task wording ≠ event wording | TF=0 event may enter through bounded prior arm; influence records entry reason | No relevance claim beyond human-gold gate |

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

## OC-0.5 integration status (2026-08-21)

Option B has merged to `main`, this branch has been rebased onto that merge,
and the repository/real-data audit is recorded. The final execution priorities
are now repository-owned by
`../planning-artifacts/option-c-priority-and-gate-plan.md`. The binding founder
dispositions are approved in
`../planning-artifacts/oc-00-5-founder-decision-record.md`.

This specification incorporates the approved OC-0.5 dispositions:

1. C2's M3/M4 shortlist rule is authoritative; OC-00 full-candidate M3 is
   directional evidence and must be rerun under the approved pipeline.
2. Bounded lexical/prior candidate union precedes deterministic reranking so a
   prior-nominated TF=0 event is not structurally excluded.
3. C3/C4 production scores are separate nonnegative fixed-point prior/Thorn
   channels with exact production reproduction; float tolerance is
   prototype-only.
4. Positive prior ships before Thorn; Thorn remains off until its independent
   human-gold false-suppression gate.
5. C4 deployment claims require preregistered human-gold and B8 evidence, not
   the current entity-continuity silver replay alone.
6. `SelectionInfluenceV1` and `SelectionExecutionV1` bind ranking evidence to
   the exact B3–B6 execution without changing Option B wires.

The dispositions and human-owned intent are frozen. Any amendment requires
founder renegotiation and must update the decision record and this spec together.

## Approval Record

- **Charter:** 2026-08-20 — founder opened the frozen dependency
  discipline for Option C design (this session).
- **Spec approval:** APPROVED 2026-08-21 by Lunarpulse. D-C-00 through D-C-10
  were approved in full; Intent and Boundaries are frozen. Approval authorizes
  P1 under the priority plan but does not claim C1–C5 completion.
- **Approval provenance:** Discord message `1540302757649457254` directed spec
  freeze and P0 commit.
