#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly OA05_BASELINE="6347a10"
cd "$ROOT"

required=(
  scripts/demo.sh scripts/verify-oa06.sh tests/oa06_demo.rs README.md
  _bmad-output/implementation-artifacts/spec-oa-06-reproducible-demo.md
)
for file in "${required[@]}"; do
  [[ -e "$file" ]] || { printf 'missing OA-06 artifact: %s\n' "$file" >&2; exit 1; }
done
for file in scripts/demo.sh scripts/verify-oa06.sh; do
  [[ -x "$file" ]] || { printf '%s\n' "$file is not executable" >&2; exit 1; }
done
printf '%s\n' 'ok: OA-06 artifacts exist'

# OA-06 adds no dependency, feature, or manifest byte.
git diff --exit-code "$OA05_BASELINE" -- Cargo.toml Cargo.lock
printf '%s\n' 'ok: Cargo.toml and Cargo.lock unchanged since the OA-05 baseline'

# The OA-00 failure sentinel is gone and the real harness is present.
if grep -q 'OA-06 pending: the two-node Option A demo' scripts/demo.sh README.md; then
  printf '%s\n' 'the OA-06 failure sentinel is still referenced' >&2
  exit 1
fi
grep -q 'OA-06 reproducible two-node demo' scripts/demo.sh
printf '%s\n' 'ok: the demo sentinel was replaced by the real two-node harness'

# README documentation requirements are present.
grep -q '## Reproducible two-node demo' README.md
grep -q '## Network deployment guidance' README.md
grep -q '## Claims, non-claims, and prohibited statements' README.md
grep -q 'verify-oa06.sh' README.md
printf '%s\n' 'ok: README documents the demo, deployment guidance, and claims'

cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
printf '%s\n' 'ok: locked build, format, strict Clippy, and all workspace tests passed'

cargo test --locked --test oa06_demo
cargo test --locked --test oa06_demo -- --ignored fresh_checkout_demo_passes
printf '%s\n' 'ok: OA-06 demo matrix and independent fresh-checkout execution passed'

demo_err="$(mktemp)"
demo_output="$(bash scripts/demo.sh 2>"$demo_err")"
[[ "$demo_output" == *"demo: PASS"* ]] || {
  printf '%s\n' 'demo completed without a PASS summary' >&2
  exit 1
}
if grep -q 'token1_' "$demo_err" <<<"$demo_output"; then
  printf '%s\n' 'token prefix leaked from the demo' >&2
  rm -f "$demo_err"
  exit 1
fi
rm -f "$demo_err"
printf '%s\n' 'ok: two-node demo passed all seventeen stages with a secret-free summary'

[[ -z "$(git status --porcelain)" ]] || {
  printf '%s\n' 'worktree is not clean' >&2
  exit 1
}
printf '%s\n' 'ok: worktree is clean after the demo and tests'

bash scripts/verify-oa00.sh
bash scripts/verify-oa01.sh
bash scripts/verify-oa02.sh
bash scripts/verify-oa03.sh
bash scripts/verify-oa04.sh
bash scripts/verify-oa04-dependencies.sh
bash scripts/verify-oa05.sh
printf '%s\n' 'ok: OA-00 through OA-05 verifiers and the D-04-01 probe verifier passed'

printf '%s\n' 'verify-oa06: all checkpoints passed'
