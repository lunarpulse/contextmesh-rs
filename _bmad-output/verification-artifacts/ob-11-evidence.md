# OB-11 Option B Recipient Capability Modeling Evidence (gate B11)

candidate-commit: d0f848f (OB-09 summaries commit; the parent of the OB-11 evidence commit)
procedure-tree: d6f9ca60fd3eaf6e478d3487b8e2fdf648099275 (tree of the candidate commit; the OB-11 commit adds the capability module, the capability matrix, the verifier, and this evidence)
gate: scripts/verify-ob11.sh (deterministic, non-recording, offline)
verdict: pass (all checkpoints green)
option-b-gate: unblocked-by-complete-verdict (OB-01 through OB-10 complete; OB-11 is the B11 recipient capability modeling package of the Option B delivery plan)

## Scope of this evidence

OB-11 implements gate B11 (recipient capability modeling) from the frozen
spec `spec-option-b-source-grounded-context-handoff.md` and package OB-11 from
`option-b-delivery-plan.md`. It is purely additive over Option A and
OB-01..OB-10:

- new `src/capability.rs` (the OB-11 work module): the recorded, versioned
  `RecipientCapabilities` model per recipient (`Capability` covers event
  kinds), `shape_handoff` (a capability mismatch is flagged in the B6
  omission/uncertainty list, never assumed), `verify_handoff` (the discipline
  is verified, not assumed), and `CapabilityVerification`;
- additive doc note and `pub mod capability;` registration in `src/lib.rs`
  only;
- no change to `src/handoff.rs` (OB-06), `src/summary.rs` (OB-09),
  `src/selection.rs` (OB-10), `src/eval.rs` (OB-08), `src/repair.rs` (OB-07),
  `src/delta.rs` (OB-04), `src/closure.rs` (OB-03), or any Option A module
  (verified by the gate's additive-only diff check), no new dependency, no
  CLI change. The capability-mismatch flags surface through the B6
  `OmissionReason::CapabilityMismatch` (typed in OB-06, wired in here).

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
- dependency-closure: 320 (unchanged; OB-11 adds no dependency)
- Cargo.lock sha256: e194c2923e697c657e1d1019dbb00718315f529c89df3e3a1374f061fe6609ef
- Locked feature graph: byte-identical to cargo-tree-oa05-features.txt
  (re-asserted by the gate).
- Forbidden-capability audit: no TLS stacks, HTTP/2/3, QUIC, cookies,
  compression, DNS resolvers, shells, libp2p/rusqlite/sqlite alternates in the
  closure; no wall-clock dependency added. Capability modeling is
  self-contained in capability.rs and reads the store read-only.

## Design notes

**Recorded, versioned capability model.** `RecipientCapabilities` is the
recorded, versioned model per recipient (identified by author identity): a
model version, a canonical capability set, and a canonical wire. Each
`Capability` declares a name and the event kinds it covers; `covers` is a
deterministic kind check. A model with no declared capabilities covers
nothing.

**Flagged, never assumed.** `shape_handoff` walks the handoff's delivered
references and flags every carried event whose kind is uncovered as an
explicit uncertainty marker (`capability mismatch: event … kind …`). Shaping
is additive to knowledge — the event stays carried — and a fully covered
handoff is returned byte-identical. A deliberately withheld
capability-mismatch source is recorded as a B6 omission with the typed
`OmissionReason::CapabilityMismatch` (the reason OB-06 reserved for this
gate), so the flag is a first-class, challengeable omission.

**The discipline is verified, not assumed.** `verify_handoff` rejects a
`DishonestFlag` (a capability-mismatch omission naming an event the recipient
can act on) and an `UnflaggedMismatch` (a carried event the recipient cannot
act on with no flag). B4 remains the known-history delta; capability is
additive to knowledge.

## B11 success evidence

- The capability model is recorded and versioned, its canonical wire
  round-trips, and it covers only the declared kinds
  (`capability_model_is_recorded_and_versioned`,
  `capability_model_covers_declared_kinds_only`).
- A carried event the recipient cannot act on is flagged as an explicit
  uncertainty marker while staying carried, and the shaped handoff verifies
  (`shape_handoff_flags_uncovered_carried_events_in_the_uncertainty_list`).
- A handoff shaped against a model that covers every carried kind is returned
  byte-identical with no flags (`shape_handoff_respects_stated_capabilities`).
- A capability-mismatch withholding is a consistent, first-class B6 omission
  that can be challenged (`capability_mismatch_omissions_are_consistent_and_challengeable`).
- A capability-mismatch flag naming a covered event is rejected as
  `DishonestFlag` (`verify_handoff_rejects_dishonest_capability_flags`).
- A carried uncovered event with no flag is rejected as `UnflaggedMismatch`
  (`verify_handoff_rejects_unflagged_mismatches`).
- Identical models and shaping runs produce byte-identical wires and reports
  (`capability_models_and_shaping_are_deterministic`).
- The shaped handoff is still state-bound against the recipient head (B5
  composes into B11) (`shaped_handoff_still_verifies_against_the_dag`).
- Duplicate capability names, empty kinds, and empty names fail closed
  (`invalid_capability_models_fail_closed`).

## Additive changes (all additions, no deletions in existing files)

| File | Addition | Why |
|---|---|---|
| src/capability.rs (new) | `Capability`, `RecipientCapabilities` (recorded + versioned), `CapabilityMismatch`, `CapabilityReport`, `ShapedHandoff`, `shape_handoff`, `verify_handoff`, `CapabilityVerification`, `CapabilityError` | the OB-11 work module: model what a recipient can do and flag capability mismatches through B6 |
| src/lib.rs | doc note for OB-11 and `pub mod capability;` | record the gate in the crate docs (additive registration only) |

The gate asserts zero deleted lines in lib.rs, tests/common/mod.rs, and
src/capability.rs, zero changes to delta.rs, receipt.rs, compiler.rs,
closure.rs, handoff.rs, repair.rs, eval.rs, selection.rs, summary.rs,
crypto.rs, cli.rs, model/store/error/sync/provider/http modules, and that
src/capability.rs + src/lib.rs are the only source modules changed.

## Acceptance per delivery plan

- The capability model is recorded and versioned: yes (`RecipientCapabilities`
  carries the recipient identity, a model version, the canonical capability
  set, and a canonical wire that round-trips).
- A handoff respects the recipient's stated capabilities; a mismatch is
  flagged in the omission/uncertainty list: yes (`shape_handoff` flags every
  uncovered carried event as an uncertainty marker and records a deliberately
  withheld capability-mismatch source as a B6 `CapabilityMismatch` omission;
  `verify_handoff` rejects dishonest and silent flags).

## Regression

- OB-09 summaries matrix green (`cargo test --test ob09_summaries`).
- OB-10 sufficiency matrix green (`cargo test --test ob10_sufficient`).
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

- Winston (architecture) — capability-model design verdict for gate B11.
- Amelia (engineer) — completion verdict for gate B11.
- Lunarpulse — final approval.
