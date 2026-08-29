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

    /// Read-only accessor for the convergence flag.
    #[must_use]
    pub const fn converged(&self) -> bool {
        self.converged
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
