# OC-04 4G+ — Real Human-Gold Corpus Preparation Plan (v5.3)

**Status:** APPROVED — v5.2 FROZEN (founder approval 2026-09-04, Discord msg 1545401201573765120: "승인") · **Date:** 2026-09-04
**Revision:** v5.2 — dual-model critique→refine: gpt-5.5 (author, 9 self-identified defects) × GLM (critic, all 9 VALID + 6 missed defects + 6 fixes) → consolidated 15-item adoption (14 as-is, 1 modified, 0 rejected). Items marked FOUNDER-GATE require founder approval before freeze; SELF items apply at plan level.
**Purpose:** Define how the real human-gold corpus for the P3-GO gate is constructed, labeled, quality-controlled, and bound into `oc04_gold.rs` — before any labeling starts.

---

## 1. Scope and gate linkage

P3-GO (OC-04 §14) flips OPEN→judged only when a **real human-gold corpus** replaces the synthetic labels shipped at 4G (`oc04_gold.rs`, `GOLD_LABELS_REAL_DATA=false`). This plan defines that corpus. The prereg (`p1-prereg-config.json`, hash `be20d8fc…`) freezes **metric policy only** — it contains no threshold outcomes (prereg record §4 non-claim 2). Where this plan states numeric decision criteria (CI-width rules, κ floors), those are NEW commitments introduced by THIS plan under founder change control, NOT prereg content, and are marked as such.

## 2. Stage 0 — Power/sensitivity analysis (BLOCKER-1 fix; runs BEFORE any labeling)

1. **Blinded simulation:** a read-only Python script (`oc04-gold/power-sim.py`, committed) simulates per-session gold counts drawn from the OC-00.5 empirical distribution {1, 2, 3} gold events and plausible nDCG@12 effect sizes (Δ = 0.05, 0.10, 0.15), running the FROZEN family-cluster bootstrap (95%, 10,000 iters, seed 20260820) to produce CI widths for corpus sizes 48/72/96 sessions.
2. **Stopping rule — clustered-outcome simulation (v5.2, item 1, SELF):** `power-sim.py` MUST simulate clustered session outcomes, not only gold counts: per-session nDCG for lexical and prior arms under DECLARED arm-conditional relevance priors (hash-pinned pre-label — labels do not exist yet, so no claim of observed nDCG simulation), parent-family ICC grid {0.0, 0.1, 0.3}, stratum heterogeneity, and shortlist/candidate composition from a dry run of the frozen extractors. Corpus size is selected only if the **75th-percentile** (not median) simulated CI width for Δ = 0.05 is ≤ 0.025 at that size; sizes 48/72/96 are simulated and if NONE passes, the gate is recorded **UNDERPOWERED** (per D-C-10 §6, inconclusive — no post-hoc gate lowering) with the full simulation artifact committed. Any-hit@12, strict-TF0, and expected-random diagnostics are reported alongside; the gating random baseline uses D-C-10 seed 20260820.
3. **Parent-family effective-N (BLOCKER-2 fix):** the simulation accounts for family clustering: sessions sharing a parent family contribute as ONE cluster (the frozen bootstrap's `cluster_level: parent-family`). The sampling frame must supply ≥12 independent parent families per stratum; the achieved count is reported in the sampling manifest, and the corpus is marked underpowered if fewer.
4. **power-sim.py assumptions are part of the frozen plan (BLOCKER-1 residual fix):** the script's gold-count distribution {1,2,3} (weights recorded from the OC-00.5 empirical frequencies, stated in the script header), effect sizes {0.05, 0.10, 0.15}, and CI computation MUST be committed and hash-pinned in the sampling manifest BEFORE any labeling. The simulation's assumptions cannot be silently revised after labels exist — any revision restarts Stage 0 and invalidates prior size decisions (recorded in the change log).

## 3. Source data and sampling manifest (BLOCKER-10, -11, WARNING-2, -4 fixes)

| Item | Decision |
|---|---|
| Source pool | Hermes session transcripts. **Temporal holdout:** only sessions created AFTER the OC-00.5 replay snapshot date are eligible (cuts historical-tuning leakage from prior development inspection). The exact cutoff is recorded in the sampling manifest. |
| Sampling manifest (frozen before labeling) | committed `oc04-gold/sampling-manifest.json`: source-snapshot SHA-256, selection-script SHA-256, RNG seed, eligibility criteria, per-stratum family-assignment rule, selected session/family IDs (HMAC — §6), 2 alternates per stratum, and exclusion reasons for every skipped session — except `strict_all_gold_tf0`, which has NO alternates (v5.2, item 13). |
| Selection script | committed `oc04-gold/sample.py` — stdlib-only, deterministic given seed + snapshot; emits the manifest. No manual picking. |
| Stratum assignment (BLOCKER-11 fix, round-2 executable form) | `terminal_with_full_cost`, `terminal_with_partial_cost`, `unterminated` are assigned deterministically from pre-label transcript state (terminal status, cost fields). `strict_all_gold_tf0` CANNOT be known pre-label: the oversupply pool size is **pre-declared by formula in the sampling manifest** — `oversupply_N = ceil(target_stratum_N / base_rate_estimate − target_stratum_N)` where `base_rate_estimate` comes from the Stage-0 simulation's strict-stratum yield estimate (the manifest states the estimate and its source). Labeled blind, then assigned by the frozen rule (all gold candidates lexical-TF=0) AFTER labeling, NO opportunistic replacement. **Success criterion:** if assigned yield < target_stratum_N, the stratum is recorded SHORT and the gate is underpowered-for-stratum — disclosed, never swapped.
   **Outcome-conditioning control (v5.2, item 2, FOUNDER-GATE — APPROVED 2026-09-04):** because `strict_all_gold_tf0` is post-label outcome-assigned, the D1/D2 primary aggregate either remains the prereg/founder-fixed judged population, or switches to a design-weighted aggregate ONLY under founder change control. If approved, the sampling manifest hash-pins pre-label strata targets, oversupply rules, and weights BEFORE labeling; D1/D2 then use the design-weighted aggregate, D3 uses only the strict-TF0 subset. No post-hoc weights or strict-oversupply enrichment may enter the primary estimand. |
| Candidate universe (BLOCKER-3 fix, extended per round 2) | a candidate-union manifest is frozen BEFORE labeling: EventId-deduplicated candidates from the TWO frozen candidate arms — lexical and positive-prior (`selection_pipeline.candidate_generation`, per-arm caps 64/30 exactly as frozen). M0/M1/M2 are EXTRACTOR VERSIONS (`evaluation.extractor_versions`) feeding the lexical arm's candidate generation and judging, NOT separate candidate arms; no fourth arm is created. Rank/score/arm-membership are hidden from annotators. Annotators label the union, not a ranked top-12. **Pool-blind relevance gap — renamed and gated honestly (MED-5 fix):** events missed by BOTH frozen arms remain unlabeled, so this corpus measures a **pooled-candidate reranking gate**, NOT full-pool retrieval. All evidence claims carry the qualifier "over the frozen shortlist32 projection of the candidate union". Because IDCG excludes union-missed relevant events, primary nDCG may be inflated relative to full-pool retrieval — this bias is DISCLOSED in every evidence record. The gate name is `P3-GO (pooled-candidate reranking)`. **Evaluation-pool split (v5.2, item 4, FOUNDER-GATE — APPROVED 2026-09-04):** labeling pool = full EventId-deduplicated arm union (audit completeness); PRIMARY evaluation pool = the preregistered `candidate_shortlist_cap: 32` projection EXACTLY as frozen — same construction, dedup order, tie-breaks, and cap timing — for nDCG@12/Any-hit@12 and M3/M4 cap accounting. The manifest records both `candidate_union_index` and `shortlist32_index`; primary metrics exclude labels outside shortlist32; evidence qualifiers read "over the frozen shortlist32 projection of the candidate union"; union-only labels feed coverage diagnostics. A separate mandatory nomination-coverage diagnostic accompanies the gate: `bounded_next12_coverage` (v5.2, item 5) labels, per sampled session, the FIRST 12 eligible transcript events by episode order that are NOT in the candidate union and fall within the same task episode through the terminal answer. It reports `missed_relevant_rate = relevant_next12 / (relevant_union + relevant_next12)`; is NOT the frozen `shortlist_recall` and NOT part of the primary metric. If `missed_relevant_rate > 0.10`, evidence wording MUST say "reranking-only; coverage inadequate for a retrieval claim" — that wording and any broader claim are founder-ratified under change control (ratification authority APPROVED 2026-09-04). A full-pool labeling campaign remains a separate plan amendment. The "@all" naming from v2 is withdrawn as unexecutable. The manifest records candidate eligibility timestamps (episode boundaries). |
   Packet builders produce two physically separate deterministic packet sets (union vs coverage); annotators never see arm membership or packet type (v5.2, item 11). If separation cannot be maintained, `bounded_next12_coverage` is marked non-blind with an explicit waiver and stays non-gating.
| Session definition | one `.jsonl` file = one session; annotator context = the task episode plus the FULL FORWARD transcript through the terminal answer (BLOCKER NEW-3 fix — `dead_end` rejection evidence may occur after the ±1 window), and 1 preceding episode. Rank/arm blinding is preserved regardless of window size. The window rule is fixed in the sampling manifest. |

## 4. Label scheme (BLOCKER-5, -6, WARNING-1 fixes)

The five frozen labels get operational definitions via a **decision tree** (committed as `oc04-gold/codebook.md` with ≥3 worked examples + counterexamples per label):

| Label | Operational rule |
|---|---|
| `required` | Removing the event's content makes the session's CORRECT final answer materially wrong or impossible — judged against the user task's correct outcome, NOT against what the model happened to use. |
| `supporting` | Content contributed to the correct answer, but ≥1 other labeled event could substitute without material loss. |
| `irrelevant` | No bearing on the task's correct outcome. Default when the decision tree reaches no other label. |
| `dead_end` | The transcript shows EXPLICIT pursuit then rejection, or the content affirmatively misled a step that had to be redone. Requires observable evidence in the transcript; mere irrelevance is `irrelevant`. |
| `uncertain` | Intermediate/audit label only. `uncertain` requires one structured reason code (see adjudication section) . |

**Precedence:** `dead_end` (if explicit evidence) > `required` > `supporting` > `irrelevant`.

**Binary mapping for the frozen primary metric:** `required`+`supporting` → relevant; `irrelevant`+`dead_end` → non-relevant (no graded penalty — that would be a metric change under §15 change control).

**`uncertain` resolution (BLOCKER-6 fix):** the primary gold set MUST NOT contain unresolved `uncertain`. Every `uncertain` is adjudicated to one of the four decisive labels before scoring; the audit trail (original `uncertain` + reason + resolution) ships with the corpus. `uncertain` rate > 15% of judgments for any annotator triggers a codebook revision before proceeding.

## 5. Labeling protocol and reliability (BLOCKER-7, -8, WARNING-3 fixes)

1. **Corpus size:** set by the Stage-0 stopping rule (§2.2), minimum 48 sessions × candidate-union.
2. **Annotators:** TWO independent human annotators, blind to each other, blind to pipeline output (no rank/score/arm visibility), NO LLM pre-labels for the reliability subset. **Every label that enters the gold corpus — reliability subset AND remainder — is produced or confirmed by a human under the same blindness rules; LLM output is never a label source (BLOCKER-7 round-2 fix).** Attrition rule (v5.2, item 15): any session/candidate with fewer than two completed independent human annotations is EXCLUDED from gold scoring and cannot be gold-ified by adjudication alone; CI fails closed if any scored label lacks two pre-adjudication human labels (plus the adjudication record where they disagree). An LLM may ONLY pre-sort/cluster candidate packets for annotator convenience (e.g. grouping similar events), and its output is never shown as a suggested label. If LLM assistance beyond packet-ordering is ever wanted, it requires a founder-approved plan amendment and a fresh reliability measurement on the affected subset.
2b. **Pilot stage (v5.2, item 14):** production labeling is preceded by a small blinded pilot; κ/`uncertain` review; codebook revision if needed; production starts only after codebook freeze. If production `uncertain` rate exceeds 15% for any annotator, the codebook is revised and the ENTIRE affected corpus is re-labeled under the revised codebook (pre-revision labels archived, never merged).
3. **Reliability (v5.2, item 6):** all statistics computed on independent raw annotator labels BEFORE any adjudication: κ(binary), κ(5-label), PLUS per-label positive agreement for `required`, `supporting`, `dead_end`, and κ stratified by pre-label stratum (prevents irrelevant-prevalence inflation). Gate proceeds only if κ(binary) ≥ 0.6 AND κ(5-label) ≥ 0.5 AND positive agreement ≥ 0.60 for each decisive non-irrelevant label with ≥20 occurrences; sparse decisive labels require adjudication audit sampling with disclosure. Failure triggers codebook revision AND **re-labeling of the ENTIRE corpus under the revised codebook by both annotators (BLOCKER NEW-2 fix — no mixing of codebook regimes; the pre-revision labels are archived, never merged)**; the second failure declares the codebook unfit and halts the gate.
4. **Adjudication (v5.2, item 10):** all non-`uncertain` disagreements are resolved by a written rule applied by a THIRD party (or the founder under blindness — adjudicator sees NO arm, rank, score, system identity, or outcome summary). Post-consensus labels are used only for scoring, NEVER for κ. Each adjudication records the rule clause applied. `uncertain` requires one STRUCTURED reason code (v5.2, item 7): `ambiguous_task_goal`, `insufficient_context`, `conflicting_evidence`, `redaction_blocks_judgment`, or `codebook_gap`, plus an optional non-content note ≤80 chars — no free-text task content.
5. **Judge-call economics (BLOCKER-12 round-2 fix — verbatim prereg scope):** the gate harness emits per-session judge-call counts and enforces the prereg caps EXACTLY as frozen: `m3_judge_calls_per_session_cap` 8; `m4_judge_calls_per_session_cap` 128; `m4_shapley_samples_per_candidate_cap` 64 (per candidate, per session). Fails CLOSED on any cap breach. The prereg defines no retry exemption, so ALL retries count toward judge-call caps without exception; cached calls likewise count. The v3 transport-error exemption is WITHDRAWN.

## 6. Privacy boundary (BLOCKER-9, review-answer-4 fixes)

| Artifact | Committed | Content |
|---|---|---|
| `labels.jsonl` | ✅ | per-judgment rows: HMAC-SHA256(session_id, event_id) keyed IDs (key held local, never committed), label, annotator pseudonym, rule-clause if adjudicated |
| `sampling-manifest.json` | ✅ | snapshot/script hashes, seed, HMAC family/session IDs, alternates, exclusions |
| `codebook.md`, `power-sim.py`, `sample.py` | ✅ | method artifacts |
| raw transcripts | ❌ local-only | annotated packets are redacted views; access control = founder machine only; deletion policy: packets purged after corpus freeze; no free-text reasons containing task content (codebook rule R-9) |

Committed task/session hashes are keyed-HMAC, non-reversible without the local key. The committed corpus ships schema version + a manifest hash that the gate verifies.

## 7. Binding into the gate (BLOCKER-12, WARNING-5, review-answer-5 fixes)

1. **Bindable corpus artifact (HIGH-1 fix — HMAC closure):** committed `labels.jsonl` uses keyed-HMAC IDs, but CI cannot reconstruct raw IDs from them without the local key. Binding therefore uses TWO committed artifacts: (a) `labels.jsonl` (HMAC IDs, annotator-facing) and (b) `bindings.jsonl` — a sanitized, deterministic replay manifest generated by the local key holder, mapping each HMAC ID to the public deterministic candidate index (arm, session ordinal, event ordinal) that `oc04_gold.rs` and the pipeline reproduce identically from the committed snapshot. The key NEVER leaves the local machine; CI consumes only public indices and verifies HMAC↔index consistency via a locally-generated check file whose SHA-256 is committed. The corpus is validated end-to-end BEFORE key-dependent files are purged (§6 retention).
   **Replay substrate (v5.2, item 3, FOUNDER-GATE — APPROVED 2026-09-04):** CI must recompute nDCG/Any-hit/bootstrap/cap checks from committed artifacts ALONE while raw transcripts stay local-only. The founder-approved substrate is a committed `replay-inputs.jsonl`: HMAC session/family IDs, public candidate/shortlist indices, gold labels, comparator outputs, and ONLY redacted deterministic feature fields needed to recompute lexical/prior/rerank metrics. Redaction MUST preserve byte-exact lexical features (strict TF=0 and scoring parity depend on it) and a re-identification review precedes commit. This item's privacy-model decision is founder-owned.
2. **Harness real-data path is a PREREQUISITE (HIGH-2 fix — sequencing):** the current `oc04_gold.rs` is synthetic-only (no `include_str!`, no JSONL parser, no manifest verification, no bootstrap, no judge-call accounting). A separate 4G-harness-extension stage implements the real-data branch with fail-closed checks BEFORE labeling begins: labels.jsonl + bindings.jsonl parsing, manifest SHA-256 verification, label mapping, family-cluster bootstrap computation, and judge-call cap emission. The synthetic 6 tests keep passing throughout. The extension ships fixture-backed acceptance tests (v5.2, item 8): valid mini-corpus passes; manifest hash mismatch fails; schema error fails; duplicate binding fails; unresolved `uncertain` fails; unknown HMAC/index fails; bootstrap deterministic for seed 20260820; judge-call cap breach fails closed. Labeling does NOT start until the extension is committed with all fixtures green.
3. **Same-commit flip + fail-closed CI:** `GOLD_LABELS_REAL_DATA=true` lands in the SAME commit as the label file, and the harness fails closed if (a) the manifest hash does not match the frozen sampling manifest, (b) schema validation of `labels.jsonl` fails, or (c) real-data mode is on with any unresolved `uncertain` in the gold set.
4. P3-GO judgment: primary nDCG@12 aggregate + per-stratum, frozen family-cluster bootstrap CI, judged against the EXACT decision rule below (NEW plan-level commitment under founder change control; the prereg freezes metric policy only and defines no threshold, per §1):

   **P3-GO decision rule — D-C-10 §3 adopted VERBATIM (founder-frozen thresholds, not plan-invented; all four must hold for positive-prior C4 deployment):**
   - **D1 (primary nDCG@12):** prior-assisted selection **beats lexical** on the preregistered primary nDCG@12 point estimate, AND the 95% family-cluster bootstrap interval (frozen params: 95%, 10,000 iters, seed 20260820) for the nDCG@12 delta has **lower bound above zero**;
   - **D2 (Any-hit@12):** prior-assisted selection **beats lexical** on the Any-hit@12 point estimate (point-estimate superiority, NOT mere non-inferiority — v5's weakened wording is withdrawn as conflicting with D-C-10 §3);
   - **D3 (strict TF=0):** strict TF=0 Any-hit is **above lexical AND above deterministic random** (a deterministic random-ranking baseline over the same frozen shortlist32 projection, seeded 20260820, computed by the harness and reported alongside);
   - **D4 (no-regression):** no Option B B3–B8 regression (workspace gate EXIT 0 at the binding commit). `shortlist_recall` remains recorded separately per D-C-06 and does not gate. **Dead-end diagnostics (v5.2, item 12, FOUNDER-GATE — APPROVED 2026-09-04):** the frozen binary mapping `dead_end → non-relevant` is preserved, and the harness additionally reports `dead_end@12`, `dead_end@1`, and mean dead-end rank per arm (negative valence invisible to nDCG). Any graded penalty or primary-metric change requires founder sign-off; founder acceptance of the frozen binary mapping is recorded before v5.2 freeze.
   Per D-C-10 §6: if sample size cannot support the interval, the result is **inconclusive** and the gate is NOT lowered post hoc. The rule is founder-frozen (D-C-10 approved 2026-08-21); this plan does not modify it — changing it is founder change control on the decision record itself. Result recorded in `oc-04-evidence.md` Layer-4 update with the achieved family count and CI width (the §2 stopping-rule disclosure).

## 8. Non-claims

No evaluation result is claimed or implied. Every label is human and blind at reliability measurement. No frozen prereg field is altered (labels/strata/metrics/budgets/normalization stay as hashed — where this plan interprets, it cites the frozen text verbatim).

## 9. Normalization (BLOCKER-4 fix — stated verbatim)

The frozen prereg value, quoted VERBATIM from `p1-prereg-config.json`: `"score_normalization": {"method": "per-arm min-max to [0, 1000000] ppm", "clip_above_ppm": 1000000, "clip_below_ppm": 0}`. The prereg record §4.6-c explicitly discloses the normalization WINDOW as a definitional gap. This plan therefore fills the gap as a NEW plan-level interpretation under founder change control (NOT claimed as frozen text): the window is per-arm, per-session, over that arm's RAW scored candidate list, applied before union and rerank. The earlier claim that the window detail was "exactly the frozen text" is withdrawn — only the method string and clip bounds are frozen.

## 10. Change log v1 → v2

- §2 NEW: blinded power/sensitivity analysis + CI-width stopping rule + underpowered disclosure path (B1).
- §3: parent-family cluster accounting; ≥12 families/stratum requirement (B2); temporal holdout cutoff (W2); frozen sampling manifest + deterministic selection script (B10); stratum-4 blind assignment, no replacement (B11); candidate-union manifest pre-frozen, rank-blind labeling, eligibility timestamps (B3, W4); context window fixed ±1 episode (W3).
- §4: decision-tree codebook with examples/counterexamples/precedence; judgment target = user task's correct outcome (B5); `uncertain` = audit-only, adjudicated before scoring, >15% triggers revision (B6); κ on 5-label AND binary (W1); `dead_end` = explicit observable pursuit-and-rejection only (B5).
- §5: two independent blind human annotators; no LLM pre-labels for reliability; adjudication by third party or blinded founder (B7, B8); judge-call caps enforced fail-closed with cache/retry accounting (B12).
- §6: keyed HMAC IDs; redacted packets; retention/deletion; codebook rule R-9 (B9, answer-4).
- §7: fail-closed hash/schema/uncertain checks on real-data mode (answer-5, W5).
- §9: normalization wording aligned verbatim to frozen prereg (B4).

## 11. Review round-2 answers (adopted into v3)

1. **Effect-size-relative rule adopted (§2.2):** CI width ≤ Δ/2 for the smallest material effect; fixed 0.10 withdrawn.
2. **Oversupply scaling adopted (§3 stratum row):** `oversupply_N` pre-declared by formula from the Stage-0 base-rate estimate; a fixed 16 is used only if the simulation proves it achieves the target.
3. **Full forward context adopted (§3 session row):** `dead_end` gets forward visibility through the terminal answer; blinding preserved.

## 12. Change log

### v5.2 → v5.3 (2026-09-05, Stage-0 simulator revision — PRE-LABEL, §2.4 clean restart)
- **Trigger**: Stage-0 v1 UNDERPOWERED at all sizes (48/72/96). Independent audit (Codex gpt-5.5, 41,370 tok) found 2 simulator defects: (FIX-1) v1 bootstrapped the prior-arm score LEVEL, not the paired prior−lexical DELTA — the effect size was invisible to the CI; (FIX-2) ICC mixed a family random intercept with extra per-session noise (impure semantics). v1's 0.131 best width at n=96 dropped to 0.058 under the corrected paired design (~55% variance reduction), proving a large share of the v1 shortfall was simulator artifact, not true underpower.
- **§2.2 threshold change (FOUNDER-SIGNED 2026-09-05, Discord msg 1545772530437202070)**: `CI_width_max_75pct` 0.025 → **0.05** (= Δ full width). Rationale: 0.025 = Δ/4 was a high-precision ESTIMATION bar, not a detectability bar; common practice targets full width ≈ Δ for positive-effect demonstration. Recorded per §2.4 — the change was made BEFORE any labeling exists, so Stage-0 restarts clean and no prior size decision is invalidated.
- **Grid extension (allowed path)**: 48/72/96 → +192/384.
- **§2.4 assumptions re-pinned**: v2 digest `5b617087302ed00175f376c24ee051b7d20e91fdcbe78e4943f156581311d356` (power-sim-v2.py, seed 20260820 unchanged, declared priors unchanged, shared-quality weight 0.6 declared).
- **Stage-0 v2 RESULT: GO — corpus size 192 sessions** (75th-pct CI width 0.0411/0.0413/0.0422 at ICC 0.0/0.1/0.3, all ≤ 0.05; n=384 also passes but 192 is the smallest passing size per the stopping rule).

### v5.2 APPROVAL (2026-09-04)
- Founder approved all 5 FOUNDER-GATE items (Discord msg 1545401201573765120): item 2 design-weighted aggregate option, item 3 replay-inputs.jsonl substrate, item 4 shortlist32 evaluation split, item 5 evidence-wording ratification authority, item 12 dead-end diagnostics (binary mapping preserved). Plan is FROZEN at v5.2.

### v5.1 → v5.2 (dual-model critique→refine: gpt-5.5 author × GLM critic)
- Author self-identified 9 defects (power-sim realism, outcome-conditioned stratum, privacy/replay conflict, shortlist32 reconciliation, coverage ambiguity, κ prevalence inflation, reason-code gap, harness fixtures, version drift); GLM critic judged all 9 VALID, added 6 missed defects (κ pre-adjudication, coverage blinding leak, dead_end valence, strict alternates incoherence, no pilot stage, attrition rule) and 6 fixes.
- Consolidated adoption: 15 items — 14 ADOPT-AS-IS, 1 ADOPT-MODIFIED (item 12 founder-acceptance wording), 0 REJECTED. FOUNDER-GATE items: 2 (design weighting), 3 (replay substrate), 4 (shortlist32 projection), 5 (evidence wording ratification), 12 (dead_end diagnostics).
- §2: clustered-outcome power sim, 75th-pct CI rule, ICC grid, priors hash-pinned. §3: union/shortlist32 split, coverage redefined, alternates scoped. §4: structured reason codes. §5: per-label positive agreement, pilot stage, adjudication blindness, attrition fail-closed. §6/§7: replay-inputs.jsonl substrate (founder-gated), fixture tests, dead_end diagnostics.

### v5.1 (cross-validation round 2 residual)
- §7.4: decision rule rewritten to adopt D-C-10 §3 VERBATIM — point-estimate superiority (not non-inferiority) on nDCG@12 AND Any-hit@12; strict TF=0 above lexical AND deterministic random (seeded baseline added); D-C-10 §6 inconclusive-if-underpowered clause added. v5's weakened D2/D3 withdrawn (reviewer: conflicted with founder-frozen D-C-10).

### v4.1 → v5 (gpt-5.6-sol fresh cross-validation, NO-GO → fixes)
- §7.1 NEW: dual-artifact binding (labels.jsonl HMAC + bindings.jsonl public indices + committed check-file SHA) — HIGH-1 HMAC closure.
- §7.2 NEW: harness real-data path is a PREREQUISITE stage committed and gated BEFORE labeling starts — HIGH-2 sequencing.
- §7.4 NEW: exact P3-GO decision rule D1–D4 (CI lower bound > 0, Any-hit non-inferiority, strict-TF0 recovery, no-regression disclosure) — HIGH-3.
- §9: normalization window explicitly re-labeled as plan-level interpretation filling prereg gap §4.6-c; frozen JSON quoted verbatim — MED-4.
- §3: gate renamed `P3-GO (pooled-candidate reranking)` with IDCG-inflation disclosure; coverage diagnostic mandatory — MED-5.
- Title/revision synced v5 — LOW-6.

### v4.1 (round 4 residuals)
- §7: "prereg threshold" wording corrected — threshold is a plan-level founder commitment, prereg has none (residual 1).
- Title: v3 → v4 version-sync fix (residual 2).

### v3 → v4 (round 3 residuals)
- §3 candidate-union: arms restated as the TWO frozen arms (lexical/prior); M0/M1/M2 clarified as extractor versions, not arms (residual 1).
- §3 diagnostic: `nomination_coverage@all` → `bounded_next12_coverage`, explicitly not frozen `shortlist_recall` (residual 2).
- §5.5: transport-error retry exemption withdrawn — all retries count toward caps (residual 3).

### v2 → v3 (round 2)
- §1: numeric criteria re-labeled as NEW plan commitments (NEW-1 fix) — prereg freezes policy only.
- §2.2: CI-width rule → Δ/2 effect-relative (blocker 1). §2.4 NEW: power-sim.py assumptions hash-pinned pre-label; revision restarts Stage 0 (blocker 1 residual).
- §3 candidate-union: pool-blind gap disclosed; `nomination_coverage@all` secondary diagnostic added (blocker 3).
- §3 stratum-4: oversupply_N formula + SHORT-stratum disclosure (blocker 11).
- §5.2: LLM never a label source; packet-ordering only (blocker 7). §5.3: codebook revision → full re-label, no regime mixing (NEW-2). §5.5: judge-call caps restated verbatim from prereg incl. per-candidate M4 scope (blocker 12).
- §3 session row: full forward transcript window (NEW-3 / §11.3).

### v1 → v2 (round 1)
- §2 NEW: blinded power/sensitivity analysis + stopping rule + underpowered path (B1). §3: parent-family accounting (B2); temporal holdout (W2); frozen sampling manifest + deterministic script (B10); stratum-4 blind assignment (B11); candidate-union manifest pre-frozen, rank-blind (B3, W4). §4: decision-tree codebook, precedence, judgment target = user task's correct outcome (B5); `uncertain` audit-only (B6); κ 5-label AND binary (W1); `dead_end` explicit evidence (B5). §5: two blind human annotators (B7, B8); judge-call caps fail-closed (B12). §6: keyed HMAC, redacted packets, retention (B9, answer-4). §7: fail-closed hash/schema/uncertain checks (answer-5, W5). §9: normalization verbatim (B4).

### Round-2 verdict summary
- Blockers: 8 RESOLVED (2,3-partial→v3,4,5,6,8,9,10), 4 NOT-RESOLVED (1,3,7,11,12) — ALL now addressed in v3 as itemized above.
- NEW issues 1–3: all addressed (§1, §5.3, §3 session row).
