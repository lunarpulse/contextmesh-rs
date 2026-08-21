# OC-0.5 Post-Rebase Repository Integration Audit

date: 2026-08-21
branch: `OC-AttentionLedger`
audited-head: `7394c9b57d56ce0d557abf5924240cc09a08fcc2`
main-at-audit: `f338536` (`Merge pull request #1 from lunarpulse/ESgCHandoff`)
status: repository and contract audit; no C1–C5 completion claim

## 1. Executive finding

The branch topology is correct after the repository change:

```text
origin/main f338536  (Option B merged)
  └─ 9618c24         Specify Option C salience provenance layer
      └─ 7394c9b     Record prototype validation evidence
```

Executed checks:

```bash
git fetch origin '+refs/heads/*:refs/remotes/origin/*' --prune
git merge-base --is-ancestor origin/main HEAD
git rev-list --left-right --count origin/main...HEAD
git diff --stat origin/main...HEAD
```

Observed:

- `origin/main` is an ancestor of `HEAD` (`merge-base --is-ancestor` exit 0).
- Branch distance is `0 behind / 2 ahead`.
- The two Option C commits add 17 files and 2,000 lines; they do not modify production `src/`.
- Option B is now present in `main` through OB-13, while `origin/ESgCHandoff` remains as a remote historical branch.

The rebase therefore solved the Git ancestry problem, but it exposed integration-contract gaps that were invisible when Option C was designed beside an unmerged Option B branch.

## 2. Sources and reliability

| Source | Accessed | Reliability | Use |
|---|---|---:|---|
| local Git graph and refs | 2026-08-21 | Primary | ancestry and diff claims |
| `graphify-out/GRAPH_REPORT.md` built at HEAD | 2026-08-21 | Primary structural extraction; inferred edges require review | architectural map |
| Option B frozen spec | 2026-08-21 | Primary human-owned intent | B1–B11 contract |
| Option C draft spec | 2026-08-21 | Primary draft intent | C1–C5 proposal |
| production Rust source and tests | 2026-08-21 | Primary implementation | exact API seams |
| OC prototype and recorded evidence | 2026-08-21 | Primary prototype evidence | E1–E3 mechanism behavior |

No web source was needed. This audit is about the local repository state and its recorded contracts.

## 3. Graph-level architecture after Option B merge

Graphify was missing when the audit began, so the zero-API AST path was executed:

```bash
graphify update .
```

Observed graph after the replay harness was added and the final AST-only update was run:

- 114 code-visible files
- 1,813 nodes, 3,261 edges, 98 communities
- committed base remains `7394c9b5`; the refreshed worktree graph also includes the uncommitted OC-0.5 replay harness
- central Option B abstractions: `compute_delta`, `close_selection`, `select_sources`, `Handoff`
- Option C remains a separate specification/prototype/replay community rather than production `src/` code

The correct integration shape is not “Option C replaces Option B.” It is a derived feedback layer around Option B:

```text
past outcomes
  → OutcomeLedger / attribution / thorn / prior          (C1–C3)
  → source scoring and recorded influence                (C4)
  → Option B closure → delta → handoff → repair/eval     (B3–B8)
  → new terminal outcome                                 (C1 feedback)
```

Option A continues to own immutable signed history. Option B continues to own bounded, state-safe handoff. Option C should own why an event mattered and how that evidence changes later ranking.

## 4. Verified reusable seams

### 4.1 DAG-reference verification

Option C can follow the Option B receipt discipline:

- `Store::event` reloads and verifies stored event wire: `src/store.rs:383-395`.
- Event context is available in the signed body: `src/model.rs:197-224`.
- Receipt DAG verification checks every reference and context: `src/receipt.rs:542-607`.
- Option C requires existence, authorization, same-context verification and fail-closed rejection: `spec-option-c-salience-provenance-layer.md:74-95,99-109`.

Recommendation: create a shared semantic pattern, but do not change Option A wire. The OC crate should verify `EventId` references against the existing `Store` and emit typed missing/wrong-context/signature/bounds errors.

### 4.2 Signing and content addressing

Reusable primitives exist:

- caller-domain signing: `src/crypto.rs:118-130`
- strict domain verification: `src/crypto.rs:133-155`
- receipt issue order (validate → ID → sign → verify): `src/receipt.rs:457-476`
- content-address derivation: `src/receipt.rs:631-637`

OC must use distinct domains for Outcome Ledger, Thorn Index, Salience Prior, and selection influence artifacts. It must not extend the strict `ReceiptBodyV1` wire, whose accepted fields are fixed at `src/receipt.rs:639-665`.

### 4.3 Task and selector provenance

Useful public concepts already exist:

- `TaskRecordV1`: verbatim task, content hash, optional caller-supplied structure (`src/receipt.rs:146-210`)
- `SelectorRecordV1`: identity, version, configuration hash (`src/receipt.rs:224-279`)
- receipt context/event/task/recipient/selector/omission/uncertainty shape (`src/receipt.rs:301-379`)

OC should reuse these vocabulary and provenance semantics, not put C fields into B receipts.

### 4.4 Selection-to-handoff path

Production flow is already present:

1. `select_sources`: `src/selection.rs:433-503`
2. `close_selection`: `src/closure.rs:340-428`
3. `compute_delta`: `src/delta.rs:378-436`
4. `Handoff::from_delta`: `src/handoff.rs:220-241`
5. `Handoff::with_uncertainty`: `src/handoff.rs:367-386`
6. stale-state verification: `src/handoff.rs:273-317`
7. assembled evaluation path: `src/eval.rs:380-422`

C4 should produce selected references plus explicit influence/uncertainty, then enter this path. It must not bypass B3 closure, B4 delta, B5 state validity, or B6 uncertainty.

### 4.5 Summary as an entry altitude

`Summary` exposes a context and covered EventId set and verifies drift/tampering (`src/summary.rs:160-189,249-315,352-380`). It can be an optional coarse candidate source for an assembler. It is not a signed OC artifact substrate because the current summary structure is content-addressed but has no signature field (`src/summary.rs:192-203`).

## 5. Blocking incompatibilities found after rebase

### 5.1 External selector cannot construct source references

`SourceReference` fields are private and its constructor is crate-private (`src/selection.rs:191-210`). A separate `contextmesh-salience` crate cannot turn a prior-selected `SourceEvent` into a reference.

Required additive bridge, subject to founder approval:

```rust
impl SourceEvent {
    pub fn reference(&self) -> SourceReference;
}
```

or an equivalent verified constructor. This changes API surface, not Option A wire.

### 5.2 Existing selector result is too narrow

`Selector::select` returns only `Vec<SourceReference>` (`src/selection.rs:243-265`). C4 must also record lexical/prior/thorn components, alpha/beta, prior and thorn IDs, provenance, and warnings. Do not break B2’s trait. Define a richer OC result and feed only its references into Option B.

### 5.3 The C4 formula cannot solve strict vocabulary mismatch

Draft formula (`spec-option-c...:135-143`):

```text
score = tf × (1 + α·prior) × (1 − β·thorn)
```

If `tf = 0`, final score remains zero regardless of the prior. Production B baseline also removes zero-score sources (`src/selection.rs:347-351`). Therefore the current formula cannot “carry selection” under strict no-overlap.

Decision required before freeze:

- additive fusion, or
- epsilon-smoothed multiplicative fusion, or
- deterministic union of lexical and prior candidates followed by reranking.

The real-data replay retains the draft multiplicative formula as the contract arm and reports an additive arm only as exploratory evidence.

### 5.4 C2 prose and prototype execute different M3 policies

Draft C2 says M3 and M4 run only on the M0–M2 shortlist (`spec-option-c...:111-118`). The Rust prototype runs M3 leave-one-out over all non-answer candidates, then uses M0 ∪ M1n ∪ M3 for M4 (`oc-prototype-validation/src/e1.rs:43-79`). The recorded C2-direction metrics therefore do not measure the exact pipeline described by the draft.

Before approval, choose one:

1. allow M3 to nominate and keep only M4 shortlist-bound, or
2. rerun M3 strictly on M0–M2 candidates and replace the frozen evidence.

### 5.5 M2 is specified but not validated

M2 citation analysis exists in C2 but is absent from E1 and the recorded prototype evidence (`oc-00-prototype-validation.md:15-17`). OC-00 is correctly labeled a C5 baseline, not C2 completion. M2 needs implementation and its own test data.

### 5.6 Synthetic fields do not exist in OA events

The prototype has direct `dur_ms`, `content`, and `ents`; OA events have context, parents, kind, author, and JSON payload. Production must define:

- explicit terminal EventId input rather than heuristic terminal discovery
- `AvailableMs` versus `Unavailable` clock values
- versioned entity extraction with mechanism/config provenance
- caller-recorded tool/token/retry values where available

It must never infer missing elapsed time.

### 5.7 Float tolerance conflicts with content addressing

The prototype accepts a declared E2 float tolerance for cross-language comparison. A content-addressed `SaliencePriorV1` cannot: a tiny float variation changes canonical bytes and ID.

Recommendation: store fixed-point integer scores (for example `score_ppb`) and require exact byte identity in production artifacts. Keep float tolerance only for the research prototype gate.

### 5.8 Dependency direction must preserve OB-12 scope

OB-12 non-adoption remains valid for the core 320-package closure (`ob-12-semantic-mechanisms-audit.md:7-20,49-62`). Option C’s founder charter opens a separate budget (`spec-option-c...:177-186`). The only safe direction is:

```text
contextmesh-salience → contextmesh
contextmesh -X→ contextmesh-salience
```

Heavy model/judge adapters should remain optional or in separate adapter crates.

## 6. Reasoning process and alternatives rejected

### Alternative A — extend Option B receipt wire

Rejected because the parser has a strict frozen field set; adding cost, attempt, attribution, and thorn fields would break the B1 artifact contract and blur ownership.

### Alternative B — run embedding or judge calls inside `Selector::select`

Rejected because Option B’s structural selector contract expects reproducible selection, while Option C correctly says model inference is recorded and not re-derived. Model work belongs in artifact-generation sidecars; deployed C4 should consume recorded priors.

### Alternative C — promote the prototype directory directly to production

Rejected because it is a standalone executable with a synthetic event model and no dependency on production `contextmesh`. Preserve it as a fixture/evidence generator and create a real library crate after OC-0.5 freeze.

### Alternative D — claim C4 from synthetic E2

Rejected because E2’s absolute precision remained low, the formula cannot rescue TF=0 candidates, and the synthetic entity graph is not the Hermes/OA event graph.

## 7. Conclusion derivation matrix

| Question | Evidence | Conclusion |
|---|---|---|
| Was rebase successful? | ancestor exit 0; 0/2 branch distance | Yes |
| Is Option B now the integration base? | merge commit in main; B modules/tests visible | Yes |
| Can OC reuse OA/OB integrity paths? | Store, receipt verifier, crypto APIs | Yes |
| Can an external OC selector compile today? | private SourceReference constructor | No |
| Does current C4 solve zero lexical overlap? | multiplicative formula and zero-score removal | No |
| Does prototype fully validate C2? | policy mismatch and missing M2 | No |
| Is real replay needed before freeze? | synthetic schema/judge and low E2 absolute values | Yes |

## 8. Recommended sequence

### OC-0.5 — integration freeze

Freeze before implementation:

1. founder disposition of the draft
2. separate crate/workspace direction
3. artifact envelope/domains/errors/bounds
4. public SourceEvent→SourceReference bridge
5. richer C selection result
6. fusion policy for TF=0
7. M3 shortlist rule
8. M2 contract
9. terminal/cost/entity semantics
10. fixed-point prior encoding
11. real-replay acceptance thresholds

### OC-01 — Outcome Ledger (C1)

Implement signed artifact, explicit terminal outcome, unavailable clock, DAG/context verifier, and tamper/cross-context tests.

### OC-02 — Attribution (C2)

Implement deterministic M0/M1, missing M2, recorded M3/M4 adapters, shortlist policy, and judge-unavailable behavior.

### OC-03 — Thorn/Prior (C3)

Implement signed lineage, bounded entity graph, expiry/world-state semantics, fixed-point PPR, all-failure warning behavior, and recipient/context conditioning.

### OC-04 — Selection/Assembler (C4)

Implement fusion, influence record, Option B closure/delta/handoff integration, warning propagation, budget assembly, and expand operation.

### OC-05 — release gate (C5)

Require artifact vectors, exact fixed-point PPR, corrected attribution pipeline, OC→OB end-to-end integration, core dependency closure preservation, and real-replay evidence.

## 9. What would invalidate this conclusion?

- A new public source-reference bridge already exists outside the audited HEAD.
- Founder intentionally approves a breaking Option B receipt/selector wire change.
- The intended C4 semantics explicitly exclude strict TF=0 candidates.
- C2 prose is changed to match full-candidate M3 and that behavior is approved.
- Option C is intentionally kept as a research prototype rather than a production layer.

Until one of those changes is recorded, OC-0.5 integration freeze is the required next engineering gate.
