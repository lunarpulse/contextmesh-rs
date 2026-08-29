# OC-03 test traceability matrix

```yaml
doc: oc-03-test-traceability-matrix
spec: ../implementation-artifacts/spec-oc-03-prior.md
status: draft — dual review pending, founder freeze approval pending
```

Total: **52 rows** — T8 + G12 + P14 + A10 + X8.

## Gate oc03_schema (T01–T08) — tests/oc03_schema.rs

| Row | Requirement | Test name | File | Evidence |
|---|---|---|---|---|
| OC03-T01 | Prior extractor version string is exactly `oc-3-prior-v1` | `prior_version_wire` | tests/oc03_schema.rs | Constant round-trips as frozen |
| OC03-T02 | Thorn status marker is exactly `thorn_disabled` | `thorn_status_marker` | tests/oc03_schema.rs | Marker literal round-trips; no Thorn type reachable |
| OC03-T03 | All §5 caps round-trip as frozen literals | `caps_frozen_literals` | tests/oc03_schema.rs | 1024/32/8/64/64/1e6/850000/1e9 asserted exactly |
| OC03-T04 | `PriorConfigV1` canonical bytes are JCS-lexicographic | `config_canonical_order` | tests/oc03_schema.rs | Byte-render equals manual JCS render |
| OC03-T05 | `validate_frozen` fails closed on any deviation | `config_validate_frozen` | tests/oc03_schema.rs | Each mutated member → Err; unmutated → Ok |
| OC03-T06 | Config hash = BLAKE3(`oc-03-priorcfg1\0` + bytes), prefix `ocpriorcfg1_` | `config_hash_domain` | tests/oc03_schema.rs | Independent re-hash matches |
| OC03-T07 | Prereg reference literal matches P1 seal `be20d8fc…eae784c9` | `prereg_reference_seal` | tests/oc03_schema.rs | Literal equals frozen SHA-256 string |
| OC03-T08 | Envelope has exactly the 13 §7.4 members, no more, no fewer | `envelope_member_set` | tests/oc03_schema.rs | Parsed member set equals spec set exactly |

## Gate oc03_graph (G01–G12) — tests/oc03_graph_seeds.rs

| Row | Requirement | Test name | File | Evidence |
|---|---|---|---|---|
| OC03-G01 | Entity keys: canonical M2 ID recognized | `entity_key_canonical_id` | tests/oc03_graph_seeds.rs | evt1_/rcpt1_/ocout1_ 43-char keys extracted |
| OC03-G02 | Entity keys: M1 normalized value spellings | `entity_key_normalized` | tests/oc03_graph_seeds.rs | path:/pct:/num: keys extracted via M1, one per NormalizedValue variant |
| OC03-G03 | Entity keys: M0 token fallback ≤1024 bytes | `entity_key_token` | tests/oc03_graph_seeds.rs | Long token truncated/rejected per cap |
| OC03-G04 | Per-event cap 8, byte-sorted, deduped | `entity_key_event_cap` | tests/oc03_graph_seeds.rs | 9 keys → 8 kept sorted, drop counted |
| OC03-G05 | Graph edges undirected, a<b, sorted, deduped | `graph_canonical_edges` | tests/oc03_graph_seeds.rs | Co-occurrence renders canonical edge list |
| OC03-G06 | Entity cap 1,024 with `truncated_entities` counter | `graph_entity_cap` | tests/oc03_graph_seeds.rs | 1,025 entities → 1,024 kept, counter =1 |
| OC03-G07 | Edge cap 32/entity with `truncated_edges` counter | `graph_edge_cap` | tests/oc03_graph_seeds.rs | 33 edges → 32 kept, counter ≥1 |
| OC03-G08 | Parent-ledger sessions contribute adjacency | `graph_parent_sessions` | tests/oc03_graph_seeds.rs | Parent+child session entity sets union into edges |
| OC03-G09 | Seeds from Complete M4 sections only; unavailable/no_nominations sections yield zero | `seeds_complete_sections_only` | tests/oc03_graph_seeds.rs | Unavailable/no_nominations sections and 0-ppm shares yield zero seeds |
| OC03-G10 | share_ppm ×1,000 → ppb, clamped at 1e9, u128 checked | `seed_ppb_conversion` | tests/oc03_graph_seeds.rs | 500000 ppm → 500,000,000 ppb; clamp + checked math |
| OC03-G11 | Unavailable report → zero seeds, `unavailable_reports`+1 | `seeds_unavailable_marker` | tests/oc03_graph_seeds.rs | Explicit marker recorded, no error |
| OC03-G12 | Seed cap 64: descending ppb, then entity asc; drops counted | `seed_cap_ordering` | tests/oc03_graph_seeds.rs | 65 seeds → 64 kept per rule, drop in artifact `dropped_seeds` |

## Gate oc03_ppr (P01–P14) — tests/oc03_propagation.rs

| Row | Requirement | Test name | File | Evidence |
|---|---|---|---|---|
| OC03-P01 | Teleport = floor(seed × 850000/1e6) | `ppr_teleport_floor` | tests/oc03_propagation.rs | Known seeds → mass ≥ teleport floor (exactness owned by P06) |
| OC03-P02 | Propagation term floor per neighbor | `ppr_neighbor_floor` | tests/oc03_propagation.rs | Hand-computed neighbor term matches floored output |
| OC03-P03 | Summation order canonical (neighbor byte order) | `ppr_summation_order` | tests/oc03_propagation.rs | Permuted input graph → identical output bytes |
| OC03-P04 | Convergence: delta ≤ 1e6 ppb stops with converged=true | `ppr_convergence_stop` | tests/oc03_propagation.rs | Delta sequence reaches threshold, flag true |
| OC03-P05 | Iteration cap 64 | `ppr_iteration_cap` | tests/oc03_propagation.rs | Bound contract: iterations ≤ 64, no error; converged flag recorded honestly (no non-converging fixture exists within frozen caps — monotone masses decay to floor-zero) |
| OC03-P06 | Degree-0 entity retains teleport only | `ppr_isolated_entity` | tests/oc03_propagation.rs | No neighbors → mass = teleport |
| OC03-P07 | Degree-0 entity contributes no outflow to any neighbor | `ppr_isolated_no_outflow` | tests/oc03_propagation.rs | Removing an isolated entity leaves all other masses byte-identical |
| OC03-P08 | All arithmetic u128 checked; range violation → Err | `ppr_overflow_fail_closed` | tests/oc03_propagation.rs | Hub-concentrated legal seeds drive one vector entry >1e9 → Err(Malformed), no partial artifact |
| OC03-P09 | No float anywhere in the propagation path | `ppr_no_float` | tests/oc03_propagation.rs | Test reads src/prior.rs via include_str! and asserts no f32/f64 tokens in non-comment lines |
| OC03-P10 | Vector lists ppb>0 only, entity byte order | `ppr_vector_ordering` | tests/oc03_propagation.rs | Zero-mass entities absent from vector |
| OC03-P11 | Values ≤ PRIOR_MAX_PPB asserted | `ppr_range_assert` | tests/oc03_propagation.rs | Max value ≤1e9 on well-formed artifacts; the >1e9 rejection path is owned by P08 |
| OC03-P12 | Empty seeds → empty vector, valid artifact | `ppr_empty_seeds` | tests/oc03_propagation.rs | All-unavailable corpus → empty vector, warnings present |
| OC03-P13 | Deterministic rerun byte-identical | `ppr_determinism` | tests/oc03_propagation.rs | 20 reruns → identical canonical bytes |
| OC03-P14 | Quantization residual equals the exact §7.6 formula | `ppr_residual_recorded` | tests/oc03_propagation.rs | residual_ppb equals hand-computed ⌊Σ r_u / 1e12⌋ over final iteration |

## Gate oc03_artifact (A01–A10) — tests/oc03_artifact.rs

| Row | Requirement | Test name | File | Evidence |
|---|---|---|---|---|
| OC03-A01 | Envelope assembly from graph+seeds+vector per §7.4 | `artifact_assembly` | tests/oc03_artifact.rs | Member values equal inputs |
| OC03-A02 | prior_id = BLAKE3(`oc-03-prior-v1\0` + placeholder-normalized bytes) | `prior_id_derivation` | tests/oc03_artifact.rs | Independent hash over placeholder bytes matches |
| OC03-A03 | prior_id flips on any byte change | `prior_id_tamper_matrix` | tests/oc03_artifact.rs | Three scalar-member mutations (config_hash prefix, terminal_status, residual key) each → verify Err |
| OC03-A04 | verify_prior recomputes; byte-identical required | `verify_recompute` | tests/oc03_artifact.rs | Rebuild from inputs verifies Ok; mutated → Err |
| OC03-A05 | verify_prior never trusts recorded intermediates | `verify_no_trust` | tests/oc03_artifact.rs | Self-consistent forged vector (re-derived prior_id) → Err via rebuild divergence; forged id → Err |
| OC03-A06 | Mixed terminal statuses rejected at assembly | `mixed_terminal_rejected` | tests/oc03_artifact.rs | terminal+unterminated reports → derive_seeds Err; unknown spelling → assemble Err |
| OC03-A07 | Golden fixture byte equality | `golden_prior_fixture_immutable` | tests/oc03_artifact.rs | Suite compares committed bytes; generator `#[ignore]` |
| OC03-A08 | Golden fixture SHA-256 sidecar matches | `golden_fixture_sha256` | tests/oc03_artifact.rs | sha256sum of file equals sidecar |
| OC03-A09 | Unverified report input → whole build Err | `unverified_report_rejected` | tests/oc03_artifact.rs | Malformed report bytes → parser Err; artifact verified against falsified reports → Err |
| OC03-A10 | Non-canonical JSON spellings rejected | `noncanonical_spelling_rejected` | tests/oc03_artifact.rs | Reordered/whitespace/padded JSON → parse Err |

## Gate ADVERSARIAL (X01–X08) — tests/oc03_adversarial.rs

| Row | Requirement | Test name | File | Evidence |
|---|---|---|---|---|
| OC03-X01 | Forged ocattr1_-shaped report rejected | `forged_report_rejected` | tests/oc03_adversarial.rs | verify_report failure → whole build Err |
| OC03-X02 | Tampered vector byte → verify failure | `tampered_vector_detected` | tests/oc03_adversarial.rs | Single byte flip → Err |
| OC03-X03 | Cross-config rebuild → different prior_id | `cross_config_divergence` | tests/oc03_adversarial.rs | Mutated config → different ID, verify fails |
| OC03-X04 | Graph overflow beyond caps → counters, never errors | `graph_overflow_counters` | tests/oc03_adversarial.rs | Massive corpus → Ok with counters>0 |
| OC03-X05 | Seed overflow → counted drop, valid artifact | `seed_overflow_counted` | tests/oc03_adversarial.rs | >64 seeds → drop counted, artifact valid |
| OC03-X06 | Negative/Thorn inputs structurally absent | `thorn_unreachable` | tests/oc03_adversarial.rs | No API accepts negative ppb or Thorn payloads |
| OC03-X07 | Duplicate report IDs folded once | `duplicate_reports_folded` | tests/oc03_adversarial.rs | Same report twice → single seed contribution |
| OC03-X08 | Falsified residual/iterations detected by verify | `falsified_metadata_detected` | tests/oc03_adversarial.rs | Altered converged/iterations/residual → Err |
