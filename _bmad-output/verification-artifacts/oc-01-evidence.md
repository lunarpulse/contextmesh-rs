# OC-01 Stage 1B Historical-Chain Acceptance Evidence (Interim)

**Status:** Stage 1A (workspace boundary) and Stage 1B (heavy historical chains)
accepted. Implementation Stages 2A–2E (primitives, schema, protocol, DAG, I/O,
vectors) and the full 8-stage gate are **pending**. This is the implementation-time
four-layer record for the Stage 1B acceptance only; it asserts nothing beyond the
artifact integrity and provenance observations listed below.

## 1. Sources

| Item | Value | Authority |
|---|---|---|
| Baseline (spec/matrix freeze) | `0cf192b625384283d10c008d4a0e984ae9d0be08` | git object |
| Stage 1A completion | `2edbe8d86dea645496dde44db7e3a2e4f6cc404e` | git object |
| Delegation kit | `5ef6be91246e83cad754a2639cd61380accf7dce` | git object |
| Allowlist fix | `59fd165bc850187d3983b4ccda7ddb90f180e81e` | git object |
| Evidence bundle commit | `9d363bd6a10ef29aa1a8f7af0d9e532d7ba625ab` | git object |
| OA-07 historical completion | `9c275f0f83b320d697dc9ccccc2b51ee60a05114` | git object |
| OB-13 historical completion | `1df53344afc29ac7730e373de1fb4a46def3a9f5` | git object |
| Bundle | `_bmad-output/verification-artifacts/oc-01-heavy-chain-bundle/bundle.txt` (13,043 lines) | committed at `9d363bd` |
| Bundle final SHA-256 | `08edeb93271d01cab3dee9d45d6f7d9295ee10c2fc196b3858d636353bcf6836` | reviewer-recomputed |
| Bundle pre-manifest SHA-256 | `77f9d9ceae49168d0cc051689c4fa05e4f6fac17fc7c20e3c636c6787bb7c8c6` (recorded as `bundle_sha256_pending` at bundle line 13043; the hash covers lines 1–13016) | reviewer-recomputed over lines 1–13016 |
| Toolchain | rustc/cargo 1.97.0 pinned by the repo's `rust-toolchain.toml`; run performed offline per the kit contract | bundle line 2 (`ok: pinned toolchain active, no env overrides, native cc present`) + `rust-toolchain.toml` and the manifest-verified runner script |
| Execution host | delegation host (capable machine); acceptance review on the cosmo-cdp agent host | — |
| Operator approvals | bundle submission Discord `1541063227675508817`; evidence-write instruction Discord `1541064884693893161`; matrix approval Discord `1540352346364842105` | Discord records |

Reviewer commands (agent host, offline): `sha256sum` over the bundle and each
`scripts/verify-o*.sh`; `git show <commit>:<path> | sha256sum`; `git diff --stat
59fd165..9d363bd`; `bash scripts/verify-oc01.sh --self-test`; `git grep -E
'token1_[A-Za-z0-9_-]{43}'`. Git blob hashes are cryptographic; bundle interior
lines are operator-supplied run output bound to the reviewer-recomputed bundle
hash above.

## 2. Reasoning

- **Why delegation:** the OA-07/OB-13 chains each require a detached-worktree
  full build (60–120+ min, multi-GB rustc peak). The agent host previously hit
  memory-cgroup OOM at rustc peak, so the frozen chains were executed on the
  delegation host via the committed kit (`5ef6be9`), exactly as designed.
- **Why acceptance by bundle instead of re-running the strict gate at HEAD:**
  `verify-oc01.sh::planned_surface_only` freezes `_bmad-output/verification-artifacts`
  against baseline `0cf192b`. Once the bundle (and later this file) is committed,
  `git diff <baseline> -- _bmad-output/verification-artifacts` is non-empty by
  construction, so the strict gate is not re-runnable at such HEADs. This is the
  accepted convention: the gate runs pre-commit on the working tree; afterwards
  acceptance rests on recorded hashes plus PASS lines inside the hash-bound
  bundle. Re-running the 90+ minute chains on the OOM-limited agent host would
  add no information — script identity is already proven cryptographically
  (manifest hashes and git blob equality against the historical commits).
- **Why the allowlist fix (`59fd165`) is legitimate:** the kit commit `5ef6be9`
  added `scripts/run-oc01-historical-chains.sh` and
  `_bmad-output/implementation-artifacts/oc01-heavy-delegation-task.md` but
  omitted both from its own allowlist, making `--planned-surface-only`
  deterministically unpassable at that HEAD (confirmed by two full runs whose
  chains passed 2-for-2 while current checks failed on
  `path is outside the approved OC-01 surface`). The fix adds exactly those two
  paths to `allowed_path()` inside `verify-oc01.sh`, which is itself on the
  allowlist; no frozen surface changed — all 22 OA/OB verifier scripts remain
  byte-identical (manifest-verified below and cross-checked against the
  historical commits via git blobs).
- **Alternatives rejected:** weakening the baseline freeze (forbidden by W13 and
  change control); treating runs #1/#2 partial bundles as acceptance (chains
  passed but current checks failed — not a clean single-run acceptance);
  re-running chains on the agent host (OOM risk, redundant with hash identity).

## 3. Conclusion derivation

| Requirement | Observation (exact) | Conclusion (bounded) |
|---|---|---|
| OC01-W09 (OA chain at `9c275f0`) | bundle L492 `[oc01-heavy] chain oa07 PASS`; `scripts/verify-oa07.sh` SHA-256 `d3cda00…f87d` equals both bundle manifest entry and the git blob at `9c275f0` | The unchanged OA-07 completion chain passed offline in a detached clean worktree on the delegation host |
| OC01-W10 (OB chain at `1df5334`) | bundle L12739 `[oc01-heavy] chain ob13 PASS`; `scripts/verify-ob13.sh` SHA-256 `8de64ab…eae1` equals both manifest entry and the git blob at `1df5334` | The unchanged OB-13 completion chain passed offline in a detached clean worktree on the delegation host |
| OC01-W11 (package-scoped current checks) | bundle tail `ok: planned_surface_only`; `current_workspace_checks_are_package_scoped_and_legacy_scripts_immutable … ok`; bundle head `ok:` lines for clean tree, pinned toolchain, exact pins/closure 320, OA-00..06 checkpoints (155), rustfmt | Current-tree checks passed package-scoped at run HEAD `59fd165` without invoking obsolete historical HEAD assertions |
| OC01-W12 (stage order) | `bash scripts/verify-oc01.sh --self-test` → `ok: stages_execute_in_dependency_order` (reviewer-executed on the agent host) | The 8 stages execute fail-fast in dependency order and stop after an injected failure |
| OC01-W13 (planned boundary) | manifest re-hash on the agent host: 22/22 identical, 0 mismatch; `git diff --stat 59fd165..9d363bd` touches only `bundle.txt`; runner `run-oc01-historical-chains.sh` SHA-256 equals the `5ef6be9` blob (`3756ec9…a1f`) | The bundle commit changed no frozen surface; the committed boundary matches the approved OC-01 allowlist |
| OC01-R08 (privacy, partial) | reviewer `git grep` token-pattern scan clean over the repo; bundle contains zero FAIL verdicts, zero panic events, and zero `^error` lines (the substring `panic` appears only inside passing test names) | No secret-token pattern or failure verdict is present in the recorded artifacts reviewed |
| OC01-R09 (claim limits) | conclusions above are limited to chain execution, script identity, and boundary integrity | No C2–C5, OC-02, completion, cost, or causal-attribution claim is made |

## 4. Invalidators

Any of the following would reverse or narrow this acceptance:

- Any `scripts/verify-oa*.sh` / `verify-ob*.sh` byte differs from the bundle
  manifest hash or from its historical completion-commit blob (dependency/script drift).
- A FAIL verdict, panic event, or `^error` line inside the bundle, or `bundle_sha256_pending`
  no longer equal to the SHA-256 of bundle lines 1–13016 (content tampering).
- `Cargo.lock`/workspace identity drift changing the closure (320) or adding a
  registry identity (re-audit via `scripts/check-core-dependencies.py`).
- A later re-run of either chain on any capable host failing at the same commits.
- Any claim drawn from this file beyond Stage 1A/1B artifact integrity
  (overreach invalidates the evidence standard itself).
- Structural note (not an invalidator): at HEADs ≥ `9d363bd` the strict
  `--planned-surface-only` check fails on the artifacts-directory baseline freeze
  by design; acceptance is the recorded hashes and PASS lines above.

## 5. Deviations and operator declarations (caller-supplied)

The following were declared by the operator and are recorded as declarations,
not independently re-verified by the reviewer:

- Free disk during run #3 was ~20 GB versus the kit's stated ≥25 GB; both prior
  runs' chains passed at similar levels with no disk-related anomalies.
- Runs #1 (2026-08-22) and #2 (2026-08-23) produced partial bundles (chains
  passed 2-for-2 in each; current checks failed on the pre-fix allowlist);
  partial outputs are preserved on the delegation host.
- Runner argument-handling quirk: a stray space silently redirects the bundle
  output path (run #1 wrote to a temporary path). Hardening (quoted `mkdir -p`,
  `set -u`, argument-count assertion) is recommended for any future kit; the
  frozen runner itself is manifest-verified and was not modified.
- The allowlist fix `59fd165` was applied on the delegation host per operator
  decision, then produced the accepted run #3.

## 6. Non-claims

OC-01 is not complete: Stages 2A–2E (primitives, schema, protocol, DAG, I/O,
vectors) are unimplemented and their gate rows are unexecuted. This record does
not establish C2, C3, C4, C5, or any part of Option C beyond C1's Stage 1A/1B
artifact-integrity portion. OC-02 remains blocked until the full OC-01 gate and
the separate P1 preregistration-hash gate both pass (spec §15).
