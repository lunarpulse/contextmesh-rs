#!/usr/bin/env bash
# OB-13 Option B end-to-end demo (phase success signal).
#
# Runs the demo binary against a fresh runtime root, asserts the phase
# success signal transcript (public-ID/count-only, no secret material), and
# verifies the runtime root is cleaned. Requires bash >= 4.4, GNU coreutils
# (mktemp), and cargo (offline).
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT"
export CARGO_NET_OFFLINE=true
fail() { printf 'demo-ob: FAIL %s\n' "$*" >&2; exit 1; }

# Build the demo binary offline.
cargo build --locked --bin demo_ob

# Run against a fresh runtime root.
RT="$(mktemp -d -t contextmesh-ob13-demo.XXXXXX)"
trap 'rm -rf "$RT"' EXIT
TRANSCRIPT="$(OB13_DEMO_RUNTIME_ROOT="$RT" ./target/debug/demo_ob)"
status=$?
[[ $status -eq 0 ]] || fail "demo exited with status $status"

printf '%s\n' "$TRANSCRIPT"

grep -q 'phase success signal: PASS' <<<"$TRANSCRIPT" || fail "phase success signal missing"
grep -q 'completed: false' <<<"$TRANSCRIPT" || fail "withheld context did not fail"
grep -q 'completed: true' <<<"$TRANSCRIPT" || fail "repaired context did not pass"
grep -q 'repair converged: true' <<<"$TRANSCRIPT" || fail "repair loop did not converge"
if grep -qE 'token1_|secret|private key|ed25519_|ctx1_' <<<"$TRANSCRIPT"; then
  fail "transcript leaks secret or private material"
fi
# The demo cleans its own artifacts: the runtime root is empty afterwards.
[[ -z "$(ls -A "$RT" 2>/dev/null || true)" ]] || fail "runtime root was not cleaned"
printf '%s\n' 'demo-ob: phase success signal PASS'
