#!/usr/bin/env bash
# OC-01 Stage 1B heavy historical-chain execution script.
#
# Runs on a capable development machine (NOT inside a memory-limited agent
# worker). Executes the frozen, unchanged OA-07 and OB-13 completion chains in
# detached historical worktrees, plus the current package-scoped checks, then
# writes a machine-verifiable result bundle. The bundle is committed to this
# repository by a human or agent; this script itself never commits.
#
# Usage:  bash scripts/run-oc01-historical-chains.sh <bundle-output-dir>
# Requires: git, bash >=4, python3 >=3.11, cargo 1.97.0 on PATH, ~25 GB disk,
#           >=4 GB RAM headroom, offline cargo registry cache populated.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:?usage: run-oc01-historical-chains.sh <bundle-output-dir>}"
readonly ROOT OUT
readonly OA07_COMMIT="9c275f0f83b320d697dc9ccccc2b51ee60a05114"
readonly OB13_COMMIT="1df53344afc29ac7730e373de1fb4a46def3a9f5"
readonly BASELINE_COMMIT="0cf192b625384283d10c008d4a0e984ae9d0be08"

log() { printf '[oc01-heavy] %s\n' "$*"; }
fail() { printf '[oc01-heavy] FAIL %s\n' "$*" >&2; exit 1; }

mkdir -p "$OUT"
: > "$OUT/bundle.txt"

require_commit() {
  git -C "$ROOT" cat-file -e "$1^{commit}" 2>/dev/null || fail "commit $1 unavailable"
  [[ "$(git -C "$ROOT" rev-parse "$1^{commit}")" == "$1" ]] || fail "commit identity mismatch"
}

# Verifier environment: historical scripts already enforce their own hygiene
# (no RUSTFLAGS/CARGO_TARGET_DIR overrides, pinned 1.97.0 via rust-toolchain.toml).
# We only guarantee offline mode and leave everything else to the frozen script.
run_chain() {
  local commit="$1" verifier="$2" label="$3" rc=0
  local parent wt
  parent="$(mktemp -d)"
  wt="$parent/checkout"
  log "worktree add $label @ $commit"
  git -C "$ROOT" worktree add --detach "$wt" "$commit" >/dev/null
  [[ -z "$(git -C "$wt" status --porcelain)" ]] || { git -C "$ROOT" worktree remove --force "$wt"; git -C "$ROOT" worktree prune; rm -rf "$parent"; fail "$label worktree not clean"; }
  log "running unchanged $verifier (this can take 60-120 minutes)"
  set +e
  ( cd "$wt" && CARGO_NET_OFFLINE=true bash "$verifier" ) >>"$OUT/bundle.txt" 2>&1
  rc=$?
  set -e
  git -C "$ROOT" worktree remove --force "$wt"
  git -C "$ROOT" worktree prune
  rm -rf "$parent"
  if [[ $rc -ne 0 ]]; then
    printf '[oc01-heavy] chain %s exit=%s\n' "$label" "$rc" >> "$OUT/bundle.txt"
    fail "$label chain exited $rc"
  fi
  log "chain $label PASSED"
  printf '[oc01-heavy] chain %s PASS\n' "$label" >> "$OUT/bundle.txt"
}

require_commit "$OA07_COMMIT"
require_commit "$OB13_COMMIT"
require_commit "$BASELINE_COMMIT"

command -v cargo >/dev/null || fail "cargo not on PATH"
[[ "$(cargo --version)" == 'cargo 1.97.0 '* ]] || fail "cargo is not 1.97.0"
command -v python3 >/dev/null || fail "python3 missing"
log "head: $(git -C "$ROOT" rev-parse HEAD)"

run_chain "$OA07_COMMIT" "scripts/verify-oa07.sh" "oa07"
run_chain "$OB13_COMMIT" "scripts/verify-ob13.sh" "ob13"

# Current-tree package-scoped checks (mirrors W11 / verify-oc01.sh workspace stage).
log "current package-scoped checks"
{
  cargo build -p contextmesh --locked &&
  cargo build -p contextmesh-salience --locked &&
  cargo build --workspace --locked &&
  CARGO_NET_OFFLINE=true cargo test -p contextmesh-salience --locked --test oc01_workspace current_workspace_checks_are_package_scoped_and_legacy_scripts_immutable -- --exact &&
  bash "$ROOT/scripts/verify-oc01.sh" --planned-surface-only
} >>"$OUT/bundle.txt" 2>&1 || fail "current package-scoped checks failed"
log "current checks PASSED"

# Machine-verifiable manifest.
python3 - "$ROOT" "$OUT" "$OA07_COMMIT" "$OB13_COMMIT" <<'PY' >> "$OUT/bundle.txt"
import hashlib, subprocess, sys
root, out, oa, ob = sys.argv[1:5]
def sha(p):
    h = hashlib.sha256()
    with open(p, 'rb') as f:
        for chunk in iter(lambda: f.read(1 << 16), b''):
            h.update(chunk)
    return h.hexdigest()
scripts = [f"scripts/verify-oa{n:02d}.sh" for n in range(8)] + \
          ["scripts/verify-oa04-dependencies.sh"] + \
          [f"scripts/verify-ob{n:02d}.sh" for n in range(1, 14)]
print("[oc01-heavy] manifest")
print(f"oa07_commit={oa}")
print(f"ob13_commit={ob}")
print(f"head={subprocess.check_output(['git', '-C', root, 'rev-parse', 'HEAD'], text=True).strip()}")
for s in sorted(scripts):
    print(f"{sha(f'{root}/{s}')} {s}")
print(f"bundle_sha256_pending={sha(f'{out}/bundle.txt')}")
PY

log "DONE — commit the bundle directory: $OUT"
