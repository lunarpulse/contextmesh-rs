# OA-07 Claim Audit

Scope: every substantive claim in README.md, classified as demonstrated
(exact proving artifact named), limited (true within a stated bound), or
removed/absent (correctly not claimed). Independent delegated claims audit
2026-08-17 found zero blockers; its classification is reproduced here in
condensed form with spot-verification against code and tests.

## Demonstrated

| Claim | Proof |
|---|---|
| Option A stack OA-01..OA-06 implemented and verified | verify-oa00..verify-oa06 chain; spec-oa-01..06 present |
| Pinned Rust 1.97.0, edition 2024, bootstrap script | rust-toolchain.toml; scripts/bootstrap-rust.sh; verify-oa00 |
| Schema v1, per-connection FK enforcement | enable_foreign_keys + PRAGMA recheck (src/store.rs); oa02_schema tests |
| Explicit IMMEDIATE transactions, commit/rollback | write()/finish_transaction (src/store.rs); oa02_rollback tests |
| Append-only allowlist; admission proves neither truth nor relevance | authors_no_update/delete triggers; README non-claim statements |
| Admission re-verifies wire, policy, parents; envelope authoritative | oa02_rollback, oa02_schema tests |
| Local refs CAS-only; stale writers leave no rows; peer namespace separate | oa02_concurrency; one_way_pull_paginates_converges_and_preserves_local_refs |
| 2..64-parent merges, sorted parents including target head, no selector | oa03_dag tests; public API surface |
| Deterministic bounded projection (100k events / 64 MiB / 256 heads), SQL-order independent | iterative_projection_is_unique_and_strictly_bounded |
| Bundle v1 strict bounds, one-transaction import, peer-namespace-only, zero-insert repeat | oa03_bundle tests; oa04_sync tests |
| verify_full reports corruption, never repairs | full_verify_passes_restart_and_reports_corruption_without_repair |
| Token format/sources, hash-only server state, fixed-size compare | oa04_auth tests; http.rs |
| One generic non-secret error shape, random request ID | authentication_matrix_returns_one_generic_shape; oa04_protocol |
| IP-literal http only; no DNS/HTTPS/proxies/redirects | hostile_responses_stay_bounded_and_redirects_are_never_followed; proxy_environment_is_ignored_by_the_client |
| Loopback default, acknowledged non-loopback + fixed warning | loopback_is_default...; sync_server_rejects_unacknowledged_non_loopback_bind |
| Frozen bounds and independent timeouts; slowloris cut pre-handler | oa04_transport boundary/timeout tests |
| /v1/refs fingerprint; cursor-bound immutable export plan | protocol fixture tests; pagination_plan_is_immutable_while_refs_move |
| Page-at-a-time admission import; ref replace after full transfer; orphans on late failure | invalid_late_page_leaves_refs_unchanged_with_earlier_orphans |
| OA-04 dependency delta exactly Axum 0.8.9 + Reqwest 0.13.4 + approved Tokio features | verify-oa04-dependencies probe verifier |
| Provider request-before-call, sole-parent linked result, no tx across call, detached retention | oa05_provider tests (05-P01..P05) |
| CommandProvider direct exec, never a shell, bounded JSONL, 30 s kill | command_provider_* tests |
| demo_agent echo-only, no tool execution, hostile-line safety | oa05_jsonl tests |
| Key/token custody: atomic 0600, symlink-rejecting, repair-only; no exposure anywhere | oa05_keys tests; secrets_never_reach_outputs; oa06 secret scans |
| CLI: one canonical JSON document, frozen exit classes, secrets never echoed | oa05-cli-golden.json snapshot matrix |
| Demo: seventeen stages, CLI-only nodes, fresh secrets, lifecycle, byte-identical exports, restart, idempotence, one-byte tamper rejection, public-ID PASS | demo.sh; oa06_demo tests incl. fresh_checkout_demo_passes |
| Demo harness cleanup/retention/fault-hook discipline | oa06_demo lifecycle tests; shell audit layer |

## Limited (true within a stated bound)

| Claim | Bound stated |
|---|---|
| One process per database file (frozen Turso engine) | README demo section; spec-oa-06 decision; daemon choreography |
| Plaintext HTTP, no confidentiality; tunnel/VPN guidance | README network deployment guidance |
| Append-only authorization; no revocation/workspace policy | README claims section; deferred scope |
| No exactly-once provider delivery; crash windows queryable only | README provider section |
| Advisory scan reflects advisory-db commit 69f93e1d (2026-08-12) | evidence limitations; offline gate re-checks closure count |
| cfg_block 0.1.1 license from repository declaration, not metadata | evidence accepted finding |
| serde/serde_jcs/thiserror keep default features (std-only) | supply-chain audit finding 5; defaults are std-only, no forbidden features |
| cargo-tree-oa01-features.txt rewritten at OA-02 (now equals OA-02 graph) | supply-chain audit finding 7; historical file, never compared by gates |
| Projection stack memory can transiently exceed the 64 MiB wire bound while remaining bounded (~O(events x parents) frames) | graph audit finding 7; bounded, non-overflowing |
| Hyper parser limits may front-run some application guards; accepted sockets bounded by the 5 s pre-header timer | transport audit findings 11-12 |
| pending_invocations over-reports off-branch requests (safe direction); demo_agent raw stdout sanitization differs from durable records (re-sanitized at recording) | provider audit findings 6, 10; fail-closed |
| invocation ancestry staged up to projection bounds before the 1024 cap rejects | provider audit finding 15; bounded transient |
| Database listings outside snapshots are not point-in-time consistent (each row still validated) | database audit finding 10 |
| Rustup installer fetched over TLS but not hash-pinned | supply-chain audit finding 9; standard rustup practice, fails closed |

## Removed / absent (correctly not claimed)

- A2A or ACP protocol compliance; agent interoperability.
- Semantic context selection or relevance; "verified truth" of content.
- Exactly-once delivery; revocation; Byzantine/consensus agreement.
- Confidentiality, encryption at rest or in transit, TLS management.
- Availability or DoS resistance beyond the frozen bounds.
- Multi-writer concurrency across processes on one database.

## Prohibited statements

No prohibited statement found in README.md, the specs, or the evidence:
every capability sentence names its proving artifact or its bound; the
non-claims section matches the frozen spec wording; no internal
contradiction between README sections or between README and specs was
found by the independent audit layer.

Verdict: claims audit release-ready; documentation claims only
demonstrations.
