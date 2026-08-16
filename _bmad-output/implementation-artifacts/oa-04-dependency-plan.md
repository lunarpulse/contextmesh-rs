---
title: 'OA-04 Dependency Selection Plan and D-04-01 Probe Record'
type: 'dependency-plan'
created: '2026-08-16'
status: 'frozen'
decision: 'D-04-01'
baseline_commit: '93d1ca122eec25d64d9d38352faf87900d3bef30'
rust_toolchain: '1.97.0'
---

# OA-04 Dependency Selection Plan and D-04-01 Probe Record

## Decision and scope

D-04-01 authorized a mandatory two-stage selection, not immediate manifest edits. This record closes the first stage: the approved candidates were compiled and exercised in a disposable Rust 1.97 project, their exact metadata and feature graph were captured, and forbidden capabilities were audited. The root Cargo.toml and Cargo.lock were not changed by this probe.

The successful candidates below are now the exact pins authorized for their owning package. A version substitution, added default feature, added direct dependency, or broader feature set requires approval rather than silent selection.

## Frozen selections

| Dependency | Exact pin | Defaults | Direct features | First owner |
|---|---:|---|---|---|
| Tokio | =1.53.1 | off | OA-04 normal: net, rt, sync, time; OA-04 dev also macros; add process, signal only when OA-05/OA-06 code uses them | shared runtime |
| Axum | =0.8.9 | off | http1, json, tokio | OA-04 |
| Reqwest | =0.13.4 | off | json | OA-04 |
| Clap | =4.6.6 | off | derive, std, help, usage, error-context | OA-05 |
| BLAKE3 | existing =1.8.6 | off | existing std | OA-01/OA-04 |

Clap, Tokio process, and Tokio signal passed the required preflight but are not to be added merely because they were probed. Cargo ownership remains package-minimal: OA-04 adds only what OA-04 uses, and OA-05 adds its already-approved CLI/process/signal selection when that package begins.

No new constant-time-comparison crate is selected. BLAKE3 1.8.6 is already a direct pinned dependency, and its documented Hash::eq implementation performs constant-time comparison. OA-04 will compare fixed-size BLAKE3 hashes of authorization-header bytes and will not compare raw bearer-token strings with ordinary equality.

No direct Hyper, Hyper-util, Tower, Tower-HTTP, URL, bytes, futures, TLS, resolver, compression, cookie, multipart, or agent-protocol dependency is approved. Transitive packages remain lockfile-controlled implementation details and may not be imported directly without approval and a dependency-plan update.

## Probe project and representative paths

The committed disposable project is under:

    _bmad-output/implementation-artifacts/oa04-dependency-probe/

It exercises, on pinned Rust 1.97.0:

1. a current-thread Tokio runtime;
2. Tokio TCP binding, task spawn, synchronization, timeout, process execution, and signal paths;
3. an Axum HTTP/1 JSON server on an ephemeral loopback listener with graceful shutdown;
4. a Reqwest JSON client forced to HTTP/1, with 1-second connect and 2-second request limits;
5. explicit Policy::none() redirect handling and no_proxy() while hostile proxy environment variables point to an unusable endpoint;
6. Clap derive, standard parsing, generated help, usage, and error-context paths; and
7. fixed-size BLAKE3 constant-time hash equality for matching and mismatching credentials.

The probe passed cargo fmt --check, locked build, strict Clippy, and execution. Its output was exactly: oa04 dependency probe passed.

## MSRV and exact resolution

The probe ran with:

- rustc 1.97.0 (2d8144b78 2026-07-07);
- cargo 1.97.0 (c980f4866 2026-06-30); and
- host x86_64-unknown-linux-gnu.

Declared target MSRVs reported by Cargo metadata are Axum 1.80, Reqwest 1.85.0, Clap 1.85, and Tokio 1.71. BLAKE3 does not publish a rust-version field in this selected release, so successful compilation and execution on the pinned toolchain are the evidence. The disposable lock contains the Rust-1.97-compatible transitive resolution; in particular Cargo selected matchit 0.8.4 although a newer release existed.

The recorded probe resolution contains 110 metadata packages across host and conditional targets. Cargo.lock, complete Cargo metadata, the normal tree, and the full feature tree are retained rather than summarized away.

## Feature and forbidden-capability audit

The exact direct features observed were:

- axum/http1,json,tokio;
- reqwest/json;
- clap/derive,error-context,help,std,usage;
- the combined Tokio probe paths macros,net,process,rt,signal,sync,time; and
- blake3/std.

The metadata audit found none of the forbidden direct features and none of the forbidden packages for TLS, cookies, compression, HTTP/2/3, multipart, proxy discovery, alternate DNS, or agent protocols. In particular there is no OpenSSL/native-TLS/Rustls stack, h2, h3, QUIC, cookie store, async compression, Hickory resolver, Multer, RMCP, or libp2p.

Reqwest 0.13.4 unconditionally resolves internal hyper-util/client-proxy and tower-http/follow-redirect support even with defaults disabled. This is not system proxy discovery: reqwest/system-proxy and its platform discovery packages are absent. OA-04 must still call ClientBuilder::no_proxy(), redirect(Policy::none()), and http1_only() in production, as the successful hostile-proxy and redirect probe did. Removing those internal transitive facilities would require an unapproved client/version substitution.

Reqwest Response::chunk() is available without its optional stream feature, and Axum body::to_bytes(body, limit) is available under the selected set. OA-04 therefore has bounded incremental response reads and bounded request-body reads without adding a futures or stream feature.

## Captured evidence

The probe directory contains:

- the exact disposable Cargo.toml, Cargo.lock, and src/main.rs;
- rustc-version.txt and cargo-version.txt;
- cargo-metadata.json with local absolute paths normalized;
- cargo-tree.txt and cargo-tree-features.txt;
- audit.txt with selected versions, enabled features, proxy/redirect results, and forbidden checks; and
- SHA256SUMS protecting all recorded files.

Run bash scripts/verify-oa04-dependencies.sh to recheck checksums, exact direct selections, forbidden features/packages, strict quality, hostile proxy behavior, redirect behavior, and the representative executable.

## Manifest-change gate

Before OA-04 changes the root manifest:

1. this plan and the OA-04 implementation specification must be frozen and committed together;
2. the root dependency edit must use the exact pins/default settings above;
3. OA-04 must not add Clap, Tokio process/signal, or any unlisted direct crate before code needs them;
4. the post-edit workspace metadata and full feature tree must be captured as cargo-tree-oa04-features.txt;
5. the OA-03 Bundle fixture and OA-01/OA-02/OA-03 verifiers must remain unchanged and pass; and
6. any resolution drift that activates a forbidden feature/package blocks OA-04.

## Verdict

**D-04-01 preflight passed.** Axum 0.8.9, Reqwest 0.13.4, Clap 4.6.6, and Tokio 1.53.1 are compatible with pinned Rust 1.97 under the exact minimized feature directions recorded here. The pins are frozen; implementation and root manifest changes remain a separate OA-04 commit phase.
