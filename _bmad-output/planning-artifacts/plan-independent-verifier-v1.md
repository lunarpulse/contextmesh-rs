# Independent Offline Verifier (IV-1) — Plan v1.1

Date: 2026-11-06 · Status: REVISED per independent review (GO-WITH-CHANGES,
`~/.hermes/cache/scratch/iv1-plan-astra-review.log`). All review findings
(a)–(g) adopted below; v1 text is superseded.
Source: Astra feature review 2026-11-05 (recommendation #1, ranked highest),
founder directive "독립 검증기 계획 세우고 계획 독립 검증 하자".

## Changelog

- v1.1 (2026-11-06): adopted Astra plan-review corrections — parsing vs
  canonical-rendering semantics, 64-parent cardinality, three-way
  validity/closure/admission split, 7-exit taxonomy, new S0 (normative
  inputs + test corpus), cross-implementation conformance naming, fixture
  limitations and corpus requirements, independence wording.
- v1 (2026-11-06): initial draft, commit `0c93f3b`.

## 1. Purpose and problem

Option A's integrity checks currently run inside the producer's own
codebase (`Store::verify_full`, gate scripts): present verification
DEPENDS ON producer validation code, so a defect there can validate a
tampered artifact, and a recipient must run that code to check a record.
IV-1 breaks the loop with a second implementation of the FROZEN public
contract: signed events (OA-01) and Bundle v1 (OA-03).

**Independence statement (exact):** "Independent implementation of
contract validation, using disclosed shared cryptographic and
canonicalization dependencies." Ed25519 and BLAKE3 are cryptographic
primitives; JCS/RFC 8785 is canonicalization, not a primitive — shared
primitive/canonicalization/parser defects remain shared failure modes and
are disclosed, not hidden. Named dependency versions and any audit
evidence are recorded in the verifier's manifest; the word "audited" is
not used without that evidence. Independence applies to the contract
logic (schema, domain strings, derivation, ordering, bounds, DAG rules,
error taxonomy), written from the normative inputs in §2 only — importing
or copying producer validation logic is prohibited, and separate
placement alone does not establish independence.

**Claims gained (bounded):** a recipient can check an exported artifact's
envelope schema, canonical rendering, event IDs, signatures, parent
ordering and cardinality, bounds, in-bundle DAG ordering, and bundle
structure without running producer code.

**Non-claims (binding):**
- A valid signature proves only that the key decoded from `body.author`
  signed those bytes — not key ownership, author identity, payload truth,
  relevance, or completeness.
- "Complete" means closure relative to the supplied or selected heads
  only; a closed artifact cannot prove the producer disclosed every event
  or branch.
- Receiver admission (provisioning, authorization, trusted/expected
  genesis, existing collisions, importability into a store) is UNASSESSED.
- A signature authenticates neither an advertised ref nor any local
  admission policy.
- v1 scope is events and bundles only — not stores, sync, or provider
  recording.
- No A2A/ACP compliance or agent-interoperability claim; the S3 harness
  is named "cross-implementation conformance", never "interop".

## 2. Normative inputs (approved set, digest-pinned) and placement

- New standalone directory `~/contextmesh-verify/` — its own Cargo
  project, NOT a member of this repository's workspace. Root `Cargo.toml`
  is hash-pinned (no_new_deps, OC05-03); workspace membership would force
  a re-pin cycle. Zero files in this repository change except this plan
  and (later) a README pointer.
- The verifier is written from, and only from:
  1. `README.md` frozen contract sections (signed-event v1, Bundle v1,
     limits, golden vectors), pinned by digest at S0.
  2. Digest-pinned normative excerpts from the frozen specs
     `_bmad-output/implementation-artifacts/spec-oa-01-signed-event-contract.md`
     and `spec-oa-03-dag-bundles-verification.md` — copied into the
     verifier repository once, recorded by digest, covering details README
     compresses: exact kind/ref grammar, depth-counting rules, parser
     acceptance (insignificant whitespace / member-order differences),
     external-frontier parent and ref-target semantics. Producer source
     code (`src/`) is NOT a normative input — never linked, shelled into,
     or read.
  3. Committed fixtures `tests/fixtures/oa01-v1-golden.json`,
     `oa03-bundle-v1-golden.json`, `oa04-protocol-golden.json` — copied at
     S0, digest-recorded; no runtime dependency on this repository.

## 3. Scope (stages)

- **IV-S0 — inputs and corpus (before any code).** Freeze the §2 digest
  pins; author and commit the expected-outcome corpus manifest: positive
  vectors (extracted as `vectors[*].canonical_envelope_utf8` / the bundle
  fixture — the OA-01 wrapper itself is not an event); exact mutation
  recipes stating whether IDs/signatures are recomputed (so the intended
  check is not masked by an earlier identity failure); boundary and
  boundary-plus-one cases for every numeric bound; fractional and Unicode
  payload cases; synthetic multi-parent events; parent-after-child;
  duplicate event IDs/keys; external-frontier parents and ref targets;
  empty bundle; combined invalid-plus-incomplete cases. Cycle detection is
  tested via an identified synthetic graph test (a valid signed cyclic
  fixture is impossible by construction). Categories and the report
  schema (versioned, with finding order, truncation, completion status)
  are defined HERE, not deferred to packaging.
- **IV-S1 — single-event verification.** Parse one event JSON file
  (duplicate keys at every depth, BOM, trailing JSON rejected; finite
  fractional payload numbers accepted — the ±9,007,199,254,740,991 bound
  applies to integer-valued numbers only; Unicode preserved without
  normalization). Canonicalize the parsed value and verify against that
  rendering; separately REPORT whether the input bytes were already
  canonical (non-canonical input is accepted for verification and
  flagged, per the frozen OA-01/OA-03 parsing rules). Then: base64url
  exactness and lengths (32/64 bytes); kind grammar (1–64 ASCII bytes,
  frozen lowercase segment grammar); parents strictly increasing, unique,
  AND at most 64; payload depth 64; raw wire 2,097,152 B / canonical body
  1,114,112 B / payload 1,048,576 B; event-ID recomputation (BLAKE3
  derive-key `org.aaif.contextmesh.event-id.v1`); Ed25519 strict
  verification of message
  `ASCII("org.aaif.contextmesh.signature.v1") || NUL || raw_event_id[32]`
  using the public key decoded from `body.author` (there is no separate
  signer field). Pass criteria: every corpus positive row verifies; every
  negative row fails with its designated category.
- **IV-S2 — multi-event + Bundle v1.** One bundle file (directory input
  deferred out of v1). Checks split three ways:
  1. **Artifact validity** — bundle version/schema; one context; unique
     event IDs; ≤1,024 events; ≤256 refs, sorted, unique, namespace form;
     16 MiB; **parent-first event-array order** (an included parent
     appearing after its child is INVALID ORDERING, not incompleteness;
     order is validated as signed, never repaired by sorting).
  2. **Closure** — parents or advertised-ref targets absent from the
     supplied artifacts (external frontier) yield INCOMPLETE findings,
     never "valid and complete". Empty event arrays are legal; genesis
     need not be present (incremental bundles) — no unconditional
     "genesis first" rule. Cycle detection runs on the provided graph.
  3. **Receiver admission — UNASSESSED** (reported as such; requires
     receiver state the verifier does not have).
- **IV-S3 — cross-implementation conformance harness.** Lives in the
  verifier repository as a DEV-ONLY harness, outside the shipped
  verifier's runtime dependencies. Two parts: (i) run the producer
  demo/export CLI, feed real exported artifacts to IV-1 (must PASS);
  (ii) differential: run both implementations' validation on identical
  inputs at equivalent validation layers and compare outcomes.
  Disagreement is a defect in ONE of: the verifier, the producer, the
  fixture, or a specification ambiguity — adjudicated against the §2
  normative inputs under this repository's change control, never papered
  over in either repo.
- **IV-S4 — packaging.** Single binary / `cargo run` one-liner (the
  offline / no-writes guarantee scopes to verifier EXECUTION; build steps
  are documented separately with a prepared vendored/offline build).
  Frozen exit codes (§4), one versioned canonical JSON report on stdout.
  README section: "Verifying artifacts without this repository".

## 4. Exit codes and report (frozen at S1 after corpus green)

| Exit | Meaning |
|---|---|
| 0 | All applicable checks passed; requested closure established |
| 2 | Usage error |
| 3 | Invalid artifact: syntax, schema, encoding, bounds, identity, or structural failure |
| 4 | Cryptographic signature failure |
| 5 | Otherwise-valid inspected artifacts with unresolved dependencies (INCOMPLETE) |
| 6 | Input/output failure preventing completion |
| 9 | Internal verifier failure |

Precedence: definite invalidity is never masked by INCOMPLETE; an
operational failure prevents success. Finer-grained stable finding codes
live in the JSON report (schema versioned at S0).

## 5. Contract-freeze discipline for the verifier itself

- Fixture/corpus digests pinned at S0; behavior changes follow a re-pin
  note in the verifier CHANGELOG (single-repo scale, no founder gate,
  every change recorded).
- A golden-vector disagreement (§3 S3) may be a verifier defect, producer
  defect, fixture defect, or specification ambiguity; all four are
  considered before adjudication, which happens under this repository's
  change control.

## 6. Test and evidence standard

Every check maps 1:1 to positive AND negative corpus rows (S0 manifest).
Evidence record: four layers (contract citation to §2 inputs,
implementation map, corpus matrix with commands, run logs) — same
standard as oc-* evidence. Deterministic; no network at verification
time.

## 7. Effort and order

S0 ≈ 1 session (inputs, corpus, categories, report schema). S1 ≈ 1–2
sessions. S2 ≈ 1 session. S3 ≈ 1 session. S4 ≈ half session. Total M.
Strict order S0→S4; S3 after S2 green. Implementation begins only after
the S0 corpus manifest and this plan are committed in the verifier
repository.
