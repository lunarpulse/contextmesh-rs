---
title: 'OA-02 through OA-07 Test Traceability Matrix'
type: 'test-plan'
created: '2026-08-16'
status: 'approved-for-execution'
approved: '2026-08-16'
approved_by: 'Lunarpulse'
baseline_commit: 'f61c4f0d147544c4011b2bb8b8094943e196c883'
source_plan: './oa-02-oa-07-detailed-execution-plan.md'
---

# OA-02 through OA-07 Test Traceability Matrix

This minimum evidence set was approved by Lunarpulse on 2026-08-16 with the detailed execution plan and decisions D-02-01, D-05-01, and D-04-01. Package specs may add rows but may not remove or weaken rows without approval.

| ID | Package | Requirement | Planned evidence | Gate |
|---|---|---|---|---|
| 02-S01 | OA-02 | Fresh schema/idempotent reopen | schema_create_and_reopen | A4 |
| 02-S02 | OA-02 | Newer/corrupt schema fail closed | schema_fail_closed | A4 |
| 02-P01 | OA-02 | Explicit provisioning/idempotence | context_provision | A2 |
| 02-P02 | OA-02 | Exact genesis only | wrong_or_second_genesis_rollback | A2 |
| 02-P03 | OA-02 | Unauthorized author atomic reject | unauthorized_author | A2/A7 |
| 02-A01 | OA-02 | OA-01 verify before storage | contract_mutation_matrix | A1/A2 |
| 02-A02 | OA-02 | Missing/cross-context parent reject | parent_matrix | A2 |
| 02-A03 | OA-02 | Canonical wire retained exactly | canonical_wire_round_trip | A1/A4 |
| 02-A04 | OA-02 | Idempotence/collision | idempotence_and_collision | A2/A4 |
| 02-R01 | OA-02 | CAS absent/head success | cas_success | A3 |
| 02-R02 | OA-02 | Stale writer current head | stale_conflict | A3 |
| 02-R03 | OA-02 | Two writers one winner | two_writer_race | A3 |
| 02-R04 | OA-02 | Retry AlreadyApplied | retry_after_ack_loss | A3/A4 |
| 02-R05 | OA-02 | Namespace separation | namespace_separation | A3/A5 |
| 02-I01 | OA-02 | Immutable events/edges | immutable_surface_and_triggers | A2/A4 |
| 02-X01 | OA-02 | Every failure no mutation | all_failure_snapshots | A2 |
| 03-D01 | OA-03 | Append/fork/merge | chain_fork_merge | A3 |
| 03-D02 | OA-03 | Merge 2/64 and invalid shapes | merge_boundaries_matrix | A2/A3 |
| 03-P01 | OA-03 | Deterministic unique projection | diamond_projection_fixture | A3 |
| 03-P02 | OA-03 | Deep iterative projection | deep_projection | A3 |
| 03-P03 | OA-03 | Projection bounds | projection_limits | A7 |
| 03-B01 | OA-03 | Canonical bundle vector | bundle_v1_round_trip | A4/A5 |
| 03-B02 | OA-03 | Parent-first export | parent_first_union | A5 |
| 03-B03 | OA-03 | Bad event whole rollback | bad_event_atomic_rollback | A2/A5 |
| 03-B04 | OA-03 | Repeat import zero | idempotent_repeat | A4/A5 |
| 03-B05 | OA-03 | Remote-only ref updates | remote_only | A5 |
| 03-B06 | OA-03 | Strict bundle limits | bundle_adversarial_matrix | A2/A7 |
| 03-V01 | OA-03 | Restart full verification | restart_valid | A4 |
| 03-V02 | OA-03 | Corruption detection | corruption_matrix | A4 |
| 04-A01 | OA-04 | Loopback/auth required | loopback_and_auth_matrix | A7 |
| 04-A02 | OA-04 | Token non-disclosure | token_non_disclosure | A7 |
| 04-L01 | OA-04 | Transport bounds | transport_limit_matrix | A7 |
| 04-S01 | OA-04 | Missing-history pull | one_way_pull | A5 |
| 04-S02 | OA-04 | Local refs unchanged | local_ref_snapshot | A5 |
| 04-S03 | OA-04 | Repeat pull zero | converged_repeat | A5 |
| 04-S04 | OA-04 | Fork/merge parent-first | merged_history | A5 |
| 04-S05 | OA-04 | Stable pagination | immutable_head_pagination | A5 |
| 04-S06 | OA-04 | Invalid page no remote-ref update | invalid_page_rejection | A2/A5 |
| 04-T01 | OA-04 | Hostile server bounded | hostile_server | A7 |
| 04-T02 | OA-04 | Timeout retry safe | timeout_retry | A5 |
| 05-K01 | OA-05 | Persistent key identity | persistent_identity | A4/A6 |
| 05-K02 | OA-05 | Hostile filesystem reject | key_filesystem_matrix | A7 |
| 05-K03 | OA-05 | No secret output | secret_non_disclosure | A7 |
| 05-P01 | OA-05 | Request before call | request_before_call | A6 |
| 05-P02 | OA-05 | Linked response/error | result_matrix | A3/A6 |
| 05-P03 | OA-05 | Exact ancestry | ancestry_fixture | A6 |
| 05-P04 | OA-05 | Detached result on conflict | post_execution_conflict | A3/A6 |
| 05-P05 | OA-05 | Crash-window recovery | pending_detached_queries | A6/A7 |
| 05-C01 | OA-05 | Stable JSON/exits | cli_snapshot_matrix | A8 |
| 05-C02 | OA-05 | Restart verification | cli_restart | A4/A8 |
| 05-J01 | OA-05 | JSONL malformed/+1 | jsonl_adversarial | A7 |
| 06-D01 | OA-06 | Independent nodes/fork sync | demo stages 1-10 | A5/A8 |
| 06-D02 | OA-06 | Merge/same projection | demo stages 11-12 | A3/A5/A8 |
| 06-D03 | OA-06 | Restart verification | demo stages 13-14 | A4/A8 |
| 06-D04 | OA-06 | Repeat zero imports | demo stage 15 | A5/A8 |
| 06-D05 | OA-06 | Tamper atomic reject | demo stage 16 | A2/A8 |
| 06-D06 | OA-06 | Cleanup and no secrets | lifecycle/security matrix | A7/A8 |
| 07-R01 | OA-07 | Clean locked quality/demo | verify-oa07 transcript | A8 |
| 07-R02 | OA-07 | Dependency/advisory audit | release evidence | A7/A8 |
| 07-R03 | OA-07 | Claim audit | claim audit artifact | A7/A8 |
| 07-R04 | OA-07 | A1-A8 evidence links | release matrix | A1-A8 |
| 07-R05 | OA-07 | No committed secrets/state | tree/status scan | A7/A8 |
| 07-R06 | OA-07 | Explicit verdict/gate | OA-07 spec | A8 |

## Mandatory independent review layers

| Package | Review layers |
|---|---|
| OA-02 | schema/migration; transaction/concurrency; policy/admission; API/supply chain |
| OA-03 | graph determinism; parser/resource; atomic import; corruption/verification |
| OA-04 | protocol state machine; auth/secrets; hostile network/resource; dependency/claims |
| OA-05 | provider crash/race; key/filesystem; CLI output/parser; semantic-claim boundary |
| OA-06 | shell/process lifecycle; reproducibility; tamper evidence; docs/security claims |
| OA-07 | end-to-end red team; supply chain; requirements traceability; final claim audit |
