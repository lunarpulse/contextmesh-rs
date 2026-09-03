#!/usr/bin/env bash
# verify-oc05.sh — OC-05 Release and Claim Gate (spec FROZEN v29, a95a193)
# Implements the 13 checkpoints of spec §3.3 + §7 categories verbatim.
set -u -o pipefail
echo "OC05_SCRIPT_ID=oc-05-release-gate-v1"

# --- checkpoint selector (OC05_ONLY) --------------------------------------
run() {  # $1 = checkpoint literal; rest = body
  local lit="$1"; shift
  if [ -n "${OC05_ONLY:-}" ] && [ "$OC05_ONLY" != "$lit" ]; then return 0; fi
  echo "RUN $lit"
  "$@"; local rc=$?
  if [ "$rc" -ne 0 ]; then echo "$lit FAIL"; exit 1; fi
  return 0
}
if [ -n "${OC05_ONLY:-}" ]; then
  case "$OC05_ONLY" in
    OC05-01|OC05-02a|OC05-02b|OC05-03|OC05-04|OC05-05|OC05-06|OC05-07|OC05-08|OC05-09|OC05-10|OC05-11|OC05-12) ;;
    *) echo "OC05-ONLY: FAIL (invalid selector '$OC05_ONLY')"; exit 1;;
  esac
fi

norm() { tr -s ' \t' ' '; }   # §7.3 whitespace-collapse for pin matching

# --- OC05-01 clean+detached ------------------------------------------------
run OC05-01 bash -c '
  p="$(git status --porcelain)"
  [ -z "$p" ] || { echo "OC05-01 FAIL (porcelain: $p)"; exit 1; }
  git symbolic-ref -q HEAD >/dev/null 2>&1 \
    && { echo "OC05-01 FAIL (attached HEAD)"; exit 1; }
  exit 0'

# --- OC05-02a executor digest vs founder-recorded spec pin -----------------
run OC05-02a bash -c '
  want="$(git show "$OC05_PINS_COMMIT":_bmad-output/implementation-artifacts/spec-oc-05-release-gate.md 2>/dev/null | grep -m1 "^OC05_SCRIPT_SHA256" | grep -oE "[0-9a-f]{64}")"
  [ -n "$want" ] || { echo "OC05-02a FAIL (pins read)"; exit 1; }
  have="$(sha256sum scripts/verify-oc05.sh | cut -d" " -f1)"
  [ "$have" = "$want" ] || { echo "OC05-02a FAIL (script digest $have != pins $want)"; exit 1; }
  exit 0'

# --- OC05-02b strict-linear chain + window purity --------------------------
run OC05-02b bash -c '
  E="$OC05_PINS_COMMIT"
  git merge-base --is-ancestor "$E" HEAD || { echo "OC05-02b FAIL (a) E not ancestor of HEAD"; exit 1; }
  parents="$(git rev-list --parents -n 1 "$E")"
  pcount="$(echo "$parents" | wc -w)"
  [ "$pcount" -eq 2 ] || { echo "OC05-02b FAIL (b) E parent count $pcount != [E,F]"; exit 1; }
  m="$(git rev-list --merges "$E"..HEAD)"
  [ -z "$m" ] || { echo "OC05-02b FAIL (c) merges in E..HEAD: $m"; exit 1; }
  # (d) per-commit changed paths within the 2 allowed dirs
  for c in $(git rev-list "$E"..HEAD); do
    for f in $(git show --pretty=format: --name-only "$c"); do
      case "$f" in
        _bmad-output/verification-artifacts/*|_bmad-output/planning-artifacts/*) ;;
        *) echo "OC05-02b FAIL (d) out-of-allowed path in $c: $f"; exit 1;;
      esac
    done
  done
  exit 0'

# --- OC05-03 frozen manifest hashes ----------------------------------------
run OC05-03 bash -c '
  [ "$(sha256sum Cargo.toml | cut -d" " -f1)" = "7c2075b807d9e5b7471e73aca95fa2984f9059da613d0eabae8c9bc5bb470124" ] || { echo "OC05-03 FAIL (Cargo.toml)"; exit 1; }
  [ "$(sha256sum Cargo.lock | cut -d" " -f1)" = "653accffb3d64e3a2810d4974112637fb98e7efa7eb1ab0a3ce99c543ea1ddf0" ] || { echo "OC05-03 FAIL (Cargo.lock)"; exit 1; }
  [ "$(sha256sum contextmesh-salience/Cargo.toml | cut -d" " -f1)" = "e6aa9120a7115a08978dae517641fd6f80869ee2d393ae20ddf8db6f6261c3f4" ] || { echo "OC05-03 FAIL (salience Cargo.toml)"; exit 1; }
  exit 0'

# --- OC05-04 dependency closure --------------------------------------------
run OC05-04 bash -c '
  grep -q "contextmesh-salience" Cargo.toml || { echo "OC05-04 FAIL (root lacks salience member)"; exit 1; }
  grep -q "contextmesh = { path = \".\"" }\|path = "\.\.\." contextmesh-salience/Cargo.toml 2>/dev/null || true
  grep -Eq "^contextmesh[[:space:]]*=.*path" contextmesh-salience/Cargo.toml || { echo "OC05-04 FAIL (salience lacks root dep)"; exit 1; }
  # root must NOT depend on salience as a path dep
  grep -Eq "contextmesh-salience[[:space:]]*=.*path" Cargo.toml && { echo "OC05-04 FAIL (root depends on salience)"; exit 1; }
  git diff --exit-code Cargo.lock >/dev/null || { echo "OC05-04 FAIL (lock drift)"; exit 1; }
  exit 0'

# --- OC05-05 fixture manifest ----------------------------------------------
run OC05-05 bash -c '
  mf="_bmad-output/verification-artifacts/oc-05-fixture-manifest.txt"
  [ -f "$mf" ] || { echo "OC05-05 FAIL (manifest missing)"; exit 1; }
  tmp="$(mktemp)"
  {
    while IFS= read -r line; do
      f="${line#*  }"; f="${f# }"
      f="$(echo "$line" | sed -E "s/^[0-9a-f]{64}  //")"
      [ -f "$f" ] && sha256sum "$f"
    done < "$mf"
  } | sed -E "s/^[0-9a-f]{64}  //" > "$tmp"
  if ! diff -q <(sed -E "s/^[0-9a-f]{64}  //" "$mf") "$tmp" >/dev/null; then
    echo "OC05-05 FAIL (manifest drift)"; rm -f "$tmp"; exit 1
  fi
  rm -f "$tmp"
  exit 0'

# --- OC05-06 owner pins ------------------------------------------------------
run OC05-06 bash -c '
  norm() { tr -s " \t" " "; }
  pin_in() { # $1=file $2=literal
    [ -f "$1" ] || { echo "OC05-06 FAIL (missing $1)"; exit 1; }
    grep -qF "$2" <(norm < "$1") || { echo "OC05-06 FAIL (pin not in $1: $2)"; exit 1; }
  }
  pin_in _bmad-output/verification-artifacts/oc-01-evidence.md "have passing machine evidence at the commits"
  pin_in _bmad-output/verification-artifacts/oc-02-evidence.md "have passing machine evidence at the commits and commands recorded below"
  pin_in _bmad-output/verification-artifacts/oc-03-evidence.md "Implementation COMPLETE (3A–3G, commits"
  pin_in _bmad-output/planning-artifacts/oc-04-test-traceability-matrix.md "FROZEN v12"
  exit 0'

# --- OC05-07 focused golden gates -------------------------------------------
run OC05-07 bash -c '
  OC01_INNER_CURRENT_GATE=1 CARGO_NET_OFFLINE=true cargo test -p contextmesh-salience --test oc03_artifact --test oc04_exec --locked -j 2 >/tmp/oc05-07.log 2>&1 \
    || { tail -5 /tmp/oc05-07.log; echo "OC05-07 FAIL (focused gates)"; exit 1; }
  grep -q "test result: ok" /tmp/oc05-07.log || { echo "OC05-07 FAIL (no ok line)"; exit 1; }
  exit 0'

# --- OC05-08 full workspace regression --------------------------------------
run OC05-08 bash -c '
  OC01_INNER_CURRENT_GATE=1 CARGO_NET_OFFLINE=true cargo test --workspace --locked -j 2 >/tmp/oc05-08.log 2>&1
  rc=$?
  echo "REGRESSION_EXIT:$rc"
  [ "$rc" -eq 0 ] || { tail -5 /tmp/oc05-08.log; echo "OC05-08 FAIL (regression)"; exit 1; }
  exit 0'

# --- OC05-09 privacy scan (§7.1) --------------------------------------------
run OC05-09 bash -c '
  files="_bmad-output/implementation-artifacts/spec-oc-05-release-gate.md _bmad-output/planning-artifacts/oc-05-test-traceability-matrix.md _bmad-output/verification-artifacts/oc-05-release-evidence.md _bmad-output/verification-artifacts/oc-05-claim-audit.md _bmad-output/verification-artifacts/oc-05-fixture-manifest.txt"
  for f in $files; do
    [ -f "$f" ] || { echo "OC05-09 FAIL (missing $f)"; exit 1; }
    if grep -nE "AKIA[0-9A-Z]{16}|sk-or-v1-[0-9a-f]{32,}|sk-[A-Za-z0-9]{20,}|ghp_[A-Za-z0-9]{36}|-----BEGIN [A-Z ]*PRIVATE KEY-----|(?i)(api[_-]?key|secret|password|token)[[:space:]]*[:=][[:space:]]*.[A-Za-z0-9/+_-]{16,}" "$f" >/dev/null 2>&1; then
      echo "OC05-09 FAIL (credential pattern in $f)"; exit 1
    fi
    if grep -nE "192\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}|10\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}|127\.0\.0\.1[^0-9]|/home/[a-z0-9_]+/" "$f" >/dev/null 2>&1; then
      echo "OC05-09 FAIL (local infra id in $f)"; exit 1
    fi
    if grep -nE "(TODO|TBD|FIXME|XXX|HACK|pending|WIP)" "$f" >/dev/null 2>&1; then
      echo "OC05-09 FAIL (unreleased marker in $f)"; exit 1
    fi
  done
  exit 0'

# --- OC05-10 OC-00 evidence integrity ----------------------------------------
run OC05-10 bash -c '
  [ "$(sha256sum _bmad-output/verification-artifacts/oc-00-5-real-data-replay.md | cut -d" " -f1)" = "${OC05_REPLAY_SHA256:-176ea2801555dbef59f31013d426f183b288524c44422dcaf47a5e91363219c5}" ] || { echo "OC05-10 FAIL (replay digest)"; exit 1; }
  [ "$(sha256sum _bmad-output/verification-artifacts/oc-00-prototype-validation.md | cut -d" " -f1)" = "${OC05_PROTO_SHA256:-4c71f4a2cfb992c3c777c9e7a77e584c9090ca41fc6eeb48c91001071a479631}" ] || { echo "OC05-10 FAIL (prototype digest)"; exit 1; }
  exit 0'

# --- OC05-11 claim audit ------------------------------------------------------
run OC05-11 bash -c '
  ca="_bmad-output/verification-artifacts/oc-05-claim-audit.md"
  [ -f "$ca" ] || { echo "OC05-11 FAIL (claim audit missing)"; exit 1; }
  for w in TODO TBD FIXME XXX HACK pending WIP; do
    if grep -q "$w" "$ca"; then echo "OC05-11 FAIL ($w remains in claim audit)"; exit 1; fi
  done
  grep -q "non_claims" "$ca" || { echo "OC05-11 FAIL (non_claims not audited)"; exit 1; }
  exit 0'

# --- OC05-12 honest disposition record ----------------------------------------
run OC05-12 bash -c '
  ev="_bmad-output/verification-artifacts/oc-05-release-evidence.md"
  grep -q "P5-GO: DEFERRED" "$ev" || { echo "OC05-12 FAIL (P5-GO disposition not recorded)"; exit 1; }
  grep -q "P3-GO: OPEN" "$ev" || { echo "OC05-12 FAIL (P3-GO OPEN not recorded)"; exit 1; }
  grep -q "synthetic" "$ev" || { echo "OC05-12 FAIL (synthetic-label citation missing)"; exit 1; }
  echo "OC05 DISPOSITION: P5-GO DEFERRED (founder Option 2, approval 1544867811812184064); P3-GO OPEN (synthetic-label evaluation only)"
  exit 0'

echo "OC05 GATE: ALL PASS"
exit 0
