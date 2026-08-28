//! OC-02 Stage 2H: `AttributionReportV1` assembly, canonical bytes, and
//! verification (spec §7.5, §8, §9).
//!
//! The report is a strict, exactly-membered canonical JSON artifact. The
//! deterministic tier is rebuilt byte-exact from (ledger, events, config);
//! the adapter tier records judge output verbatim from typed partial
//! sections produced by `run_m3`/`run_m4`. Verification rebuilds the
//! deterministic tier, compares bytes, and never re-queries a judge. No
//! function here performs I/O, reads wall-clock, or embeds model inference.

use contextmesh::model::ContextId;

use crate::attribution::{
    ATTRIBUTION_REPORT_ID_DOMAIN, AttributionConfigV1, CausalStatus, ShortlistV1,
};
use crate::attribution::{build_shortlist, canonical_id_kind};
use crate::attribution::{m0_nominate, m1_nominate, m2_extract, m2_nominate};
use crate::error::OutcomeError as Error;
use crate::judge::{
    AblationDeltaV1, AblationRequestV1, AttributionSessionKeyV1, CoalitionRequestV1,
    JudgeUnavailable, M3AdapterStatus, M3DeltaKind, M3DeltaV1, M4AdapterStatus, OutcomeJudge,
    run_m3, run_m4,
};
use crate::outcome::SignedOutcomeLedgerV1;
use crate::types::OutcomeLimits;

use blake3::Hasher;
use std::collections::BTreeSet;

/// The frozen P1 preregistration SHA-256 reference carried in every report.
const PREREG_REFERENCE: &str = crate::attribution::PREREG_SHA256;

/// Typed prefix for report IDs (spec §5).
pub const REPORT_ID_PREFIX_REPORT: &str = "ocattr1_";

/// The exact top-level members of the §7.5 envelope, in canonical order.
pub const REPORT_MEMBERS: [&str; 10] = [
    "adapter_tier",
    "config_hash",
    "deterministic_tier",
    "input_snapshot_fingerprint",
    "ledger_id",
    "prereg_reference",
    "report_id",
    "task_fingerprint",
    "terminal_status",
    "version",
];

/// Read-only, ledger-scoped event payloads (spec §8).
///
/// Construction binds one context; every request outside that context or the
/// exact pair set fails with `ContextMismatch`/`Malformed`. Payloads are
/// fingerprints' source material only — they never enter report bytes.
pub struct EventSource<'a> {
    context: ContextId,
    pairs: &'a [(String, String)],
}

impl<'a> EventSource<'a> {
    /// Builds a source from (EventId text, payload text) pairs bound to one
    /// context. Duplicate EventIds and non-canonical EventId text are
    /// rejected.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] for duplicates or non-canonical
    /// EventId text.
    pub fn from_pairs(context: ContextId, pairs: &'a [(String, String)]) -> Result<Self, Error> {
        let mut seen = BTreeSet::new();
        for (event, _payload) in pairs {
            if canonical_id_kind(event) != Some("event") || !seen.insert(event.as_str()) {
                return Err(Error::Malformed);
            }
        }
        Ok(Self { context, pairs })
    }

    /// Returns the payload for one canonical EventId within this context.
    ///
    /// # Errors
    /// Returns [`OutcomeError::ContextMismatch`] for an event resolved under
    /// another context and [`OutcomeError::Malformed`] for unknown text.
    pub fn payload(&self, context: ContextId, event: &str) -> Result<&'a str, Error> {
        if context != self.context {
            return Err(Error::ContextMismatch);
        }
        self.pairs
            .iter()
            .find(|(candidate, _)| candidate == event)
            .map(|(_event, payload)| payload.as_str())
            .ok_or(Error::Malformed)
    }
}

/// The assembled §7.4 causal section Stage 2H owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CausalSectionV1 {
    /// The exact causal status of the whole section.
    pub status: CausalStatus,
    /// M3 records in shortlist order (empty unless computed/unavailable).
    pub m3_records: Vec<M3DeltaMirror>,
    /// M4 records in shortlist order (empty unless computed/unavailable).
    pub m4_records: Vec<M4ShareMirror>,
    /// Exact typed uncertainty markers in canonical order.
    pub uncertainty_markers: Vec<String>,
}

/// Serialized M3 delta for the canonical section (§7.4 member order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct M3DeltaMirror {
    /// Shortlisted event text.
    pub event: String,
    /// Exact delta-kind wire spelling.
    pub delta_kind: &'static str,
    /// Recorded judge identity.
    pub judge: String,
    /// Recorded judge version.
    pub judge_version: String,
    /// Recorded judge config hash.
    pub judge_config_hash: String,
}

/// Serialized M4 share for the canonical section (§7.4 member order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct M4ShareMirror {
    /// Shortlisted event text.
    pub event: String,
    /// Credit share in parts per million (≤ 1,000,000).
    pub share_ppm: u128,
    /// Coalition samples consumed for this candidate.
    pub samples: u128,
    /// Recorded judge identity.
    pub judge: String,
    /// Recorded judge version.
    pub judge_version: String,
    /// Recorded judge config hash.
    pub judge_config_hash: String,
}

impl CausalSectionV1 {
    /// Deterministic strict compact JSON with the exact §7.4 members and
    /// lexicographic key order.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] for an internally inconsistent
    /// section (e.g. `computed` with markers, `unavailable` without a
    /// marker).
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, Error> {
        if self.status == CausalStatus::Computed && !self.uncertainty_markers.is_empty() {
            return Err(Error::Malformed);
        }
        if self.status == CausalStatus::Unavailable && self.uncertainty_markers.is_empty() {
            return Err(Error::Malformed);
        }
        let mut json = String::new();
        json.push_str("{\"m3\":[");
        for (index, record) in self.m3_records.iter().enumerate() {
            if index != 0 {
                json.push(',');
            }
            json.push_str("{\"delta_kind\":");
            push_json(&mut json, record.delta_kind);
            json.push_str(",\"event\":");
            push_json(&mut json, &record.event);
            json.push_str(",\"judge\":");
            push_json(&mut json, &record.judge);
            json.push_str(",\"judge_config_hash\":");
            push_json(&mut json, &record.judge_config_hash);
            json.push_str(",\"judge_version\":");
            push_json(&mut json, &record.judge_version);
            json.push('}');
        }
        json.push_str("],\"m4\":[");
        for (index, record) in self.m4_records.iter().enumerate() {
            if index != 0 {
                json.push(',');
            }
            json.push_str("{\"event\":");
            push_json(&mut json, &record.event);
            json.push_str(",\"judge\":");
            push_json(&mut json, &record.judge);
            json.push_str(",\"judge_config_hash\":");
            push_json(&mut json, &record.judge_config_hash);
            json.push_str(",\"judge_version\":");
            push_json(&mut json, &record.judge_version);
            json.push_str(",\"samples\":");
            json.push_str(&record.samples.to_string());
            json.push_str(",\"share_ppm\":");
            json.push_str(&record.share_ppm.to_string());
            json.push('}');
        }
        json.push_str("],\"status\":");
        push_json(&mut json, self.status.as_str());
        json.push_str(",\"uncertainty_markers\":[");
        for (index, marker) in self.uncertainty_markers.iter().enumerate() {
            if index != 0 {
                json.push(',');
            }
            push_json(&mut json, marker);
        }
        json.push_str("]}");
        Ok(json.into_bytes())
    }
}

/// The assembled §7.5 envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributionReportV1 {
    /// Exact report version (frozen 1).
    pub version: u8,
    /// Domain-separated, typed report identifier.
    pub report_id: String,
    /// The verified outcome-ledger identifier text.
    pub ledger_id: String,
    /// BLAKE3 hash text of the ledger's task binding.
    pub task_fingerprint: String,
    /// The ledger's input-ref snapshot fingerprint text.
    pub input_snapshot_fingerprint: String,
    /// The frozen P1 preregistration SHA-256.
    pub prereg_reference: String,
    /// The frozen attribution config hash.
    pub config_hash: String,
    /// Deterministic-tier canonical bytes (a §7.3 shortlist object).
    pub deterministic_tier: Vec<u8>,
    /// Adapter-tier canonical bytes (a §7.4 causal-section object).
    pub adapter_tier: Vec<u8>,
    /// `terminal` or `unterminated`, exactly.
    pub terminal_status: &'static str,
}

impl AttributionReportV1 {
    /// Renders the exact canonical envelope bytes (lexicographic keys) with
    /// `report_id` set to the literal `"report_id"` derivation placeholder
    /// replaced by the real ID in one pass — the ID is derived over bytes
    /// that already contain it, so construction is the only writer.
    ///
    /// # Errors
    /// Returns [`OutcomeError::Malformed`] for an invalid version or tier
    /// mismatch with this report's own state.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, Error> {
        if self.version != 1 {
            return Err(Error::Malformed);
        }
        let mut json = String::new();
        json.push_str("{\"adapter_tier\":");
        json.push_str(std::str::from_utf8(&self.adapter_tier).map_err(|_| Error::Malformed)?);
        json.push_str(",\"config_hash\":");
        push_json(&mut json, &self.config_hash);
        json.push_str(",\"deterministic_tier\":");
        json.push_str(std::str::from_utf8(&self.deterministic_tier).map_err(|_| Error::Malformed)?);
        json.push_str(",\"input_snapshot_fingerprint\":");
        push_json(&mut json, &self.input_snapshot_fingerprint);
        json.push_str(",\"ledger_id\":");
        push_json(&mut json, &self.ledger_id);
        json.push_str(",\"prereg_reference\":");
        push_json(&mut json, &self.prereg_reference);
        json.push_str(",\"report_id\":");
        push_json(&mut json, &self.report_id);
        json.push_str(",\"task_fingerprint\":");
        push_json(&mut json, &self.task_fingerprint);
        json.push_str(",\"terminal_status\":");
        push_json(&mut json, self.terminal_status);
        json.push_str(",\"version\":");
        json.push_str(&self.version.to_string());
        json.push('}');
        Ok(json.into_bytes())
    }
}

/// Computes the full attribution report for one verified ledger (spec §8).
///
/// The ledger is re-verified first (OC-01 discipline); a tampered or
/// unverifiable ledger fails the whole call before any nomination work. The
/// deterministic tier completes whenever ledger verification succeeds. The
/// causal status is `computed` only when the ledger is terminal, the
/// shortlist is non-empty, and both adapters complete; `judge: None`
/// fail-closes the causal tier only.
///
/// # Errors
/// Returns the existing [`OutcomeError`] categories for an unverifiable
/// ledger, malformed event source binding, or non-frozen configuration.
pub async fn compute_attribution(
    ledger: &SignedOutcomeLedgerV1,
    events: &EventSource<'_>,
    config: &AttributionConfigV1,
    judge: Option<&dyn OutcomeJudge>,
) -> Result<AttributionReportV1, Error> {
    ledger.verify(OutcomeLimits::default())?;
    config.validate_frozen()?;
    let context = ledger.body().context();

    let body = ledger.body();
    let evidence_text = evidence_text(ledger, events)?;
    let referenced = referenced_events(ledger);
    let referenced_refs: Vec<&str> = referenced.iter().map(String::as_str).collect();
    for event in &referenced {
        events.payload(context, event)?;
    }

    let mut nominations = Vec::new();
    for event in &referenced {
        let payload = events.payload(context, event)?;
        if let Some(nomination) =
            m0_nominate(event, payload, &evidence_text, &referenced_refs, config)?
        {
            nominations.push(nomination);
        }
        let (normalized, _skipped) =
            m1_nominate(event, payload, &evidence_text, &referenced_refs, config)?;
        if let Some(nomination) = normalized {
            nominations.push(nomination);
        }
        let extraction = m2_extract(payload, &referenced_refs, &[], &[]);
        nominations.extend(m2_nominate(event, &extraction, config)?);
    }

    let shortlist = build_shortlist(&nominations, &referenced_refs, config)?;
    let session = AttributionSessionKeyV1 {
        outcome: ledger.outcome_id(),
        context,
    };

    let unterminated = matches!(
        body.terminal(),
        crate::types::TerminalV1::Unterminated { .. }
    );
    let section = if unterminated || shortlist.entries.is_empty() {
        CausalSectionV1 {
            status: CausalStatus::NoNominations,
            m3_records: Vec::new(),
            m4_records: Vec::new(),
            uncertainty_markers: if unterminated {
                vec!["no_terminal_outcome".to_owned()]
            } else {
                Vec::new()
            },
        }
    } else if judge.is_none() {
        // judge: None fail-closes the causal tier only; the deterministic
        // tier above still completes (spec §8, rows A20/J04).
        CausalSectionV1 {
            status: CausalStatus::Unavailable,
            m3_records: Vec::new(),
            m4_records: Vec::new(),
            uncertainty_markers: vec!["judge_unavailable".to_owned()],
        }
    } else {
        let m3 = run_m3(&session, &shortlist, judge, config)?;
        let m4 = run_m4(&session, &shortlist, judge, config)?;
        // M4 provenance: §9.3 requires every share to carry judge identity.
        // This is only reachable with a judge (None fail-closes above via
        // run_m3's empty section), so the identity is taken directly.
        let m4_identity = judge.ok_or(Error::MechanismUnavailable)?.identity();
        assemble_section(&m3, &m4, &m4_identity)?
    };

    finish_report(ledger, config, &shortlist, &section)
}

/// Verifies a report against its inputs and, when the report carries a
/// judge-computed adapter tier, the recorded judge transcript (spec §9.4).
///
/// Re-verifies the ledger first (R08 ordering), rebuilds the deterministic
/// tier from (ledger, events, config), and compares bytes. For reports
/// computed without a judge (unavailable / no-nominations tiers) the
/// transcript must be empty and no judge is ever queried. For computed
/// reports the recorded transcript replays verbatim through the same
/// deterministic schedule — verification never re-queries a judge.
///
/// # Errors
/// Returns the ledger-verification error first, then
/// [`OutcomeError::IdMismatch`] for a report ID or any tier mismatch, and
/// [`OutcomeError::Malformed`] for a transcript supplied to a report that
/// needs none.
pub async fn verify_report(
    bytes: &[u8],
    ledger: &SignedOutcomeLedgerV1,
    events: &EventSource<'_>,
    config: &AttributionConfigV1,
    transcript: &[M3DeltaV1],
) -> Result<(), Error> {
    ledger.verify(OutcomeLimits::default())?;
    config.validate_frozen()?;

    // Strict, exact-shape, canonical-only parse of the committed bytes:
    // whitespace, reordered keys, or alternate spellings reject outright
    // (X10 non-canonical rejection, OC-01 I26 pattern).
    parse_report_bytes(bytes)?;
    let value = crate::json::parse_strict(bytes)?;
    crate::json::require_exact_keys(&value, &REPORT_MEMBERS)?;
    let object = value.as_object().ok_or(Error::Malformed)?;
    let committed_id = object
        .get("report_id")
        .and_then(serde_json::Value::as_str)
        .ok_or(Error::Malformed)?;

    // Non-computed reports (judge: None) carry no transcript to replay:
    // rebuild with no judge, requiring the committed adapter tier to be the
    // fail-closed section. A transcript supplied here would silently change
    // nothing, so it is rejected outright.
    let value_adapter_tier = object
        .get("adapter_tier")
        .ok_or(Error::Malformed)?
        .get("status")
        .and_then(serde_json::Value::as_str)
        .ok_or(Error::Malformed)?
        .to_owned();
    let expected = if value_adapter_tier != "computed" {
        if !transcript.is_empty() {
            return Err(Error::Malformed);
        }
        compute_attribution(ledger, events, config, None).await?
    } else {
        // Rebuild the full expected report by replaying the recorded
        // transcript verbatim through the same deterministic schedule
        // (spec §9.1/§9.4): the judge is never re-queried — answers come
        // from the recording. Provenance is bound to the transcript: each
        // entry is a full M3DeltaV1 carrying judge/version/config-hash,
        // so replaying with mismatched provenance breaks byte equality.
        let first = transcript.first().ok_or(Error::Malformed)?;
        // All transcript entries must share one judge provenance: divergence
        // is rejected before replay instead of silently changing bytes
        // (Quality re-review W1).
        if transcript.iter().any(|entry| {
            entry.judge() != first.judge()
                || entry.judge_version() != first.judge_version()
                || entry.judge_config_hash() != first.judge_config_hash()
        }) {
            return Err(Error::Malformed);
        }
        let identity = crate::types::MechanismRecordV1::new(
            first.judge().to_owned(),
            first.judge_version().to_owned(),
            first.judge_config_hash().clone(),
            &OutcomeLimits::default(),
        )?;
        let replay = ReplayJudge {
            transcript,
            position: core::cell::Cell::new(0),
            identity_record: identity,
        };
        compute_attribution(ledger, events, config, Some(&replay)).await?
    };
    let expected_bytes = expected.canonical_bytes()?;
    let committed_canonical = parse_report_bytes(bytes)?;
    if committed_canonical != expected_bytes {
        return Err(Error::IdMismatch);
    }
    // Byte equality plus this string equality closes the tamper chain.
    if committed_id != expected.report_id.as_str() {
        return Err(Error::IdMismatch);
    }
    Ok(())
}

/// A transcript-backed judge used only for verification replay. Never makes
/// network calls; ablation answers come verbatim from the recording.
/// Coalition answers replay from the same transcript tail after the M3
/// prefix is consumed (both adapters share one recorded answer sequence in
/// shortlist-schedule order).
struct ReplayJudge<'a> {
    transcript: &'a [M3DeltaV1],
    position: core::cell::Cell<usize>,
    identity_record: crate::types::MechanismRecordV1,
}

impl OutcomeJudge for ReplayJudge<'_> {
    fn identity(&self) -> crate::types::MechanismRecordV1 {
        // Provenance is bound to the transcript: verify_report seeds this
        // record from the first recorded M3 delta, so replaying a
        // transcript with mismatched provenance fails byte equality. Entry
        // provenance consistency is checked up front in verify_report.
        self.identity_record.clone()
    }
    fn ablate(&self, _req: AblationRequestV1<'_>) -> Result<AblationDeltaV1, JudgeUnavailable> {
        let index = self.position.get();
        match self.transcript.get(index) {
            Some(delta) => {
                self.position.set(index + 1);
                Ok(match delta.delta_kind() {
                    M3DeltaKind::Changed => AblationDeltaV1::Changed,
                    M3DeltaKind::Unchanged => AblationDeltaV1::Unchanged,
                    M3DeltaKind::Unavailable => return Err(JudgeUnavailable),
                })
            }
            None => Err(JudgeUnavailable),
        }
    }

    fn coalition(
        &self,
        _req: CoalitionRequestV1<'_>,
    ) -> Result<crate::judge::CoalitionOutcomeV1, JudgeUnavailable> {
        let index = self.position.get();
        match self.transcript.get(index) {
            Some(delta) => {
                self.position.set(index + 1);
                match delta.delta_kind() {
                    M3DeltaKind::Changed => Ok(crate::judge::CoalitionOutcomeV1::Contributing),
                    M3DeltaKind::Unchanged => Ok(crate::judge::CoalitionOutcomeV1::NotContributing),
                    M3DeltaKind::Unavailable => Err(JudgeUnavailable),
                }
            }
            None => Err(JudgeUnavailable),
        }
    }
}

/// Parses strict report bytes back into the canonical envelope, rejecting any
/// member deviation before comparison.
fn parse_report_bytes(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let value = crate::json::parse_strict(bytes)?;
    crate::json::require_exact_keys(&value, &REPORT_MEMBERS)?;
    let canonical = crate::json::jcs(&value)?;
    // Non-canonical bytes (whitespace, reordered keys, numeric spellings)
    // must be rejected outright — only exact canonical bytes verify.
    if canonical != bytes {
        return Err(Error::Noncanonical);
    }
    Ok(canonical)
}

/// Derives the domain-separated typed report ID (spec §9.2).
fn derive_report_id(canonical: &[u8]) -> Result<String, Error> {
    let mut hasher = Hasher::new();
    hasher.update(ATTRIBUTION_REPORT_ID_DOMAIN);
    hasher.update(canonical);
    let hex = hasher.finalize().to_hex().to_string();
    Ok(format!("{}{}", crate::attribution::REPORT_ID_PREFIX, hex))
}

/// Assembles the terminal-ledger causal section from typed partials.
fn assemble_section(
    m3: &crate::judge::M3AdapterSectionV1,
    m4: &crate::judge::M4AdapterSectionV1,
    m4_identity: &crate::types::MechanismRecordV1,
) -> Result<CausalSectionV1, Error> {
    let mut markers: Vec<String> = Vec::new();
    for marker in m3.uncertainty_markers() {
        push_unique_marker(&mut markers, marker.as_str());
    }
    for marker in m4.uncertainty_markers() {
        push_unique_marker(&mut markers, marker.as_str());
    }

    let mut m3_records = Vec::new();
    for delta in m3.m3() {
        m3_records.push(M3DeltaMirror {
            event: delta.event().to_string(),
            delta_kind: delta.delta_kind().as_str(),
            judge: delta.judge().to_owned(),
            judge_version: delta.judge_version().to_owned(),
            judge_config_hash: delta.judge_config_hash().as_str().to_owned(),
        });
    }
    let mut m4_records = Vec::new();
    for share in m4.m4() {
        m4_records.push(M4ShareMirror {
            event: share.event().to_string(),
            share_ppm: share.share_ppm(),
            samples: share.samples(),
            judge: m4_identity.identity.clone(),
            judge_version: m4_identity.version.clone(),
            judge_config_hash: m4_identity.config_hash.as_str().to_owned(),
        });
    }

    let status =
        if m3.status() == M3AdapterStatus::Complete && m4.status() == M4AdapterStatus::Complete {
            CausalStatus::Computed
        } else if m3.status() == M3AdapterStatus::NoNominations
            || m4.status() == M4AdapterStatus::NoNominations
        {
            CausalStatus::NoNominations
        } else {
            CausalStatus::Unavailable
        };
    Ok(CausalSectionV1 {
        status,
        m3_records,
        m4_records,
        uncertainty_markers: markers,
    })
}

fn push_unique_marker(markers: &mut Vec<String>, marker: &str) {
    if !markers.iter().any(|existing| existing == marker) {
        markers.push(marker.to_owned());
    }
}

/// Collects the ledger-referenced event texts in canonical ascending order.
fn referenced_events(ledger: &SignedOutcomeLedgerV1) -> Vec<String> {
    let body = ledger.body();
    let mut set = BTreeSet::new();
    for event in &body.outcome().evidence {
        set.insert(event.to_string());
    }
    if let crate::types::QualityV1::Available { evidence, .. } = body.quality() {
        for event in evidence {
            set.insert(event.to_string());
        }
    }
    for attempt in body.attempts() {
        for event in &attempt.event_refs {
            set.insert(event.to_string());
        }
    }
    for dead_end in body.dead_ends() {
        for event in &dead_end.event_refs {
            set.insert(event.to_string());
        }
    }
    set.into_iter().collect()
}

/// Builds the deterministic evidence text: outcome-evidence payloads joined
/// in canonical order. Payloads are event text, never transcript bytes.
fn evidence_text(
    ledger: &SignedOutcomeLedgerV1,
    events: &EventSource<'_>,
) -> Result<String, Error> {
    let context = ledger.body().context();
    let mut parts = Vec::new();
    for event in &ledger.body().outcome().evidence {
        let text = event.to_string();
        parts.push(events.payload(context, &text)?.to_owned());
    }
    Ok(parts.join(" "))
}

/// Freezes the envelope together with its derived report ID.
fn finish_report(
    ledger: &SignedOutcomeLedgerV1,
    config: &AttributionConfigV1,
    shortlist: &ShortlistV1,
    section: &CausalSectionV1,
) -> Result<AttributionReportV1, Error> {
    let body = ledger.body();
    let deterministic = shortlist.canonical_bytes()?;
    let adapter = section.canonical_bytes()?;
    let terminal_status = match body.terminal() {
        crate::types::TerminalV1::Event { .. } => "terminal",
        crate::types::TerminalV1::Unterminated { .. } => "unterminated",
    };
    let mut report = AttributionReportV1 {
        version: 1,
        report_id: String::new(),
        ledger_id: ledger.outcome_id().to_string(),
        task_fingerprint: body.task().content_hash.as_str().to_owned(),
        input_snapshot_fingerprint: body.input_refs().fingerprint.to_string(),
        prereg_reference: PREREG_REFERENCE.to_owned(),
        config_hash: config.config_hash()?,
        deterministic_tier: deterministic,
        adapter_tier: adapter,
        terminal_status,
    };
    let bytes_without_id = placeholder_bytes(&report)?;
    let report_id = derive_report_id(&bytes_without_id)?;
    report.report_id = report_id;
    Ok(report)
}

/// Renders envelope bytes with the report_id member set to a fixed
/// placeholder so the ID derivation is total and tamper-evident.
fn placeholder_bytes(report: &AttributionReportV1) -> Result<Vec<u8>, Error> {
    let mut sealed = report.clone();
    sealed.report_id = "report_id".to_owned();
    sealed.canonical_bytes()
}

/// Append one JSON string with RFC 8259 escaping and no unnecessary spaces.
fn push_json(output: &mut String, value: &str) {
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
