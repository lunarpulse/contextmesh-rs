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

use serde::Serialize;
use thiserror::Error;

use crate::delta::Delta;
use crate::error::StoreError;
use crate::model::{ContextId, EventId};
use crate::store::Store;

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
    /// An internal checked invariant failed.
    #[error("handoff internal failure")]
    Internal,
}

/// A state-bound handoff: a B4 delta bound to the recipient head it was
/// computed against (gate B5).
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
}

impl Handoff {
    /// Binds a B4 delta into a state-bound handoff.
    ///
    /// The recipient head the delta was computed against must be a member of
    /// the recipient closure the delta records — a recipient always knows its
    /// own head — or the record fails closed as malformed.
    pub fn from_delta(delta: Delta) -> Result<Self, HandoffError> {
        if let Some(head) = delta.recipient_head()
            && delta.recipient_closure().binary_search(&head).is_err()
        {
            return Err(HandoffError::InvalidState);
        }
        Ok(Self { delta })
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
}
