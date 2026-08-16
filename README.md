# contextmesh

contextmesh currently implements **OA-03**: the frozen OA-01 signed-event
contract, OA-02 transactional local store, and semantic-free DAG operations,
deterministic projection, strict bounded Bundle v1 transfer, and full-store
integrity verification. OA-00's Rust 1.97/Turso baseline remains intact.

## Toolchain and verification

The project pins Rust **1.97.0**, edition **2024**, rustfmt, and Clippy in
rust-toolchain.toml. Install or refresh the user-local toolchain without root:

    bash scripts/bootstrap-rust.sh
    . "$HOME/.cargo/env"

Verify the current OA-03 state from the repository root:

    cargo build --workspace --locked
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo test --workspace --locked
    bash scripts/verify-oa01.sh
    bash scripts/verify-oa02.sh
    bash scripts/verify-oa03.sh

The verifiers check exact dependencies/features, locked feature graphs, OA-01
fixture stability, OA-02 schema/admission/ref regressions, OA-03 graph/bundle/
verification evidence, deferred-module boundaries, every quality command, and
the expected OA-06 failure sentinel.

## Transactional local store

Store::open creates or verifies schema v1 in a local embedded Turso database.
Every connection enables and confirms foreign-key enforcement. Writes use
explicit IMMEDIATE transactions and explicit commit or rollback.

A context is provisioned with an exact expected context.genesis event and a
sorted local author allowlist. The allowlist is append-only in Option A;
revocation and broader workspace policy are deferred. Authorization permits
local admission but proves neither truth nor relevance.

Admission re-verifies OA-01 canonical wire, author policy, and every parent.
Parents must already exist in the same context. The canonical envelope is the
authoritative stored record; denormalized columns and ordered edges are checked
against it. Event and edge rows are protected by database triggers.

Local refs move only through explicit compare-and-swap in the event-admission
transaction. Stale writers receive the current optional head and leave no event
or edge behind. Remote-tracking refs use a separate peer namespace and never move local refs.

## DAG operations, deterministic projection, and Bundle v1

OA-03 atomically creates contexts with one signed zero-parent genesis event,
appends through expected-head CAS, creates explicit fork branches, and signs
2-to-64-parent context.merge events. Merge parents are canonically sorted and
must include the current target head. No helper selects a branch or infers
relevance.

Projection is iterative, cycle-detecting, parent-first, and unique across shared
ancestors. It depends only on explicit immutable heads and canonical parent
order, not SQL row order. Defaults and hard maxima are 100,000 events and 64 MiB
of exact canonical event envelopes.

Bundle v1 is strict canonical JSON containing one context, at most 1,024
parent-first verified events, and at most 256 sorted local advertised refs under
a 16 MiB bound. Advertised refs are unsigned peer claims. Import rechecks local
provisioning, authorization, genesis, parents, collisions, and ref targets in
one IMMEDIATE transaction; it updates only the supplied remote peer namespace.
Repeated import inserts zero, and bundle import never moves a local ref.

Store::verify_full scans canonical wire, row columns, exact edges, context and
genesis invariants, append-only author policy, cycles, names, refs, and bounded
projection in one read snapshot. Findings are bounded and non-secret. It reports
corruption and never repairs it.

These are integrity and caller-selected ancestry facilities, not truth,
semantic relevance, confidentiality, availability, authorization consensus, or
semantic context selection.

## Frozen signed-event v1 contract

A signed event has this logical wire form. Rendering always uses RFC 8785/JCS:

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

- EventId, ContextId, and AuthorId decode to exactly 32 bytes.
- EventSignature decodes to exactly 64 bytes.
- Text uses URL-safe base64 without padding and must decode/re-encode exactly.
- Parents are strictly increasing by canonical EventId text and unique.
- Kinds are 1-64 ASCII bytes under the frozen lowercase segment grammar.
- Unicode is preserved without normalization.
- Duplicate keys at every JSON depth, unknown envelope/body fields, BOMs, and
  trailing JSON are rejected.

### Identity and signature

Canonical body bytes are hashed with BLAKE3 derive-key mode using:

    org.aaif.contextmesh.event-id.v1

The Ed25519 signing message is:

    ASCII("org.aaif.contextmesh.signature.v1") || NUL || raw_event_id[32]

Verification recomputes the ID and uses Ed25519 strict verification. A body can
be signed only when its author equals the signing identity's public key.
Private keys are neither serializable nor exposed. Production identities use
fallible OS entropy; checked-in fixed seeds are test-only.

### Limits

| Item | v1 limit |
|---|---:|
| Raw wire JSON | 2,097,152 bytes |
| Canonical payload | 1,048,576 bytes |
| Canonical body | 1,114,112 bytes |
| Parents | 64 |
| Payload depth | 64 |
| Kind | 64 bytes |
| Integer-valued JSON number | +/-9,007,199,254,740,991 |

Larger exact integers must be strings. Public failures use non-secret typed
categories and malformed input must not panic.

## Golden vectors and dependencies

tests/fixtures/oa01-v1-golden.json records the deterministic OA-01 body,
envelope, ID, author, signing message, and signature. Changing a v1 field,
prefix, domain, limit, canonical byte, ID, or signature requires approval and
normally a new version.

OA-03 adds no dependency or feature. It preserves exact Turso 0.7.2 and Tokio
1.53.1 pins and the captured cargo-tree-oa02-features.txt graph. The frozen
canonical Bundle v1 fixture is tests/fixtures/oa03-bundle-v1-golden.json.

## Deferred scope

- OA-04: authenticated HTTP anti-entropy synchronization.
- OA-05: provider recording, key custody, and real CLI commands.
- OA-06: two-node demonstration; scripts/demo.sh intentionally exits 1.
- OA-07: release evidence and Option A completion verdict.
- Option B: semantic context selection and handoff.

The placeholder binaries still exit unsuccessfully so automation cannot mistake
future OA-05 behavior for an implemented command. Option B remains blocked until
OA-07 records Option A complete with direct A1-A8 evidence.
