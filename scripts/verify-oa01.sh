#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly BASELINE="53777ce3668708a5f1b668d25c2a461d04b9985e"
readonly FIXTURE_SHA256="799f326d20584b20f455d1b3027cc904848381761b6e591e81cacde0e46d7594"
cd "$ROOT"

required=(
  Cargo.toml Cargo.lock cargo-tree-oa01-features.txt
  src/lib.rs src/model.rs src/crypto.rs src/error.rs
  tests/fixtures/oa01-v1-golden.json tests/oa01_golden.rs
  tests/oa01_adversarial.rs scripts/verify-oa01.sh README.md
)
for file in "${required[@]}"; do
  [[ -e "$file" ]] || { printf 'missing OA-01 artifact: %s\n' "$file" >&2; exit 1; }
done
[[ -x scripts/verify-oa01.sh ]] || { printf '%s\n' 'verify-oa01.sh is not executable' >&2; exit 1; }
printf '%s\n' 'ok: OA-01 artifacts exist'

python3 -I - <<'PY'
import json
import subprocess
m = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"]))
root_id = m["resolve"]["root"]
assert root_id is not None
root = next(p for p in m["packages"] if p["id"] == root_id)
assert root["name"] == "contextmesh"
assert root["edition"] == "2024" and root["rust_version"] == "1.97"
deps = {d["name"]: d for d in root["dependencies"]}
expected = {
    "axum": ("=0.8.9", False, ["http1", "json", "tokio"]),
    "base64": ("=0.23.1", False, ["std"]),
    "blake3": ("=1.8.6", False, ["std"]),
    "ed25519-dalek": ("=3.0.0", False, ["fast", "zeroize"]),
    "getrandom": ("=0.4.3", False, []),
    "reqwest": ("=0.13.4", False, ["json"]),
    "serde": ("=1.0.229", True, ["derive"]),
    "serde_jcs": ("=0.2.0", True, []),
    "serde_json": ("=1.0.151", False, ["std", "float_roundtrip"]),
    "thiserror": ("=2.0.20", True, []),
    "turso": ("=0.7.2", False, []),
    "zeroize": ("=1.9.0", False, ["alloc"]),
}
for name, (req, defaults, features) in expected.items():
    dep = deps[name]
    assert dep["kind"] is None
    assert dep["req"] == req
    assert dep["uses_default_features"] is defaults
    assert sorted(dep["features"]) == sorted(features)
tokio_deps = [d for d in root["dependencies"] if d["name"] == "tokio"]
normal = next(d for d in tokio_deps if d["kind"] is None)
dev = next(d for d in tokio_deps if d["kind"] == "dev")
assert normal["req"] == "=1.53.1" and not normal["uses_default_features"]
assert sorted(normal["features"]) == ["net", "rt", "sync", "time"]
assert dev["req"] == "=1.53.1" and not dev["uses_default_features"]
assert sorted(dev["features"]) == ["macros", "net", "rt", "sync", "time"]
assert set(deps) == set(expected) | {"tokio"}
forbidden_direct = {"rand", "rand_core", "hex", "regex", "uuid", "serde_json_canonicalizer", "signature", "clap", "libp2p", "openssl", "native-tls", "rustls", "tokio-rustls", "hyper-rustls"}
assert not forbidden_direct.intersection(deps)
for name, version in {"turso": "0.7.2", "serde_jcs": "0.2.0", "ed25519-dalek": "3.0.0"}.items():
    assert any(p["name"] == name and p["version"] == version for p in m["packages"])
turso = next(p for p in m["packages"] if p["name"] == "turso" and p["version"] == "0.7.2")
turso_node = next(n for n in m["resolve"]["nodes"] if n["id"] == turso["id"])
assert not {"fts", "mimalloc", "sync"}.intersection(turso_node["features"])
assert not {"rusqlite", "libsqlite3-sys"}.intersection(p["name"] for p in m["packages"])
PY
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT
cargo tree --locked -e features | python3 -I -c 'import sys; sys.stdout.write(sys.stdin.read().replace(f"({sys.argv[1]})", "(<WORKSPACE>)"))' "$ROOT" > "$tmp_root/features.txt"
cmp "$tmp_root/features.txt" cargo-tree-oa04-features.txt
printf '%s\n' 'ok: exact dependencies and locked feature graph match'

actual_fixture_sha="$(sha256sum tests/fixtures/oa01-v1-golden.json | awk '{print $1}')"
[[ "$actual_fixture_sha" == "$FIXTURE_SHA256" ]] || {
  printf 'golden fixture changed without approved verifier update: %s\n' "$actual_fixture_sha" >&2
  exit 1
}
cargo test --locked --test oa01_golden checked_in_fixture_is_deterministically_reproducible
printf '%s\n' 'ok: frozen golden fixture checksum and regeneration match'

# OA-04 has since implemented the sync/http surfaces; provider and CLI
# modules remain deferred until OA-05/OA-06.
for file in src/provider.rs src/bin/contextmesh.rs src/bin/demo_agent.rs scripts/demo.sh; do
  git diff --exit-code "$BASELINE" -- "$file"
done
printf '%s\n' 'ok: OA-05/OA-06 provider and CLI surfaces remain deferred'

cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
printf '%s\n' 'ok: build, format, Clippy, and all workspace tests passed'

set +e
demo_output="$(bash scripts/demo.sh 2>&1)"
demo_code=$?
set -e
[[ $demo_code -eq 1 ]]
[[ "$demo_output" == 'OA-06 pending: the two-node Option A demo is not implemented in OA-00.' ]]
printf '%s\n' 'ok: OA-06 demo remains an explicit failure sentinel'
