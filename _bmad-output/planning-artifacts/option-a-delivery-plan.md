---
title: 'Option A Delivery Plan — Verifiable Distributed Agent History'
type: 'delivery-plan'
created: '2026-08-15'
status: 'approved-for-execution'
owner: 'Product / Engineering'
source_spec: '../implementation-artifacts/spec-signed-agent-context-dag.md'
next_phase: 'Option B — Effective Source-Grounded Context Handoff'
---

# Option A Delivery Plan

## 1. Decision and Product Sequence

Lunarpulse approved Option A on 2026-08-15.

The execution sequence is fixed:

1. **Complete Option A:** a verifiable, append-only, forkable, persistent, and synchronizable history of signed agent events across independent nodes.
2. **Pass every Option A completion gate.** Partial implementation, authored code, or a successful happy-path demo alone does not count as completion.
3. **Only then begin Option B:** source-grounded context selection and effective agent-to-agent handoff.

Option A establishes trustworthy history. It does not claim to select the most relevant, critical, sufficient, or understandable context.

## 2. Option A Outcome

Deliver an embeddable Rust library, CLI/daemon, provider-neutral recording wrapper, authenticated pull synchronization, and reproducible two-node demonstration with these properties:

- deterministic content-derived event identity;
- independently verifiable Ed25519 authorship signatures;
- BLAKE3-addressed Merkle DAG ancestry;
- request, response, error, fork, and explicit merge histories;
- transactional local Turso persistence and compare-and-swap branch refs;
- atomic, idempotent, parent-first synchronization between independent replicas;
- namespaced remote refs that never silently promote remote state;
- deterministic ancestry projection for a caller-selected branch head;
- restart-safe verification and an adversarial test suite;
- precise documentation of what integrity does and does not prove.

## 3. Scope Boundary

### 3.1 Included

- Versioned signed-event and wire contracts.
- Canonical JSON payload identity and golden vectors.
- Ed25519 key generation, signing, and verification with domain separation.
- BLAKE3 content addressing.
- Turso event, context, author-policy, and local/remote ref persistence.
- Event admission validation and typed errors.
- Fork, explicit multi-parent merge, deterministic ancestry projection, and CAS ref movement.
- Atomic bundle export/import.
- Localhost-by-default authenticated HTTP pull transport.
- Provider-neutral invocation and recording contract.
- CLI, daemon, JSON output, JSON Lines demo provider, two-process demo script.
- Unit, integration, adversarial, restart, and convergence tests.
- English documentation, including VPN deployment guidance and protocol-adapter boundaries.

### 3.2 Deferred until Option B

- Semantic or critical context selection.
- Embeddings, vector search, reranking, and context compilers.
- Hierarchical or project summaries.
- Minimal-sufficient-context computation.
- Recipient knowledge or capability modeling.
- Context handoff negotiation and comprehension verification.
- Progressive source-grounded context repair.
- Claims that projected ancestry is relevant or sufficient for a task.

### 3.3 Explicitly out of scope for Option A

- Blockchain or global consensus.
- CRDT frameworks.
- libp2p, peer discovery, NAT traversal, or Internet-wide zero-trust operation.
- TLS or payload encryption supplied by this application.
- Exactly-once provider execution.
- Execution of synchronized tool requests.
- Automatic promotion of remote refs.
- Private model chain-of-thought storage.
- Claims of A2A or ACP protocol compliance.

## 4. Delivery Order

```text
OA-00 Toolchain and repository baseline
  └── OA-01 Signed event contract and cryptography
        └── OA-02 Transactional DAG store and refs
              ├── OA-03 Fork, merge, projection, bundles, verification
              │     └── OA-04 Authenticated multi-node pull sync
              └── OA-05 Provider recording and CLI surface
                    └── OA-06 Two-node demo and documentation
                          └── OA-07 Release verification and completion gate
```

OA-04 and OA-05 may proceed in parallel only after the store interfaces they consume are stable. Changes to the signed wire format after OA-01 golden vectors require explicit approval.

## 5. Work Packages

### OA-00 — Toolchain and Repository Baseline

**Purpose:** Establish a reproducible development and verification environment.

**Work:**

- Make a supported Rust toolchain, `cargo`, `rustfmt`, and `clippy` available.
- Record the Rust edition and minimum supported Rust version in `Cargo.toml` and README.
- Create the single-package crate and initial module/bin/test layout.
- Define standard commands for format, lint, test, and demo execution.
- Pin and compile the stable Turso crate, audit its resolved features/dependencies, and confirm local file/in-memory operation.

**Acceptance:**

- `cargo --version` and `rustc --version` succeed.
- An empty crate builds and the four final verification commands are runnable.
- Dependency resolution is captured in `Cargo.lock`.

**Approved dependency baseline:**

- Use exact stable `turso = 0.7.2` with top-level defaults disabled; do not select the available `0.8.0-pre.4` prerelease.
- Add only a minimal Tokio dev runtime for the asynchronous local-database smoke test.
- Keep Turso `sync`, `fts`, and `mimalloc` features disabled in OA-00.
- Record the locked feature tree. Turso still unconditionally depends on `turso_core`, `turso_sdk_kit`, and `turso_sync_sdk_kit`; this large graph includes build-time bindgen and must be compiled rather than assumed compatible.
- Turso provides local persistence only. OA-04 remains responsible for validated signed-event synchronization; do not substitute Turso Cloud/database replication.

**Current status:** Blocked because `cargo` and `rustc` are not discoverable in the current environment. Source authoring can begin, but Option A cannot complete while this remains unresolved.

---

### OA-01 — Signed Event Contract and Cryptographic Identity

**Primary files:**

- `Cargo.toml`
- `src/model.rs`
- `src/crypto.rs`
- `src/error.rs`
- golden-vector fixtures under `tests/`

**Work:**

- Define versioned event body, signed wire event, IDs, context IDs, author IDs, and refs.
- Define request, response, error, and merge event conventions without coupling the envelope to a provider protocol.
- Canonicalize JSON payloads using the selected JCS implementation/algorithm.
- Define deterministic parent ordering and uniqueness rules.
- Compute event IDs using BLAKE3 over a versioned, domain-separated canonical representation.
- Sign and verify event IDs with Ed25519 and explicit signing-domain separation.
- Define payload, parent, batch, and identifier bounds.
- Produce golden vectors for canonical identity and signature verification.
- Return typed errors; malformed external input must never panic.

**Acceptance:**

- Semantically equivalent JSON objects with different key order produce the same identity.
- Mutating any signed field changes the recomputed ID or invalidates the signature.
- Parent ordering/duplication behavior is deterministic and tested.
- Golden vectors can be read and independently recomputed by tests.
- Unsupported versions and malformed keys/signatures fail with typed errors.

**Frozen decision:** Changing this wire contract after golden vectors exist requires explicit human approval.

---

### OA-02 — Transactional Turso DAG Store and Ref Semantics

**Primary file:** `src/store.rs`

**Work:**

- Define and migrate the Turso schema for events, parent edges, authorized authors, local refs, remote refs, and required metadata.
- Store immutable event bytes or an equivalent canonical representation sufficient for exact re-verification.
- Validate before admission:
  - event ID;
  - signature;
  - version and bounds;
  - signer authorization;
  - context membership;
  - parent existence;
  - parent uniqueness and canonical ordering;
  - cross-context parent prohibition.
- Admit events and move local refs transactionally.
- Implement compare-and-swap local ref updates with a typed stale-head conflict carrying the current head.
- Preserve local and remote ref namespaces and prohibit silent remote promotion.
- Make repeated event admission idempotent only when the existing canonical event is identical.

**Acceptance:**

- Every invalid-admission case leaves no active event, edge, or ref mutation.
- Concurrent/stale writers cannot silently overwrite a branch head.
- Reopening the Turso database file preserves events, policies, and refs.
- Existing immutable events cannot be updated through public APIs.
- Distinct local and remote refs survive restart.

---

### OA-03 — DAG Operations, Bundles, and Full Verification

**Primary file:** `src/store.rs`, with supporting model/error types.

**Work:**

- Create contexts/genesis events according to the approved workspace policy.
- Append linked events to a ref using expected-head CAS semantics.
- Create forks without mutating their common ancestry.
- Create explicit multi-parent merge events.
- Reject absent, duplicate, or cross-context merge parents.
- Project ancestry deterministically, emitting each ancestor once.
- Export bounded bundles with parent-first ordering and advertised refs.
- Import bundles atomically and idempotently.
- Verify all stored event IDs, signatures, parent edges, contexts, and refs after restart.
- Report corruption; never repair or ignore it silently.

**Acceptance:**

- A two-branch fork remains visible after merge.
- Projection of a merged head includes every reachable ancestor once in deterministic order.
- Any invalid event in an imported bundle rolls back every event and ref update in that batch.
- A repeated valid import adds zero new events and makes no unintended ref movement.
- Full-store verification detects tampering and dangling/invalid refs.

---

### OA-04 — Authenticated Multi-Node Pull Synchronization

**Primary files:**

- `src/sync.rs`
- `src/http.rs`

**Work:**

- Implement a bounded read API using Axum.
- Bind to localhost by default.
- Require bearer authentication and avoid logging secret tokens.
- Expose only the data required for bounded anti-entropy pull and advertised refs.
- Implement a Reqwest pull client.
- Determine missing history without claiming global consensus.
- Import received events through the same validation/admission path as local data.
- Record peer refs under `remote/<peer>/...`.
- Ensure network retries are safe through idempotent import.
- Apply request, response, event-count, and payload-size limits.

**Acceptance:**

- Node B can pull valid missing history from Node A.
- Invalid remote history causes atomic rejection and does not update remote refs.
- A second pull after convergence transfers/imports no new event.
- Synchronization never moves a local branch ref.
- Authentication failure and bound violations return controlled errors without mutation or panic.
- Documentation states that bearer authentication over plain HTTP is intended for localhost/VPN or an external TLS tunnel, not an untrusted public network.

---

### OA-05 — Provider Recording Boundary and CLI

**Primary files:**

- `src/provider.rs`
- `src/bin/contextmesh.rs`
- `src/bin/demo_agent.rs`

**Work:**

- Define an object-safe provider contract accepting opaque JSON input and deterministic ancestry for an explicitly selected branch head.
- Record the request before provider execution.
- Record a linked response or typed error event after execution.
- Expose the exact context head/package metadata used by the invocation.
- Do not claim exactly-once execution; document the crash window and retry semantics.
- Implement CLI commands for key generation, context creation, append, branch, merge, show, verify, serve, and sync.
- Use stable structured JSON output for automation.
- Implement a JSON Lines echo/demo provider without A2A/ACP compliance claims.

**Acceptance:**

- Successful invocation produces linked request and response events.
- Failed invocation produces a linked error event and returns the failure.
- The provider receives deterministic ancestry for the caller-selected head.
- CLI commands expose typed conflicts and validation errors with non-zero exits.
- Restarting the CLI/daemon against the same database preserves verifiability.

**Terminology constraint:** In Option A, “selected context” means caller-selected branch ancestry only. It never means semantic relevance or sufficiency.

---

### OA-06 — Reproducible Demo and Documentation

**Primary files:**

- `scripts/demo.sh`
- `README.md`
- test fixtures and example configuration

**Demo story:**

1. Generate independent identities for Agent A and Agent B.
2. Create a common project/context origin.
3. Start Node A and Node B with independent Turso database files.
4. Synchronize their common history.
5. Record a request/response branch on Node A.
6. Record a different request/response branch on Node B.
7. Synchronize both directions while preserving separate local and remote refs.
8. Explicitly create a multi-parent merge.
9. Synchronize again.
10. Stop and restart both processes.
11. Verify every stored event and signature.
12. Confirm identical merged ancestry while local/remote ref identities remain distinct.
13. Synchronize once more and demonstrate zero newly imported events.
14. Tamper with an exported bundle and demonstrate atomic rejection.

**Documentation must cover:**

- architecture and data flow;
- library and CLI usage;
- event identity and signature verification;
- ref, fork, merge, bundle, and synchronization semantics;
- threat model and trust boundaries;
- localhost/VPN guidance;
- integrity versus truth, authorization, confidentiality, availability, and consensus;
- provider adapter contract;
- concrete but non-compliant mapping notes for future A2A/ACP adapters;
- Option A limitations and explicit Option B deferral.

**Acceptance:**

- The demo starts from clean directories and succeeds reproducibly.
- Every process is cleaned up on normal completion and failure.
- Demo output clearly identifies each stage and final verification result.
- README commands correspond to tested CLI behavior.

---

### OA-07 — Release Verification and Completion Gate

**Work:**

- Run all test layers against a clean working state.
- Review every product/security claim against demonstrated behavior.
- Confirm no deferred Option B feature has entered the critical path.
- Record command outputs and unresolved limitations.
- Assign an Option A completion verdict.

**Required commands:**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/demo.sh
```

**Completion verdict:**

Option A is complete only if:

- all four commands exit successfully;
- completion gates A1–A8 below pass;
- no open issue contradicts the approved Always/Never constraints;
- the two-node restart/convergence/tamper demo succeeds;
- documentation claims no property that is absent from the implementation;
- verification evidence is recorded.

If any condition fails, status remains `incomplete` and Option B stays blocked.

## 6. Completion Gates

### A1 — Deterministic Cryptographic Identity

- Canonical-equivalent payloads produce identical event IDs.
- Parent ordering is deterministic.
- Any signed-field mutation is detected.
- Golden vectors are reproducible.
- Signing and hashing use explicit version/domain separation.

### A2 — Admission Integrity

- Invalid IDs, signatures, authors, versions, contexts, parents, sizes, or merge shapes are rejected.
- Failure causes no partial event, edge, or ref mutation.
- External malformed input does not panic.

### A3 — DAG and Ref Behavior

- Request/response/error chains work.
- Forks preserve common ancestry.
- Explicit merges preserve all parent histories.
- Projection emits each ancestor once in deterministic order.
- CAS detects stale heads.

### A4 — Persistence and Recovery

- Events, policies, and refs survive restart.
- Full verification succeeds for valid data after restart.
- Corruption is surfaced rather than ignored.
- Duplicate imports are safe and idempotent.

### A5 — Multi-Node Synchronization

- Independent nodes exchange missing events.
- Import is parent-first and atomic.
- Remote refs are namespaced.
- Local refs never move implicitly.
- Repeated pull imports zero new events after convergence.
- Fork, merge, restart, and re-sync produce identical verified merged ancestry.

### A6 — Provider Recording

- Request is recorded before invocation.
- Response or error is linked afterward.
- Exact selected branch head/ancestry is identifiable.
- Provider failure remains visible in history.
- No exactly-once claim is made.

### A7 — Security and Claim Boundaries

- Localhost binding is the default.
- Bearer authentication is required.
- Input and batch bounds are enforced.
- Integrity is distinguished from truth, confidentiality, authorization, availability, and consensus.
- No unsupported blockchain, TLS, encryption, A2A, or ACP claim is present.

### A8 — Executable Evidence

- Format, lint, tests, and demo all pass from documented commands.
- The demo proves convergence, persistence, explicit merge, idempotence, and tamper rejection.
- Results are recorded before declaring completion.

## 7. Test Strategy

### Unit tests

- canonicalization and IDs;
- signature success/failure;
- version, identifier, payload, and parent bounds;
- typed error behavior;
- deterministic parent and projection ordering.

### Transactional integration tests

- valid admission;
- each invalid-admission rollback path;
- CAS race/stale-head conflict;
- fork and merge behavior;
- restart and full verification;
- atomic bundle rollback;
- duplicate/idempotent import.

### Transport integration tests

- authorized and unauthorized pull;
- bounded requests/responses;
- valid convergence;
- invalid remote bundle rejection;
- local/remote ref separation;
- retry after convergence.

### Provider integration tests

- successful response recording;
- provider error recording;
- stable selected ancestry;
- process restart;
- documented non-exactly-once failure boundary.

### End-to-end test

`scripts/demo.sh` executes the full two-node fork, sync, merge, restart, convergence, idempotence, and tamper-rejection story.

## 8. Key Risks and Mitigations

| Risk | Effect | Mitigation / Evidence |
|---|---|---|
| Canonicalization differs across processes | Same event gets different IDs | Freeze algorithm; golden vectors; cross-process tests |
| Signed wire format changes casually | Existing history becomes unverifiable | Version envelope; approval required after vectors |
| Parent ordering is ambiguous | Merge identity differs by producer | Canonical order and uniqueness validation |
| Partial bundle import | Replica contains unverifiable history | Single Turso transaction; adversarial rollback tests |
| Remote refs overwrite local work | Silent history loss or confusion | Namespace remote refs; no automatic promotion |
| Retry duplicates state | Unbounded duplicate events or ref drift | Content IDs and idempotent import |
| Plain HTTP leaks bearer token | Unauthorized read over public network | Localhost default; VPN/TLS tunnel guidance; no public-network claim |
| Provider crashes between execution and response record | Possible duplicate external side effect on retry | Document crash window; no exactly-once claim; use correlation IDs |
| Turso writer contention | Stale or failed branch movement | Transactions, CAS conflicts, bounded retry policy |
| Turso dependency/API churn or unexpectedly heavy builds | Baseline fails or upgrades destabilize storage | Exact stable version, committed lockfile, defaults disabled, recorded `cargo tree`, compiled smoke test |
| Scope creep into Option B | Option A never reaches completion | Deferred list; hard A8 gate; separate Option B specification |
| Missing Rust toolchain | No executable evidence | Resolve OA-00 before claiming progress beyond authored source |

## 9. Decision and Change Control

**Storage decision amended 2026-08-15:** Lunarpulse selected stable embedded Turso instead of SQLite/rusqlite. Turso Cloud sync is not approved.

The following require explicit approval before Option A completion:

- adding A2A/ACP dependencies;
- changing the signed event wire format after golden vectors exist;
- adding system services or containers;
- expanding beyond the single-workspace MVP authorization policy;
- weakening any approved Always/Never constraint;
- pulling semantic context selection or summarization into Option A.

Normal implementation detail changes that preserve the specification, wire contract, security boundaries, and acceptance criteria do not require product reapproval.

## 10. Option B Unlock Gate

Option B planning begins only after OA-07 records `complete` for Option A.

At that point, a new specification will address:

- agent experience receipts;
- task-conditioned source selection;
- dependency closure and critical-risk coverage;
- recipient-known-history delta;
- state-bound handoff validity;
- omission challenge and uncertainty;
- progressive context repair;
- comprehension and downstream task-performance evaluation.

Option B will consume Option A events as verifiable evidence. It will remain a derived/selection layer rather than altering immutable Option A history.

## 11. Immediate Next Actions

1. Resolve the Rust toolchain availability blocker.
2. Start OA-00 and create the crate skeleton.
3. Execute OA-01 before store or transport work.
4. Freeze golden vectors before parallelizing OA-04 and OA-05.
5. Maintain implementation progress against the work-package and A1–A8 gates.
