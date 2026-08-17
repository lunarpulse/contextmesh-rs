# OB-01 Option B Receipt Evidence (gate B1)

candidate-commit: 6db9232 (delivery-plan freeze; the parent of the OB-01 evidence commit)
procedure-tree: 09d5eb928ac7fce61a819bd79a7892c3da0628c2 (tree of the candidate commit; the OB-01 commit adds the receipt module, the crypto reuse point, the CLI surface, the test matrix, the golden fixture, the verifier, and this evidence)
gate: scripts/verify-ob01.sh (deterministic, non-recording, offline)
verdict: pass (all checkpoints green)
option-b-gate: unblocked-by-complete-verdict (Option A complete; OB-01 is the first Option B implementation package)

## Scope of this evidence

OB-01 implements gate B1 (agent experience receipts) from the frozen spec
`spec-option-b-source-grounded-context-handoff.md` and package OB-01 from
`option-b-delivery-plan.md`. It is purely additive over Option A:

- new module `src/receipt.rs` — the signed, self-contained receipt artifact,
  its canonical wire form, its DAG verification, and export/import;
- additive signing reuse point in `src/crypto.rs` (`sign_domain_message` and
  `verify_domain_message`): the only way to sign a receipt with Option A's
  encapsulated Ed25519 key; introduces no new signature primitive;
- additive module registration in `src/lib.rs` and new `ob-receipt issue|verify`
  subcommands in `src/cli.rs`;
- no Option A store write, no second database, no Option A module behavioral,
  wire, or schema change (verified by the gate's additive-only diff check).

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
- dependency-closure: 320 (unchanged; OB-01 adds no dependency)
- Cargo.lock sha256: e194c2923e697c657e1d1019dbb00718315f529c89df3e3a1374f061fe6609ef
- Locked feature graph: byte-identical to cargo-tree-oa05-features.txt
  (re-asserted by the gate).
- Forbidden-capability audit: no TLS stacks, HTTP/2/3, QUIC, cookies,
  compression, DNS resolvers, shells, libp2p/rusqlite/sqlite alternates in the
  closure; no wall-clock dependency added (the RFC 3339 UTC timestamp is a
  self-contained civil-from-days formatter in receipt.rs).

## Golden fixture

- `tests/fixtures/ob01-receipt-golden.json`
- sha256: 39737b1eb03c26dd66da933bbc26076d3b61262d972c094f87b1f68059dbd642
- Provenance: `regenerate_golden_fixture` (ignored test) reconstructs it from
  the deterministic inputs (author A = fixture seed 7, author B = seed 9,
  context byte 8, B appends two `agent.request` children to A's genesis,
  selector `ob-baseline` 0.1.0, config hash `0123456789abcdef`, task
  "summarize the request chain", created_at fixed `2026-08-17T00:00:00Z`).
- The non-ignored `golden_fixture_matches_reconstruction` test asserts the
  committed bytes still equal this exact reconstruction.

## B1 success evidence

- Receipt verifies against the DAG: golden fixture verified against a rebuilt
  store with `checked_events == 4` (three references plus the recipient head);
  `golden_fixture_verifies_against_rebuilt_dag`.
- Tampered reference rejected: signature, receipt-id, and task-verbatim
  mutations all fail `from_wire` (`tampered_signature_rejected`,
  `tampered_receipt_id_rejected`, `tampered_task_rejected`).
- Missing event rejected: a receipt referencing an unadmitted event reports a
  `missing` finding (`missing_event_reference_rejected`).
- Cross-context event rejected: a receipt referencing an admitted event from
  another context in the same store reports `wrong-context`
  (`cross_context_event_reference_rejected`).
- Unknown recipient head fails closed: `recipient-missing`
  (`unknown_recipient_head_fails_closed`).
- Round-trip: issue → canonical wire → parse → verify → DAG check preserves
  every field (`round_trip_preserves_receipt`); export/import artifact
  round-trip (`export_import_round_trip`).
- Body validation: duplicate and unordered event lists rejected; oversized
  task rejected; malformed created_at rejected.
- CLI: `ob-receipt issue` writes the canonical artifact and verifies
  references before issuing (exit class 4 conflict on missing references);
  `ob-receipt verify` reports `valid:true` or exits with validation (3) on
  tampered input. `--task` accepts verbatim text, `@file`, and `-` (stdin).

## Additive Option A touch points (all additions, no deletions)

| File | Addition | Why |
|---|---|---|
| src/crypto.rs | `SigningIdentity::sign_domain_message`, `verify_domain_message` | reuse Option A's Ed25519 key + strict verification for receipts; key is encapsulated |
| src/lib.rs | `pub mod receipt;` | register the new Option B module |
| src/cli.rs | `ObReceipt` subcommand + dispatch arms | plan-sanctioned CLI surface |

The gate asserts zero deleted lines in these files and zero changes to
model/store/error/sync/provider/http modules.

## Acceptance per delivery plan

- Receipt verifies against the DAG and rejects tampered references: yes (matrix
  above).
- Receipt referencing unknown/unauthorized events fails verification: yes.
- Export → import → verify round-trip: yes.
- No Option A store write; one-process-per-DB held: yes (receipts are exported
  files; DAG checks reuse the existing read-only `Store::event` in the same
  process).

## Regression

- OA-01 through OA-05 verifier chain green (verify-oa01.sh, verify-oa02.sh,
  verify-oa03.sh, verify-oa04.sh, verify-oa04-dependencies.sh,
  verify-oa05.sh).
- Full workspace test suite green (all OA-01..OA-07 suites pass, including the
  OA-06 two-node demo).
- OA-01/OA-03/OA-04 golden fixtures byte-identical (sha256 asserted).

## Evidence owners

- Lunarpulse — completion verdict for gate B1.
- Winston — architecture review of the additive crypto reuse point and the
  one-process-per-DB compliance.
