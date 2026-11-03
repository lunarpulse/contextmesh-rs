# OC-04 Gold Label Codebook v2.1 — FROZEN for protocol v6.1 (2026-11-03)

Supersedes v2.0 (unexecuted draft; review deleg_c7f856e0 FAIL fixes).
Same five frozen labels. Boundary now observable-consumer based.

## Decision tree (first-match-wins) — downstream-consumption check FIRST

For each event, ask in order:

1. CONSUMPTION: is there an identifiable downstream consumer of this event's
   output within the packet — a later line that uses its answer text, file,
   command result, or verification outcome (including a later fix that
   consumes a failed attempt's error)? → `required`
   basis must be: `TYPE: <what later line consumed it>` where TYPE ∈
   {answer_text, file_output, command_result, verification, unblock}.
   No identifiable consumer → NOT required (even if it feels important).
2. ABANDONED FAILURE: did this event start/continue a path the session
   abandoned, with NO later line consuming its output? → `dead_end`
   (If a later fix consumed the error → rule 1 fires first → required/unblock.)
3. NO-CONSUMER VALUE: no consumer, but plausibly sped or de-risked the
   path (orientation, redundant confirmation, benign exploration)? →
   `supporting`
4. NO RELATION: no plausible connection to reaching the outcome → `irrelevant`
5. CANNOT JUDGE from the packet → `uncertain` (basis = picklist:
   `insufficient-context` | `ambiguous-role` | `ambiguous-consumer`)

## Standards (single observable standard — no plausible/demonstrable mix)

- `required` ⇔ observable downstream consumption. This is the ONLY route to
  required. "Would have been needed" without an observable consumer is
  supporting at most.
- Counterexamples (NOT required): doc reads that informed style only;
  re-verification of an already-consumed result; exploration never used.
- The removal test is a corollary of the consumer test, not a separate
  plausibility standard.

## Uncertain discipline

Per-event, last resort, never per session. A session with ≥30% uncertain
from one annotator triggers protocol gate G-2 (re-dispatch once).

## Output format (machine-validated at G-0)

{"event_index": <int>, "label": "<required|supporting|dead_end|irrelevant|uncertain>",
 "basis": "<see rules below>"}

- required: `TYPE: phrase` — 5-15 words total, TYPE from the five above.
- supporting/dead_end/irrelevant: 5-15 words stating the consumer test
  outcome (e.g. "no downstream consumer; orientation only").
- uncertain: exactly one picklist value, nothing else.
- R-9: basis must contain NO task content — no 4+-word verbatim overlap
  with the packet line text (machine-checked at G-0; violation = invalid row).
