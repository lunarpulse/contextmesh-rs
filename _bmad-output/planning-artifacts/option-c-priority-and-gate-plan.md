---
title: 'Option C Priority and Gate Plan — OC-0.5 through OC-05'
type: 'delivery-priority-plan'
created: '2026-08-21'
status: 'approved-for-execution'
approved: '2026-08-21'
approved_by: 'Lunarpulse'
approval_source: 'Discord message 1540302757649457254'
branch: 'OC-AttentionLedger'
baseline_commit: '17d7730ad18f142e55b67a650f060814534507f8'
source_spec: '../implementation-artifacts/spec-option-c-salience-provenance-layer.md'
decision_record: './oc-00-5-founder-decision-record.md'
integration_audit: '../verification-artifacts/oc-00-5-post-rebase-integration-audit.md'
real_data_evidence: '../verification-artifacts/oc-00-5-real-data-replay.md'
---

# Option C Priority and Gate Plan

## 1. Purpose and present status

This document makes the final P0–P5 sequence a repository-owned execution
contract. The sequence was previously recorded only in the project Obsidian
note and discussed in review; it was not an independent repository gate.

Lunarpulse approved this plan and D-C-00 through D-C-10 on 2026-08-21. P0 is
the approved integration freeze and P1 planning/implementation is authorized
under its gates. Approval does not make C1–C5 complete and does not approve a
deployment claim.

## 2. Sources and reliability

| Source | Accessed | Reliability | Use |
|---|---|---:|---|
| Option C draft specification | 2026-08-21 | Primary draft intent | C1–C5 and founder charter |
| OC-0.5 post-rebase integration audit | 2026-08-21 | Primary local code/contract audit | integration gaps and reusable seams |
| Hermes real-data replay evidence | 2026-08-21 | Primary execution evidence, silver labels | positive-prior direction and validity limits |
| OC prototype validation | 2026-08-21 | Primary seeded synthetic evidence | mechanism ladder and port determinism |
| Option B frozen spec, delivery plan, source, and tests | 2026-08-21 | Primary approved contract | immutable downstream handoff path |
| local Git graph at `17d7730` | 2026-08-21 | Primary | ancestry and baseline |

No web source is needed. This plan governs the local repository and its
human-owned contracts.

## 3. Decision method

Priorities are ordered by four criteria:

1. **Dependency:** a later package cannot define a valid contract without the
   earlier one.
2. **Evidence strength:** positive-prior evidence is directionally repeated on
   synthetic and real data; Thorn and strict TF=0 behavior are not.
3. **Blast radius:** separate artifacts and additive bridges precede changes
   near the frozen Option B selection path.
4. **Falsifiability:** every step must have a recorded go/no-go gate before the
   next claim is unlocked.

Alternatives rejected:

- Implement C4 first: rejected because the fusion rule fails structurally at
  TF=0 and has no human-gold gate.
- Add C fields to Option B receipts: rejected because the B1 wire is strict and
  frozen.
- Implement prior and Thorn together: rejected because the real replay shows
  prior benefit but no independent Thorn benefit; combining them hides cause.
- Promote the synthetic prototype directly: rejected because it has a synthetic
  event schema and no production `contextmesh` dependency.

## 4. Final priority sequence

### P0 — OC-0.5 integration decision freeze

**Goal:** Resolve and approve the eleven decisions in
`oc-00-5-founder-decision-record.md` before production code.

**Outputs:**

- founder disposition of the Option C draft;
- separate-crate and dependency direction;
- artifact envelope, signature domains, typed failures, and hard bounds;
- additive `SourceEvent` to `SourceReference` bridge;
- richer OC selection result without changing `Selector`;
- signed influence-to-B3/B4/B5/B6 execution binding;
- TF=0 candidate/fusion policy;
- M3 shortlist and M2 citation policies;
- terminal, cost, and entity semantics;
- fixed-point scoring;
- real-data and human-gold acceptance thresholds.

**Gate P0-GO:** **PASSED 2026-08-21.** Founder approved D-C-00 through D-C-10;
the frozen spec links to the approved record and no unresolved blocking decision
remains.

**P0-NO-GO:** Any blocking item remains `pending`, or approval requires an
Option A/B wire break not separately approved.

### P1 — OC-01 Outcome Ledger plus preregistered human-gold protocol

**Goal:** Build the lowest-ambiguity signed evidence substrate while freezing
the evaluation that later selection work must pass.

**Production track:** `OutcomeLedgerV1`, explicit terminal EventId,
`Available | Unavailable` costs, distinct signature domain, content address,
DAG/context verification, tamper/cross-context/bounds vectors.

**Evaluation track (parallel, no test-label opening yet):** freeze sample strata,
labels (`required`, `supporting`, `irrelevant`, `dead_end`, `uncertain`), family
split, extractor versions, budgets, metrics, family-cluster bootstrap, score
normalization, exact rerank formula, lexical/prior per-arm candidate caps,
deduplication/tie-break, and checked-overflow policy. Include a strict
all-gold-TF=0 stratum. Serialize that configuration canonically and freeze its
hash before P2 implementation and before test-label inspection.

**Gate P1-GO:** Outcome artifacts verify deterministically and the preregistration
hash—including the formula, normalization, per-arm caps, and overflow policy—is
frozen before P2 implementation or test-label inspection.

### P2 — OC-02 attribution plus positive-only fixed-point prior

**Goal:** Implement deterministic M0/M1, explicit structural M2, shortlist-bound
M3/M4 adapters, and a positive-only `SaliencePriorV1`.

**Boundary:** Current Thorn suppression remains disabled. The OC-00 prototype
remains directional evidence because its full-candidate M3 does not execute the
approved shortlist policy.

**Gate P2-GO:** Corrected E1 gate, exact fixed-point prior vectors, provenance and
judge-unavailable tests pass; human-gold positive-prior arm passes the threshold
frozen in P0.

### P3 — OC-04 positive-prior selection integration

**Goal:** Deterministically union lexical candidates with positive-prior
candidates, rerank with recorded influence, then pass only verified references
through Option B closure, delta, stale-state handoff, uncertainty, repair, and
B8 evaluation.

The adapter must issue a signed `SelectionExecutionV1` binding the verified
`SelectionInfluenceV1`, exact pre-closure IDs/budget, the versioned deterministic
critical-candidate projection derived from recorded input refs, B3 policy and
candidate fingerprints, closed-selection hash/count, delta hash/count, recipient
head, final handoff hash, and propagated B6 warnings. A state change or mismatch
returns neither a deliverable handoff nor an execution artifact.

**Boundary:** Do not replace or widen `Selector::select`; do not bypass B3–B8;
do not enable Thorn.

**Gate P3-GO:** strict TF=0 candidates can enter through the prior arm; all
influences are recorded; Option B regression and withheld/repaired task gates
pass; the preregistered primary metric passes.

### P4 — conditional and expiring Thorn experiment

**Goal:** Test negative knowledge independently after positive-prior integration
is stable.

Required semantics: failure category, tool/operation, task and recipient
conditioning, world-state binding, expiry, resolution/retry outcome, and
explicit warnings. Positive prior and thorn proximity are separate nonnegative
fixed-point channels; suppression happens only in the recorded reranker. Default
deployment state is off.

**Gate P4-GO:** Human labels demonstrate incremental benefit over positive-only
selection and false suppression remains below the frozen threshold. Otherwise
retain Thorn as recorded evidence only.

### P5 — OC-05 release and claim gate

**Goal:** Freeze artifact vectors, exact fixed-point reproduction, corrected
attribution, OC-to-OB end-to-end behavior, dependency closure, privacy, real
replay, and claim audit.

**Gate P5-GO:** C1–C4 evidence owners pass, no core dependency/wire regression,
clean-checkout gate passes, and claims are limited to measured regimes.

## 5. Package and priority mapping

| Priority | Package/gate | Primary deliverable | Claim unlocked |
|---|---|---|---|
| P0 | OC-0.5 | approved integration decisions | implementation authorization only |
| P1 | OC-01 / C1 | signed Outcome Ledger + preregistration | artifact integrity, not salience utility |
| P2 | OC-02 + positive C3 subset | corrected attribution + fixed-point positive prior | human-gold positive-prior evidence only |
| P3 | OC-04 / C4 positive path | prior-assisted Option B handoff | bounded measured selection improvement |
| P4 | C3 Thorn subset | conditional negative knowledge | only if incremental and safe |
| P5 | OC-05 / C5 | release evidence and claim audit | release within recorded bounds |

OC-03 is deliberately split: positive prior work is P2 because evidence is
stronger; Thorn is P4 because present real-data evidence does not support it.

## 6. Change control

Founder approval is required to:

- reorder P0–P5;
- change an approved OC-0.5 decision;
- modify Option A or Option B wire, bounds, database schema, or claim discipline;
- allow `contextmesh` to depend on `contextmesh-salience`;
- enable model inference without a recorded adapter artifact;
- enable Thorn before its independent gate;
- weaken exact fixed-point or privacy requirements;
- claim causal load-bearingness, comprehension, sufficiency, or task success
  beyond the applicable evidence.

## 7. What would make this plan wrong?

- Human-gold evaluation fails to reproduce positive-prior improvement.
- The founder keeps Option C research-only rather than production-bound.
- A safer already-public Option B bridge exists outside the audited baseline.
- Fixed-point propagation cannot preserve acceptable ranking quality.
- Candidate union creates unacceptable latency or closure expansion at the
  frozen budgets.

Any such result pauses execution and amends this plan rather than silently
changing the gate.

## 8. Immediate action

1. Commit the approved P0 record, plan, and frozen Option C spec together.
2. Author the detailed OC-01 implementation spec and test matrix.
3. In parallel, author and hash the P1 human-gold preregistration before P2
   implementation or test-label inspection.
