#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly OA01_FIXTURE_SHA256="799f326d20584b20f455d1b3027cc904848381761b6e591e81cacde0e46d7594"
readonly OA03_FIXTURE_SHA256="7752cd4b2443beb7d22d84e3fa542cc74b261f34f0b70d016ec0a1d05a372e6a"
readonly OA04_FIXTURE_SHA256="71fe501621fec368e14cc9521e58bcb992df39d475c81afe17edf89d323cbbf2"
cd "$ROOT"

required=(
  Cargo.toml Cargo.lock cargo-tree-oa05-features.txt
  src/cli.rs src/provider.rs src/store/invocation.rs src/crypto.rs src/error.rs
  src/bin/contextmesh.rs src/bin/demo_agent.rs
  tests/oa05_keys.rs tests/oa05_provider.rs tests/oa05_cli.rs tests/oa05_jsonl.rs
  tests/fixtures/oa05-cli-golden.json
  scripts/verify-oa05.sh
  _bmad-output/implementation-artifacts/spec-oa-05-provider-recording-cli.md
)
for file in "${required[@]}"; do
  [[ -e "$file" ]] || { printf 'missing OA-05 artifact: %s\n' "$file" >&2; exit 1; }
done
[[ -x scripts/verify-oa05.sh ]] || { printf '%s\n' 'verify-oa05.sh is not executable' >&2; exit 1; }
printf '%s\n' 'ok: OA-05 artifacts exist'

python3 -I - <<'PY'
import json, subprocess
m=json.loads(subprocess.check_output(["cargo","metadata","--locked","--format-version","1"]))
r=next(p for p in m["packages"] if p["id"]==m["resolve"]["root"])
normal={(d["name"],d["req"],d["uses_default_features"],tuple(sorted(d["features"]))) for d in r["dependencies"] if d["kind"] is None}
assert ("clap","=4.6.6",False,("derive","error-context","help","std","usage")) in normal
assert ("tokio","=1.53.1",False,("io-util","net","process","rt","signal","sync","time")) in normal
by_name={p["name"]:p for p in m["packages"]}
assert by_name["clap"]["version"]=="4.6.6"
assert by_name["tokio"]["version"]=="1.53.1"
clap=next(n for n in m["resolve"]["nodes"] if n["id"]==by_name["clap"]["id"])
assert not set(clap["features"]) & {"default","color","suggestions","unicode","env","wrap_help"}, clap["features"]
forbidden={"openssl","native-tls","rustls","rustls-pki-types","tokio-rustls","hyper-rustls","cookie","cookie_store","async-compression","brotli","flate2","zstd","h2","h3","quinn","hickory-resolver","trust-dns-resolver","multer","rmcp","libp2p","rusqlite","libsqlite3-sys"}
assert not set(by_name) & forbidden, sorted(set(by_name)&forbidden)
PY
printf '%s\n' 'ok: frozen OA-05 pins/features selected and forbidden surfaces absent'

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
cargo tree --locked -e features | python3 -I -c 'import sys; sys.stdout.write(sys.stdin.read().replace(f"({sys.argv[1]})", "(<WORKSPACE>)"))' "$ROOT" > "$tmp/features.txt"
cmp "$tmp/features.txt" cargo-tree-oa05-features.txt
printf '%s\n' 'ok: locked OA-05 feature graph matches recorded evidence'

[[ "$(sha256sum tests/fixtures/oa01-v1-golden.json | awk '{print $1}')" == "$OA01_FIXTURE_SHA256" ]]
[[ "$(sha256sum tests/fixtures/oa03-bundle-v1-golden.json | awk '{print $1}')" == "$OA03_FIXTURE_SHA256" ]]
[[ "$(sha256sum tests/fixtures/oa04-protocol-golden.json | awk '{print $1}')" == "$OA04_FIXTURE_SHA256" ]]
printf '%s\n' 'ok: OA-01/OA-03/OA-04 fixtures are unchanged'

cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
printf '%s\n' 'ok: locked build, format, strict Clippy, and all workspace tests passed'

cargo test --locked --test oa05_keys --test oa05_provider --test oa05_cli --test oa05_jsonl
printf '%s\n' 'ok: OA-05 custody, provider, CLI, and JSONL matrices passed'

# OA-06 replaced the sentinel with the real two-node demo harness; its full
# execution is owned by verify-oa06.sh.
grep -q 'OA-06 reproducible two-node demo' scripts/demo.sh
if grep -q 'OA-06 pending: the two-node Option A demo' scripts/demo.sh; then
  printf '%s\n' 'OA-06 failure sentinel still present' >&2
  exit 1
fi
printf '%s\n' 'ok: the OA-06 demo harness replaced the failure sentinel'

bash scripts/verify-oa01.sh
bash scripts/verify-oa02.sh
bash scripts/verify-oa03.sh
bash scripts/verify-oa04.sh
bash scripts/verify-oa04-dependencies.sh
printf '%s\n' 'ok: OA-01 through OA-04 regressions and the D-04-01 probe verifier passed'
