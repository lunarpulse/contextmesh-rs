# P1 Preregistration Record — Salience Evaluation Configuration

**Record ID:** `P1-PREREG-SALIENCE-EVAL-V1` · **Status:** frozen (hash-sealed) · **Date:** 2026-08-26

## 1. What this is

The priority plan (`option-c-priority-and-gate-plan.md` §P1, L105–115) and founder decision D-C-05/D-C-10 require the evaluation configuration — sample strata, labels, family split, extractor versions, budgets, metrics, family-cluster bootstrap, score normalization, exact rerank formula, per-arm candidate caps, deduplication/tie-break, and checked-overflow policy — to be **canonically serialized and hash-frozen before P2 (= OC-02) implementation and before any test-label inspection**.

This document records that freeze. It contains **policy only**: no status fields, no approval state, no evaluation results, no thresholds that depend on seeing data.

## 2. Frozen artifact

| Field | Value |
|---|---|
| File | `_bmad-output/implementation-artifacts/p1-prereg-config.json` |
| Commit | `c080722` (branch `OC-AttentionLedger`) |
| Git blob SHA-1 | `cd51a18855fb775c4540c89270755e00e06d8478` |
| Content SHA-256 | `be20d8fc48771098e745038b906dd13456ffcebdeb424cee25e91d52eae784c9` |
| Size | 2,688 bytes |
| Serialization | UTF-8 JSON (2-space indent, LF line endings, trailing newline) |

Verification commands (read-only):

```sh
git ls-tree c080722 -- _bmad-output/implementation-artifacts/p1-prereg-config.json
# → 100644 blob cd51a18855fb775c4540c89270755e00e06d8478
git cat-file blob cd51a18855fb775c4540c89270755e00e06d8478 | sha256sum
# → be20d8fc48771098e745038b906dd13456ffcebdeb424cee25e91d52eae784c9
```

## 3. Coverage map (decision record → frozen config)

| Required by | Config field |
|---|---|
| Plan L106 `labels` | `evaluation.labels` (5 labels, exact names) |
| Plan L106 `family split` | `evaluation.family_split` (temporal parent-family) |
| Plan L106 `extractor versions` | `evaluation.extractor_versions` (M0/M1/M2/prior) |
| Plan L106 `budgets` | `evaluation.budgets` (shortlist cap 32, M3 ≤8/session, M4 ≤64 samples & ≤128 judge calls/session; shortlist recall separately recorded per D-C-06) |
| Plan L106 `metrics` | `evaluation.metrics` (primary nDCG@12 + secondary + shortlist recall) |
| Plan L106 `family-cluster bootstrap` | `evaluation.family_bootstrap_detail` (95%, 10,000 iters, seed 20260820) |
| Plan L106 `score normalization` | `evaluation.score_normalization` (per-arm min-max to ppm) |
| Plan L106 `exact rerank formula` | `selection_pipeline.rerank_formula` (`score_ppm = lexical_ppm + prior_ppm`) |
| Plan L106 `lexical/prior per-arm caps` | `selection_pipeline.per_arm_caps` (64 / 30) |
| Plan L106 `deduplication/tie-break` | `selection_pipeline.deduplication` + `tie_break` (EventId, canonical ascending) |
| Plan L106 `checked-overflow policy` | `selection_pipeline.overflow_policy` (u128 widen, fail closed) |
| Plan L110 `strict all-gold-TF=0 stratum` | `evaluation.sample_strata[3]` = `strict_all_gold_tf0` |
| D-C-05 #2–4 | `union_rule` (EventId-deduplicated union), `rerank_formula` (additive, TF=0 not excluded), `thorn: disabled` |
| D-C-06 #1–3 | M0/M1/M2 nominate, M3/M4 shortlist-bound, `shortlist_recall` separately recorded |
| D-C-07 (M2 v1 explicit structural only) | `m2_extractor_version` placeholder (structure-only contract noted in OC-02 spec) |
| D-C-10 #1–2 | labels + strict TF=0 stratum present |

## 4. Non-claims

1. This record does not approve, define, or claim OC-02 implementation or any part of C2.
2. No evaluation has been run; no result is claimed or implied.
3. Extractor version strings for M2 (`oc-2-m2-v1`) and prior (`oc-3-prior-v1`) are versioned placeholders naming the future artifact that will bind them; M0/M1 strings reuse the OC-00 prototype identifiers for continuity, as recorded in the OC-00 replay harness (`replay.py` `M0_VERSION`).
4. The frozen hash seals bytes, not semantics: any future dispute about intent is resolved by the decision record and priority plan, not by re-deriving policy from the hash.
5. Changing any frozen field requires founder approval under §16-style change control and a new freeze record with a new hash (the old record is never edited).
6. **Known definitional gaps (disclosed):** the sealed config names strata, labels, and caps but does not itself define (a) per-stratum sample sizes, (b) the term "session" used in judge-call caps, (c) the computation window of per-arm min-max normalization (per-session vs global), or (d) label semantics beyond their names. These are frozen as names/values only; the OC-02 specification must define them unambiguously without altering any frozen field value. Recorded per the quality review (2026-08-26) warnings 1–4.

## 5. Freeze evidence

- Committed `c080722` on branch `OC-AttentionLedger`; content sealed by the blob SHA-1 and SHA-256 values in §2.
- This record itself is committed after the seal commit (it records the seal; it does not alter it).
