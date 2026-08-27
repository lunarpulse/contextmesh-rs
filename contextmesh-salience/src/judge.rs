//! OC-02 Stage 2F judge boundary and shortlist-bound M3 adapter.
//!
//! This module is deterministic orchestration only. It performs no I/O, reads
//! no clock, and exposes no transcript, payload, path, or model-client input.

use contextmesh::model::{ContextId, EventId};

use crate::attribution::{AttributionConfigV1, ShortlistV1};
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
