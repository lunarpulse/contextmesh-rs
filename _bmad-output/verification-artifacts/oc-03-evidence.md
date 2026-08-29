# OC-03 Salience Prior — Implementation Evidence

**Status**: Implementation COMPLETE (3A–3G, commits `5da489a..46a18f5`).
**Evaluation gate**: OPEN — C3 (memory propagation benefit) is explicitly
NOT asserted here; only the structural paths (T/G/P/A/X rows) are.
**Date**: 2026-08-29 · **Branch**: `OC-AttentionLedger` · **HEAD**: `46a18f5`

---

## 1. Sources

| Kind | Artifact | Verbatim pointer |
|---|---|---|
| Frozen spec | `_bmad-output/implementation-artifacts/spec-oc-03-prior.md` | commit `5da489a` (freeze), clarified `83cd7af` (§7.1 M1 spellings), `666731c` (§7.6 range wording), `ea3b27b` (§8 signature) — all logged in §17 |
| Test matrix | `_bmad-output/planning-artifacts/oc-03-test-traceability-matrix.md` | 52 rows: T8 + G12 + P14 + A10 + X8 |
| Prereg config | `_bmad-output/implementation-artifacts/p1-prereg-config.json` | SHA-256 `be20d8fc…eae784c9` consumed verbatim (never redefined) |
| Commit chain | `5da489a` → `43c8813` → `83cd7af` → `5d176f8` → `666731c` → `86a9b43` → `ea3b27b` → `46a18f5` | every commit pushed; remote SHAs verified equal |
| Golden fixture | `contextmesh-salience/tests/fixtures/oc03-prior-v1-golden.{json,sha256}` | sha256 `79944c5a74c153621594d35079fae5835923d03a88d2ce467a0e015b60a9015c`, sidecar byte-matched |
| Discord approvals | freeze `5da489a`; 3B `43c8813` (id `1543029704192434197`); §7.1 Plan A `83cd7af` (id `1543059606077575190`); 3C `5d176f8` (id `1543127882098806805`); 3D `666731c` (id `1543138539162697728`); 3E `86a9b43` (id `1543162623493808139`, incl. §7.6 wording Plan A); 3F `ea3b27b` (id `1543182032371318784`); 3G `46a18f5` (id `1543194129637576767`) | one-word "승인" per protocol |

## 2. Reasoning (gate justifications)

| Gate | Evidence |
|---|---|
| Spec dual review | Compliance GO (4 warnings fixed pre-freeze); Quality by parent (1 blocker: residual dual definition → single exact identity, independently brute-forced 200k cases + 300-graph Python simulation); §7.6 math independently verified CORRECT |
| Preflight (founder-requested) | 5 angles; 2 findings fixed (§7.3 CausalStatus wire path; m₀ = teleport frozen) |
| 3A workspace | 0 OC-03 references in prior code; no dependency drift |
| 3B schema | JCS ordering blocker (epsilon/entities) caught by dual review, fixed 3 sites + discriminating T05 11-mutation loop |
| 3C graph | sort-then-truncate blocker ("z1…z8 a1" counterexample) — fixed with discriminating test; deferred items (source_report_ids wording, new_for_test, G12 tiebreak) tracked → 3F/3H |
| 3D seeds | clamp-after-add equivalence affirmed by reviewer; doc overclaim fixed |
| 3E PPR | **two real blockers**: "impossible by construction" falsified by reviewer's exact hub simulation (4.17e9 > 1e9) — spec §7.6 corrected via §17 (normative Err unchanged); P05 dead branch redesigned as honest cap contract |
| 3F artifact | **exploit-grade blocker**: reviewer empirically broke hash-only verify with a self-consistent forgery + ghost entity → verify_prior rewritten to §8 rebuild-based byte-equality design; golden regenerated via real pipeline; re-check GO (2 warnings fixed immediately: precise re-seal, empty-inputs pin test) |
| 3G adversarial | Compliance GO (2 cell-wording fixes); Quality by parent after API timeout (X01/X06 non-vacuous proven by construction; X08 re-seal exact-match) |

Rejected alternatives (recorded): hash-only verification (broken by
exploit); cap-before-sort capture (violates §7.1); float PPR (violates
§7.6 integer-only); silent clamping of >1e9 vector entries (violates
fail-closed).

## 3. Conclusion (dependency-ordered derivation)

1. Matrix 52 rows ↔ 52 tests 1:1 (T8 in `oc03_schema`, G12+1 helper in
   `oc03_graph_seeds` [13 tests — the helper is a fixture-invariant test
   outside the G-rows by design], P14 in `oc03_propagation`, A10+1 in
   `oc03_artifact` [empty-inputs pin + ignored golden generator], X8 in
   `oc03_adversarial`).
2. All focused suites GREEN; full workspace regression **425 passed /
   0 failed, EXIT 0**; clippy `-D warnings` **0**; fmt clean.
3. §8 signature divergence: spec now matches implementation (rebuild from
   sessions/reports/event_payloads; corrected at `ea3b27b` with §17 log).
   Remaining known divergence: NONE open. Historical divergences
   (derive_entity_keys/derive_seeds signatures vs §8 sketch) were
   reconciled in-spec during 3F; no §8 text now contradicts code.
4. Therefore implementation of OC-03 (prior derivation pipeline) is
   COMPLETE. **C3 memory-propagation benefit remains an open evaluation
   claim — not asserted.**

## 4. Invalidators

1. Any focused suite or full regression failure on `46a18f5`.
2. Golden fixture bytes change without change-control regeneration
   (#[ignore] generator is the only legal path).
3. §7.6 recurrence altered (residual identity is load-bearing).
4. A forgery class passing `verify_prior` rebuild comparison.
5. Matrix row ↔ test name 1:1 breakage.
6. Spec §17 change-control log deleted or unaudited edits to frozen
   constants/caps/wire members.
7. Prereg SHA-256 mismatch (`be20d8fc…eae784c9`).

## 5. Reviewer commands

```bash
cd /home/cosmo/contextmesh-rs && source ~/.cargo/env
OC01_INNER_CURRENT_GATE=1 CARGO_NET_OFFLINE=true cargo test -p contextmesh-salience --locked   # 198 tests
OC01_INNER_CURRENT_GATE=1 CARGO_NET_OFFLINE=true cargo test --workspace --locked               # 425 passed
cargo clippy --workspace --all-targets -- -D warnings                                          # 0 errors
cargo fmt --check                                                                               # clean
sha256sum contextmesh-salience/tests/fixtures/oc03-prior-v1-golden.json                        # 79944c5a…015c
```

## 6. Claims / Non-claims

**Claimed**: deterministic, replayable salience-prior derivation
(OC-02 reports → seeds → bounded graph → integer PPR → verified artifact)
with rebuild-based verification and adversarial boundary coverage, per
frozen spec at `46a18f5`.

**Explicitly NOT claimed**: C3 benefit (does the prior actually improve
cross-session outcomes) — owned by the OC-05 evaluation gate; any
real-corpus performance; Thorn behavior (P4 scope, absent here).
