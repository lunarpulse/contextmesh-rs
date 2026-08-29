//! OC-03 Stage 3B: the salience-prior schema layer — frozen version
//! strings, caps, domain-separation constants, the canonical
//! [`PriorConfigV1`] with its domain-separated config hash, and the §7
//! wire value types with manual JCS rendering.
//!
//! Frozen by `spec-oc-03-prior.md` (§5–§7). Version strings, caps, and the
//! prereg seal are consumed verbatim from the P1 preregistration
//! (`p1-prereg-config.json`, SHA-256 `be20d8fc…`, OC-02 precedent).
//! Graph construction, seed derivation, propagation, and verification are
//! later stages (3C–3F); this stage freezes only the schema surface.
//! Change control: spec §15 — founder approval required for any change.
//!
//! No floats exist anywhere in this module, and the only arithmetic is
//! integer comparison (later stages widen all propagation arithmetic to
//! checked `u128`).

use base64::Engine as _;
use blake3::Hasher;

use std::collections::{BTreeMap, BTreeSet};

use crate::error::OutcomeError;

/// Frozen prior extractor version strings (spec §5).
pub mod versions {
    /// Prior extractor version (frozen; reserved since OC-02 §5).
    pub const PRIOR: &str = "oc-3-prior-v1";
    /// Thorn status marker (frozen: disabled until the P4 gate).
    pub const THORN_STATUS: &str = "thorn_disabled";
}

/// Frozen caps and propagation constants (spec §5).
pub mod caps {
    /// Maximum entities in one graph (prereg graph bound, spec-frozen).
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

/// Domain-separation constant for prior config hashes (spec §5; literal
/// domain bytes including the NUL terminator, OC-01 pattern).
pub const PRIOR_CONFIG_HASH_DOMAIN: &[u8] = b"oc-03-priorcfg1\0";

/// Domain-separation constant for `prior_id` derivation (spec §5, §9.2).
pub const PRIOR_ID_DOMAIN: &[u8] = b"oc-03-prior-v1\0";

/// Typed prefix for prior config hashes (spec §5).
pub const CONFIG_HASH_PREFIX: &str = "ocpriorcfg1_";

/// Typed prefix for prior artifact IDs (spec §5).
pub const PRIOR_ID_PREFIX: &str = "ocprior1_";

/// The frozen P1 preregistration SHA-256 (spec §5; identical seal to
/// OC-02 — consumed verbatim, never redefined).
pub const PREREG_SHA256: &str = "be20d8fc48771098e745038b906dd13456ffcebdeb424cee25e91d52eae784c9";

/// Terminal-status wire value for a prior derived from terminal ledgers.
pub const TERMINAL_STATUS: &str = "terminal";
/// Terminal-status wire value for a prior from unterminated ledgers.
pub const UNTERMINATED_STATUS: &str = "unterminated";

/// Canonical prior configuration (spec §7.5). Every field is a frozen
/// policy value; the struct exists so the whole prior pipeline can carry
/// and re-verify one config hash (fail-closed on any deviation, §15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriorConfigV1 {
    /// Schema version (frozen: 1).
    pub version: u8,
    /// Damping, ppm (frozen: 850,000).
    pub damping_ppm: u128,
    /// L∞ convergence threshold, ppb (frozen: 1,000,000).
    pub epsilon_ppb: u128,
    /// Maximum propagation iterations (frozen: 64).
    pub max_iterations: u32,
    /// Maximum entities in one graph (frozen: 1,024).
    pub max_entities: usize,
    /// Maximum edges kept per entity (frozen: 32).
    pub max_edges_per_entity: usize,
    /// Maximum seed entities (frozen: 64).
    pub max_seeds: usize,
    /// Maximum entities extracted per event (frozen: 8).
    pub entities_per_event: usize,
    /// Prior range upper bound, ppb (frozen: 1,000,000,000).
    pub prior_max_ppb: u128,
    /// Thorn status marker (frozen: `thorn_disabled`).
    pub thorn_status: &'static str,
    /// Frozen P1 preregistration SHA-256 seal.
    pub prereg_reference: &'static str,
}

impl Default for PriorConfigV1 {
    fn default() -> Self {
        Self {
            version: 1,
            damping_ppm: caps::DAMPING_PPM,
            epsilon_ppb: caps::EPSILON_PPB,
            max_iterations: caps::MAX_ITERATIONS,
            max_entities: caps::MAX_ENTITIES,
            max_edges_per_entity: caps::MAX_EDGES_PER_ENTITY,
            max_seeds: caps::MAX_SEEDS,
            entities_per_event: caps::ENTITIES_PER_EVENT,
            prior_max_ppb: caps::PRIOR_MAX_PPB,
            thorn_status: versions::THORN_STATUS,
            prereg_reference: PREREG_SHA256,
        }
    }
}

impl PriorConfigV1 {
    /// Fail if any member deviates from the frozen §5 values (spec §15 —
    /// the configuration is not tunable; a deviation is `Malformed`).
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] on any deviation.
    pub fn validate_frozen(&self) -> Result<(), OutcomeError> {
        if *self != Self::default() {
            return Err(OutcomeError::Malformed);
        }
        Ok(())
    }

    /// Deterministic canonical bytes (JCS single-line JSON, lexicographic
    /// member order: damping_ppm, entities_per_event, epsilon_ppb,
    /// max_edges_per_entity, max_entities, max_iterations, max_seeds,
    /// prereg_reference, prior_max_ppb, thorn_status, version; spec §6–§7.5).
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] when the config is not frozen.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, OutcomeError> {
        self.validate_frozen()?;
        let s = format!(
            concat!(
                "{{\"damping_ppm\":{},\"entities_per_event\":{},",
                "\"epsilon_ppb\":{},\"max_edges_per_entity\":{},",
                "\"max_entities\":{},\"max_iterations\":{},\"max_seeds\":{},",
                "\"prereg_reference\":",
            ),
            self.damping_ppm,
            self.entities_per_event,
            self.epsilon_ppb,
            self.max_edges_per_entity,
            self.max_entities,
            self.max_iterations,
            self.max_seeds,
        );
        let mut s = s;
        push_json_string(&mut s, self.prereg_reference);
        s.push_str(&format!(
            ",\"prior_max_ppb\":{},\"thorn_status\":",
            self.prior_max_ppb
        ));
        push_json_string(&mut s, self.thorn_status);
        s.push_str(&format!(",\"version\":{}}}", self.version));
        Ok(s.into_bytes())
    }

    /// Domain-separated BLAKE3 config hash, typed `ocpriorcfg1_…` (spec
    /// §7.5: BLAKE3(`oc-03-priorcfg1\0` + canonical bytes) → base64url).
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] when the config is not frozen.
    pub fn config_hash(&self) -> Result<String, OutcomeError> {
        let bytes = self.canonical_bytes()?;
        let mut hasher = Hasher::new();
        hasher.update(PRIOR_CONFIG_HASH_DOMAIN);
        hasher.update(bytes.as_slice());
        let b64 =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize().as_bytes());
        Ok(format!("{CONFIG_HASH_PREFIX}{b64}"))
    }
}

/// One undirected graph edge between two entity keys (spec §7.2). Fields
/// are privately constructed; `a < b` bytewise in every canonical render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityEdgeV1 {
    /// The bytewise-smaller endpoint.
    a: String,
    /// The bytewise-larger endpoint.
    b: String,
}

impl EntityEdgeV1 {
    /// Test-visible minimal constructor (Stage 3B; the canonical builder
    /// arrives with graph construction in Stage 3C).
    #[doc(hidden)]
    pub fn new_for_test(a: &str, b: &str) -> Self {
        Self {
            a: a.to_owned(),
            b: b.to_owned(),
        }
    }

    /// Read-only accessor for the smaller endpoint.
    #[must_use]
    pub fn a(&self) -> &str {
        &self.a
    }

    /// Read-only accessor for the larger endpoint.
    #[must_use]
    pub fn b(&self) -> &str {
        &self.b
    }

    /// Canonical JCS bytes: `{"a":"…","b":"…"}` (lexicographic members).
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] when `a >= b` bytewise.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, OutcomeError> {
        if self.a.is_empty() || self.b.is_empty() || self.a >= self.b {
            return Err(OutcomeError::Malformed);
        }
        let mut json = String::new();
        json.push_str("{\"a\":");
        push_json_string(&mut json, &self.a);
        json.push_str(",\"b\":");
        push_json_string(&mut json, &self.b);
        json.push('}');
        Ok(json.into_bytes())
    }
}

/// One positive seed mass: an entity key with a ppb value in
/// `0..=PRIOR_MAX_PPB` (spec §7.3). Also the vector-entry shape (§7.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriorSeedV1 {
    entity: String,
    ppb: u128,
}

impl PriorSeedV1 {
    /// Test-visible minimal constructor (Stage 3B; canonical seed
    /// derivation from verified reports arrives in Stage 3D).
    #[doc(hidden)]
    pub fn new_for_test(entity: &str, ppb: u128) -> Self {
        Self {
            entity: entity.to_owned(),
            ppb,
        }
    }

    /// Read-only accessor for the entity key.
    #[must_use]
    pub fn entity(&self) -> &str {
        &self.entity
    }

    /// Read-only accessor for the ppb mass.
    #[must_use]
    pub const fn ppb(&self) -> u128 {
        self.ppb
    }

    /// Canonical JCS bytes: `{"entity":"…","ppb":N}` (lexicographic).
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] for an empty entity or a ppb
    /// value outside `0..=PRIOR_MAX_PPB`.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, OutcomeError> {
        if self.entity.is_empty() || self.ppb > caps::PRIOR_MAX_PPB {
            return Err(OutcomeError::Malformed);
        }
        let mut json = String::new();
        json.push_str("{\"entity\":");
        push_json_string(&mut json, &self.entity);
        json.push_str(&format!(",\"ppb\":{}}}", self.ppb));
        Ok(json.into_bytes())
    }
}

/// The bounded co-occurrence entity graph (spec §7.2). Fields are
/// privately constructed; overflow counters are recorded data, never
/// errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityGraphV1 {
    version: u8,
    entities: Vec<String>,
    edges: Vec<EntityEdgeV1>,
    truncated_entities: u128,
    truncated_edges: u128,
    config_hash: String,
}

impl EntityGraphV1 {
    /// Test-visible minimal constructor (Stage 3B; the bounded canonical
    /// builder arrives in Stage 3C).
    #[doc(hidden)]
    pub fn new_for_test(
        entities: Vec<String>,
        edges: Vec<EntityEdgeV1>,
        truncated_entities: u128,
        truncated_edges: u128,
        config_hash: String,
    ) -> Self {
        Self {
            version: 1,
            entities,
            edges,
            truncated_entities,
            truncated_edges,
            config_hash,
        }
    }

    /// Read-only accessor for the entity list.
    #[must_use]
    pub fn entities(&self) -> &[String] {
        &self.entities
    }

    /// Read-only accessor for the edge list.
    #[must_use]
    pub fn edges(&self) -> &[EntityEdgeV1] {
        &self.edges
    }

    /// Read-only accessor for the dropped-entity counter.
    #[must_use]
    pub const fn truncated_entities(&self) -> u128 {
        self.truncated_entities
    }

    /// Read-only accessor for the dropped-edge counter.
    #[must_use]
    pub const fn truncated_edges(&self) -> u128 {
        self.truncated_edges
    }

    /// Privately-validated graph assembly (spec §8 construction discipline).
    /// Canonicalizes: entities byte-sorted (capped by the caller), edges
    /// `a < b` sorted deduplicated, and stamps the config hash.
    pub(crate) fn assemble(
        entities: Vec<String>,
        edges: Vec<EntityEdgeV1>,
        truncated_entities: u128,
        truncated_edges: u128,
        config: &PriorConfigV1,
    ) -> Result<Self, OutcomeError> {
        let mut sorted_entities = entities;
        sorted_entities.sort();
        sorted_entities.dedup();
        let mut sorted_edges = edges;
        sorted_edges
            .sort_by(|x, y| (x.a.as_str(), x.b.as_str()).cmp(&(y.a.as_str(), y.b.as_str())));
        sorted_edges.dedup_by(|x, y| x.a == y.a && x.b == y.b);
        if sorted_entities.len() > caps::MAX_ENTITIES {
            return Err(OutcomeError::Malformed);
        }
        for edge in &sorted_edges {
            if edge.a >= edge.b {
                return Err(OutcomeError::Malformed);
            }
            if !sorted_entities.contains(&edge.a) || !sorted_entities.contains(&edge.b) {
                return Err(OutcomeError::Malformed);
            }
        }
        Ok(Self {
            version: 1,
            entities: sorted_entities,
            edges: sorted_edges,
            truncated_entities,
            truncated_edges,
            config_hash: config.config_hash()?,
        })
    }

    /// Read-only accessor for the bound config hash.
    #[must_use]
    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }

    /// Canonical JCS bytes with the exact §7.2 members in lexicographic
    /// order: config_hash, edges, entities, truncated_edges,
    /// truncated_entities, version.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] for a wrong version or any
    /// member that fails its own canonical render.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, OutcomeError> {
        if self.version != 1 {
            return Err(OutcomeError::Malformed);
        }
        let mut json = String::new();
        json.push_str("{\"config_hash\":");
        push_json_string(&mut json, &self.config_hash);
        json.push_str(",\"edges\":[");
        for (index, edge) in self.edges.iter().enumerate() {
            if index != 0 {
                json.push(',');
            }
            json.push_str(
                std::str::from_utf8(&edge.canonical_bytes()?)
                    .map_err(|_| OutcomeError::Malformed)?,
            );
        }
        json.push_str("],\"entities\":[");
        for (index, entity) in self.entities.iter().enumerate() {
            if index != 0 {
                json.push(',');
            }
            push_json_string(&mut json, entity);
        }
        json.push_str(&format!(
            "],\"truncated_edges\":{},\"truncated_entities\":{},\"version\":{}}}",
            self.truncated_edges, self.truncated_entities, self.version
        ));
        Ok(json.into_bytes())
    }
}

/// The folded positive seed set derived from verified reports (spec §7.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriorSeedSetV1 {
    version: u8,
    seeds: Vec<PriorSeedV1>,
    source_report_ids: Vec<String>,
    unavailable_reports: u128,
    config_hash: String,
}

impl PriorSeedSetV1 {
    /// Test-visible minimal constructor (Stage 3B; canonical derivation
    /// arrives in Stage 3D).
    #[doc(hidden)]
    pub fn new_for_test(
        seeds: Vec<PriorSeedV1>,
        source_report_ids: Vec<String>,
        unavailable_reports: u128,
        config_hash: String,
    ) -> Self {
        Self {
            version: 1,
            seeds,
            source_report_ids,
            unavailable_reports,
            config_hash,
        }
    }

    /// Read-only accessor for the seed list.
    #[must_use]
    pub fn seeds(&self) -> &[PriorSeedV1] {
        &self.seeds
    }

    /// Read-only accessor for the source report-ID list.
    #[must_use]
    pub fn source_report_ids(&self) -> &[String] {
        &self.source_report_ids
    }

    /// Read-only accessor for the unavailable-report counter.
    #[must_use]
    pub const fn unavailable_reports(&self) -> u128 {
        self.unavailable_reports
    }

    /// Canonical JCS bytes with the exact §7.3 members in lexicographic
    /// order: config_hash, seeds, source_report_ids, unavailable_reports,
    /// version.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] for a wrong version or any
    /// member that fails its own canonical render.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, OutcomeError> {
        if self.version != 1 {
            return Err(OutcomeError::Malformed);
        }
        let mut json = String::new();
        json.push_str("{\"config_hash\":");
        push_json_string(&mut json, &self.config_hash);
        json.push_str(",\"seeds\":[");
        for (index, seed) in self.seeds.iter().enumerate() {
            if index != 0 {
                json.push(',');
            }
            json.push_str(
                std::str::from_utf8(&seed.canonical_bytes()?)
                    .map_err(|_| OutcomeError::Malformed)?,
            );
        }
        json.push_str("],\"source_report_ids\":[");
        for (index, id) in self.source_report_ids.iter().enumerate() {
            if index != 0 {
                json.push(',');
            }
            push_json_string(&mut json, id);
        }
        json.push_str(&format!(
            "],\"unavailable_reports\":{},\"version\":{}}}",
            self.unavailable_reports, self.version
        ));
        Ok(json.into_bytes())
    }
}

/// The sealed §7.4 prior envelope: exactly 13 top-level members, all
/// nonnegative integers/strings/bools, no floats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaliencePriorV1 {
    version: u8,
    prior_id: String,
    config_hash: String,
    source_report_ids: Vec<String>,
    graph: EntityGraphV1,
    seeds: PriorSeedSetV1,
    vector: Vec<PriorSeedV1>,
    iterations: u32,
    converged: bool,
    residual_ppb: u128,
    dropped_seeds: u128,
    thorn_status: &'static str,
    terminal_status: &'static str,
}

impl SaliencePriorV1 {
    /// Test-visible minimal constructor (Stage 3B; canonical assembly and
    /// `prior_id` derivation arrive in Stage 3F).
    #[allow(clippy::too_many_arguments)]
    #[doc(hidden)]
    pub fn new_for_test(
        prior_id: String,
        config_hash: String,
        source_report_ids: Vec<String>,
        graph: EntityGraphV1,
        seeds: PriorSeedSetV1,
        vector: Vec<PriorSeedV1>,
        iterations: u32,
        converged: bool,
        residual_ppb: u128,
        dropped_seeds: u128,
        terminal_status: &'static str,
    ) -> Self {
        Self {
            version: 1,
            prior_id,
            config_hash,
            source_report_ids,
            graph,
            seeds,
            vector,
            iterations,
            converged,
            residual_ppb,
            dropped_seeds,
            thorn_status: versions::THORN_STATUS,
            terminal_status,
        }
    }

    /// Read-only accessor for the artifact ID.
    #[must_use]
    pub fn prior_id(&self) -> &str {
        &self.prior_id
    }

    /// Read-only accessor for the bound config hash.
    #[must_use]
    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }

    /// Read-only: the source report ids (folded once each, §7.3).
    #[must_use]
    pub fn source_report_ids(&self) -> &[String] {
        &self.source_report_ids
    }

    /// Read-only: the bounded entity graph.
    #[must_use]
    pub const fn graph(&self) -> &EntityGraphV1 {
        &self.graph
    }

    /// Read-only: the folded seed set.
    #[must_use]
    pub const fn seeds(&self) -> &PriorSeedSetV1 {
        &self.seeds
    }

    /// Read-only: the positive-mass propagation vector (byte order).
    #[must_use]
    pub fn vector(&self) -> &[PriorSeedV1] {
        &self.vector
    }

    /// Read-only: iterations executed.
    #[must_use]
    pub const fn iterations(&self) -> u32 {
        self.iterations
    }

    /// Read-only: whether the L∞ threshold was reached.
    #[must_use]
    pub const fn converged(&self) -> bool {
        self.converged
    }

    /// Read-only: final-iteration flooring loss.
    #[must_use]
    pub const fn residual_ppb(&self) -> u128 {
        self.residual_ppb
    }

    /// Read-only: seed-cap drop count (§7.3).
    #[must_use]
    pub const fn dropped_seeds(&self) -> u128 {
        self.dropped_seeds
    }

    /// Read-only: `terminal` or `unterminated`.
    #[must_use]
    pub const fn terminal_status(&self) -> &'static str {
        self.terminal_status
    }

    /// Canonical JCS bytes with the exact 13 §7.4 members in lexicographic
    /// order: config_hash, converged, dropped_seeds, graph, iterations,
    /// prior_id, residual_ppb, seeds, source_report_ids, terminal_status,
    /// thorn_status, vector, version.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] for a wrong version, a
    /// non-frozen thorn status, an unknown terminal status, or any member
    /// that fails its own canonical render.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, OutcomeError> {
        if self.version != 1
            || self.thorn_status != versions::THORN_STATUS
            || (self.terminal_status != TERMINAL_STATUS
                && self.terminal_status != UNTERMINATED_STATUS)
            || self.iterations > caps::MAX_ITERATIONS
            || self.residual_ppb > caps::PRIOR_MAX_PPB
        {
            return Err(OutcomeError::Malformed);
        }
        let mut json = String::new();
        json.push_str("{\"config_hash\":");
        push_json_string(&mut json, &self.config_hash);
        json.push_str(&format!(
            ",\"converged\":{},\"dropped_seeds\":{},\"graph\":",
            self.converged, self.dropped_seeds
        ));
        json.push_str(
            std::str::from_utf8(&self.graph.canonical_bytes()?)
                .map_err(|_| OutcomeError::Malformed)?,
        );
        json.push_str(&format!(
            ",\"iterations\":{},\"prior_id\":",
            self.iterations
        ));
        push_json_string(&mut json, &self.prior_id);
        json.push_str(&format!(
            ",\"residual_ppb\":{},\"seeds\":",
            self.residual_ppb
        ));
        json.push_str(
            std::str::from_utf8(&self.seeds.canonical_bytes()?)
                .map_err(|_| OutcomeError::Malformed)?,
        );
        json.push_str(",\"source_report_ids\":[");
        for (index, id) in self.source_report_ids.iter().enumerate() {
            if index != 0 {
                json.push(',');
            }
            push_json_string(&mut json, id);
        }
        json.push_str("],\"terminal_status\":");
        push_json_string(&mut json, self.terminal_status);
        json.push_str(",\"thorn_status\":");
        push_json_string(&mut json, self.thorn_status);
        json.push_str(",\"vector\":[");
        for (index, entry) in self.vector.iter().enumerate() {
            if index != 0 {
                json.push(',');
            }
            json.push_str(
                std::str::from_utf8(&entry.canonical_bytes()?)
                    .map_err(|_| OutcomeError::Malformed)?,
            );
        }
        json.push_str(&format!("],\"version\":{}}}", self.version));
        Ok(json.into_bytes())
    }
}

/// Append one JSON string with RFC 8259 escaping and no whitespace
/// (the OC-02 `attribution.rs` renderer, mirrored).
fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            control if control <= '\u{1f}' => {
                output.push_str("\\u00");
                let byte = control as u8;
                const HEX: &[u8; 16] = b"0123456789abcdef";
                output.push(char::from(HEX[usize::from(byte >> 4)]));
                output.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
            other => output.push(other),
        }
    }
    output.push('"');
}

// ── Stage 3C: entity keys and bounded entity graph (spec §7.1–§7.2) ──────

/// Derive bounded entity keys from one event payload (spec §7.1).
///
/// Precedence per token (frozen extractors reused verbatim):
/// (a) M2 canonical ID (`canonical_id_kind`) → token as-is;
/// (b) M1 normalized value → `"path:"`/`"pct:"`/`"num:"` + `canonical()`,
///     exactly one spelling per `NormalizedValue` variant (clarified
///     2026-08-29, commit `83cd7af`);
/// (c) raw token if ≤ 1,024 bytes (already enforced by `extract_tokens`).
/// Keys are deduplicated, byte-sorted, then truncated to the 8
/// byte-smallest (`caps::ENTITIES_PER_EVENT`; the sorted tail is dropped).
#[must_use]
pub fn derive_entity_keys(payload: &str) -> Vec<String> {
    use crate::attribution::{
        NormalizedValue, canonical_id_kind, extract_tokens, parse_normalized,
    };
    let (tokens, _skipped) = extract_tokens(payload);
    let mut keys: Vec<String> = Vec::new();
    for token in tokens {
        let key = if canonical_id_kind(token).is_some() {
            (*token).to_owned()
        } else if let Some(value) = parse_normalized(token) {
            match value {
                NormalizedValue::Path(_) => format!("path:{}", value.canonical()),
                NormalizedValue::Percent(_) => format!("pct:{}", value.canonical()),
                NormalizedValue::Number(_) => format!("num:{}", value.canonical()),
            }
        } else {
            (*token).to_owned()
        };
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys.sort();
    keys.truncate(caps::ENTITIES_PER_EVENT);
    keys
}

/// One session's payload bundle: the event payloads referenced by one
/// verified ledger (one ledger = one session, OC-02 §2 precedent).
#[derive(Debug, Clone)]
pub struct SessionPayloads<'a> {
    payloads: Vec<&'a str>,
}

impl<'a> SessionPayloads<'a> {
    /// Wrap caller-owned payload slices (order irrelevant; keys are set-folded).
    #[must_use]
    pub fn from_payloads(payloads: Vec<&'a str>) -> Self {
        Self { payloads }
    }

    /// Read-only view.
    #[must_use]
    pub fn payloads(&self) -> &[&'a str] {
        &self.payloads
    }

    fn entity_set(&self) -> BTreeSet<String> {
        let mut set = BTreeSet::new();
        for payload in &self.payloads {
            for key in derive_entity_keys(payload) {
                set.insert(key);
            }
        }
        set
    }
}

/// Build the bounded entity graph from session payload bundles (spec §7.2).
///
/// Co-occurrence: two entity keys appearing in the same session are
/// adjacent. Parent-ledger sessions contribute adjacency the same way —
/// callers pass each parent bundle as another element of `sessions`
/// (propagation over parent edges). Canonicalization: entities byte-sorted
/// capped at `MAX_ENTITIES`; edges `a < b` bytewise, sorted, deduplicated,
/// per-entity capped at `MAX_EDGES_PER_ENTITY` keeping the first edges in
/// canonical list order. Both truncations are recorded counters, never
/// errors.
///
/// Ordering note (§7.2): the entity cap is applied first; edges whose
/// endpoints fall outside the capped entity set are excluded before the
/// per-entity edge cap runs and are not counted in `truncated_edges`
/// (that counter is defined by §7.2 as the remainder of the 32-per-entity
/// cap over surviving endpoints).
pub fn build_entity_graph(
    sessions: &[SessionPayloads<'_>],
    config: &PriorConfigV1,
) -> Result<EntityGraphV1, OutcomeError> {
    config.validate_frozen()?;

    // Union of per-session entity sets → candidate entity universe.
    let mut universe: BTreeSet<String> = BTreeSet::new();
    let session_sets: Vec<BTreeSet<String>> =
        sessions.iter().map(SessionPayloads::entity_set).collect();
    for set in &session_sets {
        universe.extend(set.iter().cloned());
    }

    // Entity cap: keep first MAX_ENTITIES in byte order, count the rest.
    let all_entities: Vec<String> = universe.into_iter().collect();
    let truncated_entities =
        u128::try_from(all_entities.len().saturating_sub(caps::MAX_ENTITIES)).unwrap_or(u128::MAX);
    let entities: Vec<String> = all_entities
        .into_iter()
        .take(caps::MAX_ENTITIES)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    // Co-occurrence edges over the capped entity set only.
    let mut pair_set: BTreeSet<(String, String)> = BTreeSet::new();
    for set in &session_sets {
        let members: Vec<&String> = set.iter().filter(|e| entities.contains(e)).collect();
        for (i, a) in members.iter().enumerate() {
            for b in &members[i + 1..] {
                let (lo, hi) = if a.as_str() < b.as_str() {
                    ((*a).clone(), (*b).clone())
                } else {
                    ((*b).clone(), (*a).clone())
                };
                pair_set.insert((lo, hi));
            }
        }
    }

    // Per-entity edge cap: keep the first MAX_EDGES_PER_ENTITY edges in
    // canonical list order for each entity; count every dropped edge.
    let mut edges: Vec<EntityEdgeV1> = Vec::new();
    let mut degree: BTreeMap<String, usize> = BTreeMap::new();
    let mut truncated_edges: u128 = 0;
    for (a, b) in pair_set {
        let da = degree.get(&a).copied().unwrap_or(0);
        let db = degree.get(&b).copied().unwrap_or(0);
        if da >= caps::MAX_EDGES_PER_ENTITY || db >= caps::MAX_EDGES_PER_ENTITY {
            truncated_edges += 1;
            continue;
        }
        degree.insert(a.clone(), da + 1);
        degree.insert(b.clone(), db + 1);
        edges.push(EntityEdgeV1::new_for_test(&a, &b));
    }

    EntityGraphV1::assemble(entities, edges, truncated_entities, truncated_edges, config)
}

// ── Stage 3D: seed derivation from verified reports (spec §7.3) ───────────

/// One verified report contribution: the parsed adapter tier of a single
/// `AttributionReportV1` whose structural verification has already passed
/// (callers run `verify_report` first; §1 input contract).
#[derive(Debug, Clone)]
pub struct ReportContribution {
    report_id: String,
    ledger_id: String,
    terminal_status: String,
    section_status: String,
    /// (event text, share_ppm) pairs from the section's `m4` array.
    m4_shares: Vec<(String, u128)>,
}

impl ReportContribution {
    /// Parse one report envelope's wire bytes into a contribution.
    ///
    /// Extracts `report_id`, `ledger_id`, `terminal_status`, the adapter
    /// tier's section status, and the `m4` share array. Structural
    /// verification of the report against its ledger is the caller's
    /// obligation (`verify_report`, §1); this parser only reads.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] when the bytes are not a JSON
    /// object carrying the expected members. This parser is deliberately
    /// lenient (extra members, key order, and envelope version are not
    /// checked): structural and canonical verification of the report is
    /// the caller's obligation (`verify_report`, §1).
    pub fn from_report_bytes(bytes: &[u8]) -> Result<Self, OutcomeError> {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| OutcomeError::Malformed)?;
        let object = value.as_object().ok_or(OutcomeError::Malformed)?;
        let report_id = object
            .get("report_id")
            .and_then(serde_json::Value::as_str)
            .ok_or(OutcomeError::Malformed)?
            .to_owned();
        let ledger_id = object
            .get("ledger_id")
            .and_then(serde_json::Value::as_str)
            .ok_or(OutcomeError::Malformed)?
            .to_owned();
        let terminal_status = object
            .get("terminal_status")
            .and_then(serde_json::Value::as_str)
            .ok_or(OutcomeError::Malformed)?
            .to_owned();
        if terminal_status != "terminal" && terminal_status != "unterminated" {
            return Err(OutcomeError::Malformed);
        }
        let tier_text = object
            .get("adapter_tier")
            .and_then(serde_json::Value::as_str)
            .ok_or(OutcomeError::Malformed)?;
        let tier: serde_json::Value =
            serde_json::from_str(tier_text).map_err(|_| OutcomeError::Malformed)?;
        let tier_object = tier.as_object().ok_or(OutcomeError::Malformed)?;
        let section_status = tier_object
            .get("status")
            .and_then(serde_json::Value::as_str)
            .ok_or(OutcomeError::Malformed)?
            .to_owned();
        let mut m4_shares = Vec::new();
        if let Some(records) = tier_object.get("m4").and_then(serde_json::Value::as_array) {
            for record in records {
                let record_object = record.as_object().ok_or(OutcomeError::Malformed)?;
                let event = record_object
                    .get("event")
                    .and_then(serde_json::Value::as_str)
                    .ok_or(OutcomeError::Malformed)?
                    .to_owned();
                let share_ppm = record_object
                    .get("share_ppm")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or(OutcomeError::Malformed)?;
                m4_shares.push((event, u128::from(share_ppm)));
            }
        }
        Ok(Self {
            report_id,
            ledger_id,
            terminal_status,
            section_status,
            m4_shares,
        })
    }

    /// Read-only: the source report id.
    #[must_use]
    pub fn report_id(&self) -> &str {
        &self.report_id
    }

    /// Read-only: the source ledger id.
    #[must_use]
    pub fn ledger_id(&self) -> &str {
        &self.ledger_id
    }

    /// Read-only: `terminal` or `unterminated`.
    #[must_use]
    pub fn terminal_status(&self) -> &str {
        &self.terminal_status
    }

    /// Read-only: the adapter-tier section status wire string.
    #[must_use]
    pub fn section_status(&self) -> &str {
        &self.section_status
    }

    /// Read-only: the section's `(event, share_ppm)` pairs.
    #[must_use]
    pub fn m4_shares(&self) -> &[(String, u128)] {
        &self.m4_shares
    }
}

/// Derive the folded positive seed set from verified report contributions
/// and the payload of each attributed event (spec §7.3).
///
/// Reports whose adapter-tier section status is `computed` contribute
/// `share_ppm × 1,000` ppb to every entity key of the attributed event
/// (clamped at `PRIOR_MAX_PPB`); `unavailable`/`no_nominations` sections
/// contribute zero seeds, with `unavailable` incrementing
/// `unavailable_reports` (explicit warning). Zero-ppm shares contribute
/// nothing. Duplicate `report_id`s fold exactly once. Seeds fold per
/// entity, byte-sort, and cap at `MAX_SEEDS` by descending ppb then entity
/// ascending; the drop count is returned alongside (it lands in the
/// envelope's `dropped_seeds` member, §7.4).
///
/// # Errors
/// Returns [`OutcomeError::Malformed`] on config drift or mixed terminal
/// statuses (a prior derives from a uniform set, §7.4).
pub fn derive_seeds(
    contributions: &[ReportContribution],
    event_payloads: &[(&str, &str)], // (event text, payload), reused for key derivation
    config: &PriorConfigV1,
) -> Result<(PriorSeedSetV1, u128), OutcomeError> {
    config.validate_frozen()?;

    // Uniform terminal-status requirement (§7.4 assembly gate).
    let mut terminal_status: Option<&str> = None;
    for contribution in contributions {
        match terminal_status {
            None => terminal_status = Some(contribution.terminal_status()),
            Some(existing) if existing == contribution.terminal_status() => {}
            Some(_) => return Err(OutcomeError::Malformed),
        }
    }
    let terminal_status = terminal_status.unwrap_or("terminal").to_owned();
    // Empty contribution set defaults to "terminal" (a choice; §7.4's
    // mirror-the-ledgers rule says nothing about the empty set).

    // Fold each report exactly once by report_id.
    let mut seen_report_ids: Vec<&str> = Vec::new();
    let mut source_report_ids: Vec<String> = Vec::new();
    let mut unavailable_reports: u128 = 0;

    // Per-entity seed mass accumulation (checked u128, clamped at 1e9).
    let mut mass: BTreeMap<String, u128> = BTreeMap::new();
    for contribution in contributions {
        let report_id = contribution.report_id();
        if seen_report_ids.contains(&report_id) {
            continue; // duplicate report_id folds exactly once (§7.3)
        }
        seen_report_ids.push(report_id);
        source_report_ids.push(report_id.to_owned());
        match contribution.section_status() {
            "computed" => {
                for (event, share_ppm) in contribution.m4_shares() {
                    if *share_ppm == 0 {
                        continue; // zero-ppm shares contribute nothing
                    }
                    let added = share_ppm
                        .checked_mul(1_000)
                        .ok_or(OutcomeError::Malformed)?;
                    // Entity keys of the attributed event.
                    let payload = event_payloads
                        .iter()
                        .find(|(event_text, _)| event_text == event)
                        .map(|(_, payload)| *payload)
                        .ok_or(OutcomeError::Malformed)?;
                    for key in derive_entity_keys(payload) {
                        let entry = mass.entry(key).or_insert(0);
                        *entry = (*entry)
                            .checked_add(added)
                            .ok_or(OutcomeError::Malformed)?
                            .min(caps::PRIOR_MAX_PPB);
                    }
                }
            }
            "unavailable" => {
                unavailable_reports += 1;
            }
            "no_nominations" => {}
            _ => return Err(OutcomeError::Malformed),
        }
    }

    // Cap at MAX_SEEDS: descending ppb, then entity ascending.
    let mut ranked: Vec<(String, u128)> = mass.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let dropped_seeds =
        u128::try_from(ranked.len().saturating_sub(caps::MAX_SEEDS)).unwrap_or(u128::MAX);
    ranked.truncate(caps::MAX_SEEDS);
    // Re-canonicalize to entity byte order (§7.3 rendering order).
    ranked.sort();

    let seeds: Vec<PriorSeedV1> = ranked
        .into_iter()
        .map(|(entity, ppb)| PriorSeedV1::new_for_test(&entity, ppb))
        .collect();
    source_report_ids.sort();
    source_report_ids.dedup();

    let seed_set = PriorSeedSetV1::new_for_test(
        seeds,
        source_report_ids,
        unavailable_reports,
        config.config_hash()?,
    );
    let _ = terminal_status; // consumed by the 3F envelope assembly, not the seed set
    Ok((seed_set, dropped_seeds))
}

// ── Stage 3E: integer fixed-point PPR propagation (spec §7.6) ─────────────

/// PPR propagation outcome: the final mass vector plus convergence facts.
///
/// The vector lists only entities with mass > 0, in entity byte order
/// (§7.6: "entries > 0 form the vector"). All values are ppb integers.
#[derive(Debug, Clone)]
pub struct PprOutcome {
    vector: Vec<(String, u128)>,
    iterations: u32,
    converged: bool,
    residual_ppb: u128,
}

impl PprOutcome {
    /// Read-only: positive-mass entries in entity byte order.
    #[must_use]
    pub fn vector(&self) -> &[(String, u128)] {
        &self.vector
    }
    /// Read-only: iterations executed.
    #[must_use]
    pub const fn iterations(&self) -> u32 {
        self.iterations
    }
    /// Read-only: whether the L∞ threshold was reached within the cap.
    #[must_use]
    pub const fn converged(&self) -> bool {
        self.converged
    }
    /// Read-only: final-iteration flooring loss per the exact §7.6 formula.
    #[must_use]
    pub const fn residual_ppb(&self) -> u128 {
        self.residual_ppb
    }
}

/// Run the integer fixed-point Personalized PageRank recurrence over the
/// bounded entity graph (spec §7.6). No floats anywhere; all arithmetic is
/// u128 checked and a checked overflow fails closed with
/// [`OutcomeError::Malformed`] (prereg overflow policy).
///
/// * `teleport(e) = floor(s(e) × DAMPING_PPM / 1e6)` — computed once.
/// * `m_0 = teleport`; `prop_t(e) = Σ_{u∈nbr(e)} floor(m_t(u)·C/(1e12·out(u)))`
///   summed in neighbor byte order; `m_{t+1} = teleport + prop_t`.
/// * Stops at L∞ ≤ `EPSILON_PPB` (`converged = true`) or 64 iterations
///   (`converged = false` — recorded fact, never an error).
/// * `residual_ppb = floor(Σ_{u:out>0} (n_u mod d_u) / 1e12)` over the
///   final iteration, with `n_u = m_final(u)·C`, `d_u = 1e12·out(u)`.
pub fn run_ppr(
    graph: &EntityGraphV1,
    seeds: &PriorSeedSetV1,
    config: &PriorConfigV1,
) -> Result<PprOutcome, OutcomeError> {
    config.validate_frozen()?;
    // Seed lookup by entity (seeds are already byte-ordered and deduped).
    let seed_of = |entity: &str| -> u128 {
        seeds
            .seeds()
            .iter()
            .find(|s| s.entity() == entity)
            .map_or(0, |s| s.ppb())
    };

    // Adjacency in canonical byte order (entities() is byte-sorted; edges()
    // are (a,b)-sorted, so neighbor lists inherit canonical order).
    let entities: Vec<&str> = graph.entities().iter().map(String::as_str).collect();
    let index_of = |entity: &str| -> Option<usize> { entities.iter().position(|e| *e == entity) };
    let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); entities.len()];
    for edge in graph.edges() {
        let (a, b) = (edge.a(), edge.b());
        if let (Some(ai), Some(bi)) = (index_of(a), index_of(b)) {
            neighbors[ai].push(bi);
            neighbors[bi].push(ai);
        }
    }

    let c = 1_000_000_000_000u128
        .checked_sub(config.damping_ppm * 1_000_000u128)
        .ok_or(OutcomeError::Malformed)?;
    // Teleport, computed once (m_0 = teleport).
    let teleport: Vec<u128> = entities
        .iter()
        .map(|e| {
            seed_of(e)
                .checked_mul(config.damping_ppm)
                .and_then(|v| v.checked_div(1_000_000))
                .ok_or(OutcomeError::Malformed)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut mass = teleport.clone();
    let mut iterations = 0u32;
    let mut converged = false;
    for step in 0..config.max_iterations {
        iterations = step + 1;
        // prop_t(e) = Σ floor(m_t(u)·C / (1e12·out(u))) in neighbor order.
        let mut next: Vec<u128> = teleport.clone();
        let mut delta: u128 = 0;
        for (i, entity_neighbors) in neighbors.iter().enumerate() {
            let mut prop = 0u128;
            for &u in entity_neighbors {
                let out_u =
                    u128::try_from(neighbors[u].len()).map_err(|_| OutcomeError::Malformed)?;
                let n_u = mass[u].checked_mul(c).ok_or(OutcomeError::Malformed)?;
                let d_u = 1_000_000_000_000u128
                    .checked_mul(out_u)
                    .ok_or(OutcomeError::Malformed)?;
                let term = n_u / d_u;
                prop = prop.checked_add(term).ok_or(OutcomeError::Malformed)?;
            }
            let m_next = teleport[i]
                .checked_add(prop)
                .ok_or(OutcomeError::Malformed)?;
            let diff = m_next.abs_diff(mass[i]);
            delta = delta.max(diff);
            next[i] = m_next;
        }
        mass = next;
        if delta <= config.epsilon_ppb {
            converged = true;
            break;
        }
    }

    // residual_ppb over the final iteration (exact §7.6 identity).
    let mut remainder_sum = 0u128;
    for (i, entity_neighbors) in neighbors.iter().enumerate() {
        if entity_neighbors.is_empty() {
            continue; // out=0 entities contribute nothing
        }
        let out_u = u128::try_from(entity_neighbors.len()).map_err(|_| OutcomeError::Malformed)?;
        let n_u = mass[i].checked_mul(c).ok_or(OutcomeError::Malformed)?;
        let d_u = 1_000_000_000_000u128
            .checked_mul(out_u)
            .ok_or(OutcomeError::Malformed)?;
        remainder_sum = remainder_sum
            .checked_add(n_u % d_u)
            .ok_or(OutcomeError::Malformed)?;
    }
    let residual_ppb = remainder_sum / 1_000_000_000_000;

    // Vector: entries > 0 in entity byte order; range asserted anyway.
    let mut vector = Vec::new();
    for (i, m) in mass.iter().enumerate() {
        if *m > 0 {
            if *m > caps::PRIOR_MAX_PPB {
                // Reachable: hub concentration can exceed 1e9 even from
                // per-seed-clamped legal seeds (e.g. 32 max seeds on a
                // degree-32 hub concentrate ≈4.17e9). The prior range is a
                // hard fail-closed gate, never a silent clamp.
                return Err(OutcomeError::Malformed);
            }
            vector.push((entities[i].to_owned(), *m));
        }
    }
    Ok(PprOutcome {
        vector,
        iterations,
        converged,
        residual_ppb,
    })
}

// ── Stage 3F: canonical prior assembly and verification (spec §7.4/§9) ────

/// Assemble the canonical 13-member `SaliencePriorV1` envelope from the
/// bounded graph, the folded seed set, and the PPR outcome (spec §7.4).
///
/// `prior_id` is derived over placeholder-normalized canonical bytes —
/// BLAKE3(`oc-03-prior-v1` + NUL + canonical bytes with `prior_id` set to
/// the literal `"prior_id"` placeholder; the derived value is substituted
/// back afterwards (§9, inheriting the OC-02 §9.2 precedent). `dropped_seeds`
/// is the seed-cap remainder from `derive_seeds`. Mixed terminal statuses
/// were already rejected upstream (§7.3).
///
/// # Errors
/// Returns [`OutcomeError::Malformed`] when config validation or the
/// graph/seed consistency gates fail.
pub fn assemble_prior(
    graph: EntityGraphV1,
    seeds: PriorSeedSetV1,
    ppr: &PprOutcome,
    dropped_seeds: u128,
    terminal_status: &str,
    config: &PriorConfigV1,
) -> Result<SaliencePriorV1, OutcomeError> {
    config.validate_frozen()?;
    if terminal_status != "terminal" && terminal_status != "unterminated" {
        return Err(OutcomeError::Malformed);
    }
    // Consistency: the vector must live on the graph's entities and every
    // value must be in range (run_ppr already enforces; re-assert here so
    // verification is not the only gate).
    let mut vector = Vec::with_capacity(ppr.vector().len());
    for (entity, ppb) in ppr.vector() {
        if *ppb == 0 || *ppb > caps::PRIOR_MAX_PPB {
            return Err(OutcomeError::Malformed);
        }
        if !graph.entities().contains(entity) {
            return Err(OutcomeError::Malformed);
        }
        vector.push(PriorSeedV1::new_for_test(entity, *ppb));
    }
    vector.sort_by(|a, b| a.entity().cmp(b.entity()));

    let placeholder = SaliencePriorV1::new_for_test(
        "prior_id".to_owned(),
        config.config_hash()?.clone(),
        seeds.source_report_ids().to_vec(),
        graph,
        seeds,
        vector,
        ppr.iterations(),
        ppr.converged(),
        ppr.residual_ppb(),
        dropped_seeds,
        leak_terminal(terminal_status),
    );
    let canonical = placeholder.canonical_bytes()?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(PRIOR_ID_DOMAIN);
    hasher.update(&canonical);
    let prior_id = format!("{}{}", PRIOR_ID_PREFIX, hasher.finalize().to_hex());
    Ok(SaliencePriorV1 {
        version: 1,
        prior_id,
        ..placeholder
    })
}

/// Leak a runtime terminal-status string into the `'static` field.
///
/// The wire type freezes `terminal_status` as `&'static str` for
/// compile-time spelling discipline; assembly only ever receives the two
/// frozen spellings, validated above.
fn leak_terminal(status: &str) -> &'static str {
    if status == "terminal" {
        "terminal"
    } else {
        "unterminated"
    }
}

/// Verify a prior artifact by REBUILDING every intermediate from the
/// caller-supplied inputs (spec §8/§9.4): the graph is rebuilt from the
/// session payloads, the seeds from the verified report contributions, the
/// vector from a fresh `run_ppr`, and the whole envelope is reassembled —
/// byte-equality against the artifact's canonical bytes is then required.
/// Recorded intermediates are never trusted: a self-consistent forgery
/// (re-derived prior_id over falsified members) is rejected because the
/// rebuilt envelope diverges from the recorded one.
///
/// # Errors
/// Returns [`OutcomeError::Malformed`] on any structural failure, member
/// mismatch, non-canonical bytes, or rebuild divergence.
pub fn verify_prior(
    bytes: &[u8],
    sessions: &[SessionPayloads<'_>],
    reports: &[ReportContribution],
    event_payloads: &[(&str, &str)],
    config: &PriorConfigV1,
) -> Result<(), OutcomeError> {
    let prior = parse_prior_bytes(bytes)?;
    // Canonical-bytes gate: raw bytes must equal the JCS re-render.
    if bytes != prior.canonical_bytes()? {
        return Err(OutcomeError::Malformed);
    }
    // Terminal status consistency: derive_seeds rejects mixed statuses, so
    // the artifact's spelling must match the uniform status of the inputs.
    let uniform_status = reports
        .first()
        .map_or(TERMINAL_STATUS, |r| r.terminal_status());
    if prior.terminal_status() != uniform_status {
        return Err(OutcomeError::Malformed);
    }
    // Rebuild the graph from the session payloads (never trust the
    // recorded graph).
    let rebuilt_graph = build_entity_graph(sessions, config)?;
    // Rebuild the seeds from the verified report contributions.
    let (rebuilt_seeds, dropped_seeds) = derive_seeds(reports, event_payloads, config)?;
    // Rebuild the vector from a fresh PPR run over the rebuilt inputs.
    let rebuilt_ppr = run_ppr(&rebuilt_graph, &rebuilt_seeds, config)?;
    // Reassemble the full envelope from rebuilt inputs only.
    let rebuilt = assemble_prior(
        rebuilt_graph,
        rebuilt_seeds,
        &rebuilt_ppr,
        dropped_seeds,
        uniform_status,
        config,
    )?;
    // Byte-equality gate: the artifact must equal the rebuilt envelope.
    if prior.canonical_bytes()? != rebuilt.canonical_bytes()? {
        return Err(OutcomeError::Malformed);
    }
    Ok(())
}

/// [`verify_prior`]; this only extracts members).
///
/// # Errors
/// Returns [`OutcomeError::Malformed`] on non-object input or missing
/// members.
pub fn parse_prior_bytes(bytes: &[u8]) -> Result<SaliencePriorV1, OutcomeError> {
    let text = std::str::from_utf8(bytes).map_err(|_| OutcomeError::Malformed)?;
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| OutcomeError::Malformed)?;
    let object = value.as_object().ok_or(OutcomeError::Malformed)?;
    let get_str = |k: &str| -> Result<String, OutcomeError> {
        object
            .get(k)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or(OutcomeError::Malformed)
    };
    let get_u64 = |k: &str| -> Result<u128, OutcomeError> {
        object
            .get(k)
            .and_then(serde_json::Value::as_u64)
            .map(u128::from)
            .ok_or(OutcomeError::Malformed)
    };
    let prior_id = get_str("prior_id")?;
    let config_hash = get_str("config_hash")?;
    let thorn_status = get_str("thorn_status")?;
    if thorn_status != versions::THORN_STATUS {
        return Err(OutcomeError::Malformed);
    }
    let terminal_status = get_str("terminal_status")?;
    if terminal_status != "terminal" && terminal_status != "unterminated" {
        return Err(OutcomeError::Malformed);
    }
    let source_report_ids: Vec<String> = object
        .get("source_report_ids")
        .and_then(serde_json::Value::as_array)
        .ok_or(OutcomeError::Malformed)?
        .iter()
        .map(|v| v.as_str().map(str::to_owned).ok_or(OutcomeError::Malformed))
        .collect::<Result<_, _>>()?;
    let iterations = u32::try_from(get_u64("iterations")?).map_err(|_| OutcomeError::Malformed)?;
    let converged = object
        .get("converged")
        .and_then(serde_json::Value::as_bool)
        .ok_or(OutcomeError::Malformed)?;
    let residual_ppb = get_u64("residual_ppb")?;
    let dropped_seeds = get_u64("dropped_seeds")?;
    let version = get_u64("version")?;
    if version != 1 {
        return Err(OutcomeError::Malformed);
    }

    // Nested graph, seeds, vector via lenient serde_json extraction.
    let parse_seed = |v: &serde_json::Value| -> Result<PriorSeedV1, OutcomeError> {
        let o = v.as_object().ok_or(OutcomeError::Malformed)?;
        let entity = o
            .get("entity")
            .and_then(serde_json::Value::as_str)
            .ok_or(OutcomeError::Malformed)?;
        let ppb = o
            .get("ppb")
            .and_then(serde_json::Value::as_u64)
            .ok_or(OutcomeError::Malformed)?;
        Ok(PriorSeedV1::new_for_test(entity, u128::from(ppb)))
    };
    let graph_object = object
        .get("graph")
        .and_then(serde_json::Value::as_object)
        .ok_or(OutcomeError::Malformed)?;
    let entities: Vec<String> = graph_object
        .get("entities")
        .and_then(serde_json::Value::as_array)
        .ok_or(OutcomeError::Malformed)?
        .iter()
        .map(|v| v.as_str().map(str::to_owned).ok_or(OutcomeError::Malformed))
        .collect::<Result<_, _>>()?;
    let edges: Vec<EntityEdgeV1> = graph_object
        .get("edges")
        .and_then(serde_json::Value::as_array)
        .ok_or(OutcomeError::Malformed)?
        .iter()
        .map(|v| {
            let o = v.as_object().ok_or(OutcomeError::Malformed)?;
            Ok(EntityEdgeV1::new_for_test(
                o.get("a")
                    .and_then(serde_json::Value::as_str)
                    .ok_or(OutcomeError::Malformed)?,
                o.get("b")
                    .and_then(serde_json::Value::as_str)
                    .ok_or(OutcomeError::Malformed)?,
            ))
        })
        .collect::<Result<_, _>>()?;
    let graph = EntityGraphV1::new_for_test(
        entities,
        edges,
        graph_object
            .get("truncated_entities")
            .and_then(serde_json::Value::as_u64)
            .map(u128::from)
            .ok_or(OutcomeError::Malformed)?,
        graph_object
            .get("truncated_edges")
            .and_then(serde_json::Value::as_u64)
            .map(u128::from)
            .ok_or(OutcomeError::Malformed)?,
        graph_object
            .get("config_hash")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or(OutcomeError::Malformed)?,
    );

    let seeds_object = object
        .get("seeds")
        .and_then(serde_json::Value::as_object)
        .ok_or(OutcomeError::Malformed)?;
    let seeds_list: Vec<PriorSeedV1> = seeds_object
        .get("seeds")
        .and_then(serde_json::Value::as_array)
        .ok_or(OutcomeError::Malformed)?
        .iter()
        .map(&parse_seed)
        .collect::<Result<_, _>>()?;
    let seed_source_ids: Vec<String> = seeds_object
        .get("source_report_ids")
        .and_then(serde_json::Value::as_array)
        .ok_or(OutcomeError::Malformed)?
        .iter()
        .map(|v| v.as_str().map(str::to_owned).ok_or(OutcomeError::Malformed))
        .collect::<Result<_, _>>()?;
    let seeds = PriorSeedSetV1::new_for_test(
        seeds_list,
        seed_source_ids,
        seeds_object
            .get("unavailable_reports")
            .and_then(serde_json::Value::as_u64)
            .map(u128::from)
            .ok_or(OutcomeError::Malformed)?,
        seeds_object
            .get("config_hash")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or(OutcomeError::Malformed)?,
    );

    let vector: Vec<PriorSeedV1> = object
        .get("vector")
        .and_then(serde_json::Value::as_array)
        .ok_or(OutcomeError::Malformed)?
        .iter()
        .map(&parse_seed)
        .collect::<Result<_, _>>()?;

    Ok(SaliencePriorV1::new_for_test(
        prior_id,
        config_hash,
        source_report_ids,
        graph,
        seeds,
        vector,
        iterations,
        converged,
        residual_ppb,
        dropped_seeds,
        leak_terminal(&terminal_status),
    ))
}
