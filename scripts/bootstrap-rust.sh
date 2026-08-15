#!/usr/bin/env bash
set -euo pipefail

readonly TOOLCHAIN="1.97.0"
readonly RUSTUP_INIT_URL="${RUSTUP_INIT_URL:-https://sh.rustup.rs}"
readonly CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
readonly RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export CARGO_HOME RUSTUP_HOME

if [[ ! -x "$CARGO_HOME/bin/rustup" ]]; then
  installer="$(mktemp)"
  trap 'rm -f "$installer"' EXIT
  curl --proto '=https' --tlsv1.2 --fail --silent --show-error \
    --connect-timeout 10 --max-time 120 "$RUSTUP_INIT_URL" -o "$installer"
  sh "$installer" -y --no-modify-path --profile minimal \
    --default-toolchain "$TOOLCHAIN"
fi

export PATH="$CARGO_HOME/bin:$PATH"
rustup toolchain install "$TOOLCHAIN" --profile minimal
rustup component add --toolchain "$TOOLCHAIN" rustfmt clippy

rustc "+$TOOLCHAIN" --version
cargo "+$TOOLCHAIN" --version
cargo "+$TOOLCHAIN" fmt --version
cargo "+$TOOLCHAIN" clippy --version
