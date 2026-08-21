---
title: 'OC-0.5 Option C Integration Founder Decision Record'
type: 'decision-record'
created: '2026-08-21'
status: 'approved'
approved: '2026-08-21'
approved_by: 'Lunarpulse'
approval_source: 'Discord message 1540302757649457254'
baseline_commit: '17d7730ad18f142e55b67a650f060814534507f8'
source_spec: '../implementation-artifacts/spec-option-c-salience-provenance-layer.md'
priority_plan: './option-c-priority-and-gate-plan.md'
integration_audit: '../verification-artifacts/oc-00-5-post-rebase-integration-audit.md'
real_data_evidence: '../verification-artifacts/oc-00-5-real-data-replay.md'
---

# OC-0.5 Option C Integration Founder Decision Record

## 1. Scope of approved disposition

This record resolves the integration decisions exposed when Option B merged to
`main` and `OC-AttentionLedger` was rebased. It records the founder disposition
required before Option C production code.

Approval authorizes P1 planning and implementation under these decisions. It
does **not** declare C1–C5 complete, approve current Thorn, freeze the current
multiplicative formula, convert silver replay labels into causal gold, or alter
Option A/B frozen wires.

Lunarpulse approved D-C-00 through D-C-10 in full on 2026-08-21. The decisions
below are frozen founder dispositions; change control applies.

## 2. Evidence and reasoning method

### Sources

| Source | Reliability | Relevant finding |
|---|---:|---|
| local Git graph and Option B source/tests at baseline | Primary | B is merged, tested, and the integration base |
| approved Option C spec | Primary frozen intent | C1–C5 intent and charter |
| post-rebase integration audit | Primary audit | API seams and contract mismatches |
| deterministic Python/Rust prototype | Primary synthetic execution | mechanism direction, not production equivalence |
| Hermes real-data replay | Primary silver-proxy execution | positive prior repeats direction; Thorn and TF=0 remain unresolved |

### Decision rule

Choose the smallest additive contract that:

1. preserves Option A/B frozen behavior;
2. fails closed on unverifiable references;
3. records enough provenance to reproduce or audit every deterministic result;
4. separates demonstrated mechanisms from experimental ones;
5. leaves an explicit rejection path if human-gold evidence fails.

## 3. Proposed decisions

### D-C-00 — Option C disposition and product boundary

**Status:** Approved for gated production development.

1. Option C is a derived salience-provenance layer over Option A history and
   ahead of Option B source closure/delta/handoff.
2. Option A owns signed immutable events; Option B owns safe bounded handoff;
   Option C owns outcome evidence, attribution, priors, and recorded ranking
   influence.
3. Option C must not claim objective salience or downstream success without the
   corresponding human/B8 gate.

**Consequence:** The Option C draft may be approved with the OC-0.5 amendments
in this record; P1 is authorized only after approval metadata is recorded.

### D-C-01 — Separate crate and one-way dependency

**Status:** Approved.

1. Add a sibling library crate `contextmesh-salience/` in a root Cargo workspace.
2. Dependency direction is exactly:

   ```text
   contextmesh-salience → contextmesh
   contextmesh -X→ contextmesh-salience
   ```

3. Core `contextmesh` keeps its present dependency closure except for the
   additive public bridge in D-C-03.
4. Model, embedding, and judge integrations live in optional external adapter
   crates/processes behind OC traits; they are not default dependencies of
   either core crate.
5. Rust remains pinned at 1.97; every new direct dependency requires a recorded
   feature/MSRV/supply-chain preflight.

**Rejected:** implementing OC modules inside `src/`, or making the core crate
call the salience crate.

### D-C-02 — Artifact envelope, signature domains, failures, and hard bounds

**Status:** Approved for OC v1 design; exact vectors freeze in each
package spec before implementation.

1. OC artifacts are strict canonical JSON, content-addressed, Ed25519-signed,
   exported artifacts outside the Option A store. The six v1 bodies and their
   domains are unambiguous:

   | Body | ID prefix | ID derivation domain | Signature domain |
   |---|---|---|---|
   | `OutcomeLedgerV1` | `ocout1_` | `org.aaif.contextmesh.oc.outcome-ledger-id.v1\0` | `org.aaif.contextmesh.oc.outcome-ledger-signature.v1\0` |
   | `AttributionRecordV1` | `ocatr1_` | `org.aaif.contextmesh.oc.attribution-id.v1\0` | `org.aaif.contextmesh.oc.attribution-signature.v1\0` |
   | `ThornIndexV1` | `octhn1_` | `org.aaif.contextmesh.oc.thorn-index-id.v1\0` | `org.aaif.contextmesh.oc.thorn-index-signature.v1\0` |
   | `SaliencePriorV1` | `ocpr1_` | `org.aaif.contextmesh.oc.salience-prior-id.v1\0` | `org.aaif.contextmesh.oc.salience-prior-signature.v1\0` |
   | `SelectionInfluenceV1` | `ocsel1_` | `org.aaif.contextmesh.oc.selection-influence-id.v1\0` | `org.aaif.contextmesh.oc.selection-influence-signature.v1\0` |
   | `SelectionExecutionV1` | `ocexec1_` | `org.aaif.contextmesh.oc.selection-execution-id.v1\0` | `org.aaif.contextmesh.oc.selection-execution-signature.v1\0` |

   Each ID is the typed prefix plus the unpadded base64url encoding of 32 BLAKE3
   bytes over its ID-domain prefix followed by canonical body bytes. The
   signature covers its signature-domain prefix followed by the derived ID.
   Unknown or cross-type domains fail.
2. Every referenced EventId must exist, verify, be authorized, and share the
   stated ContextId. Verification is atomic and fail-closed.
3. Typed failure categories are: malformed, noncanonical, unsupported-version,
   limit-exceeded, id-mismatch, signature-invalid, missing-event,
   unauthorized-event, context-mismatch, stale-input, mechanism-unavailable,
   and incomplete-input. No partial artifact is returned.
4. Every artifact binds to `ContextId`, a canonical Option A input-ref snapshot
   fingerprint, mechanism/config fingerprint, and any recipient head used.
   Issuance verifies authorization against the current append-only Option A
   policy. Re-verification validates the immutable events and artifact; execution
   additionally compares the recorded ref snapshot and recipient head and
   returns `stale-input` if either moved. Option A presently has no revocation,
   so this records the exact verification snapshot without inventing historical
   authorization semantics.
5. Hard v1 wire maxima, with caller limits allowed only downward:

   | Bound | Maximum |
   |---|---:|
   | one canonical artifact | 2,097,152 bytes |
   | Outcome Ledger event references | 4,096 |
   | Outcome Ledger attempts/dead ends | 1,024 each |
   | Attribution candidate references/marks | 4,096 each |
   | Thorn Index conditional failure entries | 4,096 |
   | Prior scored output entries | 4,096 |
   | Selection Influence ordered pre-closure references | 4,096 |
   | Selection Execution ordered pre-closure references | 4,096 |
   | warnings/uncertainty notes | 64 |
   | one note | 1,024 bytes |
6. Prior computation has separate logical work bounds of 100,000 nodes and
   1,000,000 derived edges. Those edges are **not serialized** into the 2 MiB
   `SaliencePriorV1`. The artifact binds the deterministic graph builder,
   extractor/config hash, input-ref snapshot, logical node/edge counts, and up
   to 4,096 scored output entries. Verification rebuilds the bounded graph from
   the recorded Option A snapshot and requires the same output fingerprint.
   There is no implicit chunking or external mutable graph file in v1.
7. A `SelectionExecutionV1` does not serialize a possible 100,000-event B3
   closure. It records the ordered pre-closure IDs, B3 policy/limit fingerprint,
   critical-candidate-set fingerprint, closed-selection wire hash/count, delta
   wire hash/count, final handoff wire hash, recipient head, and warnings. A
   verifier recomputes B3–B5 and compares every fingerprint.

8. Bounds are provisional design constants until package-spec review produces
   canonical boundary vectors; changing them after founder approval requires
   founder change control.

**Reason:** aligns with existing B selection/reference bounds while preventing
unbounded graph or artifact work.

### D-C-03 — Additive verified source-reference bridge

**Status:** Approved.

Add only:

```rust
impl SourceEvent {
    #[must_use]
    pub fn reference(&self) -> SourceReference;
}
```

The method delegates to the existing internal construction and exposes no
unchecked public field constructor. `SourceReference` fields remain private.
Option A wire, Option B receipt wire, and `Selector` remain unchanged.

**Reason:** an external OC crate can return verified B-compatible references
without duplicating or forging metadata.

### D-C-04 — Rich OC selection result, no B2 trait widening

**Status:** Approved.

`contextmesh-salience` owns one signed `SelectionInfluenceV1` shaped as:

```text
SelectionInfluenceV1
- task and context binding
- ordered SourceReferences
- per-reference lexical/prior/thorn components
- candidate-entry reason
- prior/artifact IDs
- selector identity/version/config hash
- fixed parameters
- warnings and uncertainty
- input-head/config fingerprints
```

The callable adapter contract is:

```text
execute_selection(store, verified SelectionInfluenceV1,
                  SelectionBudget, CriticalPolicy, ClosureLimits,
                  RecipientState)
  → verify input snapshot and recipient head
  → compile ordered SourceReferences under the exact B2 budget
  → extract the exact ordered pre-closure EventIds
  → deterministically project critical candidates from the recorded input refs
  → call B3 close_selection with those IDs and projected candidate/policy set
  → call B4 compute_delta
  → construct and B5-verify Handoff
  → attach B6 uncertainty derived from influence warnings
  → issue SelectionExecutionV1 binding all hashes/counts
  → return {handoff, execution_artifact}
```

If any step fails or the input/recipient state moves, neither execution artifact
nor deliverable handoff is returned. B3 may canonicalize IDs and add ancestors
or critical events; `SelectionExecutionV1` records that transformation by the
closed-selection hash/count and `added_critical` fingerprint. Ranking order
remains evidence in `SelectionInfluenceV1` but is not falsely claimed to survive
B3's set closure. The B3 candidate set is never an unverifiable caller list: it
is the sorted deterministic Option A projection reachable from the recorded
input-ref snapshot under a versioned projection rule. The execution artifact
records its fingerprint/count; verification rebuilds it and compares both.

Only `SourceReference`/EventId values enter existing Option B functions. The two
OC artifacts cryptographically bind influence to the actual B3–B6 execution.
Do not change `Selector::select -> Vec<SourceReference>` or Option B wires.

### D-C-05 — TF=0 policy: candidate union before deterministic reranking

**Status:** Approved; reject multiplication as the sole
candidate path.

1. Generate bounded lexical candidates and bounded positive-prior candidates
   independently.
2. Take deterministic EventId-deduplicated union.
3. Deterministically rerank the union with recorded components and canonical
   EventId tie-break.
4. A zero lexical score does not exclude a prior-nominated candidate.
5. Thorn remains disabled until D-C-10's independent gate.

The exact normalization, rerank formula, lexical/prior per-arm candidate caps,
deduplication, tie-break, and checked-overflow policy are part of the P1
preregistration. Their canonical config hash freezes before P2 implementation
and before any test-label inspection.

**Rejected:** pure multiplication as candidate generation, because `tf=0`
always remains zero; current real replay has no strict all-gold-TF=0 stratum and
cannot select a final formula.

### D-C-06 — M3/M4 remain shortlist-bound

**Status:** Approved; retain the shortlist policy and classify OC-00 M3 evidence as
directional.

1. M0, M1, and M2 nominate.
2. M3 and M4 verify only the frozen shortlist, never the whole DAG.
3. Record shortlist recall separately so a missed nomination is not mislabeled
   a causal-verifier failure.
4. Rerun E1 under this exact pipeline before C2 completion.

**Reason:** preserves the cost ladder and makes causal calls bounded. The current
prototype's all-candidate M3 does not validate this exact production policy.

### D-C-07 — M2 v1 is explicit structural citation only

**Status:** Approved.

M2 v1 recognizes only verifiable structure: explicit EventId citations,
provider request/result linkage, receipt/handoff references, summary coverage,
and signed artifact references. Every edge records extractor identity, version,
and configuration hash.

LLM-inferred citations are a future recorded adapter mechanism, never silently
mixed with deterministic M2 and never re-derived during verification.

### D-C-08 — Terminal, cost, and entity semantics

**Status:** Approved.

1. Outcome creation requires a caller-supplied terminal EventId or explicit
   `unterminated`; no heuristic terminal discovery in production.
2. Every cost field is `Available(value)` or `Unavailable(reason)`; missing
   wall-clock, tokens, calls, or retries are never inferred.
3. Tool/token/retry values are caller-recorded and source-provenanced.
4. Entity extraction is versioned and deterministic; each entity key is a
   typed local fingerprint plus extractor/config provenance.
5. Raw private transcript content and private paths/URLs are not required in a
   portable prior artifact. Export policy must support aggregate or opaque
   fingerprints and must never export credentials or chain-of-thought.
6. World-state-sensitive entities and failures carry context/recipient/task
   conditioning and, where applicable, expiry.

### D-C-09 — Fixed-point production scoring

**Status:** Approved.

1. Positive salience and thorn proximity are **separate nonnegative channels**:
   `prior_ppb` and `thorn_ppb`, each `0..=1,000,000,000`. Production does not
   use signed or negative PPR seeds.
2. Positive PPR is personalized by load-bearing seeds. Thorn PPR is separately
   personalized by eligible conditional failure seeds. Suppression occurs only
   in the later reranker and records both input channels and the applied rule.
   No channel is silently clamped into the other.
3. Configuration weights are nonnegative parts-per-million integers. Thorn
   suppression uses an explicit checked subtraction stage rather than a
   negative weight. Intermediate arithmetic is widened and overflow-checked
   before multiplication/addition/subtraction. Final channel values must remain
   in range or fail.
4. Production PPR and reranking require exact canonical-byte reproduction.
5. Float and tolerance remain research-harness tools only and never determine a
   production artifact ID.
6. Quantization error and rank inversions must be measured before C3 acceptance.

### D-C-10 — Evidence thresholds and mechanism rollout

**Status:** Approved.

1. Freeze a preregistered, temporal parent-family split and labels
   `required`, `supporting`, `irrelevant`, `dead_end`, `uncertain` before opening
   test labels.
2. Include a strict subset where every required/supporting candidate has TF=0.
3. Positive-prior C4 deployment requires:
   - prior-assisted selection beats lexical on preregistered primary nDCG@12
     and Any-hit@12 point estimates;
   - the 95% family-cluster bootstrap interval for nDCG@12 delta has lower bound
     above zero;
   - strict TF=0 Any-hit is above lexical and deterministic random;
   - no Option B B3–B8 regression.
4. Thorn is a separate later gate. It remains disabled unless it adds benefit
   over positive-only selection and suppresses fewer than 5% of human-labeled
   `required` events at the frozen budget; expiry and stale-world-state cases
   must pass.
5. Causal load-bearing and task-success claims require causal/human/B8 evidence,
   not the present entity-continuity silver proxy.
6. If sample size cannot support the interval, report inconclusive and do not
   lower the gate post hoc.

## 4. Decision matrix

| Decision | Blocks | Recommendation | Founder disposition |
|---|---|---|---|
| D-C-00 product boundary | all | approve | APPROVED |
| D-C-01 crate/dependency | P1 implementation | approve | APPROVED |
| D-C-02 artifact contract/bounds | C1–C4 | approve with package vectors | APPROVED |
| D-C-03 public bridge | C4 integration | approve | APPROVED |
| D-C-04 richer result | C4 provenance | approve | APPROVED |
| D-C-05 TF=0 candidate union | C4 | approve | APPROVED |
| D-C-06 shortlist M3/M4 | C2 | approve | APPROVED |
| D-C-07 explicit M2 | C2 | approve | APPROVED |
| D-C-08 terminal/cost/entity | C1/C3 | approve | APPROVED |
| D-C-09 fixed-point | C3/C4 | approve | APPROVED |
| D-C-10 thresholds/rollout | C3–C5 | approve | APPROVED |

## 5. Audit-to-decision traceability

| OC-0.5 blocker | Decision | Owning package | Required executable evidence | Disposition |
|---|---|---|---|---|
| founder disposition of draft | D-C-00 | OC-0.5 | approval metadata and frozen-diff check | APPROVED |
| separate crate/workspace | D-C-01 | OC-01 setup | dependency-direction and forbidden-core-dependency test | APPROVED |
| envelopes/domains/errors/bounds | D-C-02 | OC-01..OC-04 | canonical, cross-domain, tamper, and exact boundary vectors | APPROVED |
| public source-reference bridge | D-C-03 | OC-04 bridge | external-crate compile plus metadata-equality test | APPROVED |
| richer result and B execution binding | D-C-04 | OC-04 | influence/execution mismatch, stale-state, B3-added-critical, and no-partial-output tests | APPROVED |
| TF=0 fusion policy | D-C-05 | P1 prereg + OC-04 | strict TF=0 candidate-entry and frozen-config tests | APPROVED |
| M3/M4 shortlist rule | D-C-06 | OC-02 | corrected E1 shortlist recall/precision/cost gate | APPROVED |
| M2 contract | D-C-07 | OC-02 | explicit citation/linkage positive and forged-link negative vectors | APPROVED |
| terminal/cost/entity semantics | D-C-08 | OC-01/OC-03 | unterminated, unavailable-cost, provenance, privacy, and expiry vectors | APPROVED |
| fixed-point scoring | D-C-09 | OC-03/OC-04 | exact PPR/rerank bytes, overflow, quantization, and channel-separation tests | APPROVED |
| acceptance thresholds | D-C-10 | P1 prereg/OC-05 | prereg hash, family bootstrap, strict TF=0, B8, and Thorn false-suppression reports | APPROVED |

Every row is blocking. A document-only assertion cannot substitute for the
listed executable evidence at the owning package gate.

## 6. Execution consequences

1. This record and every D-C disposition are approved by Lunarpulse.
2. The Option C spec is frozen subject to this record; C2/C3/C4/C5 language
   matches D-C-05, D-C-06, D-C-09, and D-C-10.
3. `option-c-priority-and-gate-plan.md` is approved for execution.
4. The next authorized work is the detailed OC-01 implementation spec and test
   matrix, including the P1 preregistration track.
5. C4 and Thorn implementation remain blocked by their recorded evidence gates.

## 7. Change control

New founder approval is required for:

- Option A/B wire, schema, bound, or claim changes;
- reversing the one-way crate dependency;
- an unchecked SourceReference constructor;
- model inference inside deterministic verification;
- re-enabling pure multiplication as the only candidate path;
- full-DAG M3/M4;
- float-valued production artifact scores;
- lowering acceptance thresholds after test-label inspection;
- enabling Thorn without incremental human-gold evidence.

## 8. Rejected interpretations

- “The founder said start P0” means this record is automatically approved:
  rejected. It authorizes preparation and review, not silent disposition of the
  founder-owned choices.
- “The real replay proves C4”: rejected; labels are silver and no strict TF=0
  stratum exists.
- “The prototype proves the production M3 policy”: rejected; its M3 candidate
  set differs from the draft.
- “All failure records are reusable thorns”: rejected; present Thorn evidence is
  flat/down and lacks state/expiry conditioning.

## 9. Approval record

- 2026-08-20: Founder opened Option C's separate dependency budget and authorized
  unconstrained design exploration while preserving Option A/B integrity.
- 2026-08-21: Founder directed documentation of the final priorities and start
  of P0.
- 2026-08-21: Lunarpulse approved D-C-00 through D-C-10 in full and directed
  spec freeze and P0 commit (Discord message `1540302757649457254`).
- **OC-0.5 disposition:** APPROVED; P1 planning is authorized under this record.
