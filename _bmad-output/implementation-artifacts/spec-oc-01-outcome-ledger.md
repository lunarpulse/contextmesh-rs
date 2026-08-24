---
title: 'OC-01 OutcomeLedgerV1 Package Specification'
type: 'feature-package-specification'
created: '2026-08-21'
status: 'approved-for-implementation'
approved: '2026-08-21'
approved_by: 'Lunarpulse'
approval_source: 'Discord message 1540352346364842105'
review_loop_iteration: 4
branch: 'OC-AttentionLedger'
baseline_commit: 'a2135f73b957b2d9c34b6655b5c1f1cab2851de4'
source_spec: './spec-option-c-salience-provenance-layer.md'
decision_record: '../planning-artifacts/oc-00-5-founder-decision-record.md'
priority_plan: '../planning-artifacts/option-c-priority-and-gate-plan.md'
test_matrix: '../planning-artifacts/oc-01-test-traceability-matrix.md'
---

# OC-01 OutcomeLedgerV1 Package Specification

> **Approved for implementation** by Lunarpulse on 2026-08-21 after four
> independent review loops. This freezes the OC-01 v1 implementation contract
> and 90-row matrix but is not a C1 completion claim or authority to change an
> Option A/B wire. D-C-00 through D-C-10 remain controlling.

## 1. Intent, scope, and non-claims

OC-01 establishes the first production package in dependency order: a sibling
`contextmesh-salience` library crate and one strict, signed,
content-addressed, exported `OutcomeLedgerV1`. A ledger records a
caller-declared task outcome, provenance-qualified quality assessment, explicit
terminal EventId or `unterminated`, caller-recorded costs, an attempt tree, dead
ends, and caller-supplied attribution marks. Issuance and re-verification fail
closed against immutable Option A events and a canonical input-ref snapshot.

OC-01 proves only bounded artifact integrity and provenance recording. It does
**not**:

- infer a terminal event, task success, outcome quality, cost, or attribution;
- establish C2 causal load-bearing attribution (M3/M4 or otherwise);
- implement or establish C3 Salience Prior/Thorn utility;
- implement or establish C4 selection, ranking, comprehension, sufficiency,
  minimality, or task-performance utility;
- change or add a field to an Option A event, Option B receipt, selector,
  handoff, summary, bundle, store schema, or network wire;
- store an OC artifact in the Option A store or mutate Option A history.

All semantic values are caller declarations whose mechanism and configuration
provenance is signed. Structural verification authenticates the declaration;
it does not prove that the declaration is true.

## 2. Four-layer derivation record

### 2.1 Sources

| Source | Authority | OC-01 use |
|---|---:|---|
| approved Option C specification | frozen founder intent | C1 purpose, integrity boundary, non-claims |
| approved D-C-00..D-C-10 decision record | controlling decision | crate direction, domains, failures, bounds, snapshot, terminal/cost rules |
| approved Option C priority/gate plan | controlling sequence | P1 before attribution/prior/selection; P1 evidence boundary |
| `src/model.rs`, `src/crypto.rs`, `src/store.rs`, `src/receipt.rs` at baseline | primary implementation | public IDs, signing, verification, store/ref and receipt patterns |
| `tests/ob01_receipts.rs` and OA package conventions | primary executable pattern | deterministic fixture, strict/tamper/DAG/import-export tests |
| `graphify-out/GRAPH_REPORT.md` at `a2135f73` | structural map | canonicalization, crypto, receipt, and store integration seams |

No web source is required; this package is governed by local approved contracts
and repository APIs.

### 2.2 Reasoning

1. OC must remain outside the core, therefore workspace setup and one-way path
   dependency are the first dependency.
2. IDs, domains, errors, bounds, and strict JSON must exist before a body can
   have stable bytes.
3. Value types precede the body; the body precedes ID/signature envelope logic.
4. Pure structural parse/verify precedes store-aware issuance and DAG/context
   verification.
5. Input-ref capture and event verification precede import/export acceptance.
6. Golden and adversarial vectors precede any evidence or completion claim.
7. OC-02 remains blocked until both this package gate and the separate P1
   preregistration hash gate pass.

### 2.3 Conclusion derivation

| Evidence/question | Derivation | Specification conclusion |
|---|---|---|
| Core helpers for strict JSON and JCS are crate-private | a sibling crate cannot call them | implement OC-owned strict parser; use already-locked serde/JCS crates directly |
| Public crypto supports caller domains | `SigningIdentity::sign_domain_message` and `verify_domain_message` are public | reuse core signing key and strict Ed25519 verification; introduce no signature primitive |
| Public store reloads verified events and lists refs | `Store::event`, `list_local_refs`, `list_remote_refs` | verify all references and capture/compare canonical snapshots without schema changes |
| Public store does not expose an authorization query | admitted events were authorization-checked and `Store::event` revalidates their wire | treat admission as evidence for referenced events; authenticate the OC artifact signer by its domain signature without inventing a signer-allowlist rule |
| Root is currently a single package | a workspace makes `resolve.root` nullable and adds one local lock package | keep legacy release gates immutable in historical worktrees; measure current core closure by package-name reachability in the new OC gate |
| C2 is later | attribution marks cannot be treated as causal results | encode marks as caller-supplied candidates with mechanism provenance and explicit non-claim |

### 2.4 Invalidators

Pause implementation and return this spec to review if any of the following is
true:

- the founder changes D-C-01, D-C-02, or D-C-08;
- `contextmesh` changes admission/authorization semantics so admitted events no
  longer evidence authorization at admission;
- the approved ID derivation is interpreted as BLAKE3 derive-key mode rather
  than literal domain-prefix hashing;
- adding the workspace cannot preserve the core's frozen reachable external
  package/feature graph;
- a required field cannot fit the 2 MiB wire while respecting all count bounds;
- implementation would require an Option A/B wire or store-schema change.

## 3. Dependency-order implementation contract

Implementation and review SHALL proceed in this exact order. A later stage may
not begin until the prior stage's corresponding `OC01-*` tests pass.

1. **Workspace and one-way path dependency.**
2. **Typed IDs, domains, errors, strict JSON, and bounds.**
3. **OutcomeLedger value types, body, and envelope schemas.**
4. **Issue, strict parse, canonical render, and cryptographic verify.**
5. **DAG/context and input-ref snapshot verification.**
6. **Bounded import/export.**
7. **Committed golden and adversarial/boundary vectors.**
8. **Evidence, documentation, dependency, privacy, and claim gate.**

## 4. Workspace and dependency gate

### 4.1 Workspace shape

Convert the root manifest into a package-plus-workspace manifest without moving
the core package:

```toml
[workspace]
members = [".", "contextmesh-salience"]
resolver = "3"
```

Create `contextmesh-salience/Cargo.toml` with:

```toml
[package]
name = "contextmesh-salience"
version = "0.1.0"
edition = "2024"
rust-version = "1.97"
publish = false

[dependencies]
contextmesh = { path = ".." }
base64 = { version = "=0.23.1", default-features = false, features = ["std"] }
blake3 = { version = "=1.8.6", default-features = false, features = ["std"] }
serde = { version = "=1.0.229", features = ["derive"] }
serde_json = { version = "=1.0.151", default-features = false, features = ["std", "float_roundtrip"] }
serde_jcs = "=0.2.0"
thiserror = "=2.0.20"

[dev-dependencies]
tokio = { version = "=1.53.1", default-features = false, features = ["macros", "rt"] }
```

These registry dependencies are already present at the exact locked versions.
They are direct dependencies of the salience crate only because the equivalent
core parser/canonicalizer and receipt fixed-type macro are crate-private. This
is not authority to expose those internals, move OC behavior into core, or add
any new registry package. `tokio` is test-only and already locked.

Dependency direction is exactly:

```text
contextmesh-salience -> contextmesh
contextmesh -X-> contextmesh-salience
```

No default model, embedding, judge, network, native runtime, alternate database,
or optional heavy adapter dependency is permitted in OC-01.

### 4.2 Workspace/lock semantics and immutable legacy gates

A workspace adds exactly one local package (`contextmesh-salience`) to
Cargo.lock and metadata. Expected counts after setup are:

- workspace-local packages: 2 (`contextmesh`, `contextmesh-salience`);
- packages reachable from `contextmesh`: 320 total = `contextmesh` plus the
  same 319 registry/external packages as baseline;
- registry/git packages reachable from `contextmesh`: 319;
- total Cargo.lock package entries: 321, solely because the new local package
  is now represented;
- **new registry/git package identities: 0**.

The historical phrase “core closure stays 320” retains the approved meaning:
`contextmesh` plus its transitive external closure remains 320. It does not mean
that a multi-package workspace must still contain only 320 lock entries.

Implement one new deterministic helper,
`scripts/check-core-dependencies.py`, used only by the OC gate. It:

1. runs `cargo metadata --locked --format-version 1`;
2. locates the package whose name is exactly `contextmesh` rather than using
   nullable `resolve.root`;
3. traverses `resolve.nodes` from that package ID;
4. distinguishes `workspace_members` from registry/git packages;
5. proves the core reachable `(name, version, source)` set, exact pins/features,
   forbidden-capability set, and `cargo tree -p contextmesh --locked -e
   features` remain baseline-identical;
6. proves `contextmesh-salience` is local, depends on `contextmesh`, has no
   reverse path, uses only the approved exact direct pins, and adds zero
   registry/git package identities;
7. emits stable non-secret output and fails closed on missing/duplicate package
   identity, graph ambiguity, drift, or incomplete metadata.

Existing OA/OB release verifiers are **immutable historical evidence** and are
not edited to understand a later workspace. In particular, `verify-oa06.sh`
correctly asserts that Cargo.toml/Cargo.lock were unchanged in OA-06, and
`verify-oa07.sh` correctly chains that historical assertion. OC-01 therefore
uses explicit execution modes:

| Verifier evidence | Execution mode | Required result |
|---|---|---|
| OA release chain | detached clean worktree at `9c275f0` (OA-07 completion) | `verify-oa07.sh` passes unchanged, including OA-06 |
| Option B completion chain | detached clean worktree at `1df5334` (OB-13 completion) | `verify-ob13.sh` passes unchanged |
| Current OC workspace | current clean candidate tree | package-scoped core/salience/workspace build, fmt, Clippy, tests, demos, fixture hashes, store/schema/wire hashes, forbidden surfaces, privacy and dependency helper pass |

Historical worktrees run offline from the local repository and registry cache,
are removed after the gate, and never substitute their old tests for current-tree
regression. The current OC gate directly reruns every applicable test/demo and
compares frozen fixture and core source/wire/schema hashes. Historical verifier
success proves old release evidence was not rewritten; current-tree success
proves the workspace did not regress it.

The implementation SHALL NOT modify any existing `verify-oa*.sh` or
`verify-ob*.sh`, their recorded checkpoint semantics, or historical evidence
files. `verify-oa04-dependencies.sh` remains a standalone probe verifier. The
only new dependency gate files are `scripts/check-core-dependencies.py` and
`scripts/verify-oc01.sh`.

## 5. Frozen constants, domains, and encodings

### 5.1 Domains and typed text

| Item | Frozen v1 value |
|---|---|
| body version | JSON integer `1` |
| ID text | `ocout1_` + unpadded base64url of exactly 32 bytes |
| signature text | `ocsig1_` + unpadded base64url of exactly 64 bytes |
| ID domain bytes | `org.aaif.contextmesh.oc.outcome-ledger-id.v1\0` |
| signature domain bytes | `org.aaif.contextmesh.oc.outcome-ledger-signature.v1\0` |
| input-ref fingerprint text | `ocrefs1_` + unpadded base64url of exactly 32 bytes |
| input-ref fingerprint domain | `org.aaif.contextmesh.oc.input-ref-snapshot.v1\0` |
| task/config/error/operation hash text | `blake3_` + exactly 64 lowercase hexadecimal characters |
| timestamp | exactly `YYYY-MM-DDTHH:MM:SSZ`, valid UTC Gregorian date, year >= 1970 |

Rust constants are byte strings whose final byte is `0x00`:

```rust
pub const OUTCOME_ID_DOMAIN: &[u8] =
    b"org.aaif.contextmesh.oc.outcome-ledger-id.v1\0";
pub const OUTCOME_SIGNATURE_DOMAIN: &[u8] =
    b"org.aaif.contextmesh.oc.outcome-ledger-signature.v1\0";
pub const INPUT_REF_FINGERPRINT_DOMAIN: &[u8] =
    b"org.aaif.contextmesh.oc.input-ref-snapshot.v1\0";
```

Vectors assert `domain.last() == Some(&0)` and reject literal backslash-plus-zero.

ID derivation is ordinary BLAKE3 over the literal ID-domain bytes (including
NUL) followed by exact canonical body bytes. It is **not** BLAKE3 derive-key
mode:

```text
id_bytes = BLAKE3(ID_DOMAIN_BYTES || JCS(body))
outcome_id = "ocout1_" || BASE64URL_NOPAD(id_bytes)
```

The signature is Ed25519 over literal signature-domain bytes (including NUL)
followed by the raw 32 ID bytes. Implementation SHALL reuse
`SigningIdentity::sign_domain_message(SIGNATURE_DOMAIN, &id_bytes)` and
`verify_domain_message`; it must not sign the ID text or body directly.
Cross-type IDs, signatures, prefixes, lengths, alphabets, padding, or domains
reject.

### 5.2 D-C-02 hard maxima

Caller-configurable limits may equal or lower a maximum, never be zero or exceed
it. No implicit truncation or chunking exists.

| Bound | Maximum | OC-01 application |
|---|---:|---|
| one canonical artifact | 2,097,152 bytes | raw input and canonical envelope output |
| Outcome Ledger event references | 4,096 | every EventId-valued body occurrence, including snapshot heads, terminal, evidence, attempts, dead ends, and marks |
| Outcome Ledger attempts | 1,024 | `attempts` array |
| Outcome Ledger dead ends | 1,024 | `dead_ends` array |
| Attribution candidate references/marks | 4,096 each | later artifact bound acknowledged; OC-01 `attribution_marks` is capped at 4,096 |
| Thorn conditional failure entries | 4,096 | not serialized or implemented by OC-01 |
| Prior scored output entries | 4,096 | not serialized or implemented by OC-01 |
| Selection Influence pre-closure refs | 4,096 | not serialized or implemented by OC-01 |
| Selection Execution pre-closure refs | 4,096 | not serialized or implemented by OC-01 |
| warnings/uncertainty notes | 64 | `warnings`; no uncertainty field in OutcomeLedgerV1 |
| one note | 1,024 UTF-8 bytes | warnings and every unavailable reason |
| Prior logical work | 100,000 nodes / 1,000,000 edges | acknowledged but out of OC-01 scope |

Additional OC-01 sub-bounds are: mechanism identity 128 bytes; mechanism
version 64 bytes; attempt/dead-end category 64 ASCII bytes; one optional
human-readable note 1,024 UTF-8 bytes; all integers `0..=2^53-1`; quality
`value_ppm` `0..=1,000,000`. The total 2 MiB bound remains authoritative.
Checked arithmetic is mandatory for every aggregate count and byte total.

Mechanism identity/version, warning text, and unavailable reasons are nonempty
valid UTF-8 and reject Unicode C0/C1 control characters. Attempt/dead-end/error
categories use lowercase ASCII segments matching
`[a-z0-9]+(?:[._-][a-z0-9]+)*`, 1..=64 bytes. Opaque external artifact IDs are
1..=128 printable ASCII bytes with no whitespace/control characters. These
grammar rules are structural hygiene, not a claim that arbitrary caller text is
non-secret.

### 5.3 Stable non-secret `OutcomeError` categories

`OutcomeError` SHALL contain exactly the D-C-02 stable categories, with display
text that includes no path, input fragment, task text, note, mechanism text,
payload, key, signature, or provider response:

| Rust variant | Stable category |
|---|---|
| `Malformed` | `malformed` |
| `Noncanonical` | `noncanonical` |
| `UnsupportedVersion` | `unsupported-version` |
| `LimitExceeded` | `limit-exceeded` |
| `IdMismatch` | `id-mismatch` |
| `SignatureInvalid` | `signature-invalid` |
| `MissingEvent` | `missing-event` |
| `UnauthorizedEvent` | `unauthorized-event` |
| `ContextMismatch` | `context-mismatch` |
| `StaleInput` | `stale-input` |
| `MechanismUnavailable` | `mechanism-unavailable` |
| `IncompleteInput` | `incomplete-input` |

Parser syntax/type/duplicate/unknown/missing/typed-encoding failures collapse to
`malformed`, except wrong version, noncanonical bytes, bounds, ID, and signature
retain their dedicated categories. These are artifact categories; operational
Store/file failures use the wrapper below and do not add wire categories.
No parse, issue, verify, or import operation returns a partial ledger/report.

The twelve `OutcomeError` values are the exact artifact/wire semantic categories
and are not overloaded with operational failures. Store-aware and verified-file
APIs use a separate non-wire wrapper:

```rust
pub enum OutcomeOperationError {
    Artifact(OutcomeError),
    Store(contextmesh::error::StoreError),
    Io(std::io::Error),
}
pub type OutcomeOperationResult<T> = Result<T, OutcomeOperationError>;
```

Its Display strings, custom Debug output, verification reports, and gate output
are generic (`outcome artifact operation failed`, `outcome store operation
failed`, `outcome file operation failed`). The source chain retains the typed
cause for programmatic inspection. Stable `StoreError` causes are designed as
non-secret; arbitrary `std::io::Error` source text is not certified non-secret
and callers must not log or export traversed I/O sources. Exact mapping is:

| Condition | Result |
|---|---|
| explicit lookup returns `Ok(None)` | `Artifact(MissingEvent)` |
| loaded event has another context | `Artifact(ContextMismatch)` |
| current ref snapshot differs | `Artifact(StaleInput)` |
| body/ID/signature/bound/schema failure | `Artifact(the exact OutcomeError)` |
| any `Store::event/list_*` returns `Err(e)` | `Store(e)` without semantic remapping |
| import/export filesystem operation fails | `Io(e)` |

In particular database unavailability, migration/newer-schema,
indeterminate-commit, corruption, or any future `StoreError` is never mislabeled
`malformed`. `CorruptStorage` remains an operational Store cause; OC-01 does not
claim to identify which stored byte was corrupt.

`MechanismUnavailable`, `IncompleteInput`, and `UnauthorizedEvent` are frozen
reserved artifact categories in OC-01. No current issuance path fabricates them:
unavailable quality/cost is valid signed data, missing required JSON is
`Malformed`, and artifact-signer policy is not inferred. Later packages may
activate a reserved category only under founder-controlled change review without
renaming or adding a category.

## 6. Strict JSON and canonical ordering

The salience crate SHALL locally implement a streaming `serde_json` visitor that
rejects a BOM, trailing data, duplicate object members at every depth, unsafe or
non-finite numbers, and depth over 64. Every object rejects unknown fields and
requires every field listed below. `null` is permitted only where the schema
explicitly says so. Serialization uses `serde_jcs` RFC 8785/JCS.

`from_wire` error precedence is frozen: (1) raw wire bound, (2) strict parse,
(3) version/shape/tag/order/value/bounds validation, (4) canonicalize the full
envelope and compare bytes, (5) recompute/compare ID, (6) verify signature.
Therefore a semantically valid non-JCS envelope with a bad ID reports
`noncanonical`; crypto is not evaluated until canonical bytes match. Whitespace,
alternate member order, or normalized escape spellings return `noncanonical`;
malformed structure returns `malformed`. `to_wire` always revalidates before
returning exact JCS.

Arrays have semantic ordering independent of JSON object-key order:

- local refs: strictly ascending unique by `name` canonical text;
- remote refs: strictly ascending unique by `(peer, name)` canonical text;
- every EventId list: strictly ascending unique by canonical EventId text;
- attempts: exact contiguous IDs `attempt1_000000` through
  `attempt1_NNNNNN`; a parent ID must precede the child;
- dead ends: exact contiguous IDs `dead1_000000` through `dead1_NNNNNN`;
- attribution marks: strictly ascending unique by
  `(event, label, mechanism.identity, mechanism.version,
  mechanism.config_hash)`;
- warnings: caller order is meaningful and preserved; duplicate warning strings
  are rejected.

Constructors reject disorder and duplicates; they do not sort caller input.
Snapshot capture is the only helper that constructs canonical ref order from
store query results.

Uniqueness applies **within each array only**. The same EventId may legitimately
occur in terminal, outcome evidence, an attempt, a dead end, and an attribution
mark; each wire occurrence counts independently toward 4,096 while store reads
may be deduplicated internally.

## 7. Frozen JSON value schemas

All object fields below are required unless a tagged variant explicitly defines
a different exact field set.

### 7.1 `MechanismRecordV1`

```json
{"identity":"caller.example","version":"1.0.0","config_hash":"blake3_<64-lower-hex>"}
```

All three strings are nonempty. The configuration hash is supplied by the
mechanism owner and binds the exact rubric/extractor/collector configuration.
OC-01 records it; OC-01 does not execute or reproduce the mechanism.

### 7.2 `TaskBindingV1`

```json
{
  "content_hash":"blake3_<64-lower-hex>",
  "structured_hash":null,
  "external_artifact_id":null
}
```

`content_hash` is required and is ordinary BLAKE3 of the exact original task
bytes, rendered as `blake3_` plus lowercase hex. `structured_hash` is `null` or
the same typed hash of a caller-owned canonical structured task representation;
the representation itself is absent. `external_artifact_id` is `null` or one
canonical caller-declared opaque ID of at most 128 printable ASCII bytes. OC-01
does not dereference it, certify it non-secret, or claim compatibility with that
artifact; the caller/export policy remains responsible for its content.

Raw task text, transcript content, structured task content, filesystem paths,
URLs, and chain-of-thought are absent from the portable v1 wire. This is a
deliberate privacy-preserving divergence from Option B `TaskRecordV1`, not an
extension or alternate B1 receipt wire. OC-01 reuses the provenance vocabulary,
not the Option B task serialization. Constructors receive hashes; they never
accept raw task bytes and never claim to have recomputed a hash from unavailable
content.

### 7.3 Input-ref snapshot

```json
{
  "fingerprint":"ocrefs1_<32-byte-base64url>",
  "local":[{"name":"main","head":"evt1_..."}],
  "remote":[{"peer":"peer-a","name":"main","head":"evt1_..."}]
}
```

The fingerprint input is exact JCS for:

```json
{"context":"ctx1_...","local":[...],"remote":[...]}
```

and the fingerprint is ordinary BLAKE3 over
`INPUT_REF_FINGERPRINT_DOMAIN || JCS(input)` followed by the `ocrefs1_` typed
encoding. Snapshot capture reads `Store::list_local_refs(context)` and
`Store::list_remote_refs(None, context)`, converts names/heads to canonical text,
orders as specified, and computes the fingerprint. Empty local and remote arrays
are valid. Every snapshot head counts toward and is verified under the 4,096
Outcome Ledger event-reference occurrence maximum.

The current public Store list APIs materialize complete vectors before OC can
apply its limit. `capture(..., limits)` checks counts and checked byte totals
immediately after each query and fails `Artifact(LimitExceeded)` before body
construction, but OC-01 does **not** claim database-work allocation is bounded at
4,096. A truly bounded/paginated Store query would require separate founder
approval for a core public API change.

### 7.4 Terminal marker

Exactly one variant is present:

```json
{"status":"event","event":"evt1_..."}
```

or

```json
{"status":"unterminated","reason":"no-terminal-event"}
```

The caller must choose. There is no terminal discovery, `null`, best-effort
fallback, or heuristic. `unterminated` is a valid signed ledger state; inability
to name a terminal event must never produce a fabricated EventId. Its `reason`
is exactly one of `no-terminal-event`, `cancelled-before-terminal`,
`collector-ended`, or `unknown`; free text is not accepted in this tagged union.

### 7.5 Outcome and quality records

Outcome:

```json
{
  "value":"succeeded",
  "evidence":["evt1_..."],
  "provenance":{"identity":"...","version":"...","config_hash":"blake3_..."}
}
```

`value` is exactly one of `succeeded`, `failed`, `partial`, `cancelled`, or
`unknown`. `evidence` may be empty and is sorted unique. A terminal event does
not force `succeeded`; `unterminated` does not force `failed`.

Available quality:

```json
{
  "status":"available",
  "value_ppm":750000,
  "evidence":["evt1_..."],
  "provenance":{"identity":"...","version":"...","config_hash":"blake3_..."}
}
```

Unavailable quality:

```json
{
  "status":"unavailable",
  "reason":"no recorded rubric",
  "provenance":{"identity":"...","version":"...","config_hash":"blake3_..."}
}
```

`value_ppm` is a mechanism-local assessment under the signed configuration; it
is not objective quality, a C4 utility metric, or cross-rubric comparable.

### 7.6 Cost ledger and availability

Every cost field is present and independently tagged. Available:

```json
{"status":"available","value":17,"provenance":{"identity":"...","version":"...","config_hash":"blake3_..."}}
```

Unavailable:

```json
{"status":"unavailable","reason":"clock not exposed","provenance":{"identity":"...","version":"...","config_hash":"blake3_..."}}
```

`CostLedgerV1` has exactly these fields:

```json
{
  "wall_clock_ms":{...},
  "tool_calls":{...},
  "retries":{...},
  "input_tokens":{...},
  "output_tokens":{...}
}
```

Values are nonnegative safe integers. `0` is an available measured/recorded
zero, not unavailable. Each provenance identifies the source/collector. Missing
wall-clock, calls, retries, or token counts are never inferred from timestamps,
attempt count, event count, or provider text. Overall and per-attempt costs use
the same exact schema; OC-01 does not assert their sums agree because collectors
may have different scopes, but any aggregation claim belongs in the collector's
configuration.

### 7.7 Attempt tree

Each attempt is:

```json
{
  "attempt_id":"attempt1_000000",
  "parent_attempt_id":null,
  "status":"failed",
  "operation_fingerprint":"blake3_<64-lower-hex>",
  "event_refs":["evt1_..."],
  "error":{
    "status":"available",
    "category":"provider-timeout",
    "fingerprint":"blake3_<64-lower-hex>"
  },
  "costs":{"wall_clock_ms":{...},"tool_calls":{...},"retries":{...},"input_tokens":{...},"output_tokens":{...}},
  "provenance":{"identity":"...","version":"...","config_hash":"blake3_..."}
}
```

`status` is `succeeded`, `failed`, `partial`, `cancelled`, or `unknown`.
`parent_attempt_id` is `null` only for a root. An empty attempt array is valid;
otherwise there is exactly one root (`attempt1_000000`), and every later node
has one earlier parent, yielding one acyclic connected tree without a graph
walk. `error` is either the exact available object above or
`{"status":"unavailable","reason":"..."}`. A succeeded attempt may still carry
an available error only if the mechanism uses it as recorded diagnostic input;
OC-01 does not reinterpret it. Fingerprints are opaque and must not contain raw
errors, prompts, paths, URLs, credentials, or chain-of-thought.

### 7.8 Dead ends

Each dead end is:

```json
{
  "dead_end_id":"dead1_000000",
  "attempt_id":"attempt1_000000",
  "failure_category":"provider-timeout",
  "error_fingerprint":"blake3_<64-lower-hex>",
  "event_refs":["evt1_..."],
  "disposition":"unresolved",
  "provenance":{"identity":"...","version":"...","config_hash":"blake3_..."}
}
```

The referenced attempt must exist. `disposition` is exactly `unresolved`,
`abandoned`, `superseded`, or `recovered`. A dead end records a caller
classification; it is not automatically eligible for Thorn propagation,
negative filtering, or reuse in a different task/world state. Those C3 rules
remain out of scope.

### 7.9 Caller-supplied attribution marks

Each mark is:

```json
{
  "event":"evt1_...",
  "label":"load-bearing-candidate",
  "evidence":["evt1_..."],
  "mechanism":{"identity":"...","version":"...","config_hash":"blake3_..."}
}
```

`label` is exactly `load-bearing-candidate`, `supporting-candidate`,
`dead-end-candidate`, or `unknown`. Marks are supplied by the caller and signed
as claims. OC-01 verifies reference integrity and mechanism provenance only.
The `-candidate` vocabulary is deliberate: no mark is C2 causal attribution,
no score is generated, and no mark may be promoted to a Salience Prior without
later OC-02/OC-03 gates.

## 8. Frozen body and envelope schemas

### 8.1 `OutcomeLedgerBodyV1`

The body has exactly these required fields:

```json
{
  "version":1,
  "context":"ctx1_...",
  "input_refs":{"fingerprint":"ocrefs1_...","local":[],"remote":[]},
  "task":{"content_hash":"blake3_...","structured_hash":null,"external_artifact_id":null},
  "terminal":{"status":"unterminated","reason":"no-terminal-event"},
  "outcome":{"value":"unknown","evidence":[],"provenance":{"identity":"...","version":"...","config_hash":"blake3_..."}},
  "quality":{"status":"unavailable","reason":"...","provenance":{"identity":"...","version":"...","config_hash":"blake3_..."}},
  "costs":{"wall_clock_ms":{...},"tool_calls":{...},"retries":{...},"input_tokens":{...},"output_tokens":{...}},
  "attempts":[],
  "dead_ends":[],
  "attribution_marks":[],
  "warnings":[],
  "created_at":"2026-08-21T00:00:00Z",
  "author":"ed25519_..."
}
```

Every referenced EventId must be an admitted event in `context`. Admission is
the current public evidence that its author passed the append-only local policy
at admission time. The OC artifact `author` is authenticated by the distinct
Outcome Ledger domain signature; OC-01 does not claim that artifact signing is
Option A event admission, query a private allowlist, or invent revocation or
historical authorization semantics.

Every EventId-valued occurrence in the body is collected before store access,
counted with checked arithmetic, and capped at 4,096. Verification may
deduplicate store reads internally, but deduplication does not reduce the wire
occurrence count.

### 8.2 `SignedOutcomeLedgerV1`

The envelope has exactly:

```json
{
  "outcome_id":"ocout1_...",
  "body":{...},
  "signature":"ocsig1_..."
}
```

`outcome_id` covers only canonical body bytes under the ID domain. `signature`
covers the raw derived ID under the signature domain. The author is inside the
body and selects the strict Ed25519 verification key.

## 9. Public API semantics

The public surface is OC-owned and read-only after construction:

```rust
pub const OUTCOME_VERSION: u8 = 1;
pub const MAX_OUTCOME_WIRE_BYTES: usize = 2_097_152;
pub const MAX_OUTCOME_EVENT_REFERENCES: usize = 4_096;
pub const MAX_OUTCOME_ATTEMPTS: usize = 1_024;
pub const MAX_OUTCOME_DEAD_ENDS: usize = 1_024;
pub const MAX_OUTCOME_ATTRIBUTION_MARKS: usize = 4_096;
pub const MAX_OUTCOME_NOTES: usize = 64;
pub const MAX_OUTCOME_NOTE_BYTES: usize = 1_024;

pub struct OutcomeLimits {
    pub max_wire_bytes: usize,
    pub max_event_references: usize,
    pub max_attempts: usize,
    pub max_dead_ends: usize,
    pub max_attribution_marks: usize,
    pub max_warnings: usize,
    pub max_note_bytes: usize,
    pub max_json_depth: usize,
    pub max_mechanism_identity_bytes: usize,
    pub max_mechanism_version_bytes: usize,
    pub max_category_bytes: usize,
    pub max_external_artifact_id_bytes: usize,
}
pub struct OutcomeId([u8; 32]);
pub struct OutcomeSignature([u8; 64]);
pub struct InputRefSnapshotV1 { /* exact schema */ }
pub struct OutcomeLedgerBodyV1 { /* exact schema */ }
pub struct SignedOutcomeLedgerV1 { /* exact envelope */ }

impl InputRefSnapshotV1 {
    pub async fn capture(store: &contextmesh::store::Store,
                         context: ContextId,
                         limits: OutcomeLimits)
        -> OutcomeOperationResult<Self>;
}

impl SignedOutcomeLedgerV1 {
    pub async fn issue(identity: &SigningIdentity,
                       store: &Store,
                       body: OutcomeLedgerBodyV1,
                       limits: OutcomeLimits)
        -> OutcomeOperationResult<Self>;
    pub fn from_wire(input: &[u8], limits: OutcomeLimits)
        -> Result<Self, OutcomeError>;
    pub fn verify(&self, limits: OutcomeLimits) -> Result<(), OutcomeError>;
    pub async fn verify_against_dag(&self, store: &Store,
                                    limits: OutcomeLimits)
        -> OutcomeOperationResult<OutcomeVerification>;
    pub async fn verify_current_inputs(&self, store: &Store,
                                       limits: OutcomeLimits)
        -> OutcomeOperationResult<OutcomeVerification>;
    pub fn to_wire(&self, limits: OutcomeLimits) -> Result<Vec<u8>, OutcomeError>;
}
```

Checked constructors exist for every nested type and expose read-only accessors.
No unchecked public field constructor or `Deserialize`-only bypass is allowed.
`OutcomeLimits::default()` equals every hard maximum above. `OutcomeLimits::new`
requires every field to be nonzero and at or below its hard maximum. Depth is
hard-capped at 64; category and external-artifact limits are 64 and 128 bytes.
Nested constructors receive the same limits; no default is silently substituted.

`from_wire` is the sole untrusted-wire constructor. `OutcomeVerification`
contains checked event-occurrence, unique-event, local-ref, and remote-ref
counts plus the verified snapshot fingerprint; it has no redundant `valid`
boolean, findings, or arbitrary input text. Since methods fail closed, only a
fully valid operation returns the report.

### 9.1 Issue order

`issue` performs atomically with respect to return value (it does not mutate the
store):

1. call `body.validate`: validate caller limits, author match, every tag/order,
   contiguous attempt ordinal and parent-before-child relation, dead-end target,
   that each EventId array is sorted and unique, occurrence count, snapshot
   fingerprint, notes, and canonical body-byte bound;
2. load **every** referenced, input-ref, and terminal EventId through
   `Store::event`, requiring existence, strict stored-wire verification, and the
   exact body `ContextId`; then compare the embedded ref snapshot with a fresh
   canonical capture (`stale-input` on drift);
3. derive the typed ID from exact JCS body bytes;
4. domain-sign the raw ID bytes;
5. independently self-verify the complete envelope and revalidate with the store
   before returning.

The issuance API is async because step 2 is mandatory. No artifact is returned
after any DAG finding or failed step.

### 9.2 Structural parse/verify

`from_wire` and `verify` do not access or require a store. They enforce strict canonical
JSON, exact shape/types/tags/order/bounds, snapshot fingerprint, required task
hash, ID derivation, author key, and strict signature. They cannot claim
that EventIds exist or refs are current.

### 9.3 DAG/context and current-input verification

`verify_against_dag` first performs structural verification, then loads every
unique referenced EventId through `Store::event`. Missing returns
`Artifact(MissingEvent)`; a successfully loaded event with a different body
context returns `Artifact(ContextMismatch)`. Any `Store::event` operational
failure, including `StoreError::CorruptStorage` produced for unverifiable stored
wire, returns `OutcomeOperationError::Store(e)` without remapping. Only
`Ok(Some(event))` proceeds to context validation. Presence in the admitted
append-only store is the authorization evidence for referenced events; no
artifact-signer allowlist, revocation, or historical policy is inferred.
`unauthorized-event` remains a stable reserved category for a future approved
Store authorization result and is not fabricated from currently public APIs.

`verify_current_inputs` performs full DAG verification and captures the current
local+remote ref snapshot. A structurally valid embedded snapshot that differs
from the fresh capture—including name/head/peer addition or movement, or an
externally observable removal—returns `stale-input`. An invalid caller-supplied
embedded fingerprint is rejected before artifact construction as `id-mismatch`;
it never reaches `verify_current_inputs`. OC-01 v1's public `Store` API has no
local/remote ref-removal transition; therefore D09 executable coverage is
limited to public API-representable additions/movements, the direct invalid-
fingerprint `id-mismatch` precedence vector, and fresh-capture `stale-input`
vectors. This does not weaken runtime detection: if a future or external store
implementation returns a snapshot with a removed ref, the exact snapshot
comparison returns `stale-input`. A future approved public removal API reopens
D09 to add transition coverage. Ordinary artifact
re-verification may remain valid after refs move because immutable referenced
events still verify; any execution or freshness-sensitive consumer must call
`verify_current_inputs` immediately before use.

## 10. Import/export

```rust
pub fn export_outcome(ledger: &SignedOutcomeLedgerV1,
                      path: &Path,
                      limits: OutcomeLimits) -> OutcomeOperationResult<()>;
pub fn import_outcome(path: &Path,
                      limits: OutcomeLimits) -> OutcomeOperationResult<SignedOutcomeLedgerV1>;
pub async fn import_outcome_verified(path: &Path,
                      store: &Store,
                      limits: OutcomeLimits) -> OutcomeOperationResult<SignedOutcomeLedgerV1>;
```

Export re-verifies, renders canonical bytes, refuses an existing destination,
writes all bytes to a newly created regular file, syncs it, and removes a
partial new file on write/sync failure. It never writes the Option A database.
Import opens a regular file, reads at most `max_wire_bytes + 1`, rejects excess,
and calls `from_wire`. Verified import additionally performs DAG and current
snapshot verification before returning. Symlinks and non-regular files reject.
No import sanitizes, rewrites, sorts, or repairs input.

## 11. Exact implementation file map

Implementation may create/modify only this planned surface unless review
amends the map:

- `Cargo.toml` — add workspace members/resolver; preserve core package/deps.
- `Cargo.lock` — one new local package entry only; zero new registry/git IDs.
- `contextmesh-salience/Cargo.toml` — exact package and pinned dependencies.
- `contextmesh-salience/src/lib.rs` — crate docs, exports, non-claims.
- `contextmesh-salience/src/error.rs` — exact `OutcomeError` categories.
- `contextmesh-salience/src/json.rs` — local strict duplicate-key/I-JSON parser
  and JCS helper; no core-internal exposure.
- `contextmesh-salience/src/types.rs` — typed ID/signature/fingerprint,
  mechanism/task/availability/cost/attempt/dead-end/mark/snapshot values and
  checked limits.
- `contextmesh-salience/src/outcome.rs` — body/envelope, issue/parse/render,
  hash/signature verification.
- `contextmesh-salience/src/verify.rs` — event collection, DAG/context,
  and current snapshot verification.
- `contextmesh-salience/src/io.rs` — bounded regular-file import/export.
- `contextmesh-salience/tests/oc01_workspace.rs` — dependency direction/counts,
  helper behavior, legacy-gate migration assertions.
- `contextmesh-salience/tests/oc01_schema.rs` — values/body/order/tags/bounds.
- `contextmesh-salience/tests/oc01_crypto.rs` — ID/domain/signature/tamper.
- `contextmesh-salience/tests/oc01_dag.rs` — missing/cross-context/ref
  snapshot/current-state behavior.
- `contextmesh-salience/tests/oc01_io.rs` — import/export and filesystem bounds.
- `contextmesh-salience/tests/oc01_adversarial.rs` — strict parser, every +1,
  no-partial-output, privacy/error matrix.
- `contextmesh-salience/tests/fixtures/oc01-outcome-ledger-v1-golden.json` —
  exact canonical envelope vector.
- `contextmesh-salience/tests/fixtures/oc01-outcome-ledger-v1-unterminated.json`
  — exact canonical unavailable/unterminated vector.
- `scripts/check-core-dependencies.py` — shared package-name/reachability audit.
- `scripts/verify-oc01.sh` — non-recording package/evidence gate.
- `_bmad-output/verification-artifacts/oc-01-evidence.md` — implementation-time
  four-layer evidence; not created by this specification task.
- `README.md` — bounded artifact-integrity capability and explicit non-claims.

No production file under core `src/`, no existing OA/OB verifier, test, fixture,
or evidence file, and no store schema/wire file is an OC-01 implementation
target.

## 12. Golden, boundary, and adversarial contract

Deterministic fixture identities use published test-only seeds, a fixed context,
fixed admitted DAG, fixed refs, fixed timestamp, and fixed caller declarations.
A generator test is ignored; normal tests reconstruct and compare the committed
bytes, typed ID, and signature exactly. Fixture updates require change control,
not automatic regeneration.

Required vectors include:

- terminal-event ledger with available and unavailable costs, multi-level attempt
  tree, recovered/unresolved dead ends, and multiple attribution mechanisms;
- explicit `unterminated` ledger with unavailable clock/calls/retries/tokens;
- body ID and signature domain vectors, including cross-domain rejection;
- exact `0`, exact maximum, and `maximum + 1` for artifact bytes, event-reference
  occurrences, attempts, dead ends, marks, warnings, and note bytes;
- duplicate/unknown/missing fields at every nested schema, BOM, trailing data,
  non-JCS whitespace/member order/escape, unsafe number, invalid typed prefix,
  wrong tag field set, array disorder/duplicate, broken attempt parent/connectivity;
- body, task, snapshot, ID, signature, outcome, quality, cost, attempt, dead-end,
  mark, author, and timestamp tampering;
- missing, cross-context, corrupt/unverifiable, and moved
  local/remote ref cases;
- import/export existing path, symlink, non-regular file, short write/read excess,
  and no-partial-return behavior;
- stable error category and non-disclosure assertions.

## 13. Documentation and evidence standard

Every OC-01 implementation/evidence document must contain four separable layers:

1. **Sources:** exact commit, approved records, source/test paths, toolchain, and
   commands; source authority/reliability stated.
2. **Reasoning:** why the evidence supports each requirement, alternatives
   rejected, and assumptions; not merely command transcripts.
3. **Conclusion derivation:** a requirement-to-observation-to-conclusion table
   with exact test/gate IDs and no conclusion broader than observations.
4. **Invalidators:** concrete evidence that would reverse, pause, or narrow the
   conclusion, including dependency drift, vector drift, stale snapshot,
   unsupported sample, or claim overreach.

Documentation must distinguish recorded caller declaration from independently
verified fact. Evidence contains no secret keys, credentials, raw private
transcripts, chain-of-thought, private paths/URLs, or arbitrary payload/error
text. Safe typed IDs and aggregate counts are permitted.

## 14. Verification commands

Implementation review requires exact pinned/offline commands (the dependency
preflight runs before lockfile acceptance):

```bash
python3 -I scripts/check-core-dependencies.py --package contextmesh \
  --feature-tree-baseline cargo-tree-oa05-features.txt
cargo metadata --locked --format-version 1
cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p contextmesh-salience --locked
cargo test -p contextmesh --locked
cargo test --workspace --locked
bash scripts/verify-oc01.sh --historical-release-chains
bash scripts/verify-oc01.sh
```

`verify-oc01.sh --historical-release-chains` creates detached clean temporary
worktrees at `9c275f0` and `1df5334`, runs unchanged `verify-oa07.sh` and
`verify-ob13.sh` offline, and removes the worktrees. The normal current-tree
gate runs package-scoped build/test/demo and frozen fixture/source/wire/schema
hash checks without invoking historical HEAD assertions. Both modes are
non-recording, fail on partial/inconclusive output, and leave the tree clean.

## 15. Acceptance gates

OC-01 implementation is acceptable only when every matrix row passes and:

- **OC01-SETUP:** workspace shape, one-way dependency, zero new external package,
  core reachable closure/feature fixture, and sanctioned legacy-verifier
  migration all pass without weakening unrelated gates;
- **OC01-SCHEMA:** every exact v1 field/tag/order/bound rule and both committed
  canonical vectors pass;
- **OC01-CRYPTO:** literal ID/signature domains, typed encodings, author match,
  strict parse, round trip, cross-domain, and tamper tests pass;
- **OC01-DAG:** every referenced event exists/verifies/is same-context, admission
  supplies its authorization evidence, snapshot fingerprints bind exactly, and moved
  inputs return `stale-input` with no partial artifact;
- **OC01-IO:** bounded regular-file import/export returns only complete verified
  artifacts and never writes Option A storage;
- **OC01-ADVERSARIAL:** every maximum/+1, hostile JSON, ordering, privacy, stable
  error, and no-partial-output vector passes;
- **OC01-REGRESSION:** all Option A/B wires, fixtures, store schema, tests,
  forbidden surfaces, and core feature graph remain unchanged;
- **OC01-EVIDENCE:** four-layer evidence and claim audit are complete, and the
  verdict is limited to artifact integrity/provenance recording.

Passing the above can establish C1's OC-01 artifact-integrity portion only. P1-GO
also requires the separate preregistered human-gold configuration/hash from the
priority plan before OC-02 implementation or test-label inspection. This spec
does not define, approve, or claim that preregistration. OC-01 alone does not
complete C2, C3, C4, C5, or Option C.

## 16. Change control

Founder approval and coordinated updates to the controlling decision/spec are
required before changing:

- workspace dependency direction or adding a new external package;
- any v1 field name, requiredness, tag/value, order, typed prefix, domain,
  derivation, signature message, maximum, snapshot scope, or authorization semantics, or
  canonical parser rule;
- any stable `OutcomeError` category;
- Option A/B wire, schema, bounds, claim discipline, or core public API;
- event-reference verification, fail-closed/no-partial behavior, or artifact
  placement outside the store;
- the interpretation of core closure 320 or weakening a legacy fixture,
  forbidden-feature, security, regression, or release gate;
- promotion of caller attribution marks to causal C2 evidence or use in C3/C4.

A fixture change requires a version/change decision and explicit human review.
Implementation discoveries are recorded in the four-layer evidence and brought
back to specification review; they are not silently normalized in code.

## 17. Review checklist

- [x] Status is `approved-for-implementation` by Lunarpulse; implementation and
      evidence remain incomplete.
- [x] Dependency-order sections and matrix agree exactly.
- [x] Exact JSON examples contain every required field and no optional ambiguity.
- [x] All D-C-02 maxima and all twelve error categories are mapped to tests.
- [x] Terminal is caller-supplied EventId or explicit `unterminated` only.
- [x] Every cost and quality availability state is tagged and provenanced.
- [x] Attempt tree, dead ends, marks, task hash, snapshot, author, and timestamp
      semantics are complete.
- [x] No C2 causal, C3 prior/Thorn, C4 utility, sufficiency, or comprehension claim.
- [x] Core crate external closure remains frozen and workspace count semantics are
      explicit rather than hidden.
- [x] No production code or evidence completion is implied by this document.

## 18. Approval record

- 2026-08-21: four independent review loops moved the package from NO-GO to GO;
  all blocking contradictions and warnings were corrected.
- 2026-08-21: Lunarpulse approved the complete OC-01 spec and 90-row matrix and
  directed workspace implementation (Discord message `1540352346364842105`).
- **Authorization:** begin implementation strictly in section 3 dependency order.
- **Non-claim:** OC-01, P1, C1, and Option C are not complete until executable
  evidence passes every matrix row and the separate preregistration gate.
