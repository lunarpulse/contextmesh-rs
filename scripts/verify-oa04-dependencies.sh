#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly PROBE="$ROOT/_bmad-output/implementation-artifacts/oa04-dependency-probe"
readonly TARGET="$ROOT/target/oa04-dependency-probe-verify"
export CARGO_TARGET_DIR="$TARGET"
export PATH="$HOME/.cargo/bin:$PATH"

required=(
  Cargo.toml Cargo.lock src/main.rs cargo-metadata.json cargo-tree.txt
  cargo-tree-features.txt rustc-version.txt cargo-version.txt audit.txt SHA256SUMS
)
for file in "${required[@]}"; do
  [[ -f "$PROBE/$file" ]] || { printf 'missing OA-04 probe evidence: %s\n' "$file" >&2; exit 1; }
done
(
  cd "$PROBE"
  sha256sum -c SHA256SUMS
)
printf '%s\n' 'ok: OA-04 dependency-probe evidence checksums match'

[[ "$(rustc --version | awk '{print $2}')" == '1.97.0' ]]
[[ "$(cargo --version | awk '{print $2}')" == '1.97.0' ]]

python3 -I - "$PROBE" <<'PY'
import json, pathlib, subprocess, sys
probe=pathlib.Path(sys.argv[1])
m=json.loads(subprocess.check_output([
    "cargo","metadata","--locked","--format-version","1",
    "--manifest-path",str(probe/"Cargo.toml")
]))
root=next(p for p in m["packages"] if p["id"]==m["resolve"]["root"])
actual={
 (d["name"],d["req"],d["uses_default_features"],tuple(sorted(d["features"])))
 for d in root["dependencies"] if d["kind"] is None
}
expected={
 ("axum","=0.8.9",False,("http1","json","tokio")),
 ("blake3","=1.8.6",False,("std",)),
 ("clap","=4.6.6",False,("derive","error-context","help","std","usage")),
 ("reqwest","=0.13.4",False,("json",)),
 ("serde","=1.0.229",True,("derive",)),
 ("tokio","=1.53.1",False,("macros","net","process","rt","signal","sync","time")),
}
assert actual == expected, (actual, expected)
by_name={p["name"]:p for p in m["packages"]}
assert {n:str(by_name[n]["version"]) for n in ("axum","reqwest","clap","tokio","blake3")} == {
 "axum":"0.8.9","reqwest":"0.13.4","clap":"4.6.6","tokio":"1.53.1","blake3":"1.8.6"
}
forbidden_features={
 "axum":{"default","form","http2","multipart","query","ws"},
 "reqwest":{"default","default-tls","native-tls","rustls","cookies","brotli","charset","deflate","gzip","hickory-dns","http2","http3","multipart","socks","stream","system-proxy","zstd"},
 "clap":{"default","color","suggestions","unicode","env","wrap_help"},
 "tokio":{"full","fs","io-uring","io-std","io-util","parking_lot","rt-multi-thread","tracing"},
}
for name,bad in forbidden_features.items():
 node=next(n for n in m["resolve"]["nodes"] if n["id"]==by_name[name]["id"])
 assert not set(node["features"]) & bad, (name,node["features"])
forbidden={"openssl","native-tls","rustls","rustls-pki-types","tokio-rustls","hyper-rustls","cookie","cookie_store","async-compression","brotli","flate2","zstd","h2","h3","quinn","hickory-resolver","trust-dns-resolver","multer","rmcp","libp2p"}
assert not set(by_name) & forbidden, sorted(set(by_name)&forbidden)
PY
printf '%s\n' 'ok: exact pins/features selected and forbidden dependency surfaces absent'

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cargo tree --manifest-path "$PROBE/Cargo.toml" --locked | sed "s#$PROBE#<PROBE>#g" > "$tmp/tree.txt"
cargo tree --manifest-path "$PROBE/Cargo.toml" --locked -e features | sed "s#$PROBE#<PROBE>#g" > "$tmp/features.txt"
cmp "$tmp/tree.txt" "$PROBE/cargo-tree.txt"
cmp "$tmp/features.txt" "$PROBE/cargo-tree-features.txt"
printf '%s\n' 'ok: complete locked normal and feature trees match recorded evidence'

cargo fmt --manifest-path "$PROBE/Cargo.toml" --all -- --check
cargo build --manifest-path "$PROBE/Cargo.toml" --locked --all-targets
cargo clippy --manifest-path "$PROBE/Cargo.toml" --locked --all-targets -- -D warnings
output="$(HTTP_PROXY=http://127.0.0.1:1 HTTPS_PROXY=http://127.0.0.1:1 cargo run --manifest-path "$PROBE/Cargo.toml" --locked --quiet)"
[[ "$output" == 'oa04 dependency probe passed' ]]
printf '%s\n' 'ok: Rust 1.97 server/client/runtime/process/signal/CLI/auth probe passed'
