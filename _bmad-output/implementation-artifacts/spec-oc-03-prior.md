# OC-03 — Salience Prior (positive subset) implementation specification

```yaml
doc: spec-oc-03-prior
epic: option-c
package: P2 (OC-02 + positive C3 subset)
status: draft — dual review pending, founder freeze approval pending
upstream: spec-oc-02-attribution.md (frozen e28cacb, clarified d516c35)
prereg: ../implementation-artifacts/p1-prereg-config.json (SHA-256 be20d8fc…eae784c9)
priority_plan: ../planning-artifacts/option-c-priority-and-gate-plan.md
matrix: ../planning-artifacts/oc-03-test-traceability-matrix.md
```

## 1. Intent, scope, and non-claims

**Intent.** Propagate positive load-bearing knowledge across sessions: verified
OC-02 attribution reports are folded into a versioned, signed, nonnegative
fixed-point `SaliencePriorV1` vector over entity keys, computed by deterministic
bounded-graph personalized propagation (integer fixed-point, no floats).

**In scope.** Entity-key derivation (reusing frozen M1/M2 extractors), bounded
entity graph construction, seed derivation from OC-02 M4 contributing shares,
integer fixed-point propagation to convergence or frozen iteration cap, ppb
quantization, `SaliencePriorV1` artifact assembly with content-derived ID,
structural verification by recomputation, adversarial vectors.

**Out of scope (non-claims).**
- No Thorn. `ThornIndexV1`, negative seeds, suppression, expiry semantics for
  dead ends — all deferred to the P4 independent gate. OC-03 records only the
  frozen status marker `thorn_disabled`.
- No selection, fusion, reranking, or Option B integration (OC-04 / C4).
- No claim of salience utility. Utility is claimed only by OC-04's human-gold
  gate under the preregistered thresholds.
- No wall-clock, no I/O, no model inference. Expiry of prior entries is
  recorded data supplied by the caller, never fabricated.

**Input contract.** A nonempty set of structurally verified `AttributionReportV1`
artifacts (OC-02 §7.5) plus their parent OC-01 ledgers. Reports are accepted
only after `verify_report` succeeds on each; any failure rejects the whole
build with `Err` (no partial artifact).

## 2. Definitional gaps closed (prereg-record §4.6 discipline)

The P1 prereg froze the prior channel (positive-only), range (0..=1,000,000,000
ppb), and thorn status (disabled). It did not freeze propagation internals.
This spec freezes them; none alters a prereg value:

| Gap | Closed value |
|---|---|
| Entity key | exactly the reuse of frozen M2 canonical IDs + M1 normalized values, per event, capped 8/event (§5) |
| Graph bound | ≤1,024 entities, ≤32 edges/entity, canonical-order truncation with overflow recorded |
| Propagation | integer fixed-point PPR: recurrence of §7.6, all-u128 checked, no floats |
| Convergence | L∞ delta ≤ 1,000,000 ppb, or 64 iterations with `iteration_cap` marker recorded (never an error) |
| Seed mapping | M4 `contributing` shares only; share_ppm ×1,000 → ppb seeds, per-entity max-fold |
| Quantization | floor to ppb; per-iteration floor losses recorded as `residual_ppb` |
| Unavailable adapter | report contributes zero seeds; `judge_unavailable` marker recorded in the artifact (explicit warning, never silent filtering) |

## 3. Dependency-order implementation contract

Stages execute strictly in order; each ends with focused tests green, full
workspace regression EXIT 0, clippy `-D warnings` clean, fmt clean, then dual
review (Compliance GO + Quality APPROVE), then founder approval, then commit.

| Stage | Gate | Content |
|---|---|---|
| 3A | oc03_workspace | zero OC-03 references in existing code; zero new dependencies; direction and feature tree unchanged |
| 3B | oc03_schema | `prior.rs`: version strings, caps module, `PriorConfigV1` + `validate_frozen` + BLAKE3 domain hash, `thorn_disabled` marker |
| 3C | oc03_graph | entity-key derivation + bounded `EntityGraphV1` build (canonical truncation, overflow recorded) |
| 3D | oc03_seeds | seed derivation from verified reports (M4 contributing only; unavailable → marker; per-entity fold) |
| 3E | oc03_ppr | integer fixed-point propagation: recurrence, convergence/cap, quantization, residual accounting |
| 3F | oc03_artifact | `SaliencePriorV1` assembly, `prior_id` placeholder derivation (OC-02 §9.2 clarified precedent), `verify_prior` by recomputation |
| 3G | ADVERSARIAL | tamper matrix, forged report, cross-config, seed-cap overflow, graph-cap overflow |
| 3H | oc03_evidence | evidence document (4-layer), golden fixture + SHA-256, E-record rerun |

## 4. Workspace and dependency gate

No changes to any existing file except additive `pub mod prior;` in
`contextmesh-salience/src/lib.rs`. No new crate dependencies. `contextmesh`
must not depend on `contextmesh-salience` (frozen plan rule). Verification:
`grep -rn "oc03\|SaliencePrior\|prior_id" --include="*.rs" contextmesh-salience/src contextmesh-salience/tests` before 3B returns only the new module and tests; `cargo tree -p contextmesh-salience` unchanged.

## 5. Frozen constants, domains, and encodings

```rust
pub mod versions {
    /// Prior extractor version (frozen; reserved since OC-02 §5).
    pub const PRIOR: &str = "oc-3-prior-v1";
    /// Thorn status marker (frozen: disabled until P4 gate).
    pub const THORN_STATUS: &str = "thorn_disabled";
}
pub mod caps {
    /// Maximum entities in one graph (prereg-graph bound, spec-frozen).
    pub const MAX_ENTITIES: usize = 1024;
    /// Maximum edges kept per entity (canonical-order truncation).
    pub const MAX_EDGES_PER_ENTITY: usize = 32;
    /// Maximum entities extracted per event.
    pub const ENTITIES_PER_EVENT: usize = 8;
    /// Maximum seed entities.
    pub const MAX_SEEDS: usize = 64;
    /// Maximum propagation iterations.
    pub const MAX_ITERATIONS: u32 = 64;
    /// L∞ convergence threshold, ppb.
    pub const EPSILON_PPB: u128 = 1_000_000;
    /// Damping, ppm (0.85).
    pub const DAMPING_PPM: u128 = 850_000;
    /// Prior range upper bound, ppb (prereg `prior_range_ppb[1]`).
    pub const PRIOR_MAX_PPB: u128 = 1_000_000_000;
}
```

Domains (NUL-terminated, BLAKE3, distinct prefixes):
`oc-03-priorcfg1\0` (config hash), `oc-03-prior-v1\0` (prior_id derivation).
Wire prefixes: config `ocpriorcfg1_`, artifact `ocprior1_`.

## 6. Strict JSON and canonical ordering

OC-02 §6 rules apply verbatim: JCS lexicographic member order, no whitespace,
UTF-8, rejects duplicate keys and non-canonical number spellings. All ppb
values serialize as plain integers.

## 7. Frozen JSON value schemas

Exactly these members, these names, this order (lexicographic in bytes).

### 7.1 EntityKeyV1 (wire string)

An entity key is exactly one of, in canonical precedence order:
(a) a canonical ID string recognized by frozen M2 `canonical_id_kind`
(`evt1_…`/`rcpt1_…`/`ocout1_…`, 43 base64url chars), or
(b) a normalized value rendered by frozen M1 `parse_normalized` with a
spelling prefix, exactly one per `NormalizedValue` variant —
`path:<folded-path>`, `pct:<bps>`, `num:<decimal>` — or
(c) an M0 extract token ≤1,024 bytes.
Per event: dedup, sort by byte order, truncate to 8 (drop tail; overflow
counted). No new extractor semantics are introduced.

### 7.2 EntityGraphV1
```json
{"version": 1, "entities": ["…", "…"],
 "edges": [{"a": "…", "b": "…"}],
 "truncated_entities": 0, "truncated_edges": 0,
 "config_hash": "ocpriorcfg1_…"}
```
Undirected edges, `a < b` bytewise, list sorted (`a` then `b`), deduplicated.
Built from co-occurrence: two entity keys appearing in the same ledger session
(one ledger = one session, OC-02 §2 precedent) are adjacent; parent-ledger
sessions contribute the same way (propagation over parent edges). Entity cap
1,024: keep first 1,024 in byte order, count remainder in `truncated_entities`.
Edge cap 32 per entity: keep first 32 per entity in canonical list order, count
remainder in `truncated_edges`. Both counters are recorded data, never errors.

### 7.3 PriorSeedSetV1
```json
{"version": 1, "seeds": [{"entity": "…", "ppb": 0}],
 "source_report_ids": ["ocattr1_…"], "unavailable_reports": 0,
 "config_hash": "ocpriorcfg1_…"}
```
For each verified report whose adapter-tier section status (the
`CausalStatus` wire string of §7.4's serialized section) is `computed` —
which OC-02 defines as both M3 and M4 Complete: every share recorded in
the section's `m4` array contributes
`share_ppm × 1000` ppb to every entity key of the attributed event
(clamped at `PRIOR_MAX_PPB`). Reports whose section status is
`unavailable` or `no_nominations` contribute zero seeds; `unavailable`
ones increment `unavailable_reports` (explicit warning). Shares of 0 ppm
contribute nothing. If the same `report_id` appears more than once in
the input set, its shares are folded exactly once (duplicates ignored,
not double-counted). Seeds folded per entity, sorted by entity byte
order, capped at 64 by descending ppb then entity ascending; remainder
dropped and the drop counted in the artifact's `dropped_seeds` member
(§7.4).

### 7.4 SaliencePriorV1 (envelope)
```json
{"version": 1, "prior_id": "ocprior1_…", "config_hash": "ocpriorcfg1_…",
 "source_report_ids": ["ocattr1_…"], "graph": {EntityGraphV1},
 "seeds": {PriorSeedSetV1}, "vector": [{"entity": "…", "ppb": 0}],
 "iterations": 0, "converged": true, "residual_ppb": 0,
 "dropped_seeds": 0, "thorn_status": "thorn_disabled",
 "terminal_status": "terminal|unterminated"}
```
Exactly these 13 top-level members. `vector` lists only entities with ppb > 0,
sorted by entity byte order. `residual_ppb` is the cumulative floor loss of
the final iteration. `terminal_status` mirrors the source ledgers; mixed
statuses are rejected at assembly (a prior must derive from a uniform set).

### 7.5 PriorConfigV1
```json
{"version": 1, "damping_ppm": 850000, "epsilon_ppb": 1000000,
 "max_iterations": 64, "max_entities": 1024, "max_edges_per_entity": 32,
 "max_seeds": 64, "entities_per_event": 8, "prior_max_ppb": 1000000000,
 "thorn_status": "thorn_disabled", "prereg_reference": "be20d8fc…eae784c9"}
```
JCS-canonical bytes; `validate_frozen` fails closed on any deviation from
§5; BLAKE3(`oc-03-priorcfg1\0` + canonical bytes) → `ocpriorcfg1_` + base64url.

### 7.6 Integer fixed-point recurrence (normative)

Let `s(e)` = seed ppb, `out(e)` = graph degree, `m_t(e)` = mass at step t.
All arithmetic u128 checked; a checked overflow at any point fails the whole
build with `Err` (fail-closed, prereg overflow policy).

```
C             = 1_000_000_000_000 − DAMPING_PPM × 1_000_000   // constant
teleport(e)   = floor(s(e) * DAMPING_PPM / 1_000_000)      // once, constant
m_0(e)        = teleport(e)                                // frozen initial state
prop_t(e)     = Σ_{u ∈ nbr(e)} floor(m_t(u) * C / (1_000_000_000_000 * out(u)))
m_{t+1}(e)    = teleport(e) + prop_t(e)
delta_t       = max_e |m_{t+1}(e) − m_t(e)|                // L∞, u128 saturating abs-diff
stop          : delta_t ≤ EPSILON_PPB (converged = true) or t = MAX_ITERATIONS
                (converged = false, marker recorded, never an error)
residual_ppb  = floor( (Σ_{u : out(u)>0} r_u) / 1_000_000_000_000 ),
                where n_u = m_final(u) × C, d_u = 1e12 × out(u),
                r_u = n_u mod d_u   // final iteration only
```

Every neighbor term is individually floored; the mandated summation order is
the neighbor list in canonical byte order (deterministic). The residual
identity is exact: a source u's total exact outflow is `out(u) · n_u/d_u =
n_u/1e12` (independent of degree), so its flooring loss is precisely
`(n_u mod d_u)/1e12`; summing the integer remainders `r_u` and flooring once
by 1e12 yields the exact total ppb lost to flooring in the final iteration,
computable entirely in u128. Entities with `out=0` distribute nothing: their
mass is exactly `teleport(e)`, fully retained in the vector, and they
contribute zero residual. Final `m` values are already ppb integers; entries
>0 form the vector. Any value >`PRIOR_MAX_PPB` (impossible by construction;
asserted anyway) fails closed.

## 8. Public API semantics

- `derive_entity_keys(event_payload, referenced_events) -> Vec<String>` —
  pure reuse of frozen M0/M1/M2 extractors (§7.1), capped, sorted, deduped.
- `build_entity_graph(ledgers: &[VerifiedLedger], config) -> Result<EntityGraphV1>` —
  bounded, canonical truncation, overflow counters.
- `derive_seeds(reports: &[VerifiedReport], events, config) -> Result<PriorSeedSetV1>` —
  M4-contributing only; unavailable markers counted.
- `compute_prior(ledgers, reports, config) -> Result<SaliencePriorV1>` —
  verifies every report structurally first; then graph → seeds → propagation →
  §7.4 assembly; `prior_id` derived per §9.2.
- `verify_prior(prior_bytes, ledgers, reports, config) -> Result<()>` —
  recomputes from inputs and requires byte-identical canonical bytes and
  equal `prior_id`. No judge, no network, no re-derivation shortcut.
- All §7 types: privately-constructed fields, read-only accessors (OC-02 2F
  precedent); no public field mutation anywhere.

## 9. Determinism and provenance rules

1. Given identical (ledgers, reports, config), `compute_prior` reproduces
   byte-identical `SaliencePriorV1` canonical bytes. No I/O, wall-clock, or
   inference anywhere in this module.
2. `prior_id` = BLAKE3(`oc-03-prior-v1` + NUL + canonical bytes with the
   `prior_id` member set to its fixed derivation placeholder — the literal
   string `"prior_id"`); the derived value is then substituted into exactly
   that position in the sealed bytes, making construction the only writer of
   the ID. Flipping any byte of the sealed bytes invalidates. (Inherits the
   OC-02 §9.2 clarified placeholder precedent, commit `d516c35`.)
3. Every seed entry carries its source `report_id` set; the artifact records
   extractor version `oc-3-prior-v1`, config hash, and `thorn_disabled`.
4. Verification rebuilds graph, seeds, and vector and compares bytes; it
   never trusts recorded intermediates.
5. `share_ppm`→ppb conversion is exactly ×1,000 with u128 checked math.

## 10. Exact implementation file map

| File | Contents |
|---|---|
| `contextmesh-salience/src/prior.rs` | versions, caps, config, entity keys, graph, seeds, propagation, artifact assembly, verify (public API of §8) |
| `contextmesh-salience/tests/oc03_schema.rs` | T01–T08 |
| `contextmesh-salience/tests/oc03_graph_seeds.rs` | G01–G12 |
| `contextmesh-salience/tests/oc03_propagation.rs` | P01–P14 |
| `contextmesh-salience/tests/oc03_artifact.rs` | A01–A10 |
| `contextmesh-salience/tests/oc03_adversarial.rs` | X01–X08 |

## 11. Golden, boundary, and adversarial contract

- Golden fixture `oc03-salience-prior-v1-golden.json` + system-sha256 `.sha256`
  sidecar; suite compares committed bytes; the generator is `#[ignore]` (R07
  pattern). Fixture derives from a synthetic two-report corpus; the evidence
  document states the corpus is SYNTHESIZED (2K precedent).
- Boundary: empty seeds (all reports unavailable → vector empty, warnings
  recorded, artifact still valid — explicit-warning contract); seed cap
  exactly 64; entity cap exactly 1,024; edge cap exactly 32; iteration cap
  exactly 64 with `converged:false`.
- Adversarial (X01–X08): see matrix. Forged report (ocattr1_-shaped but
  failing `verify_report`) → whole build `Err`. Tampered vector byte →
  `verify_prior` failure. Cross-config rebuild → different `prior_id`.
  Truncated graph beyond caps → counters, never errors.

## 12. Documentation and evidence standard

The evidence document (3H) follows the 4-layer standard: Sources (commit
chain + report IDs), Reasoning (alternatives rejected — float propagation,
unbounded graph, signed seeds), Conclusion (gate IDs), Invalidators (what
would make the prior wrong: forged upstream reports, config drift, unfrozen
propagation change).

## 13. Verification commands

```
cd /home/cosmo/contextmesh-rs && source ~/.cargo/env
OC01_INNER_CURRENT_GATE=1 CARGO_NET_OFFLINE=true \
  cargo test -p contextmesh-salience --test oc03_schema --test oc03_graph_seeds \
  --test oc03_propagation --test oc03_artifact --test oc03_adversarial --locked
OC01_INNER_CURRENT_GATE=1 CARGO_NET_OFFLINE=true cargo test --workspace --locked
cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check
```

## 14. Acceptance gates

| Gate | Requirement |
|---|---|
| oc03_workspace | 3A checks green, no dependency change |
| oc03_schema | T01–T08 green |
| oc03_graph | G01–G12 green |
| oc03_seeds | covered by G-rows (same file); unavailable-marker test green |
| oc03_ppr | P01–P14 green |
| oc03_artifact | A01–A10 green incl. golden fixture byte equality |
| ADVERSARIAL | X01–X08 green |
| oc03_evidence | evidence doc dual-reviewed; E-record rerun persisted with SHA-256 |

## 15. Change control

Founder approval required before changing: any §5 constant, the recurrence of
§7.6, the entity-key definition of §7.1, schema member names or
requiredness, the `thorn_disabled` status, or the definitional values of §2.
A fixture change requires a version/change decision and explicit human
review. Clarification path: a wording correction qualifies as a
clarification only if it stands with zero code/test changes
(`git diff --stat` documents only) — the precedent frozen at OC-02 commit
`d516c35`. Discoveries return to specification review; they are never
silently normalized in code.

## 16. Review checklist

- [ ] Prereg values (positive-only channel, 0..=1e9 ppb, thorn disabled)
      consumed verbatim, never redefined.
- [ ] No float anywhere in propagation; u128 checked on every arithmetic step.
- [ ] Graph and seed caps enforced with recorded counters, never silent drops.
- [ ] `prior_id` placeholder derivation matches the OC-02 §9.2 clarified
      precedent exactly.
- [ ] Unavailable/`no_nominations` upstream reports produce explicit markers;
      no silent filtering; no negative or Thorn semantics.
- [ ] Matrix rows map 1:1 to committed tests; row count claims match actual.
- [ ] Verify path recomputes; never trusts recorded intermediates.

## 17. Approval record

- 2026-08-29: draft created after OC-02 termination (commit `d516c35`).
- 2026-08-29: Compliance review **GO** (0 blockers, 8 checks: 52-row count
  programmatic, prereg SHA-256 re-hashed and matching, upstream M4ShareV1
  arithmetic verified against code, version string verbatim, Thorn-absence
  full-text scan, gate-set match, placeholder precedent mirrored, change
  control criterion present; 4 warnings — all fixed: `dropped_seeds` member
  name, Complete-section seed wording, prereg relative path, X07 duplicate
  fold anchor).
- 2026-08-29: Quality review completed by parent after two independent
  subagent attempts died on provider API timeout (Stage 2G precedent): 1
  blocker — residual_ppb carried two conflated definitions — fixed by
  freezing the single exact identity (verified independently by
  brute-force over 200,000 random cases: 0 mismatches; and a 300-trial
  independent Python simulation of §7.6: int-vs-float agreement, exact
  residual equality via Fraction, determinism, range invariants all pass);
  2 warnings fixed (P07 split into distinct assertion, P09 reworded as
  implementable include_str! source check).
- 2026-08-29: preflight (founder-requested multi-angle consistency check):
  code-access audit (all reused extractors/verifiers public and present),
  wire audit (found and fixed §7.3 status-access wording to the actual
  section-level `computed` gate at attribution.rs:569), m_0 initial state
  frozen to teleport(e), baseline HEAD `d516c35` clean.
- 2026-08-29: **Lunarpulse approved the spec and matrix for freezing**
  (Discord message `1543029704192434197`). Status is now
  `approved-for-implementation`. Implementation proceeds in §3
  dependency order.
- 2026-08-29: **Lunarpulse approved the Plan A minimal clarification of
  §7.1 entity-key M1 spellings** (Discord message `1543059606077575190`):
  the draft listed four spellings (`path:`/`pct:`/`count:`/`amt:`) but the
  frozen M1 `NormalizedValue` has exactly three variants — the section is
  corrected to one spelling per variant: `path:<folded-path>`,
  `pct:<bps>`, `num:<decimal>`; matrix row OC03-G02's evidence cell is
  corrected to match. Cause: specification gap (draft written without
  measuring the M1 renderer). Eligibility: stands with zero code/test
  changes — no OC-03 entity-key code existed at correction time (Stage 3C
  had not started; working tree clean at `43c8813`).
