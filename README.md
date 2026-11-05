# contextmesh

> Integrity-preserving context records for AI agents — and an honest,
> pre-registered experiment in machine attention.

## What this project is trying to do

AI agent sessions produce enormous amounts of context — tool calls,
decisions, results — but the record of what happened usually lives inside
one platform and must be taken on trust. contextmesh explores the opposite
extreme: **every meaningful event is a signed, canonically-encoded,
tamper-evident artifact** that independent parties can verify and exchange
themselves, with narrow, explicitly-frozen claims and no magic.

The work runs on two tracks:

- **Option A — the substrate (complete).** Signed events, a transactional
  local store, deterministic DAG projection, strict bounded bundles,
  authenticated pull-only synchronization between nodes, and provider-call
  recording. Deterministic, offline-reproducible, and deliberately minimal
  in what it claims.
- **Option C — the attention-ledger experiment (closed 2026-11).** On top of
  the substrate: can we reconstruct which past events actually mattered to
  a task, and can a cross-family panel of LLMs agree on that well enough
  to form an evaluation gold standard? Pre-registered, blind, and gated.
  The honest answer turned out to be **no** — and the negative result is
  shipped with the same rigor as a positive one would have been.

## What Option C found (and why we publish it)

- **OC-01…OC-03** built the deterministic machinery: an outcome ledger,
  attribution marks (caller-supplied, never causal evidence), and a
  positive-only entity-graph prior — all frozen and test-pinned.
- **OC-04** ran the human-question experiment at full scale: 192 real agent
  sessions → 3,979 candidate events, labeled blind by four annotators
  (attempt 1) and a three-engine cross-family panel spanning GLM, GPT, and
  DeepSeek (attempt 2), under a frozen codebook with uncertainty budgets
  and laziness gates. Both independent attempts **failed the
  pre-registered inter-annotator agreement gates** (cross-family κ
  0.42–0.54 against a required 0.60 binary) while passing every integrity,
  completeness, and effort gate. Conclusion, recorded fail-closed:
  **cross-family LLM consensus does not form a gold standard at production
  scale** — the pilot's founder-audited κ did not generalize.
- The **Thorn** negative-knowledge suppression experiment was closed
  evidence-archived: its go-gate required human labels to demonstrate
  incremental benefit, a premise voided by the OC-04 outcome. It ships
  disabled in every deployment configuration.
- **OC-05**, the release and claim gate, ran its full twelve-checkpoint
  verification (freeze identity, offline reproduction, dependency-closure
  audit, privacy scan, replay determinism, claim audit against the
  pre-registered non-claims) to **ALL PASS** on the final artifact.
- Privacy discipline throughout: raw session transcripts and labels were
  never committed — only HMAC-keyed identifiers and SHA-256 digests are in
  git; the raw corpus stays on the machines that produced it.

If you are researching LLM annotation agreement, evaluation gold
construction, or agent-context provenance, the protocol artifacts
(pre-registration, codebook, dispatchers, gate scripts, and both attempt
reports) are the reusable part of this negative result.

## Repository layout

| Path | Contents |
|---|---|
| `src/`, `tests/` | Option A core crate and its 59 test suites |
| `contextmesh-salience/` | OC-01…OC-05: ledger, attribution, prior, annotation tests |
| `oc04-gold/` | Annotation protocol machinery (raw labels are local-only) |
| `scripts/` | Deterministic verification gates and the two-node demo |
| `_bmad-output/` | Frozen specs, plans, and evidence records |
| `cargo-tree-*.txt` | Captured dependency feature graphs per option |

## Quick start

    bash scripts/bootstrap-rust.sh
    . "$HOME/.cargo/env"
    cargo build --workspace --locked
    cargo test --workspace --locked
    bash scripts/demo.sh        # two-node signed-event sync demo
    bash scripts/verify-oa07.sh # full Option A release gate

Details and prerequisites are in "Toolchain and verification" below.

## What is implemented (Option A)

currently implements **OA-07**: Option A is complete — the
frozen OA-01 signed-event contract, OA-02 transactional local store, OA-03
semantic-free DAG operations, deterministic projection, strict bounded
Bundle v1 transfer, full-store integrity verification, OA-04 authenticated
pull-only HTTP/1 synchronization, OA-05 provider recording with a stable
automation CLI, OA-06 reproducible two-node demonstration, and the OA-07
release evidence with the A1-A8 completion verdict. OA-00's Rust
1.97/Turso baseline remains intact.

**OC-01** adds the `contextmesh-salience` workspace package: a frozen
`OutcomeLedgerV1` that records a caller-supplied outcome attempt as a
domain-signed, canonically-encoded artifact with bounded import/export and
DAG-anchored provenance verification. Its capability is limited to
artifact integrity and provenance recording — see "OC-01 claims and
non-claims" below. It is a one-way path dependency of the core package
and adds no external dependency.

## OC-01 claims and non-claims

**Claims (bounded):** exact v1 schema/order/bounds; domain-separated ID and
signature; store-aware fail-closed issuance; same-context DAG verification;
freshness (`stale-input`) checks; bounded regular-file import/export; stable
non-disclosing error categories; committed golden/adversarial vectors; full
regression of Option A/B surfaces.

**Non-claims:** no causal attribution, prior grounding, selection utility,
comprehension, cost accuracy, or outcome-quality claim (C2–C5). Attribution
marks are caller-supplied candidates, never causal evidence. OC-01 does not
authorize OC-02; the separate P1 preregistration gate still applies.
Verify with `bash scripts/verify-oc01.sh` (offline, non-recording); see
`_bmad-output/verification-artifacts/oc-01-evidence.md` for the four-layer
evidence record.

## Toolchain and verification

The project pins Rust **1.97.0**, edition **2024**, rustfmt, and Clippy in
rust-toolchain.toml. Install or refresh the user-local toolchain without root:

    bash scripts/bootstrap-rust.sh
    . "$HOME/.cargo/env"

Verify the released Option A state from the repository root:

    cargo build --workspace --locked
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo test --workspace --locked
    bash scripts/verify-oa07.sh

verify-oa07.sh is the deterministic non-recording release gate: it asserts
a clean worktree with committed evidence matching HEAD, the pinned
toolchain and native prerequisites with no overrides, exact dependencies
and the 320-crate closure with permissive licenses, the full OA-00 through
OA-06 verifier chain, a fresh-target offline repetition of the build,
Clippy, tests, and demo, secret and runtime-artifact scans, and the eight
audit layers plus the A1-A8 evidence matrix and Always/Never consistency
table recorded in the evidence artifacts.

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

## Authenticated pull synchronization

OA-04 exchanges OA-03 immutable signed events and unsigned advertised local
refs between independent stores over authenticated plain HTTP/1. A bearer
credential is exactly 32 random bytes rendered as token1_ plus canonical
unpadded base64url, loaded only from an explicit environment variable or a
permission-checked regular file; the server retains only a domain-separated
BLAKE3 hash of the exact Authorization header bytes and compares fixed-size
hashes. Every failure shape is a stable non-secret canonical JSON error with a
random-seeded request ID.

Peers are absolute http:// IP-literal endpoints with explicit ports; hostnames,
DNS discovery, HTTPS, proxies, and redirects are rejected or disabled. Loopback
is the default; non-loopback plaintext requires an explicit acknowledgement and
carries a fixed no-confidentiality warning. Request targets, parsed headers,
raw pre-header bytes, bodies, concurrency, pages, and responses are bounded
with exact frozen limits and independent timeouts; partial-header slowloris
traffic is cut at the accept layer.

GET /v1/refs returns one canonical signed-order ref snapshot with a BLAKE3
fingerprint. POST /v1/bundles/export returns deterministic parent-first
Bundle v1 pages bound by cursor to an immutable ancestry-difference plan.
PullClient imports each fully validated page through the OA-03 admission path
and, only after the complete transfer, atomically replaces the selected
peer/context remote-ref namespace. Synchronization never writes local refs,
and earlier verified page events may remain as unreachable immutable orphans
when a later page fails.

OA-04 adds only Axum 0.8.9 (http1, json, tokio) and Reqwest 0.13.4 (json),
plus the approved Tokio net/rt/sync/time features, per the frozen D-04-01
record. Plain HTTP provides no confidentiality or server identity.

## Provider recording and CLI

OA-05 records provider invocations as ordinary signed events. An invocation
signs and CAS-appends agent.request, invokes the provider only after that
commit, then signs a linked agent.response or agent.error with the request as
its sole parent and CAS-moves the branch. No transaction is held across the
provider call; if another writer moves the branch first, the linked result is
retained detached and the caller receives a post-execution conflict with the
current head. Crash windows are explicit: pending requests and detached
results are queryable, and no exactly-once claim is made.

The CommandProvider runs a caller-selected local program (never a shell) over
bounded JSONL pipes with a 30-second kill timeout. demo_agent is the reference
JSONL provider: it validates one strict input line, echoes opaque input only
under the demo namespace, and never executes tools or touches the environment.
Private keys are never serializable or exposed in public values, wire, logs,
JSON, Turso, bundles, or synchronization; D-05-01 adds only an opaque local
seed file (atomically created 0600, symlink-rejecting, explicit-repair-only)
for restart-safe signing, with no encryption-at-rest claim.

The contextmesh CLI emits exactly one canonical JSON document per command with
frozen exit classes (0 success, 2 usage, 3 validation, 4 conflict, 5 auth, 6
not found, 7 provider conflict, 8 transport, 9 internal). Secrets come only
from files and are never echoed; the full snapshot matrix is
tests/fixtures/oa05-cli-golden.json. OA-05 activates the frozen D-04-01 Clap
4.6.6 pin and the Tokio process/signal/io-util features its code needs.

## Reproducible two-node demo

`bash scripts/demo.sh` drives two fully independent node runtimes A and B
exclusively through the CLI. It builds the locked binaries, generates fresh
OS-random keys and tokens (never fixtures), provisions B from A's exported
join descriptor, boots both pull daemons on `127.0.0.1:0`, and runs seventeen
stages: genesis pull without implicit local movement, explicit branch
creation, distinct request/response chains on each node, bidirectional
exchange with ref isolation, an explicit two-parent merge, converging
projections (equal counts and byte-identical exported event sequences on
both nodes, every ancestor exactly once), daemon stop/restart on
the same databases, full-store verification, idempotent zero-insert repeat
pulls, and an atomic rejection of a one-byte signature mutation. On success
it prints `demo: PASS ...` with public IDs and counts only (followed by a
`runtime kept at` notice when `OA06_DEMO_KEEP=1` is set).

The harness runs under bash strict mode in a private 0700 temporary runtime
root, polls daemon readiness with a hard timeout and liveness checks, and
cleans up recorded child PIDs with TERM, bounded grace, then KILL. On failure
it preserves the runtime and prints its path; on success it deletes the
runtime unless `OA06_DEMO_KEEP=1` is set. `OA06_DEMO_RUNTIME_ROOT` selects an
explicit runtime root (must be an absent or empty directory; created and
chmod 0700). Three test-only fault hooks
(`OA06_DEMO_READY_TIMEOUT_SECS`, `OA06_DEMO_TEST_SERVE_DELAY_SECS`,
`OA06_DEMO_TEST_CRASH_AFTER_READY`) delay or kill work for the lifecycle
tests in tests/oa06_demo.rs; they cannot bypass any assertion.

One frozen-engine constraint shapes the choreography: the embedded Turso
0.7.2 database allows exactly **one process per database file**, so a node's
daemon runs only while no local CLI command of that node touches its
database. The demo starts and stops each daemon around local operations;
stage 5 proves both daemons boot concurrently and stage 13 proves they reopen
the same databases on new ephemeral ports. A long-running node must
therefore serialize daemon serving and local CLI access in Option A.

## Network deployment guidance

Option A synchronization is authenticated **plaintext** HTTP/1 between
absolute loopback IP endpoints with explicit ports. It is designed for
single-host and controlled tunnel use: hostnames, DNS discovery, HTTPS,
proxies, and redirects are rejected or disabled, and non-loopback plaintext
requires an explicit acknowledgement while providing no confidentiality.
Cross-machine operation requires an operator-managed encrypted tunnel or VPN
(for example WireGuard) so the peers remain IP literals inside a private
address space; contextmesh itself provides no TLS, certificate management,
key rotation service, or discovery, and makes no confidentiality claim on
its own. A bearer token authorizes read-only history pulls only.

## How to participate

This is a research codebase with strict claim discipline — that discipline
is the product as much as the code. Good ways in:

1. **Reproduce the gates.** Everything runs offline from a clean clone:
   `bash scripts/bootstrap-rust.sh`, then `bash scripts/demo.sh`,
   `bash scripts/verify-oa07.sh`, `bash scripts/verify-oc01.sh`, and the
   OC-05 wrapper in `_bmad-output/implementation-artifacts/spec-oc-05-release-gate.md` §10.
   A gate that fails on your machine is a bug we want to hear about.
2. **Read the evidence chain** in `_bmad-output/verification-artifacts/` —
   every claim above has a four-layer evidence record, and the claim
   audits list what is *not* claimed.
3. **Challenge the negative result.** OC-04 closed INCONCLUSIVE after two
   pre-registered attempts. A third attempt would need a new frozen
   pre-registration *before* any data is touched — propose one and we
   will review the protocol, not the outcome.
4. **Extend the substrate.** Option B (semantic context selection) is
   unblocked but untouched; anything new must keep Option A's frozen wire,
   bounds, and claim discipline or go through explicit change control.
5. **Report issues** with reproduction steps — failed gates, panics on
   malformed input, doc/claim mismatches are all first-class bugs.

Note: the repository has **no license yet** — code reuse before a license
is chosen requires the maintainer's explicit permission.

## Claims, non-claims, and prohibited statements

Demonstrated by tests and the demo: event integrity (canonical hashing,
strict Ed25519 signatures, verified imports), transactional admission and
CAS ref movement, deterministic projection, tamper detection without repair,
restart persistence, authenticated pull exchange with remote-ref
namespacing, idempotent re-pull, and provider recording with explicit crash
windows.

Explicitly not claimed: payload truth or semantic relevance, authorization
consensus or membership truth (the local allowlist is append-only and
trusts its operator), confidentiality or encryption at rest, availability
or denial-of-service resistance beyond bounded inputs, exactly-once provider
delivery, revocation, Byzantine agreement, or multi-writer concurrency
across processes on one database.

Prohibited statements for this project include claiming A2A or ACP protocol
compliance, agent interoperability, semantic context selection, secure
cross-internet transport, secret protection beyond file permissions, or
"verified truth" of recorded content. Any future A2A/ACP mapping is a
non-compliant external adapter concern deferred past Option A; Option A
makes no protocol-compliance claim of any kind.


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

OA-04's exact locked feature graph is cargo-tree-oa04-features.txt, and its
frozen canonical protocol fixture is tests/fixtures/oa04-protocol-golden.json.
OA-05's exact locked feature graph is cargo-tree-oa05-features.txt.

## Deferred scope

- Option B: semantic context selection and handoff — unblocked by the
  complete OA-07 verdict; no Option B work has begun.

The release evidence is _bmad-output/verification-artifacts/
oa-07-release-evidence.md and the claim audit is oa-07-claim-audit.md in
the same directory. Any future Option B work must keep Option A's frozen
wire, bounds, and claim discipline or seek explicit approval.
