---
title: 'OA-07 Release Verification and Option A Gate'
type: 'implementation-spec'
created: '2026-08-17'
status: 'frozen'
approved_plan: '../planning-artifacts/oa-02-oa-07-detailed-execution-plan.md'
decision_record: '../planning-artifacts/oa-02-oa-07-decision-record.md'
baseline_commit: 'c3c9dd4'
review_loop_iteration: 0
option_b_gate: 'blocked-until-OA-07-A1-A8'
---

# OA-07 Release Verification and Option A Gate

## Intent

OA-07 adds no product behavior except release-blocker repairs. It executes
the Option A release procedure (plan section 32), produces the three
evidence artifacts (plan section 33), assigns the A1-A8 verdict (plan
section 34), and flips the Option B gate only on a complete verdict. No
Rust source, dependency, feature, or manifest byte changes.

## Procedure mapping (plan section 32 -> concrete commands)

1. Clean worktree and candidate identity: `git status --porcelain` empty;
   the candidate commit and `git rev-parse HEAD^{tree}` are recorded in the
   evidence. The gate re-asserts cleanliness at every run.
2. Pinned toolchain and native prerequisites: rust-toolchain.toml pins
   1.97.0; `rustc --version` and `cargo --version` must match exactly;
   `rustup show active-toolchain` must show the rust-toolchain.toml
   override; no RUSTC/RUSTFLAGS/CARGO_BUILD_* environment overrides are set
   inside the gate; the native prerequisite is a working C toolchain
   (`cc --version` resolves), which the locked Turso build needs to compile
   its bundled sqlite3.c.
3. Dependency, feature, license, and advisory audit: exact direct
   dependencies and features re-asserted from `cargo metadata --locked`
   (unchanged since OA-05); the locked feature graph must still match
   cargo-tree-oa05-features.txt; the full license inventory over all 319
   locked dependencies is recorded in the evidence with every non-permissive
   or absent license as an explicit accepted finding; `cargo audit`
   (RustSec advisory database) runs once at evidence time and its database
   commit, vulnerability count, and warning count are recorded — the gate
   itself stays deterministic and offline by verifying the recorded
   advisory summary against the current Cargo.lock dependency closure.
4. Verifiers OA-00 through OA-06: `bash scripts/verify-oa06.sh`, which
   chains verify-oa00, verify-oa01, verify-oa02, verify-oa03, verify-oa04,
   verify-oa04-dependencies, verify-oa05, and its own checkpoints. The gate
   requires the chain to exit 0 with at least the baseline count of
   `^ok:`` lines (155 at baseline) and no `FAIL`/`error:` marker in its
   output; the exact count is recorded in the evidence at evidence time.
5. Locked build, rustfmt, strict Clippy, full workspace tests, and the
   seventeen-stage demo: executed on the candidate tree by the verify-oa06
   chain itself; OA-07 does not duplicate the warm runs (avoiding extra
   timing-bound flake surface) and adds the fresh-target repetition of
   step 6 plus its own scan and audit checkpoints.
6. Fresh repetition: a fresh cargo target directory (`--target-dir` under a
   temporary root) runs the locked build, the full workspace test suite,
   Clippy, and the demo with a fresh explicit runtime root, proving no
   cached-artifact dependence. "Deterministic and offline" means: no
   advisory-database or crates.io-index fetches inside the gate; the
   fresh-target build runs with CARGO_NET_OFFLINE=true and uses only the
   local cargo registry cache, so the gate performs no network access at
   all.
7. Secret and runtime-artifact scan: no tracked file contains `token1_`
   outside the approved test vectors under tests/fixtures; no tracked or
   untracked-ignored file matches `*.db`, `*.db-shm`, `*.db-wal`, or daemon
   logs; `git ls-files --others --ignored --exclude-standard` lists nothing
   outside `target/`.
8. Independent audit layers: crypto, database, graph, transport, provider,
   shell, supply-chain, and claims — run as independent delegated
   adversarial reviews with findings recorded and resolved in this spec,
   plus the executable supply-chain checks above.
9. A1-A8 evidence matrix: every row links its owners to exact test names,
   scripts, fixtures, checksums, and demo stages (below).
10. Limitations and Always/Never consistency: recorded in the evidence;
    every README/spec claim is classified demonstrated, limited, or removed
    in the claim audit; any contradiction blocks completion.
11. Verdict: complete only if every command and verifier passes, every row
    has direct evidence, the demo proves all required properties, no issue
    contradicts a frozen constraint, documentation only claims
    demonstrations, evidence is committed, and the worktree is clean.
    Anything missing or ambiguous yields incomplete and Option B stays
    blocked.

## Deterministic gate versus recorded evidence

`scripts/verify-oa07.sh` is deterministic and non-recording: it never writes
evidence, never fetches from the network, and only fails or passes. The
RustSec advisory scan and the license inventory are recorded at evidence
time (they change with external databases); the gate verifies the recorded
advisory summary exists, names the exact Cargo.lock dependency closure
count, and that the current closure still matches that count. The closure count is
   defined exactly as the number of `[[package]]` entries in Cargo.lock
   (320 at baseline: 319 dependencies plus the workspace root), computed
   with `grep -c '^name = ' Cargo.lock` under `--locked` metadata
   agreement.

## A1-A8 evidence matrix

Matrix rows that the approved traceability matrix assigns to multiple gates
are implemented by the concrete tests cited under each row below; every
multi-gate assignment is honored explicitly: 02-P03 (unauthorized author)
is A2 and A7 via policy_parent_and_stale_failures_leave_history_unchanged;
02-A03 (canonical wire retention) is A1 and A4 via
lifecycle_idempotence_namespaces_and_restart and
full_verify_passes_restart_and_reports_corruption_without_repair; 03-P03
(projection bounds) is A7 via iterative_projection_is_unique_and_strictly_bounded;
03-B06 (strict bundle limits) is A2 and A7 via
bundle_parser_rejects_unknown_duplicate_version_order_and_limits and
malformed_inputs_never_panic_or_return_partial_events; 05-P05 (crash-window
recovery) is A6 and A7 via pending_request_is_recoverable.

| Gate | Owners | Final proof (exact artifacts) |
|---|---|---|
| A1 identity | OA-01 | fixtures/oa01-v1-golden.json sha256 799f326d… frozen; tests/oa01_golden.rs (checked_in_fixture_is_deterministically_reproducible, fixed_vector_recomputes_and_verifies_independently, equivalent_json_produces_identical_body_id_and_signature); tests/oa01_adversarial.rs (every_signed_field_mutation_is_rejected, parser_rejects_equivalent_but_noncanonical_and_hostile_input, strict_ed25519_rejects_noncanonical_s_and_small_order_keys, duplicate_keys_are_rejected_at_every_depth, unicode_is_not_normalized_but_escape_aliases_are, typed_text_encodings_are_canonical_and_exact, number_boundaries_negative_zero_and_exponent_aliases_are_enforced, rfc_8785_serialization_example_matches, rfc_8785_utf16_property_order_matches); verify-oa01 checksum + git-diff guards |
| A2 admission | OA-02/03 | oa02: wrong_genesis_and_cross_context_parent_reject, provisioning_mismatch_and_external_collision_are_typed, malformed_author_and_signature_are_rejected, malformed_field_sets_versions_bom_and_trailing_data_are_typed, policy_parent_and_stale_failures_leave_history_unchanged, newer_and_incomplete_schemas_fail_closed; oa03: bundle_parser_rejects_unknown_duplicate_version_order_and_limits, malformed_inputs_never_panic_or_return_partial_events; demo stage 16 (one-byte tamper rejected atomically, exit class 9, no state change) |
| A3 DAG/refs | OA-02/03/05 | create_join_append_fork_merge_project_and_restart, iterative_projection_is_unique_and_strictly_bounded, merge_boundaries_and_invalid_shapes_are_atomic, parent_kind_and_depth_boundaries_are_enforced, names_are_strict_and_bounded, independently_opened_stores_produce_one_cas_winner, payload_body_and_wire_size_boundaries_are_enforced; oa05: request_precedes_call_and_links_response, post_execution_conflict_retains_detached_result; demo stages 7-12 (branch, chains, merge, six-ancestor projection, identical exports) |
| A4 persistence | OA-02/03 | full_verify_passes_restart_and_reports_corruption_without_repair, database_triggers_protect_immutable_rows, lifecycle_idempotence_namespaces_and_restart, canonical_bundle_fixture_is_frozen_and_independently_verified; demo stages 13-14 (stop/restart on same databases, verify both, projections unchanged); fresh-target repetition |
| A5 sync | OA-04/06 | one_way_pull_paginates_converges_and_preserves_local_refs, pagination_plan_is_immutable_while_refs_move, invalid_late_page_leaves_refs_unchanged_with_earlier_orphans, unreachable_peer_times_out_boundedly_then_retry_converges, protocol_fixture_is_frozen_canonical_and_reproducible, protocol_cardinality_boundaries_are_exact; demo stages 6, 10, 12, 15 (pull, bidirectional exchange with ref isolation, convergence, zero-insert idempotent re-pull with pages>=1 and remote_refs_updated=0) |
| A6 provider | OA-05/06 | request_precedes_call_and_links_response, stale_request_head_conflicts_without_invoking, post_execution_conflict_retains_detached_result, pending_request_is_recoverable, declared_failure_links_sanitized_error, command_provider_kills_on_execution_timeout, command_provider_maps_failures_without_hanging, command_provider_round_trips_with_demo_agent, sanitizer_replaces_controls_and_bounds_length, demo_agent_rejects_hostile_lines_without_panic, demo_agent_echoes_opaque_input_under_demo_namespace, demo_agent_bounds_oversized_lines_and_resynchronizes; demo stages 8-9 (distinct linked chains, pending=0, detached=0) |
| A7 boundaries | OA-04/05/06 | loopback_is_default_and_non_loopback_needs_acknowledgement_and_warning, sync_server_rejects_unacknowledged_non_loopback_bind, proxy_environment_is_ignored_by_the_client, hostile_responses_stay_bounded_and_redirects_are_never_followed, slow_partial_headers_are_cut_by_the_pre_handler_timer, slow_request_body_is_cut_by_the_body_read_timeout, raw_header_flood_is_rejected_before_the_application, request_target_header_and_body_boundaries_are_exact, request_body_bound_is_enforced_before_parsing, concurrency_limit_rejects_rather_than_queueing, client_response_cap_boundary_is_exact, no_route_mutates_or_serves_unknown_paths, authentication_matrix_returns_one_generic_shape, token_sources_are_validated_and_never_disclosed, hostile_filesystem_matrix, persistent_identity_survives_reload, generated_token_is_canonical_base64url, secrets_never_reach_outputs, transcripts_logs_and_process_args_have_no_secrets, environment_token_source_is_supported; oa-07-claim-audit.md; demo never promotes remote refs, never executes synchronized requests, binds loopback only |
| A8 evidence | OA-06/07 | verify-oa00..verify-oa06 chain (155 checkpoints); 86-item workspace test inventory; locked build, rustfmt, Clippy -D warnings; demo transcript with exact and id-redacted stable checksums; fresh-target repetition; this evidence file and the claim audit, committed on a clean tree |

## Evidence artifacts and no-secret rules

- `scripts/verify-oa07.sh`: the deterministic gate described above.
- `_bmad-output/verification-artifacts/oa-07-release-evidence.md`: candidate
  commit/tree, tool versions, advisory database commit and result, license
  inventory with accepted findings, command/status table with checkpoint
  counts, fixture and transcript checksums, the A1-A8 matrix, audit layers
  with reviewers, limitations, and a dedicated Always/Never consistency
  table mapping every Always/Never statement in the README and the plan
  (never moves local refs implicitly, never follows redirects, never
  executes shells, never exposes non-loopback without acknowledgement,
  never repairs corruption, never promotes remote refs, secrets never in
  outputs, append-only authorization) to its proving test or demo stage.
- `_bmad-output/verification-artifacts/oa-07-claim-audit.md`: every claim
  classified demonstrated, limited, or removed.
- this spec: status and verdict.

No evidence contains keys, tokens, sensitive paths, or arbitrary payloads:
public IDs, counts, checksums, and version strings only. The gate is rerun
on the final evidence commit; its success is recorded before the spec is
marked done.

## File map

| File | Change |
|---|---|
| `scripts/verify-oa07.sh` | new deterministic non-recording gate |
| `_bmad-output/verification-artifacts/oa-07-release-evidence.md` | new |
| `_bmad-output/verification-artifacts/oa-07-claim-audit.md` | new |
| `_bmad-output/implementation-artifacts/spec-oa-07-release-gate.md` | this spec |
| `README.md` | status header, verification commands, deferred scope |

Predecessor verifiers are not modified: OA-07 owns no product surface.

## Tasks and acceptance

1. Freeze this spec with the freeze review recorded.
2. Add scripts/verify-oa07.sh; verify it fails closed before the evidence
   exists and passes after.
3. Run the full procedure (steps 1-10) on the candidate tree, recording the
   evidence artifacts; resolve every audit-layer finding.
4. Rerun scripts/verify-oa07.sh on the final evidence commit.
5. Record the verdict; only a complete verdict unblocks Option B; commit
   with the exact subject `OA-07: record Option A completion evidence`.

## Change control and boundary

Any product-behavior change discovered by the procedure is either a
documented release blocker (verdict incomplete) or requires explicit
approval; OA-07 itself may only add the gate and evidence documents. The
Option B flip, if earned, is a statement recorded in the evidence, not the
start of Option B work.

## Freeze review evidence

An independent delegated adversarial review ran against the plan, the
traceability matrix, the verify-oa06 chain, the README, and the live test
files; it completed with findings. Direct verification ran alongside it:
all 71 test names cited in the A1-A8 matrix were machine-checked against
the compiled 86-item workspace test inventory — every citation exists.

Review verdict: approved after fixes; three majors and five minors, all
resolved:

1. Major: the fresh-target build appeared to contradict the no-network
   rule. Fixed: "offline" now means no advisory-database or index fetches,
   and the fresh-target run executes with CARGO_NET_OFFLINE=true against
   the local registry cache only.
2. Major: five multi-gate matrix rows (02-P03, 02-A03, 03-P03, 03-B06,
   05-P05) were cited under only one of their assigned gates. Fixed: the
   matrix section now links every multi-gate assignment to its concrete
   tests explicitly.
3. Major: "flips the Option B gate only if and as far as the evidence
   justifies" implied a partial flip. Fixed: the flip is binary and only on
   a complete verdict, exactly as plan section 34 defines.
4. Minor: Always/Never consistency now requires a dedicated table in the
   evidence artifact, not just a mention.
5. Minor: the native prerequisite (working `cc` for Turso's bundled
   sqlite3.c) is now probed in step 2.
6. Minor: step 5 no longer duplicates the warm build/test/demo runs the
   verify-oa06 chain already performs; OA-07 adds the fresh-target
   repetition instead, reducing timing-bound flake surface.
7. Minor: determinism definitions pinned — checkpoint success is "exit 0,
   at least the baseline ok-line count, no failure markers", and the
   dependency closure count is defined exactly (320 [[package]] entries:
   319 dependencies plus the workspace root).
8. Minor: the ignored-file allowlist is restricted to `target/` only.

Freeze verdict: ready for implementation from baseline c3c9dd4. Review loop
iteration remains zero because no implementation exists yet. Option B stays
blocked until this spec records a complete A1-A8 verdict.

