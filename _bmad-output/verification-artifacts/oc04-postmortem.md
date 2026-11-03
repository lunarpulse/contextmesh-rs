# OC-04 Post-mortem — Why Cross-Family Consensus Failed

date: 2026-11-03
inputs: packets_prod/labels_{Aprime,A2sol,B,C} (192 sessions, 3,979 events each)
scripts: /home/cosmo/.hermes/cache/scratch/{postmortem_oc04,inspect_zero,bailout_diag}.py
status: ANALYSIS COMPLETE — local, not yet committed

## Finding 1 — Annotator "bailout" sessions (operational defect, NOT semantics)
7 sessions (330 events, 8.3% of corpus) have ≥1 annotator labeling ≥50% of the
session as `uncertain`; 4 of them (256 events) have ≥2 annotators bailing,
which mechanically destroys consensus there:
- 910ff102065a (n=64): A′ AND A″ all-uncertain (B↔C still agree 0.66)
- 9aa3d3221c9b (n=64): A′ 63/64 uncertain; B separately anomalous (52 irrelevant)
- e30f83a58bf5 (n=64): A″ all-uncertain AND B 61/64 uncertain (A′↔C agree 0.67)
- f0eeced6e3a79a / abd78beea7b0dc (n=64 each): 3 annotators bail
- 7ea90104b59c / e0dec03141ea (n=5): A″ bails on tiny sessions
The per-annotator uncertain≤15% gate (measured corpus-wide) could not catch
per-session bailouts. These sessions contributed 192 of the 1,079 exclusions.
Offset-misalignment hypothesis tested and rejected (shift ±1 agreement = 0).

## Finding 2 — Removing bailouts does NOT fix the gates (the failure is real)
Clean-subset diagnostic (185 sessions, 3,649 events):
- exclusion rate 20.88% (vs 27.12% full) — still above the 15% ceiling
- κ A′↔B 0.4132/0.4731 (vs 0.4190/0.4753 full) — unchanged
- κ A″↔B 0.6669/0.7905 — GPT-twin agreement persists
Conclusion: the INCONCLUSIVE verdict is not an artifact of bad sessions.

## Finding 3 — Bloc voting, not independent disagreement
2-2 split coalition structure (436 splits):
- {A′,C} vs {A″,B}: 222 (51%) — GLM+DeepSeek bloc vs GPT bloc
- {A′,A″} vs {B,C}: 122 (28%)
- {A′,B} vs {A″,C}: 92 (21%)
The two GPT models vote as a bloc; cross-family κ measures GPT-bloc vs
everything-else, and the GPT twins' 0.89 binary agreement is family
correlation, not independent reliability.

## Finding 4 — The disagreement is concentrated at one boundary
Where the GPT pair agrees but A′ (GLM) differs (top transitions):
- required↔supporting: 293 + 214 = 507 events (largest)
- required/supporting → irrelevant: 136 + 109 = 245
- reverse (irrelevant → req/supp): 61
GLM applies `supporting` broadly (769 vs B 760 but A″ 615) and splits
required/supporting differently; GPT models are more decisive on `required`.
Binary required-vs-rest agreement matrix: A″↔B 0.885; all other pairs
0.667–0.773. The core notion "required" itself is family-relative.

## Implications for a future protocol
1. Mass-bailout detection must be a per-session gate (e.g., ≥30% uncertain
   in one session → re-dispatch that annotator's session before compile).
2. Cross-family 4-vote consensus is structurally dominated by the largest
   same-family bloc; a workable protocol needs either (a) one annotator per
   family with ≥3 families + adjudicator, or (b) family-balanced voting
   weights, pre-registered.
3. The required/supporting boundary needs an operationalized definition
   (current codebook leaves it judgmental); 507 events ≈ 12.7% of the corpus
   churn there — more than the entire 15% exclusion budget.
4. Pilot's founder↔B κ 0.75 was cross-family only because the founder is not
   a model family; human adjudication was the hidden stabilizer.
