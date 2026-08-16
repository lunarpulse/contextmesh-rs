---
title: 'OA-05 Provider Recording Boundary and CLI'
type: 'implementation-spec'
created: '2026-08-17'
status: 'done'
approved_plan: '../planning-artifacts/oa-02-oa-07-detailed-execution-plan.md'
decision_record: '../planning-artifacts/oa-02-oa-07-decision-record.md'
dependency_plan: './oa-04-dependency-plan.md'
baseline_commit: 'b593617'
review_loop_iteration: 1
option_b_gate: 'blocked-until-OA-07-A1-A8'
---

# OA-05 Provider Recording Boundary and CLI

## Intent

OA-05 exposes provider recording and a stable automation CLI. It records exact
caller-selected ancestry as OA-01 signed events in the OA-02/OA-03 store. It
makes no semantic-selection, A2A/ACP protocol-compliance, exactly-once
delivery, or encryption-at-rest claim. Provider effects are external and
opaque; the package records requests and linked results, nothing more.

This package does not change OA-01 wire bytes, OA-02 schema, OA-03 Bundle v1,
or the OA-04 protocol. It does not add TLS, discovery, revocation, broader
workspace authorization, remote execution, or provider truth claims.

## Recorded decisions

### D-05-01 — opaque local signing-key custody (approved 2026-08-16)

OA-05 may persist a 32-byte Ed25519 seed in a caller-selected local file
solely for restart-safe signing. Private material is never returned through
public values, printed, logged, command-JSON encoded, stored in Turso,
bundled, or synchronized. Creation is atomic: a same-directory temporary file
with create-new semantics and Unix mode 0600, file sync, an atomic
create-if-absent publish step (hard-link then remove the temporary, which
fails if the target exists), and a directory sync where supported. Symlink
targets and group/other-accessible files are rejected; only the explicit
repair operation changes permissions. Non-Unix platforms without equivalent
stable file identity, permission, and atomic no-replace operations reject
key-file creation and loading rather than claim race-safe support. No
encryption-at-rest claim is made. Bearer-token files (OA-04 token1_ format)
follow the same atomic creation rules; OA-04 read-only loading is unchanged.
Key and token paths are local runtime configuration and are ignored by source
control.

### D-04-01 activation for OA-05

The D-04-01 probe record in oa-04-dependency-plan.md already compiled and
exercised Clap 4.6.6 (derive, std, help, usage, error-context) and the Tokio
process/signal paths on pinned Rust 1.97. OA-05 activates exactly:

- Clap =4.6.6, defaults off, features derive, std, help, usage,
  error-context;
- Tokio =1.53.1 direct normal features extended with process and signal
  (used by the bounded provider subprocess and serve shutdown), joining the
  OA-04 net, rt, sync, time set. Dev Tokio additionally retains macros.

No other direct dependency or feature is added. The post-edit locked feature
graph is captured as cargo-tree-oa05-features.txt, and the earlier verifier
dependency guards are updated to this manifest exactly as OA-04 did.

## Frozen constants and limits

| Constant | Value |
|---|---:|
| CLI schema_version | 1 |
| invocation ID | inv1_ + 22 unpadded base64url chars (16 random bytes) |
| provider input canonical bytes | <= 1,048,576 (OA-01 payload limit) |
| provider response canonical bytes | <= 1,048,576 |
| JSONL line | <= 2 MiB (2,097,152 bytes) |
| demo ancestry events per invocation | <= 1,024 |
| PROVIDER_EXECUTION_TIMEOUT | 30 seconds |
| provider error detail | <= 1,024 UTF-8 bytes after sanitization |
| pending/detached query results | <= 1,024, fail closed beyond |

Sanitization replaces ASCII control characters (U+0000-U+001F, U+007F) with
U+FFFD and truncates at a char boundary to the byte cap. Limits are checked
with overflow-safe arithmetic before allocation, subprocess writes, or store
mutation.

## Key and token custody API

src/crypto.rs gains, with redacted Debug and no secret-returning accessor:

- SigningIdentity::from_seed(seed: [u8; 32]) — production constructor; the
  supplied copy is zeroized after key construction. from_fixture_seed remains
  doc(hidden) test-only.
- generate_key_file(path) -> Result<AuthorId, KeyFileError>: generate with OS
  entropy, atomically create the seed file (never overwrite, never follow
  symlinks, mode 0600), and return only the public author identity.
- load_key_file(path) -> Result<SigningIdentity, KeyFileError>: the OA-04
  token-file discipline — pre-open symlink metadata, opened-handle metadata,
  post-open path metadata, matching device/inode at all three observations,
  exact 32-byte length, mode with no group/other bits — then zeroize the
  read buffer. Identity is stable across restarts.
- repair_key_file_permissions(path) -> Result<(), KeyFileError>: explicit
  repair to mode 0600 after the same identity checks; never creates files.
- generate_token_file(path) -> Result<(), KeyFileError>: atomically create an
  OA-04-format token1_ file under the same no-replace/no-symlink/0600 rules.

KeyFileError is non-secret: Unavailable, AlreadyExists, InsecurePermissions,
Malformed, Unsupported. No variant or Display text carries the path, seed, OS
error, or file bytes. Key/token files are never written by any other command,
never bundled, and never synchronized.

## Provider contract

Object-safe, no new dependency:

    pub trait Provider: Send + Sync {
        fn invoke<'a>(
            &'a self,
            invocation: &'a InvocationContext,
        ) -> Pin<Box<dyn Future<Output = ProviderOutcome> + Send + 'a>>;
    }

InvocationContext carries exactly: context (ContextId), branch head selected
by the caller (selected_head: EventId), deterministic parent-first ancestry
(Vec<SignedEventV1>, OA-03 limits), the committed request event id
(request_event_id), the random invocation_id, and the opaque input Value.
ProviderOutcome is either an opaque JSON response Value (canonical size
checked <= 1,048,576 bytes before recording) or a declared failure with a
stable non-secret code and sanitized bounded detail.

Provider failure codes are frozen: provider_declared, provider_transport,
provider_malformed, provider_timeout, limit_exceeded, internal. Only
provider_declared is provider-supplied data; the rest are synthesized by the
boundary.

## Recording sequence

record_invocation(store, identity, request, provider) performs exactly:

1. Validate context, branch name, expected selected head, and input size.
2. Project deterministic ancestry under OA-03 limits; fail closed on error.
3. Generate the invocation ID from fallible OS entropy.
4. Sign and admit agent.request with sole parent selected_head and the frozen
   payload, CAS-moving branch expected_head -> request in one transaction.
5. Only after that commit, invoke the provider with the pre-request ancestry
   and exact metadata. No transaction or lock is held across invocation.
6. Success: sign agent.response with sole parent request_event_id.
7. Declared/transport/malformed/timeout failure: sign agent.error with sole
   parent request_event_id and sanitized bounded data.
8. Admit the result with CAS branch request -> result.
9. If the branch moved (StaleHead), admit the same result event with
   RefMutation::None so the linked immutable result is retained detached, and
   return ProviderError::PostExecutionConflict { result, current_head }.

Frozen payload field sets (OA-01 canonical JSON, no extra fields):

- agent.request: {"input": Value, "invocation_id": "inv1_...",
  "selected_head": "evt1_..."}
- agent.response: {"invocation_id": "inv1_...", "response": Value}
- agent.error: {"detail": sanitized string, "error_code": code,
  "invocation_id": "inv1_..."}

The public report carries request_event_id, invocation_id, outcome kind,
result event id, and branch-moved flag. Crash windows are documented and
tested: before request commit no invocation happened; after commit before
invoke the pending request is recoverable; an external side effect before
result commit may be duplicated by retry; a result admitted before its ref
CAS is detached and recoverable/mergeable. OA-05 never claims exactly-once.

## Crash-window recovery queries

src/store/invocation.rs adds two bounded read-snapshot queries:

- Store::pending_invocations(context, branch) -> requests on the branch
  ancestry (OA-03 projection) that have no agent.response/agent.error child
  whose sole parent is the request. Fail closed past 1,024.
- Store::detached_results(context, branch) -> agent.response/agent.error
  events in the context that are not reachable from the current branch head.
  Fail closed past 1,024.

Both are read-only, non-secret, and use checked limits.

## Command provider (subprocess boundary)

CommandProvider runs a caller-supplied local command per invocation:

1. write one canonical JSONL invocation document to child stdin, close stdin;
2. read exactly one JSONL response line bounded to 2 MiB + 1 and
   PROVIDER_EXECUTION_TIMEOUT; kill on timeout;
3. require protocol_version 1, the exact invocation_id, exact field sets, and
   either one response Value or one failure object;
4. check response canonical size before returning;
5. map every spawn/write/read/timeout/malformed/oversized case to the frozen
   failure codes with sanitized detail; never capture child stderr into
   results (bounded stderr is drained and discarded); never execute a shell.

The invocation document fields are frozen: {"ancestry": [event JSON...],
"context": "ctx1_...", "input": Value, "invocation_id": "inv1_...",
"protocol_version": 1, "request_event_id": "evt1_...", "selected_head":
"evt1_..."}. Tokio process is used so timeout and I/O bounds are enforced
without blocking the runtime.

## JSONL demo provider (demo_agent)

src/bin/demo_agent.rs implements the loopback-free stdin/stdout JSONL demo:

- one JSON object per line, hard cap 2 MiB per line including newline; an
  oversized line yields one failure response and resynchronizes at the next
  newline without a panic or unbounded buffer;
- input is validated strictly: protocol_version 1, exact field set, typed
  context/head/request IDs, inv1_ invocation ID, ancestry <= 1,024 valid
  canonical event values, any opaque input;
- success responds exactly once:
  {"invocation_id": "...", "ok": true, "protocol_version": 1,
   "response": {"demo": {"echo": <input>}}} — the opaque input is echoed only
  under the demo namespace;
- failure responds exactly once with ok=false, a stable code, sanitized
  bounded detail, and the parsed invocation_id or null;
- every response flushes stdout; no environment, secrets, paths, or tool
  execution; no A2A/ACP claim; malformed input never panics.

## CLI contract

src/cli.rs (Clap derive, exact frozen pins) parses and dispatches
contextmesh. Every command emits exactly one canonical RFC 8785 JSON document
to stdout and nothing else; warnings go to stderr without secrets.

Success: {"command": NAME, "ok": true, "result": {...},
"schema_version": 1}. Failure: {"command": NAME, "error": {"code": CODE,
"details": {...non-secret}}, "ok": false, "schema_version": 1}.

Exit classes (frozen): 0 success; 2 usage/config (Clap errors, unreadable
config); 3 validation (bad IDs/names/JSON/over-limit input); 4 conflict
(stale CAS, ref conflicts); 5 authentication/key/token failures; 6 not found
(context/event); 7 provider failure or post-execution conflict; 8
transport/protocol/timeout (sync); 9 database/internal.

Commands (all take --db PATH except key/token generate):

- key generate --file PATH
- token generate --file PATH
- key repair-permissions --file PATH
- context create --db PATH --key-file PATH --branch NAME
- context join --db PATH --context CTX --expected-genesis EVT --author AUTHOR
  (repeatable)
- context authorize --db PATH --context CTX --author AUTHOR
- append --db PATH --key-file PATH --context CTX --branch NAME
  --expected-head EVT --kind KIND (--payload-file PATH | --payload-stdin)
- branch create --db PATH --context CTX --name NAME --from-head EVT
- merge --db PATH --key-file PATH --context CTX --branch NAME
  --expected-head EVT --parent EVT (repeatable, 2-64)
  (--payload-file PATH | --payload-stdin)
- show event --db PATH --id EVT
- show projection --db PATH --context CTX --head EVT (repeatable)
- show refs --db PATH --context CTX [--peer NAME]
- bundle export --db PATH --context CTX --head EVT (repeatable)
  [--known-head EVT (repeatable)] --out PATH
- bundle import --db PATH --peer NAME --file PATH
- invocation pending --db PATH --context CTX --branch NAME
- invocation detached --db PATH --context CTX --branch NAME
- verify --db PATH
- invoke --db PATH --key-file PATH --context CTX --branch NAME
  --expected-head EVT (--input-file PATH | --input-stdin)
  --provider-command PATH [--provider-arg ARG (repeatable)]
- serve --db PATH --token-file PATH --listen IP:PORT --ready-file PATH
  [--acknowledge-non-loopback-plaintext]
- sync --db PATH --peer NAME --url URL --token-file PATH --context CTX
  [--acknowledge-non-loopback-plaintext]

Secrets come only from files or the environment and are never echoed to
stdout/stderr or result JSON. Payload/input larger than trivial arguments
uses files or stdin. serve writes the bound address to the ready file
atomically after binding, prints the fixed non-loopback warning to stderr
when acknowledged, and shuts down on SIGINT/SIGTERM. Every command's success
and failure JSON bytes and exit codes are snapshotted in a checked-in golden
fixture.

## Errors

src/error.rs adds non-secret KeyFileError and ProviderError taxonomies.
ProviderError: InvalidConfig, Validation, ProviderDeclared, ProviderTransport,
ProviderMalformed, ProviderTimeout, LimitExceeded, PostExecutionConflict
{ result: EventId, current_head: Option<EventId> }, Store(StoreError),
Internal. Display text contains no provider output, payload, path, command,
secret, OS error, or SQL. OA-01/OA-04 error enums are unchanged.

## File map

- src/crypto.rs: seed custody, key/token file generation, load, repair;
- src/provider.rs: Provider trait, InvocationContext, ProviderOutcome,
  CommandProvider, record_invocation, reports;
- src/store/invocation.rs: pending/detached recovery queries;
- src/cli.rs: parser, dispatch, canonical JSON rendering, exit mapping;
- src/bin/contextmesh.rs, src/bin/demo_agent.rs: real entry points;
- src/error.rs, src/lib.rs: additive enums/exports;
- tests/oa05_keys.rs, oa05_provider.rs, oa05_cli.rs, oa05_jsonl.rs;
- tests/fixtures/oa05-cli-golden.json: stdout/exit snapshots;
- cargo-tree-oa05-features.txt, scripts/verify-oa05.sh, README, this spec.

No schema version or migration is added. scripts/demo.sh remains the OA-06
sentinel.

## Test traceability

Approved rows retained without weakening:

- 05-K01 persistent_identity: generate, restart process, reload, same author;
- 05-K02 key_filesystem_matrix: existing target, symlink, group/other perms,
  wrong length, directory, repair flow, no overwrite;
- 05-K03 secret_non_disclosure: seed/token bytes absent from stdout, stderr,
  result JSON, Debug, errors, and repository scan;
- 05-P01 request_before_call: provider observes the committed request event;
- 05-P02 result_matrix: linked response, declared error, transport failure,
  malformed output, timeout — exact payloads and sole-parent links;
- 05-P03 ancestry_fixture: deterministic parent-first ancestry in the
  invocation document;
- 05-P04 post_execution_conflict: concurrent branch movement retains the
  detached result and returns result ID plus current head;
- 05-P05 pending_detached_queries: crash-window recovery queries;
- 05-C01 cli_snapshot_matrix: every command success/failure JSON and exit;
- 05-C02 cli_restart: full CLI flow survives process restart with stable
  identity and store;
- 05-J01 jsonl_adversarial: malformed/oversized exact/+1 lines, bad IDs,
  duplicate/missing fields, flush, no panic, no environment echo.

Additional tests freeze the invocation-document vector, payload field-set
rejection, input/response exact/+1 canonical bounds, provider timeout kill,
stderr discard, and the no-transaction-across-invocation guarantee.

## Tasks and acceptance

1. Record D-05-01 and the D-04-01 OA-05 activation; freeze this spec.
2. Apply only the frozen Clap/Tokio manifest delta; capture the locked
   feature graph as cargo-tree-oa05-features.txt.
3. Implement key/token custody with hostile-filesystem tests.
4. Implement Provider trait, recording sequence, recovery queries, and
   CommandProvider with adversarial doubles and crash-window tests.
5. Implement CLI parse/dispatch/JSON/exit mapping with snapshot fixture.
6. Implement demo_agent with the JSONL adversarial matrix, and reconcile
   README wording: OA-01's "private keys are neither serializable nor
   exposed" remains true for public values, wire, logs, JSON, Turso, bundles,
   and synchronization; D-05-01 adds only the opaque local seed file, and the
   README states exactly that without changing any OA-01 wire claim.
7. Run independent reviews for key custody/secrets, provider/recording state
   machine, CLI JSON stability, and JSONL/resource bounds; record findings
   and hardening here.
8. Run locked build, rustfmt, strict Clippy, full tests, dependency audit,
   OA-01..OA-04 verifiers, and verify-oa05.sh.
9. Mark this spec done only after all evidence passes, then commit with exact
   subject "OA-05: add provider recording and CLI".

## Change control and boundary

Changing frozen payload fields, error codes, exit classes, CLI grammar,
dependency pins/features, or D-05-01 custody rules, adding provider protocol
compliance or exactly-once claims, holding transactions across invocation,
executing shells/tools, or touching OA-01..OA-04 wire behavior requires
explicit review and normally new approval. Option B remains blocked until
OA-07 records Option A complete with direct A1-A8 evidence.

## Freeze review evidence

An independent delegated review was launched against the approved plan,
decision record, and test matrix but stalled without producing text because
its provider returned an authentication/quota error; it was cancelled and no
approval is claimed from it. The recorded freeze basis is the direct
adversarial review below.

- Requirements traceability: plan sections 24-28 map completely — D-05-01 is
  recorded before key-custody implementation as the approved sequence
  requires; the recording sequence steps 1-9, the no-transaction-across-
  invocation rule, and all four crash windows are restated verbatim; every
  listed CLI command, the JSON envelope, the proposed exit classes 0-9, the
  secrets-from-files rule, and the snapshot requirement are frozen; the
  JSONL contract and demo-namespace echo rule are frozen; all eleven approved
  matrix rows 05-K01..05-J01 are retained by ID without weakening.
- Scope check: no element exceeds the approved objective — no A2A/ACP
  compliance, no exactly-once claim, no shell/tool execution, no encryption
  claim, no revocation, no remote execution, and no OA-01..OA-04 wire change.
  The added 1,024-event ancestry cap and 30-second execution timeout are
  strictly tighter bounds consistent with the plan's own limit discipline.
- Dependency check: OA-05 activates exactly the already-probed Clap 4.6.6
  feature set and Tokio process/signal from the frozen D-04-01 record; no new
  dependency, feature, or unprobed pin is introduced.
- Boundary check: key/token custody follows D-05-01 atomicity (temp file,
  fsync, no-replace publish, dir sync), 0600, symlink rejection, and
  explicit-repair-only permission changes; provider failure codes, payload
  field sets, and the CLI grammar are frozen for snapshot stability.

One gap found and fixed during this review: the plan gate requires README
wording to be reconciled without wire change; task 6 now records that exact
obligation.

Freeze verdict: ready for implementation from baseline b593617. Review loop
iteration remains zero because no implementation exists yet. Option B stays
blocked until OA-07 records Option A complete with direct A1-A8 evidence.

## Implementation review and evidence (iteration 1)

An independent delegated implementation review was launched but exhausted its
action budget without producing findings text; no approval is claimed from it.
The recorded basis is the direct adversarial review below plus the executable
verification chain.

Direct review findings and resolutions:

1. Dependency delta beyond the plan letter, within its principle: D-04-01
   froze "package-minimal features as actually needed"; the bounded subprocess
   pipe exchange needs Tokio io-util (AsyncReadExt/AsyncWriteExt), so the
   normal Tokio feature set is io-util, net, process, rt, signal, sync, time.
   Clap =4.6.6 with exactly derive/std/help/usage/error-context is activated as
   frozen. No other dependency changed; forbidden-capability audit still
   passes (no TLS, HTTP/2/3, cookies, compression, resolvers, shells).
2. Fixed: key-file loading ordered permission bits before symlink identity, so
   a 0777 symlink reported InsecurePermissions instead of Malformed. Identity
   checks now precede permission classification (05-K02 evidence).
3. Fixed: CommandProvider held its child stdin/stdout pipe ends open across
   child.wait(), deadlocking well-behaved children that wait for stdin EOF
   (observed as a 30-second provider_timeout against demo_agent). Both ends
   are now closed as soon as each direction completes.
4. Fixed: CLI failure documents were first written to stderr while the frozen
   contract places exactly one JSON document on stdout for both outcomes;
   warnings only go to stderr. The snapshot matrix now covers both.
5. Fixed: ProviderError::Store(StaleHead/RefMissing) mapped to exit 7 instead
   of the frozen conflict class 4; ContextUnknown mapped to 6. Corrected.
6. Fixed: sanitizer boundary accounting truncated ASCII detail to 1,021 bytes
   and clippy-level quality issues (filter_map->filter, unused imports).
7. Verified by construction and tests: record_invocation validates input size
   before projection, commits agent.request via CAS before the provider call,
   never holds a transaction across the call, records exact frozen payload
   field sets, checks response size before recording, and retains detached
   results on post-execution conflict (05-P01..P05). Recovery queries fail
   closed past 1,024 rows per kind in one read snapshot.
8. The frozen provider-timeout kill test is included at the real 30-second
   constant (/bin/sleep 60 is killed and bounded); the suite runtime cost
   follows the existing long-running OA-03 projection precedent.
9. The earlier-verifier guards asserting that provider/CLI surfaces remain
   deferred were reduced to the OA-06 demo sentinel, following the OA-04
   precedent of each owning package updating predecessor manifest/surface
   expectations; OA-01/OA-03/OA-04 fixtures remain checksum-frozen.

Verification evidence (pinned Rust 1.97.0):

- bash scripts/verify-oa05.sh: exit 0, 73 ok checkpoints — exact pins/features
  and forbidden surfaces absent, locked feature graph matches
  cargo-tree-oa05-features.txt, OA-01/OA-03/OA-04 fixtures unchanged, locked
  build, rustfmt, strict Clippy -D warnings, full workspace tests, OA-05
  keys/provider/CLI/JSONL matrices, OA-06 sentinel, and the chained
  OA-01/OA-02/OA-03/OA-04 and D-04-01 probe verifiers.

Freeze verdict updated: implementation complete. Option B remains blocked
until OA-07 records Option A complete with direct A1-A8 evidence.
