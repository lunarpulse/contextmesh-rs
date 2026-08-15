#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

required=(
  Cargo.toml Cargo.lock rust-toolchain.toml README.md .gitignore
  cargo-tree-features.txt src/lib.rs src/model.rs src/crypto.rs src/error.rs
  src/store.rs src/sync.rs src/http.rs src/provider.rs
  src/bin/contextmesh.rs src/bin/demo_agent.rs tests/smoke.rs
  tests/fixtures/.gitkeep scripts/bootstrap-rust.sh scripts/verify-oa00.sh
  scripts/demo.sh
)
for file in "${required[@]}"; do
  [[ -e "$file" ]] || { printf 'missing required baseline file: %s\n' "$file" >&2; exit 1; }
done
for file in scripts/bootstrap-rust.sh scripts/verify-oa00.sh scripts/demo.sh; do
  [[ -x "$file" ]] || { printf 'script is not executable: %s\n' "$file" >&2; exit 1; }
done
expected_src=(src/bin/contextmesh.rs src/bin/demo_agent.rs src/crypto.rs src/error.rs src/http.rs src/lib.rs src/model.rs src/provider.rs src/store.rs src/sync.rs)
mapfile -t actual_src < <(find src -type f -print | sort)
[[ "$(printf '%s\n' "${expected_src[@]}" | sort)" == "$(printf '%s\n' "${actual_src[@]}")" ]] || {
  printf '%s\n' 'unexpected source-file surface:' "${actual_src[@]}" >&2
  exit 1
}
printf '%s\n' 'ok: approved OA-00 surfaces exist'

# Matrix: an unavailable rustup installer fails closed without invoking a
# distribution package manager or leaving a partial toolchain claim.
tmp_home="$(mktemp -d)"
trap 'rm -rf "$tmp_home"' EXIT
set +e
failure_output="$({
  HOME="$tmp_home"   CARGO_HOME="$tmp_home/.cargo"   RUSTUP_HOME="$tmp_home/.rustup"   PATH="/usr/bin:/bin"   NO_PROXY="127.0.0.1,localhost"   no_proxy="127.0.0.1,localhost"   HTTPS_PROXY= HTTP_PROXY= ALL_PROXY= https_proxy= http_proxy= all_proxy=   RUSTUP_INIT_URL="https://127.0.0.1:1/unavailable-rustup-init"     bash scripts/bootstrap-rust.sh
} 2>&1)"
failure_code=$?
set -e
[[ $failure_code -ne 0 ]] || { printf '%s\n' 'bootstrap failure test unexpectedly succeeded' >&2; exit 1; }
grep -Eq '127\.0\.0\.1|connect|Connection|curl' <<<"$failure_output" || {
  printf 'bootstrap failed for an unexpected reason:\n%s\n' "$failure_output" >&2; exit 1;
}
[[ ! -e "$tmp_home/.cargo/bin/rustc" && ! -e "$tmp_home/.cargo/bin/rustup" && ! -e "$tmp_home/.rustup/toolchains" ]] || {
  printf '%s\n' 'bootstrap failure left a partial toolchain installation' >&2; exit 1;
}
printf '%s\n' 'ok: unavailable Rust installer fails closed'

# Matrix: exercise the complete clean-home bootstrap orchestration through a
# controlled installer. The real-home checks below separately prove that the
# genuine pinned toolchain works on this host.
mock_root="$tmp_home/bootstrap-success"
mock_home="$mock_root/home"
mock_cargo="$mock_home/.cargo"
mock_rustup="$mock_home/.rustup"
mock_bin="$mock_root/bin"
mkdir -p "$mock_home" "$mock_bin"
cat > "$mock_root/rustup-init" <<'MOCK_INSTALLER'
#!/bin/sh
set -eu
mkdir -p "$CARGO_HOME/bin" "$RUSTUP_HOME"
cat > "$CARGO_HOME/bin/rustup" <<'MOCK_RUSTUP'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  toolchain)
    [[ "${2:-}" == install && "${3:-}" == 1.97.0 ]]
    touch "$RUSTUP_HOME/toolchain-1.97.0-installed"
    ;;
  component)
    [[ "${2:-}" == add ]]
    shift 2
    [[ "${1:-}" == --toolchain && "${2:-}" == 1.97.0 ]]
    shift 2
    for component in "$@"; do touch "$RUSTUP_HOME/component-$component-installed"; done
    ;;
  *) exit 64 ;;
esac
MOCK_RUSTUP
cat > "$CARGO_HOME/bin/rustc" <<'MOCK_RUSTC'
#!/usr/bin/env bash
set -euo pipefail
[[ -e "$RUSTUP_HOME/toolchain-1.97.0-installed" ]]
[[ "${1:-}" == +1.97.0 && "${2:-}" == --version ]]
printf '%s\n' 'rustc 1.97.0 (controlled bootstrap test)'
MOCK_RUSTC
cat > "$CARGO_HOME/bin/cargo" <<'MOCK_CARGO'
#!/usr/bin/env bash
set -euo pipefail
[[ -e "$RUSTUP_HOME/toolchain-1.97.0-installed" ]]
[[ "${1:-}" == +1.97.0 ]]
case "${2:-}" in
  --version) printf '%s\n' 'cargo 1.97.0 (controlled bootstrap test)' ;;
  fmt)
    [[ -e "$RUSTUP_HOME/component-rustfmt-installed" && "${3:-}" == --version ]]
    printf '%s\n' 'rustfmt 1.9.0-stable (controlled bootstrap test)'
    ;;
  clippy)
    [[ -e "$RUSTUP_HOME/component-clippy-installed" && "${3:-}" == --version ]]
    printf '%s\n' 'clippy 0.1.97 (controlled bootstrap test)'
    ;;
  *) exit 64 ;;
esac
MOCK_CARGO
chmod +x "$CARGO_HOME/bin/rustup" "$CARGO_HOME/bin/rustc" "$CARGO_HOME/bin/cargo"
MOCK_INSTALLER
chmod +x "$mock_root/rustup-init"
cat > "$mock_bin/curl" <<'MOCK_CURL'
#!/usr/bin/env bash
set -euo pipefail
output=''
while (($#)); do
  if [[ "$1" == -o ]]; then output="$2"; shift 2; else shift; fi
done
[[ -n "$output" ]]
cp "$OA00_MOCK_INSTALLER" "$output"
MOCK_CURL
chmod +x "$mock_bin/curl"
HOME="$mock_home" CARGO_HOME="$mock_cargo" RUSTUP_HOME="$mock_rustup" PATH="$mock_bin:/usr/bin:/bin" OA00_MOCK_INSTALLER="$mock_root/rustup-init" RUSTUP_INIT_URL="https://controlled.invalid/rustup-init"   bash scripts/bootstrap-rust.sh >/dev/null
[[ -x "$mock_cargo/bin/rustup" && -x "$mock_cargo/bin/rustc" && -x "$mock_cargo/bin/cargo" ]]
[[ -e "$mock_rustup/toolchain-1.97.0-installed" ]]
[[ -e "$mock_rustup/component-rustfmt-installed" && -e "$mock_rustup/component-clippy-installed" ]]
HOME="$mock_home" CARGO_HOME="$mock_cargo" RUSTUP_HOME="$mock_rustup"   "$mock_cargo/bin/rustc" +1.97.0 --version | grep -q '^rustc 1.97.0'
HOME="$mock_home" CARGO_HOME="$mock_cargo" RUSTUP_HOME="$mock_rustup"   "$mock_cargo/bin/cargo" +1.97.0 --version | grep -q '^cargo 1.97.0'
HOME="$mock_home" CARGO_HOME="$mock_cargo" RUSTUP_HOME="$mock_rustup"   "$mock_cargo/bin/cargo" +1.97.0 fmt --version | grep -q '^rustfmt 1.9.0-stable'
HOME="$mock_home" CARGO_HOME="$mock_cargo" RUSTUP_HOME="$mock_rustup"   "$mock_cargo/bin/cargo" +1.97.0 clippy --version | grep -q '^clippy 0.1.97'
printf '%s\n' 'ok: controlled clean-home bootstrap installed toolchain and components'

# Matrix: the approved user-local pinned toolchain and components are active.
# shellcheck source=/dev/null
. "$HOME/.cargo/env"
readonly EXPECTED_TOOLCHAIN="1.97.0-x86_64-unknown-linux-gnu"
[[ "$(rustup show active-toolchain | awk '{print $1}')" == "$EXPECTED_TOOLCHAIN" ]]
[[ "$(rustup run "$EXPECTED_TOOLCHAIN" rustc --version)" == rustc\ 1.97.0* ]]
[[ "$(rustup run "$EXPECTED_TOOLCHAIN" cargo --version)" == cargo\ 1.97.0* ]]
rustup run "$EXPECTED_TOOLCHAIN" cargo fmt --version | grep -q '^rustfmt 1.9.0-stable'
rustup run "$EXPECTED_TOOLCHAIN" cargo clippy --version | grep -q '^clippy 0.1.97'
[[ "$(command -v rustup)" == "$HOME/.cargo/bin/rustup" ]]
[[ "$(command -v rustc)" == "$HOME/.cargo/bin/rustc" ]]
[[ "$(command -v cargo)" == "$HOME/.cargo/bin/cargo" ]]
printf '%s\n' 'ok: pinned user-local Rust toolchain and components are active'

# Acceptance: exact package, feature, and forbidden-dependency state.
python3 - <<'PY'
import json
import subprocess
m = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"]))
root_id = m["resolve"]["root"]
assert root_id is not None
root = next(p for p in m["packages"] if p["id"] == root_id)
assert root["name"] == "contextmesh"
assert root["edition"] == "2024" and root["rust_version"] == "1.97"
deps = {d["name"]: d for d in root["dependencies"]}
assert set(deps) == {"turso", "tokio"}
turso = deps["turso"]
assert turso["req"] == "=0.7.2" and not turso["uses_default_features"] and not turso["features"]
tokio = deps["tokio"]
assert tokio["kind"] == "dev" and not tokio["uses_default_features"]
assert sorted(tokio["features"]) == ["macros", "rt"]
forbidden = {"rusqlite", "libsqlite3-sys"}
assert not forbidden.intersection(p["name"] for p in m["packages"])
turso_pkg = next(p for p in m["packages"] if p["name"] == "turso")
assert turso_pkg["version"] == "0.7.2"
turso_node = next(n for n in m["resolve"]["nodes"] if n["id"] == turso_pkg["id"])
assert not {"fts", "mimalloc", "sync"}.intersection(turso_node["features"])
PY
cargo tree --locked -e features > "$tmp_home/cargo-tree-features.txt"
cmp "$tmp_home/cargo-tree-features.txt" cargo-tree-features.txt
printf '%s\n' 'ok: locked Turso dependency and feature audit passed'

cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
printf '%s\n' 'ok: locked build, format, Clippy, and workspace tests passed'

# Matrix: explicitly identify the local Turso round-trip test in the output.
test_output="$(cargo test --workspace --locked --test smoke -- --exact in_memory_turso_write_read_round_trip 2>&1)"
grep -q 'test in_memory_turso_write_read_round_trip .* ok' <<<"$test_output"
printf '%s\n' 'ok: local Turso round trip passed'

# Matrix: the pre-OA-06 demo cannot create a false A8 success.
set +e
demo_output="$(bash scripts/demo.sh 2>&1)"
demo_code=$?
set -e
[[ $demo_code -eq 1 ]]
[[ "$demo_output" == 'OA-06 pending: the two-node Option A demo is not implemented in OA-00.' ]]
printf '%s\n' 'ok: OA-06 demo sentinel fails explicitly'
