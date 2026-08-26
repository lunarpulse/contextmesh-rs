//! OC-02 Stage 2A: attribution mechanism tags, frozen constants, and the
//! canonical `AttributionConfigV1` with its domain-separated config hash.
//!
//! Frozen by `spec-oc-02-attribution.md` (§5, §7.1). Version strings and
//! caps are consumed verbatim from the P1 preregistration
//! (`p1-prereg-config.json`, SHA-256 `be20d8fc…`, commit `c080722`).
//! Change control: spec §15 — founder approval required for any change.

use crate::error::OutcomeError;
use blake3::Hasher;

/// The mechanism ladder is exactly these five variants (spec §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mechanism {
    /// M0: raw string-overlap backtracking (deterministic, cost ~0).
    M0,
    /// M1: normalized-value nomination (deterministic, cheap).
    M1,
    /// M2: explicit structural citation only (deterministic, cheap).
    M2,
    /// M3: single-event counterfactual ablation (adapter, shortlist-only).
    M3,
    /// M4: Shapley-sampling coalition attribution (adapter, shortlist-only).
    M4,
}

impl Mechanism {
    /// Stable wire name (round-trips through the tag schema, T01).
    pub const fn as_str(self) -> &'static str {
        match self {
            Mechanism::M0 => "M0",
            Mechanism::M1 => "M1",
            Mechanism::M2 => "M2",
            Mechanism::M3 => "M3",
            Mechanism::M4 => "M4",
        }
    }

    /// Parse the exact wire name; anything else is `Malformed`.
    pub fn from_name(s: &str) -> Result<Self, OutcomeError> {
        match s {
            "M0" => Ok(Mechanism::M0),
            "M1" => Ok(Mechanism::M1),
            "M2" => Ok(Mechanism::M2),
            "M3" => Ok(Mechanism::M3),
            "M4" => Ok(Mechanism::M4),
            _ => Err(OutcomeError::Malformed),
        }
    }
}

/// Frozen extractor version strings (prereg `evaluation.extractor_versions`).
pub mod versions {
    /// M0 extractor version (frozen).
    pub const M0: &str = "oc-prototype-m0-v1-compatible";
    /// M1 extractor version (frozen).
    pub const M1: &str = "oc-1-m1n-v1";
    /// M2 extractor version (frozen placeholder bound by this spec).
    pub const M2: &str = "oc-2-m2-v1";
    /// Reference only — the prior extractor is owned by OC-03, not OC-02.
    pub const PRIOR: &str = "oc-3-prior-v1";
}

/// Frozen caps (prereg `evaluation.budgets` + spec §5).
pub mod caps {
    /// Shortlist maximum entries (prereg `candidate_shortlist_cap`).
    pub const SHORTLIST: usize = 32;
    /// M3 judge calls per session (prereg `m3_judge_calls_per_session_cap`).
    pub const M3_JUDGE_CALLS_PER_SESSION: usize = 8;
    /// M4 Shapley samples per candidate (prereg `m4_shapley_samples_per_candidate_cap`).
    pub const M4_SAMPLES_PER_CANDIDATE: usize = 64;
    /// M4 judge calls per session (prereg `m4_judge_calls_per_session_cap`).
    pub const M4_JUDGE_CALLS_PER_SESSION: usize = 128;
    /// Nomination token bound per event payload (spec §5).
    pub const TOKENS_PER_EVENT: usize = 256;
    /// Maximum token length in bytes (spec §5).
    pub const TOKEN_BYTES: usize = 1_024;
}

/// Domain-separation constant for report IDs (spec §5; OC-01 pattern:
/// literal domain bytes including the NUL terminator).
pub const ATTRIBUTION_REPORT_ID_DOMAIN: &[u8] = b"oc-02-attr-report-v1\0";

/// Domain-separation constant for config hashes (spec §5).
pub const ATTRIBUTION_CONFIG_HASH_DOMAIN: &[u8] = b"oc-02-attr-config-v1\0";

/// Typed prefix for report IDs (spec §5).
pub const REPORT_ID_PREFIX: &str = "ocattr1_";

/// Typed prefix for config hashes (spec §5).
pub const CONFIG_HASH_PREFIX: &str = "ocattrcfg1_";

/// Domain-separation constant for evidence fingerprints (spec §7.2; the
/// fingerprint derivation follows the §5 literal-domain pattern).
pub const ATTRIBUTION_EVIDENCE_FINGERPRINT_DOMAIN: &[u8] = b"oc-02-attr-evidence-v1\0";

/// Typed prefix for evidence fingerprints (spec §7.2).
pub const EVIDENCE_FINGERPRINT_PREFIX: &str = "ocfp1_";

/// The frozen P1 preregistration SHA-256 (spec §5; verifies the policy
/// the evaluation must run under — consumed verbatim, never redefined).
pub const PREREG_SHA256: &str = "be20d8fc48771098e745038b906dd13456ffcebdeb424cee25e91d52eae784c9";

/// Canonical attribution configuration (spec §7.1). All fields are the
/// frozen policy values; the struct exists so every mechanism edge can
/// carry the same config hash and verification can rebuild it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributionConfigV1 {
    /// Shortlist cap (frozen: 32).
    pub shortlist_cap: usize,
    /// M3 judge-call cap per session (frozen: 8).
    pub m3_judge_calls_per_session: usize,
    /// M4 samples per candidate (frozen: 64).
    pub m4_samples_per_candidate: usize,
    /// M4 judge-call cap per session (frozen: 128).
    pub m4_judge_calls_per_session: usize,
    /// Nomination tokens per event payload (frozen: 256).
    pub tokens_per_event: usize,
    /// Maximum token bytes (frozen: 1,024).
    pub token_bytes: usize,
}

impl Default for AttributionConfigV1 {
    fn default() -> Self {
        Self {
            shortlist_cap: caps::SHORTLIST,
            m3_judge_calls_per_session: caps::M3_JUDGE_CALLS_PER_SESSION,
            m4_samples_per_candidate: caps::M4_SAMPLES_PER_CANDIDATE,
            m4_judge_calls_per_session: caps::M4_JUDGE_CALLS_PER_SESSION,
            tokens_per_event: caps::TOKENS_PER_EVENT,
            token_bytes: caps::TOKEN_BYTES,
        }
    }
}

impl AttributionConfigV1 {
    /// Fail if any field deviates from the frozen values (spec §15 — the
    /// configuration is not tunable; a deviation is a contract violation).
    pub fn validate_frozen(&self) -> Result<(), OutcomeError> {
        let d = Self::default();
        if *self != d {
            return Err(OutcomeError::Malformed);
        }
        Ok(())
    }

    /// Deterministic canonical bytes (JCS-compatible single-line JSON,
    /// fixed key order; spec §6). Two serializations are byte-equal (T03).
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, OutcomeError> {
        self.validate_frozen()?;
        // Fixed-order canonical form; every value is the frozen constant,
        // so the bytes are a constant string by construction.
        let s = format!(
            "{{\"m3_judge_calls_per_session\":{},\"m4_judge_calls_per_session\":{},\"m4_samples_per_candidate\":{},\"shortlist_cap\":{},\"token_bytes\":{},\"tokens_per_event\":{}}}",
            self.m3_judge_calls_per_session,
            self.m4_judge_calls_per_session,
            self.m4_samples_per_candidate,
            self.shortlist_cap,
            self.token_bytes,
            self.tokens_per_event
        );
        Ok(s.into_bytes())
    }

    /// Domain-separated BLAKE3 config hash, typed `ocattrcfg1_…` (T04).
    pub fn config_hash(&self) -> Result<String, OutcomeError> {
        let bytes = self.canonical_bytes()?;
        let mut h = Hasher::new();
        h.update(ATTRIBUTION_CONFIG_HASH_DOMAIN);
        h.update(bytes.as_slice());
        let hex = h.finalize().to_hex().to_string();
        Ok(format!("{}{}", CONFIG_HASH_PREFIX, hex))
    }
}

/// Stage 2B (spec §3): M0 raw string-overlap backtracking — deterministic,
/// cost ~0, blind to reformatted values by design (A02).
///
/// A "session" for M0 is one attribution computation bound to one verified
/// ledger in one context (spec §2.1). Inputs are already-verified event
/// payloads plus the ledger's terminal/outcome evidence text.
#[derive(Debug, Clone)]
pub struct M0Nomination {
    /// The nominated event's canonical text identifier.
    pub event: String,
    /// Provenance tag: mechanism, extractor version, config hash (C2).
    pub mechanism: AttributionMechanismTag,
    /// Kind of evidence that produced the nomination (spec §7.2).
    pub evidence_kind: EvidenceKind,
    /// Domain-separated BLAKE3 fingerprint of the minimal evidence bytes
    /// (never raw transcript content; A18/X03 privacy).
    pub evidence_fingerprint: String,
}

/// Mechanism provenance tag carried by every attribution (spec §7.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributionMechanismTag {
    /// Which ladder step produced this attribution.
    pub mechanism: Mechanism,
    /// Frozen extractor version string (verbatim from prereg).
    pub extractor_version: &'static str,
    /// Domain-separated config hash of the frozen AttributionConfigV1.
    pub config_hash: String,
}

/// Evidence kinds recognized by the deterministic tier (spec §7.2 enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceKind {
    /// M0 raw token overlap.
    Overlap,
    /// M1 normalized-value equality (Stage 2C).
    Normalized,
    /// M2 explicit EventId citation (Stage 2D).
    Citation,
    /// M2 provider request/result linkage (Stage 2D).
    Linkage,
    /// M2 receipt/handoff reference (Stage 2D).
    Receipt,
    /// M2 summary coverage enumeration (Stage 2D).
    Summary,
    /// M2 signed artifact reference (Stage 2D).
    Artifact,
}

impl EvidenceKind {
    /// Stable wire name (spec §7.2 `evidence_kind` values).
    pub const fn as_str(self) -> &'static str {
        match self {
            EvidenceKind::Overlap => "overlap",
            EvidenceKind::Normalized => "normalized",
            EvidenceKind::Citation => "citation",
            EvidenceKind::Linkage => "linkage",
            EvidenceKind::Receipt => "receipt",
            EvidenceKind::Summary => "summary",
            EvidenceKind::Artifact => "artifact",
        }
    }
}

/// Extract bounded raw tokens from one event payload (A03: at most
/// `caps::TOKENS_PER_EVENT`, each at most `caps::TOKEN_BYTES` bytes).
/// Tokens are maximal runs of non-whitespace UTF-8; a token longer than
/// the bound is skipped and recorded as skipped (deterministic order =
/// payload order; dedup preserves first occurrence).
pub fn extract_tokens(payload: &str) -> (Vec<&str>, usize) {
    let mut out: Vec<&str> = Vec::new();
    let mut skipped = 0usize;
    for raw in payload.split_whitespace() {
        if raw.len() > caps::TOKEN_BYTES {
            skipped += 1;
            continue;
        }
        if out.len() >= caps::TOKENS_PER_EVENT {
            skipped += 1;
            continue;
        }
        if !out.contains(&raw) {
            out.push(raw);
        }
    }
    (out, skipped)
}

/// Domain-separated BLAKE3 fingerprint over the minimal evidence bytes,
/// typed `ocfp1_…` (spec §7.2; A18/X03 privacy — fingerprints only).
pub fn evidence_fingerprint(evidence_bytes: &[u8]) -> String {
    let mut h = Hasher::new();
    h.update(ATTRIBUTION_EVIDENCE_FINGERPRINT_DOMAIN);
    h.update(evidence_bytes);
    format!("{}{}", EVIDENCE_FINGERPRINT_PREFIX, h.finalize().to_hex())
}

/// Run M0 over one event payload against the outcome evidence text
/// (A01/A02/A05). The nomination domain is limited to the caller-provided
/// ledger-referenced events: an `event` absent from `referenced_events`
/// is rejected with `UnauthorizedEvent` and no edge is created (A04; the
/// OC-01 reserved category, no new categories). Deterministic: same
/// inputs → same nomination (including fingerprint and overlap set).
pub fn m0_nominate(
    event: &str,
    payload: &str,
    outcome_evidence: &str,
    referenced_events: &[&str],
    config: &AttributionConfigV1,
) -> Result<Option<M0Nomination>, OutcomeError> {
    config.validate_frozen()?;
    if !referenced_events.contains(&event) {
        return Err(OutcomeError::UnauthorizedEvent);
    }
    let (tokens, _skipped) = extract_tokens(payload);
    let evidence_tokens: Vec<&str> = outcome_evidence.split_whitespace().collect();
    let overlaps: Vec<&str> = tokens
        .iter()
        .copied()
        .filter(|t| evidence_tokens.contains(t))
        .collect();
    if overlaps.is_empty() {
        return Ok(None);
    }
    // Minimal evidence bytes: canonical overlap list — token-count then
    // each token, NUL-separated (deterministic, no raw transcript).
    let mut evidence = Vec::new();
    evidence.extend_from_slice(overlaps.len().to_string().as_bytes());
    for t in &overlaps {
        evidence.push(0u8);
        evidence.extend_from_slice(t.as_bytes());
    }
    Ok(Some(M0Nomination {
        event: event.to_string(),
        mechanism: AttributionMechanismTag {
            mechanism: Mechanism::M0,
            extractor_version: versions::M0,
            config_hash: config.config_hash()?,
        },
        evidence_kind: EvidenceKind::Overlap,
        evidence_fingerprint: evidence_fingerprint(&evidence),
    }))
}
