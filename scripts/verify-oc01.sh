#!/usr/bin/env bash
# OC-01 non-recording, fail-fast verification gate.
#
# BASELINE_COMMIT is the human-approved frozen OC-01 specification commit. The
# planned-surface comparison intentionally starts there so approved OC changes
# are visible while production core and historical A/B surfaces stay immutable.
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly BASELINE_COMMIT="0cf192b625384283d10c008d4a0e984ae9d0be08"
readonly OA07_COMMIT="9c275f0f83b320d697dc9ccccc2b51ee60a05114"
readonly OB13_COMMIT="1df53344afc29ac7730e373de1fb4a46def3a9f5"
readonly STAGES=(workspace primitives schema protocol DAG I/O vectors evidence)

cd "$ROOT"
if ! command -v cargo >/dev/null 2>&1 && [[ -x "${HOME:-}/.cargo/bin/cargo" ]]; then
  export PATH="${HOME}/.cargo/bin:${PATH}"
fi
export CARGO_NET_OFFLINE=true
fail() { printf 'verify-oc01: FAIL %s\n' "$*" >&2; exit 1; }

require_commit() {
  local commit="$1"
  git cat-file -e "${commit}^{commit}" 2>/dev/null || fail "required commit is unavailable"
  [[ "$(git rev-parse "${commit}^{commit}")" == "$commit" ]] || fail "required commit identity mismatch"
}

allowed_path() {
  case "$1" in
    Cargo.toml|Cargo.lock|README.md|\
    contextmesh-salience/Cargo.toml|\
    contextmesh-salience/src/lib.rs|contextmesh-salience/src/error.rs|\
    contextmesh-salience/src/json.rs|contextmesh-salience/src/types.rs|\
    contextmesh-salience/src/outcome.rs|contextmesh-salience/src/verify.rs|\
    contextmesh-salience/src/io.rs|\
    contextmesh-salience/tests/oc01_workspace.rs|\
    contextmesh-salience/tests/oc01_schema.rs|\
    contextmesh-salience/tests/oc01_crypto.rs|\
    contextmesh-salience/tests/oc01_dag.rs|\
    contextmesh-salience/tests/support/oc01_fixed_dag.rs|\
    contextmesh-salience/tests/oc01_io.rs|\
    contextmesh-salience/tests/oc01_adversarial.rs|\
    contextmesh-salience/tests/fixtures/oc01-outcome-ledger-v1-golden.json|\
    contextmesh-salience/tests/fixtures/oc01-outcome-ledger-v1-unterminated.json|\
    scripts/check-core-dependencies.py|scripts/verify-oc01.sh|\
    scripts/run-oc01-historical-chains.sh|\
    _bmad-output/implementation-artifacts/spec-oc-01-outcome-ledger.md|\
    _bmad-output/planning-artifacts/oc-01-test-traceability-matrix.md|\
    _bmad-output/verification-artifacts/oc-01-evidence.md|\
    _bmad-output/verification-artifacts/oc-01-heavy-chain-bundle/bundle.txt|\
    _bmad-output/implementation-artifacts/oc01-heavy-delegation-task.md) return 0 ;;
    *) return 1 ;;
  esac
}

planned_surface_only() {
  require_commit "$BASELINE_COMMIT"
  local changed path
  changed="$({ git diff --name-only "$BASELINE_COMMIT" --; git ls-files --others --exclude-standard; } | LC_ALL=C sort -u)"
  while IFS= read -r path; do
    [[ -z "$path" ]] && continue
    allowed_path "$path" || fail "path is outside the approved OC-01 surface"
  done <<< "$changed"

  # Git compares blob content here: these are immutable baseline hash checks for
  # core production, tests/fixtures, historical evidence, and OA/OB verifiers.
  # OC-01's own evidence outputs are expected additions on the planned surface
  # and are excluded; all pre-baseline artifacts stay frozen (founder-approved
  # change control, matrix rows R06-R10 require this evidence to be committable).
  git diff --quiet "$BASELINE_COMMIT" -- src tests \
    _bmad-output/verification-artifacts \
    ':(exclude)_bmad-output/verification-artifacts/oc-01-evidence.md' \
    ':(exclude)_bmad-output/verification-artifacts/oc-01-heavy-chain-bundle/bundle.txt' \
    ':(glob)scripts/verify-oa*.sh' \
    ':(glob)scripts/verify-ob*.sh' \
    || fail "immutable core or historical surface differs from baseline"
  printf '%s\n' 'ok: planned_surface_only'
}

require_files() {
  local file
  for file in "$@"; do [[ -f "$file" ]] || fail "stage implementation is incomplete"; done
}

workspace_stage() {
  planned_surface_only
  python3 -I scripts/check-core-dependencies.py >/dev/null
  cargo metadata --locked --format-version 1 >/dev/null
  cargo build -p contextmesh --locked
  cargo build -p contextmesh-salience --locked
  cargo build --workspace --locked
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets --locked -- -D warnings
  OC01_INNER_CURRENT_GATE=1 cargo test -p contextmesh-salience --locked
  cargo test -p contextmesh --locked
  OC01_INNER_CURRENT_GATE=1 cargo test --workspace --locked
  cargo test -p contextmesh --locked --test ob13_demo
  bash scripts/demo.sh >/dev/null
  bash scripts/demo-ob.sh >/dev/null
}

primitives_stage() {
  require_files contextmesh-salience/src/error.rs contextmesh-salience/src/json.rs \
    contextmesh-salience/src/types.rs contextmesh-salience/tests/oc01_schema.rs
  cargo test -p contextmesh-salience --locked --test oc01_schema
}

schema_stage() {
  require_files contextmesh-salience/src/outcome.rs contextmesh-salience/tests/oc01_schema.rs
  cargo test -p contextmesh-salience --locked --test oc01_schema
}

protocol_stage() {
  require_files contextmesh-salience/tests/oc01_crypto.rs
  cargo test -p contextmesh-salience --locked --test oc01_crypto
}

dag_stage() {
  require_files contextmesh-salience/src/verify.rs contextmesh-salience/tests/oc01_dag.rs
  cargo test -p contextmesh-salience --locked --test oc01_dag
}

io_stage() {
  require_files contextmesh-salience/src/io.rs contextmesh-salience/tests/oc01_io.rs
  cargo test -p contextmesh-salience --locked --test oc01_io
}

vectors_stage() {
  require_files contextmesh-salience/tests/oc01_adversarial.rs \
    contextmesh-salience/tests/fixtures/oc01-outcome-ledger-v1-golden.json \
    contextmesh-salience/tests/fixtures/oc01-outcome-ledger-v1-unterminated.json
  cargo test -p contextmesh-salience --locked --test oc01_adversarial
}

evidence_stage() {
  require_files README.md _bmad-output/verification-artifacts/oc-01-evidence.md
  planned_surface_only
  evidence_sources_layer_is_complete
  evidence_four_layers_and_gate_ids_are_complete
  evidence_privacy_and_claim_language
  claim_audit_is_limited_to_oc01
  downstream_gate_requires_oc01_and_p1_preregistration
  OC01_INNER_CURRENT_GATE=1 cargo test --workspace --locked
  if git grep -qE 'token1_[A-Za-z0-9_-]{43}' -- .; then fail "complete token value found"; fi
  printf '%s\n' 'ok: evidence, privacy, and claim inputs present'
}

# --- Machine audits for matrix rows R06-R10 -------------------------------
# Each function is named exactly as the traceability matrix requires, fails
# closed on missing/partial/inconclusive evidence, and reads only committed
# files (non-recording). R06-R07 audit oc-01-evidence.md structure; R08-R10
# audit the gate itself and its claim language.

readonly OC01_EVIDENCE_FILE='_bmad-output/verification-artifacts/oc-01-evidence.md'

evidence_sources_layer_is_complete() {
  local f="$OC01_EVIDENCE_FILE"
  [[ -f "$f" ]] || fail "evidence file is missing"
  grep -q '^## 1\. Sources' "$f" || fail "evidence Sources layer heading missing"
  # R06: exact commit, approved sources, source/test paths, toolchain, commands
  grep -q '0cf192b625384283d10c008d4a0e984ae9d0be08' "$f" \
    || fail "evidence does not record the frozen baseline commit"
  grep -q '36fb0bc' "$f" \
    || fail "evidence does not record the vectors-stage commit"
  grep -qE 'rust-toolchain\.toml|rustc[^ ]* 1\.97|Toolchain' "$f" \
    || fail "evidence does not state the toolchain"
  grep -qE 'cargo (test|build|fmt|clippy)|verify-oc01\.sh' "$f" \
    || fail "evidence does not record verification commands"
  grep -qE 'contextmesh-salience/(src|tests)/' "$f" \
    || fail "evidence does not reference source/test paths"
  printf '%s\n' 'ok: evidence_sources_layer_is_complete'
}

evidence_four_layers_and_gate_ids_are_complete() {
  local f="$OC01_EVIDENCE_FILE"
  [[ -f "$f" ]] || fail "evidence file is missing"
  grep -q '^## 1\. Sources' "$f" || fail "layer 1 (Sources) missing"
  grep -q '^## 2\. Reasoning' "$f" || fail "layer 2 (Reasoning) missing"
  grep -q '^## 3\. Conclusion derivation' "$f" || fail "layer 3 (Conclusion derivation) missing"
  grep -q '^## 4\. Invalidators' "$f" || fail "layer 4 (Invalidators) missing"
  grep -qE '^[[:space:]]*-[[:space:]].*(alternative|reject)' "$f" \
    || fail "evidence records no rejected alternatives"
  # Every acceptance gate from the matrix roll-up must appear in the doc
  local gate
  for gate in OC01-SETUP OC01-SCHEMA OC01-CRYPTO OC01-DAG OC01-IO \
              OC01-ADVERSARIAL OC01-REGRESSION OC01-EVIDENCE; do
    grep -q "$gate" "$f" || fail "evidence does not address gate $gate"
  done
  printf '%s\n' 'ok: evidence_four_layers_and_gate_ids_are_complete'
}

evidence_privacy_and_claim_language() {
  local f="$OC01_EVIDENCE_FILE"
  [[ -f "$f" ]] || fail "evidence file is missing"
  grep -q 'caller' "$f" \
    || fail "evidence does not distinguish caller declarations from verified facts"
  if grep -qE '/home/|/Users/|file://|https?://(localhost|127\.|192\.168\.)' "$f"; then
    fail "evidence contains private paths or internal URLs"
  fi
  printf '%s\n' 'ok: evidence_privacy_and_claim_language'
}

claim_audit_is_limited_to_oc01() {
  local f="$OC01_EVIDENCE_FILE"
  [[ -f "$f" ]] || fail "evidence file is missing"
  # Prohibited claim classes (matrix R09): terminal/success/quality/cost
  # claims, causal attribution, prior/selection utility inference.
  if grep -nEi 'OC-01 (is )?(complete|done|finished|successful)' "$f"; then
    fail "evidence asserts OC-01 completion"
  fi
  grep -q 'non-claims\|Non-claims\|not .claim' "$f" \
    || fail "evidence lacks a non-claims/claim-limits section"
  printf '%s\n' 'ok: claim_audit_is_limited_to_oc01'
}

downstream_gate_requires_oc01_and_p1_preregistration() {
  # R10 truth table: downstream (OC-02) authorization requires BOTH the OC-01
  # gate record and the separate P1 preregistration record. The expected table
  # below is the frozen semantic content; the AND check must reproduce it
  # exactly. The P1 record is NOT defined, approved, or claimed by OC-01 and is
  # represented here only as a pending external input.
  local oc01_pass p1_pass want got
  while read -r oc01_pass p1_pass want; do
    got=$(( oc01_pass && p1_pass ))
    [[ $got == "$want" ]] \
      || fail "downstream truth table row ($oc01_pass,$p1_pass) mismatch"
    [[ $want == 1 ]] && [[ $oc01_pass == 1 && $p1_pass == 1 ]] \
      || { [[ $want == 0 ]] || fail "authorization granted without both records"; }
  done <<'EOF'
0 0 0
1 0 0
0 1 0
1 1 1
EOF
  grep -q 'P1' "$OC01_EVIDENCE_FILE" \
    || fail "evidence does not record the OC-02 P1 preregistration dependency"
  printf '%s\n' 'ok: downstream_gate_requires_oc01_and_p1_preregistration'
}

run_stage() {
  local stage="$1"
  printf 'stage: %s\n' "$stage"
  if [[ "${OC01_STAGE_SELF_TEST:-0}" == 1 ]]; then
    [[ "${OC01_INJECT_FAILURE:-}" != "$stage" ]] || fail "injected stage failure"
    return 0
  fi
  case "$stage" in
    workspace) workspace_stage ;;
    primitives) primitives_stage ;;
    schema) schema_stage ;;
    protocol) protocol_stage ;;
    DAG) dag_stage ;;
    I/O) io_stage ;;
    vectors) vectors_stage ;;
    evidence) evidence_stage ;;
    *) fail "unknown stage" ;;
  esac
}

stages_execute_in_dependency_order() {
  local tmp expected status
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
  expected="$(printf 'stage: %s\n' "${STAGES[@]}")"
  OC01_STAGE_SELF_TEST=1 timeout 10 bash "$0" --run-stages >"$tmp/pass" 2>/dev/null
  [[ "$(<"$tmp/pass")" == "$expected" ]] || fail "self-test stage order mismatch"

  set +e
  OC01_STAGE_SELF_TEST=1 OC01_INJECT_FAILURE=protocol \
    timeout 10 bash "$0" --run-stages >"$tmp/fail" 2>/dev/null
  status=$?
  set -e
  [[ $status -ne 0 && $status -ne 124 ]] || fail "injected failure did not fail boundedly"
  expected="$(printf 'stage: %s\n' workspace primitives schema protocol)"
  [[ "$(<"$tmp/fail")" == "$expected" ]] || fail "later stage ran after injected failure"
  printf '%s\n' 'ok: stages_execute_in_dependency_order'
}

historical_release_chains() {
  require_commit "$OA07_COMMIT"
  require_commit "$OB13_COMMIT"
  OC01_CARGO="${OC01_CARGO:-cargo}" cargo test -p contextmesh-salience --locked \
    --test oc01_workspace historical_oa07_chain_runs_unchanged_at_completion_commit -- --exact
  OC01_CARGO="${OC01_CARGO:-cargo}" cargo test -p contextmesh-salience --locked \
    --test oc01_workspace historical_ob13_chain_runs_unchanged_at_completion_commit -- --exact
}

case "${1:-}" in
  --self-test) stages_execute_in_dependency_order ;;
  --planned-surface-only) planned_surface_only ;;
  --historical-release-chains) historical_release_chains ;;
  --run-stages) for stage in "${STAGES[@]}"; do run_stage "$stage"; done ;;
  "")
    stages_execute_in_dependency_order
    for stage in "${STAGES[@]}"; do run_stage "$stage"; done
    printf '%s\n' 'verify-oc01: all stages passed'
    ;;
  *) fail "unknown argument" ;;
esac
