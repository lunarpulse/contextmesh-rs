# OB-08 Option B Comprehension and Task-Performance Evaluation Evidence (gate B8)

candidate-commit: 1c937b7 (OB-07 repair commit; the parent of the OB-08 evidence commit)
procedure-tree: d8d08eefef91ba44f1a79ee952fff6a31f3be63e (tree of the candidate commit; the OB-08 commit adds the eval module, the repair matrix, the verifier, and this evidence)
gate: scripts/verify-ob08.sh (deterministic, non-recording, offline)
verdict: pass (all checkpoints green)
option-b-gate: unblocked-by-complete-verdict (OB-01 through OB-07 complete; OB-08 is the B8 comprehension and task-performance evaluation package of the Option B delivery plan)

## Scope of this evidence

OB-08 implements gate B8 (comprehension and downstream task-performance
evaluation) from the frozen spec
`spec-option-b-source-grounded-context-handoff.md` and package OB-08 from
`option-b-delivery-plan.md`. It is purely additive over Option A and
OB-01..OB-07:

- new `src/eval.rs` (the OB-08 work module): the suite runner over the
  curated, frozen task set, with the two sub-modes (challenge probes and task
  benchmarks), the deterministic task-chain builder, the simulated recipient
  (`simulate`), and the manifest validation;
- additive doc note and `pub mod eval;` registration in `src/lib.rs` only;
- additive `#[allow(dead_code)]` allowances on the shared test helpers in
  `tests/common/mod.rs` (each integration binary uses a subset of the
  helpers; no helper logic changed);
- the frozen golden manifest `tests/fixtures/ob08-eval-manifest.json`
  (task IDs, critical-context annotations, expected outcomes);
- no change to `src/handoff.rs` (OB-06), `src/repair.rs` (OB-07),
  `src/delta.rs` (OB-04), `src/receipt.rs` (OB-01), `src/selection.rs` /
  `src/compiler.rs` (OB-02), `src/closure.rs` (OB-03), or any Option A module
  (verified by the gate's additive-only diff check), no new dependency, no
  CLI change.

## Environment and toolchain

| Item | Value |
|---|---|
| rustc | 1.97.0 (2d8144b78 2026-07-07) |
| cargo | 1.97.0 (c980f4866 2026-06-30) |
| toolchain source | rust-toolchain.toml override (1.97.0-x86_64-unknown-linux-gnu) |
| native prerequisite | cc present and usable (Turso bundled sqlite3.c build) |
| env overrides | none (gate runs with CARGO_NET_OFFLINE=true; no RUSTC/RUSTFLAGS/CARGO_BUILD_*/CARGO_TARGET_DIR) |
| worktree | clean at gate start and rerun |

## Supply-chain audit

- Direct dependencies (normal): unchanged from the OA baseline — turso =0.7.2
  (no features), tokio =1.53.1 (io-util, net, process, rt, signal, sync,
  time), clap =4.6.6 (derive, error-context, help, std, usage), axum =0.8.9
  (http1, json, tokio), reqwest =0.13.4 (json), blake3 =1.8.6 (std), serde,
  serde_json, serde_jcs =0.2.0, ed25519-dalek =3.0.0, base64, getrandom,
  zeroize. Dev: tokio =1.53.1 (macros, net, rt, sync, time).
- dependency-closure: 320 (unchanged; OB-08 adds no dependency)
- Cargo.lock sha256: e194c2923e697c657e1d1019dbb00718315f529c89df3e3a1374f061fe6609ef
- Locked feature graph: byte-identical to cargo-tree-oa05-features.txt
  (re-asserted by the gate).
- Forbidden-capability audit: no TLS stacks, HTTP/2/3, QUIC, cookies,
  compression, DNS resolvers, shells, libp2p/rusqlite/sqlite alternates in the
  closure; no wall-clock dependency added. The eval suite is self-contained
  in eval.rs and reuses the selection/closure/delta/handoff layers read-only.

## Design notes

**Frozen, offline, in-repo evaluation.** `tests/fixtures/ob08-eval-manifest.json`
freezes the curated task set — four tasks across the two sub-modes, each with
its task text, deterministic chain shape, and a known critical-context
annotation (the annotated step's kind and note). The manifest is validated
fail-closed (schema/version pins, unique ids, `critical_note` must equal the
annotated chain step) and its sha256 is pinned by the gate. The suite runs
offline with `CARGO_NET_OFFLINE=true` and is deterministic on the structural
path: fixed author seed, fixed per-task contexts, fixed chains, no network,
no clock, no randomness.

**Two sub-modes, one load-bearing proof.** Challenge probes ask whether the
simulated recipient notices a withheld critical fact; task benchmarks ask
whether the recipient completes the downstream task. Every task runs in both
configurations: the withheld case excludes the critical event from the
candidate set (so it can never be selected), the repaired case includes it.
Each task's frozen expectation is `withheld: fails, repaired: passes`.

**Deterministic chains that keep the critical event withholdable.** Every
chain child is a sibling of the genesis (parents = `[genesis]`), so the
critical event is never an ancestor of another child and cannot leak into the
dependency closure of a withheld case. The closure policy kind
(`eval.no-critical-kind`) never appears in the chains, so the closure adds
nothing and the handoff content is pure selection — the load-bearing proof is
about the selection, not a policy.

**The simulated recipient is deterministic and never self-reports.** The
recipient needs the task's critical events, checks the handoff's carried
events, and treats a missing critical event as noticed only when it is
explicitly listed as an omission (`simulate`). A missing fact that is not
listed is a hidden omission and fails the case — no omission is hidden (B6
composes into B8). A case passes its frozen expectation exactly: a failing
case is not completed, notices at least one omission, and hides nothing; a
passing case is completed and notices nothing.

**B7 composes into B8.** The eval's challenge signal (the noticed withheld
critical fact) drives OB-07's repair loop through `TaskOutcome::NeedsSource`
with the eval's repaired closed selection; the loop converges and the
converged handoff passes the eval benchmark — comprehension operationalized
as "task succeeds with the selected context and fails-then-recovers when
critical context is withheld."

## B8 success evidence

- The frozen manifest fixture is valid, four tasks with the load-bearing
  expected pattern, unique ids, and consistent critical annotations
  (`manifest_fixture_is_frozen_and_valid`).
- The full suite passes: every withheld case fails and every repaired case
  passes, proving each selection was load-bearing
  (`withheld_context_case_fails_and_repaired_case_passes`).
- A challenge probe notices a withheld critical fact: the recipient sees the
  explicit deliberate omission, nothing is hidden, and the task is not
  completed (`challenge_probe_notices_a_withheld_critical_fact`).
- A task benchmark completes when the critical fact is included, and the
  repaired handoff still verifies against the DAG (B5)
  (`task_benchmark_completes_when_the_critical_fact_is_included`).
- Identical inputs produce a byte-identical canonical suite report across
  separate runs (`eval_suite_is_deterministic_on_the_structural_path`).
- The eval's challenge signal drives the B7 repair loop to convergence, and
  the converged handoff passes the eval benchmark
  (`eval_signal_drives_repair_convergence`).
- Every frozen task exhibits the pattern in both sub-modes, and the
  sub-mode classification matches the manifest
  (`challenge_probe_and_benchmark_cover_every_frozen_task`).
- The manifest rejects malformed task sets fail-closed (wrong schema,
  duplicate ids, out-of-range critical index, inconsistent critical note,
  empty task set) (`eval_manifest_rejects_malformed_tasks_fail_closed`).
- The `passes` verdict is exact in both directions, and a manufactured hidden
  omission fails (`case_result_passes_checks_both_directions`).

## Additive changes (all additions, no deletions in existing files)

| File | Addition | Why |
|---|---|---|
| src/eval.rs (new) | `EvalManifest` (load/validate), `EvalTask`, `EvalMode`, `CaseExpectation`/`ExpectedOutcome`, `TaskChain`, `CaseResult`, `CaseHandoff`, `EvalResult`/`EvalReport`, `EvalError`, `build_chain`, `build_case`, `simulate`, `run_eval_suite`, `eval_context` | the OB-08 work module: the frozen, offline comprehension and task-performance evaluation suite |
| src/lib.rs | doc note for OB-08 and `pub mod eval;` | record the gate in the crate docs (additive registration only) |
| tests/common/mod.rs | `#[allow(dead_code)]` on the shared helper fns that not every integration binary uses | each integration binary compiles the shared helpers; the allowances match the existing `genesis`/`child` pattern |
| tests/fixtures/ob08-eval-manifest.json (new) | frozen golden manifest (task IDs, critical annotations, expected outcomes) | the frozen eval task set the suite runs |

The gate asserts zero deleted lines in lib.rs and tests/common/mod.rs, zero
changes to delta.rs, receipt.rs, selection.rs, compiler.rs, closure.rs,
handoff.rs, repair.rs, crypto.rs, cli.rs, model/store/error/sync/provider/http
modules, and that src/eval.rs + src/lib.rs are the only source modules changed.

## Acceptance per delivery plan

- The withheld-context case fails and the repaired case passes, proving the
  selection was load-bearing: yes (every frozen task shows the withheld case
  failing with the critical fact noticed and nothing hidden, and the repaired
  case passing).
- The suite runs offline with `CARGO_NET_OFFLINE=true` and is deterministic
  on the structural path: yes (no network, clock, or randomness; byte-identical
  canonical reports across separate runs; the gate runs fully offline).

## Regression

- OB-07 repair matrix green (`cargo test --test ob07_repair`).
- OB-06 omission matrix green (`cargo test --test ob06_omission`).
- OB-05 validity matrix green (`cargo test --test ob05_validity`).
- OB-04 delta matrix green (`cargo test --test ob04_delta`).
- OB-03 closure matrix green (`cargo test --test ob03_closure`).
- OB-02 selection matrix green (`cargo test --test ob02_selection`).
- OB-01 receipt matrix green (`cargo test --test ob01_receipts`).
- OA-01 through OA-05 verifier chain green (verify-oa01.sh, verify-oa02.sh,
  verify-oa03.sh, verify-oa04.sh, verify-oa04-dependencies.sh,
  verify-oa05.sh).
- Full workspace test suite green (all OA suites pass).
- OB-08 manifest, OB-02/OB-01/OA-01/OA-03/OA-04 golden fixtures
  byte-identical (sha256 asserted).

## Evidence owners

- Dr. Quinn (evaluation design) — evaluation design verdict for gate B8.
- Amelia (engineer) — completion verdict for gate B8.
- Lunarpulse — final approval.
