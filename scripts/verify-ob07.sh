#!/usr/bin/env bash
# OB-07 progressive context repair release gate (gate B7).
#
# Fails or passes; never writes evidence, never fetches from the network.
# Requires the OB-07 implementation, tests, and evidence to be committed.
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly OB02_FIXTURE_SHA256="f2b52d826699c7116cba9cf182dd99dbb01b46ed736b43e5db997caa9d1787cb"
readonly OB01_FIXTURE_SHA256="39737b1eb03c26dd66da933bbc26076d3b61262d972c094f87b1f68059dbd642"
readonly OA01_FIXTURE_SHA256="799f326d20584b20f455d1b3027cc904848381761b6e591e81cacde0e46d7594"
readonly OA03_FIXTURE_SHA256="7752cd4b2443beb7d22d84e3fa542cc74b261f34f0b70d016ec0a1d05a372e6a"
readonly OA04_FIXTURE_SHA256="71fe501621fec368e14cc9521e58bcb992df39d475c81afe17edf89d323cbbf2"
readonly BASELINE_CLOSURE=320
readonly EVIDENCE="_bmad-output/verification-artifacts/ob-07-evidence.md"

cd "$ROOT"
export CARGO_NET_OFFLINE=true
fail() { printf 'verify-ob07: FAIL %s\n' "$*" >&2; exit 1; }

# --- Step 1: clean worktree and committed OB-07 artifacts -------------------
[[ -z "$(git status --porcelain)" ]] || fail "worktree is not clean"
[[ "$(git log -1 --format=%s)" == "OB-07: add progressive context repair (B7)" ]] \
  || fail "HEAD is not the OB-07 progressive context repair commit"
required=(
  src/repair.rs src/lib.rs
  tests/ob07_repair.rs
  tests/common/mod.rs
  scripts/verify-ob07.sh
  _bmad-output/implementation-artifacts/spec-option-b-source-grounded-context-handoff.md
  _bmad-output/planning-artifacts/option-b-delivery-plan.md
  "$EVIDENCE"
)
for file in "${required[@]}"; do
  [[ -e "$file" ]] || fail "missing OB-07 artifact: $file"
done
[[ -x scripts/verify-ob07.sh ]] || fail "verify-ob07.sh is not executable"
printf '%s\n' 'ok: clean worktree, OB-07 artifacts committed at HEAD'

# --- Step 2: additive-only changes ------------------------------------------
# OB-07 adds the repair module (src/repair.rs) and registers the doc note in
# lib.rs. The OB-01..OB-06 modules and all Option A modules must be untouched.
if git diff HEAD^ HEAD -- src/lib.rs tests/common/mod.rs | grep -qE '^-[^-]'; then
  fail "existing file contains deletions vs its parent: lib.rs/tests/common/mod.rs"
fi
if git diff HEAD^ HEAD -- src/receipt.rs src/crypto.rs src/cli.rs src/model.rs \
    src/store.rs src/error.rs src/sync.rs src/provider.rs src/http.rs \
    src/selection.rs src/compiler.rs src/closure.rs src/delta.rs \
    src/handoff.rs | grep -q '^[+-]'; then
  fail "an OB-01, OB-02, OB-03, OB-04, OB-05, OB-06, or Option A module changed"
fi
changed="$(git diff HEAD^ HEAD --name-only -- src | grep -v '^src/lib.rs$' | grep -v '^src/repair.rs$' || true)"
[[ -z "$changed" ]] || fail "unexpected source module change: $changed"
printf '%s\n' 'ok: only additive registration and the new OB-07 repair module'

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
assert "time" not in direct_names, "OB-07 must not add a wall-clock dependency"
PY
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
cargo tree --locked -e features | python3 -I -c 'import sys; sys.stdout.write(sys.stdin.read().replace(f"({sys.argv[1]})", "(<WORKSPACE>)"))' "$ROOT" > "$tmp/features.txt"
cmp "$tmp/features.txt" cargo-tree-oa05-features.txt \
  || fail "locked feature graph drifted from the OA baseline"
printf '%s\n' 'ok: closure 320, forbidden surfaces absent, feature graph unchanged'

# --- Step 4: fixtures frozen -------------------------------------------------
[[ "$(sha256sum tests/fixtures/ob02-selection-golden.json | awk '{print $1}')" == "$OB02_FIXTURE_SHA256" ]]
[[ "$(sha256sum tests/fixtures/ob01-receipt-golden.json | awk '{print $1}')" == "$OB01_FIXTURE_SHA256" ]]
[[ "$(sha256sum tests/fixtures/oa01-v1-golden.json | awk '{print $1}')" == "$OA01_FIXTURE_SHA256" ]]
[[ "$(sha256sum tests/fixtures/oa03-bundle-v1-golden.json | awk '{print $1}')" == "$OA03_FIXTURE_SHA256" ]]
[[ "$(sha256sum tests/fixtures/oa04-protocol-golden.json | awk '{print $1}')" == "$OA04_FIXTURE_SHA256" ]]
printf '%s\n' 'ok: OB-02, OB-01, and OA-01/OA-03/OA-04 fixtures are unchanged'

# --- Step 5: build, format, strict Clippy, full workspace tests -------------
cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
printf '%s\n' 'ok: locked build, format, strict Clippy, and all workspace tests passed'

# --- Step 6: OB-07, OB-06, OB-05, OB-04, OB-03, OB-02, and OB-01 matrices ---
cargo test --locked --test ob07_repair
cargo test --locked --test ob06_omission
cargo test --locked --test ob05_validity
cargo test --locked --test ob04_delta
cargo test --locked --test ob03_closure
cargo test --locked --test ob02_selection
cargo test --locked --test ob01_receipts
printf '%s\n' 'ok: OB-07 repair, OB-06 omission, OB-05 validity, OB-04 delta, OB-03 closure, OB-02 selection, and OB-01 receipt matrices passed'

# --- Step 7: OA regression chain ----------------------------------------------
bash scripts/verify-oa01.sh
bash scripts/verify-oa02.sh
bash scripts/verify-oa03.sh
bash scripts/verify-oa04.sh
bash scripts/verify-oa04-dependencies.sh
bash scripts/verify-oa05.sh
printf '%s\n' 'ok: OA-01 through OA-05 regression verifiers passed'

# --- Step 8: runtime-artifact and secret scan ---------------------------------
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
printf '%s\n' 'verify-ob07: all checkpoints passed'
