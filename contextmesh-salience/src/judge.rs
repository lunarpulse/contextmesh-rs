//! OC-02 Stage 2F judge boundary and shortlist-bound M3 adapter.
//!
//! This module is deterministic orchestration only. It performs no I/O, reads
//! no clock, and exposes no transcript, payload, path, or model-client input.

use contextmesh::model::{ContextId, EventId};

use crate::attribution::{AttributionConfigV1, NOMINATION_SCORE_PPM, ShortlistV1};
use crate::error::OutcomeError;
use crate::types::{Blake3HashText, MechanismRecordV1, OutcomeId, OutcomeLimits};

/// One attribution computation, identified exactly by outcome and context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttributionSessionKeyV1 {
    /// The verified outcome-ledger identifier.
    pub outcome: OutcomeId,
    /// The verified context identifier.
    pub context: ContextId,
}

/// The only input passed across the Stage 2F judge boundary.
///
/// Fields are private so only this crate's shortlist-bound adapter can create a
/// request. Judge implementations inspect the borrowed session and typed event
/// through accessors.
pub struct AblationRequestV1<'a> {
    session: &'a AttributionSessionKeyV1,
    event: EventId,
}

impl<'a> AblationRequestV1<'a> {
    /// Return the borrowed ledger/context session key.
    #[must_use]
    pub const fn session(&self) -> &'a AttributionSessionKeyV1 {
        self.session
    }

    /// Return the typed shortlisted event being ablated.
    #[must_use]
    pub const fn event(&self) -> EventId {
        self.event
    }
}

/// Exact judge response to a single-event ablation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AblationDeltaV1 {
    /// The judged outcome changed under ablation.
    Changed,
    /// The judged outcome did not change under ablation.
    Unchanged,
}

/// A judge could not complete the requested operation.
///
/// This unit error intentionally carries no caller-controlled or secret text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JudgeUnavailable;

/// Stage 2F judge interface.
///
/// Identity is mandatory, returned explicitly, validated by the adapter, and
/// recorded verbatim. It is never inferred from the trait object's type.
pub trait OutcomeJudge {
    /// Return the exact provenance record to attach to this invocation.
    fn identity(&self) -> MechanismRecordV1;

    /// Judge one shortlist-created single-event ablation request.
    fn ablate(&self, req: AblationRequestV1<'_>) -> Result<AblationDeltaV1, JudgeUnavailable>;

    /// Judge one shortlist-created coalition sampling request.
    fn coalition(
        &self,
        req: CoalitionRequestV1<'_>,
    ) -> Result<CoalitionOutcomeV1, JudgeUnavailable>;
}

/// Exact recorded M3 delta kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M3DeltaKind {
    /// The judge returned [`AblationDeltaV1::Changed`].
    Changed,
    /// The judge returned [`AblationDeltaV1::Unchanged`].
    Unchanged,
    /// The adapter could not obtain a delta.
    Unavailable,
}

impl M3DeltaKind {
    /// Stable Stage 2F spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Changed => "changed",
            Self::Unchanged => "unchanged",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Exact Stage 2F uncertainty markers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M3UncertaintyMarker {
    /// No judge was supplied or the supplied judge became unavailable.
    JudgeUnavailable,
    /// Further requests were withheld at the frozen eight-call cap.
    M3CallCap,
}

impl M3UncertaintyMarker {
    /// Stable marker spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JudgeUnavailable => "judge_unavailable",
            Self::M3CallCap => "m3_call_cap",
        }
    }
}

/// Stage 2F-specific partial adapter status.
///
/// `Complete` means only the M3 adapter completed. It is not the full causal
/// `computed` status, which Stage 2H may emit only after all required adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M3AdapterStatus {
    /// M3 completed every shortlist ablation.
    Complete,
    /// M3 could not complete all requested ablations.
    Unavailable,
    /// The deterministic shortlist was empty.
    NoNominations,
}

impl M3AdapterStatus {
    /// Stable Stage 2F spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Unavailable => "unavailable",
            Self::NoNominations => "no_nominations",
        }
    }
}

/// One shortlist-ordered Stage 2F M3 record.
///
/// Fields are private so only the validated adapter can create records for
/// later Stage 2H consumption.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M3DeltaV1 {
    event: EventId,
    delta_kind: M3DeltaKind,
    judge: String,
    judge_version: String,
    judge_config_hash: Blake3HashText,
}

impl M3DeltaV1 {
    /// Return the typed shortlist event.
    #[must_use]
    pub const fn event(&self) -> EventId {
        self.event
    }

    /// Return the measured or adapter-recorded delta kind.
    #[must_use]
    pub const fn delta_kind(&self) -> M3DeltaKind {
        self.delta_kind
    }

    /// Return the recorded judge identity.
    #[must_use]
    pub fn judge(&self) -> &str {
        &self.judge
    }

    /// Return the recorded judge version.
    #[must_use]
    pub fn judge_version(&self) -> &str {
        &self.judge_version
    }

    /// Return the recorded judge configuration hash.
    #[must_use]
    pub const fn judge_config_hash(&self) -> &Blake3HashText {
        &self.judge_config_hash
    }
}

/// Stage 2F's typed partial M3 adapter section.
///
/// Complete causal-section assembly and canonical serialization are deferred to
/// Stage 2H. Fields are private; external callers receive read-only access to
/// adapter-validated state and cannot forge authoritative-looking combinations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M3AdapterSectionV1 {
    status: M3AdapterStatus,
    m3: Vec<M3DeltaV1>,
    failure: Option<OutcomeError>,
    uncertainty_markers: Vec<M3UncertaintyMarker>,
    judge_calls: usize,
}

impl M3AdapterSectionV1 {
    /// Return the M3-specific partial status.
    #[must_use]
    pub const fn status(&self) -> M3AdapterStatus {
        self.status
    }

    /// Return shortlist-ordered M3 records.
    #[must_use]
    pub fn m3(&self) -> &[M3DeltaV1] {
        &self.m3
    }

    /// Return the stable failure category, when unavailable.
    #[must_use]
    pub const fn failure(&self) -> Option<OutcomeError> {
        self.failure
    }

    /// Return exact typed uncertainty markers.
    #[must_use]
    pub fn uncertainty_markers(&self) -> &[M3UncertaintyMarker] {
        &self.uncertainty_markers
    }

    /// Return calls actually attempted, including one returning unavailable.
    #[must_use]
    pub const fn judge_calls(&self) -> usize {
        self.judge_calls
    }
}

/// Run shortlist-bound M3 ablations under the frozen per-invocation cap.
///
/// The adapter validates the frozen configuration, the complete shortlist, all
/// typed EventIds, and returned judge identity before making any judge call.
/// Every invocation owns a fresh local counter, so distinct outcome/context
/// sessions each receive an independent allowance.
///
/// # Errors
/// Returns the existing [`OutcomeError`] categories for malformed inputs,
/// non-frozen configuration, or invalid judge provenance. Judge unavailability
/// and call-cap exhaustion are successful typed partial sections.
pub fn run_m3(
    session: &AttributionSessionKeyV1,
    shortlist: &ShortlistV1,
    judge: Option<&dyn OutcomeJudge>,
    config: &AttributionConfigV1,
) -> Result<M3AdapterSectionV1, OutcomeError> {
    config.validate_frozen()?;
    shortlist.validate()?;
    let events = shortlist
        .entries
        .iter()
        .map(|entry| {
            entry
                .event
                .parse::<EventId>()
                .map_err(|_| OutcomeError::Malformed)
        })
        .collect::<Result<Vec<_>, _>>()?;

    if events.is_empty() {
        return Ok(M3AdapterSectionV1 {
            status: M3AdapterStatus::NoNominations,
            m3: Vec::new(),
            failure: None,
            uncertainty_markers: Vec::new(),
            judge_calls: 0,
        });
    }

    let Some(judge) = judge else {
        return Ok(M3AdapterSectionV1 {
            status: M3AdapterStatus::Unavailable,
            m3: Vec::new(),
            failure: Some(OutcomeError::MechanismUnavailable),
            uncertainty_markers: vec![M3UncertaintyMarker::JudgeUnavailable],
            judge_calls: 0,
        });
    };

    let identity = judge.identity();
    identity.validate(&OutcomeLimits::default())?;
    let mut m3 = Vec::with_capacity(events.len());
    let mut judge_calls = 0usize;

    for (index, event) in events.iter().copied().enumerate() {
        if judge_calls == config.m3_judge_calls_per_session {
            m3.extend(
                events[index..]
                    .iter()
                    .copied()
                    .map(|remaining| record(remaining, M3DeltaKind::Unavailable, &identity)),
            );
            return Ok(M3AdapterSectionV1 {
                status: M3AdapterStatus::Unavailable,
                m3,
                failure: Some(OutcomeError::MechanismUnavailable),
                uncertainty_markers: vec![M3UncertaintyMarker::M3CallCap],
                judge_calls,
            });
        }

        judge_calls += 1;
        let request = AblationRequestV1 { session, event };
        match judge.ablate(request) {
            Ok(AblationDeltaV1::Changed) => {
                m3.push(record(event, M3DeltaKind::Changed, &identity));
            }
            Ok(AblationDeltaV1::Unchanged) => {
                m3.push(record(event, M3DeltaKind::Unchanged, &identity));
            }
            Err(JudgeUnavailable) => {
                m3.push(record(event, M3DeltaKind::Unavailable, &identity));
                m3.extend(
                    events[index + 1..]
                        .iter()
                        .copied()
                        .map(|remaining| record(remaining, M3DeltaKind::Unavailable, &identity)),
                );
                return Ok(M3AdapterSectionV1 {
                    status: M3AdapterStatus::Unavailable,
                    m3,
                    failure: Some(OutcomeError::MechanismUnavailable),
                    uncertainty_markers: vec![M3UncertaintyMarker::JudgeUnavailable],
                    judge_calls,
                });
            }
        }
    }

    Ok(M3AdapterSectionV1 {
        status: M3AdapterStatus::Complete,
        m3,
        failure: None,
        uncertainty_markers: Vec::new(),
        judge_calls,
    })
}

fn record(event: EventId, delta_kind: M3DeltaKind, identity: &MechanismRecordV1) -> M3DeltaV1 {
    M3DeltaV1 {
        event,
        delta_kind,
        judge: identity.identity.clone(),
        judge_version: identity.version.clone(),
        judge_config_hash: identity.config_hash.clone(),
    }
}

impl M3DeltaV1 {
    /// Public constructor for verification-side transcript assembly
    /// (spec §9.4): a recorded answer plus the judge provenance that was
    /// validated alongside it. Refuses the `Unavailable` marker kind —
    /// transcripts record only answers the judge actually gave.
    pub fn from_transcript_entry(
        event: EventId,
        delta_kind: M3DeltaKind,
        identity: &MechanismRecordV1,
    ) -> Result<Self, OutcomeError> {
        if delta_kind == M3DeltaKind::Unavailable {
            return Err(OutcomeError::Malformed);
        }
        identity.validate(&OutcomeLimits::default())?;
        Ok(Self {
            event,
            delta_kind,
            judge: identity.identity.clone(),
            judge_version: identity.version.clone(),
            judge_config_hash: identity.config_hash.clone(),
        })
    }
}

// ---- Stage 2G: shortlist-bound M4 coalition adapter (spec §7.4.2) ----

/// Exact judge response to one coalition sampling query.
///
/// The judge answers presence/absence only; it never invents a causal claim.
/// The adapter owns all interpretation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoalitionOutcomeV1 {
    /// The target contributed within the judged coalition.
    Contributing,
    /// The target did not contribute within the judged coalition.
    NotContributing,
}

/// The only input passed across the Stage 2G coalition judge boundary.
///
/// Fields are private so only this crate's shortlist-bound adapter can create
/// a request: a borrowed session key, one typed shortlist event, and a subset
/// mask over shortlist positions.
pub struct CoalitionRequestV1<'a> {
    session: &'a AttributionSessionKeyV1,
    target: EventId,
    mask: u32,
}

impl<'a> CoalitionRequestV1<'a> {
    /// Return the borrowed ledger/context session key.
    #[must_use]
    pub const fn session(&self) -> &'a AttributionSessionKeyV1 {
        self.session
    }

    /// Return the typed shortlist event being sampled.
    #[must_use]
    pub const fn target(&self) -> EventId {
        self.target
    }

    /// Return the shortlist-position subset mask.
    #[must_use]
    pub const fn mask(&self) -> u32 {
        self.mask
    }
}

/// Exact Stage 2G uncertainty markers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M4UncertaintyMarker {
    /// No judge was supplied or the supplied judge became unavailable.
    JudgeUnavailable,
    /// Further requests were withheld at the frozen 128-call session cap.
    M4CallCap,
}

impl M4UncertaintyMarker {
    /// Stable marker spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JudgeUnavailable => "judge_unavailable",
            Self::M4CallCap => "m4_call_cap",
        }
    }
}

/// Stage 2G-specific partial adapter status.
///
/// `Complete` means only the M4 adapter completed. It is not the full causal
/// `computed` status, which Stage 2H may emit only after all required adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M4AdapterStatus {
    /// M4 completed every shortlist coalition schedule.
    Complete,
    /// M4 could not complete the full sampling schedule.
    Unavailable,
    /// The deterministic shortlist was empty.
    NoNominations,
}

impl M4AdapterStatus {
    /// Stable Stage 2G spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Unavailable => "unavailable",
            Self::NoNominations => "no_nominations",
        }
    }
}

/// One recorded M4 credit share for a shortlist candidate.
///
/// Fields are private so only the validated adapter can create records for
/// later Stage 2H consumption.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M4ShareV1 {
    event: EventId,
    share_ppm: u128,
    samples: u128,
}

impl M4ShareV1 {
    /// Return the typed shortlist event.
    #[must_use]
    pub const fn event(&self) -> EventId {
        self.event
    }

    /// Return the recorded share in parts per million (at most 1,000,000).
    #[must_use]
    pub const fn share_ppm(&self) -> u128 {
        self.share_ppm
    }

    /// Return the coalition samples actually consumed for this candidate.
    #[must_use]
    pub const fn samples(&self) -> u128 {
        self.samples
    }
}

/// Stage 2G's typed partial M4 adapter section.
///
/// Complete causal-section assembly and canonical serialization are deferred to
/// Stage 2H. Fields are private; external callers receive read-only access to
/// adapter-validated state and cannot forge authoritative-looking combinations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M4AdapterSectionV1 {
    status: M4AdapterStatus,
    m4: Vec<M4ShareV1>,
    failure: Option<OutcomeError>,
    uncertainty_markers: Vec<M4UncertaintyMarker>,
    judge_calls: usize,
}

impl M4AdapterSectionV1 {
    /// Return the M4-specific partial status.
    #[must_use]
    pub const fn status(&self) -> M4AdapterStatus {
        self.status
    }

    /// Return the recorded credit shares in exact shortlist prefix order.
    #[must_use]
    pub fn m4(&self) -> &[M4ShareV1] {
        &self.m4
    }

    /// Return the stable failure category, when unavailable.
    #[must_use]
    pub const fn failure(&self) -> Option<OutcomeError> {
        self.failure
    }

    /// Return exact typed uncertainty markers.
    #[must_use]
    pub fn uncertainty_markers(&self) -> &[M4UncertaintyMarker] {
        &self.uncertainty_markers
    }

    /// Return coalition calls actually attempted, including one returning
    /// unavailable.
    #[must_use]
    pub const fn judge_calls(&self) -> usize {
        self.judge_calls
    }
}

/// Run the shortlist-bound M4 Shapley-style coalition schedule under the
/// frozen per-candidate and per-session caps.
///
/// The schedule is fully deterministic: for each shortlist candidate in
/// shortlist order, subset masks over shortlist positions are visited in
/// ascending `u32` order (skipping the empty mask, keeping only masks whose
/// bits all fit within the shortlist length), up to the frozen per-candidate
/// sample cap. A single local counter bounds total coalition calls per
/// invocation; the call after the frozen session cap is never made.
///
/// # Errors
/// Returns the existing [`OutcomeError`] categories for malformed inputs,
/// non-frozen configuration, invalid judge provenance, or checked-arithmetic
/// overflow. Judge unavailability and call-cap exhaustion are successful typed
/// partial sections.
pub fn run_m4(
    session: &AttributionSessionKeyV1,
    shortlist: &ShortlistV1,
    judge: Option<&dyn OutcomeJudge>,
    config: &AttributionConfigV1,
) -> Result<M4AdapterSectionV1, OutcomeError> {
    config.validate_frozen()?;
    shortlist.validate()?;
    let events = shortlist
        .entries
        .iter()
        .map(|entry| {
            entry
                .event
                .parse::<EventId>()
                .map_err(|_| OutcomeError::Malformed)
        })
        .collect::<Result<Vec<_>, _>>()?;

    if events.is_empty() {
        return Ok(M4AdapterSectionV1 {
            status: M4AdapterStatus::NoNominations,
            m4: Vec::new(),
            failure: None,
            uncertainty_markers: Vec::new(),
            judge_calls: 0,
        });
    }

    let Some(judge) = judge else {
        return Ok(M4AdapterSectionV1 {
            status: M4AdapterStatus::Unavailable,
            m4: Vec::new(),
            failure: Some(OutcomeError::MechanismUnavailable),
            uncertainty_markers: vec![M4UncertaintyMarker::JudgeUnavailable],
            judge_calls: 0,
        });
    };

    // Normalizes completed candidates' raw counts into recorded shares so
    // that any partial section still carries prefix-ordered recorded data
    // (mirroring how M3 keeps completed deltas).
    let normalize = |raw: &[(EventId, u128, u128)]| -> Result<Vec<M4ShareV1>, OutcomeError> {
        let mut total_contributing = 0u128;
        for (_, contributing, _) in raw {
            total_contributing = total_contributing
                .checked_add(*contributing)
                .ok_or(OutcomeError::Malformed)?;
        }
        let mut shares = Vec::with_capacity(raw.len());
        for (event, contributing, consumed) in raw {
            let share_ppm = if total_contributing == 0 {
                0
            } else {
                contributing
                    .checked_mul(u128::from(NOMINATION_SCORE_PPM))
                    .ok_or(OutcomeError::Malformed)?
                    .checked_div(total_contributing)
                    .ok_or(OutcomeError::Malformed)?
            };
            if share_ppm > u128::from(NOMINATION_SCORE_PPM) {
                return Err(OutcomeError::Malformed);
            }
            shares.push(M4ShareV1 {
                event: *event,
                share_ppm,
                samples: *consumed,
            });
        }
        Ok(shares)
    };

    let identity = judge.identity();
    identity.validate(&OutcomeLimits::default())?;
    let len = events.len();
    let mask_limit = 1u128
        .checked_shl(u32::try_from(len).expect("shortlist cap keeps length small"))
        .ok_or(OutcomeError::Malformed)?;
    let samples_cap = u128::try_from(config.m4_samples_per_candidate).expect("frozen cap");
    let session_cap = config.m4_judge_calls_per_session;

    // Deterministic schedule, pass one: for each shortlist candidate (in
    // shortlist order) visit subset masks over shortlist positions in
    // ascending u32 order, skipping the empty mask, keeping only masks whose
    // bits fit within the shortlist length, up to the frozen per-candidate
    // sample cap. Every mask that names the candidate is one judge call.
    // The call after the frozen session cap is never made.
    let mut raw: Vec<(EventId, u128, u128)> = Vec::new();
    let mut judge_calls = 0usize;

    for (index, event) in events.iter().copied().enumerate() {
        let bit = 1u32
            .checked_shl(u32::try_from(index).expect("shortlist cap keeps length small"))
            .ok_or(OutcomeError::Malformed)?;
        let mut contributing = 0u128;
        let mut consumed = 0u128;
        let mut exhausted = false;

        for mask in 1u32.. {
            if u128::from(mask) >= mask_limit {
                break;
            }
            if mask & bit == 0 {
                continue;
            }
            if consumed == samples_cap {
                break;
            }
            if judge_calls == session_cap {
                exhausted = true;
                break;
            }
            judge_calls += 1;
            let request = CoalitionRequestV1 {
                session,
                target: event,
                mask,
            };
            match judge.coalition(request) {
                Ok(CoalitionOutcomeV1::Contributing) => {
                    contributing = contributing.checked_add(1).ok_or(OutcomeError::Malformed)?;
                }
                Ok(CoalitionOutcomeV1::NotContributing) => {}
                Err(JudgeUnavailable) => {
                    return Ok(M4AdapterSectionV1 {
                        status: M4AdapterStatus::Unavailable,
                        m4: normalize(&raw)?,
                        failure: Some(OutcomeError::MechanismUnavailable),
                        uncertainty_markers: vec![M4UncertaintyMarker::JudgeUnavailable],
                        judge_calls,
                    });
                }
            }
            consumed = consumed.checked_add(1).ok_or(OutcomeError::Malformed)?;
        }

        if exhausted {
            return Ok(M4AdapterSectionV1 {
                status: M4AdapterStatus::Unavailable,
                m4: normalize(&raw)?,
                failure: Some(OutcomeError::MechanismUnavailable),
                uncertainty_markers: vec![M4UncertaintyMarker::M4CallCap],
                judge_calls,
            });
        }

        raw.push((event, contributing, consumed));
    }

    // Pass two: normalize recorded credit against the total contributing
    // answers across the section, so recorded shares sum to at most
    // 1,000,000 ppm (unallocated remainder stays implicit recorded data).
    let shares = normalize(&raw)?;

    Ok(M4AdapterSectionV1 {
        status: M4AdapterStatus::Complete,
        m4: shares,
        failure: None,
        uncertainty_markers: Vec::new(),
        judge_calls,
    })
}
