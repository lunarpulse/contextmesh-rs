---
title: 'OA-00 Rust Toolchain and Turso Crate Baseline'
type: 'chore'
created: '2026-08-15'
status: 'done'
baseline_commit: 'NO_VCS'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/spec-signed-agent-context-dag.md'
  - '{project-root}/_bmad-output/planning-artifacts/option-a-delivery-plan.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Option A cannot be compiled or verified because this empty project has no Rust crate and its Rust tools are absent. OA-01 also needs stable module boundaries and a tested embedded Turso storage baseline before the signed event contract is implemented.

**Approach:** Install a user-local pinned Rust toolchain, create one contextmesh package matching the Option A code map, pin the stable turso crate with unnecessary top-level features disabled, capture its resolved dependency graph, and prove a local in-memory Turso database builds and executes. Keep later Option A behavior unimplemented; the demo must report that OA-06 is pending rather than simulate success.

## Boundaries & Constraints

**Always:** Install through rustup under the current user without sudo; pin Rust 1.97.0 with rustfmt and clippy; use edition 2024 and advertise only tested MSRV 1.97; keep one root package; produce Cargo.lock; use exact stable turso 0.7.2 with top-level defaults disabled; use only local embedded Turso; add only the minimal Tokio dev runtime needed for the async smoke test; make approved module and binary placeholders compile without warnings; record cargo tree evidence.

**Ask First:** Installing system packages, initializing version control, changing approved crate/module/bin names, enabling Turso Cloud sync or top-level fts/mimalloc, adding containers/services, selecting a Turso prerelease, or declaring a lower MSRV.

**Never:** Use rusqlite or a system SQLite library; treat Turso database replication as the Option A event-sync protocol; implement OA-01+ behavior; add Option B dependencies; claim the placeholder demo passes A8; require root privileges; silently use an unpinned distribution Rust package.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|---------------|----------------------------|----------------|
| Toolchain absent | No Rust commands or rustup directories | rustup installs pinned tools under $HOME/.cargo and the project resolves the pin | Network/install failure stops without system-package fallback or completion claims |
| Turso baseline | Locked dependencies and fresh in-memory database | Build succeeds; async smoke test creates, writes, and reads one row through local Turso | Build/API/transaction failure keeps OA-00 incomplete |
| Demo invoked early | scripts/demo.sh before OA-06 | States that the demo is not implemented and exits non-zero | Must not produce false A8 success |

</frozen-after-approval>

## Code Map

- rust-toolchain.toml -- pins Rust 1.97.0, minimal profile, rustfmt, and clippy; rustup is currently absent.
- Cargo.toml, Cargo.lock -- one contextmesh package, edition/MSRV, two binaries, exact turso 0.7.2 without top-level defaults, and minimal Tokio dev runtime.
- src/lib.rs, src/{model,crypto,error,store,sync,http,provider}.rs -- documented placeholder boundaries matching Option A; no OA-01+ behavior.
- src/bin/{contextmesh,demo_agent}.rs -- warning-free placeholder executables.
- tests/smoke.rs -- exercises turso::Builder::new_local(":memory:") asynchronously and verifies a write/read round trip.
- tests/fixtures/.gitkeep -- reserves OA-01 golden-vector fixtures.
- scripts/bootstrap-rust.sh, scripts/verify-oa00.sh -- repeatable user-local bootstrap plus matrix-level acceptance checks.
- scripts/demo.sh -- executable OA-06 sentinel that intentionally fails.
- README.md, .gitignore -- toolchain, Turso decision, dependency caveats, commands, scope, and exclusions.

## Tasks & Acceptance

**Execution:**
- [x] $HOME/.cargo, rust-toolchain.toml, scripts/bootstrap-rust.sh -- install rustup profile minimal, pin 1.97.0, add formatting/lint components, and verify resolution.
- [x] Cargo.toml, Cargo.lock, .gitignore -- define package/targets, exact Turso and minimal Tokio dev dependency; lock and fetch the graph.
- [x] src/ -- create approved warning-free module and binary placeholders without later-package behavior.
- [x] tests/smoke.rs, tests/fixtures/.gitkeep, scripts/verify-oa00.sh, scripts/demo.sh -- prove embedded Turso operation, installation failure behavior, and prevention of false completion.
- [x] README.md -- record edition/MSRV, environment activation, Turso feature/dependency audit, commands, and limitations.

**Acceptance Criteria:**
- Given a fresh shell, when $HOME/.cargo/env is sourced in this project, then rustc, cargo, rustfmt, and Clippy resolve to pinned 1.97.0.
- Given Cargo.lock, when locked build, format, Clippy, and tests run, then they pass without warnings and the local Turso round trip succeeds.
- Given the resolved graph, when cargo tree is inspected, then turso 0.7.2 is exact, its top-level fts, mimalloc, and sync features are absent, and no direct rusqlite dependency exists.
- Given the approved code map, when inspected, then every planned surface exists without OA-01+ behavior.
- Given scripts/demo.sh before OA-06, when invoked, then it reports pending implementation and exits non-zero.

## Spec Change Log

- **2026-08-15 — Human edit:** Replaced the proposed bundled rusqlite baseline with stable embedded Turso and required a dependency/feature audit.

## Design Notes

The current stable release is turso 0.7.2; 0.8.0-pre.4 is deliberately excluded. It publishes no upstream rust-version, so passing the pinned toolchain is local compatibility evidence, not an upstream MSRV guarantee. Disabling top-level defaults removes mimalloc and fts, while leaving sync disabled avoids optional HTTP/Tokio cloud-sync dependencies. Turso still unconditionally brings turso_core, turso_sdk_kit, and turso_sync_sdk_kit; the graph is materially large, includes build-time bindgen, and activates core storage/encryption facilities. The host has a C/C++ toolchain, CMake, and libclang 18, but OA-00 must compile and record the locked graph rather than infer compatibility. Local Turso is only the persistence engine; OA-04 remains the validated signed-event anti-entropy protocol.

## Verification

**Commands:**
- Source $HOME/.cargo/env and run rustc/cargo/rustfmt/clippy version commands -- expected: pinned 1.97.0 tools.
- cargo build --workspace --locked -- expected: library and both binaries compile.
- cargo tree --locked -e features -- expected: exact graph is inspectable and top-level fts, mimalloc, sync remain disabled.
- cargo fmt --all -- --check -- expected: no differences.
- cargo clippy --workspace --all-targets --locked -- -D warnings -- expected: no warnings.
- cargo test --workspace --locked -- expected: all tests pass, including local Turso SQL round trip.
- bash scripts/demo.sh -- expected in OA-00: OA-06-pending message and intentional non-zero exit.


## Suggested Review Order

**Dependency and toolchain boundary**

- Start with the exact Turso, edition, and minimal test-runtime contract.
  [`Cargo.toml:1`](../../Cargo.toml#L1)

- Pin every developer to the same formatter, linter, compiler, and Cargo release.
  [`rust-toolchain.toml:1`](../../rust-toolchain.toml#L1)

**Reproducible bootstrap and verification**

- Install the pinned user-local toolchain without root or distribution-package fallback.
  [`bootstrap-rust.sh:1`](../../scripts/bootstrap-rust.sh#L1)

- Enforce surfaces, bootstrap paths, dependencies, quality gates, and sentinel behavior.
  [`verify-oa00.sh:1`](../../scripts/verify-oa00.sh#L1)

**Crate boundary and storage proof**

- Reserve approved Option A modules without implementing OA-01 or later behavior.
  [`lib.rs:1`](../../src/lib.rs#L1)

- Prove local embedded Turso can create, write, query, and exhaust results.
  [`smoke.rs:6`](../../tests/smoke.rs#L6)

**Operator guidance**

- Explain Turso trade-offs, prerequisites, scope exclusions, and verification workflow.
  [`README.md:1`](../../README.md#L1)
