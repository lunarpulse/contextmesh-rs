//! Option B recipient capability modeling (gate B11).
//!
//! Model what a recipient can do alongside what it knows, and shape the
//! handoff so an event the recipient cannot act on is flagged, never silently
//! handed off or dropped. The capability model is recorded and versioned per
//! recipient; B4 remains the known-history delta — capability is additive to
//! knowledge. A capability mismatch surfaces through gate B6: a carried event
//! the recipient cannot act on is flagged as an uncertainty marker on the
//! handoff, and a deliberately withheld capability-mismatch source is
//! recorded as an omission with the typed `CapabilityMismatch` reason. The
//! discipline is verified, not assumed: every capability-mismatch flag must
//! name an event the recipient truly cannot act on. Option A modules are
//! untouched; every store access is read-only.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::handoff::{Handoff, HandoffError, OmissionReason};
use crate::model::{AuthorId, EventId};
use crate::store::Store;

/// Hard maximum number of declared capabilities in one model.
pub const MAX_CAPABILITIES: usize = 64;
/// Hard maximum number of kinds one capability covers.
pub const MAX_CAPABILITY_KINDS: usize = 64;
/// Hard maximum byte size of a capability name or covered kind.
pub const MAX_CAPABILITY_TEXT_BYTES: usize = 128;
/// Hard maximum byte size of one capability-mismatch note.
pub const MAX_MISMATCH_NOTE_BYTES: usize = 1024;

/// A declared capability: a name plus the event kinds it covers.
///
/// A recipient with this capability can act on events whose kind is covered;
/// every other kind is a capability mismatch and must be flagged.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Capability {
    name: String,
    kinds: Vec<String>,
}

impl Capability {
    /// Constructs a capability, failing closed on empty or oversized names
    /// and kinds. Kinds are kept in canonical order.
    pub fn new(name: impl Into<String>, kinds: Vec<String>) -> Result<Self, CapabilityError> {
        let name = name.into();
        if name.is_empty() || name.len() > MAX_CAPABILITY_TEXT_BYTES {
            return Err(CapabilityError::InvalidState);
        }
        if kinds.is_empty() || kinds.len() > MAX_CAPABILITY_KINDS {
            return Err(CapabilityError::InvalidState);
        }
        let mut kinds = kinds;
        for kind in &kinds {
            if kind.is_empty() || kind.len() > MAX_CAPABILITY_TEXT_BYTES {
                return Err(CapabilityError::InvalidState);
            }
        }
        kinds.sort();
        kinds.dedup();
        Ok(Self { name, kinds })
    }

    /// Returns the capability name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the covered event kinds in canonical order.
    #[must_use]
    pub fn kinds(&self) -> &[String] {
        &self.kinds
    }

    /// Returns true when this capability covers the stated event kind.
    #[must_use]
    pub fn covers(&self, kind: &str) -> bool {
        self.kinds.binary_search(&kind.to_owned()).is_ok()
    }
}

/// The recorded, versioned capability model of one recipient (gate B11).
///
/// The model is recorded (canonical wire) and versioned per recipient; the
/// recipient is identified by its author identity. A model with no declared
/// capabilities means the recipient can act on nothing and every carried
/// event is a mismatch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecipientCapabilities {
    recipient: AuthorId,
    version: u64,
    capabilities: Vec<Capability>,
}

impl RecipientCapabilities {
    /// Constructs a recorded model, failing closed on duplicate capability
    /// names or an oversized capability set. Capabilities are kept in
    /// canonical name order.
    pub fn new(
        recipient: AuthorId,
        version: u64,
        capabilities: Vec<Capability>,
    ) -> Result<Self, CapabilityError> {
        if capabilities.len() > MAX_CAPABILITIES {
            return Err(CapabilityError::InvalidState);
        }
        let mut capabilities = capabilities;
        capabilities.sort_by(|left, right| left.name.cmp(&right.name));
        for pair in capabilities.windows(2) {
            if pair[0].name == pair[1].name {
                return Err(CapabilityError::InvalidState);
            }
        }
        Ok(Self {
            recipient,
            version,
            capabilities,
        })
    }

    /// Returns the modeled recipient identity.
    #[must_use]
    pub const fn recipient(&self) -> AuthorId {
        self.recipient
    }

    /// Returns the recorded model version.
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Returns the declared capabilities in canonical name order.
    #[must_use]
    pub fn capabilities(&self) -> &[Capability] {
        &self.capabilities
    }

    /// Returns true when any declared capability covers the event kind.
    #[must_use]
    pub fn covers(&self, kind: &str) -> bool {
        self.capabilities
            .iter()
            .any(|capability| capability.covers(kind))
    }

    /// Renders the model as canonical RFC 8785/JCS wire bytes.
    pub fn to_wire(&self) -> Result<Vec<u8>, CapabilityError> {
        crate::model::canonicalize(self).map_err(|_| CapabilityError::Internal)
    }
}

/// One carried event the recipient cannot act on.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityMismatch {
    /// The carried event.
    pub event: EventId,
    /// The event kind the recipient cannot act on.
    pub kind: String,
}

/// The report of shaping a handoff against a capability model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityReport {
    /// The carried events the recipient cannot act on, canonical by event.
    pub mismatches: Vec<CapabilityMismatch>,
    /// Whether any mismatch was flagged in the uncertainty list.
    pub flagged: bool,
}

/// A handoff shaped by the capability model (gate B11).
///
/// Shaping is additive to knowledge: every carried event stays carried, and
/// each event the recipient cannot act on is flagged as an explicit
/// uncertainty marker rather than silently assumed.
#[derive(Debug)]
pub struct ShapedHandoff {
    /// The shaped handoff (original record plus mismatch flags).
    pub handoff: Handoff,
    /// The mismatch report.
    pub report: CapabilityReport,
}

/// Shapes a handoff against the recipient's stated capabilities.
///
/// Every carried event whose kind is not covered is flagged as an explicit
/// uncertainty marker (`capability mismatch: event … kind …`); the event is
/// never silently handed off or dropped. A fully covered handoff is returned
/// unchanged with no flags.
pub fn shape_handoff(
    handoff: &Handoff,
    capabilities: &RecipientCapabilities,
) -> Result<ShapedHandoff, CapabilityError> {
    let mut mismatches = Vec::new();
    let mut shaped = handoff.clone();
    for reference in handoff.delta().references() {
        if !capabilities.covers(reference.kind()) {
            let note = format!(
                "capability mismatch: event {} kind {}",
                reference.event(),
                reference.kind()
            );
            if note.len() > MAX_MISMATCH_NOTE_BYTES {
                return Err(CapabilityError::NoteTooLong);
            }
            shaped = shaped.with_uncertainty(note)?;
            mismatches.push(CapabilityMismatch {
                event: reference.event(),
                kind: reference.kind().to_owned(),
            });
        }
    }
    mismatches.sort_by_key(|mismatch| mismatch.event);
    let flagged = !mismatches.is_empty();
    Ok(ShapedHandoff {
        handoff: shaped,
        report: CapabilityReport {
            mismatches,
            flagged,
        },
    })
}

/// The outcome of verifying the capability discipline on a handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityVerification {
    /// True when every flag is honest and every mismatch is flagged.
    pub valid: bool,
    /// Number of carried events and capability-mismatch omissions checked.
    pub checked: usize,
    /// The carried events the recipient cannot act on.
    pub mismatches: Vec<CapabilityMismatch>,
}

/// Verifies the capability discipline on a handoff (gate B11).
///
/// Every omission with the `CapabilityMismatch` reason must name an event
/// whose kind is genuinely uncovered (a flag naming a covered event is
/// dishonest and rejected), and every carried event whose kind is uncovered
/// must be flagged in the handoff's uncertainty list (a silent mismatch is
/// rejected).
pub async fn verify_handoff(
    store: &Store,
    handoff: &Handoff,
    capabilities: &RecipientCapabilities,
) -> Result<CapabilityVerification, CapabilityError> {
    let mut checked = 0usize;
    let mut mismatches = Vec::new();

    // Capability-mismatch omissions must name genuinely uncovered events.
    for omission in handoff.omissions() {
        if omission.reason() != OmissionReason::CapabilityMismatch {
            continue;
        }
        let stored = store
            .event(omission.event())
            .await?
            .ok_or(CapabilityError::Drifted {
                event: omission.event(),
            })?;
        let kind = stored.body().kind().to_owned();
        checked += 1;
        if capabilities.covers(&kind) {
            return Err(CapabilityError::DishonestFlag {
                event: omission.event(),
                kind,
            });
        }
    }

    // Every carried event the recipient cannot act on must be flagged.
    for reference in handoff.delta().references() {
        checked += 1;
        if !capabilities.covers(reference.kind()) {
            let flagged = handoff.uncertainty().iter().any(|note| {
                note.starts_with("capability mismatch:")
                    && note.contains(&reference.event().to_string())
            });
            if !flagged {
                return Err(CapabilityError::UnflaggedMismatch {
                    event: reference.event(),
                    kind: reference.kind().to_owned(),
                });
            }
            mismatches.push(CapabilityMismatch {
                event: reference.event(),
                kind: reference.kind().to_owned(),
            });
        }
    }
    mismatches.sort_by_key(|mismatch| mismatch.event);
    Ok(CapabilityVerification {
        valid: true,
        checked,
        mismatches,
    })
}

/// Stable typed capability failures (gate B11).
#[derive(Debug, Error)]
pub enum CapabilityError {
    /// A read-only store operation failed.
    #[error("capability store operation failed")]
    Store(#[from] crate::error::StoreError),
    /// The handoff negotiation failed while adding a flag.
    #[error("capability handoff negotiation failed")]
    Handoff(#[from] HandoffError),
    /// The capability model or note is malformed or oversized.
    #[error("capability model or note is invalid")]
    InvalidState,
    /// A mismatch note exceeds the bounded size.
    #[error("capability mismatch note is too long")]
    NoteTooLong,
    /// A capability-mismatch omission names an event missing from the DAG.
    #[error("capability mismatch omission references a drifted event: {event}")]
    Drifted {
        /// The missing referenced event.
        event: EventId,
    },
    /// A capability-mismatch flag names an event the recipient can act on.
    #[error("capability mismatch flag is dishonest: event {event} kind {kind} is covered")]
    DishonestFlag {
        /// The flagged event.
        event: EventId,
        /// The event kind.
        kind: String,
    },
    /// A carried event the recipient cannot act on is not flagged.
    #[error("capability mismatch is unflagged: event {event} kind {kind}")]
    UnflaggedMismatch {
        /// The unflagged carried event.
        event: EventId,
        /// The event kind.
        kind: String,
    },
    /// An internal checked invariant failed.
    #[error("capability internal failure")]
    Internal,
}
