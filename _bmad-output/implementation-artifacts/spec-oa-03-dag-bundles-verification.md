---
title: 'OA-03 DAG Operations, Bundles, and Full Verification'
type: 'feature'
created: '2026-08-16'
status: 'approved-for-development'
approved: '2026-08-16'
approved_by: 'Lunarpulse via approved OA-02 through OA-07 execution plan'
baseline_commit: 'bfeb2c8a1a0bd5e737c636f56c33ffa3d43915b2'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/spec-signed-agent-context-dag.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-oa-02-transactional-store.md'
  - '{project-root}/_bmad-output/planning-artifacts/option-a-delivery-plan.md'
  - '{project-root}/_bmad-output/planning-artifacts/oa-02-oa-07-detailed-execution-plan.md'
  - '{project-root}/_bmad-output/planning-artifacts/oa-02-oa-07-decision-record.md'
  - '{project-root}/_bmad-output/planning-artifacts/oa-02-oa-07-test-traceability-matrix.md'
---

<frozen-after-approval reason="human-owned OA-03 intent and approved execution/test plan — do not modify unless human renegotiates">

## Intent

**Problem:** OA-02 can atomically admit one already-constructed event and move one local ref, but callers still lack safe context/bootstrap helpers, fork and merge operations, deterministic ancestry projection, a strict bounded transfer representation, atomic batch import, and a complete restart-time integrity report. OA-04 and OA-05 must not reimplement or bypass these boundaries.

**Approach:** Extend the OA-02 store with semantic-free context, branch, append, and explicit merge helpers; iterative deterministic parent-first projection; independently versioned canonical Bundle v1 import/export; and read-snapshot full-store verification. Every mutation continues through verified OA-01 events, local authorization, same-context parent checks, explicit transactions, and local/remote ref separation.

## Boundaries and constraints

**Always:** Preserve every frozen OA-01 byte and every OA-02 schema/admission/CAS behavior. Generate new ContextId values from 32 bytes of fallible OS entropy. Sign exactly one zero-parent context.genesis event with payload {}. Require explicit caller-selected context, branch, expected head, requested heads, known frontier, peer, limits, and advertised refs as applicable. Sort merge parents canonically and project stored parents by canonical EventId order rather than SQL order. Use iterative traversal, detect cycles, deduplicate shared ancestors, and emit parent-first. Enforce hard count and byte bounds before returning data or mutating storage. Parse bundles as strict JSON, independently verify every envelope, and import one bundle in one IMMEDIATE transaction. Update only the explicitly supplied remote peer namespace during import; never move a local ref. Verify canonical wire as authoritative and report bounded, non-secret findings without repair.

**Ask first:** Change OA-01 wire, event kinds, context.genesis payload, context bootstrap/authorization, schema v1, name grammar, ref CAS, hard bounds, Bundle v1 fields/meaning/canonical bytes, merge kind, projection order, import atomicity, verification coverage, or public interfaces frozen here; accept unprovisioned peer trust, author revocation, implicit branch choice/ref movement, recursive traversal, bundle truncation, alternate persistence/replication, or a new dependency.

**Never:** Infer truth, relevance, sufficiency, consensus, or semantic context. Treat advertised refs as signed. Trust denormalized rows, SQL row order, a peer, or a bundle before local verification. Admit a child before its bundled parent. Partially commit a bundle. Delete an absent advertised remote ref, promote a remote ref, or mutate a local ref as a side effect of projection/export/import. Repair corruption silently. Store or transfer seeds, bearer tokens, private chain-of-thought, provider configuration, or Option B metadata. Implement HTTP, provider execution, CLI behavior, or Option B.

## Frozen constants and limits

| Constant | Value | Rule |
|---|---:|---|
| BUNDLE_VERSION | 1 | Exact; every other value rejects |
| MAX_PROJECTION_EVENTS | 100,000 | Hard maximum and default |
| MAX_PROJECTION_WIRE_BYTES | 67,108,864 (64 MiB) | Sum of canonical event envelopes; hard maximum and default |
| MAX_BUNDLE_EVENTS | 1,024 | Hard maximum and default |
| MAX_BUNDLE_CANONICAL_BYTES | 16,777,216 (16 MiB) | Raw input and canonical Bundle v1 output must each fit |
| MAX_BUNDLE_REFS | 256 | Hard maximum and default |
| MAX_VERIFICATION_FINDINGS | 256 | Default report cap; caller may request 1..=1,024 |

ProjectionLimits and BundleLimits are immutable validated value types. A caller may lower a corresponding maximum but cannot pass zero or exceed the hard maximum. Requested-head and known-frontier lists are each unique, canonicalized by EventId order, and bounded by MAX_BUNDLE_EVENTS for export. Projection accepts 1..=MAX_BUNDLE_REFS explicit heads. Limits are checked with overflow-safe arithmetic. OA-01 per-event limits continue to apply independently.

## Public DAG model and operations

Types:

- CreatedContext { context, genesis, branch } where genesis is the admitted SignedEventV1 and branch is its local ref.
- ProjectionLimits { max_events, max_wire_bytes } with Default equal to the hard limits and a checked constructor for lower limits.
- Projection { context, heads, events, canonical_wire_bytes }; heads are sorted unique and events are verified parent-first unique SignedEventV1 values.

Asynchronous Store operations:

- create_context(&SigningIdentity, LocalRefName) -> CreatedContext.
- join_context(ContextProvision) -> (); this is an explicit alias of idempotent pending provisioning and performs no peer/network action.
- append(&SigningIdentity, ContextId, LocalRefName, expected: EventId, kind, payload) -> SignedEventV1.
- create_branch(ContextId, LocalRefName, from_head: EventId) -> LocalRef.
- merge(&SigningIdentity, ContextId, LocalRefName, expected: EventId, parents, payload) -> SignedEventV1.
- project(ContextId, heads, ProjectionLimits) -> Projection.

create_context obtains entropy, constructs context.genesis with no parents and payload {}, provisions only the signing identity, admits the genesis, activates the context, and creates the requested absent local ref as one store-level atomic operation. An entropy/signing/database/ref failure leaves no context, author, event, edge, or ref. A generated ContextId collision fails closed; it is not reported as successful or attached to existing policy.

join_context creates or confirms only the pending ContextProvision. It never admits genesis, contacts a peer, imports a bundle, or adds trust beyond the supplied append-only allowlist.

append requires the named local ref currently equals expected. It signs one event whose sole parent is expected and atomically admits it with RefExpectation::Head(expected). context.genesis and context.merge are reserved helper kinds and reject through append. Any stale head leaves the event absent. The payload remains opaque.

create_branch requires from_head exist and derive from canonical wire in context. It atomically inserts an absent local ref and no event. The same name and same head is an idempotent success; the same name at another head returns RefAlreadyExists and does not move it.

merge requires 2..=64 supplied unique parents, sorts them by canonical EventId text before signing, requires every parent exist in context, and requires expected both equal the current target branch head and appear in parents. It signs kind context.merge and atomically admits/CAS-moves the target branch. A missing, duplicate, cross-context, absent-expected, one-parent, 65-parent, stale-head, or reserved-shape failure leaves no event/ref mutation. A fork is represented only by two explicit refs to common immutable ancestry followed by ordinary appends; there is no hidden fork state.

## Deterministic projection

1. Canonicalize explicit heads to sorted unique EventIds; reject empty, over-limit, missing, corrupt, or cross-context heads.
2. Use one read connection/snapshot for every event, edge, and wire read in the operation.
3. Traverse with iterative Enter/Exit frames and white/gray/black visitation state.
4. Reparse and verify each canonical envelope. Require row identity/columns and exact stored edge ordinals to equal the signed body.
5. On Enter, visit signed parents in ascending canonical EventId order. A gray parent is ProjectionCycle; black nodes are not revisited.
6. Emit on Exit. Therefore all parents precede a child, shared ancestors appear once, and independent ready nodes are resolved solely by sorted heads/parents.
7. Count each unique event and its exact canonical envelope bytes before returning. Exceeding either requested limit returns ProjectionLimitExceeded with no partial Projection.
8. Build and validate the complete ordered ID plan before exposing or streaming event data.

A projection is ancestry selected by explicit immutable heads. It is not relevance ranking, prompt construction, truth filtering, or consensus.

## Bundle v1 wire contract

The top-level strict JSON object has exactly these fields:

- bundle_version: JSON integer 1.
- context: canonical ContextId text.
- events: parent-first unique array of complete OA-01 signed-event JSON objects, not strings.
- refs: sorted unique array of { namespace, name, head } objects.

An advertised ref has exactly namespace="local", a canonical LocalRefName, and a canonical EventId head. namespace is explicit for versioning and attribution; Bundle v1 does not advertise another peer's remote-tracking refs. Refs are strictly sorted and unique by (namespace, name), so one advertised local name has one head. Their target events must be in the bundle or already exist locally in the same context at import. They are unsigned peer claims; event signatures authenticate no ref position.

Bundle JSON rejects a BOM, trailing data, malformed JSON, duplicate keys at every depth, unknown/missing fields, wrong types, noncanonical typed text, unsupported version/namespace, duplicate events, parent-after-child, ref disorder/duplicates, mixed contexts, and every hard-bound excess. Import accepts insignificant JSON whitespace/member-order differences, then canonicalizes using RFC 8785/JCS. to_wire emits exact JCS bytes. Both raw input and canonical output must fit MAX_BUNDLE_CANONICAL_BYTES.

BundleV1 is an immutable validated value containing context, events, and refs. BundleV1::from_wire performs strict syntax/shape/event verification and in-bundle order checks without storage access. BundleV1::to_wire revalidates and emits JCS. BundleV1::from_parts is a checked constructor used by deterministic export and future OA-04 paging; a parent absent from those parts is an external frontier requirement that import must resolve locally.

Types:

- AdvertisedRef { namespace: RefNamespace::Local, name: LocalRefName, head: EventId }.
- BundleLimits { max_events, max_canonical_bytes, max_refs }.
- BundleV1 { bundle_version, context, events, refs } with read-only accessors.
- ImportReport { inserted, already_present, remote_refs_updated }.

No private key, token, database path, local authorization list, provider data outside signed payloads, or remote-ref table row has a Bundle v1 representation.

## Deterministic export

Store::export_bundle(context, requested_heads, known_frontier, advertised_ref_snapshot, limits) returns BundleV1.

- Inputs are copied and canonicalized before traversal. Requested heads are nonempty, immutable, locally present, verified, and in context. Known-frontier heads must also be locally present, verified, and in context.
- Under one read snapshot, compute the parent-first unique ancestry union of requested heads and subtract the complete ancestry union of known-frontier heads.
- The resulting event order is the deterministic projection order after filtering. Exclusion never leaves an included child whose excluded parent is not in known ancestry.
- advertised_ref_snapshot consists of immutable LocalRef values captured by list_local_refs. Export validates names, context, sorted uniqueness, target existence/context, and the ref limit but does not require the mutable database ref still have that head. This preserves caller-selected snapshot semantics if refs move concurrently.
- Export fails rather than truncates when any requested BundleLimits value would be exceeded. It returns no partial bundle.
- Empty events are valid when known frontier already covers all requested ancestry; refs may still be advertised.

The caller is responsible for supplying known frontier that the receiver actually has. Import never trusts that claim and rejects a missing parent atomically. OA-04 may build deterministic pages with checked BundleV1::from_parts; its cursor/fingerprint and cross-page guarantees remain OA-04 scope and cannot weaken Bundle v1 validation.

## Atomic import

Store::import_bundle(peer: PeerName, wire, BundleLimits) -> ImportReport performs:

1. Reject over-limit raw bytes before parse/allocation; strictly parse, canonicalize, and enforce count/canonical-byte/ref limits before locking.
2. Independently verify all OA-01 envelopes, one-context membership, unique IDs, and parent-first in-bundle order in memory.
3. Acquire the write gate and begin one IMMEDIATE transaction.
4. Require an explicitly provisioned context; recheck expected genesis, active/pending state, append-only author authorization, event collisions, exact parent order, and every external-frontier parent against canonical local wire.
5. Insert absent events and exact edges in array order. Existing same-wire events count already_present and are fully cross-checked; different wire is EventCollision.
6. Permit activation only when the exact provisioned genesis is the first needed root. Require exactly that root for a pending context; active contexts must already retain it. Reject every other zero-parent event.
7. After all events are valid/present, require every advertised ref target exist in the bundle or canonical local storage and belong to the bundle context.
8. Upsert only remote_refs rows for the explicit peer/context/supplied local name. Count only inserted rows or rows whose head changed; identical refreshes count zero. Do not delete peer refs absent from the bundle and never read or mutate local_refs for promotion.
9. Commit exactly once. Any event, policy, parent, collision, order, context, limit, or ref failure explicitly rolls back every event, edge, context transition, and remote-ref update from this bundle.

A repeated valid import reports inserted=0 and already_present equal to the number of bundle events; identical advertised refs report remote_refs_updated=0. ImportReport counters use checked usize conversion and reveal no payload or secret.

## Full-store verification

Types:

- VerificationCategory: Schema, Context, Authorization, EventWire, EventIdentity, EventColumns, EdgeSet, ParentMissing, ParentContext, Genesis, Cycle, LocalRef, RemoteRef, RefName, ProjectionLimit.
- VerificationFinding { category, context: Option<ContextId>, event: Option<EventId>, related_event: Option<EventId> }. It carries no SQL, payload, wire, author secret, peer/token, path, or arbitrary database error text.
- VerificationReport { valid, checked_contexts, checked_events, checked_refs, findings, truncated }.
- VerificationLimits { max_findings, projection: ProjectionLimits }, defaulting to MAX_VERIFICATION_FINDINGS and default projection bounds.

Store::verify_full(VerificationLimits) uses one read snapshot, never mutates, and:

- verifies schema version, fingerprint, required objects, foreign keys, and structural constraints;
- reparses and verifies every canonical envelope and exact row EventId/context/author/kind columns;
- verifies exact edge count, contiguous ordinal, signed order, no extras/duplicates, parent existence, and same canonical context;
- verifies each context's expected genesis/state relationship, exactly one matching context.genesis root for every active context, no other root, at least one authorized author, and every stored event author in the append-only local allowlist;
- detects cycles iteratively across the complete graph;
- verifies every local/remote ref name, target existence, and target canonical context;
- runs bounded deterministic projection from each ref head so reachable corruption and resource excess are explicit.

Verification scans the complete store even after the finding cap where safe; findings stop at max_findings and truncated becomes true when any further finding is observed. If corruption makes continued interpretation unsafe, it records the safest available category, sets truncated=true, and fails closed. valid is true only when no finding exists and the scan completed. Database/schema failures that prevent creating the read snapshot remain typed StoreError failures; verification never claims validity or repairs data.

## Error categories

Add stable non-secret StoreError categories: EntropyUnavailable, ReservedEventKind, InvalidMerge, ProjectionCycle, ProjectionLimitExceeded, BundleMalformed, BundleUnsupportedVersion, BundleOrder, BundleLimitExceeded, BundleRefInvalid, VerificationLimitInvalid. Existing OA-01 ContractError and OA-02 StoreError behavior remains compatible. Safe parent/current-head IDs may continue to appear where already approved; bundle parse errors do not echo input.

## I/O and edge-case matrix

| Scenario | Input/state | Required behavior | Failure behavior |
|---|---|---|---|
| Bootstrap/restart | New identity and branch | One active context/genesis/local ref survives reopen | Any failure leaves no partial context |
| Join | Exact ContextProvision | Pending policy only; idempotent | Mismatch leaves old policy unchanged |
| Append/fork | Two refs share root then diverge | Common ancestry immutable and visible | Stale/foreign head leaves no event/ref |
| Merge | 2 or 64 same-context parents including expected | Sorted context.merge admitted and branch CAS moves | Every invalid shape is typed and atomic |
| Diamond projection | Merged/forked DAG | Exact parent-first deterministic unique sequence | Missing/cycle/corruption/limit returns no partial output |
| Bundle round trip | Canonical Bundle v1 | Exact vector bytes parse/render/reparse | Unknown/duplicate/order/+1 input rejects |
| Import | Valid parent-first events and refs | One atomic commit and remote-only updates | Any bad event/ref rolls back whole bundle |
| Repeat import | Same bundle and peer | Zero inserts/ref changes | Local refs remain byte-for-byte unchanged |
| Verify restart | Valid reopened store | valid=true and complete counts | No repair or unsupported validity claim |
| Verify corruption | Wire/column/edge/context/ref mutation | Bounded safe finding or typed open failure | Never panic, disclose data, or silently ignore |

</frozen-after-approval>

## File map

- src/store.rs: preserve OA-02 facade/types; share transaction, row, and canonical validation internals.
- src/store/dag.rs: atomic context/join/append/branch/merge helpers and iterative projection.
- src/store/bundle.rs: strict Bundle v1 model, canonical render, deterministic export, atomic import.
- src/store/verify.rs: read-snapshot complete verification and bounded report types.
- src/model.rs: only a crate-private strict-JSON/canonical helper reuse if needed; no OA-01 public/wire change.
- src/error.rs: additive non-secret OA-03 categories.
- tests/oa03_dag.rs: 03-D01/D02 and bootstrap/restart/fork/merge behavior.
- tests/oa03_projection.rs: 03-P01/P02/P03 exact diamond, deep iterative, cycle/corruption, and bounds.
- tests/oa03_bundle.rs: 03-B01/B02/B04/B05 vector, parent-first union, repeat import, remote-only behavior.
- tests/oa03_adversarial.rs: 03-B03/B06 strict parser, every +1, malformed order/event/ref, and whole rollback.
- tests/oa03_verify.rs: 03-V01/V02 valid restart and corruption matrix.
- tests/fixtures/oa03-bundle-v1-golden.json: independently reviewable exact canonical Bundle v1 vector and provenance.
- scripts/verify-oa03.sh: artifact, dependency, OA-01/OA-02 regression, fixture, quality, test, and deferred-scope gates.
- README.md and this specification: current capabilities and bounded integrity/not-truth claims.

The planned split into src/store/*.rs may be adapted to Rust module mechanics without changing the frozen public behavior. No OA-04/OA-05 dependency or implementation surface is authorized by this package.

## Tasks and acceptance

- [ ] Add only additive OA-03 types/errors and preserve OA-02 public API/source behavior.
- [ ] Implement atomic create/join/append/branch/merge helpers and all invalid-shape rollback paths.
- [ ] Implement one-snapshot iterative deterministic projection with exact count/wire bounds.
- [ ] Implement strict immutable Bundle v1 model, exact canonical vector, and deterministic bounded export.
- [ ] Implement one-transaction policy-rechecked idempotent import and remote-only ref updates.
- [ ] Implement full-store read-snapshot verification with bounded non-secret findings and no repair.
- [ ] Cover every approved 03-D/P/B/V traceability row plus bootstrap/restart and all detailed-plan adversarial cases.
- [ ] Update README, feature snapshot if and only if graph output changes, and verify-oa03.sh without implementing OA-04+.
- [ ] Complete independent graph-determinism, parser/resource, atomic-import, and corruption/verification review layers; patch all actionable findings.

Acceptance requires:

- chain, fork, 2-parent merge, and 64-parent merge preserve exact immutable ancestry and CAS semantics;
- deterministic projection returns every reachable ancestor exactly once, parent-first, independent of SQL/insertion/head order, and deep traversal does not recurse;
- Bundle v1 exact bytes are frozen, strict, bounded, independently event-verified, and usable by OA-04 without bypass paths;
- one invalid event, parent, policy, collision, or ref makes bundle import leave all event/edge/context/ref counts unchanged;
- repeat import inserts zero, identical remote refs change zero, and local refs never move;
- valid state passes after restart while every approved corruption class is surfaced safely;
- OA-00/OA-01/OA-02 locked tests, vectors, schema, API behavior, and verifiers remain green.

## Change log

- 2026-08-16, specification freeze: derived from the human-approved OA-03 execution plan and minimum traceability matrix. Resolved public method semantics, hard/default bounds, canonical Bundle v1 field rules, known-frontier/ref-snapshot export behavior, idempotent remote-ref counting, and bounded verification report shape without changing OA-01 or OA-02.

## Verification

Required final commands:

- cargo build --workspace --locked
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- bash scripts/verify-oa01.sh
- bash scripts/verify-oa02.sh
- bash scripts/verify-oa03.sh

OA-04 HTTP/synchronization, OA-05 provider/key/CLI, OA-06 demo, OA-07 release verdict, and Option B remain deferred. Completion commit subject: OA-03: add DAG operations bundles and verification.
