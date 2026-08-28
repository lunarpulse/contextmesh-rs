# OC-02 Attribution — Implementation Specification

**Status:** approved-for-implementation (frozen 2026-08-26 by Lunarpulse, Discord message `1541934459069145168`)
**Package:** OC-02 (C2 attribution mechanism ladder)
**Controlling documents:** `spec-option-c-salience-provenance-layer.md` (C2), `oc-00-5-founder-decision-record.md` (D-C-02, D-C-05, D-C-06, D-C-07, D-C-08), `option-c-priority-and-gate-plan.md` (§P2), `p1-prereg-record.md` (frozen evaluation policy), `spec-oc-01-outcome-ledger.md` (dependency substrate, §16 change control).
**Gate precondition satisfied:** OC-01 evidence gate complete (`93a76d7`) and P1 preregistration frozen (`c080722`/`65f7e4f`). R10 truth-table inputs both present.

## 1. Intent, scope, and non-claims

**Intent.** Implement the C2 attribution ladder over OC-01 verified
outcome ledgers: deterministic M0 (raw string-overlap backtracking), M1
(normalized-value nomination), M2 v1 (explicit structural citation only),
and shortlist-bound M3/M4 judge adapters (counterfactual ablation and
Shapley-sampling coalition attribution). Produce a content-addressed,
tamper-evident `AttributionReportV1` carrying per-event mechanism tags,
extractor/judge identity, version, and configuration hash.

**Composition rule (frozen, D-C-06).** Cheap mechanisms nominate;
expensive mechanisms verify. M3/M4 execute only on the frozen M0–M2
shortlist, never on the full event set. The nomination domain itself is
bounded: only events already referenced by the verified ledger (input
refs, attempts, dead ends, marks — occurrence-capped at 4,096 by OC-01)
are eligible for nomination.

**Scope boundary.** OC-02 computes attribution reports. It does not
modify `SignedOutcomeLedgerV1`, does not promote caller-supplied
attribution marks to causal evidence (OC-01 §16), does not implement the
positive prior (OC-03/P3 track), does not enable Thorn, and does not
change Option A/B wires or `Selector::select`.

**Non-claims.** Passing every OC-02 gate establishes deterministic
mechanism correctness, provenance completeness, bounded-cost compliance,
and fail-closed judge behavior within tested bounds only. It does not
establish: causal correctness of any production judge, C3/C4/C5
completion, salience utility beyond the preregistered evaluation regime,
or dominance of the ladder outside the frozen evaluation configuration.
The E1-rerun evaluation (§15) reports measurements; D-C-06 #4 requires it
before C2 completion, and its thresholds are read from the frozen P1
preregistration — never chosen after inspection.

## 2. Definitional gaps closed (prereg-record §4.6)

The P1 freeze sealed names and values; this specification defines the
four disclosed gaps without altering any frozen value:

1. **Session** — one attribution computation bound to exactly one
   verified `SignedOutcomeLedgerV1` (same outcome_id) within one Store
   context (the context verified by `verify_against_dag`). All per-session
   caps (M3 ≤ 8, M4 ≤ 64 samples/candidate and ≤ 128 judge calls) are
   counted over this unit.
2. **Normalization window** — per-arm min-max normalization is computed
   over the candidate set of the current session only (per-session,
   per-arm), never globally across sessions.
3. **Label semantics (evaluation track only)** — `required`: removing the
   event changes or prevents the recorded outcome; `supporting`: event
   content is used but a substitute carrier exists; `irrelevant`: event
   content is not used by the outcome; `dead_end`: event appears only in
   failed attempts recorded as dead ends; `uncertain`: evidence is
   insufficient to classify. These define the frozen label names for
   scoring; they do not alter the names themselves.
4. **Per-stratum minimum sizes (E1-rerun protocol, founder-adjustable
   before freeze)** — terminal_with_full_cost ≥ 12, terminal_with_partial_cost
   ≥ 12, unterminated ≥ 8, strict_all_gold_tf0 ≥ 8 sessions; target 48 total
   (matching the OC-00 prototype scale). Inconclusive if any stratum is
   under minimum; the gate is never lowered post hoc (D-C-10 #5–6).

## 3. Dependency-order implementation contract

Implementation proceeds strictly in this order; each stage lands green
(fmt, clippy `-D warnings`, focused tests, workspace regression) before
the next begins:

1. **Stage 1A — workspace immutability.** No new external dependencies.
   The Judge adapter is a trait in `contextmesh-salience`; adapter
   implementations live behind it. No model inference inside deterministic
   verification. Forbidden-capability scan extended unchanged in scope.
2. **Stage 2A — constants, mechanism tags, configuration.** Mechanism
   enum (exactly `M0, M1, M2, M3, M4`), extractor version strings
   (matching the frozen prereg strings), `AttributionConfigV1` with
   canonical serialization and BLAKE3 config hash; report ID domain
   separation constant.
3. **Stage 2B — M0 raw string-overlap core.** Deterministic token
   extraction from ledger-referenced event payloads; overlap backtracking
   against outcome/answer evidence; nomination edges.
4. **Stage 2C — M1 normalized nomination core.** Numeric parsing with
   unit suffixes (k/M/B/G, %), case/whitespace folding, path
   normalization; normalized-equality nomination.
5. **Stage 2D — M2 v1 explicit structural extractor.** Recognition of
   exactly five verifiable structures (D-C-07): explicit canonical
   EventId citations, provider request/result linkage from core public
   event metadata, Option B receipt/handoff references, summary coverage
   enumerations, and signed artifact references (`ocout1_`). Forged or
   unverifiable links reject. Every edge records extractor identity,
   version, and configuration hash.
6. **Stage 2E — shortlist policy.** Union of M0–M2 nominations,
   EventId-deduplicated, deterministic ordering, cap 32 (0/32/33
   boundaries), empty shortlist recorded as `no nominations` (success,
   not error), shortlist recall computed and recorded separately from
   causal-verifier recall (D-C-06 #3). Because M0–M2 are frozen boolean
   nominators, every uniquely nominated event has `score_ppm = 1000000`
   in OC-02; therefore EventId ascending is the effective Stage 2E
   tie-break. OC-03/OC-04 prior or lexical scores never enter this stage.
   Stage 2E emits the exact `CausalStatus::NoNominations` marker/status
   value for an empty shortlist; Stage 2H owns assembly and canonical
   serialization of the complete `CausalSectionV1` containing that value.
7. **Stage 2F — Judge trait and M3 adapter bound.** `OutcomeJudge` trait
   (ablation); M3 runs only on shortlist entries, ≤ 8 calls/session,
   each call recorded with judge identity/version/config hash;
   judge-unavailable → `MechanismUnavailable` (reserved OC-01 category,
   no new categories), causal section emitted with uncertainty marker,
   M0/M1/M2 marks retained, no causal claim text produced. Stage 2F owns
   J01–J06 and J13–J14. It emits a typed partial M3 adapter section;
   Stage 2H owns complete `CausalSectionV1` assembly and canonical report
   bytes. J12's full transcript replay/verification evidence therefore
   lands with Stage 2H. Stage 2G owns J07–J11 and adds the final coalition
   method plus its request/response types to the judge surface.
8. **Stage 2G — M4 adapter bound.** Shapley-sampling coalition
   attribution over the shortlist; ≤ 64 samples/candidate and ≤ 128
   judge calls/session; redundant-carrier credit split verified against
   fixed fixtures; same fail-closed semantics as 2F.
9. **Stage 2H — AttributionReportV1 assembly.** Canonical strict-JSON
   bytes, BLAKE3 domain-separated report ID, deterministic-tier
   byte-exact reproduction guarantee, adapter-tier verbatim recording.
10. **Stage 2I — adversarial, boundary, and privacy vectors.** Hostile
    payloads panic-free; bounded token lists; no credentials or raw
    private transcript content in reports (fingerprints only); u128
    widened checked arithmetic; cross-ledger/cross-context rejection.
11. **Evidence stage.** Matrix coverage proof and `verify-oc02.sh`
    full-pipeline gate mirroring the OC-01 eight-stage structure.
12. **Evaluation stage (separate track).** E1 rerun under the exact
    production pipeline and frozen P1 configuration, before C2 completion
    (D-C-06 #4). Produces measurements only.

## 4. Workspace and dependency gate

- No new entries in `contextmesh-salience/Cargo.toml` `[dependencies]`
  beyond the OC-01-frozen set (blake3, ed25519-dalek, serde, and core
  contextmesh as landed in OC-01 1A).
- Dependency direction remains strictly one-way: salience → core.
- The Judge trait introduces zero dependencies; adapter crates are
  out-of-tree sidecars and are not part of this package.
- Workspace regression, closure immutability, and legacy OA/OB chains
  remain green and untouched.

## 5. Frozen constants, domains, and encodings

| Constant | Value |
|---|---|
| Mechanism enum | exactly `M0, M1, M2, M3, M4` (no others, ever) |
| m0 version | `oc-prototype-m0-v1-compatible` (frozen in prereg) |
| m1 version | `oc-1-m1n-v1` (frozen in prereg) |
| m2 version | `oc-2-m2-v1` (frozen prereg placeholder; this spec binds it) |
| prior version (reference only) | `oc-3-prior-v1` (not used by OC-02) |
| Report ID domain | `oc-02-attr-report-v1` + NUL (BLAKE3, literal-domain, same pattern as OC-01 `OUTCOME_ID_DOMAIN`) |
| Report ID prefix | `ocattr1_` (base62-style typed prefix, 50 chars total like `ocout1_`) |
| Config hash domain | `oc-02-attr-config-v1` + NUL over canonical `AttributionConfigV1` bytes |
| Shortlist cap | 32 (frozen) |
| M3 judge calls / session | ≤ 8 (frozen) |
| M4 samples / candidate | ≤ 64 (frozen) |
| M4 judge calls / session | ≤ 128 (frozen) |
| Prereg reference hash | SHA-256 `be20d8fc48771098e745038b906dd13456ffcebdeb424cee25e91d52eae784c9` (commit `c080722` blob) |
| Nomination domain bound | ledger event-reference occurrences (≤ 4,096, inherited from OC-01) |
| Nomination token bound | ≤ 256 tokens per event payload; token length ≤ 1,024 bytes |
| Numeric normalization magnitudes | ≤ 10^18 absolute; u128 widened intermediate; out-of-range → nomination skipped (recorded, not error) |

## 6. Strict JSON and canonical ordering

Identical rules to OC-01 §6: strict parser (no duplicates, no NaN,
depth ≤ 64), JCS canonical ordering (BTreeMap key order, UTF-8, no
whitespace), byte-exact serialization for the deterministic tier. The
adapter tier records judge outputs as opaque canonical-JSON values
verbatim (no re-derivation, no sanitization beyond schema validation).

## 7. Frozen JSON value schemas

All schemas are strict: unknown members reject (`Malformed`), tagged
variants use the OC-01 disjoint-prefix discipline, nulls illegal except
where explicitly typed.

### 7.1 AttributionMechanismTag
```json
{"mechanism": "M0", "extractor_version": "oc-prototype-m0-v1-compatible",
 "config_hash": "ocattrcfg1_…"}
```
Exactly three required members; `mechanism` ∈ the 5-value enum;
`config_hash` is the typed hash of `AttributionConfigV1`.

### 7.2 NominationEdgeV1
```json
{"event": "…EventId text…", "mechanisms": [AttributionMechanismTag, …],
 "evidence_kind": "overlap|normalized|citation|linkage|receipt|summary|artifact",
 "evidence_fingerprint": "ocfp1_…"}
```
`evidence_fingerprint` is a BLAKE3 fingerprint of the minimal evidence
bytes (never raw transcript content).

### 7.3 ShortlistV1
```json
{"entries": [ {"event": "…", "rank": 1,
  "nominating_mechanisms": ["M0","M2"], "score_ppm": 1000000}, … ],
 "cap": 32, "dedup": "EventId", "order": "score_ppm desc, EventId asc",
 "recall_basis": {"nominated": 0, "eligible": 0}}
```
Deterministic order: score desc, then canonical EventId ascending.
For OC-02, M0/M1/M2 are boolean nominators: every EventId present in
their deduplicated union has `score_ppm = 1000000`. No mechanism weight,
prior score, lexical score, or caller-supplied score is inferred here.
Consequently all Stage 2E entries tie on score and canonical EventId
ascending determines their relative order and cap-boundary retention.

### 7.4 CausalSectionV1
```json
{"status": "computed|unavailable|no_nominations",
 "m3": [ {"event": "…", "delta_kind": "changed|unchanged|unavailable",
   "judge": "…identity…", "judge_version": "…", "judge_config_hash": "…"} , …],
 "m4": [ {"event": "…", "share_ppm": 500000, "samples": 64,
   "judge": "…", "judge_version": "…", "judge_config_hash": "…"}, …],
 "uncertainty_markers": ["judge_unavailable", …]}
```
`status: unavailable` carries `uncertainty_markers` including
`judge_unavailable`; no causal claim vocabulary appears in any
`computed`-status output text beyond measured deltas.

#### 7.4.1 Stage 2F typed M3 adapter contract

- `JudgeIdentity` is the existing OC-01 `MechanismRecordV1`; its frozen
  bounds and typed `Blake3HashText` configuration hash are reused rather
  than redefined.
- `AttributionSessionKeyV1` is exactly `(OutcomeId, ContextId)`. A fresh
  `run_m3` invocation owns a fresh local call counter, so caps are counted
  independently per ledger/context computation.
- `AblationRequestV1<'a>` contains only a borrowed session key and the
  typed shortlist `EventId`. It carries no transcript, payload, path,
  credential, wall-clock, I/O handle, or model client; an out-of-tree
  judge implementation owns any execution context it needs.
- The judge's ablation response is exactly `changed|unchanged`.
  `unavailable` is adapter-recorded data, never a judge-invented causal
  claim. Every recorded M3 delta flattens the returned
  `MechanismRecordV1` into judge identity, version, and config hash.
- Empty shortlist returns `no_nominations` with zero calls. For a
  nonempty shortlist and `judge: None`, return a successful typed adapter
  result with status `unavailable`, failure category
  `MechanismUnavailable`, exact marker `judge_unavailable`, and the
  deterministic shortlist left untouched.
- A mid-run `JudgeUnavailable` keeps completed deltas, records the current
  and remaining shortlist entries as `unavailable`, stops all further
  calls, and returns the same category/marker semantics.
- The ninth requested M3 call is never made. After eight calls, remaining
  entries are recorded `unavailable`, status is `unavailable`, failure
  category is `MechanismUnavailable`, and the exact marker is
  `m3_call_cap`.
- Stage 2F's typed partial section has no free-form causal prose and no
  canonical report serializer. Stage 2H maps it into the frozen §7.4
  fields and owns byte-level claim scans and transcript replay.
- The partial section uses the M3-specific status enum
  `complete|unavailable|no_nominations`; it never emits the full causal
  status `computed`. Stage 2H may map to `CausalStatus::Computed` only
  after every required adapter tier, including M4, has completed.
- M3 partial-section and delta fields are privately constructed by the
  validated adapter and exposed read-only. External callers cannot forge
  authoritative-looking combinations for later Stage 2H consumption.

### 7.5 AttributionReportV1 (envelope)
```json
{"version": 1, "report_id": "ocattr1_…", "ledger_id": "ocout1_…",
 "task_fingerprint": "…", "input_snapshot_fingerprint": "…",
 "prereg_reference": "be20d8fc…", "config_hash": "ocattrcfg1_…",
 "deterministic_tier": { ShortlistV1, edges… },
 "adapter_tier": { CausalSectionV1 },
 "terminal_status": "terminal|unterminated"}
```
Exactly these top-level members. Unterminated ledgers: deterministic tier
may still nominate from attempt/dead-end evidence, but the
`adapter_tier.status` is exactly `no_nominations` (the enum value —
never `computed` for an unterminated ledger) with marker
`no_terminal_outcome`; no fabricated causal content.

## 8. Public API semantics

```rust
pub struct AttributionConfigV1 { /* caps, versions, normalization params */ }
impl AttributionConfigV1 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, OutcomeError>;
    pub fn config_hash(&self) -> Result<ConfigHash, OutcomeError>;
}

pub trait OutcomeJudge {
    fn identity(&self) -> MechanismRecordV1;       // recorded, never inferred
    fn ablate(&self, req: AblationRequestV1<'_>) -> Result<AblationDeltaV1, JudgeUnavailable>;
    // Stage 2G adds the final coalition method and its exact types.
}

pub fn compute_attribution(
    ledger: &SignedOutcomeLedgerV1,
    events: &EventSource<'_>,                    // read-only, ledger-referenced only
    config: &AttributionConfigV1,
    judge: Option<&dyn OutcomeJudge>,            // None ⇒ causal tier unavailable
) -> Result<AttributionReportV1, OutcomeError>;
```

- `compute_attribution` verifies the ledger structurally first (reuse
  OC-01 `from_wire` discipline); a tampered ledger is rejected before any
  nomination work.
- `EventSource` exposes only events referenced by the ledger in the
  verified context; requests outside that set return
  `UnauthorizedEvent` (reserved category).
- `judge: None` or any `JudgeUnavailable` fail-closes the causal tier
  only. The deterministic tier completes whenever ledger verification
  itself succeeds (a tampered or unverifiable ledger fails the whole
  call with `Err`, per §8 bullet 1 and rows A20/R08).
- No function in this module performs I/O, reads wall-clock, or embeds
  model inference.

## 9. Determinism and provenance rules

1. Given identical (ledger, events, config) and a fixed judge
   transcript, `compute_attribution` reproduces byte-identical
   deterministic-tier bytes; the adapter tier reproduces byte-identical
   bytes when the judge transcript is replayed verbatim.
2. `report_id` = BLAKE3(`oc-02-attr-report-v1` + NUL + canonical report
   bytes with the `report_id` member set to its fixed derivation
   placeholder — the literal string `"report_id"`); the derived value is
   then substituted into exactly that position in the sealed bytes,
   making construction the only writer of the ID. Flipping any byte of
   the sealed bytes invalidates. (Clarified 2026-08-29: the frozen
   wording "canonical full report bytes" omitted the placeholder
   substitution that breaks the self-reference; implementation behavior
   and all derived IDs are unchanged.)
3. Every nomination edge, M3 delta, and M4 share carries mechanism tag,
   extractor/judge identity, version, and configuration hash (C2 intent).
4. Verification rebuilds the deterministic tier and compares bytes;
   adapter-tier values are compared against the recorded judge transcript
   (never re-queried).
5. Caller attribution marks inside the ledger are inputs to nothing;
   they are copied into no report section (no promotion, OC-01 §16).

## 10. Exact implementation file map

| File | Contents |
|---|---|
| `contextmesh-salience/src/attribution.rs` | mechanism tags, config, M0/M1/M2 cores, shortlist policy, report assembly (public API of §8) |
| `contextmesh-salience/src/judge.rs` | `OutcomeJudge` trait, request/response types, `JudgeUnavailable`, transcript types |
| `contextmesh-salience/tests/oc02_schema.rs` | T01–T10 |
| `contextmesh-salience/tests/oc02_mechanisms.rs` | A01–A26 (M0/M1/M2) |
| `contextmesh-salience/tests/oc02_shortlist_judges.rs` | S01–S08, J01–J14 |
| `contextmesh-salience/tests/oc02_reports.rs` | R01–R08 |
| `contextmesh-salience/tests/oc02_adversarial.rs` | X01–X10 |
| `contextmesh-salience/tests/oc02_evaluation.rs` | V01–V05 (E1-rerun harness, deterministic-replay mode) |
| `scripts/verify-oc02.sh` | 9-stage gate pipeline |
| `_bmad-output/planning-artifacts/oc-02-test-traceability-matrix.md` | full matrix |

No production file outside this map changes.

## 11. Golden, boundary, and adversarial contract

- **Fixed fixtures.** A fixed admitted DAG with a signed terminal ledger
  (reusing the OC-01 fixture-builder discipline): one lone carrier event,
  one redundant pair, one reformatted numeric carrier (`9.5M` ↔
  `9500000`), one explicit EventId citation, one forged citation whose
  event does not exist, one receipt reference, one irrelevant event.
  Fixture bytes and SHA-256 committed; generator `#[ignore]`d with change
  control.
- **Boundaries.** Every frozen cap tested at 0 / exact-max / max+1:
  shortlist 32, M3 calls 8, M4 samples 64, M4 calls 128, tokens 256,
  token bytes 1,024, magnitude 10^18.
- **Hostile inputs.** BOM, trailing data, deep nesting, duplicate keys,
  NaN/Infinity in payloads — panic-free, `Malformed` or skip-and-record.
- **Privacy.** Reports contain fingerprints only; no raw transcript
  bytes, no credentials, no private paths (OC-01 X18/X19 scan reused).

## 12. Documentation and evidence standard

Identical to OC-01 §13: four-layer evidence (Sources / Reasoning /
Conclusion derivation / Invalidators), machine-audited by
`verify-oc02.sh`, claim language bounded, non-claims explicit.

## 13. Verification commands

```sh
bash scripts/verify-oc02.sh --self-test
bash scripts/verify-oc02.sh --planned-surface-only
bash scripts/verify-oc02.sh            # full 9-stage pipeline
```
Stages: SETUP → SCHEMA → MECHANISMS → SHORTLIST → JUDGES → REPORTS →
ADVERSARIAL → REGRESSION → EVIDENCE (9-stage pipeline; the gate roll-up's
`OC02-ADVERSARIAL` executes the X rows). Cargo invocations use the OC-01
environment (`OC01_INNER_CURRENT_GATE=1`, cache `TMPDIR`, offline
`--locked`).

## 14. Acceptance gates

- **OC02-SETUP:** workspace immutable (no new deps, one-way direction,
  forbidden-capability scan green);
- **OC02-SCHEMA:** T01–T10 pass (tag exactness, config-hash binding,
  strict schemas, unknown-member rejection);
- **OC02-MECHANISMS:** A01–A26 pass (M0/M1/M2 positives, negatives,
  provenance, determinism);
- **OC02-SHORTLIST:** S01–S08 pass (cap boundaries, dedup, ordering,
  empty-shortlist recording, separate recall basis);
- **OC02-JUDGES:** J01–J14 pass (shortlist-only execution, call caps,
  fail-closed unavailability, uncertainty markers, no causal vocabulary);
- **OC02-REPORTS:** R01–R08 pass (ID domain separation, tamper
  rejection, deterministic-tier byte-exact rebuild, adapter-tier
  verbatim recording, cross-context/cross-ledger rejection,
  unterminated honesty);
- **OC02-ADVERSARIAL:** X01–X10 pass (all-cap boundary sweep,
  hostile-payload panic freedom, privacy scan, OC-01 error-category
  immutability, fail-closed no-partial-artifact, noncanonical report
  rejection);
- **OC02-REGRESSION:** full workspace + legacy chains unchanged;
- **OC02-EVIDENCE:** four-layer evidence and claim audit complete;
- **OC02-EVALUATION (C2 completion prerequisite, D-C-06 #4):** E1 rerun
  under the frozen P1 configuration with per-mechanism
  precision/recall/F1, judge-call economics, and separately recorded
  shortlist recall. No threshold is selected or altered after label
  inspection; inconclusive is reported as inconclusive.

## 15. Change control

Founder approval required before changing: the mechanism enum, any frozen
version string or cap, the report/config domains and prefixes, schema
member names or requiredness, the five M2 v1 recognized structures,
fail-closed judge semantics, the nomination-domain bound, or the
definitional values of §2. A fixture change requires a version/change
decision and explicit human review. Discoveries return to specification
review; they are never silently normalized in code.

## 16. Review checklist

- [ ] Mechanism ladder exactly matches C2 intent and D-C-06/D-C-07.
- [ ] M3/M4 provably shortlist-bound (types + tests + cap arithmetic).
- [ ] Judge-unavailable is fail-closed with uncertainty markers; no
      causal claim text exists on unavailable paths.
- [ ] All frozen prereg values (caps, versions, formula inputs) are
      consumed verbatim, never redefined.
- [ ] Definitional gaps of §2 close prereg-record §4.6 without altering
      frozen values; per-stratum minimums flagged founder-adjustable.
- [ ] No ledger mutation, no mark promotion, no Option A/B wire change.
- [ ] Report privacy: fingerprints only.
- [ ] Matrix rows map 1:1 to committed tests; no unmapped rows.

## 17. Approval record

- 2026-08-26: draft created post-P1-freeze; dual independent review
  pending; founder approval pending. This document authorizes nothing
  until approved and frozen.
- 2026-08-26: Quality review REJECT (2 blockers: row count 91→81,
  missing ADVERSARIAL gate; 4 warnings) → all six fixes applied →
  verification re-review APPROVE (0 blockers).
- 2026-08-26: Compliance review GO (0 blockers, 11 API calls; C2
  fidelity, D-C-06/D-C-07 alignment, prereg hash independently
  recomputed, 81 row IDs verified contiguous, scope discipline clean).
- 2026-08-26: **Lunarpulse approved the spec and matrix for freezing,
  including the §2.4 per-stratum minimums (≥12/≥12/≥8/≥8, target 48)
  verbatim** (Discord message `1541934459069145168`). Status is now
  `approved-for-implementation`. Implementation proceeds in §3
  dependency order.
- 2026-08-27: **Lunarpulse approved the minimal Stage 2E freeze
  clarification** (Discord message `1542499082533343264`): boolean
  M0–M2 nominations score exactly 1,000,000 ppm with EventId ascending
  as the effective tie-break; Stage 2E emits the `no_nominations`
  marker/status value while Stage 2H owns complete `CausalSectionV1`
  assembly and serialization. No cap, wire member, mechanism, error
  category, or Option A/B contract changed.
- 2026-08-27: **Lunarpulse approved the minimal Stage 2F Judge/M3 freeze
  clarification** (Discord message `1542525263240364093`): J01–J06 and
  J13–J14 are Stage 2F-owned; J07–J11 are Stage 2G-owned; J12 full replay
  is Stage 2H-owned. Judge provenance reuses `MechanismRecordV1`, session
  identity is `(OutcomeId, ContextId)`, exact unavailable/cap markers and
  successful partial-section semantics are frozen as §7.4.1, and the
  coalition method remains deferred to Stage 2G. No existing wire, cap,
  error enum, dependency direction, or Option A/B contract changed.
- 2026-08-28: **Lunarpulse approved the minimal Stage 2G M4 freeze
  clarification** (Discord message `1542555983090557129`), frozen as
  spec §7.4.2: shortlist-only coalition requests (typed target EventId +
  32-bit subset mask), judge answers exactly
  `contributing | not_contributing`, u128-checked `share_ppm` summing to
  ≤1,000,000 ppm with unallocated remainder recorded, deterministic
  lexicographic permutation sampling under the frozen caps, exact marker
  `m4_call_cap`, M4 partial-section status
  `complete | unavailable | no_nominations`, and typed-M4-partial-only
  ownership for Stage 2G (CausalSectionV1 assembly and J12 stay Stage
  2H-owned). Row count and gate IDs are unchanged.
- 2026-08-29: **Lunarpulse approved the Plan A wording-only corrections
  for the two unresolved change-control items** (Discord message
  `1543000276221427912`): (1) spec §9.2 rule 2 now states that
  `report_id` is derived over canonical bytes with the `report_id`
  member set to its fixed derivation placeholder — closing the
  specification gap where the frozen wording ("canonical full report
  bytes") omitted the placeholder substitution that breaks the
  self-reference; implementation behavior, all derived IDs, and the
  golden fixture are unchanged. (2) Matrix row OC02-R02's evidence
  column now states the test's actual assertion scope: rebuild is
  byte-identical across judge presence and reruns; committed-fixture
  byte equality is owned by OC02-R07. Both edits are wording-only;
  no code, test, cap, wire member, mechanism, or Option A/B contract
  changed. Clarification eligibility test recorded: each edit stands
  with zero code/test changes — verified by `git diff --stat` showing
  only the two document files.

### §7.4.2 Stage 2G typed M4 adapter contract (frozen 2026-08-28)

- `CoalitionRequestV1<'a>` is privately constructed by the validated
  adapter only: borrowed session key, one typed target `EventId` from
  the shortlist, and a bounded 32-bit subset mask selecting shortlist
  positions. It carries no transcript, payload, path, credential,
  wall-clock, I/O handle, or model client.
- The judge's coalition response is exactly `contributing |
  not_contributing`. `JudgeUnavailable` reuse and all fail-closed
  semantics (None, mid-run, cap) mirror §7.4.1.
- Every recorded M4 share flattens the returned `MechanismRecordV1`
  into judge identity, version, and config hash, and carries the
  sample count actually consumed (≤64).
- `share_ppm` arithmetic is u128-checked; recorded shares sum to at
  most 1,000,000 ppm; any unallocated remainder is recorded data, never
  fabricated.
- Sampling order is a fixed lexicographic permutation schedule,
  identical across runs (byte-reproducible), bounded by the frozen caps
  so the 129th judge call in a session is never made.
- The M4 partial section and its share fields are privately constructed
  and exposed read-only; it never emits full causal `computed`; the
  exact cap marker string is `m4_call_cap`.
