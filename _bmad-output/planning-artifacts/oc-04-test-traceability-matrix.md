# OC-04 Test Traceability Matrix — FROZEN v12

57 rows: S12 + U8 + R9 + E14 + X14 (post-freeze v12 additions per Codex contract
review: S03b prereg-path split, E09/E10/E11 failure-surface rows, X11/X11b +
X12/X12b atomicity splits; count verified programmatically). Every test
name verbatim; evidence cells describe ONE executable assertion each.

| Row | Requirement | Test name | File | Evidence |
|---|---|---|---|---|
| OC04-S01 | InfluenceV1 body renders 6 JCS members lexicographic | `influence_jcs_render` | tests/oc04_schema.rs | Exact byte compare vs hand-rendered canonical |
| OC04-S02 | ExecutionV1 body renders 19 JCS members lexicographic | `execution_jcs_render` | tests/oc04_schema.rs | Exact byte compare |
| OC04-S03 | Config consumes P1 prereg selection_pipeline verbatim | `config_prereg_verbatim` | tests/oc04_schema.rs | Each constant equals the JSON value loaded at test time |
| OC04-S03b | Config consumes P1 prereg evaluation.score_normalization verbatim | `config_score_normalization_verbatim` | tests/oc04_schema.rs | Normalization constants equal the JSON values under evaluation.score_normalization |
| OC04-S04 | Non-default config mutation → Err | `config_validate_rejects_mutation` | tests/oc04_schema.rs | Loop over every config member mutation → all Err |
| OC04-S05 | ID derivation placeholder discipline | `id_placeholder_derivation` | tests/oc04_schema.rs | BLAKE3 over placeholder bytes; `oc04inf1_`/`oc04exec1_` prefixes |
| OC04-S06 | Forged ID prefix rejected | `id_prefix_rejected` | tests/oc04_schema.rs | Wrong prefix → Err |
| OC04-S07 | Parser lenient: extra member still parses | `parser_lenient` | tests/oc04_schema.rs | Extra member parses Ok (no rejection claim) |
| OC04-S07b | Extra member rejected by canonical gate on verify | `canonical_extra_member_rejected` | tests/oc04_exec.rs | Parsed extra-member bytes → canonical gate Err; parsed value-tampered (forged) bytes → verify_execution Err |
| OC04-S08 | No f32/f64 tokens in new code | `no_float_tokens` | tests/oc04_schema.rs | include_str! scan of oc04_selection.rs |
| OC04-S09 | Signature issuance + verification round-trip | `signature_roundtrip` | tests/oc04_schema.rs | Sign body → verify Ok |
| OC04-S10 | Signature fails on wrong domain | `signature_domain_isolated` | tests/oc04_schema.rs | Same body signed over other domain → Err |
| OC04-U01 | Union dedups by EventId; `both` entry reason recorded | `union_dedup_both_reason` | tests/oc04_union.rs | Overlap → single entry, reason `both` |
| OC04-U02 | TF=0 event enters via prior arm only, reason `prior` | `tf_zero_enters_via_prior` | tests/oc04_union.rs | No lexical match + positive prior → included, reason `prior` |
| OC04-U03 | Prior vector is positive-only by OC-03 construction; no-positive-match entity contributes nothing | `zero_prior_no_entry` | tests/oc04_union.rs | Source whose derived entity key has no positive verified-vector match → absent from prior arm |
| OC04-U04 | Orphan prior entity skipped, counted (bound owned by X10) | `orphan_entity_counted` | tests/oc04_union.rs | Counter>0, Ok |
| OC04-U05 | Empty prior → union ≡ capped lexical arm | `empty_prior_identity` | tests/oc04_union.rs | Byte-equal to lexical-only |
| OC04-U06 | Prior-only candidates when lexical arm empty | `prior_only_union` | tests/oc04_union.rs | All prior-arm, reasons recorded |
| OC04-U07 | Union deterministic across input permutations | `union_permutation_stable` | tests/oc04_union.rs | Shuffled sources → identical union bytes |
| OC04-U08 | Per-arm caps enforced pre-union (64 lexical / 30 prior) | `per_arm_caps_enforced` | tests/oc04_union.rs | 100-candidate arms → exactly 64+30 capped entries |
| OC04-R01 | Normalization min-max to [0,1e6] ppm exact | `normalization_exact` | tests/oc04_rerank.rs | Hand-computed u128 values, clip bounds |
| OC04-R02 | Rank: score desc, canonical EventId TEXT asc (divergence pair) | `tie_break_canonical_text` | tests/oc04_rerank.rs | Fixture pair where text order ≠ raw-byte order; frozen order wins |
| OC04-R03 | Influence entry order = rerank order | `influence_order_matches` | tests/oc04_rerank.rs | Order zip-compare (entry-count completeness owned by R03b) |
| OC04-R03b | Influence entries cover every union member | `influence_covers_union` | tests/oc04_rerank.rs | len(entries) == union size |
| OC04-R04 | score_ppm = lexical_ppm + prior_ppm exactly | `formula_exact` | tests/oc04_rerank.rs | Per-entry sum assert, u128 |
| OC04-R05 | Degenerate arm (min=max) rule: >0→1e6, else 0 | `degenerate_arm_rule` | tests/oc04_rerank.rs | Single-candidate arm cases |
| OC04-R06 | Rerank deterministic on re-run | `rerank_determinism` | tests/oc04_rerank.rs | Two runs byte-identical |
| OC04-R07 | Prior-arm ranking: ppb desc, EventId asc tie | `prior_arm_ordering` | tests/oc04_rerank.rs | Equal-ppb fixtures → EventId asc |
| OC04-R08 | Multi-entity event folds to MAX ppb | `multi_entity_max_fold` | tests/oc04_rerank.rs | Event matching 2 entities → max, not sum |
| OC04-E01 | Execution binds pre-closure hash/count | `execution_binds_preclosure` | tests/oc04_exec.rs | Recompute over actual set |
| OC04-E02 | B3 policy + candidate fingerprints bound | `execution_binds_b3` | tests/oc04_exec.rs | Recompute from chain output |
| OC04-E03 | Delta hash + count bound | `execution_binds_delta` | tests/oc04_exec.rs | Recompute; both match B4 output |
| OC04-E04 | Recipient head bound as explicit body member | `execution_binds_recipient_head` | tests/oc04_exec.rs | Body member equals B5-verified head |
| OC04-E04b | b6_warnings_hash bound per derivation table | `execution_binds_b6_warnings` | tests/oc04_exec.rs | Recompute oc-04-b6warn-v1 hash over `Handoff::uncertainty()` exposure → equal |
| OC04-E04c | Normative exact-two-marker rule holds (used/empty alternative) | `b6_marker_rule_exact` | tests/oc04_exec.rs | prior-arm fixture → markers exactly {prior_arm_used, orphan_prior_entities=n}; empty-arm fixture → {prior_arm_empty, ...} |
| OC04-E05 | Influence/execution mismatch → Err, no artifact | `influence_mismatch_rejected` | tests/oc04_exec.rs | Bind-side: pre-closure is influence-derived (§7.3 structural identity — a mismatched influence cannot bind). Verify-side: envelope bound over chain A refused on chain B (recorded pre_closure/hash members ≠ replayed chain); plus forged envelope (canonical bytes, fresh signature) refused by replay |
| OC04-E06 | Budget: over-budget events → refusal | `budget_events_refusal` | tests/oc04_exec.rs | max_events exceeded alone → Err + reason |
| OC04-E06b | Budget: over-budget bytes → refusal | `budget_bytes_refusal` | tests/oc04_exec.rs | max_bytes exceeded alone → Err + reason |
| OC04-E07 | VerifiedPrior token privacy (compile gate) | `verified_prior_compile_gate` | tests/oc04_adversarial.rs (harness; snippet source: tests/compile/oc04_token_privacy.rs) | rustc-compile-fail: privacy-violating snippet exit-fail + expected E0xxx; runtime forgery → Err in X01 |
| OC04-E08 | Full-pipeline golden fixture (committed bytes + sha256) | `execution_golden` | tests/oc04_exec.rs | #[ignore] generator + committed fixture compared (generator run IS the pipeline) |
| OC04-E09 | B7 non-convergence → no artifact, Err | `b7_nonconvergence_no_artifact` | tests/oc04_exec.rs | Non-converging driver → bind_execution Err, no SignedExecutionV1 emitted |
| OC04-E10 | B8 simulate failure → no artifact, Err | `b8_failure_no_artifact` | tests/oc04_exec.rs | simulate fail fixture → bind_execution Err, no SignedExecutionV1 emitted |
| OC04-E11 | Checked-u128 overflow/out-of-range → Err (fail-closed) | `checked_overflow_rejected` | tests/oc04_exec.rs | u128 checked add of a u64 pair cannot overflow (max sum fits exactly) — the extreme exact-sum fixture proves determinism/no-clamp; overflow is structurally unreachable (documented vacuity, §17 U03-class) |
| OC04-X01 | Unverifiable prior bytes → VerifiedPrior::verify Err | `unverified_prior_rejected` | tests/oc04_adversarial.rs | Tampered prior bytes → verify Err (runtime; privacy itself is E07 compile gate) |
| OC04-X02 | Forged influence with re-derived id → Err | `forged_influence_rejected` | tests/oc04_adversarial.rs | Self-consistent forgery caught by rebuild divergence |
| OC04-X03 | Tampered execution signature → Err | `forged_execution_rejected` | tests/oc04_adversarial.rs | Body tamper with original sig → Err (wrong-key case covered by S10 domain isolation) |
| OC04-X04 | Baseline invariance: B2 output byte-equal with/without OC-04 | `baseline_invariance` | tests/oc04_adversarial.rs | select() on same inputs → identical Vec |
| OC04-X05 | Thorn structurally absent | `thorn_absent` | tests/oc04_adversarial.rs | No Thorn tokens in oc04 code |
| OC04-X06 | No new dependencies | `no_new_deps` | tests/oc04_adversarial.rs | Cargo.toml/lock diff empty vs committed (include_str! hash pin) |
| OC04-X07 | Duplicate event in both arms folded once, reason `both` | `duplicate_folded_once` | tests/oc04_adversarial.rs | Single entry |
| OC04-X08 | Stale-state handoff mismatch → no artifact, no handoff | `stale_state_no_artifact` | tests/oc04_adversarial.rs | Recipient head drift → Err |
| OC04-X09 | Non-canonical prior payload → VerifiedPrior::verify Err | `noncanonical_prior_payload_rejected` | tests/oc04_adversarial.rs | Equivalent JSON with non-canonical formatting → verify Err |
| OC04-X10 | Orphan count > 1024 → Err | `orphan_bound_fail_closed` | tests/oc04_adversarial.rs | 1,025 orphan entities fixture → union Err |
| OC04-X11 | Verifier replay: bind→verify Ok on same chain, production history unchanged | `verifier_replay_positive` | tests/oc04_adversarial.rs | bind→verify Ok; production RepairHistory bytes identical before/after |
| OC04-X11b | Verifier replay: tampered recorded handoff_hash → Err | `verifier_replay_wrong_hash` | tests/oc04_adversarial.rs | Envelope with falsified handoff_hash → verify Err |
| OC04-X12 | ScratchHistoryGuard rejects same-path as production history | `scratch_guard_same_path_rejected` | tests/oc04_adversarial.rs | scratch path == repair_history.path() → Err |
| OC04-X12b | ScratchHistoryGuard rejects pre-existing file | `scratch_guard_existing_file_rejected` | tests/oc04_adversarial.rs | Existing file at scratch path → Err (create_new reservation) |
