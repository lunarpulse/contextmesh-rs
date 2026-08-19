#!/usr/bin/env bash
# OB-13 Option B completion release gate.
#
# Fails or passes; never writes evidence, never fetches from the network.
# Requires the OB-13 demo, tests, and completion evidence to be committed.
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly OB08_FIXTURE_SHA256="c68ddef31105c6280e02d288f4e65fbdf6364d7307517dca87b79d6620fa2003"
readonly OB02_FIXTURE_SHA256="f2b52d826699c7116cba9cf182dd99dbb01b46ed736b43e5db997caa9d1787cb"
readonly OB01_FIXTURE_SHA256="39737b1eb03c26dd66da933bbc26076d3b61262d972c094f87b1f68059dbd642"
readonly OA01_FIXTURE_SHA256="799f326d20584b20f455d1b3027cc904848381761b6e591e81cacde0e46d7594"
readonly OA03_FIXTURE_SHA256="7752cd4b2443beb7d22d84e3fa542cc74b261f34f0b70d016ec0a1d05a372e6a"
readonly OA04_FIXTURE_SHA256="71fe501621fec368e14cc9521e58bcb992df39d475c81afe17edf89d323cbbf2"
readonly BASELINE_CLOSURE=320
readonly EVIDENCE="_bmad-output/verification-artifacts/ob-completion-evidence.md"
readonly CLAIMS="_bmad-output/verification-artifacts/ob-claim-audit.md"
readonly AUDIT="_bmad-output/verification-artifacts/ob-12-semantic-mechanisms-audit.md"

cd "$ROOT"
export CARGO_NET_OFFLINE=true
fail() { printf 'verify-ob13: FAIL %s\n' "$*" >&2; exit 1; }

# --- Step 1: clean worktree and candidate identity -------------------------
[[ -z "$(git status --porcelain)" ]] || fail "worktree is not clean"
[[ "$(git log -1 --format=%s)" == "OB-13: add end-to-end demo and Option B completion evidence" ]] \
  || fail "HEAD is not the OB-13 completion commit"
required=(
  src/lib.rs src/bin/demo_ob.rs
  tests/ob13_demo.rs
  tests/common/mod.rs
  scripts/demo-ob.sh
  scripts/verify-ob13.sh
  _bmad-output/implementation-artifacts/spec-option-b-source-grounded-context-handoff.md
  _bmad-output/planning-artifacts/option-b-delivery-plan.md
  "$EVIDENCE" "$CLAIMS" "$AUDIT"
)
for file in "${required[@]}"; do
  [[ -e "$file" ]] || fail "missing OB-13 artifact: $file"
done
[[ -x scripts/demo-ob.sh ]] || fail "demo-ob.sh is not executable"
[[ -x scripts/verify-ob13.sh ]] || fail "verify-ob13.sh is not executable"
grep -q '^verdict: complete' "$EVIDENCE" || fail "verdict is not recorded complete"
grep -qE '^[- ]*dependency-closure: 320$' "$EVIDENCE" \
  || fail "recorded dependency closure does not match the frozen count"
recorded_tree="$(awk '/^procedure-tree: /{print $2; exit}' "$EVIDENCE")"
[[ -n "$recorded_tree" ]] || fail "procedure tree record is empty"
parent_commit="$(git rev-parse HEAD^)"
[[ "$(git rev-parse "${parent_commit}^{tree}")" == "$recorded_tree" ]] \
  || fail "recorded procedure tree does not match the evidence commit's parent"
printf '%s\n' 'ok: clean worktree, committed completion evidence matches HEAD'

# --- Step 2: additive-only changes ------------------------------------------
# OB-13 adds the demo binary (src/bin/demo_ob.rs), the demo script, the test
# matrix, and registers the [[bin]] in Cargo.toml plus the doc note in lib.rs.
# No existing source module or test file may change.
if git diff HEAD^ HEAD -- src/lib.rs tests/common/mod.rs | grep -qE '^-[^-]'; then
  fail "existing file contains deletions vs its parent: lib.rs/tests/common/mod.rs"
fi
changed="$(git diff HEAD^ HEAD --name-only -- src | grep -v '^src/lib.rs$' | grep -v '^src/bin/demo_ob.rs$' || true)"
[[ -z "$changed" ]] || fail "unexpected source module change: $changed"
if git diff HEAD^ HEAD -- Cargo.toml | grep -qE '^-[^-]'; then
  fail "Cargo.toml contains deletions (only the demo_ob [[bin]] may be added)"
fi
tests_changed="$(git diff HEAD^ HEAD --name-only -- tests | grep -v '^tests/ob13_demo.rs$' || true)"
[[ -z "$tests_changed" ]] || fail "unexpected test change: $tests_changed"
printf '%s\n' 'ok: only the new demo binary, demo test, script, and [[bin]] registration'

# --- Step 3: exact pins, closure, forbidden surfaces, unchanged feature graph
[[ "$(grep -c '^name = ' Cargo.lock)" -eq "$BASELINE_CLOSURE" ]] \
  || fail "Cargo.lock dependency closure changed from ${BASELINE_CLOSURE}"
python3 -I - <<'PY' || fail "dependency/feature pins drifted"
import json, subprocess
m = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"]))
root = next(p for p in m["packages"] if p["id"] == m["resolve"]["root"])
by_name = {p["name"]: p for p in m["packages"]}
forbidden_closure = {"openssl", "native-tls", "rustls", "tokio-rustls",
                     "hyper-rustls", "cookie", "cookie_store",
                     "async-compression", "brotli", "flate2", "zstd", "h2",
                     "h3", "quinn", "hickory-resolver", "trust-dns-resolver",
                     "multer", "rmcp", "libp2p", "rusqlite", "libsqlite3-sys"}
assert not set(by_name) & forbidden_closure, sorted(set(by_name) & forbidden_closure)
direct_names = {d["name"] for d in root["dependencies"]}
assert "time" not in direct_names, "OB-13 must not add a wall-clock dependency"
semantic_closure = {"ort", "candle-core", "tch", "fastembed", "hnswlib",
                    "usearch", "tokenizers", "half", "onnxruntime"}
assert not set(by_name) & semantic_closure, sorted(set(by_name) & semantic_closure)
PY
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
cargo tree --locked -e features | python3 -I -c 'import sys; sys.stdout.write(sys.stdin.read().replace(f"({sys.argv[1]})", "(<WORKSPACE>)"))' "$ROOT" > "$tmp/features.txt"
cmp "$tmp/features.txt" cargo-tree-oa05-features.txt \
  || fail "locked feature graph drifted from the OA baseline"
printf '%s\n' 'ok: closure 320, forbidden and semantic surfaces absent, feature graph unchanged'

# --- Step 4: fixtures frozen -------------------------------------------------
[[ "$(sha256sum tests/fixtures/ob08-eval-manifest.json | awk '{print $1}')" == "$OB08_FIXTURE_SHA256" ]]
[[ "$(sha256sum tests/fixtures/ob02-selection-golden.json | awk '{print $1}')" == "$OB02_FIXTURE_SHA256" ]]
[[ "$(sha256sum tests/fixtures/ob01-receipt-golden.json | awk '{print $1}')" == "$OB01_FIXTURE_SHA256" ]]
[[ "$(sha256sum tests/fixtures/oa01-v1-golden.json | awk '{print $1}')" == "$OA01_FIXTURE_SHA256" ]]
[[ "$(sha256sum tests/fixtures/oa03-bundle-v1-golden.json | awk '{print $1}')" == "$OA03_FIXTURE_SHA256" ]]
[[ "$(sha256sum tests/fixtures/oa04-protocol-golden.json | awk '{print $1}')" == "$OA04_FIXTURE_SHA256" ]]
printf '%s\n' 'ok: OB-08 manifest, OB-02, OB-01, and OA-01/OA-03/OA-04 fixtures are unchanged'

# --- Step 5: build, format, strict Clippy, full workspace tests -------------
cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
printf '%s\n' 'ok: locked build, format, strict Clippy, and all workspace tests passed'

# --- Step 6: the full OB matrix (B1..B11 packages plus the demo) ------------
cargo test --locked --test ob13_demo
cargo test --locked --test ob11_capability
cargo test --locked --test ob09_summaries
cargo test --locked --test ob10_sufficient
cargo test --locked --test ob08_eval
cargo test --locked --test ob07_repair
cargo test --locked --test ob06_omission
cargo test --locked --test ob05_validity
cargo test --locked --test ob04_delta
cargo test --locked --test ob03_closure
cargo test --locked --test ob02_selection
cargo test --locked --test ob01_receipts
printf '%s\n' 'ok: OB-13 demo and the OB-11..OB-01 matrices passed'

# --- Step 7: the Option B demo (phase success signal) ------------------------
bash scripts/demo-ob.sh
printf '%s\n' 'ok: the end-to-end Option B demo passed'

# --- Step 8: OA regression chain ----------------------------------------------
bash scripts/verify-oa01.sh
bash scripts/verify-oa02.sh
bash scripts/verify-oa03.sh
bash scripts/verify-oa04.sh
bash scripts/verify-oa04-dependencies.sh
bash scripts/verify-oa05.sh
printf '%s\n' 'ok: OA-01 through OA-05 regression verifiers passed'

# --- Step 9: runtime-artifact and secret scan ---------------------------------
if git ls-files | grep -Eq '\.(db|db-shm|db-wal|log|jsonl)$'; then
  fail "tracked runtime artifact present"
fi
if find . -path ./.git -prune -o -path ./target -prune -o \
     -type f \( -name '*.db' -o -name '*.db-shm' -o -name '*.db-wal' \
     -o -name '*.jsonl' -o -name '*.token' -o -name '*.key' \) -print | grep -q .; then
  fail "untracked runtime secret, repair history, or database present in the tree"
fi
if git grep -qE 'token1_[A-Za-z0-9_-]{43}' -- .; then
  fail "a complete token value appears in tracked content"
fi
printf '%s\n' 'ok: no tracked runtime artifacts or secret values'

[[ -z "$(git status --porcelain)" ]] || fail "worktree became dirty during the gate"
printf '%s\n' 'verify-ob13: all checkpoints passed'
