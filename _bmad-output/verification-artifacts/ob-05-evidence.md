# OB-05 Option B State-Bound Handoff Validity Evidence (gate B5)

candidate-commit: 92427d1 (OB-04 delta commit; the parent of the OB-05 evidence commit)
procedure-tree: ab41e721aa13da630383a5abc067f0aaae95acca (tree of the candidate commit; the OB-05 commit adds the handoff module, the validity matrix, the verifier, and this evidence)
gate: scripts/verify-ob05.sh (deterministic, non-recording, offline)
verdict: pass (all checkpoints green)
option-b-gate: unblocked-by-complete-verdict (OB-01, OB-02, OB-03, and OB-04 complete; OB-05 is the B5 state-bound handoff validity package of the Option B delivery plan)

## Scope of this evidence

OB-05 implements gate B5 (state-bound handoff validity) from the frozen spec
`spec-option-b-source-grounded-context-handoff.md` and package OB-05 from
`option-b-delivery-plan.md`. It is purely additive over Option A, OB-01,
OB-02, OB-03, and OB-04:

- new module `src/handoff.rs` — the state-bound `Handoff` type (a B4 `Delta`
  bound to the recipient head it was computed against), the typed stale error,
  the store-backed validity check, and the deliverable gate;
- additive module registration in `src/lib.rs`;
- no change to `src/delta.rs` (OB-04), no change to `src/receipt.rs` (OB-01),
  no change to `src/selection.rs` / `src/compiler.rs` (OB-02), no change to
  `src/closure.rs` (OB-03), no change to any Option A module (verified by the
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
- dependency-closure: 320 (unchanged; OB-05 adds no dependency)
- Cargo.lock sha256: e194c2923e697c657e1d1019dbb00718315f529c89df3e3a1374f061fe6609ef
- Locked feature graph: byte-identical to cargo-tree-oa05-features.txt
  (re-asserted by the gate).
- Forbidden-capability audit: no TLS stacks, HTTP/2/3, QUIC, cookies,
  compression, DNS resolvers, shells, libp2p/rusqlite/sqlite alternates in the
  closure; no wall-clock dependency added. The validity check is
  self-contained in handoff.rs.

## Design notes

**State-bound handoff.** `Handoff` wraps the B4 `Delta` — which already
carries the recipient head and recipient closure it was computed against — and
adds the validity contract. `Handoff::from_delta` fails closed with
`InvalidState` if the recorded recipient head is not a member of the recorded
recipient closure (a recipient always knows its own head). The handoff's
canonical wire form is deterministic (RFC 8785/JCS over the serialized record).

**Validity check.** `Handoff::verify_valid(store, current_head)` requires both
the handoff's embedded recipient head and the recipient's current stated head
to be present in the DAG and to belong to the handoff's context — an unknown
recipient state fails closed with `UnknownRecipientHead` / `WrongContext` and
is never assumed, matching the I/O & edge-case matrix. The handoff is valid
only when the two heads agree; when the recipient advanced, it is rejected
with the typed `Stale { computed, current }` error. The check is a pure
function of the DAG and the stated head, so it is idempotent while the head is
unchanged.

**Never applied.** The delta is obtainable only through
`Handoff::verified_delta`, which runs the validity check first: a stale
handoff yields the typed stale error and its delta is never returned, so a
stale handoff is never applied. Re-derivation is the caller's explicit
follow-up: build the recipient's new state with `RecipientState::at_head` at
the advanced head and run B4 `compute_delta` again; the original handoff
record is left intact.

## B5 success evidence

- A handoff computed against head H verifies against H and its delta is
  obtainable; the handoff records context, head, cold-start flag, and the
  canonical delta events (`handoff_is_valid_against_its_stated_head`).
- When the recipient advances to H′, the same handoff is rejected with the
  typed `Stale { computed: H, current: H′ }` error, and `verified_delta`
  refuses to return the delta (`stale_handoff_is_rejected_when_recipient_advances`).
- Re-deriving against H′ succeeds (the new handoff carries H′ and verifies),
  and the original handoff's canonical wire form is byte-identical before and
  after (`re_deriving_against_the_new_head_succeeds_and_original_is_intact`).
- A cold-start handoff (computed against no head) is valid while the
  recipient is still cold and carries the full closed selection; it becomes
  `Stale { computed: None, current: Some(H) }` once the recipient advances
  (`cold_start_handoff_is_valid_until_the_recipient_advances`).
- A stated recipient head not present in the DAG — the current head, or the
  handoff's embedded head checked against a store that does not contain it —
  fails closed with `UnknownRecipientHead`, never assumed
  (`unknown_recipient_head_fails_closed`).
- A current stated head that is present but lies in another context fails
  closed with `WrongContext` (`head_from_another_context_fails_closed`).
- Verification is idempotent while the head is unchanged: repeated valid
  checks all pass, and repeated stale checks return the identical error
  (`verification_is_idempotent_while_the_head_is_unchanged`).
- Identical inputs produce byte-identical canonical handoff wires that parse
  as JSON (`handoff_wire_is_deterministic`).
- Composition with OB-01/OB-02/OB-03/OB-04: selection → closure → delta →
  handoff → receipt over the delta events (child + critical, recipient
  knowing only genesis) verifies against the DAG with `checked_events == 3`
  (delta events plus the recipient head), and the same handoff is stale once
  the recipient advances to the child (`handoff_composition_selection_closure_delta_receipt`).

## Additive changes (all additions, no deletions)

| File | Addition | Why |
|---|---|---|
| src/handoff.rs | new module | state-bound handoff type, typed stale error, validity check, deliverable gate |
| src/lib.rs | `pub mod handoff;` plus doc note | register the new Option B module |

The gate asserts zero deleted lines in lib.rs and tests/common/mod.rs and zero
changes to delta.rs, receipt.rs, selection.rs, compiler.rs, closure.rs,
crypto.rs, cli.rs, model/store/error/sync/provider/http modules.

## Acceptance per delivery plan

- A handoff computed against head H is rejected when the recipient advances to
  H′: yes (`Stale` error; `verified_delta` refuses delivery).
- Re-deriving against H′ succeeds; the original handoff is left intact: yes
  (re-derived handoff valid, original wire byte-identical).
- A recipient head not present in the DAG fails closed (B4/B5): yes
  (`UnknownRecipientHead` / `WrongContext`, never assumed).
- Idempotent when the head is unchanged: yes (repeated verification returns
  the same verdict).

## Regression

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

- Amelia (engineer) — completion verdict for gate B5.
- Winston (architecture review) — state-safety review of the B4+B5 handoff
  composition.
- Lunarpulse — final approval.
