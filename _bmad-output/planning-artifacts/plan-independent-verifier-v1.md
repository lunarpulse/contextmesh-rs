# Independent Offline Verifier (IV-1) — Plan v1 (DRAFT for independent review)

Date: 2026-11-06 · Status: DRAFT — pending independent review
Source: Astra feature review 2026-11-05 (recommendation #1, ranked highest),
founder directive "독립 검증기 계획 세우고 계획 독립 검증 하자".

## 1. Purpose and problem

Option A's integrity claims are today verified only by code inside this
repository (`Store::verify_full`, gate scripts). Producer and verifier share
one implementation: a bug in it can validate a tampered artifact, and a
recipient must trust the whole contextmesh codebase to trust a record. IV-1
breaks that loop with a second, independent implementation of the FROZEN
public contract: signed events (OA-01) and Bundle v1 (OA-03).

**Claims gained (bounded):** a recipient can check an exported artifact's
canonical encoding, event IDs, signatures, parent ordering, bounds, DAG
closure, and bundle structure without running producer code.

**Non-claims (binding):**
- A valid signature proves only that the key signed those bytes — not key
  ownership, author identity, payload truth, relevance, or completeness.
- An ancestry export with missing events is reported as INCOMPLETE, never
  as valid-and-complete.
- IV-1 does not verify stores, sync, or provider recording; events and
  bundles only (v1 scope).
- No A2A/ACP compliance, interoperability, or "verified truth" claim.

**Honest independence boundary:** cryptographic primitives (Ed25519,
BLAKE3, JCS/RFC 8785) will use established audited libraries — possibly the
same families the producer uses. Independence applies to the CONTRACT
IMPLEMENTATION (schema, domain strings, derivation, ordering, bounds, DAG
logic, error taxonomy), which is written from the frozen spec text and
vectors alone, not by importing or copying producer code. A shared-primitive
bug would affect both implementations; this residual is disclosed, not
hidden.

## 2. Placement and frozen-surface policy

- New standalone directory `~/contextmesh-verify/` — its own Cargo project,
  NOT a member of this repository's workspace. Root `Cargo.toml` is
  hash-pinned (no_new_deps, OC05-03); workspace membership would force a
  re-pin cycle. Zero files in this repository change except this plan and
  (later) a README pointer.
- The verifier's ONLY inputs are: the frozen wire contract in README
  ("Frozen signed-event v1 contract", "DAG operations… Bundle v1"), the
  committed golden/adversarial fixtures, and exported artifacts. It must
  never link against, shell into, or read source from `src/`.
- Development flow: fixture-driven. Repo fixtures are copied into the
  verifier's own test-data directory at import time (recorded by digest);
  no runtime dependency on this repo.

## 3. Scope (stages)

- **IV-S1 — single-event verification (MVP).** Read one event JSON file;
  report: envelope/body schema exactness (unknown fields, duplicate keys,
  BOM, trailing JSON rejected); base64url exactness and lengths (32/64
  bytes); kind grammar (1–64 ASCII, frozen lowercase segment grammar);
  parents strictly increasing + unique; integer bounds (±9,007,199,254,
  740,991); payload depth 64; raw-wire 2 MiB / canonical body 1,114,112 B /
  payload 1 MiB limits; canonical re-encode byte-equality (RFC 8785);
  event-ID recomputation (BLAKE3 derive-key `org.aaif.contextmesh.event-id.v1`);
  Ed25519 strict verification with message
  `ASCII("org.aaif.contextmesh.signature.v1") || NUL || raw_event_id[32]`,
  author==signer check. Pass criteria: `oa01-v1-golden.json` verifies;
  every adversarial vector fails with the designated category.
- **IV-S2 — multi-event + Bundle v1.** A directory or bundle of events:
  same-context check; genesis (zero parents, first); parent closure within
  the provided set (missing parent ⇒ INCOMPLETE, not invalid); cycle
  detection; merge-parent canonical sort; bundle structural checks (one
  context, ≤1,024 events, ≤256 sorted refs, 16 MiB, ref namespace form);
  unsigned advertised refs reported as "asserted, unsigned" — never as
  verified. Per-category findings, bounded output.
- **IV-S3 — interop + tamper harness.** Scripted end-to-end: run the
  producer demo/export CLI in this repo, feed real exported artifacts to
  IV-1 (must PASS); apply the designated mutations (1-byte signature
  flip, parent reorder, duplicate key, oversized number, truncated
  bundle) — each must fail with the right category. This is the
  cross-implementation differential: disagreement in either direction is a
  bug in one of the two implementations, adjudicated against the frozen
  spec text.
- **IV-S4 — packaging.** Single static binary or `cargo run` one-liner,
  frozen exit codes (0 valid / 2 usage / 3 malformed / 4 signature-invalid /
  5 incomplete-ancestry / 9 internal), one canonical JSON audit report on
  stdout, no network, no writes outside explicit `--out`. README section
  "Verifying artifacts without this repository".

## 4. Contract-freeze discipline for the verifier itself

- The verifier pins its own fixture digests after IV-S1; subsequent
  changes follow a light re-pin note in its CHANGELOG (single-repo scale,
  no founder gate required, but every behavior change is recorded).
- If verification of a committed golden fixture FAILS in IV-1 while
  passing producer tests, that is a spec-ambiguity or producer-defect
  report to this repository — adjudicated here under change control,
  never papered over in the verifier.

## 5. Test and evidence standard

- Every check above maps 1:1 to at least one positive and one negative
  test row in the verifier's matrix (mirrors this repo's OC matrix style).
- Evidence record: four layers (contract citation, implementation map,
  test matrix with commands, run logs) — same standard as oc-* evidence.
- Deterministic, offline, `--locked`; no floats in verification logic.

## 6. Effort and order

S1 ≈ 1–2 sessions (schema+crypto checks against fixtures). S2 ≈ 1 session
(DAG/bundle). S3 ≈ 1 session (harness + tamper suite). S4 ≈ half session.
Total M. Order strictly S1→S4; S3 may begin once S2 is green.

## 7. Review questions for the independent reviewer

1. Is the independence boundary (§1) stated honestly — is "contract
   independence, shared primitives" a defensible claim, and how should it
   be worded to avoid overclaiming?
2. Does S2's treatment of missing parents (INCOMPLETE vs invalid) match
   the frozen Bundle v1 import semantics, or does it invent new policy?
3. Are the proposed exit-code categories complete for the failure modes
   of S1+S2, and stable enough to freeze?
4. Is any part of §3 scope creep relative to "verifier" (e.g., should the
   interop harness live outside the verifier repo)?
5. Any frozen contract detail (README "Frozen signed-event v1 contract")
   that this plan misreads or omits?
