# OB-10 Option B Minimal-Sufficient-Context Computation Evidence (gate B10)

candidate-commit: 01369b3 (OB-08 eval commit; the parent of the OB-10 evidence commit)
procedure-tree: a69f3175036c0688416edee8c0e4bb5427a3dda6 (tree of the candidate commit; the OB-10 commit extends the selection module with the claim discipline, adds the sufficiency matrix, the verifier, and this evidence)
gate: scripts/verify-ob10.sh (deterministic, non-recording, offline)
verdict: pass (all checkpoints green)
option-b-gate: unblocked-by-complete-verdict (OB-01 through OB-08 complete; OB-10 is the B10 minimal-sufficient-context computation package of the Option B delivery plan)

## Scope of this evidence

OB-10 implements gate B10 (minimal-sufficient-context computation) from the
frozen spec `spec-option-b-source-grounded-context-handoff.md` and package
OB-10 from `option-b-delivery-plan.md`. It is purely additive over Option A
and OB-01..OB-08:

- `src/selection.rs` extended (the OB-10 work module) with the
  sufficiency/minimality claim discipline: `SelectionMetric`, `ClaimBasis`,
  `SufficiencyClaim`, `MinimalityClaim`, `ClaimRequest`/`ClaimRefusal`,
  `ClaimError`, `check_sufficiency` (wired to the frozen B8 evaluation), and
  `check_minimality` (backed by the recorded metric);
- additive doc note in `src/lib.rs` only;
- no change to `src/eval.rs` (OB-08), `src/repair.rs` (OB-07),
  `src/handoff.rs` (OB-06), `src/delta.rs` (OB-04), `src/closure.rs` (OB-03),
  `src/compiler.rs` (OB-02 companion), `src/receipt.rs` (OB-01), or any
  Option A module (verified by the gate's additive-only diff check), no new
  dependency, no CLI change. The claim checks reuse the existing read-only
  closure/delta/handoff/eval APIs.

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
- dependency-closure: 320 (unchanged; OB-10 adds no dependency)
- Cargo.lock sha256: e194c2923e697c657e1d1019dbb00718315f529c89df3e3a1374f061fe6609ef
- Locked feature graph: byte-identical to cargo-tree-oa05-features.txt
  (re-asserted by the gate).
- Forbidden-capability audit: no TLS stacks, HTTP/2/3, QUIC, cookies,
  compression, DNS resolvers, shells, libp2p/rusqlite/sqlite alternates in the
  closure; no wall-clock dependency added. The claim checks are self-contained
  in selection.rs and reuse the read-only closure/delta/handoff/eval layers.

## Design notes

**Sufficiency is B8-backed.** `check_sufficiency` builds the handoff a closed
selection delivers (closure → delta → handoff) and runs the frozen eval's
simulated recipient against the task's critical events. The claim is
sufficient only when the recipient completes the task and nothing is hidden;
the claim carries `ClaimBasis::B8Evaluation`, so the backing is auditable from
the claim alone. A claim of sufficiency without the B8 evaluation is refused.

**Minimality is metric-backed.** `check_minimality` records
`SelectionMetric` (selected count and exported bytes against the selection
budget) and checks removal-minimality: every selected source must be
load-bearing, i.e., removing any one of them makes the selection insufficient
under the B8 evaluation. The claim carries the metric and
`ClaimBasis::Metric`. The metric proves removal-minimality only — a request
for global minimality across the candidate set is refused, and a claim of
minimality without the recorded metric is refused.

**Claim refusal.** `ClaimRefusal::refuse` is the typed refusal gate:
sufficiency without the B8 evaluation, minimality without the recorded
metric, and global minimality (never backed by the removal metric) are always
refused with an auditable reason and, when offered, the metric.

## B10 success evidence

- The repaired selection is claimed sufficient with the B8 evaluation basis,
  and the withheld selection is claimed not sufficient
  (`sufficiency_claim_is_backed_by_the_b8_evaluation`).
- The sufficiency check works on arbitrary hand-built closed selections, not
  only the eval's own selections
  (`sufficiency_check_works_on_arbitrary_closed_selections`).
- The recorded metric backs the minimality claim in both directions: the
  repaired selection is sufficient but not removal-minimal (its non-critical
  children are redundant), while the critical-only selection is sufficient
  and removal-minimal (`minimality_claim_is_backed_by_the_recorded_metric`).
- The metric records selected count and exported bytes against the budget,
  including the within-budget verdict
  (`selection_metric_records_count_and_bytes_against_budget`).
- Claims beyond the metric are refused: sufficiency without the B8
  evaluation, minimality without the metric, and global minimality even with
  a metric (`claims_beyond_the_metric_are_refused`).
- Identical inputs produce identical sufficiency and minimality claims, and
  the claims carry their basis on the wire
  (`sufficiency_check_is_deterministic_on_the_structural_path`).
- The pattern holds across every frozen eval task: each task's full repaired
  selection is sufficient and its critical-only selection is removal-minimal
  (`minimality_holds_across_every_frozen_task`).

## Additive changes (all additions, no deletions in existing files)

| File | Addition | Why |
|---|---|---|
| src/selection.rs | claim discipline: `SelectionMetric`, `ClaimBasis`, `SufficiencyClaim`, `MinimalityClaim`, `ClaimRequest`/`ClaimRefusal`, `ClaimError`, `check_sufficiency`, `check_minimality` | extend the OB-02 selection module (the OB-10 work module) with the sufficiency/minimality claim discipline |
| src/lib.rs | doc note for OB-10 | record the gate in the crate docs (additive registration only) |

The gate asserts zero deleted lines in lib.rs, tests/common/mod.rs, and
src/selection.rs, zero changes to delta.rs, receipt.rs, compiler.rs,
closure.rs, handoff.rs, repair.rs, eval.rs, crypto.rs, cli.rs,
model/store/error/sync/provider/http modules, and that src/selection.rs +
src/lib.rs are the only source modules changed.

## Acceptance per delivery plan

- The sufficiency claim is backed by the B8 evaluation: yes (the claim is
  produced by running the frozen eval's simulated recipient over the
  selection's handoff and carries `ClaimBasis::B8Evaluation`).
- The minimality claim is backed by the recorded metric (selected count/bytes
  against budget): yes (the claim carries `SelectionMetric` and
  `ClaimBasis::Metric`).
- A request for a claim beyond the metric is refused: yes (typed
  `ClaimRefusal` for sufficiency without the B8 evaluation, minimality
  without the metric, and global minimality).

## Regression

- OB-08 eval matrix green (`cargo test --test ob08_eval`).
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

- Mary (analyst) — sufficiency/minimality metric verdict for gate B10.
- Amelia (engineer) — completion verdict for gate B10.
- Lunarpulse — final approval.
