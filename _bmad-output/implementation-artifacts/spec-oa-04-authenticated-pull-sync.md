---
title: 'OA-04 Authenticated Multi-Node Pull Synchronization'
type: 'implementation-spec'
created: '2026-08-16'
status: 'ready-for-implementation'
approved_plan: '../planning-artifacts/oa-02-oa-07-detailed-execution-plan.md'
dependency_plan: './oa-04-dependency-plan.md'
baseline_commit: '93d1ca122eec25d64d9d38352faf87900d3bef30'
review_loop_iteration: 0
option_b_gate: 'blocked-until-OA-07-A1-A8'
---

# OA-04 Authenticated Multi-Node Pull Synchronization

## Intent

OA-04 adds authenticated, bounded, pull-only exchange of OA-03 immutable signed events and unsigned advertised refs between independent stores. It is an HTTP/1 anti-entropy protocol over caller-selected history. Every received event still passes the OA-01 and OA-02/OA-03 admission rules.

This package does not add TLS, discovery, consensus, truth, semantic relevance, provider execution, remote append, generic database queries, filesystem access, or local-ref mutation. Plain HTTP provides no confidentiality or server identity. Loopback is the default; any non-loopback plaintext use requires an explicit acknowledgement and a fixed warning.

## Frozen dependency selection

The successful D-04-01 record in oa-04-dependency-plan.md is normative.

OA-04 may add only:

- Axum =0.8.9, defaults off, features http1,json,tokio;
- Reqwest =0.13.4, defaults off, feature json; and
- the OA-04 Tokio direct features net,rt,sync,time on the existing =1.53.1 pin, with macros retained for tests.

BLAKE3, base64, getrandom, serde, serde_json, thiserror, Tokio, zeroize, and the store are already direct dependencies and are reused. No constant-time crate, direct Hyper/Tower crate, TLS stack, proxy resolver, compression, cookie, multipart, stream/futures, or agent-protocol dependency is added. Clap and Tokio process/signal remain deferred to their owning later package.

Every Reqwest client builder must call redirect(Policy::none()), no_proxy(), http1_only(), and the frozen connect/request timeouts.

## Frozen constants and limits

| Constant | Value |
|---|---:|
| SYNC_PROTOCOL_VERSION | 1 |
| bearer token | exactly 32 decoded random bytes |
| MAX_HTTP_HEADERS | 96 parsed fields (Axum/Hyper rejects the 101st including required fields) |
| MAX_HTTP_HEADER_BYTES | 16 KiB aggregate names plus values after parsing |
| MAX_HTTP_HEADER_VALUE_BYTES | 8 KiB after parsing |
| MAX_REQUEST_TARGET_BYTES | 2 KiB after parsing |
| MAX_PULL_REQUEST_BODY_BYTES | 64 KiB |
| MAX_SYNC_RESPONSE_BODY_BYTES | 16 MiB + 64 KiB |
| MAX_CONCURRENT_HTTP_REQUESTS | 16, reject rather than queue excess |
| BODY_READ_TIMEOUT | 5 seconds |
| CONNECT_TIMEOUT | 5 seconds |
| REQUEST_AND_HANDLER_TIMEOUT | 30 seconds |
| refs in one snapshot | 256 |
| requested heads | 256 sorted unique IDs |
| known heads | 256 sorted unique IDs |
| pages in one pull | 100,000 |
| events in one page | 1,024 |
| canonical Bundle bytes in one page | 16 MiB |
| export-plan projection | OA-03: 100,000 events / 64 MiB event envelopes |

Limits are checked with overflow-safe arithmetic before caller-controlled growth or store mutation. Axum 0.8.9's selected serve path delegates to Hyper's HTTP/1 parser, whose hard parsed-map maximum is 100 headers; OA-04 accepts at most 96 parsed fields so the required Host, Authorization, Content-Type, and Content-Length fields can coexist at the exact application limit. The server checks request-target and post-parse header count/value/aggregate limits before authentication or route handling, acquires one of 16 permits with a non-waiting attempt, bounds body collection to 64 KiB plus one, and wraps body reading and handlers in their independent timeouts. The client rejects an excessive or contradictory Content-Length before reading, then repeatedly calls Reqwest Response::chunk() and stops at the response cap plus one even when Content-Length is absent or false.

The selected Axum serve facade does not expose Hyper's lower-level header-read timer or parser-buffer configuration. OA-04 therefore adds an accept-layer TapIo wrapper, using Tokio poll-based AsyncRead/AsyncWrite traits available through tokio/net, that starts a 5-second deadline on first connection read and fails reads until a complete HTTP header terminator is observed within the frozen 16 KiB pre-handler cap. The wrapper then passes the unchanged bytes to Axum. This bounds partial-header slowloris traffic before a Request exists without importing Hyper directly or enabling tokio/io-util. Exact/+1 and slow-partial-header socket tests are mandatory.

Library parser limits are documented defense in depth; application request checks and the accept-layer pre-header cap remain independently tested. Bodies are never read with unbounded bytes(), text(), or json() helpers.

## Credential contract

A bearer credential is exactly 32 random bytes rendered as canonical unpadded base64url text with prefix token1_. It is loaded only from an explicit environment-variable name or file path; no OA-04 API accepts a command-line token. The source must contain exactly the canonical token text with no BOM, whitespace, or trailing newline.

Token files must be regular non-symlink files. On Unix, group or other permission bits are rejected. File loading performs pre-open symlink metadata, opened-handle metadata, and post-open path metadata checks; device and inode must match at all three observations. Platforms without an equivalent stable file-identity and permission check reject file token sources rather than claim race-safe support. OA-04 reads existing token files but does not create or repair them. Environment and file failures map to one non-secret token-source category and never include the variable name, path, bytes, or OS error.

The client retains the exact Authorization header bytes in a zeroizing private value with a redacted custom Debug and marks the Reqwest HeaderValue sensitive. Parsing and construction avoid ordinary String copies of secret text. The server hashes the expected header, zeroizes its source buffer, and retains only a BLAKE3 Hash after startup. The digest uses derive context org.aaif.contextmesh.sync.auth.v1 over the exact Authorization header bytes. For an incoming request it requires exactly one header field, hashes its exact bytes under the same domain, and compares fixed-size BLAKE3 Hash values using BLAKE3's documented constant-time Hash::eq. Missing, duplicated, non-ASCII, malformed, wrong-scheme, short, and mismatched credentials all return the same status, headers, and JSON shape:

    401
    {"error":{"code":"authentication_failed","request_id":"req1_<22 base64url chars>"},"protocol_version":1}

No header, token, request body, response body, URL credentials, or secret source is logged or included in an error. Authentication runs before route-specific parsing so an unauthenticated caller cannot distinguish valid routes or contexts. A fixed WWW-Authenticate: Bearer header is allowed; it carries no detail.

At server construction, 32 bytes of OS randomness seed request IDs. Each ID is the first 16 bytes of BLAKE3 keyed-hash output over an 8-byte big-endian checked atomic counter and renders as req1_ plus 16 unpadded base64url bytes. The seed is private and zeroized when the server state is dropped. Entropy or counter exhaustion fails closed without disclosing state.

## Endpoint and listener policy

A peer endpoint is an absolute http:// URL with an IP-literal host and explicit port. It has no user information, query, fragment, or base path other than /. Hostnames are rejected: OA-04 does not provide DNS discovery. HTTPS is rejected because no TLS feature is selected.

Server bind addresses and client endpoint IPs default to loopback. A non-loopback IP is accepted only with a dedicated plaintext-exposure acknowledgement in the corresponding config. The resulting server/client value carries NON_LOOPBACK_PLAINTEXT_WARNING; callers must surface that fixed warning. The acknowledgement is not a TLS or public-network safety claim.

Reqwest proxy discovery is disabled regardless of process environment, redirects are disabled, and only HTTP/1 is used. A redirect is a protocol error and its target is never followed.

## Strict protocol JSON

All successful requests and all responses are exact RFC 8785/JCS bytes with Content-Type: application/json. Parsers reject BOMs, trailing data, duplicate keys at every depth, unknown or missing fields, unsafe numbers, wrong types, unsupported versions, noncanonical typed values, noncanonical member/array order, and any input whose bytes differ from its canonical rendering. Error responses use the same canonical writer.

Arrays of IDs and refs are strictly sorted and unique. Every request context must be provisioned and active at the serving store. Protocol parsers perform no store mutation.

### GET /v1/refs

The exact target is /v1/refs?context=CTX, with one canonical ContextId and no second query member. A GET body is forbidden.

Success is:

    {
      "protocol_version": 1,
      "context": "ctx1_...",
      "refs": [
        {"namespace":"local","name":"main","head":"evt1_..."}
      ],
      "snapshot_fingerprint": "refs1_..."
    }

Refs are one read-snapshot result in canonical name order and are unsigned claims by the authenticated peer. More than 256 refs fails rather than truncates.

The fingerprint is 32 BLAKE3 derive-key bytes rendered as unpadded base64url with prefix refs1_. The derive context is org.aaif.contextmesh.sync.refs.v1; input is the canonical JSON object containing exactly protocol_version, context, and refs. The fingerprint field itself is excluded.

### POST /v1/bundles/export

The exact canonical request is:

    {
      "protocol_version": 1,
      "context": "ctx1_...",
      "requested_heads": ["evt1_..."],
      "known_heads": ["evt1_..."],
      "cursor": null,
      "limits": {"max_events":1024,"max_bundle_bytes":16777216}
    }

Requested heads are immutable IDs selected from the earlier ref snapshot. They must all exist in the serving context. Known heads are a client hint: the server retains only IDs currently resolvable in the same serving context and subtracts their complete ancestry. Unknown or cross-context known IDs are ignored, never treated as trusted frontier.

Limits are nonzero and no greater than OA-03 hard Bundle limits. The first request uses null cursor. A later request repeats every field exactly except for the returned cursor.

Success is:

    {
      "protocol_version": 1,
      "context": "ctx1_...",
      "requested_head_fingerprint": "heads1_...",
      "bundle": {"bundle_version":1,"context":"ctx1_...","events":[],"refs":[]},
      "next_cursor": null,
      "complete": true
    }

The embedded value is an independently valid exact OA-03 Bundle v1. Its refs array is always empty: no page import may move remote refs before whole-transfer completion. The protocol envelope and embedded Bundle must each satisfy their byte bounds.

The requested-head fingerprint uses BLAKE3 derive context org.aaif.contextmesh.sync.heads.v1 over canonical {protocol_version,context,requested_heads} and prefix heads1_. It is identical on every page and is checked against the initial ref snapshot heads.

## Deterministic pagination

For each export request, the server performs one deferred store snapshot and:

1. validates and canonically normalizes requested heads;
2. filters known heads to same-context IDs present in that snapshot;
3. computes OA-03 deterministic parent-first ancestry for requested heads;
4. subtracts the complete ancestry set of the effective known heads;
5. preserves the remaining parent-first order as the immutable plan;
6. starts at offset zero or the validated cursor offset; and
7. emits the longest nonempty prefix fitting both requested page limits and the complete response-body cap.

An empty difference returns one complete page with zero events. A single event that cannot fit fails with a limit error; pages are never silently truncated below a claimed complete boundary.

A cursor is cursor1_ plus unpadded base64url of exactly 40 bytes: an 8-byte big-endian next-event offset followed by a 32-byte plan fingerprint. The plan fingerprint uses derive context org.aaif.contextmesh.sync.plan.v1 over canonical {protocol_version,context,requested_heads,effective_known_heads,limits}. The cursor is opaque to clients. The server validates prefix, encoding, offset boundary, plan fingerprint, and all repeated request fields. Any mismatch is pagination_conflict; it never guesses or restarts silently.

Because requested event IDs and ancestry are immutable, moving a local ref after GET cannot alter the selected plan. A concurrent import that changes which supplied known heads are effective can invalidate a cursor and require a safe restart; it cannot cause a mixed plan.

Every page is parent-first relative to the effective known ancestry and prior pages. The client imports a page successfully before using its next cursor. Thus any parent omitted from page N is either in the effective known frontier or an acknowledged earlier page.

## Pull state machine

PullClient::pull performs exactly:

1. validate peer name, endpoint, token source, context, and limits;
2. fetch and strictly verify one remote local-ref snapshot;
3. read the local refs and the selected peer's existing remote refs without mutating either namespace;
4. form the known frontier as the sorted unique union of those heads; fail if the union exceeds 256, while an empty frontier remains valid;
5. request pages for the immutable sorted union of snapshotted remote ref heads;
6. verify version, context, requested-head fingerprint, cursor progression, completion relation, empty Bundle refs, response/body limits, and Bundle v1 on every page;
7. call OA-03 Store::import_bundle for each page, with the explicit peer namespace, only after complete page validation;
8. advance the cursor only after import succeeds;
9. after a complete page, call Store::replace_remote_ref_snapshot once; that transaction rechecks every snapshotted head exists locally, compares the whole peer/context namespace, and atomically deletes stale names plus inserts/updates supplied names; and
10. return the checked report without invoking any local-ref mutation API.

Synchronization never writes local refs. Tests compare exact before/after local snapshots when no independent local writer runs. A concurrent local append or branch change is allowed to proceed and is neither overwritten nor misreported as a synchronization failure.

The public report contains pages, received, inserted, already_present, and remote_refs_updated. Counters use checked addition. remote_refs_updated counts inserted, changed, and deleted rows relative to the old peer/context namespace. An identical replacement returns zero.

Store::replace_remote_ref_snapshot accepts at most 256 sorted unique local advertised refs, rejects other contexts/namespaces and absent targets, and uses one IMMEDIATE transaction. It cannot address local refs or another peer namespace.

A malformed or invalid page causes no mutation for that page and prevents final remote-ref replacement. Earlier fully verified page events may remain as unreachable immutable orphans. OA-04 does not claim one transaction spans HTTP requests. Retry is safe because event import is idempotent and remote refs remain on the last fully completed snapshot.

An empty remote ref snapshot is valid: no bundle pages are needed, and the final atomic replacement removes prior remote refs for that peer/context while local refs remain unchanged.

## Public API shape

The implementation exposes documented checked values rather than raw header strings:

- TokenSource: explicit environment or file source;
- PeerEndpoint: validated plain-HTTP IP-literal base endpoint plus exposure acknowledgement;
- TransportLimits: checked server/client transport bounds no greater than hard maxima;
- PullLimits: checked page event/byte and total page bounds;
- SyncServerConfig, SyncServer, and its local address/exposure-warning/serve-until lifecycle;
- PullClientConfig, PullClient, and PullReport; and
- Store::replace_remote_ref_snapshot for the final peer-namespace transaction.

Secret-bearing types implement redacted Debug, do not implement Display, serialization, or secret-returning accessors, and zeroize retained plaintext on drop. Server/router internals and protocol JSON structures remain crate-private unless a testable non-secret checked value requires exposure.

## Errors and stable wire failures

OA-04 adds a separate non-secret SyncError/SyncResult taxonomy for invalid config, token source, endpoint, authentication, transport, timeout, protocol, pagination conflict, response/request limit, exposure acknowledgement, store failure, and internal failure. It does not add network cases to OA-01 ContractError.

Public Display text contains no token, header, URL, host response, arbitrary JSON, event payload, OS error, SQL, source path, or provider output. Store errors are mapped to safe protocol codes. Success never embeds a server error string.

Authenticated wire errors contain only protocol version, stable code, and request ID. Authentication uses authentication_failed/401. Other fixed mappings are malformed_request/400, unsupported_version/400, not_found/404, method_not_allowed/405, limit_exceeded/413, pagination_conflict/409, timeout/408, unavailable/503, and internal/500. A request-target excess uses HTTP 414 with limit_exceeded, and a parsed-header excess uses HTTP 431 with limit_exceeded. The non-waiting concurrency limit uses unavailable/503. All status/code shapes are frozen in protocol golden tests.

## I/O and failure matrix

- Missing/duplicate/malformed/wrong auth: identical generic 401; no store access.
- Unknown route or method after valid auth: stable 404/405; no body echo.
- Header/target/body/concurrency excess: rejected before protocol parsing or mutation.
- Unknown/pending context, bad requested head, plan overflow, or corruption: fail closed.
- Ref moves during transfer: selected requested heads and pages unchanged.
- Malformed envelope, Bundle, event, signature, parent, order, context, cursor, or fingerprint: page not imported.
- Truncated/chunked/false-length/slow/over-limit response: bounded failure; final refs unchanged.
- Redirect/proxy environment: no follow and no proxy use.
- Timeout then retry: duplicate events count already present; only final successful completion replaces remote refs.
- Server routes: read only; no local-ref/remote-ref mutation and no provider invocation.
- Client import: remote namespace only; local refs checked unchanged.
- Commit ambiguity from the store: surface safe failure and require idempotent retry.

## Test traceability

Required approved matrix rows retain their IDs:

- 04-A01 loopback_and_auth_matrix: loopback default, non-loopback dual acknowledgement/warning, one-header auth matrix, generic 401.
- 04-A02 token_non_disclosure: Debug/Display/error/server/client captures and repository scan contain no credential.
- 04-L01 transport_limit_matrix: exact/+1 target, parsed header count/value/aggregate, raw pre-header bytes, slow partial headers, body, concurrency, refs, heads, page events/bytes, response bytes, body-read and handler time.
- 04-S01 one_way_pull: missing parent-first history imports and creates only namespaced remote refs.
- 04-S02 local_ref_snapshot: all success/failure paths preserve exact local refs.
- 04-S03 converged_repeat: repeat has zero inserts and zero remote-ref changes.
- 04-S04 merged_history: fork/merge shared ancestry transfers uniquely parent-first.
- 04-S05 immutable_head_pagination: multiple pages remain tied to snapshotted heads while source refs move.
- 04-S06 invalid_page_rejection: malformed late page leaves that page and final refs unchanged; earlier valid orphan events are documented.
- 04-T01 hostile_server: truncated, chunked, contradictory/false Content-Length, redirect, proxy env, oversized chunks, and slow bodies stay bounded.
- 04-T02 timeout_retry: timeout is non-mutating for its page and retry converges idempotently.

Additional tests freeze canonical refs/page/error vectors, fingerprints/cursors, empty snapshots, stale-ref deletion counts, parser duplicate/unknown/noncanonical matrices, request mismatch, concurrent-known-set conflict, unauthorized route indistinguishability, no provider route, and restart behavior.

## File map

Planned implementation ownership:

- src/http.rs: transport facade and public checked configs;
- src/http/auth.rs: secret loading, redaction, request IDs, authentication;
- src/http/server.rs: listener policy, bounds, routes, stable errors;
- src/http/client.rs: endpoint validation, no-proxy/no-redirect HTTP/1 client, bounded reads;
- src/sync.rs: pull facade, protocol values, canonical parser/writer, state machine;
- src/store/sync.rs: deferred export-plan page and atomic remote-ref snapshot replacement;
- src/error.rs: additive non-secret SyncError only;
- tests/oa04_auth.rs, oa04_protocol.rs, oa04_sync.rs, oa04_transport.rs;
- tests/fixtures/oa04-*.json: exact canonical protocol fixtures;
- cargo-tree-oa04-features.txt, scripts/verify-oa04.sh, README, and this spec.

No schema version or migration is added. OA-01 event bytes and OA-03 Bundle v1 bytes remain frozen.

## Tasks and acceptance

1. Apply only the frozen OA-04 manifest pins/features and capture the locked feature graph.
2. Implement strict protocol types, fingerprints, cursors, canonical vectors, and parser adversarial tests.
3. Implement redacted token loading and constant-time header authentication.
4. Implement loopback-default bounded HTTP/1 server and stable error shapes.
5. Implement no-proxy/no-redirect bounded client transport and hostile-server tests.
6. Implement deterministic export-page snapshot and atomic whole-namespace remote-ref replacement.
7. Implement pull state machine, counters, idempotence, pagination, failure semantics, and local-ref invariants.
8. Run independent reviews for protocol state machine, auth/secrets, hostile network/resources, and dependencies/claims; record findings and hardening here.
9. Run locked build, rustfmt, strict Clippy, full tests, dependency verifier, OA-01/OA-02/OA-03 verifiers, and verify-oa04.sh.
10. Mark this spec done only after all evidence passes, then commit with exact subject OA-04: add authenticated pull synchronization.

## Change control and boundary

This specification resolves implementation detail inside the already-approved OA-04 objective and D-04-01 dependency direction. Changing versions/features, adding TLS/DNS/proxy discovery, altering Bundle v1, adding server mutation/provider routes, moving local refs, trusting remote admission, deleting earlier valid pages on late failure, or claiming confidentiality/truth/consensus requires explicit review and normally new approval.

Option B remains blocked until OA-07 records Option A complete with direct A1-A8 evidence.
## Freeze review evidence

The dependency plan, probe, verifier, and implementation specification were reviewed before any root-manifest change.

- Requirements traceability: approved OA-04 sections 19-23 and every 04-A/L/S/T matrix row are mapped without weakening; exact commit subject and Option B gate are retained.
- Dependency/supply chain: exact candidate pins compile and run on Rust 1.97; complete metadata/trees are recorded; root manifest stayed unchanged; TLS, cookies, compression, HTTP/2/3, multipart, system proxy discovery, alternate DNS, and agent-protocol packages are absent.
- Authentication/secrets: the freeze adds canonical token encoding, race-aware permission-checked file reads, zeroization/redaction, a domain-separated fixed-size constant-time BLAKE3 comparison, identical 401 behavior, and exact request-ID derivation.
- Protocol/pagination: arrays, fingerprints, cursor bytes, effective-known-head behavior, empty snapshots, remote-ref replacement, late-page orphan semantics, counters, and stable status/code mappings are explicit.
- Network/resources: exact/+1 request, parsed-header, raw pre-header, body, concurrency, page, response, and timeout bounds are frozen; the Axum facade limitation is addressed by a no-new-dependency TapIo pre-header timer/cap; Reqwest is HTTP/1-only, no-proxy, no-redirect, and incrementally bounded.
- Concurrency hardening: synchronization never writes local refs, but an unrelated concurrent local writer is not falsely treated as a sync mutation or failure.
- Independent delegated review attempts stalled without text and were cancelled; no unsupported approval is claimed from those delegates. The direct adversarial review and executable dependency evidence are the recorded freeze basis.

Freeze verdict: ready for implementation from baseline 93d1ca122eec25d64d9d38352faf87900d3bef30. Review loop iteration remains zero because no implementation exists yet.
