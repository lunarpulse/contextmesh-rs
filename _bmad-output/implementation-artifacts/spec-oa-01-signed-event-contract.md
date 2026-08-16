---
title: 'OA-01 Signed Event Contract and Cryptographic Identity'
type: 'feature'
created: '2026-08-16'
status: 'done'
baseline_commit: '53777ce3668708a5f1b668d25c2a461d04b9985e'
review_loop_iteration: 1
context:
  - '{project-root}/_bmad-output/implementation-artifacts/spec-signed-agent-context-dag.md'
  - '{project-root}/_bmad-output/planning-artifacts/oa-01-dependency-plan.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Independent processes cannot yet derive identical IDs from equivalent JSON or verify event authorship without trusting storage or transport.

**Approach:** Define a strict version-1 JSON envelope, RFC 8785/JCS body canonicalization, domain-separated BLAKE3 event IDs, domain-separated Ed25519 signatures, canonical typed text encodings, bounded validation, and frozen golden vectors. Keep it persistence-independent for OA-02 admission.

## Boundaries & Constraints

**Always:** Use v1 body {version, context, parents, kind, author, payload} and envelope {event_id, body, signature}; reject unknown envelope/body fields and duplicate object keys at every depth; order parents by canonical EventId text and require strict ascending uniqueness; do not normalize Unicode; require finite numbers and safe-range integer-valued numbers; use serde_jcs RFC 8785 for canonical body and wire bytes. Encode 32-byte values as unpadded base64url with prefixes evt1_, ctx1_, and ed25519_; encode 64-byte signatures as sig1_. Require decode/re-encode equality. Derive IDs with BLAKE3 derive-key context org.aaif.contextmesh.event-id.v1 over JCS(body). Sign ASCII org.aaif.contextmesh.signature.v1 followed by NUL and the raw 32-byte event ID; verify with Ed25519 strict verification. Require body.author to match the signing key. Limits: 64 parents; 64-byte kind matching ^[a-z][a-z0-9]*(?:[._-][a-z0-9]+)*$; depth 64; 1 MiB canonical payload; 1,114,112-byte body; 2 MiB raw wire. Return typed errors; never panic on external input.

**Ask First:** Altering any approved field, prefix, domain, algorithm, limit, canonicalization/number rule; changing the contract after vectors exist; accepting legacy encodings; adding protocol semantics/dependencies.

**Never:** Store events (OA-02), inspect parent existence/context, authorize authors/update refs, synchronize nodes (OA-04), execute providers/tools (OA-05), add timestamps/mutable signed metadata, normalize Unicode, serialize private keys/chain-of-thought, or implement Option B.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|---|---|---|---|
| Equivalent JSON | Reordered/whitespace/escape aliases | Same body bytes, ID, and fixed-key signature | Duplicate keys reject, never last-wins |
| Create and verify | Valid body plus matching signing key | Canonical envelope verifies independently | Author/key mismatch is typed failure |
| Tampering | Any body, ID, author, parent, or signature bit changes | Verification fails | No partially verified object returned |
| Boundaries | Limits, Unicode, -0, exponents, nesting | Valid edge matches fixtures | Excess, unsafe integer, depth, or encoding rejects before crypto |
| Malformed wire | Truncated JSON, unknown/duplicate fields, wrong lengths/version | No panic and no accepted event | Stable typed validation category returned |

</frozen-after-approval>

## Code Map

- `Cargo.toml`, `Cargo.lock` -- add only exact plan dependencies; preserve Turso constraints.
- `src/model.rs` -- typed IDs, strict v1 wire model, recursive validation, limits, canonical parse/render.
- `src/crypto.rs` -- OS-random signing identity, BLAKE3 derive-key ID, signing-message construction, Ed25519 sign/strict verify; zeroize temporary seed.
- `src/error.rs` -- non-secret typed parse/canonicalization/limit/identity/signature failures.
- `tests/fixtures/oa01-v1-golden.json` -- fixed-seed canonical bytes, IDs, key, signature, and provenance.
- `tests/oa01_golden.rs`, `tests/oa01_adversarial.rs` -- RFC fixtures, frozen vectors, mutations, malformed JSON, matrix boundaries.
- `scripts/verify-oa01.sh`, `README.md` -- reproducible dependency/fixture audit and public contract/limitations; OA-05 binaries and OA-06 demo remain pending.

## Tasks & Acceptance

**Execution:**
- [x] `Cargo.toml`, `Cargo.lock` -- apply exact dependency plan and record a locked feature audit.
- [x] `src/model.rs`, `src/error.rs` -- implement the strict bounded wire/model contract and canonical encodings.
- [x] `src/crypto.rs` -- implement key generation, domain-separated identity, signing, and strict verification.
- [x] `tests/fixtures/oa01-v1-golden.json`, `tests/oa01_*.rs` -- freeze traceable vectors; cover valid, mutation, malformed, and boundary paths.
- [x] `scripts/verify-oa01.sh`, `README.md` -- automate dependency, vector, format, lint, test, and scope checks; document the frozen v1 contract.

**Acceptance Criteria:**
- Given RFC 8785 and equivalent bodies, when canonicalized, then bytes and IDs match fixtures.
- Given a fixed seed/event, when independently parsed and verified, then ID, author, and signature recompute exactly.
- Given every signed-field mutation and malformed/boundary case, when parsed or verified, then it returns the expected typed error without panic.
- Given the dependency graph, when audited, then approved pins/features hold and later-phase dependencies remain absent.

## Spec Change Log

- 2026-08-16, review loop 1: completed independent cryptographic/protocol,
  hostile-input/resource, requirements/test, and supply-chain/API adversarial
  layers. Hardened verifier isolation and checkout portability; strengthened
  checked-in-vector independence, strict-Ed25519, deepest-duplicate, typed-text,
  single/64-parent, non-finite representation, and exact raw-wire coverage. No
  frozen v1 field, prefix, domain, algorithm, limit, or canonical byte changed.

## Design Notes

The ID excludes event_id and signature, preventing circularity; the author key remains inside the signed body. Parsing accepts insignificant whitespace/order after duplicate detection; rendering is canonical. ContextId is opaque 32 bytes; creation policy is deferred. Event kinds have no semantics in OA-01.

## Verification

**Commands:**
- `cargo fmt --all -- --check` -- no differences.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` -- no warnings.
- `cargo test --workspace --locked` -- all OA-00/OA-01 and doc tests pass.
- `bash scripts/verify-oa01.sh` -- dependency, fixture, matrix, mutation, and scope audits pass.
- `bash scripts/demo.sh` -- still intentionally exits 1 with OA-06 pending.
