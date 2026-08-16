# contextmesh

contextmesh currently implements **OA-01**, the persistence-independent signed
event contract for Option A (verifiable distributed agent history). OA-00's
Rust 1.97/Turso baseline remains intact. Turso persistence is not used by the
contract yet; that begins in OA-02.

## Toolchain and verification

The project pins Rust **1.97.0**, edition **2024**, rustfmt, and Clippy in
`rust-toolchain.toml`. Install or refresh the user-local toolchain without
root privileges:

```bash
bash scripts/bootstrap-rust.sh
. "$HOME/.cargo/env"
```

Verify OA-01 from the repository root:

```bash
cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
bash scripts/verify-oa01.sh
```

The verifier checks exact dependency versions/features, the locked feature
graph, golden-vector checksum and regeneration, deferred-module boundaries,
every quality command, and the expected OA-06 failure sentinel.

## Frozen signed-event v1 contract

A signed event has this logical wire form. Rendering always uses RFC 8785/JCS:

```json
{
  "event_id": "evt1_<43 unpadded base64url characters>",
  "body": {
    "version": 1,
    "context": "ctx1_<43>",
    "parents": [],
    "kind": "agent.request",
    "author": "ed25519_<43>",
    "payload": {}
  },
  "signature": "sig1_<86>"
}
```

- `EventId`, `ContextId`, and `AuthorId` decode to exactly 32 bytes.
- `EventSignature` decodes to exactly 64 bytes.
- Text uses URL-safe base64 without padding and must decode/re-encode exactly.
- Parents are strictly increasing by canonical `EventId` text and unique.
- Kinds are 1–64 ASCII bytes under the frozen lowercase segment grammar.
- Unicode is preserved without normalization.
- Duplicate keys at every JSON depth, unknown envelope/body fields, BOMs, and
  trailing JSON are rejected.

### Identity and signature

Canonical body bytes are hashed with BLAKE3 derive-key mode using:

```text
org.aaif.contextmesh.event-id.v1
```

The Ed25519 signing message is:

```text
ASCII("org.aaif.contextmesh.signature.v1") || NUL || raw_event_id[32]
```

Verification recomputes the ID and uses Ed25519 strict verification. A body can
be signed only when its author equals the signing identity's public key.
Private keys are neither serializable nor exposed. Production identities use
fallible OS entropy; checked-in fixed seeds are explicitly test-only material.

### Limits

| Item | v1 limit |
|---|---:|
| Raw wire JSON | 2,097,152 bytes |
| Canonical payload | 1,048,576 bytes |
| Canonical body | 1,114,112 bytes |
| Parents | 64 |
| Payload depth | 64 |
| Kind | 64 bytes |
| Integer-valued JSON number | ±9,007,199,254,740,991 |

Larger exact integers must be strings. Finite non-integer binary64 values are
allowed. Public failures use non-secret `ContractError` categories and
external malformed input must not panic.

## Golden vectors

`tests/fixtures/oa01-v1-golden.json` records a deterministic fixed-seed event,
canonical body/envelope bytes, ID, author key, signing message, and signature.
The seed is public and must never be used in production. Tests also cover RFC
8785 number and UTF-16 ordering examples, canonical-equivalent JSON, Unicode
non-normalization, all signed-field mutations, strict duplicate detection at
maximum depth, every typed encoding, strict-Ed25519 malleability/weak-key cases,
limits (including exact raw-wire size), malformed input, and no-panic behavior.

Changing any v1 field, prefix, domain, limit, canonical bytes, ID, or signature
now requires explicit human approval and normally a new wire version.

## Dependency boundary

OA-01 uses exact audited versions of `serde_jcs`, `blake3`,
`ed25519-dalek`, `getrandom`, `zeroize`, `base64`, Serde, and
`thiserror`. Turso remains exactly 0.7.2 with top-level defaults and sync
disabled. The captured graph is `cargo-tree-oa01-features.txt`.

## Deferred scope

- OA-02: Turso schema, admission authorization, parent existence/context, refs.
- OA-03: projections and bundles.
- OA-04: authenticated HTTP anti-entropy synchronization.
- OA-05: provider recording and real CLI commands.
- OA-06: two-node demonstration. Until then, `scripts/demo.sh` intentionally
  exits 1.
- Option B: semantic context selection and handoff.

The current placeholder binaries also exit unsuccessfully so automation cannot
mistake future OA-05 functionality for an implemented command.
