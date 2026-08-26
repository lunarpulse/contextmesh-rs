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
    if let Some(n) = m0_nominate_inner(event, payload, outcome_evidence, referenced_events, config)?
    {
        return Ok(Some(n));
    }
    Ok(None)
}

/// Shared inner nomination used by M0 and (via re-tagging) M1 tests;
/// keeps the domain gate and overlap detection in one place.
fn m0_nominate_inner(
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

/// Maximum absolute magnitude for normalized numeric values (spec §5:
/// ≤ 10^18 absolute; u128 widened intermediate; out-of-range → the
/// nomination is skipped and recorded, never an error).
pub const NUMERIC_MAGNITUDE_LIMIT: u128 = 1_000_000_000_000_000_000;

/// A normalized scalar extracted from one token (A06/A07/A08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedValue {
    /// A dimensionless number within the magnitude bound.
    Number(u128),
    /// A percentage in basis points (e.g. "9.5%" → 950 bps).
    Percent(u128),
    /// A filesystem-like path with trailing-slash and duplicate-slash
    /// folding already applied ("/a//b/" and "/a/b" are equal).
    Path(String),
}

impl NormalizedValue {
    /// Canonical text form used for equality comparison and evidence
    /// bytes (Number → decimal, Percent → "<bps>bps", Path as-is).
    pub fn canonical(&self) -> String {
        match self {
            NormalizedValue::Number(n) => n.to_string(),
            NormalizedValue::Percent(b) => format!("{b}bps"),
            NormalizedValue::Path(p) => p.clone(),
        }
    }
}

/// Parse one token into a normalized value (A06/A08). Recognizes:
/// integer or simple decimal with optional unit suffix k/M/B/G
/// (case-insensitive), optionally percent ("%"); plain percent values
/// ("12.5%"); and path-like tokens (containing "/") with slash folding.
/// Returns None for non-numeric, non-path tokens and for values outside
/// the magnitude bound (callers record the skip — A07).
pub fn parse_normalized(token: &str) -> Option<NormalizedValue> {
    // Path folding: any token containing '/' is treated as a path.
    if token.contains('/') {
        return Some(NormalizedValue::Path(fold_path(token)));
    }
    // Split off a trailing percent sign if present. Percent values are
    // canonicalized to basis points (1% = 100bps), so the numeric part
    // scales by 100 ("9.5%" → 950bps; "50%" → 5000bps). Unit-suffixed
    // percent is not a recognized form (percent takes plain decimals).
    let (body, is_percent) = match token.strip_suffix('%') {
        Some(b) => (b, true),
        None => (token, false),
    };
    let lower = body.to_ascii_lowercase();
    if is_percent {
        let scaled = parse_decimal_to_scaled_u128(&lower, 100)?;
        if scaled > NUMERIC_MAGNITUDE_LIMIT {
            return None;
        }
        return Some(NormalizedValue::Percent(scaled));
    }
    // Split off a unit suffix (case-insensitive k/M/B/G).
    let (num_part, multiplier): (&str, u128) = if let Some(n) = lower.strip_suffix('k') {
        (n, 1_000)
    } else if let Some(n) = lower.strip_suffix('m') {
        (n, 1_000_000)
    } else if let Some(n) = lower.strip_suffix('b') {
        (n, 1_000_000_000)
    } else if let Some(n) = lower.strip_suffix('g') {
        (n, 1_000_000_000_000)
    } else {
        (lower.as_str(), 1)
    };
    let scaled = parse_decimal_to_scaled_u128(num_part, multiplier)?;
    // Magnitude bound: ≤ 10^18 absolute (A07); out-of-range → None.
    if scaled > NUMERIC_MAGNITUDE_LIMIT {
        return None;
    }
    Some(NormalizedValue::Number(scaled))
}

/// Fold a path token: lowercase, collapse duplicate slashes, drop the
/// trailing slash ("/A//b/" → "/a/b"). Deterministic (A08).
pub fn fold_path(token: &str) -> String {
    let lower = token.to_ascii_lowercase();
    let mut folded = String::with_capacity(lower.len());
    let mut prev_slash = false;
    for ch in lower.chars() {
        if ch == '/' {
            if !prev_slash {
                folded.push(ch);
            }
            prev_slash = true;
        } else {
            folded.push(ch);
            prev_slash = false;
        }
    }
    while folded.len() > 1 && folded.ends_with('/') {
        folded.pop();
    }
    folded
}

/// Parse an integer or one-dot decimal string into u128 scaled by
/// `multiplier`. Digits before the dot scale by `multiplier`; digits
/// after the dot contribute d*multiplier/10^(i+1) with integer
/// arithmetic (deterministic truncation). Returns None for empty,
/// signs, exponents, multiple dots, or u128 overflow (fail-closed).
pub fn parse_decimal_to_scaled_u128(s: &str, multiplier: u128) -> Option<u128> {
    if s.is_empty() {
        return None;
    }
    let (int_part, frac_part) = match s.split_once('.') {
        Some((i, f)) => (i, f),
        None => (s, ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let int_val: u128 = if int_part.is_empty() {
        0
    } else {
        int_part.parse().ok()?
    };
    // u128 checked multiply/add (fail-closed to None on overflow).
    let base = int_val.checked_mul(multiplier)?;
    if frac_part.is_empty() {
        return Some(base);
    }
    let mut acc = base;
    let mut denom_pow = 10u128;
    for ch in frac_part.chars() {
        let d = (ch as u8 - b'0') as u128;
        let add = d.checked_mul(multiplier)?.checked_div(denom_pow);
        acc = acc.checked_add(add?)?;
        denom_pow = denom_pow.checked_mul(10)?;
    }
    Some(acc)
}

/// Stage 2C (spec §3): M1 normalized-equality nomination (A06). For one
/// referenced event payload, if any payload token normalizes to the same
/// canonical value as any evidence token, nominate the event with an M1
/// tag. Numeric-looking payload tokens that fail to normalize
/// (out-of-magnitude — A07) are returned in `skipped` (recorded, not an
/// error). Deterministic: same inputs → same nomination.
pub fn m1_nominate(
    event: &str,
    payload: &str,
    outcome_evidence: &str,
    referenced_events: &[&str],
    config: &AttributionConfigV1,
) -> Result<(Option<M0Nomination>, Vec<String>), OutcomeError> {
    config.validate_frozen()?;
    if !referenced_events.contains(&event) {
        return Err(OutcomeError::UnauthorizedEvent);
    }
    let (tokens, _t_skip) = extract_tokens(payload);
    let mut evidence_canonical: Vec<String> = Vec::new();
    for et in outcome_evidence.split_whitespace() {
        if let Some(v) = parse_normalized(et) {
            let c = v.canonical();
            if !evidence_canonical.contains(&c) {
                evidence_canonical.push(c);
            }
        }
    }
    let mut matched: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for t in &tokens {
        match parse_normalized(t) {
            Some(v) => {
                let c = v.canonical();
                if evidence_canonical.contains(&c) && !matched.contains(&c) {
                    matched.push(c);
                }
            }
            None => {
                if looks_numericish(t) {
                    let raw = t.to_string();
                    if !skipped.contains(&raw) {
                        skipped.push(raw);
                    }
                }
            }
        }
    }
    if matched.is_empty() {
        return Ok((None, skipped));
    }
    let mut evidence = Vec::new();
    evidence.extend_from_slice(matched.len().to_string().as_bytes());
    for c in &matched {
        evidence.push(0u8);
        evidence.extend_from_slice(c.as_bytes());
    }
    let nom = M0Nomination {
        event: event.to_string(),
        mechanism: AttributionMechanismTag {
            mechanism: Mechanism::M1,
            extractor_version: versions::M1,
            config_hash: config.config_hash()?,
        },
        evidence_kind: EvidenceKind::Normalized,
        evidence_fingerprint: evidence_fingerprint(&evidence),
    };
    Ok((Some(nom), skipped))
}

/// Heuristic: does this token look like it was meant as a number or
/// percent (so a normalization failure means out-of-magnitude or
/// malformed)? Used only to decide skip-recording (A07).
fn looks_numericish(t: &str) -> bool {
    let core = t.strip_suffix('%').unwrap_or(t);
    let core = core
        .strip_suffix(|c: char| matches!(c, 'k' | 'K' | 'm' | 'M' | 'b' | 'B' | 'g' | 'G'))
        .unwrap_or(core);
    !core.is_empty() && core.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Typed-token shape check for one of the canonical identifier forms
/// (`evt1_`/`rcpt1_`/`ocout1_` + 43 base64url-no-pad chars = 32 bytes).
/// Returns the prefix when the token matches, else None (A09/A12/A14).
pub fn canonical_id_kind(token: &str) -> Option<&'static str> {
    for (prefix, _) in [("evt1_", 43usize), ("rcpt1_", 43), ("ocout1_", 43)] {
        if token.len() == prefix.len() + 43
            && token.starts_with(prefix)
            && token[prefix.len()..]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Some(match prefix {
                "evt1_" => "event",
                "rcpt1_" => "receipt",
                _ => "artifact",
            });
        }
    }
    None
}

/// Stage 2D (spec §3, D-C-07): the M2 v1 explicit structural extractor
/// recognizes EXACTLY five verifiable structures. This enum enumerates
/// them; `m2_extract` is the only producer (A15: near-miss text —
/// paraphrases, citation-like prose, unparseable IDs — yields nothing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum M2Structure {
    /// 1. Explicit canonical EventId citation (`evt1_…` in payload).
    EventIdCitation(String),
    /// 2. Provider request/result linkage from core public metadata
    ///    (a matched request-id/result-id pair supplied by the caller
    ///    from event metadata — never inferred from prose).
    ProviderLinkage {
        /// The core public request identifier.
        request_id: String,
        /// The core public result identifier.
        result_id: String,
    },
    /// 3. Option B receipt/handoff reference (`rcpt1_…`).
    ReceiptReference(String),
    /// 4. Summary coverage enumeration (a `covers:[…]`-style list whose
    ///    entries are all canonical EventIds present in the referenced
    ///    set — an enumeration that names events it covers).
    SummaryCoverage(Vec<String>),
    /// 5. Signed artifact reference (`ocout1_…`).
    ArtifactReference(String),
}

/// Outcome of one M2 extraction pass over a single event payload.
#[derive(Debug, Clone)]
pub struct M2Extraction {
    /// The structures recognized in this payload.
    pub structures: Vec<M2Structure>,
    /// Canonical-id-shaped tokens whose referent does not exist in the
    /// caller-provided universe — recorded as forged, no edge (A10).
    pub forged: Vec<String>,
}

impl M2Structure {
    /// The evidence kind wire value for this structure (§7.2).
    pub fn evidence_kind(&self) -> EvidenceKind {
        match self {
            M2Structure::EventIdCitation(_) => EvidenceKind::Citation,
            M2Structure::ProviderLinkage { .. } => EvidenceKind::Linkage,
            M2Structure::ReceiptReference(_) => EvidenceKind::Receipt,
            M2Structure::SummaryCoverage(_) => EvidenceKind::Summary,
            M2Structure::ArtifactReference(_) => EvidenceKind::Artifact,
        }
    }

    /// Deterministic canonical bytes for the evidence fingerprint.
    fn evidence_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let tag = match self {
            M2Structure::EventIdCitation(_) => b"c",
            M2Structure::ProviderLinkage { .. } => b"l",
            M2Structure::ReceiptReference(_) => b"r",
            M2Structure::SummaryCoverage(_) => b"s",
            M2Structure::ArtifactReference(_) => b"a",
        };
        out.extend_from_slice(tag);
        match self {
            M2Structure::EventIdCitation(id)
            | M2Structure::ReceiptReference(id)
            | M2Structure::ArtifactReference(id) => {
                out.push(0);
                out.extend_from_slice(id.as_bytes());
            }
            M2Structure::ProviderLinkage {
                request_id,
                result_id,
            } => {
                out.push(0);
                out.extend_from_slice(request_id.as_bytes());
                out.push(0);
                out.extend_from_slice(result_id.as_bytes());
            }
            M2Structure::SummaryCoverage(ids) => {
                out.extend_from_slice(ids.len().to_string().as_bytes());
                for id in ids {
                    out.push(0);
                    out.extend_from_slice(id.as_bytes());
                }
            }
        }
        out
    }
}

/// Extract the exactly-five M2 structures from one payload (A09–A15).
///
/// * `referenced_events` — the caller-provided universe of ledger-
///   referenced EventIds (the nomination domain; unknown `evt1_` IDs
///   are recorded as forged and produce no edge — A10).
/// * `metadata_pairs` — core public provider request/result pairs from
///   event metadata (structure 2 reads ONLY these, never prose).
/// * `summary_enumeration` — when the payload carries a summary-
///   coverage list, the caller supplies its entries here; every entry
///   must be a canonical referenced EventId (else nothing is recorded).
///
/// Near-miss text recognizes nothing (A15): paraphrases with no
/// literal canonical id, wrong-length ids, and wrong prefixes. A
/// canonical id embedded in prose IS recognized (extraction is
/// whitespace-token based — see A09's "analysis based on <id> and
/// prior work"), which is intended behavior.
pub fn m2_extract(
    payload: &str,
    referenced_events: &[&str],
    metadata_pairs: &[(&str, &str)],
    summary_enumeration: &[&str],
) -> M2Extraction {
    let mut structures = Vec::new();
    let mut forged = Vec::new();

    let mut seen_citations: Vec<String> = Vec::new();
    let mut seen_receipts: Vec<String> = Vec::new();
    let mut seen_artifacts: Vec<String> = Vec::new();
    for token in payload.split_whitespace() {
        let trimmed = token.trim_matches(|c: char| matches!(c, ',' | ';' | '.' | ')' | ']' | '"'));
        let kind = canonical_id_kind(trimmed);
        match (kind, trimmed) {
            (Some("event"), id) => {
                if referenced_events.contains(&id) {
                    if !seen_citations.iter().any(|s| s == id) {
                        seen_citations.push(id.to_string());
                        structures.push(M2Structure::EventIdCitation(id.to_string()));
                    }
                } else if !forged.contains(&id.to_string()) {
                    forged.push(id.to_string());
                }
            }
            (Some("receipt"), id) => {
                if !seen_receipts.iter().any(|s| s == id) {
                    seen_receipts.push(id.to_string());
                    structures.push(M2Structure::ReceiptReference(id.to_string()));
                }
            }
            (Some("artifact"), id) if !seen_artifacts.iter().any(|s| s == id) => {
                seen_artifacts.push(id.to_string());
                structures.push(M2Structure::ArtifactReference(id.to_string()));
            }
            _ => {}
        }
    }

    // Structure 2: provider linkage from public metadata only.
    for (req, res) in metadata_pairs {
        structures.push(M2Structure::ProviderLinkage {
            request_id: (*req).to_string(),
            result_id: (*res).to_string(),
        });
    }

    // Structure 4: summary coverage enumeration — every entry must be
    // a canonical EventId inside the referenced universe.
    if !summary_enumeration.is_empty()
        && summary_enumeration
            .iter()
            .all(|e| referenced_events.contains(e))
    {
        structures.push(M2Structure::SummaryCoverage(
            summary_enumeration.iter().map(|e| e.to_string()).collect(),
        ));
    }

    M2Extraction { structures, forged }
}

/// Build the M2-tagged nomination edges for one event from an
/// extraction (A16: every edge records extractor identity, version,
/// and configuration hash). Unverifiable links were already withheld
/// by `m2_extract` (nothing here re-derives anything — D-C-07, A17).
pub fn m2_nominate(
    event: &str,
    extraction: &M2Extraction,
    config: &AttributionConfigV1,
) -> Result<Vec<M0Nomination>, OutcomeError> {
    config.validate_frozen()?;
    let config_hash = config.config_hash()?;
    let mut out = Vec::new();
    for s in &extraction.structures {
        out.push(M0Nomination {
            event: event.to_string(),
            mechanism: AttributionMechanismTag {
                mechanism: Mechanism::M2,
                extractor_version: versions::M2,
                config_hash: config_hash.clone(),
            },
            evidence_kind: s.evidence_kind(),
            evidence_fingerprint: evidence_fingerprint(&s.evidence_bytes()),
        });
    }
    Ok(out)
}
