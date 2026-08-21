# OC-01 Stage 1B Heavy Historical-Chain Delegation Task

**For:** the development machine agent (capable host) · **From:** Hermes @ cosmo-cdp
**Branch:** `OC-AttentionLedger` · **Base HEAD:** `2edbe8d` (Stage 1A) + uncommitted Stage 1B files

## Objective

Execute the frozen OA-07/OB-13 historical completion chains that cannot run
inside the memory-limited agent worker on cosmo-cdp (cgroup OOM at rustc peak).
Your machine previously completed the equivalent OB-13-class chain in ~90 min.

## Required setup

1. `git clone` / fetch this repository and checkout `OC-AttentionLedger` at
   commit `2edbe8d86dea645496dde44db7e3a2e4f6cc404e` **plus** the Stage 1B
   working files from the same push (see the commit that adds
   `scripts/run-oc01-historical-chains.sh`).
2. Toolchain: rustup-managed `rustc/cargo 1.97.0` driven by the repo's
   `rust-toolchain.toml` (the frozen scripts verify and fail otherwise).
   No `RUSTFLAGS`, `CARGO_BUILD_*`, or `CARGO_TARGET_DIR` env overrides.
3. Disk: ≥25 GB free per run. RAM: ≥4 GB headroom. Network: none needed after
   `cargo fetch` once with the repo's `Cargo.lock`.

## Execution

Run exactly:

```bash
bash scripts/run-oc01-historical-chains.sh /tmp/oc01-heavy-bundle
```

The script (already syntax-checked, mode 755):
- verifies the pinned commits `9c275f0` (OA-07) and `1df5334` (OB-13),
- creates a detached temp worktree per chain, requires it clean,
- runs the **unchanged** `scripts/verify-oa07.sh` then `scripts/verify-ob13.sh`
  with only `CARGO_NET_OFFLINE=true` (60–120+ min each; do not interrupt),
- always removes the worktree (RAII-style cleanup on all paths),
- runs current package-scoped checks + `verify-oc01.sh --planned-surface-only`,
- emits `/tmp/oc01-heavy-bundle/bundle.txt` with PASS lines, per-script
  SHA-256 manifest of all 9 OA + 13 OB verifier scripts, and HEAD.

## Deliverable (commit back)

1. Copy the whole bundle directory into the repo:
   `_bmad-output/verification-artifacts/oc-01-heavy-chain-bundle/bundle.txt`
   (keep the exact file name; no other edits).
2. Commit **only** that file on `OC-AttentionLedger` with message:
   `OC-01: record heavy historical-chain evidence bundle`
3. Push to origin `OC-AttentionLedger`. Do not touch any other file, any
   verifier script, `src/`, `tests/`, fixtures, evidence, or the lockfile.

## Constraints

- Never modify `scripts/verify-oa*.sh` / `verify-ob*.sh` (immutability is
  asserted by SHA-256 against the manifest inside the bundle).
- If either chain fails, do NOT commit a PASS bundle; commit nothing and
  report the exact `FAIL ...` line from `bundle.txt` instead.
- No secrets, tokens, or absolute private paths in the bundle.

## Acceptance on our side

cosmo-cdp will re-verify: commit contains only the bundle file; manifest
hashes match local scripts; `PASS` lines for `oa07`, `ob13`, current checks;
then run the W09/W10 gate review against the bundle.
