# OC-04 v6.1 Re-labeling Plan — PRE-REGISTERED (2026-11-03, rev v6.1)

status: FROZEN for execution (founder: "진행해. 재라벨 시도" msg 1567073594746601473)
revision: v6.1 (deleg_c7f856e0 FAIL fixes) + r2 (deleg_00fd28f0 FAIL fixes:
strata enumerated exactly, adjudicator cannot adopt uncertain, model string
frozen literally, smoke pass criterion numeric)
basis: oc04-protocol-v6-proposal.md + oc04-postmortem.md (681fd8c)
precedent: P3 = INCONCLUSIVE (d554a2c) STANDS — attempt #2, new
pre-registration. No retroactive change, no reuse of v1.0 labels (codebook
changed → full re-label under codebook v2.1).

## Corpus
Same frozen packet universe as attempt #1: packets_prod/annot_A (192 sessions,
3,979 candidate events, union manifest digest fa84ba0c…, prior=0 disclosed).
Packets unchanged; codebook v2.1 + this protocol govern. Candidate-only
packet context = protocol amendment v5.6 (carried forward, adjudicated v3).

## Annotators — one per family, no twins
- A6 = GLM (frontier GLM, blind)          [family GLM]
- B6 = gpt-6.1-sol (codex read-only)      [family GPT]
- C6 = deepseek-chat (API)                [family DS]
Blind: codebook v2.1 + packets only. No arm/rank/scores, no knowledge of
other annotators.

## Gate order (fail-closed, exactly this sequence; later gates re-run if
## any upstream artifact changes)
G-0 → G-1 → G-2 → [G-0/G-1 re-run if G-2 replaced anything] → G-3 → G-5
(adjudication) → G-4 (final exclusion ceiling, on the FINAL event set after
adjudication). No gate is ever evaluated on a set that later changes.

- **G-0 completeness + machine validation** (all 3 annotators): key set
  identical to packet universe; per-row: label ∈ 5 frozen labels; basis
  format per codebook v2.1 (5-15 words; required has `TYPE:` prefix from
  the five types; uncertain basis exactly one picklist value); R-9 n-gram
  check (no 4+-word verbatim overlap of basis with that event's packet
  text). ANY invalid row → that annotator's session re-dispatched once
  (same retry budget as G-2); second failure → session excluded.
- **G-1 per-annotator corpus-wide uncertain ≤15%** (re-run after any G-2/G-0
  replacement).
- **G-2 per-session bailout**: any annotator ≥30% uncertain in a session →
  that annotator's session re-dispatched ONCE (fresh request). Replacement
  output is authoritative (atomic replace); if the replacement ALSO fails
  G-0 row validation or hits ≥30% uncertain → session excluded (all its
  events enter the exclusion numerator) and G-0/G-1 re-run on the reduced
  universe.
- **G-3 reliability — NOT weakened from attempt #1**: for EACH of the 3
  pairs (A6-B6, A6-C6, B6-C6), BOTH binary(required vs rest; uncertain
  counts as rest) κ ≥0.60 AND 5-label κ ≥0.50. Computed on the
  post-G-2 authoritative set (all surviving events, uncertain labels
  included as a category). Any pair failing either metric → attempt #2
  FAILS (no 2-of-3, no 0.45 floor — that data-dependent weakening is
  withdrawn per review).
- **G-5 adjudication** (only if G-3 passes): adjudicator is frozen NOW as
  OpenRouter model string `qwen/qwen3-max`, temperature 0 (if that exact
  string is unavailable at runtime → G-5 = NOT-APPLICABLE, all splits
  excluded). Adjudicator input per event: the FULL session packet (all
  lines, same context the annotators had) + target event marked + the three
  (label, basis) pairs with family attribution removed and ORDER RANDOMIZED
  (per-event, seed 20260821, hash committed before dispatch). Constrained
  adoption: the adjudicator may ONLY choose among the labels actually voted
  by annotators EXCLUDING `uncertain` — i.e., candidates = distinct
  non-uncertain voted labels; if an event has NO non-uncertain voted label,
  it is excluded without adjudication; an adjudicator output of `uncertain`
  or any label outside the candidate set → event EXCLUDED (uncertain is
  never adopted into gold). Output must carry a v2.1-valid basis (basis
  rules for its chosen label); invalid output → event excluded. Pilot gold
  307 agreement is reported as a NON-GATING diagnostic only (it is
  v1.0-codebook gold; using it as a v2.1 gate would be circular —
  withdrawn per review).
- **G-4 exclusion ceiling — FINAL, after adjudication**: numerator = every
  event of the 3,979-universe not adopted (second-bailout sessions, splits
  unresolved/unadjudicated, adjudicator-invalid, no-non-uncertain-vote).
  denominator = 3,979 (fixed universe — no "decisive candidates" shrinkage).
  Gate: overall ≤15% (≤596 events). Strata — exact enumeration, 5 strata
  from the frozen attempt-#1 outputs (labels_prod.jsonl 2,900 adopted:
  `required`, `supporting`, `irrelevant`, `dead_end` — `uncertain` was
  never adopted under v5.5 so there is no adopted-uncertain stratum — plus
  1 stratum = the 1,079 attempt-#1-excluded events): strata with ≥100
  events gated at ≤15%; smaller strata reported only. Frozen before
  compile; recomputed once, after G-5.

## Consensus rule (v6.1)
- All 3 non-uncertain and unanimous → ADOPT.
- Otherwise → ADJUDICATE under G-5. No majority-of-3 adoption anywhere.

## κ gate history (context)
Attempt #1 failed κ 0.419/0.475 with bloc voting. v6.1 removes twins,
makes the boundary observable (consumer test), gates per-session bailouts,
and requires the FULL attempt-#1 κ standard on all 3 pairs. If v6.1 fails
any gate → P3 INCONCLUSIVE becomes FINAL (no attempt #3 without a new
founder decision).

## Execution order (frozen)
1. Independent review of THIS plan + codebook v2.1 (single reviewer,
   fail-closed: FAIL → amend → re-review before any labeling).
2. dispatcher_v6 updated to enforce v2.1 basis rules; smoke test on 2
   sessions × 3 engines. Smoke PASS criterion (frozen): each engine yields
   2/2 sessions with 100% G-0-valid rows (parse + basis format). FAIL →
   output-format prompt wording ONLY may be amended once (codebook
   semantics untouched) and re-smoked once; second failure → stop and
   report, no full launch without founder. Smoke outputs discarded.
3. Labeling: A6 (GLM ×8 shards), B6 (codex gpt-6.1-sol ×2), C6 (deepseek
   ×2), background, resumable.
4. Gate sequence above → compile → report → HMAC-only commit gate → founder
   commit approval.
5. Only after a v6.1 gold exists: D1–D4 evaluation (P3 decision), unchanged.

## Unchanged frozen elements
D1–D4 definitions, K=3 family-fold cross-fitting (seed 20260820), bootstrap,
pilot gold 307, attempt-#1 P3 INCONCLUSIVE verdict, 15% ceiling value.
