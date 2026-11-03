# P4 — Thorn Conditional Experiment: Closure as Recorded Evidence Only

date: 2026-11-03 · authority: founder directive "권고 대로 하자" (Discord
message 1567241535446192270, 2026-11-03); roadmap §P4 of
`option-c-priority-and-gate-plan.md` (P4 — conditional and expiring Thorn
experiment).

## Disposition

**P4-GO: NOT-DETERMINED — Thorn retained as recorded evidence only.**
The experiment is not entered. Thorn remains disabled in every deployment
configuration (default off, unchanged), and its negative-knowledge semantics
stay recorded in the C3 evidence surface for any future, separately gated
attempt.

## Grounds

1. The frozen P4-GO gate requires that **human labels demonstrate incremental
   benefit over positive-only selection** and that false suppression remain
   below the frozen threshold.
2. That evidentiary prerequisite is a cross-family consensus gold corpus.
   OC-04 has now closed with **P3 = INCONCLUSIVE, final** (attempt #1 commit
   `d554a2c`: kappa A'–B 0.419/0.475, exclusion 27.12% > 15%; attempt #2
   commit `3212d08`, protocol v6.1+r2, one annotator per family:
   kappa A6–B6 binary 0.536 < 0.60, B6–C6 0.389/0.386 — structural
   cross-family divergence, independently confirmed twice). A third labeling
   attempt is pre-registered as out of scope.
3. Without a human-audited cross-family gold, no incremental-benefit or
   false-suppression measurement satisfying the P4 gate can be constructed.
   Entering the experiment now would produce, at best, an unfalsifiable
   NOT-DETERMINED loop; fail-closed closure records that honestly instead.

## Record

- No Thorn code path is enabled, added, or modified by this closure.
- Positive-prior work (P2) and the Option B pipeline are unaffected.
- This document lives in the allowed planning-artifacts directory and makes
  no causal, completion, or utility claim beyond the disposition above.
