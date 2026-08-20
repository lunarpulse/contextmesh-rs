# OB-07 Option B Progressive Context Repair Evidence (gate B7)

candidate-commit: c4f9464 (OB-06 handoff commit; the parent of the OB-07 evidence commit)
procedure-tree: 26d734974c39b33c7f2d7b60f388eb792f90304a (tree of the candidate commit; the OB-07 commit adds the repair module, the repair matrix, the verifier, and this evidence)
gate: scripts/verify-ob07.sh (deterministic, non-recording, offline)
verdict: pass (all checkpoints green)
option-b-gate: unblocked-by-complete-verdict (OB-01, OB-02, OB-03, OB-04, OB-05, and OB-06 complete; OB-07 is the B7 progressive context repair package of the Option B delivery plan)

## Scope of this evidence

OB-07 implements gate B7 (progressive context repair) from the frozen spec
`spec-option-b-source-grounded-context-handoff.md` and package OB-07 from
`option-b-delivery-plan.md`. It is purely additive over Option A and
OB-01..OB-06:

- new `src/repair.rs` (the OB-07 work module): the bounded repair loop
  (`run_repair`), the typed `RepairBounds`, the task-outcome driver seam
  (`TaskOutcome`), the typed `NonConvergence` reasons, the JSON-lines
  repair-history store (`RepairHistory` / `RepairAttempt`), and the
  `RepairReport`;
- additive doc note and `pub mod repair;` registration in `src/lib.rs` only;
- no change to `src/handoff.rs` (OB-06), `src/delta.rs` (OB-04),
  `src/receipt.rs` (OB-01), `src/selection.rs` / `src/compiler.rs` (OB-02),
  `src/closure.rs` (OB-03), or any Option A module (verified by the gate's
  additive-only diff check), no new dependency, no CLI change.

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
- dependency-closure: 320 (unchanged; OB-07 adds no dependency)
- Cargo.lock sha256: e194c2923e697c657e1d1019dbb00718315f529c89df3e3a1374f061fe6609ef
- Locked feature graph: byte-identical to cargo-tree-oa05-features.txt
  (re-asserted by the gate).
- Forbidden-capability audit: no TLS stacks, HTTP/2/3, QUIC, cookies,
  compression, DNS resolvers, shells, libp2p/rusqlite/sqlite alternates in the
  closure; no wall-clock dependency added. Repair is self-contained in
  repair.rs and reuses the handoff negotiation (B5/B6) read-only.

## Design notes

**Bounded repair loop.** `run_repair` iteratively re-includes omitted context
and re-handoffs within `RepairBounds { max_iterations, max_re_included_events,
max_delta_bytes }`. Each evaluation of the task driver is one repair attempt;
the loop is always finite, and a sequence converges or reports
non-convergence. A `Success` outcome converges the sequence on the current
handoff; a `Failure` outcome reports non-convergence immediately; a
`NeedsSource` outcome re-includes the named source through the B6 handoff
negotiation and continues.

**Eval-driven convergence seam.** The loop is driven by a task-outcome
callback (`TaskOutcome`). OB-08's eval suite supplies eval-driven convergence
signals through this seam; direct use and the test matrix drive it with a
scripted challenge. The loop itself never fabricates an outcome.

**B5 and B6 compose into B7.** Every re-inclusion goes through
`Handoff::follow_up`, so a stale handoff is never re-negotiated (typed
`Stale`), a source that was never a listed omission fails closed
(`UnknownOmission`), and a re-inclusion whose closed selection does not land
the source in the follow-up delta fails closed (`InvalidState`). Repair cannot
invent context the handoff never omitted.

**JSON-lines history, independent of Option A.** `RepairHistory` records each
attempt as one JSON line in a distinct file that is opened append-only. It
never touches Option A's store and is not a second embedded database in the
store sense — the store is untouched (repair only calls the read-only handoff
negotiation), the file is plain JSON lines, and the sequence numbering
continues across runs. On non-convergence the original handoff is left intact
(proven byte-for-byte), and the typed `NonConvergence` reason is recorded on
the terminal history record so the sequence is auditable from the evidence
alone.

## B7 success evidence

- A scripted challenge drives a convergent sequence within the bound: the
  re-inclusion lands in the follow-up handoff, the convergent handoff is still
  state-bound (B5), the original handoff is byte-identical afterward, and the
  two-attempt history (re-inclusion step + convergent terminal) round-trips
  from the file (`repair_converges_within_the_bound_and_records_attempt_history`).
- A task that succeeds immediately converges with one attempt, no
  re-inclusion, and a convergent terminal record
  (`repair_converges_immediately_when_the_task_succeeds`).
- A sequence that exhausts the iteration bound reports
  `IterationBudgetExceeded`, leaves the original handoff byte-identical, and
  records the re-inclusion attempt plus the terminal non-convergence record
  (`repair_reports_non_convergence_when_the_iteration_budget_is_exhausted`).
- A driver that reports a task failure yields `OutcomeFailure` with the
  original handoff intact and the failure note on the terminal record
  (`repair_reports_non_convergence_when_the_driver_reports_failure`).
- The re-inclusion budget is enforced: a bound of 1 stops the two-step
  sequence with `ReInclusionBudgetExceeded` and the original handoff intact,
  while a bound of 2 converges it (`repair_re_inclusion_budget_is_bounded_and_converges_within_it`).
- The delta byte budget is enforced: a bound equal to the initial delta
  reports `ByteBudgetExceeded` with the original handoff intact, while a bound
  that admits the follow-up delta converges (`repair_byte_budget_is_bounded`).
- Repair fails closed when the driver asks for a source that was never a
  listed omission (`UnknownOmission`) or when the supplied closed selection
  never lands the source in the delta (`InvalidState`), recording nothing
  (`repair_fails_closed_for_a_source_that_was_never_omitted_or_never_lands`).
- The repair-history file is a distinct JSON-lines file, not the Option A DB:
  the store still answers after the repair, no sqlite artifacts appear
  alongside the history, and the records round-trip from the file alone
  (`repair_history_file_is_independent_of_option_a_db`).
- Identical inputs produce a byte-identical repair history across separate
  runs (`repair_history_is_deterministic_on_the_wire`).
- A stale handoff is never re-negotiated: when the recipient advances, the
  repair fails closed with the typed `Stale` error and records nothing
  (`repair_never_negotiates_a_stale_handoff`).

## Additive changes (all additions, no deletions in existing files)

| File | Addition | Why |
|---|---|---|
| src/repair.rs (new) | `RepairBounds`, `TaskOutcome`, `NonConvergence`, `TerminalRecord`, `RepairAttempt`, `RepairHistory` (append-only JSON-lines store), `RepairReport`, `RepairError`, and `run_repair` (the bounded repair loop) | the OB-07 work module: progressive context repair with attempt-history evidence |
| src/lib.rs | doc note for OB-07 and `pub mod repair;` | record the gate in the crate docs (additive registration only) |

The gate asserts zero deleted lines in lib.rs and tests/common/mod.rs, zero
changes to delta.rs, receipt.rs, selection.rs, compiler.rs, closure.rs,
handoff.rs, crypto.rs, cli.rs, model/store/error/sync/provider/http modules,
and that src/repair.rs + src/lib.rs are the only source modules changed.

## Acceptance per delivery plan

- A repair sequence converges within the bound and records attempt history:
  yes (the convergent sequence records every attempt including the terminal
  `Converged` record).
- A non-converging sequence reports non-convergence and leaves the original
  handoff intact: yes (typed `NonConvergence` on the terminal record and on
  the report; the original handoff is byte-identical afterward).
- The repair-history file is independent of Option A's DB: yes (a distinct
  JSON-lines file, append-only, never opened through the store; verified by
  the independence test and the gate's runtime-artifact scan).

## Regression

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
- OB-02/OB-01/OA-01/OA-03/OA-04 golden fixtures byte-identical (sha256
  asserted).

## Evidence owners

- Amelia (engineer) — completion verdict for gate B7.
- Lunarpulse — final approval.
