# OC-04 Stage 4G — Human-Gold Metric Gate Evidence (4-layer)

**Status:** 4G harness SHIPPED · **P3-GO: OPEN** (synthetic labels — real human gold pending, per spec §14)
**Branch:** `OC-AttentionLedger` · **Date:** 2026-09-04
**Artifacts:** `contextmesh-salience/tests/oc04_gold.rs` (harness, 6 tests) + this document.

---

## Layer 1 — 출처 (Sources)

| Claim source | Reference |
|---|---|
| Gate definition | `spec-oc-04-selection.md` §14 + matrix row 4G (l.53): harness = `contextmesh-salience/tests/oc04_gold.rs` (nDCG@12 evaluator, synthetic-label path marked NOT-real-data); P3-GO stays OPEN until real gold |
| Preregistered primary metric | `p1-prereg-record.md` §3 → `evaluation.metrics` (primary nDCG@12), frozen hash-sealed |
| Strict TF=0 stratum | `p1-prereg-record.md` §3 → `evaluation.sample_strata[3]` = `strict_all_gold_tf0` |
| Disclosure discipline | `oc02_evaluation.rs` header: synthetic corpus is NOT the real replay data (4D/4C fixture lineage) |
| 5D linkage | `oc-05-release-evidence.md`: P5-GO DEFERRED until 4G + real gold |

## Layer 2 — 사고과정 (Reasoning)

1. **Metric choice is prereg-bound, not re-derived.** nDCG@12 was frozen in P1 prereg (`evaluation.metrics`). 4G implements exactly that depth (k=12) and binary relevance — no re-parameterization.
2. **Integer arithmetic only.** The repo-wide determinism discipline (no floats, no clocks, no network — cf. `oc02_evaluation.rs`, `replay.py`) extends to the evaluator: discounts are a fixed-point ppm table (`DISCOUNT_PPM[i] = round(1e6/log2(i+2))`), and nDCG is `dcg*1e6/idcg` with truncating integer division. Hand-computed constants make every value auditable.
3. **Synthetic-label honesty.** §14 mandates: if no human gold exists at 4G, ship the harness with synthetic labels marked NOT-real-data and keep the gate OPEN. The constant `GOLD_LABELS_REAL_DATA = false` is the machine-checkable disclosure; test 4G-V01 panics if it flips, so a real-data swap must edit this file (founder change control visible in diff).
4. **Pipeline realism over mock metrics.** The ranked list under evaluation comes from the REAL 4C→4D chain (`union_candidates` → `rerank` over real signed events and a pipeline-built `VerifiedPrior`) — the same code path production selection uses, so the evaluator measures the actual artifact, not a mock.
5. **Evaluator correctness precedes metric claims.** 4G-V02 pins hand-computed nDCG values (including rank-order sensitivity, beyond-k exclusion, empty-gold guard) BEFORE any pipeline metric is asserted — an evaluator bug cannot masquerade as a pipeline result.

## Layer 3 — 결론 도출법 (Derivation method)

| Test | Verdict derived how |
|---|---|
| 4G-V01 `gold_labels_disclosed_synthetic` | Constant-inspection: `GOLD_LABELS_REAL_DATA == false` else panic (disclosure enforced, not assumed) |
| 4G-V02 `ndcg_evaluator_exact` | Hand-computed integer table: gold@rank0 → 1,000,000 ppm; @rank1 → 630,930; two-gold 2-of-3 → `1_130_930×1e6/1_630_930 = 693_426` (truncating); rank-13 hit → 0 (beyond k=12); empty gold → 0 |
| 4G-V03 `pipeline_ndcg_perfect_on_named_gold` | Real pipeline run: task "alpha" names the `evt-a` payload → gold={evt-a} → ranked[0]=evt-a → nDCG@12 = 1,000,000 ppm |
| 4G-V04 `strict_tf0_recovers_through_prior_arm` | Task "zzz qqq" (TF=0 for all sources) → lexical arm empty (asserted) → union non-empty via prior arm → ranked list non-empty, every entry `lexical_ppm == 0` (TF=0 not excluded — P3 strict-recovery discipline) |
| 4G-V05 `ndcg_rank_order_sensitive` | Demoting gold rank0→rank1 strictly lowers nDCG (1,000,000 → 630,930): a constant metric could never gate |
| 4G-V06 `pipeline_ranking_deterministic` | Two identical pipeline runs → byte-identical ranking |

## Layer 4 — 도출 이유 (Why this way)

- **Gate stays OPEN by design, not by omission.** Recording "P3-GO: OPEN" with a shipped evaluator is the honest state: the machinery to judge is in place; the human input it needs is not. Fabricating a synthetic "GO" would violate the prereg non-claims (no evaluation result exists) and the claim-honesty principle that froze OC-05 §12 (Option 2).
- **No new dependencies, no API change.** The harness is a pure test module over existing public APIs (`union_candidates`, `rerank`, `VerifiedPrior`) — X06 no-new-deps gate unaffected; root-crate surface unchanged.
- **Why binary relevance:** the frozen prereg names nDCG@12 without graded gains; binary is the minimal faithful reading. Graded gains would be a metric redefinition requiring founder renegotiation (§15 change control).
- **Why the pipeline fixture is small (2 sources):** 4G gates the EVALUATOR + pipeline-binding correctness, not corpus scale. Scale is a real-gold-corpus property (human-labeled sessions), which is exactly what 4G defers.

## Gate result

| Gate | Result |
|---|---|
| Focused (oc03_artifact 11 + oc04_exec 15 + oc04_gold 6) | GREEN, EXIT 0 |
| Workspace regression | **494 passed / 0 failed**, REGRESSION_EXIT=0 |
| clippy -p contextmesh-salience --tests | 0 warnings |
| fmt --check | clean |
| **P3-GO** | **OPEN** — awaiting real human gold (synthetic labels shipped per §14) |
| P5-GO (OC-05) | DEFERRED — unblocks only when 4G real gold exists |

## Non-claims

This evidence does NOT claim: real-world retrieval quality; human-judged relevance; OC-0.5 replay-data results; model quality; or any evaluation outcome. It claims ONLY that the nDCG@12 evaluator is integer-exact and rank-order-sensitive, and that the real OC-04 selection pipeline produces deterministic, TF=0-inclusive rankings on the disclosed synthetic corpus.
