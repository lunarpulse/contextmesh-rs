---
title: 'OC-04 Production Labeling & P3-GO Execution Plan'
type: 'execution-plan'
created: '2026-11-02'
revision: 'v3 (2026-11-02) — recheck deleg_b5690f8d FAIL fixes: METH-03 formal amendment + context-bias quantification, D3 restricted-universe consequence, exclusion ceiling defined, fold assignment frozen'
status: 'CLOSED 2026-11-03 — P3 INCONCLUSIVE (fail-closed): Phase 2 κ primary axis A′↔B 0.419/0.475 and exclusion 27.12% breach frozen gates; founder selected INCONCLUSIVE closeout (msg 1566973269322764309). Prior status: founder-approved-for-execution (msg 1566788535594389575); v2/v3 amendments under standing auto-approval directive (2026-10-23)'
branch: 'OC-AttentionLedger'
baseline_commit: '01cb86f'
review_evidence: 'deleg_ca2d267a (2026-11-02): METH-01..08 / FM-01..10 / F-01..07 — all 3 verdicts NO-GO on v1; consolidated below'
---

# OC-04 Production Labeling & P3-GO Execution Plan (v3)

Extends the pilot gold cycle (v5.5 consensus protocol, commit `01cb86f`)
to the frozen 192-session corpus (sampling-manifest.json v5.4, seed
20260820) through the P3-GO judgment.

## Phase 0 — Production union manifest (DONE, digest-verified)
- `run_union_prod.py`: frozen pipeline (importer → lexical cap 64 →
  empty-prior arm, disclosed) over all 192 sessions →
  `production-union-manifest.json` (local; raw text never committed).
- Acceptance MET: 192/192 sessions, 6,414 events → 3,979 candidates,
  double-run sha256 identical (`fa84ba0c…93aa60`).
- v2 hardening (FM-09): future generator runs must (a) check cargo
  subprocess returncode, (b) write to a staging path and promote
  atomically only after 192-session set + digest verification, (c)
  packet builder rebuilds into a staging tree and swaps only when the
  expected file set is exact; dispatcher globs are validated against
  the manifest allowlist.

## Phase 1 — Blind 4-annotator labeling (3,979 events) [IN FLIGHT]
- Same 4 blind annotators, same frozen prompt (codebook v1.0): A′ GLM,
  A″ gpt-6.1-sol, B gpt-5.6-sol, C deepseek-chat. Sharded, resumable.
- v2 acceptance per annotator (FM-02/03, F-05) — a PRODUCTION verifier
  (not pilot verify_labels.py) must fail-closed unless ALL hold:
  - filename set == exactly 192 files == annot_A packet set;
  - key universe == exactly {(session_hmac, event_index)} of the
    packet manifest (3,979 keys), identical across all 4 annotators;
  - per-file: no dup, enum valid, event_index set equal (set equality,
    not just count), full-HMAC join (12-char stems rejected);
  - per-attempt ledger: engine/model/prompt-hash/time/error/result-hash.
- v2 retry policy (FM-01): per-session max 3 attempts, backoff,
  terminal-failure sessions quarantined to an explicit list; a worker
  exits non-zero if any owned session lacks verified output; Phase 1
  completes only when the 192-session success ledger is full.
- v2 blocking gate (F-03): per-annotator uncertain ≤15%. If exceeded:
  STOP, codebook revision, full relabel of affected corpus under the
  revised codebook (no version mixing). (A′ measured 6.5% ✓.)
- v2 coverage diagnostic (METH-08): bounded_next12_coverage restored —
  per session, the FIRST 12 eligible non-union events (deterministic
  order) are blind-labeled by the same protocol; reported as a
  NON-GATING diagnostic with the pooled-candidate optimism caveat.
  The "all non-candidates" coverage file is replaced by this bounded
  extraction.
- **Protocol amendment v5.6 (METH-03, formal — replaces v2's
  "disclosed deviation" wording)**: annotator packets are candidate-only,
  NOT the v5.4 §3 full-forward-transcript contract. This is a RECORDED
  AMENDMENT of the annotation protocol (same class as §12 v5.5), not
  "the same frozen protocol": gold labels are PROTOCOL-RELATIVE to
  candidate-only context. Rationale: pilot corpus (frozen `01cb86f`)
  was produced under this packet design; comparability requires
  keeping it. Mitigation — **context-bias quantification (blocking
  diagnostic)**: after Phase 1, a stratified sample of 8 sessions is
  re-labeled by the SAME 4 annotators with full-session dossier
  context (blind to their candidate-only labels); per-event agreement
  between candidate-only and full-context modes is reported. If
  agreement < 85%, the corpus is flagged CONTEXT-SENSITIVE and the
  flag is carried into every P3 evidence claim. Known systematic
  limitation: all annotators share the same context deficit, so κ and
  3-of-4 cannot detect it — hence the quantification probe.

## Phase 2 — Consensus compile + reliability
- Rule (v5.5, F-04 wording fixed): **founder override > 3-of-4
  cross-family majority > otherwise EXCLUDED fail-closed.** Camps are
  frozen: {A″, B} = GPT family, {A′, C} = non-GPT family. An adopted
  3-of-4 must span both camps (compiler asserts this per event).
- v2 compiler spec (FM-06): deterministic compiler runs only after
  all-4 key-set equality passes; only decisive 4/4 or 3/4 majorities
  adopt — an uncertain-majority is NOT adoptable (excluded, or sent
  to optional founder adjudication); every exclusion carries a
  row-level reason; sessions whose gold positives become 0 are
  excluded from nDCG (IDCG=0) and their count reported.
- v2 reliability axes (METH-05/FM-08 fixed — pilot "A↔B" naming
  retired): pairwise κ among A′, A″, B, C; PRIMARY axis = A′↔B
  (GLM↔GPT, cross-family). C is a FULL VOTING MEMBER of consensus but
  reference-only in the reliability gate (frozen pilot disposition).
  Thresholds unchanged: binary ≥0.6, 5-label ≥0.5, PA ≥0.60.
- v2 κ tooling (METH-07/FM-05): kappa.py hardened — exit non-zero on
  any key-set mismatch (no silent intersection); paired-count
  assertion vs frozen candidate count; stratified (pre-label stratum)
  κ and family-cluster bootstrap CI reported alongside pooled κ.
- Optional minimal founder dossier audit of top-disagreement sessions
  (offered, not required — v5.5 waiver covers skipping).
- v2 exclusion reporting (METH-06) + **v3 frozen exclusion ceiling**:
  report exclusion rates by overall/family/stratum/shortlist-position/
  arm-membership/strict-TF0; compute worst-case bounds (excluded events
  counted all-relevant vs all-non-relevant). **Ceiling (now defined):
  consensus-excluded events must be ≤15% of decisive candidates
  (adopted + excluded), measured both overall and per pre-label
  stratum; denominator and per-stratum numbers are frozen in the Phase
  2 report BEFORE Phase 3 begins.** Exceeding the ceiling, or
  conclusions flipping across the worst-case bounds → P3 = INCONCLUSIVE.

## Phase 3 — m4 prior seeds + universe closure gate (METH-01/02, FM-04, F-01)
- **Target-leakage fix — family-fold cross-fitting (frozen before any
  scoring):** partition the corpus families into K=3 folds
  **(v3 algorithm frozen: families sorted by family_hmac ascending,
  assigned round-robin to folds 0/1/2; fold assignment table hashed
  and recorded BEFORE any seed derivation; fold size imbalance
  disclosed)**. Prior seeds used to score fold i are derived ONLY
  from gold positives of the other folds. Per-fold seed derivation,
  union re-run, and aggregation (pooled per-fold scores, family-
  cluster bootstrap across families, seed 20260820) are frozen
  before the first score is computed. No global-seed scoring of the
  corpus that produced the seeds is admissible as the primary result
  (may be reported separately as an explicitly-labeled in-sample
  diagnostic).
- **Universe closure gate:** after each populated-prior union re-run,
  compare the candidate key set to the Phase-1 frozen universe. New
  candidates (prior-only recoveries) must EITHER be blind-labeled by
  the same 4-annotator protocol and consensus-compiled (incremental,
  before scoring — the PRIMARY path) OR the evaluation is explicitly
  restricted to the pre-label universe. **v3 consequence rule: if the
  restricted-universe path is taken, D3 becomes NOT-MEASURABLE
  (prior-only recovery was not scored), and P3 therefore closes as
  INCONCLUSIVE — the restricted path can never yield P3-GO.** Silent
  post-label universe expansion is prohibited.
- Strict TF=0 candidate recovery measured under the closure gate.

## Phase 4 — Gold scoring + P3-GO judgment (METH-04/FM-07/F-02 fixed)
- **P3-GO = D1 ∧ D2 ∧ D3 ∧ D4 (frozen prep-plan §D, restored):**
  - D1: ΔnDCG@12 (prior − lexical) point estimate > 0 AND
    family-cluster bootstrap (10,000 resamples, seed 20260820) CI
    lower bound > 0;
  - D2: Any-hit@12 point-estimate superiority (prior > lexical);
  - D3: strict TF=0 recovery exceeds BOTH lexical arm AND a
    deterministic-random baseline (frozen seed) on the same shortlist32
    (D3 = NOT-MEASURABLE if the restricted-universe path of Phase 3
    was taken → P3 closes INCONCLUSIVE, never GO);
  - D4: Option B B3–B8 regression gate exit 0 (no regressions).
  - Any D fail → NO-GO; under-power/undefined (e.g. CI unattainable,
    exclusion ceiling breached) → INCONCLUSIVE. Never a soft GO.
- CI width ≤0.05 at n=192 is the Stage-0 v2 founder-approved
  PLAN-LEVEL precision acceptance, reported as achieved-precision
  disclosure — it is NOT the P3-GO criterion (F-07 wording fix).
- Evidence doc 4-layer; claim wording bound: "human-audited
  cross-family consensus gold" (never "human gold").

## Phase 5 — Documentation + commit batch (founder approval, once)
- v2 pre-commit blocking gates (F-06/FM-10): (a) read-back
  verification — final labels/bindings reloaded through the harness,
  manifest hash/schema/dup/unknown-key/uncertain/metric replay
  verified; (b) commit allowlist scanned HMAC-only (no raw IDs,
  transcripts, keys); (c) only after the corpus freeze commit are
  local packets/coverage raw files deleted, with the deletion and the
  remaining local-only artifact list recorded in evidence.
- Evidence, plan changelog v5.6, roadmap §9, labels.jsonl (192-session
  corpus), single commit → push → graphify update.

## Non-claims carried forward
- Empty-prior pilot boundary (prior=lexical) until Phase 3.
- SHORT strata disclosure (v5.4) unchanged.
- C-annotator deviation (pilot κ) applies corpus-wide; C votes in
  consensus, reference-only in reliability.
- Candidate-only packet context = protocol amendment v5.6 (recorded,
  with the Phase-1 context-bias quantification probe; gold labels are
  protocol-relative — see Phase 1).

## Timeline estimate
- P0: DONE. P1: 1-2 days (codex workers serialize; GLM/DS done).
- P2: hours. P3-P4: ~1 day (cross-fitting adds fold reruns).
- Total ~2-3 days wall clock.
