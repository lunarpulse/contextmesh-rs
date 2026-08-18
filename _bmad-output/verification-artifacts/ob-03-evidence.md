# OB-03 Option B Closure Evidence (gate B3)

candidate-commit: be437e2 (OB-02 selection commit; the parent of the OB-03 evidence commit)
procedure-tree: fdded3c9cd6579ebd402a3c3e0c62bccd3cde192 (tree of the candidate commit; the OB-03 commit adds the closure module, the test matrix, the verifier, and this evidence)
gate: scripts/verify-ob03.sh (deterministic, non-recording, offline)
verdict: pass (all checkpoints green)
option-b-gate: unblocked-by-complete-verdict (OB-01 and OB-02 complete; OB-03 is the B3 closure package of the Option B delivery plan)

## Scope of this evidence

OB-03 implements gate B3 (dependency closure and critical-risk coverage) from
the frozen spec `spec-option-b-source-grounded-context-handoff.md` and package
OB-03 from `option-b-delivery-plan.md`. It is purely additive over Option A,
OB-01, and OB-02:

- new module `src/closure.rs` — the deterministic parent-closure core
  (`close_over` / `close_check`), the read-only store-backed walker
  (`load_closure_nodes`), the critical/risk coverage policy
  (`CriticalPolicy`), checked bounds (`ClosureLimits`), and the combined entry
  point `close_selection`;
- additive module registration in `src/lib.rs` and one additive attribute in
  the shared test helper `tests/common/mod.rs`;
- no change to `src/receipt.rs` (OB-01), no change to `src/selection.rs` /
  `src/compiler.rs` (OB-02), no change to any Option A module (verified by the
  gate's additive-only diff check), no new dependency, no CLI change.

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
- dependency-closure: 320 (unchanged; OB-03 adds no dependency)
- Cargo.lock sha256: e194c2923e697c657e1d1019dbb00718315f529c89df3e3a1374f061fe6609ef
- Locked feature graph: byte-identical to cargo-tree-oa05-features.txt
  (re-asserted by the gate).
- Forbidden-capability audit: no TLS stacks, HTTP/2/3, QUIC, cookies,
  compression, DNS resolvers, shells, libp2p/rusqlite/sqlite alternates in the
  closure; no wall-clock dependency added. The closure walker is self-contained
  in closure.rs.

## Design note: closure walker vs. OA-03 projection

The closure reuses Option A's read-only store access (`Store::event` reparses
and strictly verifies every event on read) and mirrors the deterministic
Enter/Exit ancestry walker of `src/store/dag.rs` (`project_on`), including
in-progress cycle detection and canonical parent ordering. A dedicated walker
is used instead of `Store::project` because projection is bounded to 256 heads
while a selection may reference up to 4096 events, and because the closure
must name the child of a severed edge in its typed failure. The byte limit is
summed over canonical payload bytes to stay consistent with the OB-02
selection budget (OA-03 projection's wire-byte accounting concerns transfer,
not context size).

## B3 success evidence

- Zero dangling references on a valid selection: `close_selection` over a
  real store succeeds with the closed set exactly equal to the selected events
  plus every ancestor, and the pure checker reports an empty dangling list
  (`closure_reports_zero_dangling_on_valid_selection`,
  `valid_nodes_report_zero_dangling`).
- Deliberately severed parent rejected: a node whose parent is absent fails
  `close_check` with the typed `DanglingParent { child, parent }`, and
  `close_over` reports the exact `DanglingEdge`; multiple severed edges are
  all reported in canonical order (`deliberately_severed_parent_rejected`,
  `multiple_dangling_edges_all_reported`).
- Cycle rejected: a cyclic node set fails closed with `Cycle`
  (`cycle_rejected`).
- Closure includes every ancestor: closing over the last event of a 3-child
  linear chain returns the full ancestry (`closure_includes_all_ancestors`).
- Critical/risk coverage: a `context.critical` candidate outside the closure
  is added and reported in `added_critical`; a critical event already in the
  closure is not double-counted; an empty selection still covers critical
  candidates (`critical_events_are_added`,
  `critical_event_in_closure_not_double_counted`,
  `empty_selection_still_covers_critical`).
- Bounds enforced: event-count and exported-byte limits both fail closed with
  `LimitExceeded`, never silently truncated (`closure_respects_event_limit`,
  `closure_respects_byte_limit`).
- Fail-closed edge cases: a selected source absent from the store fails with
  `UnverifiableSource`; a selected event from another context in the same
  store fails with `WrongContext` (`unverifiable_selected_source_fails_closed`,
  `wrong_context_selected_event_fails_closed`).
- Determinism: identical inputs produce byte-identical canonical closed
  selections; scrambled duplicate selected sets normalize to the same result
  (`closure_is_deterministic`, `selected_set_is_normalized`).
- Closed references carry source metadata (event, context, kind, author,
  payload bytes) in the same vocabulary as OB-02
  (`closed_references_carry_source_metadata`).
- Composition with OB-02: selection → closure → receipt over the closed set
  verifies against the DAG with `checked_events == 4`, proving the
  selection→closure→handoff pipeline loads (`composition_selection_then_closure_then_receipt`).

## Additive changes (all additions, no deletions)

| File | Addition | Why |
|---|---|---|
| src/closure.rs | new module | parent closure, severed-parent rejection, critical/risk coverage, bounds |
| src/lib.rs | `pub mod closure;` | register the new Option B module |
| tests/common/mod.rs | `#[allow(dead_code)]` on the shared `genesis` helper | each integration binary uses a subset of the shared helpers |

The gate asserts zero deleted lines in these files and zero changes to
receipt.rs, selection.rs, compiler.rs, crypto.rs, cli.rs,
model/store/error/sync/provider/http modules.

## Acceptance per delivery plan

- Closure check reports zero dangling references on a valid selection: yes.
- A deliberately severed parent is rejected with a typed error: yes
  (`ClosureError::DanglingParent`, pure adversarial matrix).
- Critical/risk-flagged events are present in the closed set: yes
  (`CriticalPolicy` + `added_critical`).

## Regression

- OB-02 selection matrix green (`cargo test --test ob02_selection`).
- OB-01 receipt matrix green (`cargo test --test ob01_receipts`).
- OA-01 through OA-05 verifier chain green (verify-oa01.sh, verify-oa02.sh,
  verify-oa03.sh, verify-oa04.sh, verify-oa04-dependencies.sh,
  verify-oa05.sh).
- Full workspace test suite green (all OA suites pass).
- OB-02/OB-01/OA-01/OA-03/OA-04 golden fixtures byte-identical (sha256
  asserted).

## Evidence owners

- Amelia (engineer) — completion verdict for gate B3.
- Lunarpulse — final approval.
