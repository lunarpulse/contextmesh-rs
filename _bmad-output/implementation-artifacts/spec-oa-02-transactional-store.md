---
title: 'OA-02 Transactional Turso DAG Store and Ref Semantics'
type: 'feature'
created: '2026-08-16'
status: 'approved-for-execution'
approved: '2026-08-16'
approved_by: 'Lunarpulse'
baseline_commit: 'e82b386'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/spec-signed-agent-context-dag.md'
  - '{project-root}/_bmad-output/planning-artifacts/oa-02-oa-07-detailed-execution-plan.md'
  - '{project-root}/_bmad-output/planning-artifacts/oa-02-oa-07-decision-record.md'
  - '{project-root}/_bmad-output/planning-artifacts/oa-02-oa-07-test-traceability-matrix.md'
---

<frozen-after-approval reason="human-owned OA-02 intent and approved D-02-01 policy — do not modify unless human renegotiates">

## Intent

**Problem:** OA-01 events are cryptographically self-verifying but have no persistent admission boundary. A process cannot yet prove that an event belongs to a provisioned context, was authored by a locally authorized identity, references existing same-context parents, survives restart, or advances a branch without overwriting concurrent work.

**Approach:** Add an asynchronous local embedded Turso store that persists exact canonical OA-01 event envelopes and ordered parent edges. Provision each context with one expected genesis and an append-only local author allowlist. Re-run OA-01 verification before admission, validate context/authorization/parent rules, and admit events with optional local-ref compare-and-swap in one explicit IMMEDIATE transaction. Keep local and remote ref namespaces physically distinct and expose no arbitrary SQL or mutable-event API.

## Boundaries and constraints

**Always:** Preserve every frozen OA-01 byte and validator. Store exact canonical event wire as the authoritative record. Use one explicitly provisioned zero-parent context.genesis per context. Treat authorization as explicit append-only local policy. Enable and verify foreign keys on every connection. Use explicit IMMEDIATE transactions for writes and explicit commit/rollback. Check every parent exists and shares the event context. Keep local and remote refs separate. Require explicit CAS for local-ref movement. Return typed non-secret errors. Keep failed admission free of partial event, edge, context-state, authorization, or ref mutation. Reopen and independently reverify stored history.

**Ask first:** Change context bootstrap, permit a second root, add author removal/revocation, broaden beyond the approved local workspace policy, change ref-name grammar/bounds, weaken CAS or atomicity, change OA-01 wire behavior, replace Turso, enable Turso replication, expose raw connections/SQL, implement bundles/projection/merge helpers/HTTP/providers/CLI, or add dependencies beyond a proven minimal OA-02 runtime requirement.

**Never:** Treat authorization as truth or consensus. Trust denormalized columns over canonical wire. Update or delete admitted events/edges through public APIs. Accept absent or cross-context parents. Automatically move local refs from remote state. Retry a stale-head conflict as if it succeeded. Depend on dropped-transaction cleanup. Store private keys, bearer tokens, chain-of-thought, or Option B semantic metadata. Implement OA-03+ behavior early.

## Approved D-02-01 policy

1. Each context has exactly one signed zero-parent context.genesis event.
2. ContextId is generated from 32 bytes of fallible OS entropy by the future OA-03 creation helper; OA-02 accepts an explicitly provisioned typed ID.
3. Context trust and author authorization are local operator policy, not synchronized consensus.
4. Provisioning supplies ContextId, expected genesis EventId, and sorted unique initial authors.
5. A pending context accepts only the exact expected genesis as its first event.
6. Every other accepted event has at least one existing same-context parent.
7. Author allowlists are append-only for Option A. Revocation and trusted policy chronology are deferred.
8. Authorizing an identity permits local admission only and proves no truth, quality, or relevance.

## I/O and edge-case matrix

| Scenario | Input/state | Required behavior | Failure behavior |
|---|---|---|---|
| Open fresh store | Missing local database | Create schema v1 and verify required objects | Migration error, no partial version claim |
| Reopen | Valid schema/data | Preserve policy, events, edges, refs | Corruption/newer schema fails closed |
| Provision | New context and policy | Persist pending context and authors atomically | Conflict leaves prior policy unchanged |
| Genesis | Exact expected root and authorized signer | Store event, activate context, optionally create first ref | Wrong root/signer rolls back everything |
| Append | Valid signed event and parents | Store exact wire and ordered edges | Invalid/missing/cross-context parent leaves no row |
| Duplicate | Same ID and same canonical wire | Idempotent AlreadyPresent | Same ID/different wire is EventCollision |
| CAS append | Expected local head matches | Event/edges/ref commit together | StaleHead carries safe current head and rolls back |
| Retry | Same event and ref already at new head | Return AlreadyApplied | Different current head is stale conflict |
| Concurrency | Two writers share expected head | Exactly one winner | Loser receives typed conflict |
| Namespaces | Same branch name local and peer remote | Both persist independently | No implicit promotion or collision |
| Malformed storage | Bad row/wire/edge | Bounded typed corruption report, no panic | Never silently repair |

</frozen-after-approval>

## Schema v1

Identifiers are raw 32-byte BLOBs. canonical_wire is the source of truth.

- metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL), including schema_version=1.
- contexts(context_id BLOB PRIMARY KEY, expected_genesis_id BLOB NOT NULL, genesis_event_id BLOB NULL UNIQUE, state INTEGER pending/active).
- authorized_authors(context_id BLOB, author_id BLOB, PRIMARY KEY(context_id, author_id)).
- events(event_id BLOB PRIMARY KEY, context_id BLOB, author_id BLOB, kind TEXT, canonical_wire BLOB).
- parent_edges(child_id BLOB, ordinal INTEGER 0..63, parent_id BLOB, PRIMARY KEY(child_id, ordinal), UNIQUE(child_id, parent_id)).
- local_refs(context_id BLOB, name TEXT, event_id BLOB, PRIMARY KEY(context_id, name)).
- remote_refs(peer TEXT, context_id BLOB, name TEXT, event_id BLOB, PRIMARY KEY(peer, context_id, name)).

All relevant foreign keys and exact ID-length checks are mandatory. Triggers reject UPDATE/DELETE of events, parent_edges, and authorized_authors. Context state can only transition pending to active. Refs remain mutable only through store methods.

## Migration and connection procedure

1. Build the pinned local Turso database and open a dedicated migration connection.
2. Execute PRAGMA foreign_keys=ON and query it back; repeat for every later connection.
3. Begin an IMMEDIATE transaction.
4. Create/read schema version and apply ordered migrations exactly once.
5. Reject a database whose version is newer than this binary.
6. Verify required tables, indexes, constraints, and triggers.
7. Explicitly commit. On any error explicitly roll back; do not rely on transaction drop.
8. Every Store-created read/write connection runs the foreign-key setup check before use.

The approved disposable Rust 1.97/Turso 0.7.2 probe demonstrated explicit IMMEDIATE commit/rollback, per-connection foreign-key enforcement, immutable triggers across restart, multiple connections, independent handles to one file, and persistence after reopen.

## Public model

- Store: cloneable facade over local Turso database and a process-local write gate.
- ContextProvision: context, expected genesis, sorted unique initial authors.
- LocalRefName and PeerName: validated 1-64 byte lowercase segmented ASCII names.
- RefExpectation: Absent or Head(EventId).
- RefMutation: None or CompareAndSwap with context, local name, expectation, and new head.
- AdmissionStatus: Inserted, AlreadyPresent, AlreadyApplied.
- LocalRef and RemoteRef: immutable typed query results.
- StoreError: stable, non-secret storage/policy/ref categories that may wrap ContractError without leaking arbitrary data.

Required asynchronous methods:

- Store::open(path)
- Store::provision_context(provision)
- Store::authorize_author(context, author)
- Store::admit(event, ref mutation)
- Store::event(event ID)
- Store::local_ref(context, name)
- Store::list_local_refs(context)
- Store::list_remote_refs(optional peer, context)

No method exposes a mutable connection, arbitrary SQL, update/delete event, projection, bundle, or synchronization path.

## Validation and admission order

1. Verify OA-01 event and canonicalize wire before acquiring write lock.
2. Validate names/counts and optional ref mutation in memory.
3. Acquire process-local write gate.
4. Open configured connection and begin IMMEDIATE transaction.
5. Load context or reject ContextUnknown.
6. For pending context require exact expected context.genesis with zero parents.
7. Require body author in context allowlist.
8. For active context require one or more parents.
9. Load every parent; require existence and same context. Recheck order, uniqueness, and count.
10. If ID absent insert exact wire and derived columns. If present compare exact wire; identical is idempotent, different is collision.
11. Insert exact parent edges in body order.
12. For genesis atomically record genesis and activate context.
13. Apply optional local-ref CAS. Expected absent/head must match. If retry finds the same new head and event bytes, return AlreadyApplied; otherwise return StaleHead with current optional head.
14. Explicitly commit and return status.

Database errors after a commit acknowledgement are surfaced as indeterminate; a caller may safely retry using event identity and CAS. Stale conflicts are never automatically retried.

## Concurrency and resource policy

- Canonical verification occurs before the process write gate.
- Store clones serialize write transactions with one Tokio mutex.
- Independent Store instances rely on IMMEDIATE transaction behavior proven against pinned Turso.
- A bounded retry may cover only positively identified transient busy failures before commit acknowledgement.
- Read operations use a consistent connection/snapshot where multiple rows form one public result.
- All row counts and wire bytes are checked before allocation; OA-01's per-event bounds remain authoritative.
- No recursive graph traversal exists in OA-02.

## Error categories

DatabaseUnavailable, MigrationFailed, NewerSchema, CorruptStorage, ContextUnknown, ContextAlreadyExists, ContextProvisionMismatch, GenesisMismatch, UnauthorizedAuthor, ParentMissing, ParentContextMismatch, EventCollision, InvalidRefName, RefMissing, RefAlreadyExists, StaleHead, LimitExceeded, IndeterminateCommit, and Contract(ContractError).

Errors carry no SQL, canonical wire, payload, seed, token, or arbitrary database path. StaleHead may safely include the current optional EventId.

## File map

- src/store.rs: public facade and exports.
- src/store/schema.rs: schema and migrations.
- src/store/admission.rs: transactional admission.
- src/store/refs.rs: names and ref operations.
- src/error.rs: StoreError.
- src/model.rs: only safe canonical persistence helpers; no wire change.
- tests/oa02_store.rs: valid lifecycle/restart/policy/ref behavior.
- tests/oa02_rollback.rs: all invalid admission snapshots.
- tests/oa02_concurrency.rs: independent writer conflicts/retries.
- tests/oa02_probe.rs or scripts/probe-oa02-turso.sh: pinned Turso capability evidence.
- scripts/verify-oa02.sh: dependency, scope, schema, quality and rollback gate.

## Tasks and acceptance

- [ ] Record and lock any minimal Tokio runtime feature delta; preserve Turso 0.7.2/defaults-off.
- [ ] Implement schema v1, exact migrations, foreign-key setup, and immutable triggers.
- [ ] Implement typed names, provision/context policy, append-only authorization.
- [ ] Implement exact-wire admission, parent checks, idempotence/collision handling.
- [ ] Implement local-ref CAS and separate remote-ref query namespace.
- [ ] Implement stable non-secret StoreError behavior.
- [ ] Add fresh/reopen/restart, valid admission, policy, idempotence, CAS, concurrency tests.
- [ ] Add every invalid-admission rollback snapshot and malformed-storage no-panic tests.
- [ ] Add verifier/spec/README current-state documentation without implementing OA-03+.
- [ ] Complete schema/migration, transaction/concurrency, policy/admission, and API/supply-chain adversarial reviews; patch findings.

Acceptance requires:

- every invalid ID/signature/version/context/author/parent/ref case leaves no partial mutation;
- stale/concurrent writers cannot silently overwrite a head;
- exact events, policy, and local/remote refs survive restart;
- existing events and edges cannot be mutated through public APIs;
- duplicate identical admission is safe and collision fails closed;
- all OA-00/OA-01 gates and frozen fixture checks remain green.

## Verification

Required commands:

- cargo build --workspace --locked
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- bash scripts/verify-oa01.sh
- bash scripts/verify-oa02.sh

OA-03, OA-04, OA-05, OA-06, OA-07, and Option B surfaces remain deferred. Completion commit subject: OA-02: add transactional DAG store and refs.
