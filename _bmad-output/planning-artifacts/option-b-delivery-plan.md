---
title: 'Option B Delivery Plan — Effective Source-Grounded Context Handoff'
type: 'delivery-plan'
created: '2026-08-17'
status: 'approved-for-execution'
approved: '2026-08-17'
approved_by: 'Lunarpulse'
owner: 'Product / Engineering'
source_spec: '../implementation-artifacts/spec-option-b-source-grounded-context-handoff.md'
predecessor_plan: './option-a-delivery-plan.md'
option_b_gate: 'unblocked-by-complete-verdict'
---

# Option B Delivery Plan

## 1. Decision and Product Sequence

Lunarpulse approved the Option B feature spec on 2026-08-17 and froze it as
commit `a950836` ("OB-00: freeze Option B source-grounded context handoff spec
(B1-B11)") on branch `ESgCHandoff`. Lunarpulse approved this delivery plan on
2026-08-17; its status is `approved-for-execution`. Option A is complete (OA-07
verdict `complete`), so Option B is unblocked by
`unblocked-by-complete-verdict`.

The execution sequence is fixed:

1. **Build the selection layer as a derived layer over Option A's immutable
   DAG.** Every Option B artifact is a signed reference into Option A history;
   Option A modules, wire bytes, and database schema are untouched.
2. **Pass every completion gate B1–B11.** Each gate is an intent + success pair
   in the frozen spec, owned by an executable test and an evidence owner.
3. **Demonstrate the phase success signal:** a recipient with partial known
   history and a task receives a selection, challenges a deliberate omission,
   the repair loop re-includes it, and the recipient succeeds only after repair.
   This is Option B's analogue of Option A's OA-06 demo.

Option B delivers relevance and handoff honesty on top of Option A's integrity.
It never claims to mutate, reorder, or rewrite Option A history, and it never
claims "the" relevant context — selections are task- and recipient-conditioned,
recorded, and measurable.

## 2. Option B Outcome

Deliver an embeddable Rust layer over the existing `contextmesh` crate that:

- records signed, self-contained agent experience receipts (B1);
- selects task-conditioned sources by semantic relevance via a context
  compiler, bounded by a concrete budget, with selector provenance (B2);
- closes the selected set over DAG parent edges and covers critical/risk
  events (B3);
- computes the delta against the recipient's known history (B4) and binds
  handoff validity to a recipient head, failing closed when stale (B5);
- states every omission and uncertainty explicitly and accepts challenges
  (B6);
- progressively repairs omitted context in a bounded loop, recording the
  attempt history (B7);
- proves comprehension and downstream task performance with an in-repo,
  frozen, offline evaluation suite (B8);
- produces verifiable hierarchical and project summaries (B9);
- computes demonstrably sufficient selections with any minimality claim
  backed by a recorded metric (B10);
- models recipient capability alongside knowledge so a handoff the recipient
  cannot act on is flagged, not assumed (B11).

The phase success signal is an end-to-end demonstration (OB-13) that closes the
loop: selection → delta → handoff → challenged omission → repair → task success.

## 3. Scope Boundary

### 3.1 Included

Everything in the frozen spec's gate set B1–B11: the union of plan section 3.2
(deferred items) and section 10 (the Option B unlock gate). The
feature-completeness map in the spec is the traceability contract; the
delivery-plan matrix in section 10 below is its executable form.

### 3.2 Explicitly out of scope for Option B

- Any change to Option A's frozen wire, bounds, claim discipline, or database
  schema.
- Receipts entering Option A's store (they never do; OB-01 stores them as
  exported artifacts, and OB-07's repair-history store is a distinct file).
- A second embedded database process alongside Option A's Turso file (Option
  A's one-process-per-database-file constraint governs every OB package that
  reads the DAG).
- Consensus, blockchain, or A2A/ACP compliance claims.
- Claims of sufficiency, minimality, or understanding beyond what a recorded
  metric proves.
- Private chain-of-thought storage.
- Any dependency that violates the forbidden-surface discipline (no TLS
  stacks, HTTP/2/3, QUIC, cookies, compression, DNS resolvers, shells,
  libp2p, or sqlite alternates) unless it passes the Ask-First review in
  OB-12.

## 4. Delivery Order

Packages are named OB-01…OB-11 to mirror gates B1…B11, plus OB-12 (semantic
mechanisms, completes B2's named mechanisms) and OB-13 (end-to-end
demonstration + completion evidence). OB-00 is the spec freeze, already
committed.

Dependency edges (→ means "requires before acceptance"):

- OB-01 (B1) → none (foundation).
- OB-02 (B2 core) → OB-01.
- OB-03 (B3) → OB-02.
- OB-04 (B4) → OB-02, OB-03 (delta needs closure of both sets).
- OB-05 (B5) → OB-04.
- OB-06 (B6) → OB-04, OB-05.
- OB-07 (B7) → OB-06; uses OB-08's eval signals for eval-driven convergence.
- OB-08 (B8) → OB-01, OB-02 (suite needs receipts and selection to build
  tasks); parallelizable with OB-06/OB-07.
- OB-09 (B9) → OB-01, OB-02.
- OB-10 (B10) → OB-03, OB-08, OB-02 budget.
- OB-11 (B11) → OB-05, OB-06.
- OB-12 (semantic mechanisms) → OB-02; Ask-First gated; parallelizable with
  OB-03…OB-11.
- OB-13 (demo + evidence) → all of OB-01…OB-12.

Recommended critical path: OB-01 → OB-02 → OB-03 → OB-04 → OB-05 → OB-06 →
OB-07 → OB-08 → OB-10 → OB-13. OB-09, OB-11, and OB-12 run alongside.

## 5. Work Packages

### OB-00 — Option B Spec Freeze

**Status:** done, commit `a950836` on `ESgCHandoff`.

**Purpose:** Freeze intent, boundaries, and the complete B1–B11 gate set before
any Option B product code.

**Acceptance:** Spec has `status: approved`, `approved_by: Lunarpulse`, the
`frozen-after-approval` wrapper, and the feature-completeness map proving every
plan §3.2 and §10 item has an owner.

**Evidence owner:** Lunarpulse.

---

### OB-01 — Agent Experience Receipts (B1)

**Purpose:** Define the Option B receipt artifact and its lifecycle, on top of
Option A's crypto, with no second database.

**Work:**

- New `src/receipt.rs`: receipt type — canonical JSON + Ed25519 signature
  (reuse `src/crypto.rs`'s `verify_strict` and BLAKE3 domain separation; no
  new signature primitive), content addressing, and verification against the
  DAG (every referenced event exists, is authorized, and is in the stated
  context).
- Receipt fields: referenced Option A event IDs, task/recipient-state binding,
  selector provenance (identity, version, configuration hash), omission and
  uncertainty lists (populated from B6 onward), created time, signer.
- Persistence: receipts are exported artifacts (JSON files under a documented
  output directory), self-contained and content-addressed — not a second
  embedded database, and they never enter Option A's store.
- CLI surface: `contextmesh ob-receipt issue|verify` (mirroring the OA-05 CLI
  conventions).

**Acceptance:**

- A receipt verifies against the DAG; tampering with a referenced event ID or
  the receipt body is rejected.
- A receipt referencing an unknown or unauthorized event fails verification.
- Receipt round-trips survive export → import → verify.
- No Option A store write occurs; the one-process-per-DB constraint holds.

**Tests:** `tests/ob01_receipts.rs`; golden fixture
`tests/fixtures/ob01-receipt-golden.json`; adversarial tamper matrix.

**Evidence:** `scripts/verify-ob01.sh`; evidence recorded in
`_bmad-output/verification-artifacts/ob-01-evidence.md`.

**Evidence owner:** Lunarpulse (verification), Winston (architecture review).

---

### OB-02 — Task-Conditioned Source Selection Core (B2 core)

**Purpose:** Build the selection engine and context compiler skeleton: task
intake in both accepted forms, budget enforcement, and recorded selector
provenance. This is the B2 core; the heavy semantic mechanisms (embeddings,
vector search, reranking) are OB-12.

**Work:**

- New `src/selection.rs`: selector trait, budget type (maximum selected event
  count + maximum exported byte size) enforced at handoff time, and the
  baseline deterministic selector (lexical/term-frequency matching over event
  payloads) that requires no new dependencies.
- New `src/compiler.rs`: context compiler that assembles the bounded set of
  source references from selector output.
- Task intake: free text and structured queries. The receipt records the task
  verbatim plus a content hash; a structured canonical form is recorded only
  when the caller supplies one. The system never claims to derive a
  deterministic canonical form from free text.
- Provenance: selector identity, selector version, and selector configuration
  hash are recorded in every receipt. Changing the selector version changes
  the recorded version — never the history.
- Edge cases from the spec: empty history → "no sources" marker; empty/absent
  task → fail closed; task with no matching source → empty selection plus
  uncertainty marker; selector error → fail closed, prior state intact.

**Acceptance:**

- Selection respects the budget for both count and bytes; over-budget
  selections are refused, not truncated silently.
- The receipt records selector provenance; two selector versions produce
  distinct provenance records over the same history.
- Free text and structured tasks both produce selections; receipts record the
  task verbatim and its content hash.
- Edge cases behave per the I/O matrix.

**Tests:** `tests/ob02_selection.rs`; fixture `tests/fixtures/ob02-selection-golden.json`.

**Evidence:** `scripts/verify-ob02.sh`;
`_bmad-output/verification-artifacts/ob-02-evidence.md`.

**Evidence owner:** Amelia (engineer), with Sally (UX) on task-intake shape.

---

### OB-03 — Dependency Closure and Critical-Risk Coverage (B3)

**Purpose:** Close the selected set over DAG parent edges and cover flagged
critical/risk events.

**Work:**

- New `src/closure.rs`: for a selected reference set, compute the DAG parent
  closure using Option A's existing DAG/projection machinery (read-only);
  flag any dangling parent reference; add flagged critical/risk events to the
  selection.
- Typed failure for a severed parent: reject, never silently drop.

**Acceptance:**

- Closure check reports zero dangling references on a valid selection.
- A deliberately severed parent is rejected with a typed error.
- Critical/risk-flagged events are present in the closed set.

**Tests:** `tests/ob03_closure.rs`; adversarial severance matrix.

**Evidence:** `scripts/verify-ob03.sh`;
`_bmad-output/verification-artifacts/ob-03-evidence.md`.

**Evidence owner:** Amelia (engineer).

---

### OB-04 — Recipient-Known-History Delta (B4)

**Purpose:** Given a recipient known-history head, compute the delta — selected
events outside the recipient's closure.

**Work:**

- New `src/delta.rs`: recipient-state record (known-history head + closure),
  delta computation against the closed selected set.
- Delta is provable: the recipient head must be an ancestor in the same DAG;
  no selected event may already be inside the recipient's closure.
- Cold-start recipient (empty known history) → delta equals full selection.
- Recipient head not present in the DAG → fail closed (unknown recipient
  state), never assumed.

**Acceptance:**

- Delta contains exactly the selected events outside the recipient's closure.
- A recipient head that is not an ancestor, or not present, fails closed.
- Cold-start produces the full selection as the delta.

**Tests:** `tests/ob04_delta.rs`.

**Evidence:** `scripts/verify-ob04.sh`;
`_bmad-output/verification-artifacts/ob-04-evidence.md`.

**Evidence owner:** Amelia (engineer).

---

### OB-05 — State-Bound Handoff Validity (B5)

**Purpose:** Bind a handoff to a recipient head so a stale handoff is rejected,
never applied.

**Work:**

- New `src/handoff.rs` (validity portion): handoff type embedding the
  recipient head it was computed against; validity check = recipient head is
  still the current stated head in the same DAG.
- Stale handoff → typed stale error, re-derive required; idempotent when the
  head is unchanged (B4 + B5 together make handoff state-safe).

**Acceptance:**

- A handoff computed against head H is rejected when the recipient advances to
  H′.
- Re-deriving against H′ succeeds; the original handoff is left intact.

**Tests:** `tests/ob05_validity.rs`.

**Evidence:** `scripts/verify-ob05.sh`;
`_bmad-output/verification-artifacts/ob-05-evidence.md`.

**Evidence owner:** Amelia (engineer), Winston (architecture review).

---

### OB-06 — Omission Challenge and Uncertainty (B6)

**Purpose:** Make omissions first-class, challengeable data; implement the
"handoff negotiation" entry point from plan §3.2.

**Work:**

- Extend `src/handoff.rs` (negotiation portion): every handoff carries an
  explicit omission list and uncertainty markers.
- Challenge API: a recipient can challenge a listed omission; the challenge is
  recorded, and the re-included source lands in a follow-up handoff.
- Capability-mismatch flags from B11 surface here as uncertainty (B11 wires in
  during OB-11).

**Acceptance:**

- A challenged omission is re-included in a follow-up handoff with the
  challenge recorded.
- No omission is hidden: the omission list is present on every handoff.

**Tests:** `tests/ob06_omission.rs`.

**Evidence:** `scripts/verify-ob06.sh`;
`_bmad-output/verification-artifacts/ob-06-evidence.md`.

**Evidence owner:** Amelia (engineer), Sally (UX) on challenge ergonomics.

---

### OB-07 — Progressive Context Repair (B7)

**Purpose:** Iteratively re-include omitted context and re-handoff within a
bounded repair loop, recording attempt history as evidence.

**Work:**

- New `src/repair.rs`: bounded repair loop (max iterations, max re-included
  events, max bytes), driven by a task-outcome callback. Outcome signals come
  from OB-08's eval suite (eval-driven convergence) or a scripted challenge.
- New repair-history store: a distinct file (JSON lines) that records attempt
  history. It never touches Option A's DB and is not a second embedded
  database in the store sense.
- Convergence or non-convergence is always reported; the original handoff is
  left intact on non-convergence.

**Acceptance:**

- A repair sequence converges within the bound and records attempt history.
- A non-converging sequence reports non-convergence and leaves the original
  handoff intact.
- The repair-history file is independent of Option A's DB.

**Tests:** `tests/ob07_repair.rs`; convergence and bound-exceeded matrices.

**Evidence:** `scripts/verify-ob07.sh`;
`_bmad-output/verification-artifacts/ob-07-evidence.md`.

**Evidence owner:** Amelia (engineer).

---

### OB-08 — Comprehension and Task-Performance Evaluation Suite (B8)

**Purpose:** Ship the in-repo, frozen, offline evaluation suite that makes
"comprehension" measurable rather than claimed.

**Work:**

- New `src/eval.rs` plus `tests/ob08_eval.rs`: suite runner over a curated,
  frozen task set with two sub-modes:
  - (a) challenge probes — does the recipient notice a withheld critical fact?
  - (b) task benchmarks — does the recipient complete the downstream task?
- Every task carries a known critical-context annotation, so withheld/repaired
  cases are deterministic and offline.
- External benchmarks are advisory only; they never gate acceptance.
- The suite freezes a golden manifest (task IDs, critical annotations, expected
  outcome) as `tests/fixtures/ob08-eval-manifest.json`.

**Acceptance:**

- The withheld-context case fails and the repaired case passes, proving the
  selection was load-bearing.
- The suite runs offline with `CARGO_NET_OFFLINE=true` and is deterministic on
  the structural path.

**Tests:** `tests/ob08_eval.rs`; manifest fixture.

**Evidence:** `scripts/verify-ob08.sh`;
`_bmad-output/verification-artifacts/ob-08-evidence.md`.

**Evidence owner:** Dr. Quinn (evaluation design), Amelia (engineer).

---

### OB-09 — Hierarchical and Project Summaries (B9)

**Purpose:** Produce derived, verifiable summaries at hierarchical levels
(event → ref → project) as content-addressed references over Option A history.

**Work:**

- New `src/summary.rs`: summary types per level; each summary is a
  content-addressed reference over the events it summarizes, verifiable
  against the DAG using the B1 verification machinery.
- A summary may only reference the events it summarizes; tampered or drifted
  summaries are rejected.

**Acceptance:**

- A summary verifies against the DAG and references exactly its covered events.
- Tampering with a summary or its referenced events is rejected.

**Tests:** `tests/ob09_summaries.rs`.

**Evidence:** `scripts/verify-ob09.sh`;
`_bmad-output/verification-artifacts/ob-09-evidence.md`.

**Evidence owner:** Mary (analyst), Amelia (engineer).

---

### OB-10 — Minimal-Sufficient-Context Computation (B10)

**Purpose:** Compute selections that are demonstrably sufficient and, where a
defined metric exists, minimal — never beyond what the metric shows.

**Work:**

- Extend `src/selection.rs`: sufficiency check wired to the B8 eval (task
  succeeds with the selected context under the frozen suite); minimality check
  wired to a recorded metric (selected count/bytes against budget).
- A claim of sufficiency or minimality beyond the metric is refused.

**Acceptance:**

- The sufficiency claim is backed by the B8 evaluation.
- The minimality claim is backed by the recorded metric.
- A request for a claim beyond the metric is refused.

**Tests:** `tests/ob10_sufficient.rs`.

**Evidence:** `scripts/verify-ob10.sh`;
`_bmad-output/verification-artifacts/ob-10-evidence.md`.

**Evidence owner:** Mary (analyst), Amelia (engineer).

---

### OB-11 — Recipient Capability Modeling (B11)

**Purpose:** Model what a recipient can do alongside what it knows, so an event
the recipient cannot act on is flagged, not silently handed off or dropped.

**Work:**

- New `src/capability.rs`: recorded, versioned capability model per recipient
  (declared capabilities, verifier where applicable).
- Handoff shaping: the capability model constrains the handoff; a capability
  mismatch is flagged in the omission/uncertainty list (surfaces via B6) rather
  than assumed.
- B4 remains the known-history delta; capability is additive to knowledge.

**Acceptance:**

- The capability model is recorded and versioned.
- A handoff respects the recipient's stated capabilities; a mismatch is
  flagged in the omission/uncertainty list.

**Tests:** `tests/ob11_capability.rs`.

**Evidence:** `scripts/verify-ob11.sh`;
`_bmad-output/verification-artifacts/ob-11-evidence.md`.

**Evidence owner:** Winston (architecture), Amelia (engineer).

---

### OB-12 — Semantic Mechanisms Adoption (embeddings, vector search, reranking)

**Purpose:** Resolve B2's named heavy mechanisms with a recorded decision —
adopt a compliant dependency or record a demonstrated-baseline decision.

**Work:**

- Evaluate candidate embedding/vector-search/reranking dependencies against
  the forbidden-surface audit (no TLS stacks, HTTP/2/3, QUIC, cookies,
  compression, DNS resolvers, shells, libp2p, sqlite alternates) and the
  offline, recorded-not-re-derived discipline.
- If a compliant dependency exists: integrate as a selector backend behind the
  OB-02 selector trait; offline model artifacts pinned in the repo; provenance
  records model identity + hash; determinism remains guaranteed only on the
  structural path.
- If none passes: record the non-adoption decision with the demonstrated
  baseline (lexical selector + evaluation results) in the evidence artifacts.
  This is a recorded decision, not a silent deferral.

**Acceptance:**

- Either a compliant mechanism is adopted with pinned artifacts and an audit
  record, or a non-adoption decision is recorded with baseline evidence.
- In both cases B2 remains complete under the frozen spec (selection respects
  budget, provenance recorded, version changes never history).

**Tests:** `tests/ob12_semantic.rs` (if adopted); audit artifact either way.

**Evidence:** `scripts/verify-ob12.sh`;
`_bmad-output/verification-artifacts/ob-12-evidence.md`.

**Evidence owner:** Winston (dependency gate), Amelia (engineer), Lunarpulse
(Ask-First approver).

---

### OB-13 — End-to-End Demonstration and Option B Completion Evidence

**Purpose:** Prove the phase success signal and record Option B completion
evidence, mirroring OA-06 + OA-07.

**Work:**

- Extend `scripts/demo.sh` (or add `scripts/demo-ob.sh`) with the Option B
  scenario: a recipient with a partial known history and a task receives a
  selection, challenges a deliberate omission, the repair loop re-includes it,
  and the recipient succeeds on the task only after repair.
- Record completion evidence:
  `_bmad-output/verification-artifacts/ob-completion-evidence.md` with a
  B1–B11 matrix, the demo transcript, the dependency-closure baseline (reuse
  the OA-07 baseline count; OB additions recorded separately), and the
  Always/Never consistency check.
- Add `scripts/verify-ob13.sh` as the final release gate: offline, fresh
  target, deterministic on the structural path, no runtime artifacts, no
  secret leakage.

**Acceptance:**

- The demo passes: task fails with withheld context, succeeds after repair.
- The evidence matrix has a row for every gate B1–B11 with a passing
  verifier.
- The release gate runs offline and is reproducible from documented commands.

**Tests:** `tests/ob13_demo.rs`; demo scenario fixture.

**Evidence:** `_bmad-output/verification-artifacts/ob-completion-evidence.md`,
`_bmad-output/verification-artifacts/ob-claim-audit.md`.

**Evidence owner:** Lunarpulse (completion verdict), Amelia (demo), Dr. Quinn
(eval scenario).

## 6. Completion Gates

The gates are the frozen spec's B1–B11 intent + success pairs. The executable
form of each gate is its package's acceptance tests and verifier script:

| Gate | Package | Verifier | Test file |
|------|---------|----------|-----------|
| B1 receipts | OB-01 | `verify-ob01.sh` | `tests/ob01_receipts.rs` |
| B2 selection core | OB-02 | `verify-ob02.sh` | `tests/ob02_selection.rs` |
| B2 semantic mechanisms | OB-12 | `verify-ob12.sh` | `tests/ob12_semantic.rs` |
| B3 closure/risk | OB-03 | `verify-ob03.sh` | `tests/ob03_closure.rs` |
| B4 known-history delta | OB-04 | `verify-ob04.sh` | `tests/ob04_delta.rs` |
| B5 state-bound validity | OB-05 | `verify-ob05.sh` | `tests/ob05_validity.rs` |
| B6 omission challenge | OB-06 | `verify-ob06.sh` | `tests/ob06_omission.rs` |
| B7 progressive repair | OB-07 | `verify-ob07.sh` | `tests/ob07_repair.rs` |
| B8 eval suite | OB-08 | `verify-ob08.sh` | `tests/ob08_eval.rs` |
| B9 summaries | OB-09 | `verify-ob09.sh` | `tests/ob09_summaries.rs` |
| B10 minimal-sufficient | OB-10 | `verify-ob10.sh` | `tests/ob10_sufficient.rs` |
| B11 capability modeling | OB-11 | `verify-ob11.sh` | `tests/ob11_capability.rs` |
| Phase signal + evidence | OB-13 | `verify-ob13.sh` | `tests/ob13_demo.rs` |

## 7. Test Strategy

- **Unit tests:** receipt serialization/verification, canonical JSON, budget
  arithmetic, closure, delta, validity, repair bounds, summary verification,
  capability record versioning.
- **Golden fixtures:** frozen golden files for the receipt, selection, and
  eval manifest; regenerate only by explicit re-freeze with the change logged.
- **Adversarial tests:** tampered references, severed parents, stale heads,
  over-budget selections, unknown recipient heads, capability mismatches.
- **Integration tests:** selection → closure → delta → handoff → challenge →
  repair against a real Option A store (single process; reuse the OA-02/03
  test store choreography).
- **End-to-end test:** the OB-13 demo scenario, mirroring `tests/oa06_demo.rs`.
- **Verifier chain:** `verify-obNN.sh` per package, and `verify-ob13.sh` as the
  chained release gate — offline, fresh-target, deterministic on the
  structural path, mirroring the OA-07 gate's discipline.

## 8. Key Risks and Mitigations

1. **Semantic mechanisms cannot pass the forbidden-surface audit.** Mitigation:
   OB-12 resolves this as a recorded decision with a demonstrated baseline
   (lexical selector + eval results); the feature is not dropped, the decision
   is recorded.
2. **Turso one-process-per-database-file conflicts with OB packages reading
   the DAG.** Mitigation: OB packages read Option A's store read-only through
   the existing store API in the same process; no second process or second DB
   is introduced.
3. **Cross-run determinism creep into the semantic path.** Mitigation: the
   frozen spec's recorded-not-re-derived rule; receipts, not model reruns, are
   the source of truth.
4. **B8 suite becoming a human-judgment gate.** Mitigation: frozen, offline,
   deterministic suite with known critical-context annotations; external
   benchmarks are advisory only.
5. **Scope creep into Option A surfaces.** Mitigation: the frozen wire and
   forbidden-surface discipline; any Ask-First item requires Lunarpulse
   approval and is recorded.
6. **Repair loop divergence.** Mitigation: hard bounds and mandatory
   non-convergence reporting; the original handoff is always left intact.
7. **Disk/toolchain failure modes observed in Option A.** Mitigation: reuse
   the OA-07 offline fresh-target repetition and `CARGO_INCREMENTAL=0` scoping;
   keep the locked closure baseline.

## 9. Decision and Change Control

- The frozen spec is the contract; the delivery plan sequences it. Any change
  to the spec requires human renegotiation (frozen-after-approval).
- Package-level decisions (dependency adoption, baseline-vs-adopt in OB-12,
  eval task curation) are recorded in the package evidence and approved by
  Lunarpulse where they touch Ask-First boundaries.
- All evidence artifacts live in `_bmad-output/verification-artifacts/`;
  all verifiers live in `scripts/`; all tests live in `tests/`.

## 10. Feature-Completeness Assurance Matrix

Every plan §3.2 and §10 item has a gate, a package, a test, and an evidence
owner. Nothing is deferred without a gate; nothing is dropped.

| Plan item | Gate | Package | Test | Evidence owner |
|-----------|------|---------|------|----------------|
| §10.1 agent experience receipts | B1 | OB-01 | `ob01_receipts` | Lunarpulse / Winston |
| §10.2 task-conditioned source selection | B2 | OB-02 | `ob02_selection` | Amelia / Sally |
| §3.2 semantic/critical context selection | B2 | OB-02, OB-12 | `ob02_selection`, `ob12_semantic` | Amelia / Winston |
| §3.2 embeddings, vector search, reranking, context compilers | B2 mechanisms | OB-12, OB-02 | `ob12_semantic`, `ob02_selection` | Winston / Lunarpulse |
| §10.3 dependency closure + critical-risk coverage | B3 | OB-03 | `ob03_closure` | Amelia |
| §10.4 recipient-known-history delta | B4 | OB-04 | `ob04_delta` | Amelia |
| §3.2 recipient knowledge modeling | B4 | OB-04 | `ob04_delta` | Amelia |
| §10.5 state-bound handoff validity | B5 | OB-05 | `ob05_validity` | Amelia / Winston |
| §10.6 omission challenge + uncertainty | B6 | OB-06 | `ob06_omission` | Amelia / Sally |
| §3.2 context handoff negotiation | B6 | OB-06 | `ob06_omission` | Amelia / Sally |
| §10.7 progressive context repair | B7 | OB-07 | `ob07_repair` | Amelia |
| §3.2 progressive source-grounded context repair | B7 | OB-07 | `ob07_repair` | Amelia |
| §10.8 comprehension + task-performance evaluation | B8 | OB-08 | `ob08_eval` | Dr. Quinn / Amelia |
| §3.2 comprehension verification | B8 | OB-08 | `ob08_eval` | Dr. Quinn / Amelia |
| §3.2 hierarchical or project summaries | B9 | OB-09 | `ob09_summaries` | Mary / Amelia |
| §3.2 minimal-sufficient-context computation | B10 | OB-10 | `ob10_sufficient` | Mary / Amelia |
| §3.2 recipient capability modeling | B11 | OB-11 | `ob11_capability` | Winston / Amelia |
| §3.2 claims that ancestry is relevant/sufficient | Boundaries | spec | claim audit (OB-13) | Lunarpulse |

## 11. Immediate Next Actions

1. Review and approve this delivery plan (status → `approved-for-execution`).
2. Start OB-01: define the receipt artifact and `verify-ob01.sh`; freeze the
   golden receipt fixture before OB-02 parallelizes.
3. Execute OB-02 before any OB-03+ work; OB-12's dependency evaluation can
   start in parallel.
4. Freeze the OB-08 eval manifest early so OB-07 and OB-10 can depend on it.
5. Maintain progress against the package sequence and the B1–B11 gates; record
   evidence per package before declaring the package complete.
6. Close with OB-13: end-to-end demo and the Option B completion evidence,
   gated by `verify-ob13.sh`.
