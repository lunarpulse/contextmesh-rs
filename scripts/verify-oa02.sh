#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly OA01_COMMIT="f61c4f0d147544c4011b2bb8b8094943e196c883"
readonly FIXTURE_SHA256="799f326d20584b20f455d1b3027cc904848381761b6e591e81cacde0e46d7594"
cd "$ROOT"

required=(
  Cargo.toml Cargo.lock cargo-tree-oa02-features.txt
  src/store.rs src/error.rs
  tests/oa02_store.rs tests/oa02_rollback.rs tests/oa02_schema.rs tests/oa02_concurrency.rs
  scripts/verify-oa02.sh
  _bmad-output/implementation-artifacts/spec-oa-02-transactional-store.md
  _bmad-output/verification-artifacts/oa-02-turso-capability-probe.md
)
for file in "${required[@]}"; do
  [[ -e "$file" ]] || { printf 'missing OA-02 artifact: %s\n' "$file" >&2; exit 1; }
done
[[ -x scripts/verify-oa02.sh ]] || { printf '%s\n' 'verify-oa02.sh is not executable' >&2; exit 1; }
printf '%s\n' 'ok: OA-02 artifacts exist'

python3 -I - <<'PY'
import json, subprocess
m=json.loads(subprocess.check_output(["cargo","metadata","--locked","--format-version","1"]))
r=next(p for p in m["packages"] if p["id"]==m["resolve"]["root"])
deps=[d for d in r["dependencies"] if d["name"]=="tokio"]
normal=next(d for d in deps if d["kind"] is None)
dev=next(d for d in deps if d["kind"]=="dev")
assert normal["req"]=="=1.53.1" and not normal["uses_default_features"] and sorted(normal["features"])==["io-util","net","process","rt","signal","sync","time"]
assert dev["req"]=="=1.53.1" and not dev["uses_default_features"] and sorted(dev["features"])==["macros","net","rt","sync","time"]
turso=next(d for d in r["dependencies"] if d["name"]=="turso" and d["kind"] is None)
assert turso["req"]=="=0.7.2" and not turso["uses_default_features"] and not turso["features"]
clap=next(d for d in r["dependencies"] if d["name"]=="clap" and d["kind"] is None)
assert clap["req"]=="=4.6.6" and not clap["uses_default_features"] and sorted(clap["features"])==["derive","error-context","help","std","usage"]
assert not {"libp2p","rusqlite","libsqlite3-sys","openssl","native-tls","rustls","tokio-rustls","hyper-rustls","h2","h3","quinn"}.intersection(p["name"] for p in m["packages"])
PY
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
cargo tree --locked -e features | python3 -I -c 'import sys; sys.stdout.write(sys.stdin.read().replace(f"({sys.argv[1]})", "(<WORKSPACE>)"))' "$ROOT" > "$tmp/features.txt"
cmp "$tmp/features.txt" cargo-tree-oa05-features.txt
printf '%s\n' 'ok: exact runtime/dev features and locked graph match'

[[ "$(sha256sum tests/fixtures/oa01-v1-golden.json | awk '{print $1}')" == "$FIXTURE_SHA256" ]]
git diff --exit-code "$OA01_COMMIT" -- tests/fixtures/oa01-v1-golden.json
for file in scripts/demo.sh; do
  git diff --exit-code "$OA01_COMMIT" -- "$file"
done
printf '%s\n' 'ok: OA-01 fixture unchanged; only the OA-06 demo sentinel remains frozen'

cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
printf '%s\n' 'ok: locked build, format, Clippy, and all tests passed'
