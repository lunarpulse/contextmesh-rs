//! Option B state-bound handoff validity (gate B5).
//!
//! A handoff binds a B4 delta to the recipient head it was computed against.
//! The handoff is valid only while the recipient's current stated head is
//! still that head, in the same DAG: if the recipient advances, the handoff
//! is stale, is rejected with a typed stale error, and must be re-derived —
//! a stale handoff is never applied. When the head is unchanged the validity
//! check is idempotent, so delivery retries are safe. B4 supplies the
//! recipient-known-history delta; B5 adds the state binding that makes a
//! handoff state-safe. Option A modules are untouched; every store access is
//! read-only.
//!
//! Gate B6 extends the same record with handoff negotiation: every handoff
//! carries an explicit omission list and uncertainty markers, a recipient can
//! challenge a listed omission, and a challenged omission is re-included in a
//! follow-up handoff with the challenge recorded — no omission is hidden.

use serde::Serialize;
use thiserror::Error;

use crate::closure::ClosedSelection;
use crate::delta::Delta;
use crate::delta::{DeltaError, RecipientState, compute_delta};
use crate::error::StoreError;
use crate::model::{ContextId, EventId};
use crate::store::Store;

/// Hard maximum number of listed omissions on one handoff (gate B6).
pub const MAX_OMISSIONS: usize = 4096;
/// Hard maximum number of uncertainty markers on one handoff (gate B6).
pub const MAX_UNCERTAINTY_NOTES: usize = 64;
/// Hard maximum byte size of one uncertainty marker or challenge note (B6).
pub const MAX_NOTE_BYTES: usize = 1024;

/// Stable typed handoff-validity failures (gate B5).
///
/// Option A's error module is untouched; this module owns its fail-closed
/// contract. Variants carry no secret material, so displaying an error cannot
/// disclose caller-controlled or secret data.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HandoffError {
    /// The handoff was computed against a different recipient head than the
    /// recipient's current stated head: the handoff is stale, must be
    /// re-derived, and is never applied.
    #[error(
        "handoff is stale: computed against recipient head {computed:?}, current recipient head is {current:?}"
    )]
    Stale {
        /// The recipient head the handoff was computed against.
        computed: Option<EventId>,
        /// The recipient's current stated head.
        current: Option<EventId>,
    },
    /// A stated recipient head is not a node of the DAG.
    #[error("handoff recipient head is not present in the DAG")]
    UnknownRecipientHead {
        /// The stated recipient head.
        head: EventId,
    },
    /// A stated recipient head lies outside the handoff's context.
    #[error("handoff recipient head is outside the stated context")]
    WrongContext {
        /// The affected event.
        event: EventId,
    },
    /// The handoff record is malformed.
    #[error("handoff record is malformed")]
    InvalidState,
    /// A read-only store operation failed.
    #[error("handoff store operation failed")]
    Store(#[from] StoreError),
    /// A challenged event is not a listed omission.
    #[error("challenged event is not a listed omission")]
    UnknownOmission {
        /// The challenged event.
        event: EventId,
    },
    /// A B4 delta computation failed during handoff negotiation.
    #[error("handoff negotiation delta computation failed")]
    Delta(#[from] DeltaError),
    /// An internal checked invariant failed.
    #[error("handoff internal failure")]
    Internal,
}

/// Why a source was deliberately omitted from the handoff (gate B6).
///
/// The reason is recorded, typed, and deterministic, so the omission list is
/// auditable from the record alone; no omission is hidden.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum OmissionReason {
    /// The source was a candidate but was not selected (rank or budget).
    NotSelected,
    /// The source was deliberately withheld pending recipient challenge.
    Deliberate,
    /// The recipient's stated capabilities do not cover the source (B11
    /// capability-mismatch flags surface here; wired in during OB-11).
    CapabilityMismatch,
}

impl std::fmt::Display for OmissionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSelected => f.write_str("not-selected"),
            Self::Deliberate => f.write_str("deliberate"),
            Self::CapabilityMismatch => f.write_str("capability-mismatch"),
        }
    }
}

/// An explicit omission: a source deliberately withheld, with its reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Omission {
    event: EventId,
    reason: OmissionReason,
}

impl Omission {
    /// Constructs an explicit omission record.
    #[must_use]
    pub const fn new(event: EventId, reason: OmissionReason) -> Self {
        Self { event, reason }
    }

    /// Returns the withheld event.
    #[must_use]
    pub const fn event(&self) -> EventId {
        self.event
    }

    /// Returns the recorded reason the source was withheld.
    #[must_use]
    pub const fn reason(&self) -> OmissionReason {
        self.reason
    }
}

/// A recipient's recorded challenge against a listed omission (gate B6).
///
/// The challenge is the typed entry point of handoff negotiation: it names the
/// omitted event the recipient needs and records the recipient's stated
/// reason, with no secret material.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OmissionChallenge {
    event: EventId,
    note: String,
}

impl OmissionChallenge {
    /// Constructs a challenge record.
    #[must_use]
    pub const fn new(event: EventId, note: String) -> Self {
        Self { event, note }
    }

    /// Returns the challenged event.
    #[must_use]
    pub const fn event(&self) -> EventId {
        self.event
    }

    /// Returns the recipient's stated reason for the challenge.
    #[must_use]
    pub fn note(&self) -> &str {
        &self.note
    }
}

/// A source re-included after a recorded challenge (gate B6).
///
/// The follow-up handoff carries the re-included source in its delta and
/// records the challenge that prompted the re-inclusion here, so the
/// negotiation is auditable from the record alone.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReIncluded {
    event: EventId,
    challenge: OmissionChallenge,
}

impl ReIncluded {
    /// Constructs a re-inclusion record.
    #[must_use]
    pub const fn new(event: EventId, challenge: OmissionChallenge) -> Self {
        Self { event, challenge }
    }

    /// Returns the re-included event (present in the handoff's delta).
    #[must_use]
    pub const fn event(&self) -> EventId {
        self.event
    }

    /// Returns the recorded challenge that prompted the re-inclusion.
    #[must_use]
    pub const fn challenge(&self) -> &OmissionChallenge {
        &self.challenge
    }
}

/// A state-bound handoff: a B4 delta bound to the recipient head it was
/// computed against (gate B5).
///
/// From gate B6 the record also carries an explicit omission list, uncertainty
/// markers, and the re-inclusion history of handoff negotiation.
///
/// The handoff is deliverable only while [`Self::verify_valid`] passes — the
/// recipient's current stated head equals the head the delta was computed
/// against and that head is still a node of the same DAG. Any advance by the
/// recipient makes the handoff stale; a stale handoff is rejected and must be
/// re-derived, never applied. While the head is unchanged, verification is
/// idempotent.
#[derive(Clone, Debug, Serialize)]
pub struct Handoff {
    delta: Delta,
    omissions: Vec<Omission>,
    uncertainty: Vec<String>,
    re_included: Vec<ReIncluded>,
}

impl Handoff {
    /// Binds a B4 delta into a state-bound handoff.
    ///
    /// The recipient head the delta was computed against must be a member of
    /// the recipient closure the delta records — a recipient always knows its
    /// own head — or the record fails closed as malformed.
    ///
    /// The handoff always carries an explicit omission list and uncertainty
    /// markers (empty until recorded), so no omission is ever hidden.
    pub fn from_delta(delta: Delta) -> Result<Self, HandoffError> {
        if let Some(head) = delta.recipient_head()
            && delta.recipient_closure().binary_search(&head).is_err()
        {
            return Err(HandoffError::InvalidState);
        }
        Ok(Self {
            delta,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
            re_included: Vec::new(),
        })
    }

    /// Returns the handoff's context.
    #[must_use]
    pub const fn context(&self) -> ContextId {
        self.delta.context()
    }

    /// Returns the recipient head the handoff was computed against.
    #[must_use]
    pub const fn recipient_head(&self) -> Option<EventId> {
        self.delta.recipient_head()
    }

    /// Returns the underlying B4 delta.
    #[must_use]
    pub const fn delta(&self) -> &Delta {
        &self.delta
    }

    /// Returns the delta event IDs in canonical order.
    #[must_use]
    pub fn events(&self) -> Vec<EventId> {
        self.delta.events()
    }

    /// Returns true when the handoff targets a cold-start recipient.
    #[must_use]
    pub const fn is_cold_start(&self) -> bool {
        self.delta.is_cold_start()
    }

    /// Verifies the handoff is still valid against the recipient's current
    /// stated head in this DAG.
    ///
    /// Both the handoff's stated recipient head and the recipient's current
    /// stated head must be present in the DAG and belong to the handoff's
    /// context (an unknown recipient state fails closed and is never
    /// assumed). The handoff is then valid only when the two heads agree;
    /// when the recipient advanced, the handoff is stale and is rejected.
    /// The check is idempotent: while the current head is unchanged,
    /// re-verification returns the same verdict.
    pub async fn verify_valid(
        &self,
        store: &Store,
        current_head: Option<EventId>,
    ) -> Result<(), HandoffError> {
        let context = self.delta.context();
        if let Some(embedded) = self.delta.recipient_head() {
            match store.event(embedded).await {
                Ok(Some(event)) => {
                    if event.body().context() != context {
                        return Err(HandoffError::WrongContext { event: embedded });
                    }
                }
                Ok(None) => return Err(HandoffError::UnknownRecipientHead { head: embedded }),
                Err(error) => return Err(HandoffError::Store(error)),
            }
        }
        if let Some(current) = current_head {
            match store.event(current).await {
                Ok(Some(event)) => {
                    if event.body().context() != context {
                        return Err(HandoffError::WrongContext { event: current });
                    }
                }
                Ok(None) => return Err(HandoffError::UnknownRecipientHead { head: current }),
                Err(error) => return Err(HandoffError::Store(error)),
            }
        }
        if self.delta.recipient_head() != current_head {
            return Err(HandoffError::Stale {
                computed: self.delta.recipient_head(),
                current: current_head,
            });
        }
        Ok(())
    }

    /// Returns the delta only when the handoff is still valid.
    ///
    /// This is the deliverable gate: a stale handoff is rejected and the
    /// delta is never obtainable, so a stale handoff is never applied.
    pub async fn verified_delta(
        &self,
        store: &Store,
        current_head: Option<EventId>,
    ) -> Result<&Delta, HandoffError> {
        self.verify_valid(store, current_head).await?;
        Ok(&self.delta)
    }

    /// Renders the handoff as canonical RFC 8785/JCS wire bytes.
    pub fn to_wire(&self) -> Result<Vec<u8>, HandoffError> {
        crate::model::canonicalize(self).map_err(|_| HandoffError::Internal)
    }

    /// Records an explicit omission of a source the handoff does not carry.
    ///
    /// The omitted event must not already be in the handoff's delta — listing
    /// a carried source as omitted would hide it, not record a truth — and
    /// the record fails closed as malformed otherwise. The omission list is
    /// kept in canonical order, and listing an already-listed event is
    /// idempotent.
    pub fn with_omission(
        mut self,
        event: EventId,
        reason: OmissionReason,
    ) -> Result<Self, HandoffError> {
        if self.delta.events().binary_search(&event).is_ok() {
            return Err(HandoffError::InvalidState);
        }
        if !self
            .omissions
            .iter()
            .any(|omission| omission.event == event)
        {
            if self.omissions.len() >= MAX_OMISSIONS {
                return Err(HandoffError::InvalidState);
            }
            self.omissions.push(Omission { event, reason });
            self.omissions.sort_by_key(|omission| omission.event);
        }
        Ok(self)
    }

    /// Records an uncertainty marker on the handoff.
    ///
    /// Markers are the explicit uncertainty channel of the handoff: selection
    /// uncertainty notes (for example "no source matches the task") and, from
    /// OB-11 onward, recipient capability-mismatch flags surface here. The
    /// marker must be non-empty and bounded; the list is kept in canonical
    /// order, and recording an already-recorded marker is idempotent.
    pub fn with_uncertainty(mut self, note: impl Into<String>) -> Result<Self, HandoffError> {
        let note = note.into();
        if note.is_empty() || note.len() > MAX_NOTE_BYTES {
            return Err(HandoffError::InvalidState);
        }
        self.uncertainty.push(note);
        self.uncertainty.sort();
        self.uncertainty.dedup();
        if self.uncertainty.len() > MAX_UNCERTAINTY_NOTES {
            return Err(HandoffError::InvalidState);
        }
        Ok(self)
    }

    /// Returns the explicit omission list (present on every handoff).
    #[must_use]
    pub fn omissions(&self) -> &[Omission] {
        &self.omissions
    }

    /// Returns the explicit uncertainty markers.
    #[must_use]
    pub fn uncertainty(&self) -> &[String] {
        &self.uncertainty
    }

    /// Returns the sources re-included after a recorded challenge.
    #[must_use]
    pub fn re_included(&self) -> &[ReIncluded] {
        &self.re_included
    }

    /// A recipient challenges a listed omission (gate B6).
    ///
    /// The challenge is the typed entry point of handoff negotiation. It
    /// fails closed with [`HandoffError::UnknownOmission`] when the event is
    /// not a listed omission, and with [`HandoffError::InvalidState`] when
    /// the note is empty or oversized. The returned challenge is what the
    /// follow-up handoff records.
    pub fn challenge(&self, event: EventId, note: &str) -> Result<OmissionChallenge, HandoffError> {
        if note.is_empty() || note.len() > MAX_NOTE_BYTES {
            return Err(HandoffError::InvalidState);
        }
        if !self
            .omissions
            .iter()
            .any(|omission| omission.event == event)
        {
            return Err(HandoffError::UnknownOmission { event });
        }
        Ok(OmissionChallenge {
            event,
            note: note.to_owned(),
        })
    }

    /// Builds the follow-up handoff that re-includes a challenged omission.
    ///
    /// This is the re-inclusion half of handoff negotiation. The original
    /// handoff must still be valid against the recipient's current head (B5
    /// composes into B6 — a stale handoff is never negotiated), the challenge
    /// must target a listed omission of this handoff, and the re-inclusion
    /// must be real: the supplied closed selection must contain the challenged
    /// source and the recomputed delta must land it in the follow-up handoff,
    /// or the negotiation fails closed. The follow-up handoff carries the
    /// challenge recorded on its re-inclusion list, drops the re-included
    /// omission, carries every other listed omission and uncertainty marker
    /// forward, and leaves this handoff intact.
    pub async fn follow_up(
        &self,
        store: &Store,
        closed: &ClosedSelection,
        recipient: &RecipientState,
        challenge: &OmissionChallenge,
    ) -> Result<Handoff, HandoffError> {
        self.verify_valid(store, recipient.head()).await?;
        if !self
            .omissions
            .iter()
            .any(|omission| omission.event == challenge.event)
        {
            return Err(HandoffError::UnknownOmission {
                event: challenge.event,
            });
        }
        let mut selected: Vec<EventId> = closed
            .references()
            .iter()
            .map(|reference| reference.event())
            .collect();
        selected.sort();
        if selected.binary_search(&challenge.event).is_err() {
            return Err(HandoffError::InvalidState);
        }
        let delta = compute_delta(store, closed, recipient).await?;
        if delta.events().binary_search(&challenge.event).is_err() {
            return Err(HandoffError::InvalidState);
        }

        let delta_events = delta.events();
        let mut follow_up = Handoff::from_delta(delta)?;
        follow_up.omissions = self
            .omissions
            .iter()
            .filter(|omission| delta_events.binary_search(&omission.event).is_err())
            .cloned()
            .collect();
        follow_up.uncertainty = self.uncertainty.clone();
        follow_up.re_included = self.re_included.clone();
        follow_up.re_included.push(ReIncluded {
            event: challenge.event,
            challenge: challenge.clone(),
        });
        Ok(follow_up)
    }
}
