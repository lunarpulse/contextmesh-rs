---
title: 'Option B — Effective Source-Grounded Context Handoff'
type: 'feature'
created: '2026-08-17'
status: 'approved'
approved: '2026-08-17'
approved_by: 'Lunarpulse'
phase: 'Option B — Effective Source-Grounded Context Handoff'
review_loop_iteration: 4
context: []
predecessor_spec: '../implementation-artifacts/spec-signed-agent-context-dag.md'
delivery_plan: '../planning-artifacts/option-a-delivery-plan.md'
option_b_gate: 'unblocked-by-complete-verdict'
---

# Option B — Effective Source-Grounded Context Handoff

> Approved v5 (feature-complete, frozen) — approved by Lunarpulse on
> 2026-08-17. Mirrors `spec-signed-agent-context-dag.md` and consumes plan
> section 3.2 (deferred items) and section 10 (the Option B unlock gate). Every
> planned feature maps to a completion gate; nothing is silently dropped.

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Option A delivers trustworthy, independently verifiable history —
integrity, not relevance. Option A's own projection is *caller-selected ancestry*
(semantic-free): it proves what happened and in what order, but says nothing
about which events matter to a downstream task or to a specific recipient.
Handing "all history" overflows context windows, exposes irrelevant material,
and provides no signal that the recipient understood or can act on the context.

**Seam vs Option A:** Option A selects by *caller-named ancestry*. Option B
selects by *task + recipient state*. Nothing in Option B changes what Option A
records; Option B only adds a layer that chooses, from that record, what to hand
off.

**Approach:** Option B is a derived selection-and-handoff layer over Option A's
immutable DAG. It selects task-conditioned sources as references into that DAG,
computes the delta against the recipient's known history, binds handoff validity
to recipient state, states omissions and uncertainty explicitly, and closes the
loop with progressive repair and downstream task-performance evaluation. Its full
scope is the union of plan section 3.2 and section 10, delivered as gates B1–B11.

**Phase success signal:** one end-to-end demonstration — a recipient with a
partial (known) history and a task receives a selection, challenges a deliberate
omission, the repair loop re-includes it, and the recipient succeeds on the task
only after repair. This is Option B's analogue of Option A's OA-06 two-node demo.

## Boundaries & Constraints

**Always:** Consume Option A events only as verifiable evidence; every selection
is a signed set of Option A event references, independently checkable against the
DAG, and an unverifiable reference is an error (never sanitized). Reuse Option A's
signing and identity scheme (Ed25519 `verify_strict` + BLAKE3 domain separation);
introduce no new signature primitive. Preserve Option A's frozen wire, bounds,
and claim discipline; Option A modules are untouched. Record selector identity,
selector version, and selector configuration hash in every receipt. Treat a
selection as recorded, not re-derived: the receipt — not a model rerun — is the
source of truth for what was selected, and determinism is guaranteed only on the
structural and verification path (DAG closure, delta, budget cut, signature
checks), never on model inference. Give "bounded" a concrete budget — a maximum
selected event count and a maximum exported byte size — enforced at handoff
time. State every omission explicitly; bind handoff validity to a recipient
known-history head and fail closed when stale. Respect Option A's
one-process-per-database-file constraint when reading the store the DAG lives
in. Measure comprehension by downstream task performance plus challenge probes,
never by self-report alone.

**Ask First:** Any change to Option A wire or bounds. Any selector dependency
(embedding model, vector index, reranker) — the selector is Option B's first
heavy-dependency point and is governed by Option A's forbidden-surface discipline
(no TLS stacks, HTTP/2/3, QUIC, cookies, compression, DNS resolvers, shells,
libp2p, or sqlite alternates). A claim of sufficiency, minimality, or
understanding beyond what a metric demonstrates. Storing private chain-of-thought.

**Never:** Mutate, reorder, or rewrite Option A history; receipts are Option B
artifacts and never enter Option A's store. Claim a selection is "the" relevant
context — it is task- and recipient-conditioned, not objective. Claim
minimal-sufficient context or comprehension without a metric that proves it. Add
consensus, blockchain, or A2A/ACP compliance. Emit a selection whose references
cannot be verified against the DAG.

## Completion Gates (B1–B11)

Plan section 10 items map to B1–B8; the remaining section 3.2 features map to
B9–B11. Each gate is an intent + success pair.

### B1 — Agent experience receipts
**Intent:** A signed Option B record of what an agent observed and selected,
expressed as references to Option A event IDs plus a task/recipient-state binding
and selector provenance (identity, version, configuration hash). Receipts are
Option B artifacts — not Option A events, and they never enter Option A's store.
They are self-contained, content-addressed, signed artifacts (canonical JSON +
Ed25519), persisted rather than derived-only; the first OB package stores them as
exported artifacts, not a second embedded database.
**Success:** A receipt verifies against the DAG (every referenced event exists,
is authorized, and is in the stated context) and rejects a tampered reference.

### B2 — Task-conditioned source selection
**Intent:** Given a task description — accepted as free text or a structured
query — select sources by semantic relevance (not ancestry alone) and assemble a
bounded set of source references via a context compiler. The receipt records the
task verbatim plus a content hash, and a structured canonical form only when the
caller supplies one; the system does not claim to derive a deterministic
canonical form from free text.
**Success:** Selection respects the budget; the receipt records selector
provenance; changing the selector version changes the recorded version, never the
history.

### B3 — Dependency closure and critical-risk coverage
**Intent:** The selected set is closed over DAG parent edges and covers flagged
critical/risk events; no dangling parent reference.
**Success:** A closure check reports zero dangling references; a deliberately
severed parent is rejected, not silently dropped.

### B4 — Recipient-known-history delta
**Intent:** Given the recipient's known-history head, select only events outside
the recipient's closure — the delta it does not yet have.
**Success:** The delta is provable: the recipient head is an ancestor in the same
DAG, and no selected event is already inside the recipient's closure. Recipient
*capability* modeling (B11) is a separate gate; B4 is strictly a known-history
delta.

### B5 — State-bound handoff validity
**Intent:** A handoff is valid only against a stated recipient head; if the
recipient has advanced, the handoff is stale.
**Success:** A stale handoff is rejected and re-derived, never applied.

### B6 — Omission challenge and uncertainty
**Intent:** Every handoff carries an explicit omission list and uncertainty
markers; a recipient can challenge an omission. This is the "handoff negotiation"
entry point from plan section 3.2.
**Success:** A challenged omission is re-included in a follow-up handoff with the
challenge recorded, and no omission is hidden.

### B7 — Progressive context repair
**Intent:** On comprehension or task failure, iteratively re-include omitted
context and re-handoff within a bounded repair loop.
**Success:** A repair sequence converges (or reports non-convergence) with the
attempt history recorded as evidence.

### B8 — Comprehension and downstream task-performance evaluation
**Intent:** An in-repo, frozen evaluation suite with two sub-modes — (a)
challenge probes (does the recipient notice a withheld critical fact?) and (b)
task benchmarks (does the recipient complete the downstream task?) — not human
judgment. The suite is a curated task set with a known critical-context
annotation, so the withheld/repaired cases are deterministic and offline;
external benchmarks may be advisory, never the gate.
**Success:** The withheld-context case fails and the repaired case passes,
demonstrating the selection was load-bearing.

### B9 — Hierarchical and project summaries
**Intent:** Produce derived, verifiable summaries at hierarchical levels (event →
ref → project) as content-addressed references over Option A history, so a
recipient can enter a large history at the right altitude.
**Success:** A summary references only the events it summarizes, verifies against
the DAG, and a tampered or drifted summary is rejected.

### B10 — Minimal-sufficient-context computation
**Intent:** Compute a selection that is demonstrably sufficient (the task
succeeds under B8) and, where a defined metric exists, minimal — never claim
minimality or sufficiency beyond what that metric shows.
**Success:** The sufficiency claim is backed by the B8 evaluation; the minimality
claim is backed by a recorded metric (selected count/bytes against budget); any
claim beyond the metric is refused.

### B11 — Recipient capability modeling
**Intent:** Model what a recipient can do alongside what it knows, and use that
model to shape the handoff so an event the recipient cannot act on is not
silently handed off.
**Success:** The capability model is recorded and versioned; a handoff respects
the recipient's stated capabilities, and a capability mismatch is flagged in the
omission/uncertainty list rather than assumed.

## I/O & Edge-Case Matrix

- Empty history → selection is empty with an explicit "no sources" marker, not a
  fabricated context.
- Cold-start recipient (empty known history) → the delta equals the full
  selection.
- Empty or absent task → fail closed; no selection is produced.
- Recipient head not present in the DAG → B4/B5 fail closed (unknown recipient
  state), never assumed.
- Unverifiable reference → receipt rejected (Never rule), not sanitized.
- Task with no matching source → empty selection plus an uncertainty marker, not
  a hallucinated mapping.
- Selector errors or is unavailable → fail closed; handoff aborted, prior state
  intact.
- Repair loop exceeds its bound → non-convergence reported, original handoff left
  intact.

## Feature-completeness map

Every planned Option B item (plan section 3.2 ∪ section 10) has an owner. No
feature is deferred without a gate.

| Plan item | Owner |
|---|---|
| §10.1 agent experience receipts | B1 |
| §10.2 task-conditioned source selection | B2 |
| §3.2 semantic/critical context selection | B2 |
| §3.2 embeddings, vector search, reranking, context compilers | B2 mechanisms (named in the delivery plan; forbidden-surface governed) |
| §10.3 dependency closure + critical-risk coverage | B3 |
| §10.4 recipient-known-history delta | B4 |
| §3.2 recipient knowledge modeling | B4 |
| §10.5 state-bound handoff validity | B5 |
| §10.6 omission challenge + uncertainty | B6 |
| §3.2 context handoff negotiation | B6 |
| §10.7 progressive context repair | B7 |
| §3.2 progressive source-grounded context repair | B7 |
| §10.8 comprehension + task-performance evaluation | B8 |
| §3.2 comprehension verification | B8 |
| §3.2 hierarchical or project summaries | B9 |
| §3.2 minimal-sufficient-context computation | B10 |
| §3.2 recipient capability modeling | B11 |
| §3.2 claims that ancestry is relevant/sufficient | Boundaries (claims discipline), not a feature |

## Resolved decisions (party-mode consensus, 2026-08-17)

1. **Task input shape** — both free text and structured queries are accepted as
   inputs. The receipt records the task verbatim plus a content hash, and a
   structured canonical form only when the caller supplies one; the system does
   not claim to derive a deterministic canonical form from free text.
2. **Receipt persistence** — receipts are self-contained, content-addressed,
   signed artifacts (canonical JSON + Ed25519), persisted, never derived-only.
   OB-01 ships them as exported artifacts; the repair-history store required by
   B7 is a scheduled capability in the B7 package, implemented as a distinct file
   that never touches Option A's DB.
3. **Determinism** — selection is recorded, not re-derived. Cross-run determinism
   is not guaranteed over model inference (infeasible across hardware/versions);
   determinism is guaranteed only on the structural and verification path (DAG
   closure, delta, budget cut, signature checks).
4. **Recipient capability modeling** — committed as B11, sequenced after OB-01,
   not dropped. B4 remains the known-history delta.
5. **Task-performance suite source** — an in-repo, frozen task suite (curated
   tasks with a known critical-context annotation). External benchmarks may be
   advisory, never the gate.

## Assumptions

- Option B is implemented as new modules under `src/`; no Option A module, wire
  byte, or database-schema change.
- The selection budget (event count + byte size) is a first-class parameter, not
  an afterthought.
- Semantic ranking (embeddings, vector search, reranking) and the context
  compiler are B2's named mechanisms; each dependency requires Ask-First approval
  and passes the forbidden-surface audit before adoption.

</frozen-after-approval>

## Approval Record

- **Decision:** Approved by Lunarpulse on 2026-08-17.
- **Specification:** `spec-option-b-source-grounded-context-handoff.md`.
- **Gate set:** B1–B11 — the union of plan section 3.2 and section 10, with no
  feature deferred or dropped.
- **Consensus:** Party-mode room decision of 2026-08-17 folded into the spec as
  "Resolved decisions 1–5" (task shape, receipt persistence, determinism,
  capability modeling, eval source).
- **Sequence:** Option B implementation begins only after this freeze; packages
  are sequenced by the Option B delivery plan, preserving Option A's frozen wire,
  bounds, and forbidden-surface discipline.

## Code Map

None yet. Option B artifacts (receipt, selection, delta, repair, evaluation,
summaries) land in new modules under `src/` during OB-xx implementation; Option A
modules remain untouched.

## Tasks & Acceptance

Decomposed in the forthcoming Option B delivery plan, which sequences B1–B11 into
OB-xx packages and assigns each gate a test and an evidence owner. This spec
freezes intent, boundaries, and the complete gate set; it does not sequence the
packages.

## Spec Change Log

- 2026-08-17 — approved v5: frozen after Lunarpulse approval; status moved to
  `approved`; `frozen-after-approval` wrapper added mirroring the Option A spec;
  Approval Record added; review_loop_iteration 4.
- 2026-08-17 — draft v1 authored from plan section 3.2 + section 10.
- 2026-08-17 — draft v2: self-critique applied (Option A seam, phase success
  signal, receipt-not-an-event, crypto reuse, selector provenance + budget,
  embedding/determinism tension, executable B8 suite, cold-start and
  selector-error edge cases, Open Questions + Assumptions).
- 2026-08-17 — draft v3: party-mode consensus resolved the five open questions
  (both task forms recorded verbatim; receipts as persisted self-contained
  artifacts; recorded-not-re-derived selection with determinism scoped to the
  structural path; capability modeling deferred; in-repo frozen eval suite).
- 2026-08-17 — draft v4: feature-completeness pass — added B9 (hierarchical/project
  summaries), B10 (minimal-sufficient-context computation), B11 (recipient
  capability modeling); expanded B2 to require semantic ranking + context
  compiler; replaced all "deferred/if-ever-needed" language with sequenced
  commitments; added the feature-completeness map proving every §3.2 + §10 item
  has an owner.

## Design Notes

- Selection is references, not copies, and recorded, not re-derived: the receipt
  is a new Option B artifact that points at Option A history; Option A history
  stays immutable, and the receipt — not a model rerun — is the source of truth
  for what was selected.
- B4 + B5 together make handoff idempotent and state-safe; the recipient head is
  the checkpoint.
- B6 makes selection honest: omissions are first-class, challengeable data.
- B7 + B8 operationalize comprehension as "task succeeds with the selected
  context and fails-then-recovers when critical context is withheld."
- The selector is the first point where Option B may need a heavier dependency;
  it is governed by the same forbidden-surface discipline as Option A.

## Verification

Mirrors Option A's A1–A8: each B-gate (B1–B11) maps to an executable test and an
evidence owner in the Option B delivery plan. This spec authorizes no product code
and no Option A byte changes.
