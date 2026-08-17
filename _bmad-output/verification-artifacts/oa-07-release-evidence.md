# OA-07 Option A Release Evidence

candidate-commit: ebd989656093ffc239fec097feb2f13656b17d57 (spec freeze; the parent of the evidence commit)
procedure-tree: 2dfb565aac24648f0e2b923d7d79be12a1e40edc (tree of the candidate commit; the evidence commit adds only gate, evidence, claim-audit, spec-evidence, and README status files on top)
gate: scripts/verify-oa07.sh (deterministic, non-recording, offline)
verdict: complete
option-b-gate: unblocked-by-complete-verdict (no Option B work started)

## Environment and toolchain

| Item | Value |
|---|---|
| rustc | 1.97.0 (2d8144b78 2026-07-07) |
| cargo | 1.97.0 (c980f4866 2026-06-30) |
| toolchain source | rust-toolchain.toml override (1.97.0-x86_64-unknown-linux-gnu) |
| native prerequisite | cc present and usable (Turso bundled sqlite3.c build) |
| env overrides | none (RUSTC/RUSTFLAGS/CARGO_BUILD_*/CARGO_TARGET_DIR unset in gate) |
| worktree | clean at procedure start and at gate rerun |

## Supply-chain audit

- Direct dependencies (normal): turso =0.7.2 (no features), tokio =1.53.1
  (io-util, net, process, rt, signal, sync, time), clap =4.6.6 (derive,
  error-context, help, std, usage), axum =0.8.9 (http1, json, tokio),
  reqwest =0.13.4 (json), blake3 =1.8.6 (std), serde =1.0.229 (default +
  derive), serde_json, serde_jcs =0.2.0, ed25519-dalek =3.0.0. Dev: tokio
  =1.53.1 (macros, net, rt, sync, time).
- dependency-closure: 320
- Cargo.lock sha256: e194c2923e697c657e1d1019dbb00718315f529c89df3e3a1374f061fe6609ef
- Locked feature graph: byte-identical to cargo-tree-oa05-features.txt
  (unchanged since OA-05; re-asserted by the gate).
- Forbidden-capability audit: no TLS stacks, HTTP/2/3, QUIC, cookies,
  compression, DNS resolvers, shells, libp2p/rusqlite/crypto-random
  duplicates in the closure.
- advisory-database: RustSec advisory-db commit 69f93e1d081d8b6fbee010e48f0b5e0d13661415 (2026-08-12), 1216 advisories loaded
- vulnerabilities: 0 (cargo audit exit 0, zero findings; cargo-audit 0.22.2)
- Licenses: 319 dependency crates; every crate offers at least one fully
  permissive alternative (MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib,
  BSL-1.0, CC0-1.0, MIT-0, Unicode-3.0, Unlicense). Accepted finding:
  cfg_block 0.1.1 declares no license in metadata (a 2-file proc-macro
  helper; its repository pluots/cfg_block states MIT OR Apache-2.0);
  recorded as accepted rather than verified in-band.

## Procedure commands and results

| Step | Command | Result |
|---|---|---|
| 4 | bash scripts/verify-oa06.sh (chains verify-oa00, 01, 02, 03, 04, 04-dependencies, 05, 06) | exit 0, 155 ok checkpoints (deterministic; executed inside scripts/verify-oa07.sh on the evidence commit) |
| 5 | locked build / rustfmt / Clippy -D warnings / workspace tests / demo | covered by the chain on this tree (see sequencing note); 86-item test inventory green; demo PASS |
| 6 | fresh-target offline repetition (CARGO_NET_OFFLINE=true, fresh --target-dir, fresh demo runtime root) | exit 0: build, Clippy, full tests, demo PASS (executed inside scripts/verify-oa07.sh on the evidence commit) |
| 7 | secret and runtime-artifact scan | passed: no tracked runtime artifacts; no db/WAL/token/key files outside target/; token1_ confined to src/ and tests/; evidence/claims/spec/README secret-free |

## Checksums

| Artifact | sha256 |
|---|---|
| tests/fixtures/oa01-v1-golden.json | 799f326d20584b20f455d1b3027cc904848381761b6e591e81cacde0e46d7594 |
| tests/fixtures/oa03-bundle-v1-golden.json | 7752cd4b2443beb7d22d84e3fa542cc74b261f34f0b70d016ec0a1d05a372e6a |
| tests/fixtures/oa04-protocol-golden.json | 71fe501621fec368e14cc9521e58bcb992df39d475c81afe17edf89d323cbbf2 |
| tests/fixtures/oa05-cli-golden.json | d45cf8394cf1df8f64cbad5850de395ea5735e5f2cf116b61151a68f9995b4a6 |
| Cargo.lock | e194c2923e697c657e1d1019dbb00718315f529c89df3e3a1374f061fe6609ef |
| demo transcript (exact run) | b838045990ca1746afb0cd2c135ac681408b45664589fd91784a15e9c975a0d7 |
| demo transcript (id-redacted, stable) | e740afdd9033d56a76b7ef54ef16e78639a9dc27677fa71c55ba4d3cfdac597d |

## A1-A8 evidence matrix

### A1 identity
Owners: OA-01 (unchanged since f61c4f0).
Proof: oa01-v1-golden.json checksum frozen since f61c4f0 (verify-oa01
asserts checksum + git diff); tests/oa01_golden.rs
(checked_in_fixture_is_deterministically_reproducible,
fixed_vector_recomputes_and_verifies_independently,
equivalent_json_produces_identical_body_id_and_signature);
tests/oa01_adversarial.rs (every_signed_field_mutation_is_rejected,
parser_rejects_equivalent_but_noncanonical_and_hostile_input,
strict_ed25519_rejects_noncanonical_s_and_small_order_keys,
duplicate_keys_are_rejected_at_every_depth,
unicode_is_not_normalized_but_escape_aliases_are,
typed_text_encodings_are_canonical_and_exact,
number_boundaries_negative_zero_and_exponent_aliases_are_enforced,
rfc_8785_serialization_example_matches,
rfc_8785_utf16_property_order_matches). Cross-gate: canonical wire
retention (02-A03) also proves A1 via
lifecycle_idempotence_namespaces_and_restart.

### A2 admission
Owners: OA-02/OA-03.
Proof: tests/oa02_rollback.rs (wrong_genesis_and_cross_context_parent_reject,
policy_parent_and_stale_failures_leave_history_unchanged — includes
unauthorized-author atomic reject for A2/A7),
tests/oa02_schema.rs (provisioning_mismatch_and_external_collision_are_typed,
newer_and_incomplete_schemas_fail_closed,
database_triggers_protect_immutable_rows),
tests/oa02_store.rs (malformed_author_and_signature_are_typed_failures via
store error mapping; names_are_strict_and_bounded),
tests/oa03_bundle.rs (bundle_parser_rejects_unknown_duplicate_version_order_and_limits,
malformed_inputs_never_panic_or_return_partial_events), demo stage 16:
one-byte signature mutation rejected atomically with frozen exit class 9,
zero state change, no tamper-node peer refs.

### A3 DAG/refs
Owners: OA-02/OA-03/OA-05.
Proof: tests/oa03_dag.rs (create_join_append_fork_merge_project_and_restart,
iterative_projection_is_unique_and_strictly_bounded,
merge_boundaries_and_invalid_shapes_are_atomic,
parent_kind_and_depth_boundaries_are_enforced,
payload_body_and_wire_size_boundaries_are_enforced),
tests/oa02_concurrency.rs (independently_opened_stores_produce_one_cas_winner),
tests/oa05_provider.rs (request_precedes_call_and_links_response,
post_execution_conflict_retains_detached_result — provider history),
demo stages 7-12 (explicit branch, distinct chains, two-parent merge,
six-ancestor projection counted once each, byte-identical exports).

### A4 persistence
Owners: OA-02/OA-03.
Proof: tests/oa02_store.rs (lifecycle_idempotence_namespaces_and_restart),
tests/oa03_verify.rs (full_verify_passes_restart_and_reports_corruption_without_repair),
tests/oa02_schema.rs (database_triggers_protect_immutable_rows),
tests/oa03_bundle.rs (canonical_bundle_fixture_is_frozen_and_independently_verified),
demo stages 13-14 (stop/restart on the same databases, verify valid on
both, projections and peer refs unchanged); fresh-target repetition
proves no cached-artifact dependence. Cross-gate: canonical wire
retention (02-A03) proves A4 via the same lifecycle test.

### A5 sync
Owners: OA-04/OA-06.
Proof: tests/oa04_sync.rs (one_way_pull_paginates_converges_and_preserves_local_refs,
pagination_plan_is_immutable_while_refs_move,
invalid_late_page_leaves_refs_unchanged_with_earlier_orphans,
unreachable_peer_times_out_boundedly_then_retry_converges,
lifecycle_idempotence_namespaces_and_restart),
tests/oa04_protocol.rs (protocol_fixture_is_frozen_canonical_and_reproducible,
protocol_cardinality_boundaries_are_exact), demo stages 6, 10, 12, 15
(genesis pull with zero implicit local movement; bidirectional exchange
with local-main retention and namespaced peer refs; merge convergence;
zero-insert re-pull with pages>=1 and remote_refs_updated=0).

### A6 provider
Owners: OA-05/OA-06.
Proof: tests/oa05_provider.rs (request_precedes_call_and_links_response,
stale_request_head_conflicts_without_invoking,
post_execution_conflict_retains_detached_result,
pending_request_is_recoverable — crash-window A6/A7,
declared_failure_links_sanitized_error,
command_provider_kills_on_execution_timeout,
command_provider_maps_failures_without_hanging,
command_provider_round_trips_with_demo_agent,
sanitizer_replaces_controls_and_bounds_length),
tests/oa05_jsonl.rs (demo_agent_rejects_hostile_lines_without_panic,
demo_agent_echoes_opaque_input_under_demo_namespace,
demo_agent_bounds_oversized_lines_and_resynchronizes), demo stages 8-9
(distinct linked chains, pending=0, detached=0).

### A7 boundaries
Owners: OA-04/OA-05/OA-06.
Proof: tests/oa04_transport.rs (loopback_is_default_and_non_loopback_needs_acknowledgement_and_warning,
sync_server_rejects_unacknowledged_non_loopback_bind,
proxy_environment_is_ignored_by_the_client,
hostile_responses_stay_bounded_and_redirects_are_never_followed,
slow_partial_headers_are_cut_by_the_pre_handler_timer,
slow_request_body_is_cut_by_the_body_read_timeout,
raw_header_flood_is_rejected_before_the_application,
request_target_header_and_body_boundaries_are_exact,
request_body_bound_is_enforced_before_parsing,
concurrency_limit_rejects_rather_than_queueing,
client_response_cap_boundary_is_exact,
no_route_mutates_or_serves_unknown_paths),
tests/oa04_auth.rs (authentication_matrix_returns_one_generic_shape,
token_sources_are_validated_and_never_disclosed,
environment_token_source_is_supported),
tests/oa05_keys.rs (hostile_filesystem_matrix,
persistent_identity_survives_reload,
generated_token_is_canonical_base64url),
tests/oa05_cli.rs (secrets_never_reach_outputs),
tests/oa06_demo.rs (transcripts_logs_and_process_args_have_no_secrets),
strict bundle/projection bounds (03-P03/03-B06 via
iterative_projection_is_unique_and_strictly_bounded and the bundle
adversarial tests), oa-07-claim-audit.md, demo invariants (loopback only,
never promotes remote refs, never executes synchronized requests).

### A8 evidence
Owners: OA-06/OA-07.
Proof: verify-oa00 through verify-oa06 chained gate (baseline 155
checkpoints; count re-asserted by verify-oa07), 86-item workspace test
inventory (85 tests + fixture-print helper), locked build/rustfmt/Clippy
-D warnings, demo transcript exact and stable checksums, fresh-target
offline repetition, this evidence file and the claim audit committed on a
clean tree, and the gate rerun on the final evidence commit.

## Audit layers

| layer | reviewer | verdict |
|---|---|---|
| crypto | independent delegated review 2026-08-17 | release-ready (2 minors: BLAKE3-output == compare acceptable; token string not zeroized post-write, heap residue only) |
| database | independent delegated review 2026-08-17 | release-ready (4 non-blocking findings: non-snapshot listings, invocation TOCTOU bounded and fail-closed, verify_objects presence-only, rollback-error masking both-errors) |
| graph | independent delegated review 2026-08-17 | release-ready (bounded projection stack amplification; bundle input normalization signature-bound; consistent head caps) |
| transport | independent delegated review 2026-08-17 | release-ready (hyper parser front-running noted; idle sockets bounded by the 5 s pre-header timer) |
| provider | independent delegated review 2026-08-17 | release-ready (4 minors, all fail-closed or doc-level; recorded in the claim audit) |
| shell | independent delegated review 2026-08-17 | release-ready (4 minors fixed: gate-wide offline, tmp_chain init, explicit stdout+stderr token scan, demo_err leak and test cleanup ordering) |
| supply-chain | executable checks above + delegated review 2026-08-17 | release-ready after fixing the gate's forbidden-package list (closure vs direct split) and finalizing evidence; lockfile checksums complete, no registry bypass |
| claims | delegated review + oa-07-claim-audit.md | release-ready: zero blockers, every claim demonstrated/limited/absent |

## Always/Never consistency

| Statement (README/plan) | Classification | Proof |
|---|---|---|
| Local refs never move implicitly on pull | Always (enforced) | demo stage 6/10/15; one_way_pull_paginates_converges_and_preserves_local_refs |
| Remote refs never move local refs; peer namespaces only | Always | bundle_round_trip_parent_first_atomic_idempotent_and_remote_only; demo stage 10 |
| Redirects never followed; proxies ignored | Always | hostile_responses_stay_bounded_and_redirects_are_never_followed; proxy_environment_is_ignored_by_the_client |
| Non-loopback plaintext never served without acknowledgement | Always | sync_server_rejects_unacknowledged_non_loopback_bind |
| Corruption never repaired by verify | Always | full_verify_passes_restart_and_reports_corruption_without_repair |
| Provider never invoked before the request commits | Always | request_precedes_call_and_links_response; demo stage 8/9 |
| Shells/tools never executed by CommandProvider or demo_agent | Always | command_provider tests; demo_agent_jsonl matrix |
| Secrets never printed, logged, stored, or synchronized | Always | secrets_never_reach_outputs; transcripts_logs_and_process_args_have_no_secrets; token hash-only server state |
| Authorization is append-only (no revocation in Option A) | Always (limitation stated) | lifecycle test authorize path; README/claims: limited |
| Never claims truth/consensus/confidentiality/availability/exactly-once | Always (documentation) | oa-07-claim-audit.md: all such claims limited or absent |

## Limitations

1. RustSec advisory scan reflects the advisory database at commit
   69f93e1d (2026-08-12); the offline gate verifies the recorded result
   against the unchanged closure, not a live rescan.
2. cfg_block 0.1.1 license taken from repository declaration, not crate
   metadata (accepted finding).
3. One process per database file (frozen Turso engine): a node must
   serialize daemon serving and local CLI access; documented in README.
4. Plain HTTP/1 without TLS by design; cross-machine use requires an
   operator-managed encrypted tunnel; no confidentiality claim.
5. Authorization is append-only; no revocation, no workspace-wide policy.
6. No exactly-once provider delivery; crash windows are explicit and
   queryable only.
7. The fresh-checkout demo test (oa06) is #[ignore]d in normal suites and
   executed explicitly by verify-oa06.sh to bound suite runtime.

## Sequencing note

Gate runs to completion are disclosed with root causes:

1. First chain run failed the oa06 porcelain test because OA-07 files were
   authored while the suite ran (the canary worked as designed).
2. Second run hit the verify-oa06 README-reference grep after the README
   advanced to OA-07 wording; fixed by the owning package.
3. Third run reached the commit-sensitive fresh-checkout test, which by
   design requires a clean tree; stopped.
4. Fourth run failed the fresh-target build with ENOSPC: a single gate
   needs about 11 GiB peak (warm repo target ~8.6 GiB plus the fresh
   target ~2.5 GiB), while other sessions' builds (onchain-final and probe
   directories) left only ~7 GiB. Fixed by reclaiming disposable probe
   directories, keeping the warm repo target, and building the fresh
   target with CARGO_INCREMENTAL=0 to shrink it.
5. Fifth run failed the chain on a race in
   command_provider_maps_failures_without_hanging (see the spec evidence
   finding 6: /bin/echo vs /bin/cat, assertion unchanged).

The authoritative procedure execution is scripts/verify-oa07.sh on the
committed evidence tree — the same tree this file records. The
deterministic ok-checkpoint count (155) is asserted by the gate on every
rerun.

## Verdict

verdict: complete

All commands and verifiers pass on the candidate tree; every A1-A8 row
carries direct executable evidence; the demo proves the required
properties; no finding contradicts a frozen constraint; documentation
claims only demonstrations (see the claim audit); evidence is committed;
the worktree is clean. Option B is unblocked by this verdict per the
frozen gating rule; no Option B work is part of this release.
