---
title: 'OA-01 Dependency and Interoperability Plan'
type: 'dependency-plan'
created: '2026-08-16'
status: 'approved'
source_spec: '../implementation-artifacts/spec-oa-01-signed-event-contract.md'
baseline_commit: '53777ce3668708a5f1b668d25c2a461d04b9985e'
---

# OA-01 Dependency and Interoperability Plan

## 1. Decision Summary

Use exact stable versions while the v1 wire contract and golden vectors are established. Cargo.lock remains authoritative. The set below compiled and executed together on Rust 1.97.0 in a disposable probe on 2026-08-16; it resolved 46 packages before merging with the existing Turso graph.

```toml
[dependencies]
base64 = { version = "=0.23.1", default-features = false, features = ["std"] }
blake3 = { version = "=1.8.6", default-features = false, features = ["std"] }
ed25519-dalek = { version = "=3.0.0", default-features = false, features = ["fast", "zeroize"] }
getrandom = { version = "=0.4.3", default-features = false }
serde = { version = "=1.0.229", features = ["derive"] }
serde_jcs = "=0.2.0"
serde_json = { version = "=1.0.151", default-features = false, features = ["std", "float_roundtrip"] }
thiserror = "=2.0.20"
zeroize = { version = "=1.9.0", default-features = false, features = ["alloc"] }
```

Do not add direct dependencies on rand, rand_core, hex, regex, uuid, serde_json_canonicalizer, signature, or an alternative database/network stack in OA-01.

## 2. Dependency Decisions

| Dependency | Purpose and selected features | Evidence / boundary |
|---|---|---|
| serde 1.0.229 | Derive stable typed v1 structures | MSRV 1.56; direct derives only |
| serde_json 1.0.151 | Strict raw JSON ingress and Value payload | MSRV 1.71; std + float_roundtrip; never enable arbitrary_precision, preserve_order, raw_value, or unbounded_depth |
| serde_jcs 0.2.0 | RFC 8785 canonical bytes | MSRV 1.85, edition 2024, MIT/Apache-2.0; uses UTF-16 key ordering and ryu-js; cross-check with RFC fixtures |
| blake3 1.8.6 | 32-byte EventId using derive-key mode | std only; no serde, rayon, mmap, digest, or keyed-hash API in the contract |
| ed25519-dalek 3.0.0 | Ed25519 signatures and strict verification | MSRV 1.85; fast + zeroize only; no serde, batch, hazmat, pem, pkcs8, digest, legacy compatibility, or rand_core |
| getrandom 0.4.3 | Fallible OS entropy for 32-byte seed | MSRV 1.85; call fill directly; no wasm_js/sys_rng; unsupported targets return typed error |
| zeroize 1.9.0 | Zeroizing temporary seed and wrapper secrets | alloc only; SigningKey also has dalek zeroize-on-drop |
| base64 0.23.1 | Canonical URL_SAFE_NO_PAD wire strings | std only; disables default simd-unsafe; reject padding and decode aliases by exact re-encode |
| thiserror 2.0.20 | Stable typed public failures | std default; errors must not contain payloads, private material, or secret bytes |

### Why serde_jcs

It is the current stable 0.2.0 release, declares MSRV 1.85, uses UTF-16 property ordering, ryu-js ECMAScript number formatting, and disables dependency defaults. serde_json_canonicalizer 0.3.2 has broader test data/downloads but declares no MSRV and enables broader defaults; json-canon 0.1.3 has not been updated since 2023. The selected crate is not trusted alone: checked-in RFC 8785 and upstream JCS fixtures define expected bytes.

### RNG decision

ed25519-dalek 3.0.0 SigningKey::generate expects infallible CryptoRng, while getrandom 0.4 SysRng is fallible and failed the compatibility probe. Generate with a zeroizing [u8;32], call getrandom::fill, then SigningKey::from_bytes. Never persist or serialize the seed in OA-01. Fixed test seeds are fixture-only and conspicuously labeled non-production.

## 3. Frozen v1 Encoding Decisions

| Type | Wire form | Decoded size |
|---|---|---:|
| EventId | evt1_ + URL_SAFE_NO_PAD | 32 bytes / 43 payload chars |
| ContextId | ctx1_ + URL_SAFE_NO_PAD | 32 bytes / 43 payload chars |
| AuthorId | ed25519_ + URL_SAFE_NO_PAD verifying key | 32 bytes / 43 payload chars |
| Signature | sig1_ + URL_SAFE_NO_PAD | 64 bytes / 86 payload chars |

All parsers require exact prefix, exact total length, successful decode, and byte-for-byte equality with re-encoding. Display/Serialize emits only canonical text. Deserialize/FromStr shares the same parser.

```json
{"event_id":"evt1_<43>","body":{"version":1,"context":"ctx1_<43>","parents":[],"kind":"agent.request","author":"ed25519_<43>","payload":{}},"signature":"sig1_<86>"}
```

- Body bytes: serde_jcs::to_vec(EventBodyV1).
- Event ID: BLAKE3 Hasher::new_derive_key("org.aaif.contextmesh.event-id.v1") over body bytes.
- Signing message: b"org.aaif.contextmesh.signature.v1\0" || event_id[32].
- Signature: ordinary Ed25519 over the signing message; verify_strict is mandatory.
- Wire output: JCS of SignedEventV1. Input may contain insignificant whitespace/key-order differences but not duplicate/unknown fields.

## 4. Strict JSON and I-JSON Subset

A serde_json::Value alone is insufficient because duplicate object members become last-wins. Implement a recursive strict Value deserializer using MapAccess plus a key set; apply deny_unknown_fields to body/envelope. Reject trailing data and a UTF-8 BOM. Keep serde_json's recursion protection and additionally reject payload depth greater than 64.

Before canonicalization, recursively enforce:

- unique object names at ingress;
- valid Rust UTF-8 strings; no Unicode normalization;
- finite JSON numbers;
- finite binary64 decimals are allowed; any integer-valued number must be in [-9007199254740991, 9007199254740991], with larger integers represented as strings;
- canonical payload size <= 1,048,576 bytes;
- canonical body size <= 1,114,112 bytes;
- raw wire size <= 2,097,152 bytes;
- parents <= 64 and strictly increasing canonical EventId text;
- kind 1..64 ASCII bytes matching the spec grammar.

Programmatic construction must run the same semantic validation as raw ingress.

Validation order is deterministic: raw size; strict JSON syntax/duplicates/trailing data; envelope/body field set and version; canonical text decoding; kind/parent/depth/number limits; canonical payload/body sizes; recomputed ID equality; strict signature verification. Creation validates the body first, confirms the signing key matches author, then computes ID and signature. Public error variants must distinguish WireTooLarge, JsonSyntax, DuplicateKey, UnknownField, MissingField, UnsupportedVersion, InvalidEncoding, InvalidKind, ParentOrder, LimitExceeded, UnsafeNumber, Canonicalization, Entropy, AuthorMismatch, IdMismatch, and SignatureInvalid without embedding payload or secret bytes.

## 5. Golden Vector Plan

Create tests/fixtures/oa01-v1-golden.json with a schema version and provenance URLs/commit identifiers. It must contain full canonical body/envelope UTF-8 or base64, raw identifier bytes, canonical wire strings, author public key, signature, and expected typed outcome. Never include a production secret; use a documented fixed 32-byte seed only to regenerate vectors.

Required valid vectors:

1. Empty-object payload and zero parents.
2. Same nested payload supplied in multiple object orders/whitespace/escape forms.
3. RFC 8785 serialization example, number formatting, and UTF-16 property ordering.
4. Unicode without NFC normalization and escaped/unescaped equivalent text.
5. Single parent and sorted 64-parent boundary.
6. -0, exponent aliases, safe integer extrema, maximum kind, depth 64, and payload/body size boundaries.

Required rejection/mutation cases:

- duplicate keys at envelope/body/every payload depth;
- unknown/missing/wrong-type fields, version 0/2, BOM, trailing data;
- padded, wrong-prefix, wrong-length, invalid-alphabet, and noncanonical text encodings;
- duplicate/unsorted/65 parents; invalid kind; depth 65; each size limit +1;
- unsafe integer extrema +1 and non-finite programmatic values;
- author/signing-key mismatch;
- every envelope/body field mutated independently, event ID bit flip, signature bit flip, malformed public key/signature, wrong signing-domain message.

Golden generation must be a deterministic test/helper that compares against the checked-in fixture; normal tests must never rewrite fixtures. Updating any expected bytes requires explicit human approval and a wire-version/change decision.

## 6. Dependency and Supply-Chain Verification

scripts/verify-oa01.sh must assert via cargo metadata/tree:

- all direct dependencies equal the versions/features above;
- Turso remains exactly 0.7.2 with top-level defaults/sync absent;
- forbidden direct dependencies and Option B/network/protocol crates are absent;
- Cargo.lock and a captured cargo-tree-oa01-features.txt are current;
- rust-version remains 1.97 and all checks use --locked;
- fixture checksum and deterministic regeneration match.

Required commands:

```bash
cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
bash scripts/verify-oa01.sh
```

## 7. Upgrade Policy

Exact pins may be relaxed only after v1 is frozen and CI demonstrates identical official and project golden bytes. Security upgrades are allowed without a wire version bump only when all canonical bytes, IDs, keys, and signatures remain identical. Any changed canonical byte, prefix, domain, number rule, field, or signature behavior requires explicit human approval and normally a new wire version. OA-02 consumes only OA-01 types/validators. OA-04 transports these immutable events without delegating validation to database sync. Neither may reinterpret v1.
