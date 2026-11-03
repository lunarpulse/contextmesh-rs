# OC-04 Phase 2 Consensus Report — P3 INCONCLUSIVE (fail-closed)

date: 2026-11-03
plan: oc04-production-labeling-plan.md v3 (recheck deleg_37c1b731 PASS-WITH-RESIDUALS)
status: FINAL — closed per founder decision (Discord msg 1566973269322764309:
"1번 하고 싶지만 내가 결정하는건 시간이 너무 오래 걸려. 다른 대안은 없나? 없으면 2번"
→ alternative 1-Lite assessed as not verdict-changing; option 2 selected)

## Phase 1 — all gates PASS
- 4 blind annotators, 192 sessions, 3,979 candidates each, 0 defective rows:
  A′ GLM×8 subagents, A″ gpt-6.1-sol×2, B gpt-5.6-sol×2, C deepseek-chat×2
- Key sets (session_hmac, event_index) identical across all 4 annotators and
  equal to the annot_A packet universe (check_keys_prod.py PHASE1-KEYGATE: PASS)
- Uncertain ≤15% per annotator: A′ 6.5% / B 5.0% / A″ 8.5% / C 1.1% — PASS

## Phase 2 first consensus compile (compile_consensus_prod.py, v5.5 rule:
## 3-of-4 majority incl. ≥2 model-family camps; uncertain never adopted)
- Adopted 2,900 / Excluded 1,079 (27.12% — ceiling 15% BREACHED)
- Excluded composition: 2-2 splits 436 · scatter (2-1-1 / all-distinct) 641 ·
  all-uncertain 2
- Adopted distribution: required 1,909 · supporting 425 · irrelevant 456 ·
  dead_end 110

## Pairwise κ (Cohen, n=3,979)
- PRIMARY AXIS A′↔B (GLM↔GPT, cross-family): κ5 0.4190, κbin 0.4753
  — FAILS frozen gates (κ5 ≥0.5, κbin ≥0.6)
- A′↔A″ 0.4073/0.4645 · A′↔C 0.2342/0.3194 · B↔C 0.4094/0.5335 ·
  A″↔C 0.3717/0.5429
- Only passing pair: A″↔B 0.6644/0.7687 — same family (GPT↔GPT),
  ineligible as primary axis and indicative of intra-family correlation,
  not independent reliability

## Verdict
Per plan v3 gates (exclusion ceiling; κ primary axis), the human-audited
cross-family consensus gold DOES NOT FORM at production scale. P3 closes
INCONCLUSIVE (fail-closed). D1–D4 never evaluated.

Honest finding: at this scale, GLM↔GPT cross-family label agreement
(κ ≈ 0.42–0.48) is materially below the pilot founder↔B level (0.688/0.750);
the pilot's cross-family PASS does not generalize from founder adjudication
to pure LLM consensus. This negative reliability result is itself the
deliverable of the production run.

Alternative assessed and rejected (1-Lite: frontier tie-break adjudicator for
the 436 2-2 splits + founder sample audit): recovers the exclusion ceiling
only; κ is computed on raw annotator labels and cannot be changed by
adjudication, so the P3 verdict would remain INCONCLUSIVE.

## Artifacts (local-only, HMAC-only where noted)
- packets_prod/labels_prod.jsonl — 2,900 adopted (HMAC-only), NOT a passing
  gold corpus; retained for post-mortem analysis
- packets_prod/founder_docket_22.json — 436 2-2 split events (votes, no raw)
- packets_prod/consensus_report.json — machine-readable tallies
- oc04-gold/compile_consensus_prod.py — frozen compiler (v5.5 rule)
- Phase 5 HMAC-only commit gate applies unchanged to any committed artifact
