# OC-04 — Prior-Assisted Selection Integration (P3 / C4)

**Status:** FROZEN v12 — approved 2026-08-29 (commit `99c9ee6`, v11); post-freeze
corrections v12 applied same day per independent Codex contract review (§17),
before any implementation started. Frozen values unchanged except as logged.
**Created:** 2026-08-29 · **Owner:** Engineering (agent) / Founder (lunarpulse_)
**Upstream spec:** `spec-option-c-salience-provenance-layer.md` §C4 (frozen),
`option-c-priority-and-gate-plan.md` §P3 (frozen gate P3-GO).
**Predecessors:** OC-01 ✅, OC-02 ✅, OC-03 ✅ (`6c42523`).
**Preregistration:** `p1-prereg-config.json` `selection_pipeline` and
`evaluation.score_normalization` blocks are consumed VERBATIM in §7 — they are already
frozen and are NOT redefined here.

---

## 1. Purpose and Contract

OC-04 integrates the OC-03 salience prior into Option B's selection path as a
deterministic, recorded, signed, fail-closed second arm. It NEVER replaces or
widens `Selector::select` (B2 stays intact), NEVER bypasses B3–B8 (the
adapter drives the FULL chain through B7 repair and B8 evaluation), and NEVER
enables Thorn (P4 scope, structurally absent).

Caller contract: the adapter consumes a VERIFIED prior — a
`VerifiedPrior` token that only `verify_prior` success can construct (§8) —
plus the scored lexical baseline and candidate pool, and produces a signed
`SelectionExecutionV1` binding the union, rerank, and downstream B3–B8 chain.

## 2. Scope

**In scope:** lexical+prior candidate arms (independently generated, per-arm
capped), deterministic EventId-deduplicated union, preregistered
normalization + rerank (§7.2), TF=0 prior-arm entry with recorded entry
reason, `SelectionInfluenceV1` ordered influence, SIGNED
`SelectionExecutionV1` binding pre-closure IDs/budget/projection/fingerprints
through the B3–B8 chain, human-gold metric gate harness (P3-GO), adversarial
vectors.

**Out of scope:** Thorn (P4); `Selector::select` replacement or semantic
widening; root→salience dependency-direction change (founder-controlled);
float arithmetic; network; new dependencies.

## 3. Stage Plan

| Stage | Content | Gate |
|---|---|---|
| 4A | Workspace gate (exact commands): (1) `git rev-parse HEAD` = the latest OC-04 spec commit on `OC-AttentionLedger` at implementation start (`edc5a76` v12, superseded by spec-only wording commits — each is a valid 4A baseline); (2) `git status --porcelain` is EMPTY (freeze drafts are committed — implementation starts from a clean tree); (3) `grep -rn "oc04" src/ contextmesh-salience/src/` exits 1 (zero new-code refs; spelling `oc04` is the identifier spelling — the pre-existing `OC-04` doc comment at `attribution.rs:776` is a known non-identifier occurrence and is out of scope); (4) `git diff --exit-code HEAD -- Cargo.toml Cargo.lock contextmesh-salience/Cargo.toml` (no dep drift) | invariant check |
| 4B | Schema: `Oc04ConfigV1`, `SelectionInfluenceV1`, `SelectionExecutionV1` (body+envelope), signed issuance/verification, **`VerifiedPrior` (implemented + tested HERE — 4C consumes it; 4B gate = S01–S06, S08–S10)** | S-rows GREEN + dual review (S07b deferred to 4E — see note) |
| 4C | Scored baseline carrier (additive root API, `select` untouched) + prior-arm candidate generation + entity→event reconstruction | U-rows GREEN + dual review |
| 4D | Union + preregistered normalization/rerank + influence record | R-rows GREEN + dual review |
| 4E | Execution binding: B3→B4→B5→B6→B7→B8 chain, signature, verification (+ S07b `canonical_extra_member_rejected` — moved from 4B: it needs `verify_execution`) | E-rows GREEN **except E07** (E07 delivered at 4F) + S07b + dual review |
| 4F | Adversarial boundary vectors + **E07 compile-fail gate**: `contextmesh-salience/tests/compile/oc04_token_privacy.rs` is NOT auto-discovered by Cargo (nested dir) and trybuild is prohibited — the runner is a **zero-dependency harness test** in `tests/oc04_adversarial.rs` that writes the privacy-violating snippet to a temp file under `env::temp_dir()` and invokes `rustc --edition 2021 --crate-type lib` via `std::process::Command` from the `RUSTC` env var (falling back to `rustc` on PATH), asserting exit failure + expected E0xxx in stderr | X-rows GREEN + E07 harness GREEN + dual review |
| 4G | Human-gold gate P3-GO (§14): harness = `contextmesh-salience/tests/oc04_gold.rs` (nDCG@12 evaluator, synthetic-label path marked NOT-real-data); evidence = `_bmad-output/verification-artifacts/oc-04-evidence.md` (4-layer doc) | P3-GO stays OPEN until real gold |

## 4. Definitions

- **Lexical arm**: `BaselineSelector` scored output, capped at the prereg
  `lexical_arm_cap = 64` best-first.
- **Prior arm**: candidates whose reconstructed entity keys (§7.1) carry a
  positive prior vector entry, capped at `prior_arm_cap = 30` by prior ppb
  descending, canonical-EventId-text ascending tie-break.
- **Union**: EventId-deduplicated merge of both capped arms; a duplicate
  keeps one entry with entry-reason `both`.
- **Entry reason (recorded per influence entry)**: `lexical` | `prior` |
  `both`.
- **Fail-closed**: any overflow, out-of-range value, budget violation, or
  recorded-vs-actual mismatch → Err; no handoff, no execution artifact.

## 5. Constants — prereg verbatim + NEW OC-04 freezes (founder approval at freeze)

**Consumed VERBATIM from P1 prereg (no redefinition):**

| Constant | Value | Source |
|---|---|---|
| normalization | per-arm min-max to `[0, 1_000_000]` ppm, clip above/below | `evaluation.score_normalization` |
| rerank formula | `score_ppm = lexical_ppm + prior_ppm` | `selection_pipeline.rerank_formula` |
| tie-break | canonical EventId ascending | `selection_pipeline.tie_break` |
| `LEXICAL_ARM_CAP` | 64 | `per_arm_caps.lexical_arm_cap` |
| `PRIOR_ARM_CAP` | 30 | `per_arm_caps.prior_arm_cap` |
| overflow policy | checked u128; fail closed on any overflow or out-of-range final | `selection_pipeline.overflow_policy` |

**NEW OC-04 values (no prereg conflict — prereg is silent on each; frozen
only with founder approval at freeze):**

| New value | Purpose |
|---|---|
| ID prefixes `oc04inf1_` / `oc04exec1_` | OC prefix discipline |
| signature domain `oc-04-exec-v1\0` | domain-separated issuance |
| derivation domains `oc-04-b3policy-v1\0` `oc-04-b3cand-v1\0` `oc-04-preclosure-v1\0` `oc-04-closed-v1\0` `oc-04-delta-v1\0` `oc-04-handoff-v1\0` `oc-04-b6warn-v1\0` | fingerprint domains (all seven; hyphenated + versioned + NUL-terminated per OC-02/OC-03 domain precedent) |
| `ORPHAN_PRIOR_ENTITIES_MAX = 1024` | orphan counter bound (u32, fail-closed Err on exceed) |

No `PRIOR_WEIGHT_PPM`, no `MAX_COMBINED` — v1's invented constants remain
removed (prereg violations; §17).

## 6. Wire Members (JCS lexicographic at render)

**`SelectionInfluenceV1` body** (6 members):
`config_hash, entries, influence_id, prior_id, task_fingerprint, version` —
`task_fingerprint` is a STRING copied verbatim from the OC-02 report's
`task_fingerprint` member (same-name, same-derivation — OC-02 owns the
definition); `version` is decimal JSON integer `1`. Each entry:
`entry_reason, event_id, lexical_ppm, prior_ppm, score_ppm` —
`entry_reason` is an exact string enum, one of `"lexical"`, `"prior"`,
`"both"`; the entries array is ordered by rerank order (score_ppm
descending, then EventId canonical-text ascending — same key as §7.2).
(normalized ppm values; a prior-arm-only entry has `lexical_ppm = 0` and
vice versa).

**`SelectionExecutionV1` body** (19 members, JCS lexicographic):
`b3_candidate_fingerprint, b3_policy_fingerprint, b6_warnings_hash,
budget_max_bytes, budget_max_events, closed_count, closed_hash, config_hash,
critical_projection, delta_count, delta_hash, execution_id, handoff_hash,
influence_id, pre_closure_count, pre_closure_ids_hash, prior_id,
recipient_head, version` — `delta_count` and `recipient_head` are
P3-required additions over v2; `b6_warnings_hash` is the P3-required
propagated-B6-warnings binding.

**Complete derivation table (every member, byte-for-byte, fail-closed):**

| Member | Derivation |
|---|---|
| `version` | decimal JSON integer `1` (OC-02/OC-03 precedent — NOT a string) |
| `config_hash` | existing `Oc04ConfigV1::config_hash`: lowercase-hex BLAKE3 over `oc-04-config-v1\0` + canonical config bytes (schema frozen at 4B; u64 fields only, no floats) |
| `prior_id` / `influence_id` | copied from the verified token / §9 derivation |
| `execution_id` | §9 (`oc04exec1_` placeholder discipline) |
| `pre_closure_count` | `u64` = len(reranked pre-closure set) |
| `pre_closure_ids_hash` | hex(BLAKE3(`oc-04-preclosure-v1\0` + EventIds canonical-text-ascending, comma-joined UTF-8)) |
| `b3_policy_fingerprint` | hex(BLAKE3(`oc-04-b3policy-v1\0` + `CriticalPolicy::kinds()` joined by NUL, in the canonical order the accessor already returns)) — `CriticalPolicy` has no wire serializer; the kinds-accessor derivation IS the canonical policy bytes |
| `b3_candidate_fingerprint` | hex(BLAKE3(`oc-04-b3cand-v1\0` + B3's sorted-deduplicated candidate EventIds, canonical-text-ascending, comma-joined)) — B3 itself sorts+dedups candidate inputs (`close_selection` entry), so this is B3's own canonical order, not a separate one |
| `closed_count` / `closed_hash` | B3 output: count = len (**u64 decimal**); hash = hex(BLAKE3(`oc-04-closed-v1\0` + closed EventIds text-ascending comma-joined)) |
| `delta_count` / `delta_hash` | B4 output: count = len (**u64 decimal**); hash = hex(BLAKE3(`oc-04-delta-v1\0` + B4 delta wire bytes)) |
| `recipient_head` | B5-verified recipient head EventId canonical text, or JSON `null` when absent |
| `handoff_hash` | hex(BLAKE3(`oc-04-handoff-v1\0` + FINAL post-B7 handoff wire bytes)) |
| `b6_warnings_hash` | hex(BLAKE3(`oc-04-b6warn-v1\0` + the handoff's uncertainty-marker list as exposed by `Handoff::uncertainty()` — already canonically sorted+deduplicated, each NUL-terminated, in that exposed order)) |
| `budget_max_events` / `budget_max_bytes` | `u64` copies of the caller's `SelectionBudget` fields |
| `critical_projection` | string: `"critproj1:" + comma-joined EventIds canonical-text-ascending` (the versioned deterministic critical-candidate projection derived from recorded input refs) |

All hashes lowercase hex ASCII. For LIST-CONCATENATION hashes only
(`pre_closure_ids_hash`, `b3_candidate_fingerprint`, `closed_hash`,
`b6_warnings_hash`): an empty list hashes over the bare domain bytes (empty
joined string). `delta_hash` and `handoff_hash` always hash their (nonempty
structural) wire bytes regardless of event-list emptiness.

**Signed envelope**: `{ body, signer, signature }` where `signature` =
Ed25519 (existing `SigningIdentity`) over domain `oc-04-exec-v1\0` +
canonical(body). Verification recomputes over the body — never trusts the
recorded signature.

## 7. Core Semantics

### 7.1 Candidate generation and entity→event reconstruction

`SaliencePriorV1` records entity names + ppb, NOT EventIds. Reconstruction
is therefore NORMATIVE (not a gap): for every candidate `SourceEvent`, the
adapter calls the public `derive_entity_keys(source.text())` — the CANONICAL
payload text (`SourceEvent::text()`, `selection.rs:186`, already
canonicalized via `canonical_payload_bytes`) — and records the match against
the prior's positive vector entries.

**Canonicalization gate (NORMATIVE, not structural):** `VerifiedPrior::verify`
REJECTS any sessions/events payload string that is not itself canonical
payload text (checked via `canonical_payload_bytes` round-trip equality
before OC-03 rebuild). Since the prior can then only have been built from
canonical texts and OC-04 reconstruction also uses `SourceEvent::text()`
canonical text, raw-vs-canonical divergence is excluded BY CONSTRUCTION OF
THE GATE, not by type. Matrix coverage: **X09
`noncanonical_prior_payload_rejected`** (see X-rows).

An event's raw prior value = **max** ppb over its matching entities
(bounded [0, 1e9]; sum rejected as unbounded-scale). Events with no positive
match are not prior-arm candidates. Orphan prior entities (positive vector
entry with no matching candidate event) increment
`UnionOutcomeV1::orphan_prior_entities` (u32; **fail-closed Err on exceeding
`ORPHAN_PRIOR_ENTITIES_MAX = 1024`** — §5 new freeze). Reconstruction
iterates the candidate pool in its canonical order; HashMap iteration is
prohibited.

### 7.2 Normalization and rerank (P1 verbatim)

Each arm is normalized independently by min-max to [0, 1e6] ppm over that
arm's candidates: `ppm = (raw − min) × 1_000_000 / (max − min)`, clipped,
checked u128. Degenerate single-value arm (min = max): every member maps to
`1_000_000` if raw > 0, else `0`. `score_ppm = lexical_ppm + prior_ppm`
(≤ 2e6, u128-safe trivially). Rank by score desc, then canonical EventId
text ascending (= existing `EventId::Ord`, NOT raw-byte order — these differ
and the canonical-text order is frozen). TF=0 events enter via the prior arm
with `lexical_ppm = 0`.

### 7.3 Execution binding (B3–B8, no bypass)

The adapter drives the exact chain: B3 `close_selection` over the reranked
pre-closure set → B4 delta → B5 stale-state handoff verification →
B6 uncertainty via `Handoff::with_uncertainty` per the **normative warning
inputs rule below** → B7 `run_repair` (`src/repair.rs:320`, async,
store-driven; convergence or explicit repair-terminal outcome required —
non-convergence is a recorded failure, not a silent pass) → B8 `simulate`
(`src/eval.rs:428`) evaluation over the critical projection with required
passing expectations. The signed execution artifact is issued ONLY after B7
convergence AND B8 pass; `handoff_hash` covers the final post-B7 handoff.
A state change or mismatch returns neither deliverable handoff nor artifact.

**Normative B6 warning inputs rule (deterministic, recorded):** OC-04 adds
uncertainty markers from exactly two deterministic sources, in this order,
each via `with_uncertainty`: (1) `prior_arm_used=true` (when the prior arm
contributed ≥1 union candidate), else `prior_arm_empty`; (2)
`orphan_prior_entities=<n>` (the U04 counter, decimal string). No other
marker may be added by OC-04. `Handoff::from_delta` starts empty and B6's
own sort+dedup applies, so the list is exactly the two-source set above;
`b6_warnings_hash` (§6) hashes the `uncertainty()` exposure.

**B7 driver (caller-supplied, spec-visible):** `run_repair` takes
`driver: D where D: FnMut(&Handoff) -> Fut<Output=TaskOutcome>` — the task
driver is a CALLER-supplied input, not derived. `ExecutionChainInputs`
therefore carries it (see §8), and 4E tests inject deterministic drivers.
`run_repair` also requires `recipient: &RecipientState` (carried in
`ExecutionChainInputs`).

### 7.4 Budget

`SelectionBudget` carries BOTH `max_selected_events` and
`max_exported_bytes`. The union is the candidate set (≤ 94 by caps);
selection admission after rerank respects both fields; the Option B compiler
fail-closes (refuses) on excess — OC-04 mirrors this: over-budget is a
deterministic refusal with a recorded reason, never silent truncation. Both
budget values are bound into the execution body.

### 7.5 Stale/mismatch rejection

Prior artifact whose `prior_id` fails re-verification against the caller's
chain inputs (the `VerifiedPrior` token is constructed only by `verify_prior`
Ok) cannot enter. Influence record whose entries ≠ actual union/rerank →
Err. Signature mismatch → Err.

## 8. Public API (D — finalized at 4B against live types)

```rust
// additive, root crate — does NOT touch Selector::select
pub struct ScoredSelection { /* private fields */ }
impl ScoredSelection {
    pub const fn reference(&self) -> &SourceReference;
    pub const fn lexical_raw(&self) -> u128;   // raw TF, checked u128 accumulate
    pub const fn lexical_rank(&self) -> usize; // best-first rank in the arm
}
impl BaselineSelector {
    pub fn select_scored(&self, task: &TaskRecordV1, sources: &[SourceEvent])
        -> Result<Vec<ScoredSelection>, SelectionError>; // same semantics as select()
}

// salience crate (depends on root — existing direction, no cycle)
pub struct VerifiedPrior { prior: SaliencePriorV1 }
impl VerifiedPrior {
    /// The ONLY constructor: runs the full rebuild-based verify_prior
    /// (OC-03 §8) internally and returns the token on Ok only.
    /// Every sessions/events payload string MUST be canonical payload text
    /// (via canonical_payload_bytes) — non-canonical input → Err (see §7.1
    /// canonicalization gate).
    pub fn verify(bytes: &[u8], sessions: &[SessionPayloads<'_>],
        reports: &[ReportContribution], events: &[(&str, &str)],
        config: &PriorConfigV1) -> Result<Self, OutcomeError>;
    pub fn prior_id(&self) -> &str;
    /// Positive vector entries, read-only, entity-name-ascending — returns
    /// the OC-03 PriorSeedV1 view (entity + ppb) without conversion.
    pub fn positive_seeds(&self) -> &[PriorSeedV1];
}
pub fn union_candidates(lexical: &[ScoredSelection], prior: &VerifiedPrior,
    sources: &[SourceEvent], config: &Oc04ConfigV1) -> Result<UnionOutcomeV1, OutcomeError>;
pub fn rerank(union: &UnionOutcomeV1, config: &Oc04ConfigV1)
    -> Result<SelectionInfluenceV1, OutcomeError>;
pub async fn bind_execution<'a, F, D, Fut>(
    influence: &SelectionInfluenceV1,
    chain: &mut ExecutionChainInputs<'a, F, D, Fut>,
    signer: &SigningIdentity, config: &Oc04ConfigV1)
    -> Result<(SignedExecutionV1, Handoff), HandoffError>
where
    F: Fn() -> D,
    D: FnMut(&Handoff) -> Fut,
    Fut: Future<Output = TaskOutcome>;
pub async fn verify_execution<'a, F, D, Fut>(
    env: &SignedExecutionV1,
    chain: &mut ExecutionChainInputs<'a, F, D, Fut>,
    config: &Oc04ConfigV1)
    -> Result<(), HandoffError>
where
    F: Fn() -> D,
    D: FnMut(&Handoff) -> Fut,
    Fut: Future<Output = TaskOutcome>;

/// Every input the B3–B8 chain needs that is NOT derivable from the
/// influence record (pinned at 4B against live signatures):
pub struct ExecutionChainInputs<'a, F, D, Fut>
where
    F: Fn() -> D,
    D: FnMut(&Handoff) -> Fut,
    Fut: Future<Output = TaskOutcome>,
{
    pub context: &'a ContextId,
    pub store: &'a Store,                    // concrete live store type
    pub b3_candidates: &'a [EventId],
    pub b3_policy: &'a CriticalPolicy,
    pub b3_limits: &'a ClosureLimits,
    pub budget: &'a SelectionBudget,
    pub recipient: &'a RecipientState,       // B7 run_repair input
    pub repair_bounds: &'a RepairBounds,     // B7 driver bound
    /// B7 driver FACTORY: bind and verify each call it once to obtain a
    /// FRESH stateful driver — verification replay never reuses consumed
    /// FnMut state.
    pub repair_driver_factory: F,
    pub repair_history: &'a mut RepairHistory, // B7 mutable state (bind)
    /// Scratch-history location for verify_execution's B7 replay: verify
    /// opens a FRESH RepairHistory here via `RepairHistory::open` inside an
    /// OC-04-provided RAII guard (`ScratchHistoryGuard`) that DELETES the
    /// file on drop — RepairHistory itself has no drop-cleanup, so OC-04
    /// owns the guard. FAIL-CLOSED RESERVATION: the guard FIRST rejects
    /// `scratch_history_path == repair_history.path()` (same-path) and any
    /// already-existing file, then atomically reserves the path
    /// (`File::create_new`) before `RepairHistory::open` — a mistaken
    /// caller path can never append to and then delete production history.
    /// Never the production append-only history path.
    pub scratch_history_path: &'a std::path::Path,
    pub critical_ids: &'a [EventId],         // B8 projection input
}
```

`bind_execution` drives the full B3–B8 chain internally (§7.3) using
`influence` + `chain` (taken `&mut` because `repair_history` is mutable B7
state) and returns `(SignedExecutionV1, Handoff)` — **the deliverable
post-B7 handoff is RETURNED, never discarded** (the envelope records only
its hash; the handoff itself is the delivery artifact). `verify_execution`
recomputes the chain against a FRESH scratch `RepairHistory` opened at
`chain.scratch_history_path` (inside `ScratchHistoryGuard`, RAII-deleted;
never the production history) and a FRESH driver from
`repair_driver_factory` — replay proof = recomputed final handoff
byte-equality with the recorded `handoff_hash`. Exact trait/type names
pinned at 4B against live signatures (concrete `Store` — no `dyn EventStore`
exists; `RecipientState`; `RepairBounds`/`RepairHistory::open`;
generic driver factory); the struct fields listed here are the parameter
SURFACE, and 4B will fail if any cannot be satisfied.

## 9. ID Derivation

`influence_id` = BLAKE3(`oc-04-inf-v1-id\0` + canonical body with the id member =
literal `"influence_id"`), prefix `oc04inf1_` + **base64url digest, no padding —
exactly as OC-03 `ocprior1_` renders**. `execution_id` likewise with
`oc-04-exec-v1-id\0` / `oc04exec1_` + base64url. Placeholder discipline per
OC-02/OC-03 (ID computed LAST over placeholder-substituted canonical bytes,
before signing).

## 10. File Map (D)

`src/selection.rs` (+`select_scored`, additive only); `contextmesh-salience/src/oc04_selection.rs`;
`contextmesh-salience/tests/oc04_{schema,union,rerank,exec,adversarial,gold}.rs`;
`contextmesh-salience/tests/compile/oc04_token_privacy.rs` (E07 compile-fail snippet source — driven by the zero-dependency rustc harness in oc04_adversarial.rs, §3 4F);
`_bmad-output/verification-artifacts/oc-04-evidence.md` (4G).

## 11. Commands

Standard: `OC01_INNER_CURRENT_GATE=1 CARGO_NET_OFFLINE=true cargo test
--workspace --locked`; clippy `-D warnings`; fmt; `git diff --check`.

## 12. Test Matrix Summary

57 rows: S12 + U8 + R9 + E14 + X14 (full matrix separate artifact; splits:
E06→E06/E06b per-field budget, S07→S07/S07b parse vs rejection, R03→R03/
R03b order vs completeness, E07 compile-fail gate, E04b B6-warnings hash,
E04c normative marker rule, X09 canonicalization gate, X10 orphan bound, X11/X11b
verifier-replay split, X12/X12b scratch-guard split, S03b prereg-path split,
E09/E10/E11 failure-surface rows — v12 Codex additions).

Row atomicity convention (v12, per Codex review): S04/U08/R05/E04c/E06/E06b
are ACCEPTED as-is — each describes ONE rule whose evidence enumerates the
rule's exhaustive case set (a rule with enumerated cases is one assertion
about the rule, not multiple assertions). X11/X12 were split because they
combined INDEPENDENT assertions; S03 was split because the prereg has two
distinct authority blocks.

## 13. Gates

Every stage: focused GREEN → full regression 0 failed → clippy 0 → fmt →
dual review → founder 승인 → commit/push/graphify. No-new-deps gate (X06)
is the matrix's mechanism: a unit test in `tests/oc04_adversarial.rs` that
checks the Cargo.toml/lock diff is empty vs committed (include_str! hash
pin). OB regression gate: workspace-run `cargo test --workspace` captured
to a log (file-redirection pattern), EXIT 0 required.

## 14. Human-Gold Metric Gate (P3-GO)

Preregistered primary nDCG@12 + strict TF=0 recovery. If the gold corpus is
unavailable at 4G, the harness ships with synthetic labels marked
NOT-real-data and the gate stays OPEN (cite: OC-02 evidence-stage
synthesized-fixture discipline, `oc-02` evaluation harness artifacts —
exact section pinned at 4G).

## 15. Change Control

Wording-only (안 A) with code+test unchanged, logged in §17 with Discord
approval ID. Frozen values never change without founder renegotiation.
Root-crate additive API additions are reported to the founder at the 4B
gate; any dependency-DIRECTION change requires explicit founder approval.

## 16. Risks

- `select_scored` touches root crate → additive-only, X04 proves baseline
  invariance.
- Multi-entity max-fold is a new freeze (no prereg conflict — prereg is
  silent on aggregation) — flagged for founder at freeze.
- EventId canonical-text ordering differs from raw-byte — test pair where
  they diverge (R02).

## 17. Change Log

- 2026-08-29: v1 draft created.
- 2026-08-29: **v2 rewrite after dual review (deleg_c26c9fa7): Compliance
  NO-GO (7 blockers), Quality REQUEST_CHANGES (7 blockers).** Discovered
  the P1 prereg already freezes `selection_pipeline` (normalization,
  `score_ppm = lexical_ppm + prior_ppm`, per-arm caps 64/30, EventId
  tie-break, checked-u128 policy) — v1's invented `PRIOR_WEIGHT_PPM` /
  `MAX_COMBINED` REMOVED as prereg violations. Added: signed execution
  envelope (P3-mandated), full B3-B8 chain incl. B7/B8, entity-to-event
  NORMATIVE reconstruction via `derive_entity_keys` with max-fold,
  `ScoredSelection` additive carrier (lexical scores were discarded),
  canonical-EventId-TEXT ordering, dual-field budget fail-closed,
  entry-reason recording, matrix count reconciled 40-42. Crate boundary
  confirmed sound (salience->root already exists).
- 2026-08-29: **v3 corrections after re-verification (deleg_2549c7e5):
  NO-GO 6 blockers.** Added `delta_count` + `recipient_head` (18-member
  body, lexicographic budget members); partial derivation table; B7
  `run_repair`/B8 `simulate` pinned with convergence+pass requirements;
  concrete `ScoredSelection` accessors + `VerifiedPrior::verify` wrapper;
  reconstruction bound to `SourceEvent::text()`; single-assertion matrix
  rows; count 42-43.
- 2026-08-29: **v4 corrections after second re-verification
  (deleg_11bf5e91): NO-GO 5 blockers.** Complete 19-row member-by-member
  derivation table (was partial); P3-required `b6_warnings_hash` (19th
  member + E04b row); `VerifiedPrior::positive_seeds` returns
  `&[PriorSeedV1]` (unimplementable tuple borrow fixed);
  `bind_execution`/`verify_execution` parameter surface spelled out;
  §7.1 canonicalization gate NORMATIVE (round-trip at verify — overclaim
  removed); `ORPHAN_PRIOR_ENTITIES_MAX` promoted to §5 declared new
  freeze; §5 split prereg-verbatim vs new-value tables; S07→S07/S07b,
  R03→R03/R03b; headers v4; matrix 43-46 programmatically verified.
- 2026-08-29: **v5 corrections after third re-verification
  (deleg_174191ca): NO-GO 4 blockers.** Derivations re-anchored to live
  APIs: `b3_policy_fingerprint` over `CriticalPolicy::kinds()` (no wire
  serializer exists — accessor derivation IS canonical);
  `b3_candidate_fingerprint` over B3's own sorted-deduped input order;
  `b6_warnings_hash` over `Handoff::uncertainty()` exposed list (sorted
  +deduped by B6 itself); empty-list rule scoped to list-concatenation
  hashes only. API reworked: `ExecutionChainInputs` struct carries every
  non-derivable B3-B8 input (context/store/b3 inputs/recipient/
  RepairBounds/RepairHistory/critical IDs), concrete `Store` (no `dyn
  EventStore` exists), `verify_execution` async. Canonicalization-gate
  coverage moved to dedicated X09; orphan bound to X10; U04 single
  assertion. §5 domain list completed (all 7). Matrix 46-48
  (S11+U8+R9+E10+X10).
- 2026-08-29: **v6 corrections after fourth re-verification
  (deleg_a7ce1529): NO-GO 3 blockers.** B7 driver made spec-visible:
  `run_repair` takes caller-supplied `D: FnMut(&Handoff) -> Fut<
  Output=TaskOutcome>` + `recipient: &RecipientState` — both carried in
  `ExecutionChainInputs` (now generic over D/Fut); `bind/verify_execution`
  take `&mut chain`; `verify_execution` recomputes B7 against a FRESH
  scratch RepairHistory with deterministic driver replay (never the
  production append-only history). **Normative B6 warning-inputs rule**
  added (exactly two deterministic markers: `prior_arm_used/empty`,
  `orphan_prior_entities=<n>` — closes the empty-uncertainty gap).
  Warnings: `close_over`→`close_selection` correction; E04b wording;
  changelog U04→X10 ownership.
- 2026-08-29: **v7 corrections after fifth re-verification
  (deleg_f66b24d4): NO-GO 2 blockers (reviewer rustc-proved §8 invalid:
  E0562 nested impl Trait, E0121 bare '_ generics).** §8 rewritten with
  explicit `<D, Fut>` generics + where clauses; `repair_driver` →
  `repair_driver_factory: F where F: Fn() -> D` — bind and verify each
  mint a FRESH stateful driver (replay never reuses consumed FnMut
  state); verify's scratch RepairHistory path caller-supplied + RAII
  cleaned; replay proof = recomputed handoff byte-equality vs recorded
  `handoff_hash`. Parent rustc-probed the new signature shape (0 errors).
  E04c added (normative exact-two-marker rule, both alternatives); count
  49-50 (X11 verifier_replay_integrity); headers v7.
- 2026-08-29: **v8 corrections after sixth re-verification
  (deleg_23bb5546): NO-GO 3 blockers.** §8 rewritten to FULLY explicit
  `<F, D, Fut>` generics on both functions (no `impl Trait` in argument
  position, no bare `'_`); `scratch_history_path: &Path` field added to
  `ExecutionChainInputs` with OC-04-owned `ScratchHistoryGuard` RAII
  deletion (`RepairHistory::open` has no drop-cleanup — guard is ours);
  `bind_execution` now returns `(SignedExecutionV1, Handoff)` — the
  deliverable post-B7 handoff is returned, never discarded.
- 2026-08-29: **v9 corrections after seventh re-verification
  (deleg_86fa88a6): NO-GO 1 blocker (E0261: 'a undeclared).** Both §8
  functions now `<'a, F, D, Fut>`. Warning adopted: ScratchHistoryGuard
  made fail-closed + atomic — rejects same-path (== repair_history.path())
  and pre-existing files, reserves via `File::create_new` before
  `RepairHistory::open`; X12 `scratch_guard_fail_closed` added; count
  50-51; headers v9.
- 2026-08-29: **EIGHTH RE-VERIFICATION: GO (deleg_93ecf2ca) — 0 blockers,
  1 editorial warning ("FAIR-CLOSED"→"FAIL-CLOSED", fixed same day).
  Spec dual-review + re-verification round CLOSED at v9.**
- 2026-08-29: **v10 corrections after preflight angle 3 (deleg_0a6589a5):
  FAIL — 7 wire findings, all fixed.** (1) `version` re-specified as decimal
  JSON integer `1` (OC-02/03 precedent; was string `"1"`). (2)
  `task_fingerprint` defined: STRING copied verbatim from OC-02 report's
  member (OC-02 owns the derivation). (3) `entry_reason` exact string enum
  `"lexical"|"prior"|"both"` + entries array order normative (rerank key).
  (4) `config_hash` domain frozen: lowercase-hex BLAKE3 over
  `oc-04-config-v1\0` + canonical config bytes; Oc04ConfigV1 schema (u64
  only) pinned at 4B. (5) ID digest encoding fixed: base64url no padding
  (ocprior1_ precedent), ID domains `oc-04-inf-v1-id\0` /
  `oc-04-exec-v1-id\0`. (6) `closed_count`/`delta_count` explicit u64
  decimal. (7) All 7 derivation domains renamed to precedent shape
  `oc-04-*-v1\0`.
- 2026-08-29: **Preflight angle 5 (deleg_b06f4e1b): NOT FREEZE-READY, 3
  findings, all fixed same day:** (1) model-name metadata removed from
  §17; (2) changelog reordered to complete v1→v9 with explicit GO
  record; (3) `contextmesh-salience/tests/compile/oc04_token_privacy.rs`
  added to §10 (E07 compile-fail gate file was unnamed).
- 2026-08-29: **v11 corrections after preflight angle 4 (deleg_f81876b5):
  FAIL — 3 stage-plan defects, all fixed.** (1) S07b moved from 4B to 4E
  (forward dependency: needs `verify_execution`); 4B gate now S01–S06,
  S08–S11. (2) E07 made runnable: zero-dependency rustc harness in
  oc04_adversarial.rs (temp-file snippet + `RUSTC`-env `std::process::
  Command`, assert exit-fail + E0xxx) — nested file is snippet source only,
  trybuild prohibited. (3) 4A gate fully specified: 4 exact commands
  (HEAD=6c42523, porcelain=2 drafts only, `grep -rn "oc04"` exits 1 with
  the known `attribution.rs:776` `OC-04` doc-comment occurrence scoped
  out, dep-manifest diff clean). Bonus: VerifiedPrior ownership pinned to
  4B; 4G harness/evidence artifacts named (oc04_gold.rs, oc-04-evidence.md).
  Re-check fixes (deleg_37afbd4d): 4B gate corrected to S01–S06, S08–S10
  (S11 doesn't exist; S07b is the 11th S-row); 4E gate excludes E07
  explicitly; E07 matrix File column → oc04_adversarial.rs harness with
  tests/compile file demoted to snippet source.
- 2026-08-29: **FROZEN at v11 (commit 99c9ee6, founder 승인).**
- 2026-08-29: **v12 post-freeze corrections after independent Codex
  contract-driven review (ChatGPT subscription; 7 blockers):** (1) status
  headers DRAFT→FROZEN (v11 freeze + v12 corrections recorded; no frozen
  value changed); (2) §17 chronology fixed (v10 now precedes v11);
  (3) 4A gate HEAD updated 6c42523→99c9ee6 + porcelain now EMPTY
  (drafts committed); (4) prereg path corrected to
  evaluation.score_normalization + S03b row added; (5) X11/X12 split into
  atomic rows X11/X11b, X12/X12b; (6) E09/E10/E11 failure-surface rows
  added (B7 non-convergence, B8 failure, checked-u128 overflow);
  (7) X06 spec/mat conflict resolved (unit-test mechanism per matrix).
  Warnings accepted: 4G citation pin deferred to 4G; Oc04ConfigV1 member
  list is pinned at 4B per plan (schema spec-before-code at that stage).
  Matrix 51→57 rows (S12+U8+R9+E14+X14). Implementation had NOT started —
  zero code changes, wording-only eligibility satisfied.
