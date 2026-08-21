---
title: 'OC-01 OutcomeLedgerV1 Test Traceability Matrix'
type: 'test-traceability-matrix'
created: '2026-08-21'
status: 'approved-for-implementation'
approved: '2026-08-21'
approved_by: 'Lunarpulse'
approval_source: 'Discord message 1540352346364842105'
baseline_commit: 'a2135f73b957b2d9c34b6655b5c1f1cab2851de4'
source_spec: '../implementation-artifacts/spec-oc-01-outcome-ledger.md'
decision_record: './oc-00-5-founder-decision-record.md'
---

# OC-01 OutcomeLedgerV1 Test Traceability Matrix

> **Approved for implementation** by Lunarpulse on 2026-08-21. Test names and
> files below are required targets, not claims that tests or feature already
> exist. Passing this matrix establishes only bounded artifact integrity and
> provenance recording; it does not establish C2, C3, C4, C5, or Option C.

## Conventions

- Gate IDs are stable review identifiers: `OC01-W*` workspace/dependency,
  `OC01-I*` schema/integrity, `OC01-P*` protocol/issue/crypto, `OC01-D*`
  DAG/current-input, `OC01-X*` hostile I/O/privacy, and `OC01-R*`
  regression/evidence.
- Rust entries name the exact integration-test function. Script entries use
  `path::check_name` for an exact required check in that script.
- A parameterized test must report each named case independently; one failed case
  fails its gate. Exact-max tests include `0`, maximum, and maximum-plus-one where
  the type permits zero.
- All rows are blocking. No document-only assertion substitutes for executable
  evidence, except the evidence-document structure/claim audit rows, which are
  themselves checked by the non-recording gate.

## Workspace, dependencies, and delivery order

| Gate | Requirement traced | Exact test/check | File | Pass evidence |
|---|---|---|---|---|
| OC01-W01 | Root is package-plus-workspace with members `.` and `contextmesh-salience`, resolver 3; core package is not moved. | `workspace_shape_is_exact` | `contextmesh-salience/tests/oc01_workspace.rs` | Metadata has exactly two workspace members and the root package path is unchanged. |
| OC01-W02 | Salience manifest has exact name/version/edition/MSRV/publish setting and exact direct pins/features; `tokio` is test-only. | `salience_manifest_and_direct_pins_are_exact` | `contextmesh-salience/tests/oc01_workspace.rs` | Manifest fields and dependency feature sets equal the specification. |
| OC01-W03 | Dependency is one-way: salience directly depends on core and core has no path to salience. | `dependency_direction_is_strictly_one_way` | `contextmesh-salience/tests/oc01_workspace.rs` | Metadata reachability proves `contextmesh-salience -> contextmesh` and no reverse path. |
| OC01-W04 | No new registry/git identity; no model, embedding, judge, network, native runtime, alternate DB, or heavy optional adapter enters OC-01. | `salience_adds_no_registry_or_forbidden_capability` | `contextmesh-salience/tests/oc01_workspace.rs` | Salience registry/git IDs are a subset of baseline and forbidden capability sets are empty. |
| OC01-W05 | Workspace/lock counts are exactly 2 local, core-reachable 320 total/319 external, and 321 lock entries. | `workspace_and_lock_counts_match_migration_contract` | `contextmesh-salience/tests/oc01_workspace.rs` | Stable helper JSON reports all four exact counts. |
| OC01-W06 | Core reachable `(name, version, source)` set and exact direct pins/features remain baseline-identical. | `core_registry_closure_is_byte_for_byte_unchanged` | `contextmesh-salience/tests/oc01_workspace.rs` | Shared-helper baseline comparison has zero additions/removals/changes. |
| OC01-W07 | Core package lookup is by exact package name, traversal starts at its package ID, and never depends on nullable `resolve.root`. | `dependency_helper_uses_named_package_reachability` | `contextmesh-salience/tests/oc01_workspace.rs` | Synthetic workspace metadata with `resolve.root = null` passes; missing/duplicate named core fails closed. |
| OC01-W08 | Helper output is deterministic/non-secret and package-scoped feature tree remains byte-identical. | `dependency_helper_output_and_feature_tree_are_stable` | `contextmesh-salience/tests/oc01_workspace.rs` | Two runs match; `cargo tree -p contextmesh --locked -e features` matches `cargo-tree-oa05-features.txt`. |
| OC01-W09 | Frozen OA verifiers remain byte-unchanged and the OA chain runs in a detached clean `9c275f0` worktree. | `historical_oa07_chain_runs_unchanged_at_completion_commit` | `contextmesh-salience/tests/oc01_workspace.rs` | `verify-oa07.sh` passes offline and includes unchanged OA-06 manifest/lock semantics; temporary worktree is removed. |
| OC01-W10 | Frozen Option B verifiers remain byte-unchanged and the completion chain runs in a detached clean `1df5334` worktree. | `historical_ob13_chain_runs_unchanged_at_completion_commit` | `contextmesh-salience/tests/oc01_workspace.rs` | `verify-ob13.sh` passes offline without script migration; temporary worktree is removed. |
| OC01-W11 | Current workspace regression is package-scoped and does not invoke obsolete historical HEAD assertions. | `current_workspace_checks_are_package_scoped_and_legacy_scripts_immutable` | `contextmesh-salience/tests/oc01_workspace.rs` | Core/salience/workspace build/tests/tree, demos, frozen hashes and security checks pass; every existing `verify-oa*.sh`/`verify-ob*.sh` hash equals baseline. |
| OC01-W12 | Implementation/review stages execute in the specified eight-stage dependency order and stop on the first failed stage. | `scripts/verify-oc01.sh::stages_execute_in_dependency_order` | `scripts/verify-oc01.sh` | Logged stages are workspace, primitives, schema, protocol, DAG, I/O, vectors, evidence; injected failure prevents later stages. |
| OC01-W13 | Planned file boundary is enforced: no production core `src/`, existing OA/OB verifier/test/evidence/fixture, Option A/B wire/schema, or store-schema target is changed. | `scripts/verify-oc01.sh::planned_surface_only` | `scripts/verify-oc01.sh` | Changed-path allowlist passes and forbidden production/historical surfaces have baseline hashes. |

## Schema, values, ordering, and bounds

| Gate | Requirement traced | Exact test | File | Pass evidence |
|---|---|---|---|---|
| OC01-I01 | Version, exact required body/envelope fields, tagged-variant field sets, and unknown/missing/null rules are frozen. | `exact_v1_shapes_tags_requiredness_and_version` | `contextmesh-salience/tests/oc01_schema.rs` | Every schema/variant positive case passes; unknown, missing, illegal `null`, wrong tag set, and version cases reject. |
| OC01-I02 | Typed ID/signature/ref-fingerprint/hash/timestamp prefixes, byte lengths, alphabets, no-padding, lowercase hex, and UTC Gregorian grammar are exact. | `typed_text_encodings_and_timestamp_are_exact` | `contextmesh-salience/tests/oc01_schema.rs` | Valid boundaries pass; cross-type prefix, padding, length, alphabet, case, pre-1970, and invalid-date cases reject. |
| OC01-I03 | Mechanism identity/version/config are nonempty and bounded at 128/64 bytes with exact hash grammar; text rejects C0/C1 controls. | `mechanism_record_boundaries_and_provenance` | `contextmesh-salience/tests/oc01_schema.rs` | Empty/control and +1 cases reject; exact maxima pass. |
| OC01-I04 | Task binding requires content hash; nullable structured hash/external ID obey exact limits and constructors accept hashes, not task bytes or notes. | `task_binding_is_hash_only_and_bounded` | `contextmesh-salience/tests/oc01_schema.rs` | Hash-only API and schema compile; external ID 128 passes and +1 rejects; raw task/note fields reject. |
| OC01-I05 | Snapshot local refs are ascending unique by name; remote refs by `(peer,name)`; empty arrays are valid; fingerprint binds context and exact arrays. | `input_ref_snapshot_order_uniqueness_and_fingerprint` | `contextmesh-salience/tests/oc01_schema.rs` | Ordered/empty vectors pass; duplicates, disorder, context/head/name/peer tamper reject. |
| OC01-I06 | Every EventId list is strictly ascending unique; constructors reject rather than sort. | `event_id_lists_require_caller_canonical_order` | `contextmesh-salience/tests/oc01_schema.rs` | All event-list fields reject duplicate/disordered inputs and preserve accepted input. |
| OC01-I07 | Terminal is exactly caller-supplied event or explicit `unterminated` with one of four reasons; no null/discovery/fallback. | `terminal_event_and_unterminated_variants_are_exhaustive` | `contextmesh-salience/tests/oc01_schema.rs` | Event and four reasons pass; null/free-text/mixed fields reject. |
| OC01-I08 | Outcome values are exact; evidence may be empty; terminal status does not infer outcome. | `outcome_values_are_caller_declared_not_inferred` | `contextmesh-salience/tests/oc01_schema.rs` | Five values round-trip independently with terminal and unterminated variants. |
| OC01-I09 | Quality is tagged available/unavailable with provenance; available ppm is `0..=1,000,000`; unavailable reason is bounded. | `quality_availability_values_and_provenance_are_exact` | `contextmesh-salience/tests/oc01_schema.rs` | 0 and 1,000,000 pass; +1, missing provenance, mixed variants, and overlong reason reject. |
| OC01-I10 | Each of five cost fields is independently available/unavailable and provenanced; safe integer 0 is available, never inferred. | `cost_availability_zero_and_unavailable_are_preserved` | `contextmesh-salience/tests/oc01_schema.rs` | Mixed availability round-trips; zero stays available; missing/mixed/inferred values reject. |
| OC01-I11 | All integer values are within `0..=2^53-1`; checked arithmetic is used for aggregate counts/bytes. | `safe_integer_and_checked_aggregate_boundaries` | `contextmesh-salience/tests/oc01_schema.rs` | Exact safe maximum passes; +1/negative/overflow paths return `limit-exceeded`. |
| OC01-I12 | Attempts are empty or contiguous IDs, exactly one root, parent-before-child, connected and acyclic; category is nonempty ASCII <=64. | `attempt_tree_ordinals_parent_order_connectivity_and_categories` | `contextmesh-salience/tests/oc01_schema.rs` | Multi-level tree passes; gap, second root, forward/missing parent, disconnected/cycle, non-ASCII and 65-byte category reject. |
| OC01-I13 | Attempt status/error/operation fingerprint/per-attempt costs/provenance use exact variants without reinterpretation. | `attempt_values_errors_costs_and_provenance_round_trip_exactly` | `contextmesh-salience/tests/oc01_schema.rs` | All statuses and available/unavailable errors round-trip, including succeeded-with-diagnostic. |
| OC01-I14 | Dead-end IDs are contiguous, target attempts exist, category is bounded, and disposition is one of four exact values. | `dead_end_ordinals_targets_categories_and_dispositions` | `contextmesh-salience/tests/oc01_schema.rs` | Four dispositions pass; gap, absent target, bad category and unknown disposition reject. |
| OC01-I15 | Attribution marks use exact candidate labels, mechanism provenance, and ascending-unique composite order; they are not scores/causal results. | `attribution_marks_are_ordered_provenanced_candidates_only` | `contextmesh-salience/tests/oc01_schema.rs` | Four labels pass; duplicate/disorder/unknown label or score-like fields reject. |
| OC01-I16 | Warnings preserve caller order, reject duplicates, are nonempty/control-free, and each warning/reason obeys 1,024 UTF-8 bytes; categories follow frozen lowercase ASCII grammar. | `warnings_reasons_and_categories_obey_text_grammar_and_bounds` | `contextmesh-salience/tests/oc01_schema.rs` | Ordered distinct text passes; empty/control/duplicate/bad-category and +1 cases reject without sorting/truncation. |
| OC01-I17 | Caller limits are nonzero, downward-only, and never exceed hard maxima; no truncation/chunking. | `outcome_limits_are_nonzero_downward_only_and_never_truncate` | `contextmesh-salience/tests/oc01_schema.rs` | Equal/lower limits enforce exactly; zero and above-hard-max constructors reject. |
| OC01-I18 | Canonical artifact raw input/output bound is 2,097,152 bytes. | `wire_bytes_zero_maximum_and_maximum_plus_one` | `contextmesh-salience/tests/oc01_adversarial.rs` | Exact max reaches schema validation; max+1 returns `limit-exceeded` before parse/write and returns no artifact. |
| OC01-I19 | EventId-valued body occurrences are capped at 4,096 before store access; duplicate store-read optimization does not lower occurrence count. | `event_reference_occurrences_zero_4096_and_4097` | `contextmesh-salience/tests/oc01_adversarial.rs` | 4,096 passes; 4,097 fails before store call; repeated occurrences count independently. |
| OC01-I20 | Attempts are capped at 1,024. | `attempt_count_zero_1024_and_1025` | `contextmesh-salience/tests/oc01_adversarial.rs` | 0/1,024 pass and 1,025 returns `limit-exceeded`. |
| OC01-I21 | Dead ends are capped at 1,024. | `dead_end_count_zero_1024_and_1025` | `contextmesh-salience/tests/oc01_adversarial.rs` | 0/1,024 pass and 1,025 returns `limit-exceeded`. |
| OC01-I22 | Attribution marks are capped at 4,096. | `attribution_mark_count_zero_4096_and_4097` | `contextmesh-salience/tests/oc01_adversarial.rs` | 0/4,096 pass and 4,097 returns `limit-exceeded`. |
| OC01-I23 | Warnings are capped at 64. | `warning_count_zero_64_and_65` | `contextmesh-salience/tests/oc01_adversarial.rs` | 0/64 pass and 65 returns `limit-exceeded`. |
| OC01-I24 | Every permitted warning/unavailable reason is capped at 1,024 UTF-8 bytes. | `all_note_locations_enforce_zero_1024_and_1025_bytes` | `contextmesh-salience/tests/oc01_adversarial.rs` | Parameterized cases cover warning and quality/cost/error unavailable reasons; +1 fails; TaskBinding exposes no note. |
| OC01-I25 | Strict parser rejects BOM, trailing data, duplicates at every depth, unsafe/non-finite numbers, and depth >64. | `strict_json_hostile_syntax_matrix` | `contextmesh-salience/tests/oc01_adversarial.rs` | Every named hostile vector rejects with no panic or partial ledger. |
| OC01-I26 | JCS is exact; semantic equivalents with whitespace/member order/normalized escapes are `noncanonical`; `to_wire` revalidates. | `canonical_wire_is_exact_and_render_revalidates` | `contextmesh-salience/tests/oc01_adversarial.rs` | Exact JCS round-trips byte-for-byte; non-JCS equivalents reject; invalid in-memory state cannot render. |

## Issuance, structural verification, crypto, and vectors

| Gate | Requirement traced | Exact test | File | Pass evidence |
|---|---|---|---|---|
| OC01-P01 | ID is ordinary BLAKE3 of literal NUL-terminated ID domain plus exact JCS body, not derive-key mode. | `outcome_id_uses_literal_domain_prefix_hashing` | `contextmesh-salience/tests/oc01_crypto.rs` | Published domain vector matches exact typed ID and differs from derive-key/text alternatives. |
| OC01-P02 | Signature is Ed25519 over literal signature domain plus raw 32-byte ID, reusing core signing/strict verify; not ID text/body. | `signature_covers_domain_and_raw_id_bytes` | `contextmesh-salience/tests/oc01_crypto.rs` | Exact signature vector matches; ID-text/body signing alternatives fail. |
| OC01-P03 | Cross-type IDs, signatures, prefixes, lengths, alphabets, padding, domains, and authors reject. | `cross_domain_typed_encoding_and_author_mismatch_matrix` | `contextmesh-salience/tests/oc01_crypto.rs` | Every cross-domain/type/author case rejects with its stable category. |
| OC01-P04 | Body, task, snapshot, ID, signature, outcome, quality, cost, attempt, dead-end, mark, author, and timestamp tampering reject. | `tamper_matrix_rejects_every_signed_or_derived_component` | `contextmesh-salience/tests/oc01_crypto.rs` | Parameterized field-level tamper cases all fail; no repaired/sorted artifact is returned. |
| OC01-P05 | `from_wire` is sole untrusted constructor; checked nested constructors/read-only accessors prevent deserialize/unchecked bypass. | `public_api_has_no_unchecked_or_deserialize_bypass` | `contextmesh-salience/tests/oc01_schema.rs` | Compile/API audit cannot construct invalid public state; valid accessors expose no mutation. |
| OC01-P06 | Structural parse/verify is store-free and freezes precedence wire bound -> parse -> schema -> canonicality -> ID -> signature, but makes no DAG/freshness claim. | `structural_verify_is_store_free_claim_bounded_and_precedence_exact` | `contextmesh-salience/tests/oc01_crypto.rs` | Compound-invalid vectors return the earliest frozen category; valid immutable artifact verifies without Store access. |
| OC01-P07 | `issue` order is validate -> load/verify every referenced event/context/current refs -> derive ID -> sign -> independent self/store verify. | `issue_executes_fail_closed_steps_in_exact_order` | `contextmesh-salience/tests/oc01_dag.rs` | Instrumented fixture observes exact phases; injected failure stops all later phases. |
| OC01-P08 | Any validation/DAG/context/stale/sign/self-verify failure returns neither artifact nor partial report and does not mutate store. | `issue_returns_no_artifact_and_never_mutates_store_on_any_failure` | `contextmesh-salience/tests/oc01_dag.rs` | Failure matrix returns only error; store/ref/event hashes and counts remain unchanged. |
| OC01-P09 | Terminal-event golden includes mixed cost availability, multi-level attempts, recovered/unresolved dead ends, and multiple attribution mechanisms. | `terminal_golden_fixture_matches_bytes_id_and_signature` | `contextmesh-salience/tests/oc01_crypto.rs` | Reconstruction exactly equals `tests/fixtures/oc01-outcome-ledger-v1-golden.json`, typed ID, and signature. |
| OC01-P10 | Unterminated golden includes unavailable clock/calls/retries/tokens and exact terminal reason. | `unterminated_golden_fixture_matches_bytes_id_and_signature` | `contextmesh-salience/tests/oc01_crypto.rs` | Reconstruction exactly equals `tests/fixtures/oc01-outcome-ledger-v1-unterminated.json`, typed ID, and signature. |
| OC01-P11 | Golden updates are never automatic; generator is ignored and committed vectors are normal-test inputs. | `golden_generator_is_ignored_and_fixtures_are_immutable_inputs` | `contextmesh-salience/tests/oc01_crypto.rs` | Normal suite compares committed bytes; generator has `#[ignore]` and gate detects fixture drift. |

## DAG, admission evidence, and current inputs

| Gate | Requirement traced | Exact test | File | Pass evidence |
|---|---|---|---|---|
| OC01-D01 | Snapshot capture reads local+all-peer remote refs, canonicalizes order, supports empty refs, and computes exact context-bound fingerprint. | `capture_snapshot_canonicalizes_complete_local_and_remote_refs` | `contextmesh-salience/tests/oc01_dag.rs` | Permuted store insertion yields one exact snapshot/fingerprint; empty case passes. |
| OC01-D02 | Every unique referenced/input/terminal event is loaded with strict stored-wire verification and exact body context. | `dag_verification_covers_every_event_role_and_deduplicates_only_reads` | `contextmesh-salience/tests/oc01_dag.rs` | Coverage fixture touches every role; read dedup does not alter occurrence-bound result. |
| OC01-D03 | Missing event fails closed. | `missing_event_returns_missing_event_without_partial_report` | `contextmesh-salience/tests/oc01_dag.rs` | Each event role missing returns `missing-event`, no verification object/artifact. |
| OC01-D04 | Cross-context event fails closed. | `cross_context_event_returns_context_mismatch` | `contextmesh-salience/tests/oc01_dag.rs` | Each event role with another ContextId returns `context-mismatch`. |
| OC01-D05 | Any Store operational failure remains an `OutcomeOperationError::Store` cause and is never mislabeled as an artifact category. | `store_error_mapping_is_total_generic_and_nonsecret` | `contextmesh-salience/tests/oc01_dag.rs` | Every public `StoreError` variant maps through the Store wrapper; `CorruptStorage` is not `malformed`; no partial report. |
| OC01-D06 | Admission is the authorization evidence for every referenced event; the OC signer is authenticated only by its distinct domain signature. | `admitted_references_and_independent_artifact_signer_are_not_conflated` | `contextmesh-salience/tests/oc01_dag.rs` | Admitted same-context references pass even when the valid OC signer authored no Option A event; no allowlist query or witness field exists. |
| OC01-D07 | No artifact-signer allowlist, revocation, or historical authorization semantics are inferred from current public APIs. | `authorization_verification_does_not_invent_signer_policy` | `contextmesh-salience/tests/oc01_dag.rs` | Signature author match is enforced; signer admission/revocation claims and hidden core bridges are absent. |
| OC01-D08 | Immutable DAG re-verification remains valid after refs move, while freshness verification fails. | `dag_verify_survives_ref_move_but_current_inputs_returns_stale_input` | `contextmesh-salience/tests/oc01_dag.rs` | `verify_against_dag` passes and `verify_current_inputs` returns `stale-input`. |
| OC01-D09 | Any local/remote ref add, remove, name/head move, or fingerprint mismatch is stale input. | `current_input_snapshot_change_matrix_returns_stale_input` | `contextmesh-salience/tests/oc01_dag.rs` | Parameterized local/remote add/remove/move/fingerprint cases all return `stale-input`. |
| OC01-D10 | Successful verification reports only checked counts and snapshot fingerprint; failed methods return no report. | `verification_reports_are_bounded_nonredundant_and_all_failures_atomic` | `contextmesh-salience/tests/oc01_dag.rs` | Success report has no `valid` boolean/findings/text; every failure is `Err` only. |

## Import/export, adversarial failures, and privacy

| Gate | Requirement traced | Exact test | File | Pass evidence |
|---|---|---|---|---|
| OC01-X01 | Export re-verifies, emits exact JCS to a newly created regular file, refuses existing destination, syncs, and never writes Option A DB. | `export_is_create_new_canonical_synced_and_store_independent` | `contextmesh-salience/tests/oc01_io.rs` | Bytes equal `to_wire`; existing path rejects; Option A DB hash/count unchanged. |
| OC01-X02 | Export write/sync failure removes only its partial new file and returns no success artifact. | `export_failure_removes_partial_new_file` | `contextmesh-salience/tests/oc01_io.rs` | Injected short-write/sync failures leave no destination and preserve unrelated files. |
| OC01-X03 | Import accepts only regular non-symlink files, reads at most max+1, and never repairs/sorts/rewrites input. | `import_is_bounded_regular_file_only_and_never_repairs` | `contextmesh-salience/tests/oc01_io.rs` | Valid file passes; symlink/directory/device/excess/noncanonical cases reject unchanged. |
| OC01-X04 | Verified import additionally requires full DAG and current snapshot before return. | `verified_import_requires_dag_and_current_inputs` | `contextmesh-salience/tests/oc01_io.rs` | Valid case returns ledger; missing/context/stale cases return no ledger. |
| OC01-X05 | Parse/issue/verify/import/export never panic or return partial ledgers/reports/files. | `all_public_failure_paths_are_panic_free_and_partial_free` | `contextmesh-salience/tests/oc01_adversarial.rs` | Hostile and injected-failure matrix produces stable errors and no partial outputs. |
| OC01-X06 | `malformed` category and collapse rules are exact and non-secret. | `outcome_error_category_malformed_is_exact_and_secret_free` | `contextmesh-salience/tests/oc01_adversarial.rs` | Syntax/type/duplicate/unknown/missing/typed-encoding vectors display exactly `malformed` without input text. |
| OC01-X07 | `noncanonical` category is exact and non-secret. | `outcome_error_category_noncanonical_is_exact_and_secret_free` | `contextmesh-salience/tests/oc01_adversarial.rs` | Semantic non-JCS vectors display only stable category text. |
| OC01-X08 | `unsupported-version` category is exact and non-secret. | `outcome_error_category_unsupported_version_is_exact_and_secret_free` | `contextmesh-salience/tests/oc01_adversarial.rs` | Wrong-version vector maps exactly. |
| OC01-X09 | `limit-exceeded` category is exact and non-secret. | `outcome_error_category_limit_exceeded_is_exact_and_secret_free` | `contextmesh-salience/tests/oc01_adversarial.rs` | Every +1/downward-limit vector maps exactly. |
| OC01-X10 | `id-mismatch` category is exact and non-secret. | `outcome_error_category_id_mismatch_is_exact_and_secret_free` | `contextmesh-salience/tests/oc01_adversarial.rs` | Recomputed-ID mismatch maps exactly. |
| OC01-X11 | `signature-invalid` category is exact and non-secret. | `outcome_error_category_signature_invalid_is_exact_and_secret_free` | `contextmesh-salience/tests/oc01_adversarial.rs` | Signature/domain/author cryptographic failures map exactly where structurally valid. |
| OC01-X12 | `missing-event` category is exact and non-secret. | `outcome_error_category_missing_event_is_exact_and_secret_free` | `contextmesh-salience/tests/oc01_adversarial.rs` | Missing DAG reference maps exactly without EventId leakage. |
| OC01-X13 | Reserved `unauthorized-event` category is exact and non-secret and is not fabricated from unavailable policy APIs. | `outcome_error_category_unauthorized_event_is_exact_reserved_and_secret_free` | `contextmesh-salience/tests/oc01_adversarial.rs` | Enum/display vector is exact without key leakage; current issuance paths do not emit it from guessed signer policy. |
| OC01-X14 | `context-mismatch` category is exact and non-secret. | `outcome_error_category_context_mismatch_is_exact_and_secret_free` | `contextmesh-salience/tests/oc01_adversarial.rs` | Cross-context vector maps exactly without IDs. |
| OC01-X15 | `stale-input` category is exact and non-secret. | `outcome_error_category_stale_input_is_exact_and_secret_free` | `contextmesh-salience/tests/oc01_adversarial.rs` | Ref drift/fingerprint mismatch maps exactly without ref names/heads. |
| OC01-X16 | Reserved `mechanism-unavailable` category is exact/non-secret and current unavailable values do not emit it. | `outcome_error_category_mechanism_unavailable_is_exact_reserved_and_secret_free` | `contextmesh-salience/tests/oc01_adversarial.rs` | Enum/display vector is exact; quality/cost unavailable round-trips as data rather than error. |
| OC01-X17 | Reserved `incomplete-input` category is exact/non-secret and missing required wire fields remain `malformed`. | `outcome_error_category_incomplete_input_is_exact_reserved_and_secret_free` | `contextmesh-salience/tests/oc01_adversarial.rs` | Enum/display vector is exact; parser missing-field vector returns `malformed`, not fabricated incomplete-input. |
| OC01-X18 | Error displays/reports exclude paths, task/note/mechanism text, payloads, keys, signatures, provider responses, and arbitrary errors. | `all_error_and_report_surfaces_are_secret_free` | `contextmesh-salience/tests/oc01_adversarial.rs` | Canary secrets injected into every source are absent from Display/Debug/report/gate output. |
| OC01-X19 | Portable schema/API has no dedicated raw task/transcript/structured/path/URL/error/prompt/CoT fields; arbitrary caller text is not falsely certified non-secret. | `portable_schema_excludes_raw_content_fields_and_scopes_privacy_claim` | `contextmesh-salience/tests/oc01_adversarial.rs` | Forbidden fields/API inputs are absent and errors do not echo canaries; docs state warnings/reasons/mechanism text remain caller responsibility. |
| OC01-X20 | Artifact, Store, and file failures use `OutcomeOperationError::{Artifact,Store,Io}` while wire categories remain exactly twelve. | `operation_error_wrapper_preserves_artifact_store_and_io_causes` | `contextmesh-salience/tests/oc01_io.rs` | Mapping/source chain is total; wrapper Display/custom Debug/report/gate output is generic and non-secret, while arbitrary traversed I/O source text is not logged/exported or certified non-secret. |

## Legacy regression, gate, and evidence

| Gate | Requirement traced | Exact test/check | File | Pass evidence |
|---|---|---|---|---|
| OC01-R01 | Core builds/tests unchanged under package and workspace invocations. | `scripts/verify-oc01.sh::workspace_build_lint_and_test_matrix` | `scripts/verify-oc01.sh` | Pinned locked build, fmt, clippy, salience/core/workspace tests all pass. |
| OC01-R02 | Unchanged OA-07 and OB-13 release chains pass in their recorded historical clean worktrees; no obsolete assertion is bypassed. | `historical_release_verifier_chains_pass_unchanged` | `contextmesh-salience/tests/oc01_workspace.rs` | OA `9c275f0` and OB `1df5334` chains pass offline and worktrees are removed. |
| OC01-R03 | Current workspace independently preserves Option A/B wire bytes, fixtures, store schema, tests, forbidden surfaces, security/license/secret checks, and demos. | `scripts/verify-oc01.sh::current_workspace_full_regression` | `scripts/verify-oc01.sh` | Package-scoped current-tree tests/demos and baseline hashes pass with no waived or inconclusive result. |
| OC01-R04 | Exact registry/core closure and feature tree remain unchanged after all OC-01 work. | `scripts/verify-oc01.sh::final_dependency_closure_recheck` | `scripts/verify-oc01.sh` | Shared helper passes at both preflight and final gate; zero external drift. |
| OC01-R05 | Gate is offline, non-recording, fail-closed on partial/inconclusive results, and leaves worktree unchanged. | `scripts/verify-oc01.sh::gate_is_offline_nonrecording_and_clean` | `scripts/verify-oc01.sh` | Before/after status and hashes match; network-denied run passes; injected partial result fails. |
| OC01-R06 | Evidence records exact commit, approved sources/authority, source/test paths, toolchain, and commands. | `scripts/verify-oc01.sh::evidence_sources_layer_is_complete` | `_bmad-output/verification-artifacts/oc-01-evidence.md` | Machine-audited Sources layer has every required reference and command. |
| OC01-R07 | Evidence separates reasoning, alternatives/assumptions, requirement-observation-conclusion derivation, and invalidators. | `scripts/verify-oc01.sh::evidence_four_layers_and_gate_ids_are_complete` | `_bmad-output/verification-artifacts/oc-01-evidence.md` | All four layers exist; every matrix gate ID maps to an observation and bounded conclusion. |
| OC01-R08 | Evidence and docs distinguish caller declarations from verified facts and contain no secrets/private transcripts/paths/URLs/arbitrary payloads. | `scripts/verify-oc01.sh::evidence_privacy_and_claim_language` | `scripts/verify-oc01.sh` | Canary/phrase scan and human-review checklist pass. |
| OC01-R09 | Claim audit limits verdict to C1 artifact integrity/provenance; rejects terminal/success/quality/cost/causal attribution/prior/selection utility inference. | `scripts/verify-oc01.sh::claim_audit_is_limited_to_oc01` | `scripts/verify-oc01.sh` | Prohibited C2/C3/C4/C5/completion claims absent; attribution consistently says caller-supplied candidate. |
| OC01-R10 | OC-02 remains blocked until OC-01 and separate P1 preregistration-hash gates both pass. | `scripts/verify-oc01.sh::downstream_gate_requires_oc01_and_p1_preregistration` | `scripts/verify-oc01.sh` | Truth-table check permits downstream authorization only when both independent gate records pass. |

## Gate roll-up

| Acceptance gate | Required matrix rows |
|---|---|
| `OC01-SETUP` | OC01-W01..W13 |
| `OC01-SCHEMA` | OC01-I01..I24, OC01-P09..P11 |
| `OC01-CRYPTO` | OC01-I25..I26, OC01-P01..P06, OC01-P09..P10 |
| `OC01-DAG` | OC01-P07..P08, OC01-D01..D10 |
| `OC01-IO` | OC01-X01..X05, OC01-X20 |
| `OC01-ADVERSARIAL` | OC01-I18..I26, OC01-P03..P04, OC01-X05..X20 |
| `OC01-REGRESSION` | OC01-W04..W11, OC01-W13, OC01-R01..R05 |
| `OC01-EVIDENCE` | OC01-R06..R10 |

No roll-up passes unless every referenced row passes. Fixture drift, dependency
identity/feature drift, any legacy regression, stale input, partial output,
privacy leak, unsupported claim, or inconclusive evidence is a failing result.

## Approval record

- Approved in full by Lunarpulse on 2026-08-21 after independent compliance GO.
- Approval source: Discord message `1540352346364842105`.
- All 90 rows are blocking; removal or weakening requires founder change control.
