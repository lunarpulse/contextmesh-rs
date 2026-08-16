#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly OA01_COMMIT="f61c4f0d147544c4011b2bb8b8094943e196c883"
readonly OA01_FIXTURE_SHA256="799f326d20584b20f455d1b3027cc904848381761b6e591e81cacde0e46d7594"
readonly OA03_FIXTURE_SHA256="7752cd4b2443beb7d22d84e3fa542cc74b261f34f0b70d016ec0a1d05a372e6a"
readonly OA04_FIXTURE_SHA256="71fe501621fec368e14cc9521e58bcb992df39d475c81afe17edf89d323cbbf2"
cd "$ROOT"

required=(
  Cargo.toml Cargo.lock cargo-tree-oa04-features.txt
  src/http.rs src/sync.rs src/store/sync.rs src/error.rs src/lib.rs
  tests/oa04_auth.rs tests/oa04_protocol.rs tests/oa04_sync.rs tests/oa04_transport.rs
  tests/fixtures/oa04-protocol-golden.json
  scripts/verify-oa04.sh scripts/verify-oa04-dependencies.sh
  _bmad-output/implementation-artifacts/spec-oa-04-authenticated-pull-sync.md
  _bmad-output/implementation-artifacts/oa-04-dependency-plan.md
)
for file in "${required[@]}"; do
  [[ -e "$file" ]] || { printf 'missing OA-04 artifact: %s\n' "$file" >&2; exit 1; }
done
[[ -x scripts/verify-oa04.sh ]] || { printf '%s\n' 'verify-oa04.sh is not executable' >&2; exit 1; }
printf '%s\n' 'ok: OA-04 artifacts exist'

python3 -I - <<'PY'
import json, subprocess
m=json.loads(subprocess.check_output(["cargo","metadata","--locked","--format-version","1"]))
r=next(p for p in m["packages"] if p["id"]==m["resolve"]["root"])
normal={(d["name"],d["req"],d["uses_default_features"],tuple(sorted(d["features"]))) for d in r["dependencies"] if d["kind"] is None}
assert ("axum","=0.8.9",False,("http1","json","tokio")) in normal
assert ("reqwest","=0.13.4",False,("json",)) in normal
assert ("tokio","=1.53.1",False,("io-util","net","process","rt","signal","sync","time")) in normal
assert ("clap","=4.6.6",False,("derive","error-context","help","std","usage")) in normal
dev=[d for d in r["dependencies"] if d["name"]=="tokio" and d["kind"]=="dev"]
assert len(dev)==1 and sorted(dev[0]["features"])==["macros","net","rt","sync","time"]
by_name={p["name"]:p for p in m["packages"]}
assert by_name["axum"]["version"]=="0.8.9"
assert by_name["reqwest"]["version"]=="0.13.4"
assert by_name["tokio"]["version"]=="1.53.1"
assert by_name["clap"]["version"]=="4.6.6"
forbidden_features={
 "axum":{"default","form","http2","multipart","query","ws"},
 "reqwest":{"default","default-tls","native-tls","rustls","cookies","brotli","charset","deflate","gzip","hickory-dns","http2","http3","multipart","socks","stream","system-proxy","zstd"},
 "tokio":{"full","fs","io-uring","io-std","parking_lot","rt-multi-thread","tracing"},
}
for name,bad in forbidden_features.items():
 node=next(n for n in m["resolve"]["nodes"] if n["id"]==by_name[name]["id"])
 assert not set(node["features"]) & bad, (name,node["features"])
forbidden={"openssl","native-tls","rustls","rustls-pki-types","tokio-rustls","hyper-rustls","cookie","cookie_store","async-compression","brotli","flate2","zstd","h2","h3","quinn","hickory-resolver","trust-dns-resolver","multer","rmcp","libp2p"}
assert not set(by_name) & forbidden, sorted(set(by_name)&forbidden)
PY
printf '%s\n' 'ok: frozen OA-04 pins/features selected and forbidden surfaces absent'

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
cargo tree --locked -e features | python3 -I -c 'import sys; sys.stdout.write(sys.stdin.read().replace(f"({sys.argv[1]})", "(<WORKSPACE>)"))' "$ROOT" > "$tmp/features.txt"
cmp "$tmp/features.txt" cargo-tree-oa05-features.txt
printf '%s\n' 'ok: locked OA-04 feature graph matches recorded evidence'

[[ "$(sha256sum tests/fixtures/oa04-protocol-golden.json | awk '{print $1}')" == "$OA04_FIXTURE_SHA256" ]]
[[ "$(sha256sum tests/fixtures/oa01-v1-golden.json | awk '{print $1}')" == "$OA01_FIXTURE_SHA256" ]]
[[ "$(sha256sum tests/fixtures/oa03-bundle-v1-golden.json | awk '{print $1}')" == "$OA03_FIXTURE_SHA256" ]]
git diff --exit-code "$OA01_COMMIT" -- tests/fixtures/oa01-v1-golden.json
cargo test --locked --test oa04_protocol protocol_fixture_is_frozen_canonical_and_reproducible
printf '%s\n' 'ok: OA-04 protocol fixture is frozen and OA-01/OA-03 fixtures are unchanged'

cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
printf '%s\n' 'ok: locked build, format, strict Clippy, and all workspace tests passed'

cargo test --locked --test oa04_auth --test oa04_protocol --test oa04_sync --test oa04_transport
printf '%s\n' 'ok: OA-04 authentication, protocol, synchronization, and transport matrices passed'

bash scripts/verify-oa01.sh
bash scripts/verify-oa02.sh
bash scripts/verify-oa03.sh
bash scripts/verify-oa04-dependencies.sh
printf '%s\n' 'ok: OA-01/OA-02/OA-03 regressions and the D-04-01 dependency probe verifier passed'
