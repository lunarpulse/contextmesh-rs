# OC-00 Option C Prototype Validation Evidence (gate C5 baseline)

date: 2026-08-20
artifact: `_bmad-output/implementation-artifacts/oc-prototype-validation/`
branch: `OC-AttentionLedger` (draft spec `spec-option-c-salience-provenance-layer.md`)
verdict: PASS — port verified; results recorded below
status: prototype evidence for the draft spec; not a completion claim for C1–C4

## What was validated

The founder-chartered design session (dependency discipline opened for
Option C) produced a Python prototype and a zero-dependency Rust port of
the Attention Ledger mechanisms. Three experiments:

- **E1** attribution ladder vs ground truth (M0 raw overlap, M1n normalized
  nomination, M3 leave-one-out counterfactual, M4 Shapley sampling,
  naive union) — precision/recall/F1 and judge-call economics.
- **E2** cross-session propagation — personalized PageRank over parent +
  entity edges, prior-fused selection vs a reimplementation of
  `ob-baseline-lexical-tf`, on vocabulary-mismatched probes.
- **E3** cost ledger — useful vs wasted (dead ends + noise) effort.

## Environment

| Item | Value |
|---|---|
| python | 3.11.15 (stdlib only, no deps) |
| rustc / cargo | 1.97.0 (2d8144b78 2026-07-07) / 1.97.0 (c980f4866 2026-06-30) |
| crate deps | 0 external (offline by construction) |
| seed | 20260820 |
| corpus | 48 sessions, 399 events, 120-file world |

## Results (Python ground truth = Rust port, all fields)

E1 (48 sessions):

| Mechanism | Precision | Recall | F1 | Judge calls/session |
|---|---|---|---|---|
| M0 raw overlap | 0.661 | 0.863 | 0.749 | 0.0 |
| M1n normalized | 0.693 | 1.000 | 0.819 | 0.0 |
| M3 counterfactual | 1.000 | 0.880 | 0.936 | 7.3 |
| M4 Shapley sampling | 1.000 | 0.995 | 0.997 | 314.5 |
| Naive union (M0∪M1n∪M4) | 0.693 | 1.000 | 0.819 | — |

Reading: cheap mechanisms nominate (M1n recall 1.0 at zero cost) but
cannot prove causality (precision ≤ 0.693); causal verification (M3/M4)
reaches precision 1.0; M3 misses redundant carriers by design, M4 repairs
that (recall 0.995); naive union strictly loses to nominate-verify — the
ladder composition rule is necessary, not stylistic.

E2 (12 probes, budgets 6/12; task wording deliberately mismatched from
event vocabulary):

| Metric | Lexical-TF baseline | Prior-fused |
|---|---|---|
| recall@6 | 0.008 | 0.025 |
| recall@12 | 0.017 | 0.050 |
| precision@12 | 0.014 | 0.042 |

Expected random precision@12: 0.014 — the lexical baseline sits exactly at
random under vocabulary mismatch; prior fusion triples precision at equal
budget. Honest limitation: absolute values remain low (seed dilution from
whole-history PPR seeding); recorded as the C4 improvement target, not
hidden.

E3: useful 78.2s vs dead 17.4s + noise 69.5s — 52.6% of recorded effort
did not contribute to outcomes; the founder's "과정이긴 하지만 결과에 영향을
미치지 않는 정보" intuition quantified.

## Port verification

`compare.py` gate: E1 and E3 exact match; E2 within 0.002 declared
tolerance for set-iteration float summation order (observed: exact). The
Rust crate reproduces CPython's MT19937 (init_by_array seeding,
`_randbelow` rejection sampling, `sample` pool/set branches, `getrandbits`
word assembly, half-even rounding) — unit-tested against captured CPython
draws — and mirrors the generator's call sequence; Python `{:.1f}`/`%.1f`
formatting reproduced via decimal round-tripping.

Observed gate output:

    PORT VERIFIED: E1 exact, E3 exact, E2 within 0.002 (observed: exact)

## Reproduction (from this directory)

    python3 prototype.py          # writes results.json
    cargo run --release --offline # writes results-rust.json
    python3 compare.py            # exits 0 on agreement

E2 was additionally checked stable across Python hash seeds 0/1/2 before
the tolerance was set.

## Non-claims

Prototype-scale synthetic corpus; the simulated recipient judge is
deterministic value coverage, not a model; E2 absolutes are not a
deployment claim; no gate C1–C4 completion is claimed by this evidence.
