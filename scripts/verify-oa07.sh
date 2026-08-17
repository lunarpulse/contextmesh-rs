#!/usr/bin/env bash
# OA-07 deterministic non-recording release gate.
#
# Fails or passes; never writes evidence, never fetches from the network
# (the fresh-target repetition runs with CARGO_NET_OFFLINE=true against the
# local registry cache). Requires the evidence artifacts to be committed.
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly EVIDENCE="_bmad-output/verification-artifacts/oa-07-release-evidence.md"
readonly CLAIMS="_bmad-output/verification-artifacts/oa-07-claim-audit.md"
readonly SPEC="_bmad-output/implementation-artifacts/spec-oa-07-release-gate.md"
readonly BASELINE_OK_COUNT=155
readonly BASELINE_CLOSURE=320

cd "$ROOT"
# The entire gate is offline: every cargo invocation resolves from the local
# registry cache only (--locked plus a warm cache; no advisory or index
# fetches).
export CARGO_NET_OFFLINE=true
fail() { printf 'verify-oa07: FAIL %s\n' "$*" >&2; exit 1; }

# --- Step 1: clean worktree and candidate identity -------------------------
[[ -z "$(git status --porcelain)" ]] || fail "worktree is not clean"
for file in "$EVIDENCE" "$CLAIMS" "$SPEC" scripts/verify-oa07.sh; do
  [[ -e "$file" ]] || fail "missing OA-07 artifact: $file"
done
[[ -x scripts/verify-oa07.sh ]] || fail "verify-oa07.sh is not executable"
grep -q '^procedure-tree: ' "$EVIDENCE" || fail "evidence does not record the procedure tree"
recorded_tree="$(awk '/^procedure-tree: /{print $2; exit}' "$EVIDENCE")"
[[ -n "$recorded_tree" ]] || fail "procedure tree record is empty"
parent_commit="$(git rev-parse HEAD^)"
[[ "$(git rev-parse "${parent_commit}^{tree}")" == "$recorded_tree" ]] \
  || fail "recorded procedure tree does not match the evidence commit's parent"
[[ "$(git log -1 --format=%s)" == "OA-07: record Option A completion evidence" ]] \
  || fail "HEAD is not the OA-07 evidence commit"
grep -q '^verdict: complete' "$EVIDENCE" || fail "verdict is not recorded complete"
grep -qE '^(- )?advisory-database: ' "$EVIDENCE" || fail "advisory database not recorded"
grep -qE '^(- )?vulnerabilities: 0' "$EVIDENCE" || fail "advisory result is not zero"
grep -qE "^(- )?dependency-closure: ${BASELINE_CLOSURE}$" "$EVIDENCE" \
  || fail "recorded dependency closure does not match the frozen count"
printf '%s\n' 'ok: clean worktree, committed evidence matches HEAD, verdict recorded'

# --- Step 2: pinned toolchain, no overrides, native prerequisites ----------
[[ "$(rustc --version)" == 'rustc 1.97.0 '* ]] || fail "rustc is not the pinned 1.97.0"
[[ "$(cargo --version)" == 'cargo 1.97.0 '* ]] || fail "cargo is not the pinned 1.97.0"
rustup show active-toolchain 2>/dev/null | grep -q 'rust-toolchain.toml' \
  || fail "active toolchain is not driven by rust-toolchain.toml"
grep -q '1.97.0' rust-toolchain.toml || fail "rust-toolchain.toml lost the pin"
for var in RUSTC RUSTFLAGS CARGO_BUILD_TARGET CARGO_BUILD_RUSTFLAGS \
           CARGO_BUILD_INCREMENTAL CARGO_TARGET_DIR; do
  [[ -z "${!var:-}" ]] || fail "environment override set: $var"
done
command -v cc >/dev/null 2>&1 || fail "native C toolchain (cc) missing"
cc --version >/dev/null 2>&1 || fail "cc is not usable"
printf '%s\n' 'ok: pinned toolchain active, no env overrides, native cc present'

# --- Step 3: exact dependencies, closure, graph, licenses (offline) --------
[[ "$(grep -c '^name = ' Cargo.lock)" -eq "$BASELINE_CLOSURE" ]] \
  || fail "Cargo.lock dependency closure changed"
python3 -I - <<'PY' || fail "dependency/feature pins drifted"
import json, subprocess
m = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"]))
root = next(p for p in m["packages"] if p["id"] == m["resolve"]["root"])
normal = {(d["name"], d["req"], d["uses_default_features"], tuple(sorted(d["features"])))
          for d in root["dependencies"] if d["kind"] is None}
assert ("turso", "=0.7.2", False, ()) in normal
assert ("tokio", "=1.53.1", False,
        ("io-util", "net", "process", "rt", "signal", "sync", "time")) in normal
assert ("clap", "=4.6.6", False,
        ("derive", "error-context", "help", "std", "usage")) in normal
assert ("axum", "=0.8.9", False, ("http1", "json", "tokio")) in normal
assert ("reqwest", "=0.13.4", False, ("json",)) in normal
by_name = {p["name"]: p for p in m["packages"]}
# Forbidden in the entire transitive closure: capability surfaces only.
# (rand/rand_core/hex/regex/uuid/signature appear legitimately as transitive
# dependencies of ed25519-dalek and turso; they are forbidden only as direct
# dependencies, asserted below.)
forbidden_closure = {"openssl", "native-tls", "rustls", "tokio-rustls",
                     "hyper-rustls", "cookie", "cookie_store",
                     "async-compression", "brotli", "flate2", "zstd", "h2",
                     "h3", "quinn", "hickory-resolver", "trust-dns-resolver",
                     "multer", "rmcp", "libp2p", "rusqlite", "libsqlite3-sys"}
assert not set(by_name) & forbidden_closure, sorted(set(by_name) & forbidden_closure)
direct_names = {d["name"] for d in root["dependencies"]}
forbidden_direct = forbidden_closure | {"rand", "rand_core", "hex", "regex",
                                        "uuid", "signature"}
assert not direct_names & forbidden_direct, sorted(direct_names & forbidden_direct)
# License allowlist: every dependency offers at least one fully permissive
# alternative; cfg_block 0.1.1 carries no license field and is the one
# recorded accepted finding.
permissive = {"MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception",
              "BSD-2-Clause", "BSD-3-Clause", "BSD-1-Clause", "ISC", "Zlib",
              "BSL-1.0", "CC0-1.0", "MIT-0", "Unicode-3.0", "Unlicense"}
for p in m["packages"]:
    if p["name"] == "contextmesh":
        continue
    expr = p.get("license")
    if not expr:
        assert (p["name"], p["version"]) == ("cfg_block", "0.1.1"), (p["name"], p["version"])
        continue
    # Informal dual-license spellings use "/" as an OR separator and may
    # parenthesize alternatives.
    expr = expr.replace("/", " OR ").replace("(", " ").replace(")", " ")
    alternatives = [alt.strip() for alt in expr.split(" OR ")]
    assert any(all(part.strip() in permissive for part in alt.split(" AND "))
               for alt in alternatives), (p["name"], expr)
PY
tmp="$(mktemp -d)"; tmp_chain=""
trap 'rm -rf "$tmp" "$tmp_chain"' EXIT
cargo tree --locked -e features | python3 -I -c 'import sys; sys.stdout.write(sys.stdin.read().replace(f"({sys.argv[1]})", "(<WORKSPACE>)"))' "$ROOT" > "$tmp/features.txt"
cmp "$tmp/features.txt" cargo-tree-oa05-features.txt \
  || fail "locked feature graph drifted from the recorded evidence"
printf '%s\n' 'ok: exact pins, closure 320, licenses permissive, locked graph matches'

# --- Steps 4+5: verifiers OA-00 through OA-06 on this tree -----------------
tmp_chain="$tmp/chain.log"
if ! bash scripts/verify-oa06.sh > "$tmp_chain" 2>&1; then
  tail -n 50 "$tmp_chain" >&2
  fail "the verify-oa06 chain failed"
fi
chain_ok="$(grep -c '^ok:' "$tmp_chain")"
(( chain_ok >= BASELINE_OK_COUNT )) \
  || fail "chain ok-checkpoint count ${chain_ok} is below baseline ${BASELINE_OK_COUNT}"
if grep -Eq 'FAIL|error\[|error: ' "$tmp_chain"; then
  fail "chain output contains failure markers"
fi
grep -q 'verify-oa06: all checkpoints passed' "$tmp_chain" \
  || fail "chain did not print its completion line"
printf 'ok: OA-00..OA-06 chain passed with %s checkpoints\n' "$chain_ok"

cargo fmt --all -- --check
printf '%s\n' 'ok: rustfmt clean'

# --- Step 6: fresh-target offline repetition --------------------------------
fresh="$tmp/fresh"
mkdir -p "$fresh/demo-root"
# Disable incremental compilation for the single-shot fresh build: it shrinks
# the fresh target considerably and the fresh repetition is inherently a
# from-scratch build. Scoped here so the warm chain target is unaffected.
export CARGO_INCREMENTAL=0
cargo build --workspace --locked --target-dir "$fresh/target"
cargo clippy --workspace --all-targets --locked --target-dir "$fresh/target" -- -D warnings
if ! cargo test --workspace --locked --target-dir "$fresh/target" > "$tmp/fresh-test.log" 2>&1; then
  tail -n 60 "$tmp/fresh-test.log" >&2
  fail "fresh-target test suite exited unsuccessfully"
fi
python3 -I - "$tmp/fresh-test.log" <<'PY' || fail "fresh-target test suite had failures"
import re, sys
text = open(sys.argv[1]).read()
results = re.findall(r"test result: (ok|FAILED)", text)
assert results and all(r == "ok" for r in results), results
PY
demo_out="$(OA06_DEMO_RUNTIME_ROOT="$fresh/demo-root/run" bash scripts/demo.sh)"
[[ "$demo_out" == *"demo: PASS"* ]] || fail "fresh-root demo failed"
[[ "$demo_out" != *"token1_"* ]] || fail "fresh-root demo leaked a token prefix"
printf '%s\n' 'ok: fresh-target offline build, Clippy, tests, and demo passed'

# --- Step 7: secret and runtime-artifact scan -------------------------------
if git ls-files | grep -Eq '\.(db|db-shm|db-wal|log)$'; then
  fail "tracked runtime artifact present"
fi
if find . -path ./.git -prune -o -path ./target -prune -o \
     -type f \( -name '*.db' -o -name '*.db-shm' -o -name '*.db-wal' \
     -o -name '*.token' -o -name '*.key' \) -print | grep -q .; then
  fail "untracked runtime secret or database present in the tree"
fi
while IFS= read -r ignored; do
  [[ -z "$ignored" ]] && continue
  case "$ignored" in
    target/*|.agents/*|_bmad/*) : ;;
    *) fail "unexpected ignored file: $ignored" ;;
  esac
  case "$ignored" in
    *.db|*.db-shm|*.db-wal|*.log|*.token|*.key) fail "ignored runtime artifact: $ignored" ;;
  esac
done < <(git ls-files --others --ignored --exclude-standard)
# Scan for complete token VALUES (token1_ + exactly 43 base64url chars).
# The bare prefix string is legitimate documentation and scan text; a full
# value anywhere in tracked files or the evidence would be a leak.
if git grep -qE 'token1_[A-Za-z0-9_-]{43}' -- .; then
  fail "a complete token value appears in tracked content"
fi
if grep -qE 'token1_[A-Za-z0-9_-]{43}' "$EVIDENCE" "$CLAIMS" "$SPEC"; then
  fail "a complete token value appears in the evidence artifacts"
fi
printf '%s\n' 'ok: no tracked runtime artifacts, secrets confined to code and vectors'

# --- Steps 8-10: audit layers, A1-A8 rows, Always/Never ---------------------
for layer in crypto database graph transport provider shell supply-chain claims; do
  grep -q "| $layer " "$EVIDENCE" || fail "audit layer not recorded: $layer"
done
for gate in A1 A2 A3 A4 A5 A6 A7 A8; do
  grep -q "^### $gate" "$EVIDENCE" || fail "evidence matrix missing row: $gate"
done
grep -q '^## Always/Never consistency' "$EVIDENCE" \
  || fail "evidence lacks the Always/Never consistency table"
for classification in demonstrated limited removed; do
  grep -q "$classification" "$CLAIMS" || fail "claim audit lacks '$classification' rows"
done
printf '%s\n' 'ok: eight audit layers, A1-A8 rows, Always/Never table, claim classes recorded'

[[ -z "$(git status --porcelain)" ]] || fail "worktree became dirty during the gate"
printf '%s\n' 'verify-oa07: all checkpoints passed'
