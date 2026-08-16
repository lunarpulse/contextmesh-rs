#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly OA01_COMMIT="f61c4f0d147544c4011b2bb8b8094943e196c883"
readonly OA03_FIXTURE_SHA256="7752cd4b2443beb7d22d84e3fa542cc74b261f34f0b70d016ec0a1d05a372e6a"
cd "$ROOT"

required=(
  src/store.rs src/store/dag.rs src/store/bundle.rs src/store/verify.rs src/error.rs
  tests/oa03_dag.rs tests/oa03_projection.rs tests/oa03_bundle.rs tests/oa03_verify.rs
  tests/fixtures/oa03-bundle-v1-golden.json scripts/verify-oa03.sh
  _bmad-output/implementation-artifacts/spec-oa-03-dag-bundles-verification.md
)
for file in "${required[@]}"; do
  [[ -e "$file" ]] || { printf 'missing OA-03 artifact: %s\n' "$file" >&2; exit 1; }
done
[[ -x scripts/verify-oa03.sh ]] || { printf '%s\n' 'verify-oa03.sh is not executable' >&2; exit 1; }
printf '%s\n' 'ok: OA-03 artifacts exist'

python3 -I - <<'PY'
import json, subprocess
m=json.loads(subprocess.check_output(["cargo","metadata","--locked","--format-version","1"]))
r=next(p for p in m["packages"] if p["id"]==m["resolve"]["root"])
normal={(d["name"],d["req"],d["uses_default_features"],tuple(sorted(d["features"]))) for d in r["dependencies"] if d["kind"] is None}
assert ("tokio","=1.53.1",False,("io-util","net","process","rt","signal","sync","time")) in normal
assert ("clap","=4.6.6",False,("derive","error-context","help","std","usage")) in normal
assert ("turso","=0.7.2",False,()) in normal
assert ("axum","=0.8.9",False,("http1","json","tokio")) in normal
assert ("reqwest","=0.13.4",False,("json",)) in normal
assert not {"libp2p","rusqlite","libsqlite3-sys","openssl","native-tls","rustls","tokio-rustls","hyper-rustls","h2","h3","quinn"}.intersection(p["name"] for p in m["packages"])
PY
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
cargo tree --locked -e features | python3 -I -c 'import sys; sys.stdout.write(sys.stdin.read().replace(f"({sys.argv[1]})", "(<WORKSPACE>)"))' "$ROOT" > "$tmp/features.txt"
cmp "$tmp/features.txt" cargo-tree-oa05-features.txt
printf '%s\n' 'ok: locked graph matches the frozen OA-04 dependency selection'

[[ "$(sha256sum tests/fixtures/oa03-bundle-v1-golden.json | awk '{print $1}')" == "$OA03_FIXTURE_SHA256" ]]
cargo test --locked --test oa03_bundle canonical_bundle_fixture_is_frozen_and_independently_verified
printf '%s\n' 'ok: canonical Bundle v1 fixture checksum and independent event verification match'

[[ "$(sha256sum tests/fixtures/oa01-v1-golden.json | awk '{print $1}')" == "799f326d20584b20f455d1b3027cc904848381761b6e591e81cacde0e46d7594" ]]
git diff --exit-code "$OA01_COMMIT" -- tests/fixtures/oa01-v1-golden.json
printf '%s\n' 'ok: OA-01 fixture unchanged'

cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
bash scripts/verify-oa01.sh
bash scripts/verify-oa02.sh
printf '%s\n' 'ok: locked quality suite and OA-01/OA-02 regressions passed'
