# OC-02 Test Traceability Matrix

**Status:** draft-for-founder-review · **Spec:** `spec-oc-02-attribution.md`
**Rule:** every matrix row maps to exactly one committed test; no unmapped rows; evidence text must match executable behavior (overclaim = blocker).

## Conventions

- Row ID prefix: T=schema, A=mechanisms, S=shortlist, J=judges, R=reports, X=adversarial/privacy, V=evaluation harness.
- Boundary notation: `0/exact-max/max+1` tests are three vectors in one row unless stated.
- `verify-oc02.sh` stage roll-up maps prefixes to gates (§ Gate roll-up).

## Schema, tags, and configuration

| Row | Requirement | Test name | File | Evidence |
|---|---|---|---|---|
| OC02-T01 | Mechanism enum is exactly M0..M4; no other variants; Display round-trips | `mechanism_enum_is_exact_and_roundtrips` | tests/oc02_schema.rs | Enum exhaustiveness + 5-way round-trip; 6th variant fails to compile (cfg_test probe) |
| OC02-T02 | Extractor version strings equal frozen prereg strings verbatim | `extractor_versions_match_frozen_prereg` | tests/oc02_schema.rs | Asserts equality against literals; compares with `p1-prereg-config.json` committed copy |
| OC02-T03 | AttributionConfigV1 canonical bytes are deterministic JCS | `config_canonical_bytes_are_deterministic` | tests/oc02_schema.rs | Two serializations byte-equal; key order canonical |
| OC02-T04 | Config hash = BLAKE3(`oc-02-attr-config-v1`+NUL, canonical bytes); typed prefix | `config_hash_domain_and_prefix_exact` | tests/oc02_schema.rs | Domain bytes incl. NUL asserted; wrong domain differs; prefix `ocattrcfg1_` |
| OC02-T05 | Tag schema: exactly 3 required members; unknown member rejects Malformed | `tag_schema_strictness` | tests/oc02_schema.rs | Positive + unknown-member + missing-member + null vectors |
| OC02-T06 | Report ID = BLAKE3(`oc-02-attr-report-v1`+NUL, canonical bytes); prefix `ocattr1_` | `report_id_domain_separation_exact` | tests/oc02_schema.rs | Domain bytes asserted; derive-key/undomained both differ (OC-01 P01 pattern) |
| OC02-T07 | ShortlistV1 schema: order rule `score_ppm desc, EventId asc`; cap field frozen 32 | `shortlist_schema_and_order_rule` | tests/oc02_schema.rs | Schema strictness + ordering arithmetic |
| OC02-T08 | CausalSectionV1 status enum exactly `computed/unavailable/no_nominations` | `causal_status_enum_exact` | tests/oc02_schema.rs | Exhaustive match + unknown rejects |
| OC02-T09 | Report envelope: exactly the 10 top-level members of spec §7.5; unknown rejects | `report_envelope_exact_members` | tests/oc02_schema.rs | Positive + unknown-member + missing-member |
| OC02-T10 | `prereg_reference` equals the frozen SHA-256 `be20d8fc…` verbatim | `prereg_reference_is_frozen_hash` | tests/oc02_schema.rs | Literal equality; any other 64-hex rejects at schema level |

## Mechanisms M0/M1/M2

| Row | Requirement | Test name | File | Evidence |
|---|---|---|---|---|
| OC02-A01 | M0 nominates on exact raw token overlap with outcome evidence | `m0_exact_overlap_nominates` | tests/oc02_mechanisms.rs | Lone-carrier fixture: M0 edge present with tag+version+config hash |
| OC02-A02 | M0 does not nominate reformatted values | `m0_reformat_blind_spot_held` | tests/oc02_mechanisms.rs | `9.5M` vs `9500000`: no M0 edge (documented blind spot) |
| OC02-A03 | M0 token extraction bounded: 256/event, 1,024B/token | `m0_token_bounds_enforced` | tests/oc02_mechanisms.rs | 256 ok / 257 recorded-skip; 1,024 ok / 1,025 skip |
| OC02-A04 | M0 nomination domain limited to ledger-referenced events | `m0_outside_ledger_refs_rejected` | tests/oc02_mechanisms.rs | Non-referenced event → `UnauthorizedEvent`, no edge |
| OC02-A05 | M0 deterministic: same inputs → identical edges | `m0_deterministic_reproduction` | tests/oc02_mechanisms.rs | Two runs byte-equal |
| OC02-A06 | M1 nominates unit-suffixed numerics (k/M/B/G, %) | `m1_normalized_numeric_nominates` | tests/oc02_mechanisms.rs | `9.5M` ↔ `9500000` edge present |
| OC02-A07 | M1 magnitude bound 10^18; out-of-range skips with record | `m1_magnitude_bound` | tests/oc02_mechanisms.rs | 10^18 ok; 10^18+1 recorded-skip, no error |
| OC02-A08 | M1 case/whitespace/path folding deterministic | `m1_folding_rules` | tests/oc02_mechanisms.rs | Fixture set of fold pairs |
| OC02-A09 | M2 explicit EventId citation recognized | `m2_eventid_citation_positive` | tests/oc02_mechanisms.rs | Canonical EventId in payload → citation edge |
| OC02-A10 | M2 forged citation (event absent) rejects | `m2_forged_link_negative` | tests/oc02_mechanisms.rs | Nonexistent EventId → rejected, recorded as forged, no edge |
| OC02-A11 | M2 provider request/result linkage from public metadata only | `m2_provider_linkage_positive` | tests/oc02_mechanisms.rs | Core public metadata pair → linkage edge |
| OC02-A12 | M2 receipt/handoff references recognized | `m2_receipt_reference_positive` | tests/oc02_mechanisms.rs | Option B receipt ref → receipt edge |
| OC02-A13 | M2 summary coverage enumeration recognized | `m2_summary_coverage_positive` | tests/oc02_mechanisms.rs | Enumeration covering listed events → summary edge |
| OC02-A14 | M2 signed artifact references (`ocout1_`) recognized | `m2_signed_artifact_reference_positive` | tests/oc02_mechanisms.rs | Ledger ID ref → artifact edge |
| OC02-A15 | M2 recognizes exactly the five structures; near-miss text does not | `m2_exactly_five_structures` | tests/oc02_mechanisms.rs | Five positives; paraphrase/citation-like text negative |
| OC02-A16 | M2 every edge records extractor identity, version, config hash | `m2_provenance_on_every_edge` | tests/oc02_mechanisms.rs | All edges carry 3 provenance fields |
| OC02-A17 | M2 never re-derives LLM-inferred citations during verification | `m2_no_rederivation_during_verify` | tests/oc02_mechanisms.rs | Verify path reads recorded edges only (D-C-07) |
| OC02-A18 | Nomination evidence uses fingerprints, never raw transcript bytes | `mechanisms_fingerprint_only_evidence` | tests/oc02_mechanisms.rs | Edge bytes contain no payload literals (canary scan) |
| OC02-A19 | M0+M1+M2 combined fixture reproduces edges deterministically | `combined_deterministic_tier_reproduction` | tests/oc02_mechanisms.rs | Full fixture rebuild byte-equal |
| OC02-A20 | Tampered ledger rejected before any nomination work | `tampered_ledger_rejected_early` | tests/oc02_mechanisms.rs | Flipped signature → error, zero nomination side effects |
| OC02-A21 | Unterminated ledger: deterministic tier may nominate; no causal content | `unterminated_deterministic_only` | tests/oc02_mechanisms.rs | Report with marker `no_terminal_outcome` |
| OC02-A22 | Ledger caller marks are inputs to nothing | `caller_marks_not_promoted` | tests/oc02_mechanisms.rs | Marks present in ledger → absent from every report section |
| OC02-A23 | EventSource refuses events outside verified context | `eventsource_context_boundary` | tests/oc02_mechanisms.rs | Cross-context request → `UnauthorizedEvent` |
| OC02-A24 | Empty nomination set is `no nominations`, success not error | `empty_nomination_is_success` | tests/oc02_mechanisms.rs | Zero events → shortlist empty + recorded marker |
| OC02-A25 | All mechanism arithmetic u128-widened, checked, fail-closed | `arithmetic_checked_overflow` | tests/oc02_mechanisms.rs | Overflow-injection vectors reject, never wrap |
| OC02-A26 | Hostile payloads panic-free (BOM, deep, dup keys, NaN) | `mechanisms_panic_free_hostile` | tests/oc02_mechanisms.rs | catch_unwind clean; Malformed or skip-and-record |

## Shortlist policy

| Row | Requirement | Test name | File | Evidence |
|---|---|---|---|---|
| OC02-S01 | Union of M0–M2 nominations EventId-deduplicated | `shortlist_union_dedup` | tests/oc02_shortlist_judges.rs | Multi-mechanism same-event → single entry, mechanisms merged |
| OC02-S02 | Cap 32 at boundaries 0/32/33 | `shortlist_cap_boundaries` | tests/oc02_shortlist_judges.rs | 0 ok; 32 ok; 33 → capped deterministically, overflow recorded |
| OC02-S03 | Deterministic order score desc, EventId asc; boolean M0–M2 nominees score exactly 1,000,000 ppm | `shortlist_order_deterministic` | tests/oc02_shortlist_judges.rs | Tie-score pair ordered by EventId; no OC-03/OC-04 score channel |
| OC02-S04 | Empty shortlist recorded `no nominations`, not error | `shortlist_empty_recorded` | tests/oc02_shortlist_judges.rs | Stage 2E emits exact `CausalStatus::NoNominations` marker/status; complete `CausalSectionV1` assembly/serialization is Stage 2H-owned |
| OC02-S05 | Shortlist recall computed separately from verifier recall | `shortlist_recall_recorded_separately` | tests/oc02_shortlist_judges.rs | recall_basis {nominated, eligible} present and correct (D-C-06 #3) |
| OC02-S06 | Shortlist cap arithmetic checked (no overflow at bounds) | `shortlist_arithmetic_checked` | tests/oc02_shortlist_judges.rs | u128 widened path |
| OC02-S07 | Shortlist never includes non-ledger-referenced events | `shortlist_domain_purity` | tests/oc02_shortlist_judges.rs | Injected foreign event absent |
| OC02-S08 | Shortlist bytes reproduce deterministically | `shortlist_byte_reproduction` | tests/oc02_shortlist_judges.rs | Rebuild byte-equal |

## Judge adapters (M3/M4)

| Row | Requirement | Test name | File | Evidence |
|---|---|---|---|---|
| OC02-J01 | M3 executes only on shortlist entries | `m3_shortlist_only_execution` | tests/oc02_shortlist_judges.rs | Non-shortlist ablation request rejected by construction |
| OC02-J02 | M3 call cap 8/session at 0/8/9 | `m3_call_cap_boundaries` | tests/oc02_shortlist_judges.rs | 9th call → `MechanismUnavailable` with cap marker, deterministic tier intact |
| OC02-J03 | M3 records judge identity/version/config hash per call | `m3_call_provenance` | tests/oc02_shortlist_judges.rs | Every delta carries 3 provenance fields |
| OC02-J04 | Judge None → causal tier unavailable, deterministic tier completes | `judge_none_fail_closed` | tests/oc02_shortlist_judges.rs | Report status `unavailable`, marker `judge_unavailable`, shortlist intact |
| OC02-J05 | JudgeUnavailable mid-run → same fail-closed, partial M3 results recorded | `judge_unavailable_midrun` | tests/oc02_shortlist_judges.rs | Completed deltas kept; remaining `unavailable` |
| OC02-J06 | Unavailable paths contain no causal claim vocabulary | `unavailable_no_causal_vocabulary` | tests/oc02_shortlist_judges.rs | Grep-scan of report bytes for claim words = 0 |
| OC02-J07 | M4 executes only on shortlist | `m4_shortlist_only_execution` | tests/oc02_shortlist_judges.rs | Same construction proof |
| OC02-J08 | M4 samples/candidate cap 64 at 0/64/65 | `m4_sample_cap_boundaries` | tests/oc02_shortlist_judges.rs | 65th sample → capped recorded |
| OC02-J09 | M4 judge-call cap 128/session at 0/128/129 | `m4_call_cap_boundaries` | tests/oc02_shortlist_judges.rs | 129th → `MechanismUnavailable` cap marker |
| OC02-J10 | M4 splits credit on redundant pair | `m4_redundant_pair_credit_split` | tests/oc02_shortlist_judges.rs | Redundant-carrier fixture: shares sum ≤ 1e6 ppm, both > 0 |
| OC02-J11 | M3 under-marks redundant pair by design; both outcomes recorded | `m3_undermarks_redundant_by_design` | tests/oc02_shortlist_judges.rs | M3 `unchanged` on one-of-pair; recorded alongside M4 split |
| OC02-J12 | Adapter tier records judge transcript verbatim; verify never re-queries | `adapter_tier_verbatim_transcript` | tests/oc02_shortlist_judges.rs | Verify compares bytes to transcript; judge handle not called on verify |
| OC02-J13 | Judge identity is recorded, never inferred | `judge_identity_recorded_not_inferred` | tests/oc02_shortlist_judges.rs | Trait object without identity metadata rejected at type level |
| OC02-J14 | Session definition: caps counted per (ledger, context) | `caps_counted_per_session_definition` | tests/oc02_shortlist_judges.rs | Two ledgers same store: independent counters |

## Report assembly and verification

| Row | Requirement | Test name | File | Evidence |
|---|---|---|---|---|
| OC02-R01 | Report ID flips on any byte change (tamper matrix) | `report_id_tamper_matrix` | tests/oc02_reports.rs | Flip each top-level member → different report_id; original re-renders |
| OC02-R02 | Deterministic tier rebuild byte-exact from (ledger, events, config) | `deterministic_tier_byte_rebuild` | tests/oc02_reports.rs | Rebuild equals committed fixture bytes |
| OC02-R03 | Adapter tier equals recorded transcript on replay | `adapter_tier_transcript_replay` | tests/oc02_reports.rs | Replayed judge transcript → identical adapter bytes |
| OC02-R04 | Cross-ledger report rejected | `cross_ledger_report_rejected` | tests/oc02_reports.rs | Report referencing other ledger_id → verification failure |
| OC02-R05 | Cross-context event rejection at report level | `cross_context_report_rejected` | tests/oc02_reports.rs | Foreign-context event in edges → reject, no partial artifact |
| OC02-R06 | Unterminated ledger: no fabricated causal content | `unterminated_no_fabrication` | tests/oc02_reports.rs | Marker present; adapter status not `computed` |
| OC02-R07 | Golden report fixture: bytes + SHA-256 committed, generator ignored | `golden_report_fixture_immutable` | tests/oc02_reports.rs | Suite compares committed bytes; generator `#[ignore]` |
| OC02-R08 | Report verification reuses OC-01 ledger verification first | `report_verify_reuses_ledger_verify` | tests/oc02_reports.rs | Tampered ledger fails at ledger step (ordering proof) |

## Adversarial, boundary, and privacy

| Row | Requirement | Test name | File | Evidence |
|---|---|---|---|---|
| OC02-X01 | All caps at 0/exact-max/max+1 (consolidated boundary sweep) | `all_caps_boundary_sweep` | tests/oc02_adversarial.rs | shortlist 32, M3 8, M4 samples 64, M4 calls 128, tokens 256, token B 1,024, magnitude 1e18 |
| OC02-X02 | Hostile JSON payloads panic-free, Malformed or skip | `hostile_payloads_panic_free` | tests/oc02_adversarial.rs | 8 hostile classes incl. depth>64 |
| OC02-X03 | No credentials/private paths/raw transcripts in any report bytes | `privacy_scan_all_reports` | tests/oc02_adversarial.rs | Canary + path + credential scan (OC-01 X18/X19 pattern) |
| OC02-X04 | Error surface uses only reserved OC-01 categories; no new categories | `error_categories_unchanged` | tests/oc02_adversarial.rs | 12-category OC-01 enum (spec-oc-01 §5.3 error table) asserted unchanged; `MechanismUnavailable`/`UnauthorizedEvent` used, not invented |
| OC02-X05 | Fail-closed: no partial artifact on any mid-pipeline failure | `no_partial_artifact_on_failure` | tests/oc02_adversarial.rs | Injected failures at each stage → Err only |
| OC02-X06 | Deterministic under repeated hostile re-runs | `hostile_rerun_stability` | tests/oc02_adversarial.rs | Same hostile inputs → identical outcomes |
| OC02-X07 | Extremely long token lists bounded (memory safety) | `bounded_memory_hostile_inputs` | tests/oc02_adversarial.rs | 4,096-occurrence ledger with max tokens → bounded time/memory |
| OC02-X08 | Forged citations across all five M2 structures reject | `forged_structure_matrix` | tests/oc02_adversarial.rs | One forged vector per structure kind |
| OC02-X09 | Report bytes contain no mark-promotion leakage | `no_mark_promotion_leakage` | tests/oc02_adversarial.rs | Marks never appear in report text |
| OC02-X10 | Non-canonical report bytes rejected (whitespace/key-order) | `noncanonical_report_rejected` | tests/oc02_adversarial.rs | Whitespace insertion → reject (OC-01 I26 pattern) |

## Evaluation harness (deterministic-replay mode)

| Row | Requirement | Test name | File | Evidence |
|---|---|---|---|---|
| OC02-V01 | Harness loads frozen P1 config and verifies its SHA-256 | `harness_verifies_frozen_prereg` | tests/oc02_evaluation.rs | Refuses to run if hash ≠ `be20d8fc…` |
| OC02-V02 | Harness computes per-mechanism P/R/F1 and judge-call economics | `harness_metric_computation` | tests/oc02_evaluation.rs | Fixed replay corpus → exact expected numbers |
| OC02-V03 | Shortlist recall reported separately | `harness_shortlist_recall_separate` | tests/oc02_evaluation.rs | Present in output, distinct field |
| OC02-V04 | Stratum minimums enforced; inconclusive reported, gate never lowered | `harness_stratum_minimums` | tests/oc02_evaluation.rs | Under-minimum corpus → `inconclusive` |
| OC02-V05 | Harness is deterministic-replay only; no live judge calls in tests | `harness_replay_only` | tests/oc02_evaluation.rs | Zero network/judge side effects (trait null-object) |

## Gate roll-up

| Gate | Rows |
|---|---|
| `OC02-SCHEMA` | OC02-T01..T10 |
| `OC02-MECHANISMS` | OC02-A01..A26 |
| `OC02-SHORTLIST` | OC02-S01..S08 |
| `OC02-JUDGES` | OC02-J01..J14 |
| `OC02-REPORTS` | OC02-R01..R08 |
| `OC02-ADVERSARIAL` | OC02-X01..X10 |
| `OC02-REGRESSION` | workspace + legacy chains unchanged |
| `OC02-EVIDENCE` | four-layer evidence + claim audit |
| `OC02-EVALUATION` | OC02-V01..V05 + executed E1-rerun report (C2 completion prerequisite) |

**Total: 81 rows.**

## Approval record

- 2026-08-26: draft created alongside spec; dual independent review and founder approval pending. No implementation authorized.
- 2026-08-26: quality REJECT fixed (81 rows verified; ADVERSARIAL gate added) → re-review APPROVE; compliance GO.
- 2026-08-26: **Lunarpulse approved the spec and this matrix (81 rows) for freezing** (Discord message `1541934459069145168`). Implementation authorized in spec §3 dependency order.
- 2026-08-27: **Lunarpulse approved the minimal S03/S04 Stage 2E freeze clarification** (Discord message `1542499082533343264`): boolean M0–M2 nominees score exactly 1,000,000 ppm; Stage 2E emits the `no_nominations` marker/status while Stage 2H owns complete causal-section assembly. Row count and gate IDs are unchanged.
