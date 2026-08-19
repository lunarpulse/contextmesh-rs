# OB-13 Option B Completion Evidence

candidate-commit: 6d0484d (OB-12 semantic decision commit; the parent of the OB-13 evidence commit)
procedure-tree: 935ec959c6abd0a603377a321ea185e45703054e (tree of the candidate commit; the OB-13 commit adds the demo binary, the demo script, the final release gate, and this evidence)
gate: scripts/verify-ob13.sh (deterministic, non-recording, offline)
verdict: complete
option-b-gate: complete (OB-01 through OB-13 all gated green; every gate B1–B11 has a passing verifier)

## Phase success signal

The Option B phase success signal is demonstrated end-to-end: a recipient with
partial known history and a task receives a handoff that deliberately omits a
critical fact, the task fails, the recipient challenges the omission, the
repair loop re-includes it, and the recipient succeeds only after repair.

```
OB-13 option-b demo
task: probe-security-constraint
stage 1/5: build the dag with a partial-history recipient
  genesis: evt1_rWZ_GhJ5I_zCZs5lW12anjEooe_g9VTScu3G2wFgzwg
  critical fact: evt1_O0R-FQZu1VTZoMa84nVqu0mdPZgJxK5Rbr5H8PE_c-A
stage 2/5: select and hand off with a deliberate omission
  handoff events: 2
  omission: evt1_O0R-FQZu1VTZoMa84nVqu0mdPZgJxK5Rbr5H8PE_c-A (deliberate)
stage 3/5: recipient benchmark with withheld context
  completed: false
stage 4/5: recipient challenges the omission; repair re-includes it
  repair converged: true
  repair iterations: 2
  re-included: 1
stage 5/5: recipient benchmark after repair
  completed: true
  final handoff wire bytes: 1299
phase success signal: PASS
```

The transcript is deterministic on the structural path (fixture seeds, frozen
eval manifest, no network, no wall clock) and contains only public `evt1_`
identifiers and stage markers — no key, token, or seed material (asserted by
`tests/ob13_demo.rs`).

## B1–B11 completion matrix

| Gate | Package | Verifier | Verdict |
|------|---------|----------|---------|
| B1 agent experience receipts | OB-01 | scripts/verify-ob01.sh | pass (all checkpoints green) |
| B2 task-conditioned source selection core | OB-02 | scripts/verify-ob02.sh | pass |
| B2 semantic mechanisms (embeddings/vector search/reranking) | OB-12 | scripts/verify-ob12.sh | pass (non-adoption recorded) |
| B3 dependency closure + critical-risk coverage | OB-03 | scripts/verify-ob03.sh | pass |
| B4 recipient-known-history delta | OB-04 | scripts/verify-ob04.sh | pass |
| B5 state-bound handoff validity | OB-05 | scripts/verify-ob05.sh | pass |
| B6 omission challenge + uncertainty | OB-06 | scripts/verify-ob06.sh | pass |
| B7 progressive context repair | OB-07 | scripts/verify-ob07.sh | pass |
| B8 comprehension + task-performance evaluation | OB-08 | scripts/verify-ob08.sh | pass |
| B9 hierarchical and project summaries | OB-09 | scripts/verify-ob09.sh | pass |
| B10 minimal-sufficient-context computation | OB-10 | scripts/verify-ob10.sh | pass |
| B11 recipient capability modeling | OB-11 | scripts/verify-ob11.sh | pass |
| B13 phase success signal (completion) | OB-13 | scripts/verify-ob13.sh | pass (this gate) |

Every gate B1–B11 has a passing verifier; OB-12 completes B2's named semantic
mechanisms with a recorded non-adoption decision; OB-13 demonstrates the phase
success signal and records this completion evidence.

## Dependency-closure baseline

- dependency-closure: 320
  (unchanged from the OA-07 baseline; no OB package added a dependency).
- Cargo.lock sha256: e194c2923e697c657e1d1019dbb00718315f529c89df3e3a1374f061fe6609ef
- Locked feature graph: byte-identical to cargo-tree-oa05-features.txt
  (re-asserted by every OB gate and by the final release gate).
- OB additions: no direct dependency, no closure change, no wall-clock
  dependency — the entire Option B layer is additive library code over the
  frozen Option A surface.

## Always / Never consistency check

Every OB package is checked against the frozen spec's Always/Ask First/Never
lists; each gate's Step 3 re-asserts the supply-chain half, and the tests
re-assert the behavioral half.

- **Always** — consume Option A events only as verifiable evidence: receipts
  (OB-01) verify against the DAG; an unverifiable reference is an error, never
  sanitized (OB-01/OB-02 tests). No new signature primitive (Ed25519 + BLAKE3
  reused). Frozen wire/bounds/claim discipline preserved (every gate's
  additive-only diff check). Selector identity/version/config-hash recorded in
  every receipt (OB-01/OB-02). Selection recorded, not re-derived; determinism
  scoped to the structural path (OB-08/OB-10 tests). Bounded selection
  enforced at handoff time (OB-02 budget). Every omission explicit (OB-06);
  handoff validity bound to the recipient head and fail-closed when stale
  (OB-05). One-process-per-database-file respected (read-only store access).
  Comprehension measured by downstream task performance plus challenge probes,
  never self-report (OB-08).
- **Ask First** — no change to Option A wire or bounds (every gate diff
  check). No selector dependency adopted (OB-12 non-adoption recorded). No
  claim of sufficiency/minimality beyond a recorded metric (OB-10 claim
  discipline). No private chain-of-thought stored (no such storage exists).
- **Never** — Option A history is never mutated/reordered/rewritten (additive
  discipline, read-only store access). Receipts never enter Option A's store
  (OB-01; repair-history is a distinct file per OB-07). No claim that a
  selection is "the" relevant context (task- and recipient-conditioned
  language throughout). No claim of minimal-sufficient context or
  comprehension without a metric (OB-08/OB-10). No consensus/blockchain/A2A/ACP
  compliance claims (absent). No selection whose references cannot be verified
  against the DAG (OB-01/OB-02 fail-closed).

## Evidence owners

- Amelia (demo) — demo scenario verdict for OB-13.
- Dr. Quinn (eval scenario) — eval-driven scenario verdict for OB-13.
- Lunarpulse (completion verdict) — final approval.
