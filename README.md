# contextmesh

contextmesh is currently the **OA-00 toolchain and embedded-storage baseline**
for Option A (verifiable distributed agent history). The approved module and
binary surfaces exist, but OA-01+ behavior is intentionally not implemented.
In particular, this baseline does not claim to pass the final Option A A8 gate.

## Rust toolchain

The project pins Rust **1.97.0**, the minimal rustup profile, rustfmt, and
Clippy in rust-toolchain.toml. It uses Rust edition **2024** and advertises
only the tested MSRV **1.97**. A lower MSRV has not been tested or declared.

Rustup and the toolchain are installed under the current user's home directory;
no root privileges or distribution Rust package are required. The repeatable
bootstrap fails closed if the official installer cannot be fetched:

    bash scripts/bootstrap-rust.sh

In a fresh shell, expose the user-local tools and let the project pin select
the toolchain:

    . "$HOME/.cargo/env"
    rustc --version
    cargo --version
    rustfmt --version
    cargo clippy --version

## Embedded Turso decision and dependency audit

OA-00 pins exactly turso = 0.7.2 with default-features = false. This keeps
the top-level Turso fts and mimalloc defaults disabled; the top-level
sync feature is also not enabled. Only local embedded Turso is used. Turso
Cloud sync/database replication is outside this baseline and must not replace
the validated signed-event anti-entropy protocol planned for OA-04.

The only development dependency is Tokio with its macros and current-thread
rt features, which are needed by the asynchronous integration smoke test.
There is no resolved `rusqlite` or `libsqlite3-sys` dependency. OA-00 does not claim that arbitrary future native dependencies cannot link SQLite.

Turso 0.7.2 does not publish an upstream rust-version, so a successful build
with this project's pin is local compatibility evidence, not an upstream MSRV
guarantee. Even with the top-level optional features disabled, Turso
unconditionally brings turso_core, turso_sdk_kit, and
turso_sync_sdk_kit. The locked graph is materially large, includes
build-time bindgen, and activates core storage/encryption facilities. It is
captured in Cargo.lock and can be audited with:

    cargo tree --locked -e features

The bootstrap itself requires Bash, curl, mktemp, TLS CA certificates, and network access to the configured rustup installer. Overriding `RUSTUP_INIT_URL`, `CARGO_HOME`, or `RUSTUP_HOME` changes the trusted installer or installation location and is intended for controlled testing/automation.

The host also needs the native build prerequisites required by the locked graph (including a C/C++ toolchain, CMake, and libclang). OA-00 does not
install system packages or infer compatibility; its locked build and smoke test
are the evidence.

## Layout and scope

- src/model.rs, crypto.rs, and error.rs reserve the OA-01 contract.
- src/store.rs reserves OA-02/OA-03 local DAG persistence.
- src/sync.rs and http.rs reserve OA-04 event synchronization/transport.
- src/provider.rs and both binaries reserve OA-05 provider/CLI work.
- tests/fixtures/ is reserved for OA-01 golden vectors.
- tests/smoke.rs is the only implemented behavior: it creates a fresh
  in-memory local Turso database, creates a table, writes one row, reads it
  back, and checks the result.

Both placeholder binaries print a pending message and exit unsuccessfully, so automation cannot mistake them for implemented commands.

No signed-event contract, cryptography, persistent DAG/ref behavior, provider
execution, HTTP service, multi-node synchronization, semantic context
selection, Option B dependency, A2A/ACP integration, or Turso Cloud sync is
implemented in OA-00.

## Verification

From the repository root after sourcing the rustup environment:

    . "$HOME/.cargo/env"
    cargo build --workspace --locked
    cargo tree --locked -e features
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo test --workspace --locked
    bash scripts/verify-oa00.sh

The matrix verifier exercises a controlled clean-home bootstrap, a failed-installer path, the genuine pinned local toolchain, exact dependency and feature boundaries, every quality command, the embedded Turso round trip, and the intentional demo failure.

The final script is deliberately a sentinel until OA-06:

    bash scripts/demo.sh

It prints an OA-06 pending message and exits non-zero. That intentional
failure prevents this placeholder baseline from reporting false A8 success.
