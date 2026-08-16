---
title: 'OA-02 through OA-07 Detailed Execution Plan'
type: 'execution-plan'
created: '2026-08-16'
status: 'approved-for-execution'
approved: '2026-08-16'
approved_by: 'Lunarpulse'
baseline_commit: 'f61c4f0d147544c4011b2bb8b8094943e196c883'
source_spec: '../implementation-artifacts/spec-signed-agent-context-dag.md'
source_delivery_plan: './option-a-delivery-plan.md'
option_b_gate: 'Blocked until OA-07 records Option A complete'
---

# OA-02 through OA-07 Detailed Execution Plan

## Approval record

Lunarpulse approved this execution plan, its minimum test matrix, and decisions D-02-01, D-05-01, and D-04-01 on 2026-08-16.

- **D-02-01:** Approved exactly as documented: one provisioned genesis, explicit local context trust, and an append-only local author allowlist for Option A. Revocation remains outside Option A.
- **D-05-01:** Approved exactly as documented: opaque local seed-file custody with atomic creation, restrictive permissions, no public/wire exposure, no Turso/bundle/sync inclusion, and no encryption-at-rest claim.
- **D-04-01:** Approved as the dependency target set and mandatory selection procedure. The listed versions and minimal feature direction are authorized for Rust 1.97 probes. They become frozen pins only after successful recorded probes. Failed candidates or substitute versions/features require further approval.

This approval authorizes OA-02 specification and execution under the package gates below. It does not declare an unexecuted probe successful, change OA-01 wire bytes, complete Option A, or unlock Option B.

## 1. Purpose and non-negotiable boundaries

This plan turns the approved Option A delivery plan into implementation-ready work packages after OA-01. It does not modify the frozen OA-01 v1 event envelope, canonicalization, typed encodings, domains, algorithms, limits, IDs, or golden vectors.

Non-negotiable boundaries:

- Option A records and transports verifiable history; it does not select semantically relevant or sufficient context.
- Local embedded Turso is persistence only. Turso Cloud replication is not the synchronization protocol.
- Events are immutable. Mutable refs are local convenience pointers, not consensus state.
- Every external event passes OA-01 verification before admission.
- Local refs move only by explicit compare-and-swap. Sync never promotes a remote ref.
- Bundle import is bounded, parent-first, atomic per bundle, and idempotent.
- HTTP binds to loopback by default and requires a bearer token. Plain HTTP is only for localhost, VPN, or an external TLS tunnel.
- Synchronized tool requests are data and are never executed automatically.
- Private chain-of-thought is never stored.
- Option B remains blocked until OA-07 records Option A complete after A1-A8 pass.

## 2. Blocking decisions to approve

### D-02-01 — Context bootstrap and authorization policy — APPROVED

Recommended decision:

- A context has exactly one signed zero-parent context.genesis event.
- ContextId is generated from 32 bytes of fallible OS entropy.
- Membership and author authorization are local operator policy, not replicated consensus.
- The allowlist is append-only in Option A. Revocation is deferred because v1 has no signed policy chronology or trusted time for historical authorization.
- A joining node is explicitly provisioned with context ID, expected genesis EventId, and initial author allowlist.
- The first accepted event must match the provisioned genesis exactly. Any other zero-parent event is rejected.
- Adding an author is an explicit local administrative action and permits admission; it does not make the author's statements true.

**Decision:** Approved by Lunarpulse on 2026-08-16. These rules close the workspace-policy gap. Author revocation, trusted policy chronology, or broader multi-workspace authorization require new approval.

### D-05-01 — Persistent signing-key custody — APPROVED

OA-01 does not expose or serialize private keys, but a restart-safe CLI needs persistent custody.

Recommended decision:

- Add an opaque key-file API that atomically writes a 32-byte Ed25519 seed to a caller-selected local path.
- Never return, print, log, JSON-encode, or store the seed in Turso.
- On Unix create mode 0600; reject group/other access unless an explicit repair command is used.
- Refuse symlink targets; use same-directory temporary file, fsync, rename, and directory fsync where supported.
- Make no encryption-at-rest claim; users rely on OS or disk encryption.
- Key and bearer-token files are never bundled or synchronized.

**Decision:** Approved by Lunarpulse on 2026-08-16. OA-01's boundary is interpreted as no private material in public values, wire formats, logs, JSON, Turso, bundles, or synchronization. The narrowly scoped opaque local seed file is authorized for OA-05 restart-safe signing and changes no OA-01 bytes or cryptographic behavior.

### D-04-01 — Exact downstream dependency set — APPROVED FOR MANDATORY PREFLIGHT

The architecture names Axum and Reqwest but does not approve versions/features; Clap is an implementation choice.

Approved target candidates and feature direction; effective pins still require successful recorded probes:

- Tokio 1.53.1 with package-minimal runtime, process, signal, sync, time, net, and macro features as actually needed.
- Axum 0.8.9 with defaults off and HTTP/1, JSON, and Tokio only.
- Reqwest 0.13.4 with defaults off and only plain HTTP/1 and JSON support.
- Clap 4.6.6 with minimized derive, std, help, usage, and error-context features.
- A direct constant-time comparison crate only if no suitable audited primitive is already available.

**Decision:** Approved by Lunarpulse on 2026-08-16 as a two-stage selection. Before OA-04/OA-05 manifest changes, run the disposable Rust 1.97 probes, inspect MSRV, execute representative paths, and record Cargo metadata and the full feature tree. Successful targets become the exact minimal pins recorded in the OA-04 dependency plan. Reject TLS, cookies, compression, HTTP/2, multipart, proxy discovery, and agent-protocol dependencies. A failed target or any substitute version/feature set must return for approval and may not be selected silently.

## 3. Execution graph

    f61c4f0 OA-01 frozen event contract
      |
      +-- D-02-01 approval
      +-- OA-02 transactional store, admission, refs
            |
            +-- OA-03 DAG, projection, bundles, full verification
            |     |
            |     +-- D-04-01 dependency approval
            |     +-- OA-04 authenticated pull synchronization
            |
            +-- D-05-01 key-custody approval
            +-- OA-05 provider recording and CLI
                        |
                        +-- integrate OA-03 bundle/show/verify APIs

    OA-04 and OA-05 complete
      +-- OA-06 two-node demo and documentation
            +-- OA-07 release verification and A1-A8 verdict
                  +-- Option B unlock only when verdict is complete

OA-04 and OA-05 may run in parallel only after shared store APIs, error taxonomy, ref grammar, bundle types, Tokio features, and Cargo ownership are frozen. Parallel workers must own disjoint files.

## 4. Shared contracts frozen by OA-03

### 4.1 Names

- Local branch and peer names: 1-64 ASCII bytes under the same lowercase segmented grammar as event kind.
- Public refs render as local/NAME or remote/PEER/NAME.
- Storage uses separate local and remote tables; public strings are not parsed into a shared namespace.
- Reject path separators, reserved prefixes, noncanonical names, and any name used as SQL or a path.

### 4.2 Error categories

Keep ContractError for OA-01. Add non-secret package errors for:

- database unavailable, corrupt, migration failure, newer schema;
- context unknown, pending, exists, genesis mismatch;
- unauthorized author;
- missing or cross-context parent;
- same EventId with different canonical bytes;
- invalid/missing/existing/stale ref;
- projection/bundle/request/response limit;
- malformed/order/version/ref-invalid bundle;
- authentication, transport, protocol, timeout;
- provider failure and post-execution conflict;
- key permission, format, and I/O;
- CLI usage and internal failure.

Only safe identifiers and a current ref head may be returned. Do not expose SQL, tokens, seed bytes, arbitrary payloads, provider stderr, or sensitive paths.

### 4.3 Proposed default bounds

| Bound | Proposed default | Owner |
|---|---:|---|
| Branch/peer name | 64 ASCII bytes | OA-02 |
| Projection events | 100,000 | OA-03 |
| Projection wire bytes | 64 MiB | OA-03 |
| Bundle events | 1,024 | OA-03 |
| Bundle canonical bytes | 16 MiB | OA-03 |
| Bundle advertised refs | 256 | OA-03 |
| Pull request body | 64 KiB | OA-04 |
| Concurrent HTTP requests | 16 | OA-04 |
| Connect/request timeout | 5/30 seconds | OA-04 |
| Provider input/output | OA-01 payload limit each | OA-05 |
| JSONL line | 2 MiB | OA-05 |

Enforce limits before unbounded allocation or mutation. Graph traversal is iterative.

# OA-02 — Transactional Turso DAG Store and Ref Semantics

## 5. Objective and excluded scope

OA-02 stores canonical OA-01 events, validates local policy and parent/context constraints, and atomically updates local refs. It does not implement projection, merge helpers, bundles, HTTP, providers, or production CLI behavior.

Exit state: A2 and the persistence/ref subset of A3/A4 are demonstrated; the store API is stable for OA-03/OA-05; OA-01 vectors are unchanged.

## 6. File plan

- src/store.rs: documented public facade and core types.
- src/store/schema.rs: schema version, migration, constraints, triggers.
- src/store/admission.rs: verified admission/idempotence.
- src/store/refs.rs: typed names and namespace operations.
- src/error.rs: StoreError without changing ContractError behavior.
- src/model.rs: only safe helper exposure needed for canonical persistence/reload.
- tests/oa02_store.rs, oa02_rollback.rs, oa02_concurrency.rs.
- scripts/verify-oa02.sh and spec-oa-02-transactional-store.md.

## 7. Schema v1

Store identifiers as raw fixed-length BLOBs and the complete canonical event envelope as BLOB. Denormalized columns are indexes; canonical wire is authoritative.

Tables and invariants:

- metadata(key primary key, value), including schema_version.
- contexts(context_id primary key, expected_genesis_id, nullable genesis_event_id, state pending/active).
- authorized_authors(context_id, author_id), composite primary key and context foreign key.
- events(event_id primary key, context_id, author_id, kind, canonical_wire).
- parent_edges(child_id, ordinal, parent_id), primary key child/ordinal, unique child/parent, both foreign keys.
- local_refs(context_id, name, event_id), primary key context/name.
- remote_refs(peer, context_id, name, event_id), primary key peer/context/name.

All ID columns require exact 32-byte length. Parent ordinal is 0-63. Ref targets and event contexts are foreign-key constrained.

Migration sequence:

1. Open a dedicated migration connection.
2. Enable and verify foreign keys on every connection if Turso is connection-scoped.
3. Begin IMMEDIATE transaction.
4. Read/create schema version and apply each migration once.
5. Fail closed on a newer schema.
6. Run structural checks.
7. Install triggers rejecting UPDATE/DELETE of events, parent edges, and authorization rows. Context state may only move pending to active. Refs stay mutable.
8. Explicitly commit or roll back. Never depend on Turso's next-access cleanup of a dropped transaction.

A focused Turso 0.7.2 probe must first prove transaction rollback, foreign-key enforcement, triggers, concurrent open, and restart behavior.

## 8. Public API semantics

Core types:

- Store: cloneable facade over database plus process-local write gate.
- ContextProvision: context, expected genesis, sorted unique initial authors.
- LocalRefName and PeerName: validated immutable name types.
- RefExpectation: Absent or Head(EventId).
- RefMutation: None or CompareAndSwap(context, name, expected, new_head).
- AdmissionStatus: Inserted, AlreadyPresent, or AlreadyApplied.

Async operations:

- open(path);
- provision_context(provision);
- authorize_author(context, author);
- admit(event, optional ref mutation);
- event(EventId);
- local_ref(context, name);
- list_local_refs(context);
- list_remote_refs(optional peer, context).

No public mutable connection or arbitrary SQL capability.

## 9. Admission transaction

1. Re-run event.verify and canonical to_wire before opening a write transaction.
2. Check in-memory bounds/names.
3. Acquire process write gate and begin IMMEDIATE transaction.
4. Load context; reject unknown.
5. Pending context accepts only exact expected zero-parent context.genesis.
6. Require author in append-only local allowlist.
7. Non-genesis requires at least one parent.
8. Load every parent; require existence and same context; defensively recheck order/count/uniqueness.
9. Event absent: insert wire/indexes. Same ID/same wire: idempotent. Same ID/different wire: collision and rollback.
10. Insert edges in body order.
11. Genesis atomically activates context and records genesis ID.
12. Apply optional CAS. Expected absent/head must match. A retry finding the same new head and same event returns AlreadyApplied; otherwise stale conflict includes current optional head.
13. Explicitly commit and return status.

Any validation/policy/parent/ref failure leaves no event, edge, context-state, or ref mutation. An error after commit acknowledgement is indeterminate; callers retry safely by ID and CAS.

## 10. Concurrency

- Serialize writes among Store clones with a Tokio mutex; do canonicalization before locking.
- Use IMMEDIATE transactions before reading CAS state.
- Never retry stale-head conflicts.
- Retry only proven transient busy failures, before commit acknowledgement, with a small bounded policy.
- Test two independently opened Stores against one file; do not claim stronger multiprocess semantics than pinned Turso proves.
- Consistency-sensitive reads use one connection snapshot.

## 11. Test matrix and gate

Tests:

- fresh schema, idempotent reopen, newer/corrupt fail closed;
- provisioning idempotent only for identical policy;
- wrong genesis, second root, unknown context, unauthorized author;
- missing/cross-context/duplicate/unsorted parents;
- OA-01 signature/ID/version/size mutation matrix;
- exact canonical wire/edge persistence;
- same event idempotence and collision rejection;
- CAS absent/head success, stale current-head error, retry AlreadyApplied;
- two writers yield one winner and one conflict;
- immutable table UPDATE/DELETE rejected;
- local/remote same-name isolation;
- restart preserves events, policy, and refs;
- malformed rows produce bounded errors without panic;
- every failure compares before/after table/ref snapshots.

Gate:

- D-02-01 approved and recorded.
- Schema/API reviewed independently.
- Rust 1.97 locked build, fmt, Clippy, predecessor regressions and OA-02 tests pass.
- verify-oa02 asserts schema version, dependency graph, deferred OA-03+ surfaces, rollback matrix, and no runtime/secret artifacts.
- Commit subject: OA-02: add transactional DAG store and refs.

# OA-03 — DAG Operations, Bundles, and Full Verification

## 12. Objective and file plan

OA-03 adds semantic-free context creation/join, append/fork/merge, deterministic projection, bounded bundle import/export, and full-store verification. It freezes the interfaces consumed by OA-04 and most CLI commands.

Files: store facade extensions; store/dag.rs; store/bundle.rs; store/verify.rs; strict bundle helpers; oa03 DAG/bundle/verify/adversarial tests; verify-oa03.sh; package spec.

## 13. DAG operations

- create_context(identity, branch): generate ContextId, sign context.genesis, provision author/genesis, admit, and create branch atomically at store level.
- join_context(provision): pending local policy; no implicit trust in peer.
- append(identity, context, branch, expected, kind, payload): one parent equal to expected; sign, admit, CAS.
- create_branch(context, name, from_head): require same-context event and atomically create absent local ref; no event.
- merge(identity, context, branch, expected, parents, payload): 2-64 unique parents, canonical sort, include expected target head, kind context.merge, admit and CAS.
- A fork is two refs to common immutable history followed by appends.
- Never infer relevance, choose a branch, or promote remote state.

## 14. Deterministic projection

1. Validate explicit heads exist in one context.
2. Iterative DFS with Enter/Exit frames.
3. Visit stored parents in ascending canonical EventId order.
4. Detect cycles with gray/black state.
5. Emit on Exit for parent-first topological order.
6. Deduplicate shared ancestors.
7. Count events and wire bytes before returning; no partial result on limit failure.
8. Stream only after a complete ordered ID plan passes limits.

Determinism never depends on SQL row order.

## 15. Bundle v1

Bundle is independently versioned and does not change OA-01.

Fields: bundle_version=1, one context, parent-first unique full signed event envelopes, and sorted unique advertised refs containing namespace/name/head.

Rules:

- strict JSON: no BOM, trailing data, duplicates, or unknown fields;
- every event independently OA-01 verified;
- one context; each parent earlier in bundle or already local;
- refs sorted/unique and target same context;
- advertised refs are unsigned peer claims; only target events are cryptographically verified;
- max 16 MiB canonical bytes, 1,024 events, 256 refs;
- export uses JCS; import accepts only insignificant whitespace/order differences.

Types: BundleV1, AdvertisedRef, BundleLimits, ImportReport(inserted, already_present, remote_refs_updated).

## 16. Export/import

Export receives context, immutable requested heads, locally resolvable known frontier, a ref snapshot, and limits. It computes deterministic parent-first union, excludes known ancestry, snapshots immutable heads/refs before traversal, sorts everything, and fails rather than truncates.

Import:

1. Strictly parse and enforce raw/canonical/count bounds before mutation.
2. Verify all events and parent-first order in memory.
3. Begin one IMMEDIATE transaction.
4. Recheck policy, authorization, parents, context, collisions, and order.
5. Insert events/edges idempotently.
6. Validate all ref targets now exist in context.
7. Update only explicitly supplied remote peer namespace; never local refs.
8. Commit once; any event/ref failure rolls back the bundle.

Repeated valid import inserts zero. Remote refs may refresh to identical values.

## 17. Full verification

From canonical wire, verify:

- schema version/objects;
- all envelopes and signatures;
- row EventId/context/author/kind equals parsed body;
- exact edge count, ordinal, order, no extras;
- parent existence and same context;
- one exact genesis per active context and no other root;
- event author allowed by append-only local policy;
- graph acyclic;
- local/remote refs exist in stated context;
- names canonical;
- bounded projection from each ref.

Use a read snapshot. Never repair silently. Return bounded structured findings and a truncated flag.

## 18. Tests and gate

- create/join/bootstrap/restart;
- fork common ancestry;
- 2-parent/64-parent merge and all invalid shapes;
- exact diamond and shared-ancestor projection fixtures;
- deep iterative chain and projection exact/+1 limits;
- bundle canonical vector and round-trip;
- parent-after-child, duplicate event/ref, unknown version/field, malformed event, cross-context and every +1 bound;
- one bad event rolls back all counts/refs;
- repeated import zero and local refs unchanged;
- injected wire/column/edge/context/ref corruption detected after reopen;
- bounded non-secret findings.

Gate: OA-02 API compatibility; Bundle v1 frozen for OA-04; full locked suite and verifier; independent graph, parser/resource, transaction, and corruption reviews. Commit: OA-03: add DAG operations bundles and verification.

# OA-04 — Authenticated Multi-Node Pull Synchronization

## 19. Objective and dependency plan

OA-04 lets one independent node pull immutable signed events and advertised refs. It offers bearer-authenticated bounded HTTP/1, not TLS, discovery, consensus, remote execution, or truth guarantees.

First approve oa-04-dependency-plan.md from a Rust 1.97 probe and exact feature audit. Files: http facade/server/client, sync facade/protocol, transport/sync/adversarial tests, verifier, feature snapshot, package spec.

## 20. Protocol v1

All endpoints require exactly one valid Authorization header.

- GET /v1/refs?context=ID returns sorted local refs, protocol version, context, and an opaque snapshot fingerprint over canonical ref names/heads.
- POST /v1/bundles/export accepts protocol version, context, immutable requested heads, known heads, offset/cursor, and limits; returns one bounded BundleV1 page, next cursor, requested-head fingerprint, and complete flag.

Expose no generic query, append, mutation, provider, or filesystem route.

Pagination:

- Build deterministic parent-first union for immutable requested heads and subtract ancestry of locally resolvable known heads.
- Offset/cursor identifies that immutable plan; request mismatch is an error.
- A local ref moving during transfer does not alter selected heads.
- Each page is valid relative to already acknowledged/imported pages.
- Update remote refs only after all pages complete and every snapshotted head exists locally.

## 21. Authentication and hardening

- Token has at least 32 OS-random bytes, sourced from file or environment, never demo command line.
- Missing, duplicate, malformed, wrong-scheme, short, or mismatched headers receive one generic 401 shape.
- Constant-time token comparison; never log headers.
- Default listener is loopback; non-loopback needs explicit acknowledgement and warning.
- Bound headers, body, concurrent requests, refs/events, response bytes, body-read time, and handlers.
- Stable errors contain code/request ID only.
- Disable redirects and unused features. Do not claim certificate management.

## 22. Pull algorithm and failure semantics

1. Validate peer, URL, token source, context, limits.
2. Fetch remote local-ref snapshot.
3. Read local known frontier.
4. Request deterministic pages for immutable snapshotted heads.
5. Enforce Content-Length when present and independently cap streamed bytes.
6. Strictly parse protocol/bundle and import through OA-03 only.
7. Never use a trusted-remote admission path.
8. After all pages, atomically verify heads and replace only remote/peer refs for context.
9. Return pages, received, inserted, already-present, remote-ref-update counts.
10. Repeated pull has inserted zero and unchanged local refs.

A bad page rolls back that page and prevents remote-ref update. Earlier verified orphan events may remain across pages. Do not falsely claim one transaction spans HTTP pages; if all-or-nothing transfer is required, stay inside one bundle bound or introduce reviewed staging tables.

## 23. Tests and gate

- loopback default and explicit override;
- auth matrix and token non-disclosure;
- exact/+1 header/body/concurrency/ref/event/byte/time bounds;
- one-way missing history, namespaced remote refs, local refs unchanged;
- repeated convergence zero;
- fork/merge parent-first transfer;
- immutable-head pagination during ref movement;
- malformed protocol/bundle/signature/parent/order rejection;
- truncated, chunked, false Content-Length, slow response bounded;
- timeout retry idempotence;
- no route mutates local refs or invokes providers;
- claim audit for HTTP/TLS/public network.

Gate: exact dependencies/features approved; OA-03 bundle vectors unchanged; protocol/auth/resource/supply-chain reviews; locked suite and verify-oa04. Commit: OA-04: add authenticated pull synchronization.

# OA-05 — Provider Recording Boundary and CLI

## 24. Objective and files

OA-05 exposes provider recording and stable automation CLI. It records exact caller-selected ancestry; no semantic selection, protocol compliance, or exactly-once claim.

Approve D-05-01 first. Files: provider and recording modules, approved opaque key-file functions, contextmesh command parser/dispatch, demo_agent JSONL process, key/provider/CLI/adversarial tests, verifier and package spec.

## 25. Provider contract and recording sequence

Use an object-safe Provider returning a boxed Send future. InvocationContext includes context ID, selected EventId, deterministic ancestry, request EventId, and random invocation ID. Outcome is opaque JSON response or bounded sanitized failure data.

Sequence:

1. Caller explicitly supplies context, local branch, expected selected head.
2. Project deterministic ancestry under OA-03 limits.
3. Generate random invocation/correlation ID.
4. Sign and CAS-append agent.request with parent selected head and payload containing invocation ID, selected head, and opaque input.
5. Only after commit invoke provider with pre-request ancestry and exact metadata.
6. Success creates agent.response with sole parent request.
7. Declared/transport failure creates agent.error with sole parent request and sanitized bounded data.
8. CAS branch request to result.
9. If another writer moved it, retain linked immutable result and return PostExecutionConflict(result ID, current head).

Never hold a transaction/lock across provider execution.

Crash windows documented and tested:

- before request commit: no invocation;
- after request commit before invoke: pending request recoverable;
- external side effect before result commit: retry may duplicate effect;
- result admission before ref CAS: detached result recoverable/mergeable.

## 26. CLI and JSON contract

Commands:

- key/token generate;
- context create/join/authorize;
- append and branch create;
- merge;
- show event/projection/refs;
- bundle export/import;
- verify;
- invoke with provider command;
- serve with token file/listen/ready file;
- sync with peer/url/token/context.

One stable JSON document to stdout:

- schema_version, ok, command, result on success;
- schema_version, ok=false, command, error.code and safe details on failure.

Warnings to stderr without secrets. Proposed exit classes: 0 success, 2 usage/config, 3 validation, 4 conflict, 5 auth, 6 not found, 7 provider/post-execution conflict, 8 transport/protocol/timeout, 9 database/internal.

Secrets come from files/environment and are never echoed. Bounded payload input uses file/stdin, not giant command arguments. Snapshot all JSON fields/error codes/exits.

## 27. JSONL demo provider

- One object per line, max 2 MiB.
- Input includes protocol version, invocation ID, selected context/head, canonical ancestry wires, and opaque input.
- Exactly one response/failure with matching invocation ID.
- Echo explicit opaque input only under demo namespace.
- No tool execution or A2A/ACP claim.
- Reject malformed/oversized lines without panic; never print environment/secrets; flush response.

## 28. Tests and gate

- atomic key/token create, symlink/permission rejection, no secret output, identity stable after restart;
- request visible before provider call;
- linked response and sanitized error;
- exact ancestry fixture;
- concurrent branch movement retains detached result plus conflict;
- pending/detached crash-window recovery queries;
- CLI JSON and every exit snapshot;
- malformed payload/JSONL/provider output exact/+1 bounds;
- restart verification;
- prohibited semantic/exactly-once/protocol-claim scan.

Gate: key decision approved and README wording reconciled without wire change; shared dependency audit; provider/key/CLI adversarial reviews; locked suite and verify-oa05. Commit: OA-05: add provider recording and CLI.

# OA-06 — Reproducible Demo and Documentation

## 29. Harness

- Bash strict mode, private temporary root, EXIT/INT/TERM trap.
- Independent A/B key/token/database/log/PID/ready files.
- Fresh OS-random secrets, never fixtures.
- Daemons use 127.0.0.1:0 and atomically publish readiness after migration/listen.
- Poll readiness with hard timeout and liveness; no blind sleep.
- Cleanup TERM, bounded wait, then KILL recorded child PIDs only.
- Preserve logs on failure; delete runtime secrets on success unless debug requested.
- Parse JSON with isolated Python standard library or tested CLI; no ambiguous grep.
- Print stable stages/assertions without secrets.

## 30. Seventeen-stage demo

1. Build locked binaries once.
2. Generate independent A/B keys and tokens.
3. A creates context/genesis and authorizes B.
4. Export join descriptor; provision B explicitly.
5. Start independent A/B daemons.
6. B pulls A genesis; assert one import, remote A ref, no implicit local movement.
7. Explicitly create B local main from genesis.
8. Record distinct request/response chain on A.
9. Record distinct request/response chain on B.
10. Pull both directions; retain own local main and namespaced peer main.
11. A explicitly merges sorted A-local and remote-B parents into merged by CAS.
12. B pulls A; both merged projection sequences match and contain each ancestor once.
13. Stop/restart both on same databases and new ephemeral ports.
14. Full verify both and compare projection again.
15. Pull both ways; assert zero inserts and unchanged local refs.
16. Export and mutate one signed byte; import fails and counts/refs remain unchanged.
17. Print public-ID/count-only PASS summary.

Never execute synchronized requests or promote remote refs.

## 31. Documentation and tests

README covers architecture, identity/domains, schema/policy/bootstrap, append-only authorization limitation, refs/fork/merge/projection, bundles/pull/pagination/idempotence, provider crash windows, CLI JSON/exits, secret files, loopback/VPN/tunnel guidance, integrity versus truth/auth/confidentiality/availability/consensus, prohibited claims, non-compliant future A2A/ACP mapping, and Option B deferral.

Tests run demo twice; inject lifecycle failures; occupy port/delay readiness/crash process; prove cleanup; scan transcript/log/process args for secrets; verify ignored runtime files; test every documented command; independent fresh-checkout execution.

Gate: OA-04/OA-05 committed; demo passes all stages; shell/reproducibility/tamper/docs reviews; verify-oa06 runs predecessor gates and demo. Commit: OA-06: add reproducible two-node demo.

# OA-07 — Release Verification and Option A Gate

## 32. Procedure

OA-07 adds no product behavior except release-blocker repairs.

1. Require clean worktree; record candidate commit/tree.
2. Verify pinned toolchain/native prerequisites and no unapproved overrides.
3. Audit exact dependencies/features/licenses/advisories and record accepted findings.
4. Run verifiers OA-00 through OA-06.
5. Run locked build, fmt check, Clippy -D warnings, workspace tests, and demo.
6. Repeat with fresh target and demo runtime roots.
7. Scan ignored/staged/tree content for secrets, databases, WAL, logs.
8. Run independent crypto, database, graph, transport, provider, shell, supply-chain and claim audits.
9. Build A1-A8 evidence links to exact tests/scripts/transcript.
10. Record limitations and check Always/Never consistency.
11. Assign complete or incomplete; missing/ambiguous evidence means incomplete.

## 33. Evidence artifacts

- scripts/verify-oa07.sh: deterministic non-recording gate.
- verification-artifacts/oa-07-release-evidence.md: commits, tools, commands/status, counts, checksums, demo transcript checksum, A1-A8 matrix, reviewers, limitations.
- verification-artifacts/oa-07-claim-audit.md: every claim demonstrated, limited, or removed.
- implementation-artifacts/spec-oa-07-release-gate.md: status and verdict.

No evidence contains keys, tokens, sensitive paths, or arbitrary payloads. Rerun the gate on the final evidence commit.

## 34. A1-A8 ownership

| Gate | Owners | Final proof |
|---|---|---|
| A1 identity | OA-01 | unchanged fixture/vector/domain/mutation evidence |
| A2 admission | OA-02/03 | invalid admission/import rollback and no panic |
| A3 DAG/refs | OA-02/03/05 | chains, fork, merge, projection, CAS, provider history |
| A4 persistence | OA-02/03 | restart, verify, corruption, idempotence |
| A5 sync | OA-04/06 | exchange, parent-first import, ref isolation, convergence |
| A6 provider | OA-05/06 | request-before-call, linked result/error, ancestry/crash limits |
| A7 boundaries | OA-04/05/06 | loopback/auth/bounds and claim audit |
| A8 evidence | OA-06/07 | clean locked quality suite and demo transcript |

Verdict is complete only when all commands/verifiers pass, every row has evidence, demo proves all required properties, no issue contradicts frozen constraints, documentation only claims demonstrations, evidence is committed, and worktree is clean. Otherwise Option B stays blocked.

Commit: OA-07: record Option A completion evidence.

## 35. Cross-package governance

Definition of ready: predecessors committed, clean tree, blocking decisions approved, dependency delta planned, public inputs/outputs/bounds/errors listed, tests mapped to acceptance.

Definition of done OA-02 through OA-06: checklist complete; exact dependencies locked; format/Clippy/tests/package and predecessor verifiers pass; malformed/mutation/boundary/restart tests pass; four required adversarial review layers patched; README describes current behavior; no later package or Option B work leaks in; one focused commit, no push unless requested.

Ask first before wire changes, replacing Turso, Turso replication, A2A/ACP/libp2p/CRDT/discovery/TLS-management/encryption/semantic dependencies, revocation or broader workspace policy, weakening auth/bounds/atomicity/CAS, remote promotion, remote execution, or unsupported claims.

## 36. Immediate next actions

1. Record D-02-01 in the frozen OA-02 implementation spec.
2. Run the Turso transaction/foreign-key/trigger/multi-connection/restart probe.
3. Implement and complete OA-02 before OA-03.
4. Run the approved D-04-01 Rust 1.97 probes before OA-04/OA-05 manifest changes.
5. Record D-05-01 in the OA-05 spec before implementing key custody.
6. Keep Option B blocked in every verifier until OA-07 records a complete verdict.
