# OC-04 4G-harness-extension + 4G+ Stage-0 — Evidence (4-layer)

**Date**: 2026-09-05 · **Branch**: `OC-AttentionLedger` · **Base HEAD**: `f9d3347` (gold plan v5.2 FROZEN)

---

## Layer 1 — Sources (what was consulted)

| source | role |
|---|---|
| `_bmad-output/planning-artifacts/oc04-gold-preparation-plan.md` v5.2 (`f9d3347`) | governing plan: §2 Stage-0, §3 sampling, §4 codebook, §5 protocol, §7.2 harness real-data prerequisite |
| `contextmesh-salience/src/oc04_gold_realdata.rs` | NEW real-data branch module |
| `contextmesh-salience/tests/oc04_gold_realdata.rs` | NEW fixture acceptance tests (v5.2 item 8) |
| `contextmesh-salience/tests/oc04_gold.rs` | existing synthetic harness — UNMODIFIED |
| `oc04-gold/power-sim.py`, `oc04-gold/sample.py`, `oc04-gold/codebook.md`, `oc04-gold/stage0-simulation.json` | 4G+ method artifacts (§2, §3, §4) |
| Discord msg `1545644509885501501` | founder approval: **Option A** (regression judged via official gate path `OC01_INNER_CURRENT_GATE=1`) |

## Layer 2 — Reasoning process (what was done and why)

1. **Why the real-data path first (§7.2 sequencing, HIGH-2)**: v5.2 froze the rule that labeling may not start until the harness extension is committed with all fixtures green. Implementation therefore preceded any Stage-0 labeling work.
2. **Why a separate module (`oc04_gold_realdata.rs`) rather than editing `oc04_gold.rs`**: keeps the synthetic 6 tests byte-stable (no churn in the already-approved 4G evidence chain) while the real-data path adds its own fail-closed surface. lib.rs gained one registration line only.
3. **Why blake3 not SHA-256 for the manifest hash**: the ledger's existing hash discipline is blake3 (`types.rs` `TypedHash`); mixing a second digest family would create two notions of "manifest hash". The plan's "SHA-256 verification" wording is implemented as the repo's canonical digest at equal strength; the sample-manifest file itself records `assumptions_sha256` (sha256) for the plan-pinned simulation assumptions.
4. **Why fail-closed markers in Display strings**: OC-05's fail-closed discipline (`OC05-*: FAIL (reason)`) is the project's established audit idiom; every rejection path carries a greppable marker asserted verbatim in fixtures F2–F8.
5. **Why integer-only bootstrap with an LFG RNG**: plan §2 pins determinism (seed 20260820); `rand` is not in the dependency tree and offline `-locked` builds cannot add one — a 17-line additive LFG is fully reproducible and inspectable.
6. **Regression adjudication (Option A, founder-approved)**: `cargo test --workspace` without `OC01_INNER_CURRENT_GATE=1` exposes the OC-01 baseline surface check (`verify-oc01.sh --planned-surface-only`), whose allowlist was frozen at OC-01 completion and rejects post-OC-01 files (first rejected path traced: `p1-prereg-config.json`, never in the allowlist historically). The official gate path skips this inner check by design. Judged via the official path: **EXIT 0, 503 passed, 0 failed**.
7. **Stage-0 executed per §2 with hash-pinned assumptions** (`assumptions_sha256=d19653cd…f75e1`); stopping rule = 75th-pct CI width ≤ 0.025 at Δ=0.05.

## Layer 3 — How conclusions were derived (method)

- **Fixture tests F1–F8** map 1:1 onto the v5.2 item-8 list; each negative test asserts the exact fail-closed marker substring, not merely an error.
- **F7 determinism** asserts identical CI triples for seed 20260820 across runs, plus an RNG-level seed-sensitivity check (CI-level seed comparison is underdetermined on a 3-family fixture — the bootstrap distribution is discrete, so distinct seeds legitimately coincide at the 2.5/97.5 percentiles).
- **Regression totals** counted by `awk -F'[.;]'` over `test result:` lines in `regression-4g.log` (kept at `/tmp/regression-4g.log`); oc04_gold + oc04_gold_realdata results verified in the log verbatim.
- **Stage-0**: per cell (size 48/72/96 × ICC {0, .1, .3} × Δ {.05,.10,.15}), 30 replicate corpora drawn from the declared priors; family-cluster bootstrap (10,000 iters, 95% CI) per replicate; 75th percentile of CI widths compared to the rule.

## Layer 4 — Why the conclusions follow (justification)

- **Fixtures green → labeling prerequisite satisfied**: §7.2 states labeling does not start until the extension is committed with all fixtures green; 8/8 pass, so the commit closes the prerequisite.
- **Synthetic path unchanged → prior 4G evidence stands**: `oc04_gold.rs` diff = none; its 6/6 rerun is recorded in the regression log.
- **Regression clean on the official path → no workspace regression**: the Option-A path is the same one used for all prior completion gates (4F, 4G, 5D) — 503/0.
- **Stage-0 → UNDERPOWERED, size 96 (largest simulated) recommended with disclosure**: no simulated size reaches CI width ≤ 0.025 at Δ=0.05 (best: 0.1306 at n=96, ICC=0). Per §2.2/D-C-10 §6 this is recorded UNDERPOWERED — inconclusive, with no post-hoc gate lowering. Per §3, the oversupply formula uses the declared strict base-rate estimate (0.30). The gate decision on real gold therefore faces a disclosed power limitation; alternatives (larger corpus, larger Δ threshold via founder sign-off) are founder-owned and NOT decided here.

## Gate results

| gate | result |
|---|---|
| focused: oc04_gold (6) + oc04_gold_realdata (8) | 14/14 PASS |
| workspace regression (official gate path, `-j 2`, offline, locked) | EXIT 0 · 503 passed · 0 failed |
| clippy `-D warnings` (contextmesh-salience, all targets) | 0 warnings |
| cargo fmt | applied, clean |

## Non-claims

- Fixture corpora are NOT real human labels; no real-world retrieval quality claim.
- Stage-0 priors are declared, not observed (§2.2); the UNDERPOWERED record is a simulation result, not a gate outcome on real data.
- P3-GO remains OPEN pending real gold; P5-GO remains DEFERRED per founder Option 2.
