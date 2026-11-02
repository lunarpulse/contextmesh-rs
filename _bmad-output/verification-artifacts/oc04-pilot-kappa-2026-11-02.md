# OC-04 Pilot κ Record — 2026-11-02 (gold labeling cycle 1)

## 1. Sources (출처)
- Labels: `packets/labels_{Aprime,A2sol,B,C}/<hmac>.jsonl` (344 each,
  blind, codebook v1.0; A′ GLM via Hermes subagents 2026-10-16,
  A″ gpt-6.1-sol + B gpt-5.6-sol via frozen `label_dispatcher.py`
  prompt, C deepseek-chat).
- Founder audit: `packets/adjudication_A/dossiers/{907ffba1,6c93c25f,
  b0760839,ace6cd53}.md` 재정의 fields (44 overrides, 2026-11-02;
  founder direction: no further human adjudication, msg
  `1566764000182673452`).
- κ computation: `oc04-gold/kappa.py` (frozen §5.3), inputs
  `packets/kappa_{A,B,C}.jsonl` (307 included events).

## 2. Process (사고과정)
- Isolated-event packets were unjudgeable for the human (context
  starvation — LLM annotators saw whole sessions, human saw one
  event). v3 session-dossier audit format fixed this; founder
  audited the top-4 highest-disagreement sessions only (minimal
  review budget, eval rule 7).
- Final A per event = founder override (44) > 3-of-4 cross-family
  consensus (263) > excluded (37 split events in unaudited sessions,
  no majority and no human decision — fail-closed exclusion).
- 4-way raw agreement: A′↔A″ 76.5%, A′↔B 70.3%, A′↔C 77.0%,
  A″↔B 77.0%, A″↔C 72.1%, B↔C 66.6%. Quad-unanimous 198/344.

## 3. Derivation (결론 도출법)
Final corpus: 307 events (required 216 / supporting 65 /
irrelevant 22 / dead_end 4). κ on 307 paired events:

| axis | κ binary | κ 5-label | PA required | PA supporting | PA dead_end | verdict |
|---|---|---|---|---|---|---|
| A↔B | 0.7502 | 0.6881 | 0.9248 (n=408) | 0.6647 (n=139) | 0.70 (n=14 <20 → audit sampling) | **PASS** |
| A↔C | 0.7118 | 0.5092 | 0.9018 (n=466) | **0.4704 (n=99) FAIL** | n=4 <20 | **FAIL** |

## 4. Rationale (도출 이유)
- The A↔C failure is concentrated on `supporting`: C's label
  distribution is degenerate on the decisive-tail labels (C emitted
  dead_end=0 across all 344; supporting=52 vs A-final 65), i.e. an
  annotator-validity problem, not a codebook ambiguity problem —
  the codebook separates required/irrelevant well for all axes
  (PA required ≥ 0.90 everywhere).
- Frozen §5.3 consequence of a failed axis (codebook revision +
  full re-label by both annotators) is NOT executable: founder
  waived further human annotation (review budget). Disposition
  proposed for founder approval: C demoted to reference-only axis
  (recorded deviation, disclosed in P3-GO evidence), primary
  reliability axis = A↔B; production labeling of the 192-session
  corpus proceeds on the cross-family consensus protocol validated
  here (A↔B κ binary 0.75 / 5-label 0.69).
- Honest-claim boundary: this pilot's A is a hybrid (consensus +
  44 human overrides on 4 sessions), not a pure-human annotator;
  P3-GO claims must say "human-audited cross-family consensus
  gold", not "human gold".
