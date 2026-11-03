# OC-05 Release Evidence

**Status:** OC-05 release gate executed over FROZEN spec v29 (commit a95a193).
**Script:** `scripts/verify-oc05.sh` (F = e552bcd27bfa6a2312c83c9842e57f43015edfed, SHA-256 6952b516f1c71122b36251aa28d2324a569cc52d62a1deba4936c2ac9f81bb85)
**Date:** 2026-09-03 · **Branch:** `OC-AttentionLedger`

## Gate disposition (§12)

- **P5-GO: DEFERRED** — founder Option 2 (approval messages 1544867811812184064, 1544966446021607536, 1545012358332547162). P5-GO will be recorded only when OC-04 4G + real gold exist. No conditional release record is made.
- **P3-GO: OPEN** — the upstream evaluation gate remains open; OC-05 asserts no evaluation result. The evaluation corpus is synthetic-label only (citation: P1-PREREG-SALIENCE-EVAL-V1 prereg `non_claims` — no causal claim, no C3 prior completion claim, no OC-02 implementation claim, policy-only freeze).

## Execution record

| Stage | Result |
|---|---|
| 5A workspace | PASS — HEAD a95a193, porcelain = 4 regression logs (untracked), no oc05 product code, manifests clean |
| 5B script | F = e552bcd — `scripts/verify-oc05.sh` implements the 13 checkpoints of FROZEN §3.3 |
| 5C evidence | E = (this commit) — spec §5 digest pinned, 4 evidence artifacts tracked |
| 5C gate run | see gate output appended below |
| 5D adversarial | R13–R15 probe results appended below |

## Scope of claims (bounded)

This evidence asserts ONLY: (1) the OC-05 gate script matched its founder-pinned digest; (2) the 13 checkpoints ran over the tracked evidence surface at E; (3) the adversarial probes reproduced the designated failure markers. It does NOT assert model quality, evaluation outcomes, or upstream OC-01..04 correctness beyond what their own frozen evidence records.

## 5D adversarial probe results (final, 2026-09-04)

Probe E = 2c1d4f0ddf5358ad0f0dd8155141792cf28734c6 (corrections 8+9: R13(vi) sibling re-based off E; probe clone moved to disk path). Script: /tmp snapshot of spec §5D block. Result: **PASS=18 FAIL=0, 5D_EXIT=0**.

| Test | Marker asserted | Result |
|---|---|---|
| R13-i committed fixture tamper | OC05-02b FAIL | OK |
| R13-ii tamper + in-window evidence | OC05-02b FAIL | OK |
| R13-iii HEAD gate-script replacement | OC05-02a FAIL | OK |
| R13-iv selected OC05-05 on tamper | OC05-05 FAIL | OK |
| R13-v merge in E..HEAD | OC05-02b FAIL | OK |
| R13-vi sibling off E (pins=S, HEAD=E) | OC05-02b FAIL | OK |
| R13-vii E' substitution | OC05-BIND: FAIL | OK |
| R15-neg selector x3 + bootstrap x3 | designated FAIL markers | OK (6/6) |
| R15-pos OC05-02a / OC05-02b | single-checkpoint PASS | OK (2/2) |
| R14-diff (§7.3 normalized transcripts) | identical | OK |
| R14-porcelain | clean worktree | OK |

Note: R14 full-run rc was blocked in earlier attempts (E8–E12) solely by host disk exhaustion (/tmp tmpfs 3.9GB < probe cargo build ~2.5GB; CARGO_TARGET_DIR workaround broke oa06_demo inner build via demo.sh ROOT/target expectation). Correction #9 relocates the probe clone to disk; R14 then passes with zero failures.

## ADDENDUM 2026-11-03 — post-OC-04 re-pin run (spec v30)

- OC-04 closed: **P3 verdict INCONCLUSIVE, final** — attempt #1 commit
  `d554a2c` (kappa A'–B 0.419/0.475, exclusion 27.12% > 15%); attempt #2
  commit `3212d08` (protocol v6.1+r2, one annotator per family: kappa A6–B6
  binary 0.536 < 0.60; B6–C6 0.389/0.386). Structural cross-family
  divergence independently confirmed twice; a third labeling attempt is
  pre-registered as out of scope. The freeze-era disposition lines above
  (`P5-GO: DEFERRED`, `P3-GO: OPEN`, synthetic-label corpus citation) remain
  the recorded Option-2 record; P3-GO was never granted and is now moot.
- P4 (Thorn) closed as recorded evidence only — `p4-thorn-closure.md` in
  the planning-artifacts directory; P4-GO NOT-DETERMINED because the
  human-audited incremental-benefit gold prerequisite does not exist.
- This gate run (v30 re-pin, F `cf432df`) re-proves evidence integrity at
  the current HEAD: manifests re-frozen to the legitimate values introduced
  by the founder-approved OC-04 commit `013524d`; no other freeze changed;
  §10 wrapper byte-identical in procedure.
