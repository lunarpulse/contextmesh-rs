# OC-04 Gold Protocol v6 — Proposal (DRAFT, NOT PRE-REGISTERED)

date: 2026-11-03
basis: oc04-postmortem.md (commit 681fd8c), P3 INCONCLUSIVE closeout (d554a2c)
status: DRAFT — requires founder approval before any pre-registration or execution

## Diagnosis this proposal answers (from post-mortem)
- D1. Per-session annotator bailouts (7 sessions, 8.3% of corpus) destroyed
  consensus mechanically; the corpus-wide uncertain gate could not see them.
- D2. Voting was bloc voting: 2 GPT annotators acted as one bloc (binary
  agreement 0.885); cross-family kappa measured "GPT bloc vs rest".
- D3. 12.7% of the corpus churns on the required<->supporting boundary alone —
  the codebook leaves it judgmental.
- D4. The pilot's κ 0.75 was stabilized by the human founder, who is not a
  model family. Pure-LLM consensus has no such anchor.

## Design changes

### 1. One annotator per family, ≥3 families, family-weighted vote
- Annotators: GLM, GPT, DS (one each). Optional 4th family (e.g. Qwen/Gemini)
  if available.
- Consensus rule: unanimous-or-adjudicate. With 3-4 voters from distinct
  families there are no same-family twins to form a bloc; any split goes to
  the adjudicator instead of being resolved by bloc majority.
- Rationale: post-mortem Finding 3 — the 3-of-4 cross-family rule failed
  because the largest same-family bloc de facto controlled ties.

### 2. Per-session bailout gate (fail-closed, pre-compile)
- After each annotator run: if any session has ≥30% `uncertain` from that
  annotator, that annotator's session is re-dispatched once (fresh context,
  same packet). Second bailout → session excluded and counted against the
  exclusion ceiling; event never silently destroys consensus.
- New gate: per-session uncertain ≤30% AND corpus-wide ≤15% (unchanged).

### 3. Operationalized required/supporting boundary (codebook v2.0)
- `required`: removal of the event would plausibly change the terminal answer
  or make the task fail. Test: annotator must cite the specific downstream
  dependency (answer text, file written, test passed).
- `supporting`: plausibly accelerated or de-risked the path but removal would
  not change the terminal answer.
- Annotators must output a one-clause justification per non-uncertain label;
  adjudicator reviews justifications on splits (not bare votes).

### 4. Adjudicator = tie-breaker with justification review
- A frontier model from a 4th family (not any annotator family), or the
  founder for a bounded sample (e.g. 50 events max).
- Adjudicator sees the split votes + justifications + packet, blind to which
  family cast which vote.

### 5. Exclusion ceiling (unchanged): ≤15% of decisive candidates,
  overall and per pre-label stratum, frozen before compile.

## Open questions for the founder
1. Proceed with a re-labeling attempt at all (option 1-territory), or keep
   P3 INCONCLUSIVE as final and move to P4/P5 track decisions?
2. Budget: a v6 run re-labels 192 sessions with 3-4 annotators + adjudication
   of splits. The existing 4 label sets can be reused partially only if the
   codebook change is judged compatible (it is NOT — labels are judgmental,
   so full re-label is the honest default).
3. If re-labeling: same 192-session corpus or a fresh sample?

## What this does NOT claim
- v6 does not retroactively change P3 = INCONCLUSIVE. That verdict stands.
- No reuse of existing labels under a new rule without re-labeling.
