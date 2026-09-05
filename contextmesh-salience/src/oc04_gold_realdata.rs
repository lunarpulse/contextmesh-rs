//! OC-04 4G-harness-extension: real-data branch (plan v5.2 §7.2, HIGH-2).
//!
//! PREREQUISITE for labeling (§7.2): "the extension ships fixture-backed
//! acceptance tests ... Labeling does NOT start until the extension is
//! committed with all fixtures green."
//!
//! Scope (frozen, §7.2 obligations):
//! - `labels.jsonl` + `bindings.jsonl` parsing (strict schema)
//! - sampling-manifest hash verification (blake3 — the ledger's hash
//!   discipline) — hash mismatch fails closed
//! - label mapping over the FROZEN 5-label scheme (`required`,
//!   `supporting`, `dead_end`, `irrelevant`, `uncertain`); an unresolved
//!   `uncertain` in the gold set fails closed (§4)
//! - family-cluster bootstrap computation (deterministic, seed 20260820,
//!   integer ppm arithmetic only — no floats, no clocks, no network)
//! - judge-call cap accounting and emission (§5.5 prereg caps; retries and
//!   cached calls count — fail-closed, no retry exemption)
//!
//! The real corpus does NOT exist yet: this module is exercised against a
//! committed fixture mini-corpus only (NOT-REAL-DATA discipline).

use serde_json::Value;
use std::collections::BTreeMap;

/// Plan-pinned deterministic bootstrap seed (D-C-10 / §2).
pub const FROZEN_BOOTSTRAP_SEED: u64 = 20260820;

/// The frozen 5-label scheme (§4).
pub const FROZEN_LABELS: [&str; 5] = [
    "required",
    "supporting",
    "dead_end",
    "irrelevant",
    "uncertain",
];

/// Structured reason codes for adjudicated `uncertain` rows (§5.4, v5.2
/// item 7). No free-text task content.
pub const REASON_CODES: [&str; 5] = [
    "ambiguous_task_goal",
    "insufficient_context",
    "conflicting_evidence",
    "redaction_blocks_judgment",
    "codebook_gap",
];

// ---------------------------------------------------------------------------
// Errors (fail-closed: every rejection is an Err with a specific marker)
// ---------------------------------------------------------------------------

/// Corpus load error. `Display` carries the fail-closed marker the
/// acceptance tests assert on.
#[derive(Debug, Clone)]
pub struct CorpusError(pub String);

impl std::fmt::Display for CorpusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for CorpusError {}

fn err(msg: impl Into<String>) -> CorpusError {
    CorpusError(msg.into())
}

// ---------------------------------------------------------------------------
// Manifest hash (blake3 — the repo's hash discipline, integer/hex only)
// ---------------------------------------------------------------------------

/// blake3 digest, lowercase hex (the sampling manifest's committed hash).
pub fn manifest_hash_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

// ---------------------------------------------------------------------------
// Loaded corpus
// ---------------------------------------------------------------------------

/// A validated real-data gold corpus (post `load_corpus`, all checks green).
#[derive(Debug, Clone)]
pub struct GoldCorpus {
    sessions: Vec<String>,
    /// Decisive-label, non-irrelevant sessions, sorted (the gold set; an
    /// `uncertain` reaches it only via recorded adjudication — §4).
    gold_set: Vec<String>,
    families: BTreeMap<String, String>,
    /// Effective (decisive) label per session; adjudicated `uncertain`
    /// rows carry their resolution here.
    judgments: BTreeMap<String, String>,
    /// Adjudication audit trail: session → (original, reason_code,
    /// resolution). Ships with the corpus (§4 audit requirement).
    adjudications: BTreeMap<String, AdjudicationRecord>,
    bindings: BTreeMap<String, SessionBinding>,
}

impl GoldCorpus {
    /// Manifest sessions (sorted), labeled and bound.
    pub fn sessions(&self) -> &[String] {
        &self.sessions
    }

    /// Gold set: labeled, non-`irrelevant`, decisive sessions (sorted).
    pub fn gold_set(&self) -> &[String] {
        &self.gold_set
    }

    /// Session → parent family (family-cluster unit, §3 manifest).
    pub fn families(&self) -> &BTreeMap<String, String> {
        &self.families
    }

    /// Effective (decisive) label per session.
    pub fn judgments(&self) -> &BTreeMap<String, String> {
        &self.judgments
    }

    /// Adjudication audit trail: session → (original judgment,
    /// reason_code, resolution) — ships with the corpus (§4).
    pub fn adjudications(&self) -> &BTreeMap<String, AdjudicationRecord> {
        &self.adjudications
    }

    /// Per-session replay bindings (§7.1 substrate).
    pub fn bindings(&self) -> &BTreeMap<String, SessionBinding> {
        &self.bindings
    }
}

/// Per-session replay binding (the committed replay substrate, §7.1:
/// HMAC session/family IDs, public candidate/shortlist indices, comparator
/// outputs; byte-exact lexical features for strict-TF0 scoring parity).
#[derive(Debug, Clone)]
pub struct SessionBinding {
    /// Keyed session ID (HMAC, §6 privacy boundary).
    pub session_hmac: String,
    /// Keyed parent-family ID (HMAC, §6 privacy boundary).
    pub family_hmac: String,
    /// Public candidate indices (distinct, validated).
    pub candidates: Vec<u64>,
    /// Frozen shortlist32 projection indices (subset of `candidates`).
    pub shortlist32: Vec<u64>,
    /// Redacted byte-exact lexical comparator features (ppm).
    pub lexical_ppm: Vec<u64>,
    /// Redacted prior comparator features (ppm).
    pub prior_ppm: Vec<u64>,
}

// ---------------------------------------------------------------------------
// Judge-call accounting (§5.5 verbatim: per-session caps, fail-closed,
// ALL retries and cached calls count — no retry exemption)
// ---------------------------------------------------------------------------

/// Per-session, per-stage judge-call counts against frozen caps.
#[derive(Debug, Clone)]
pub struct JudgeCallAccounting {
    caps: BTreeMap<String, u64>,
    counts: BTreeMap<(String, String), u64>,
}

impl JudgeCallAccounting {
    /// Caps as `(stage, per_session_cap)` pairs — the prereg §5.5 frozen
    /// caps, e.g. `[("m3", 8), ("m4", 128)]`. (M4 Shapley per-candidate
    /// caps stay prereg-owned and are enforced at the call site.)
    pub fn new(caps: &[(&str, u64)]) -> Self {
        Self {
            caps: caps.iter().map(|(s, c)| ((*s).to_owned(), *c)).collect(),
            counts: BTreeMap::new(),
        }
    }

    /// Record calls. Retries and cached calls MUST be recorded too (§5.5).
    pub fn record(&mut self, session: &str, stage: &str, calls: u64) {
        *self
            .counts
            .entry((session.to_owned(), stage.to_owned()))
            .or_insert(0) += calls;
    }

    /// Per-session total across all stages.
    pub fn session_total(&self, session: &str) -> u64 {
        self.counts
            .iter()
            .filter(|((s, _), _)| s == session)
            .map(|((_, _), c)| *c)
            .sum()
    }

    /// Emit per-session pass/fail against the frozen caps: `false` for any
    /// stage whose count exceeds its cap. The gate harness fails closed on
    /// any `false`.
    pub fn emit_caps(&self) -> BTreeMap<String, bool> {
        let mut sessions: Vec<&str> = self.counts.keys().map(|(s, _)| s.as_str()).collect();
        sessions.sort_unstable();
        sessions.dedup();
        sessions
            .into_iter()
            .map(|session| {
                let ok = self.counts.iter().filter(|((s, _), _)| s == session).all(
                    |((_, stage), calls)| *calls <= self.caps.get(stage).copied().unwrap_or(0),
                );
                (session.to_owned(), ok)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Frozen sampling manifest
// ---------------------------------------------------------------------------

/// Adjudication audit entry: (original judgment, reason_code, resolution).
pub type AdjudicationRecord = (String, String, String);

/// Labeled-judgment result: (effective labels, adjudication audit trail).
pub type ParsedLabels = (
    BTreeMap<String, String>,
    BTreeMap<String, AdjudicationRecord>,
);

/// Frozen sampling manifest (§3): session → HMAC (keyed IDs, §6 privacy
/// boundary), session → parent family (family-cluster unit, §2.2).
#[derive(Debug, Clone)]
pub struct SamplingManifest {
    /// Session → keyed session ID (HMAC, §6).
    pub session_hmacs: BTreeMap<String, String>,
    /// Session → parent family (family-cluster unit, §2.2).
    pub families: BTreeMap<String, String>,
}

impl SamplingManifest {
    fn from_doc(doc: &Value) -> Result<Self, CorpusError> {
        let sessions = doc
            .get("sessions")
            .and_then(Value::as_object)
            .ok_or_else(|| err("manifest schema: missing `sessions` object"))?;
        let families = doc
            .get("families")
            .and_then(Value::as_object)
            .ok_or_else(|| err("manifest schema: missing `families` object"))?;
        let mut session_hmacs = BTreeMap::new();
        let mut fam = BTreeMap::new();
        for (session, hmac) in sessions {
            let hmac = hmac
                .as_str()
                .ok_or_else(|| err("manifest schema: session HMAC not a string"))?;
            session_hmacs.insert(session.clone(), hmac.to_owned());
        }
        for (session, family) in families {
            let family = family
                .as_str()
                .ok_or_else(|| err("manifest schema: family not a string"))?;
            fam.insert(session.clone(), family.to_owned());
        }
        if fam.len() != session_hmacs.len() {
            return Err(err("manifest schema: sessions/families key mismatch"));
        }
        for session in session_hmacs.keys() {
            if !fam.contains_key(session) {
                return Err(err(format!(
                    "manifest schema: session `{session}` has no family"
                )));
            }
        }
        Ok(Self {
            session_hmacs,
            families: fam,
        })
    }

    fn by_hmac(&self) -> BTreeMap<&str, &str> {
        self.session_hmacs
            .iter()
            .map(|(s, h)| (h.as_str(), s.as_str()))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Corpus loading (all §7.2 fail-closed checks, in order)
// ---------------------------------------------------------------------------

/// Load and fully validate a real-data gold corpus.
///
/// Order of fail-closed checks (each rejection carries a specific marker):
/// 1. manifest hash verification (committed sampling manifest digest)
/// 2. manifest schema (sessions/families maps)
/// 3. labels.jsonl schema + frozen 5-label mapping + unresolved-`uncertain`
/// 4. bindings.jsonl schema + unknown-HMAC + duplicate-binding
/// 5. coverage: every manifest session labeled and bound
pub fn load_corpus(
    manifest_bytes: &[u8],
    labels_bytes: &[u8],
    bindings_bytes: &[u8],
    expected_manifest_hash: &str,
) -> Result<GoldCorpus, CorpusError> {
    // 1. Hash FIRST — fail fast on a drifted manifest (OC05-05 discipline).
    let actual = manifest_hash_hex(manifest_bytes);
    if actual != expected_manifest_hash {
        return Err(err(format!(
            "manifest hash mismatch: have={actual} want={expected_manifest_hash}"
        )));
    }
    // 2. Manifest schema.
    let doc: Value =
        serde_json::from_slice(manifest_bytes).map_err(|e| err(format!("manifest schema: {e}")))?;
    let manifest = SamplingManifest::from_doc(&doc)?;

    // 3. Labels.
    let (judgments, adjudications) = parse_labels(labels_bytes)?;

    // 4. Bindings.
    let bindings = parse_bindings(bindings_bytes, &manifest)?;

    // 5. Coverage: every manifest session labeled and bound.
    let mut sessions: Vec<String> = Vec::new();
    for session in manifest.session_hmacs.keys() {
        if !judgments.contains_key(session) {
            return Err(err(format!("session `{session}` has no label")));
        }
        if !bindings.contains_key(session) {
            return Err(err(format!("session `{session}` has no replay binding")));
        }
        sessions.push(session.clone());
    }
    sessions.sort();

    let mut gold_set: Vec<String> = judgments
        .iter()
        .filter(|(_, j)| j.as_str() != "irrelevant")
        .map(|(s, _)| s.clone())
        .collect();
    gold_set.sort();

    Ok(GoldCorpus {
        families: manifest.families,
        bindings,
        adjudications,
        judgments,
        gold_set,
        sessions,
    })
}

/// labels.jsonl: one judgment per session over the frozen 5-label scheme.
/// Every `uncertain` row MUST carry a structured adjudication (reason_code
/// from the frozen set + a decisive resolution) — §4 BLOCKER-6; otherwise
/// the row is an unresolved `uncertain` and fails closed. Returns the
/// effective (decisive) label per session plus the audit trail.
fn parse_labels(labels_bytes: &[u8]) -> Result<ParsedLabels, CorpusError> {
    let text = std::str::from_utf8(labels_bytes).map_err(|e| err(format!("labels utf8: {e}")))?;
    let mut judgments = BTreeMap::new();
    let mut adjudications = BTreeMap::new();
    for (lineno, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let doc: Value = serde_json::from_str(line)
            .map_err(|e| err(format!("labels schema (line {}): {e}", lineno + 1)))?;
        let session = doc
            .get("session")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                err(format!(
                    "labels schema (line {}): missing `session`",
                    lineno + 1
                ))
            })?
            .to_owned();
        let judgment = doc.get("judgment").and_then(Value::as_str).ok_or_else(|| {
            err(format!(
                "labels schema (line {}): missing `judgment`",
                lineno + 1
            ))
        })?;
        if !FROZEN_LABELS.contains(&judgment) {
            return Err(err(format!(
                "unknown label `{judgment}` (frozen scheme: {FROZEN_LABELS:?})"
            )));
        }
        if judgments.contains_key(&session) {
            return Err(err(format!("duplicate label for session `{session}`")));
        }
        if judgment == "uncertain" {
            // §4: uncertain is adjudicated to a decisive label BEFORE
            // scoring; unresolved uncertain fails closed.
            let adj = doc.get("adjudication").ok_or_else(|| {
                err(format!(
                    "unresolved `uncertain` for session `{session}` (no adjudication)"
                ))
            })?;
            let reason = adj
                .get("reason_code")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    err(format!(
                        "unresolved `uncertain` for session `{session}` (no reason_code)"
                    ))
                })?;
            if !REASON_CODES.contains(&reason) {
                return Err(err(format!("unknown reason_code `{reason}`")));
            }
            let resolution = adj
                .get("resolution")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    err(format!(
                        "unresolved `uncertain` for session `{session}` (no resolution)"
                    ))
                })?;
            if resolution == "uncertain" || !FROZEN_LABELS.contains(&resolution) {
                return Err(err(format!(
                    "unresolved `uncertain` for session `{session}` (resolution `{resolution}` not decisive)"
                )));
            }
            adjudications.insert(
                session.clone(),
                (
                    judgment.to_owned(),
                    reason.to_owned(),
                    resolution.to_owned(),
                ),
            );
            judgments.insert(session, resolution.to_owned());
        } else {
            judgments.insert(session, judgment.to_owned());
        }
    }
    Ok((judgments, adjudications))
}

/// bindings.jsonl: one replay binding per session. An unknown session HMAC
/// (not in the frozen manifest) or an index outside the session's candidate
/// set fails closed.
fn parse_bindings(
    bindings_bytes: &[u8],
    manifest: &SamplingManifest,
) -> Result<BTreeMap<String, SessionBinding>, CorpusError> {
    let text =
        std::str::from_utf8(bindings_bytes).map_err(|e| err(format!("bindings utf8: {e}")))?;
    let by_hmac = manifest.by_hmac();
    let mut out = BTreeMap::new();
    for (lineno, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let doc: Value = serde_json::from_str(line)
            .map_err(|e| err(format!("bindings schema (line {}): {e}", lineno + 1)))?;
        let hmac = doc
            .get("session_hmac")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                err(format!(
                    "bindings schema (line {}): missing `session_hmac`",
                    lineno + 1
                ))
            })?;
        let session = match by_hmac.get(hmac) {
            Some(session) => (*session).to_owned(),
            None => {
                return Err(err(format!(
                    "unknown session binding (line {}): session_hmac `{hmac}` not in frozen manifest",
                    lineno + 1
                )));
            }
        };
        if out.contains_key(&session) {
            return Err(err(format!("duplicate session binding for `{session}`")));
        }
        let family_hmac = doc
            .get("family_hmac")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                err(format!(
                    "bindings schema (line {}): missing `family_hmac`",
                    lineno + 1
                ))
            })?
            .to_owned();
        let candidates: Vec<u64> = doc
            .get("candidates")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                err(format!(
                    "bindings schema (line {}): missing `candidates`",
                    lineno + 1
                ))
            })?
            .iter()
            .map(|v| {
                v.as_u64().ok_or_else(|| {
                    err(format!(
                        "bindings schema (line {}): candidate index not u64",
                        lineno + 1
                    ))
                })
            })
            .collect::<Result<_, _>>()?;
        if candidates.is_empty() {
            return Err(err(format!(
                "bindings schema (line {}): empty candidates",
                lineno + 1
            )));
        }
        let mut sorted = candidates.clone();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted.len() != candidates.len() {
            return Err(err(format!(
                "bindings schema (line {}): duplicate candidate index",
                lineno + 1
            )));
        }
        let shortlist32: Vec<u64> = doc
            .get("shortlist32")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                err(format!(
                    "bindings schema (line {}): missing `shortlist32`",
                    lineno + 1
                ))
            })?
            .iter()
            .map(|v| {
                v.as_u64().ok_or_else(|| {
                    err(format!(
                        "bindings schema (line {}): shortlist index not u64",
                        lineno + 1
                    ))
                })
            })
            .collect::<Result<_, _>>()?;
        for idx in &shortlist32 {
            if !candidates.contains(idx) {
                return Err(err(format!(
                    "unknown candidate index {idx} in shortlist32 (line {})",
                    lineno + 1
                )));
            }
        }
        let comparator = doc.get("comparator").ok_or_else(|| {
            err(format!(
                "bindings schema (line {}): missing `comparator`",
                lineno + 1
            ))
        })?;
        let lexical_ppm = ppm_array(comparator, "lexical_ppm", candidates.len(), lineno + 1)?;
        let prior_ppm = ppm_array(comparator, "prior_ppm", candidates.len(), lineno + 1)?;
        out.insert(
            session,
            SessionBinding {
                session_hmac: hmac.to_owned(),
                family_hmac,
                candidates,
                shortlist32,
                lexical_ppm,
                prior_ppm,
            },
        );
    }
    Ok(out)
}

fn ppm_array(
    comparator: &Value,
    key: &str,
    want_len: usize,
    lineno: usize,
) -> Result<Vec<u64>, CorpusError> {
    let arr = comparator
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            err(format!(
                "bindings schema (line {lineno}): comparator missing `{key}`"
            ))
        })?;
    if arr.len() != want_len {
        return Err(err(format!(
            "bindings schema (line {lineno}): comparator `{key}` length {} != candidates {want_len}",
            arr.len()
        )));
    }
    arr.iter()
        .map(|v| {
            v.as_u64().ok_or_else(|| {
                err(format!(
                    "bindings schema (line {lineno}): `{key}` entry not u64"
                ))
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Family-cluster bootstrap (§2: FROZEN — 95% CI, integer ppm, seeded)
// ---------------------------------------------------------------------------

/// Deterministic Lagged-Fibonacci generator (additive LFG with lags 17/5)
/// — integer only, seeded from the plan-pinned seed. Distributional
/// quality is secondary at family counts this small; reproducibility is
/// the requirement (§2.4: assumptions committed and hash-pinned).
pub struct Lfg {
    state: [u64; 17],
    i: usize,
    j: usize,
}

impl Lfg {
    /// Seed the generator (clamped-LCG warmup over the 17-word state).
    pub fn new(seed: u64) -> Self {
        let mut state = [0u64; 17];
        let mut x = seed;
        for slot in state.iter_mut() {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            *slot = x;
        }
        Self { state, i: 0, j: 12 }
    }

    /// Next raw u64 (additive lagged-Fibonacci step, lags 17/5). Named
    /// `next_raw` to avoid confusion with `Iterator::next`.
    pub fn next_raw(&mut self) -> u64 {
        let v = self.state[self.i].wrapping_add(self.state[self.j]);
        self.state[self.i] = v;
        self.i = (self.i + 1) % 17;
        self.j = (self.j + 1) % 17;
        v
    }
}

/// Bootstrap CI over the family-cluster mean of per-session binary
/// relevance scores (ppm), frozen binary mapping: required=1_000_000,
/// supporting=500_000, dead_end/irrelevant=0 (dead_end → non-relevant;
/// D3 dead-end diagnostics are prereg-owned and reported separately).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapCi {
    /// Point estimate: mean of family means (ppm).
    pub point_ppm: i64,
    /// 2.5th-percentile bootstrap replicate mean (ppm).
    pub ci_lo_ppm: i64,
    /// 97.5th-percentile bootstrap replicate mean (ppm).
    pub ci_hi_ppm: i64,
}

/// Family-cluster bootstrap: resample FAMILIES (not sessions) with
/// replacement — the clustered-outcome discipline (§2.2, v5.2 item 1).
/// Deterministic for a pinned seed (acceptance test F7).
pub fn family_cluster_bootstrap(corpus: &GoldCorpus, seed: u64, iters: usize) -> BootstrapCi {
    let mut by_family: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
    for session in &corpus.sessions {
        let score = match corpus.judgments.get(session).map(String::as_str) {
            Some("required") => 1_000_000,
            Some("supporting") => 500_000,
            _ => 0,
        };
        let family = corpus
            .families
            .get(session)
            .map(String::as_str)
            .unwrap_or(session);
        by_family.entry(family).or_default().push(score);
    }
    let fam_means: Vec<i64> = by_family
        .values()
        .map(|scores| scores.iter().sum::<i64>() / scores.len() as i64)
        .collect();
    let f = fam_means.len();
    let point_ppm = fam_means.iter().sum::<i64>() / f.max(1) as i64;
    let mut rng = Lfg::new(seed);
    let mut means = Vec::with_capacity(iters);
    for _ in 0..iters {
        let mut acc: i64 = 0;
        for _ in 0..f {
            let idx = (rng.next_raw() % f as u64) as usize;
            acc += fam_means[idx];
        }
        means.push(acc / f.max(1) as i64);
    }
    means.sort_unstable();
    let lo = means[(iters * 25) / 1000];
    let hi = means[((iters * 975) / 1000).min(iters - 1)];
    BootstrapCi {
        point_ppm,
        ci_lo_ppm: lo,
        ci_hi_ppm: hi,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unit: the digest is stable and hex-shaped (self-check).
    #[test]
    fn manifest_hash_is_stable() {
        let h1 = manifest_hash_hex(b"abc");
        let h2 = manifest_hash_hex(b"abc");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }
}
