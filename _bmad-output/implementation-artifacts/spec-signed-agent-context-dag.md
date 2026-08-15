---
title: 'Signed Agent Context DAG Library and Multi-Node Demo'
type: 'feature'
created: '2026-08-15'
status: 'approved'
approved: '2026-08-15'
approved_by: 'Lunarpulse'
phase: 'Option A — Verifiable Distributed History'
review_loop_iteration: 0
context: []
delivery_plan: '../planning-artifacts/option-a-delivery-plan.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Coding-agent processes need a small, usable way to persist request/response provenance, fork histories, merge selected work, and exchange verified context without a central database. Existing blockchain machinery is too heavy, while direct A2A/ACP integration would prematurely couple the data model to one evolving protocol.

**Approach:** Build an embeddable Rust library that stores Ed25519-signed events in a BLAKE3-addressed Merkle DAG backed by local embedded Turso database files, plus an authenticated HTTP anti-entropy transport, a transport-neutral provider wrapper, CLI daemon, and reproducible two-node demo. Treat protocol adapters as thin mappings onto stable library types; demonstrate a JSON/stdio-shaped agent integration without claiming A2A or ACP compliance.

**Product sequence:** Complete Option A, the verifiable distributed-history substrate, before specifying or implementing Option B, the effective source-grounded agent-handoff layer. Option A establishes trustworthy history; it does not claim semantic context relevance, criticality, sufficiency, summarization, or recipient comprehension.

## Boundaries & Constraints

**Always:** Use deterministic event IDs independent of JSON object key order; sign the event ID with domain separation; validate IDs, signatures, context, parent ordering/uniqueness/existence, size limits, and signer authorization before admission. Keep branches as local compare-and-swap refs, represent merges as multi-parent events, preserve remote refs without silently moving local refs, and derive context in deterministic ancestor order. Bind the daemon to localhost by default, require a bearer token, keep each node in an independent local Turso database file, make imports atomic/idempotent, and clearly distinguish integrity from trust/confidentiality.

**Ask First:** Adding a protocol-specific A2A/ACP dependency, changing the event wire format after golden vectors exist, introducing system services or containers, expanding beyond the single-workspace MVP security policy, weakening an Always/Never constraint, or moving an Option B selection feature into Option A.

**Never:** Claim blockchain, consensus, exactly-once provider execution, TLS, payload encryption, A2A compliance, ACP compliance, semantic relevance, minimum-sufficient context, or recipient understanding. Do not use Turso database replication as a substitute for the signed event anti-entropy protocol. Do not add libp2p, CRDT frameworks, service discovery, mutable event records, automatic remote-branch promotion, execution of synced tool requests, semantic/vector context selection, hierarchical summarization, or storage of private model chain-of-thought.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|---------------|----------------------------|----------------|
| Signed append | Authorized key, context, expected branch head, JSON payload | Persist one verifiable event and atomically advance the local ref | Stale head returns a typed conflict with current head |
| Fork and merge | Two refs diverge from one ancestor, then merge | Multi-parent event preserves both histories; projection contains every ancestor once | Missing/cross-context/duplicate parent is rejected |
| Peer synchronization | Node B pulls a signed bundle and refs from Node A | B validates and imports parent-first, records `remote/A/*`, and repeated pull transfers no new events | Any invalid event rolls back the batch and remote refs remain unchanged |
| Untrusted event | Mutated ID/signature, unauthorized author, oversized payload, or absent parent | No active event or ref is created | Return typed validation error without panic |
| Provider invocation | Caller-selected branch ancestry plus opaque JSON input | Record request before invocation and response/error afterward; provider receives deterministic ancestry | Provider failure is recorded as an error event and returned |
| Restart | Reopen a populated Turso replica | Events, local refs, and remote refs remain verifiable | Corruption is reported by `verify`, not ignored |

</frozen-after-approval>

## Approval Record

- **Decision:** Approved by Lunarpulse on 2026-08-15.
- **Sequence:** Option A must complete before Option B planning or implementation starts.
- **Option A interpretation:** “Selected context” means only the deterministic ancestry of a branch head explicitly selected by the caller. It does not mean semantic selection.
- **Option B boundary:** Semantic retrieval, source-grounded handoff, recipient-knowledge modeling, critical-risk coverage, progressive context repair, and comprehension verification remain deferred.
- **Completion rule:** Option A is complete only when gates A1–A8 pass and the required verification commands succeed.
- **Execution plan:** `_bmad-output/planning-artifacts/option-a-delivery-plan.md`.

## Code Map

- `Cargo.toml` -- new single-package Rust crate; no existing application code or project conventions are available to reuse.
- `src/model.rs`, `src/crypto.rs`, `src/error.rs` -- versioned event/wire types, canonical JCS payload hashing, Ed25519 signing, typed failures.
- `src/store.rs` -- Turso schema and transactional DAG/ref operations, history projection, bundle export/import, and full-store verification.
- `src/sync.rs`, `src/http.rs` -- authenticated Axum read API and Reqwest pull client; protocol-specific adapters remain outside the core.
- `src/provider.rs` -- object-safe provider contract, ancestry envelope, and recording wrapper for future A2A/ACP/agent bridges.
- `src/bin/contextmesh.rs` -- key generation, create/append/branch/merge/show/verify/serve/sync commands and JSON output.
- `src/bin/demo_agent.rs` -- JSON Lines echo provider demonstrating how an external coding-agent adapter consumes and records caller-selected shared ancestry.
- `tests/` and `scripts/demo.sh` -- adversarial library tests and two-process convergence/persistence demonstration.
- `README.md` -- architecture, threat boundaries, APIs, CLI walkthrough, VPN guidance, Option B boundary, and concrete A2A/ACP mapping notes.

## Tasks & Acceptance

**Execution:**

- [ ] `Cargo.toml`, `src/model.rs`, `src/crypto.rs`, `src/error.rs` -- establish the versioned signed-event contract and canonical golden vectors.
- [ ] `src/store.rs` -- implement transactional Turso admission, refs, forks, merges, deterministic projection, verification, and atomic bundles.
- [ ] `src/sync.rs`, `src/http.rs` -- implement bounded bearer-authenticated pull synchronization and remote-ref tracking.
- [ ] `src/provider.rs`, `src/bin/demo_agent.rs` -- expose and exercise the provider-neutral ancestry/recording boundary.
- [ ] `src/bin/contextmesh.rs`, `scripts/demo.sh`, `README.md` -- provide an operable two-node daemon/CLI demo and integration guidance.
- [ ] `tests/` -- cover canonical identity, tampering, authorization, parent validation, CAS races, forks/merges, rollback, restart, provider failures, and sync convergence/idempotence.

**Acceptance Criteria:**

- Given two clean node directories and authorized identities, when the demo forks work on both nodes, synchronizes, explicitly merges, restarts, and synchronizes again, then both replicas verify the same merged ancestry while retaining distinct local and remote refs.
- Given any admitted event, when another process reads it from Turso or HTTP, then it can recompute the same ID and verify its author signature without trusting the sender.
- Given an arbitrary external provider implementing the documented trait or JSON Lines contract, when invoked on a branch, then its exact caller-selected ancestry and resulting response/error are represented by linked immutable events.
- Given any invalid event in an imported bundle, when import is attempted, then the entire event/ref batch is rolled back and no partial active history remains.
- Given a converged replica pair, when synchronization repeats, then no event is newly imported and no local ref moves implicitly.

## Completion Gates

### A1 — Deterministic Cryptographic Identity

- Canonical-equivalent payloads produce identical IDs.
- Parent ordering is deterministic and duplicate-free.
- Any signed-field mutation is detected.
- Golden vectors are reproducible.
- Signing and hashing use explicit version/domain separation.

### A2 — Admission Integrity

- Invalid IDs, signatures, authors, versions, contexts, parents, sizes, and merge shapes are rejected.
- Validation failure causes no partial event, edge, or ref mutation.
- Malformed external input does not panic.

### A3 — DAG and Ref Behavior

- Request/response/error chains, forks, and explicit merges behave as specified.
- Projection emits each ancestor once in deterministic order.
- Compare-and-swap detects stale branch heads.

### A4 — Persistence and Recovery

- Events, authorization policy, and refs survive restart.
- Full verification succeeds for valid data and reports corruption.
- Repeated imports are idempotent.

### A5 — Multi-Node Synchronization

- Independent replicas exchange missing events parent-first and atomically.
- Remote refs remain namespaced and local refs never move implicitly.
- Repeated pull after convergence imports zero events.
- Fork, merge, restart, and re-sync produce identical verified merged ancestry.

### A6 — Provider Recording

- Request is recorded before invocation.
- A linked response or error is recorded afterward.
- The exact caller-selected branch head and deterministic ancestry are identifiable.
- No exactly-once execution claim is made.

### A7 — Security and Claim Boundaries

- Localhost binding is the default and bearer authentication is required.
- Event, request, response, and batch bounds are enforced.
- Integrity is distinguished from truth, authorization, confidentiality, availability, and consensus.
- No unsupported blockchain, TLS, encryption, A2A, ACP, or semantic-sufficiency claim appears.

### A8 — Executable Evidence

All commands must succeed:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/demo.sh
```

The demo must prove fork, bidirectional synchronization, explicit merge, restart persistence, convergence, idempotent repeat synchronization, and atomic tamper rejection.

## Spec Change Log

- **2026-08-15 — Approval:** Lunarpulse approved Option A and fixed the sequence as Option A first, Option B only after Option A completes. Clarified that deterministic ancestry projection is not semantic context selection and added hard completion gates A1–A8.
- **2026-08-15 — Storage renegotiation:** At Lunarpulse’s direction, replaced the SQLite/rusqlite assumption with local embedded Turso; Turso Cloud sync remains outside Option A and cannot replace event-level validation and anti-entropy.

## Design Notes

A branch name is a mutable local convenience pointer, never consensus state. Sync exchanges immutable events and advertised refs; imported refs are namespaced under the peer. A future A2A task/message or ACP session/update adapter should translate protocol payloads into `agent.request`, `agent.response`, `agent.error`, and optional merge events while retaining upstream IDs in payload metadata.

Option B must consume Option A events as verifiable evidence through derived projections. It must not mutate the approved immutable event core merely to add selection semantics.

## Verification

**Commands:**

- `cargo fmt --all -- --check` -- expected: no formatting differences.
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: no warnings.
- `cargo test --workspace` -- expected: all unit and integration tests pass.
- `bash scripts/demo.sh` -- expected: two live daemon processes exchange a fork, merge it, restart, reject tampering atomically, and report identical verified ancestry.

**Known pre-execution blocker:** At approval time, `cargo` and `rustc` are not discoverable in the current environment. Source implementation may be authored, but A8 and therefore Option A completion require a working Rust toolchain.
