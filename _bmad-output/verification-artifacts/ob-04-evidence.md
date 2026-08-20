# OB-04 Option B Delta Evidence (gate B4)

candidate-commit: e2c23a8 (OB-03 closure commit; the parent of the OB-04 evidence commit)
procedure-tree: 3d167e378fa3a3172d4b4a163b59e21c154f915f (tree of the candidate commit; the OB-04 commit adds the delta module, the test matrix, the verifier, and this evidence)
gate: scripts/verify-ob04.sh (deterministic, non-recording, offline)
verdict: pass (all checkpoints green)
option-b-gate: unblocked-by-complete-verdict (OB-01, OB-02, and OB-03 complete; OB-04 is the B4 delta package of the Option B delivery plan)

## Scope of this evidence

OB-04 implements gate B4 (recipient-known-history delta) from the frozen spec
`spec-option-b-source-grounded-context-handoff.md` and package OB-04 from
`option-b-delivery-plan.md`. It is purely additive over Option A, OB-01,
OB-02, and OB-03:

- new module `src/delta.rs` — the recipient-state record (known-history head +
  derived closure), the strict read-only ancestry walker (`RecipientState::
  at_head`), the deterministic pure partition (`delta_over`), and the
  store-backed entry point `compute_delta`;
- additive module registration in `src/lib.rs`;
- no change to `src/receipt.rs` (OB-01), no change to `src/selection.rs` /
  `src/compiler.rs` (OB-02), no change to `src/closure.rs` (OB-03), no change
  to any Option A module (verified by the gate's additive-only diff check), no
  new dependency, no CLI change.

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
- dependency-closure: 320 (unchanged; OB-04 adds no dependency)
- Cargo.lock sha256: e194c2923e697c657e1d1019dbb00718315f529c89df3e3a1374f061fe6609ef
- Locked feature graph: byte-identical to cargo-tree-oa05-features.txt
  (re-asserted by the gate).
- Forbidden-capability audit: no TLS stacks, HTTP/2/3, QUIC, cookies,
  compression, DNS resolvers, shells, libp2p/rusqlite/sqlite alternates in the
  closure; no wall-clock dependency added. The delta walker is self-contained
  in delta.rs.

## Design notes

**Recipient-state record.** `RecipientState` is the stated known history:
context, head (`None` for cold-start), the derived closure (head plus every
ancestor over Option A parent edges), and the canonical-payload byte total of
the known history. The closure is derived and strictly verified by
`RecipientState::at_head` against the same read-only store discipline as B3
(every event reparsed and verified on read; cycle, dangling parent,
cross-context edge, and bound violations fail closed). B4 is strictly a
known-history delta; recipient *capability* modeling remains gate B11.

**Provability.** `compute_delta` fails closed with `UnknownRecipientHead` when
the stated head is not a node of the DAG (unknown recipient state is never
assumed) and with `WrongContext` / `ContextMismatch` when the head or the
recipient state disagrees with the selection's context. The delta record
carries the recipient head, the recipient closure used, and the selected
events already known, so `delta ∪ known == selected` and `known ⊆ closure` are
auditable from the record alone. The provability test re-derives the closure
independently from the store and asserts the recorded partition is exactly
selected minus the re-derived closure.

**Pure vs store-backed.** `delta_over` is a deterministic pure partition
(normalized canonical output regardless of input order), so adversarial and
determinism tests run without a database; `compute_delta` and
`RecipientState::at_head` are the store-backed entry points. The byte total is
summed over canonical payload bytes, consistent with OB-02/OB-03 accounting.

## B4 success evidence

- Delta equals exactly the closed selected events outside the recipient's
  closure: closing a 4-event chain and setting the recipient head to the first
  child yields `{c2, c3}` as the delta and `{genesis, c1}` as the selected
  known events; a head at the tip yields an empty delta with all selected
  events known (`delta_matches_selected_minus_recipient_closure`,
  `recipient_head_at_tip_produces_empty_delta`).
- Cold-start recipient (empty known history) produces the full closed
  selection as the delta, with an empty known set and zero bytes
  (`cold_start_recipient_produces_full_selection`).
- A recipient head not present in the DAG fails closed with the typed
  `UnknownRecipientHead`, never assumed (`unknown_recipient_head_fails_closed`).
- A recipient head in another context, and a recipient state whose context
  disagrees with the selection, both fail closed (`WrongContext`,
  `ContextMismatch`) (`recipient_head_wrong_context_fails_closed`,
  `context_mismatch_fails_closed`).
- The delta is provable: independent re-derivation of the closure from the
  store reproduces the recorded closure, and the recorded partition is exactly
  selected minus the re-derived closure; every delta event is absent from the
  closure and every known event is present in it
  (`delta_is_provable_from_store`).
- Bounds enforced: a tight event-count or byte limit on the recipient closure
  walk fails closed with `LimitExceeded`; an over-budget fabricated closure is
  rejected at the record level (`delta_respects_recipient_closure_bounds`).
- Determinism: identical inputs produce byte-identical canonical delta wires
  and identical references and known sets across runs
  (`delta_is_deterministic_across_runs`); the pure partition is canonical over
  scrambled, duplicated inputs (`delta_over_pure_partition_is_canonical`).
- The recipient-state record validates: a head outside its own closure is
  rejected as `InvalidState`; scrambled, duplicated closures normalize to
  canonical order (`recipient_state_from_closure_validates`).
- Delta byte accounting is consistent: `total_bytes` equals the sum of the
  delta references' canonical payload bytes, and the delta inherits the
  selection's limits (`delta_total_bytes_match_references`).
- Composition with OB-01/OB-02/OB-03: selection → closure → delta → receipt
  over the delta events (child + critical, recipient knowing only genesis)
  verifies against the DAG with `checked_events == 3` (delta events plus the
  recipient head), proving the selection→closure→delta→handoff pipeline loads
  (`composition_selection_closure_delta_receipt`).

## Additive changes (all additions, no deletions)

| File | Addition | Why |
|---|---|---|
| src/delta.rs | new module | recipient-state record, closure derivation, delta partition and store-backed computation |
| src/lib.rs | `pub mod delta;` plus doc note | register the new Option B module |

The gate asserts zero deleted lines in lib.rs and tests/common/mod.rs and zero
changes to receipt.rs, selection.rs, compiler.rs, closure.rs, crypto.rs,
cli.rs, model/store/error/sync/provider/http modules.

## Acceptance per delivery plan

- Delta contains exactly the selected events outside the recipient's closure:
  yes (partition tests, provability test).
- A recipient head that is not an ancestor, or not present, fails closed: yes
  (`UnknownRecipientHead`, `WrongContext`, `ContextMismatch`).
- Cold-start produces the full selection as the delta: yes
  (`cold_start_recipient_produces_full_selection`).

## Regression

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

- Amelia (engineer) — completion verdict for gate B4.
- Lunarpulse — final approval.
