//! Option B recipient-known-history delta (gate B4).
//!
//! Given a recipient's stated known-history head, this module derives the
//! recipient's closure — the head plus every ancestor reachable over Option A
//! parent edges, via a strict read-only store walk — and computes the delta:
//! exactly the closed selected events the recipient does not yet have. The
//! delta is provable: the recipient head must be present in the same DAG and
//! in the stated context (fail closed otherwise, never assumed), and the
//! delta record carries the recipient head, the recipient closure it was
//! computed against, and the selected events already known, so a verifier can
//! re-derive the delta from the store.
//!
//! A cold-start recipient (empty known history) receives the full closed
//! selection as the delta. Recipient *capability* modeling is a separate gate
//! (B11); B4 is strictly a known-history delta. Option A modules are
//! untouched; every store access is read-only.

use std::collections::HashMap;

use serde::Serialize;
use thiserror::Error;

use crate::closure::{ClosedSelection, ClosureLimits};
use crate::error::StoreError;
use crate::model::{ContextId, EventId, canonical_payload_bytes};
use crate::selection::SourceReference;
use crate::store::Store;

/// Hard maximum number of events in one delta.
pub const MAX_DELTA_EVENTS: usize = 100_000;
/// Hard maximum total exported byte size of one delta.
pub const MAX_DELTA_BYTES: usize = 64 * 1024 * 1024;

/// Stable typed delta failures (gate B4).
///
/// Option A's error module is untouched; this module owns its fail-closed
/// contract. Variants carry no secret material, so displaying an error cannot
/// disclose caller-controlled or secret data.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DeltaError {
    /// The stated recipient known-history head is not a node of the DAG.
    #[error("recipient known-history head is not present in the DAG")]
    UnknownRecipientHead {
        /// The stated recipient head.
        head: EventId,
    },
    /// The recipient state and the selection belong to different contexts.
    #[error("recipient state and selected context disagree")]
    ContextMismatch,
    /// The recipient head or a walked event lies outside the stated context.
    #[error("delta event is outside the stated context")]
    WrongContext {
        /// The affected event.
        event: EventId,
    },
    /// The recipient's known history contains a cycle.
    #[error("recipient history contains a cycle")]
    Cycle,
    /// A parent reference in the recipient's history is unresolvable.
    #[error("recipient history contains a dangling parent reference")]
    DanglingParent {
        /// The child event whose parent edge is unresolvable.
        child: EventId,
        /// The referenced parent that is absent.
        parent: EventId,
    },
    /// The delta or recipient closure exceeds its checked bounds.
    #[error("delta limit exceeded")]
    LimitExceeded,
    /// A stored event failed strict verification.
    #[error("delta source is missing or unverifiable")]
    UnverifiableSource {
        /// The affected event.
        event: EventId,
    },
    /// The recipient state record is malformed.
    #[error("recipient state is malformed")]
    InvalidState,
    /// A read-only store operation failed.
    #[error("delta store operation failed")]
    Store(#[from] StoreError),
    /// An internal checked invariant failed.
    #[error("delta internal failure")]
    Internal,
}

/// A recipient's stated known history (gate B4).
///
/// Carries the context, the known-history head (`None` for a cold-start
/// recipient with empty known history), and the derived closure — the head
/// plus every ancestor in the same DAG, in canonical order. The closure is
/// derived and strictly verified against the store by [`Self::at_head`]
/// before it is used, so the delta computed from it is a deterministic
/// function of the DAG and the stated head.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RecipientState {
    context: ContextId,
    head: Option<EventId>,
    closure: Vec<EventId>,
    total_bytes: usize,
}

impl RecipientState {
    /// A cold-start recipient: empty known history.
    #[must_use]
    pub const fn cold_start(context: ContextId) -> Self {
        Self {
            context,
            head: None,
            closure: Vec::new(),
            total_bytes: 0,
        }
    }

    /// Derives and verifies the known-history closure of the given head.
    ///
    /// The head must be present in the store and belong to the stated
    /// context, or the state fails closed with `UnknownRecipientHead` /
    /// `WrongContext` (an unknown recipient state is never assumed). The
    /// ancestry walk is a strict read-only Enter/Exit walk mirroring the
    /// B3 closure walker: every event is reparsed and strictly verified on
    /// read, and a cycle, dangling parent, cross-context edge, or bound
    /// violation fails closed.
    pub async fn at_head(
        store: &Store,
        context: ContextId,
        head: EventId,
        limits: &ClosureLimits,
    ) -> Result<Self, DeltaError> {
        let limits = ClosureLimits::new(limits.max_events, limits.max_exported_bytes)
            .map_err(|_| DeltaError::LimitExceeded)?;
        match store.event(head).await {
            Ok(Some(event)) => {
                if event.body().context() != context {
                    return Err(DeltaError::WrongContext { event: head });
                }
            }
            Ok(None) => return Err(DeltaError::UnknownRecipientHead { head }),
            Err(error) => return Err(DeltaError::Store(error)),
        }

        enum Frame {
            Enter(EventId, Option<EventId>),
            Exit(EventId),
        }
        let mut state: HashMap<EventId, u8> = HashMap::new();
        let mut closure: Vec<EventId> = Vec::new();
        let mut stack: Vec<Frame> = vec![Frame::Enter(head, None)];
        let mut count = 0usize;
        let mut total_bytes = 0usize;
        while let Some(frame) = stack.pop() {
            match frame {
                Frame::Exit(id) => {
                    state.insert(id, 2);
                }
                Frame::Enter(id, child) => match state.get(&id).copied().unwrap_or(0) {
                    2 => continue,
                    1 => return Err(DeltaError::Cycle),
                    _ => {
                        if count >= limits.max_events {
                            return Err(DeltaError::LimitExceeded);
                        }
                        let event = match store.event(id).await {
                            Ok(Some(event)) => event,
                            Ok(None) => {
                                return Err(match child {
                                    Some(referenced_by) => DeltaError::DanglingParent {
                                        child: referenced_by,
                                        parent: id,
                                    },
                                    None => DeltaError::UnverifiableSource { event: id },
                                });
                            }
                            Err(error) => return Err(DeltaError::Store(error)),
                        };
                        if event.body().context() != context {
                            return Err(DeltaError::WrongContext { event: id });
                        }
                        let payload_bytes = canonical_payload_bytes(event.body().payload())
                            .map_err(|_| DeltaError::UnverifiableSource { event: id })?
                            .len();
                        total_bytes = total_bytes
                            .checked_add(payload_bytes)
                            .ok_or(DeltaError::LimitExceeded)?;
                        if total_bytes > limits.max_exported_bytes {
                            return Err(DeltaError::LimitExceeded);
                        }
                        count += 1;
                        state.insert(id, 1);
                        closure.push(id);
                        let parents = event.body().parents().to_vec();
                        stack.push(Frame::Exit(id));
                        for parent in parents.iter().rev() {
                            stack.push(Frame::Enter(*parent, Some(id)));
                        }
                    }
                },
            }
        }
        closure.sort();
        closure.dedup();
        Ok(Self {
            context,
            head: Some(head),
            closure,
            total_bytes,
        })
    }

    /// Constructs a recipient state from an explicitly stated closure.
    ///
    /// The closure is normalized to canonical order. The head, when stated,
    /// must be a member of its own closure (a recipient always knows its
    /// head). Used by adversarial tests; production state is built by
    /// [`Self::at_head`], which derives and verifies the closure against the
    /// store.
    pub fn from_closure(
        context: ContextId,
        head: Option<EventId>,
        mut closure: Vec<EventId>,
        total_bytes: usize,
    ) -> Result<Self, DeltaError> {
        closure.sort();
        closure.dedup();
        if closure.len() > MAX_DELTA_EVENTS {
            return Err(DeltaError::LimitExceeded);
        }
        if let Some(head) = head
            && closure.binary_search(&head).is_err()
        {
            return Err(DeltaError::InvalidState);
        }
        Ok(Self {
            context,
            head,
            closure,
            total_bytes,
        })
    }

    /// Returns the context this known history belongs to.
    #[must_use]
    pub const fn context(&self) -> ContextId {
        self.context
    }

    /// Returns the stated known-history head (`None` for cold-start).
    #[must_use]
    pub const fn head(&self) -> Option<EventId> {
        self.head
    }

    /// Returns the derived known-history closure in canonical order.
    #[must_use]
    pub fn closure(&self) -> &[EventId] {
        &self.closure
    }

    /// Returns the total canonical payload bytes of the known history.
    #[must_use]
    pub const fn total_bytes(&self) -> usize {
        self.total_bytes
    }
}

/// Deterministic partition of a selected set against a recipient closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeltaReport {
    /// The selected events outside the recipient closure, in canonical order.
    pub delta: Vec<EventId>,
    /// The selected events already inside the recipient closure, canonical.
    pub known: Vec<EventId>,
}

/// Partitions a selected event set against a recipient closure.
///
/// Pure and deterministic: inputs are normalized (sorted, deduplicated) and
/// the delta and known sets are returned in canonical EventId order regardless
/// of input order. A cold-start recipient supplies an empty closure, so the
/// delta equals the full selected set.
#[must_use]
pub fn delta_over(selected: &[EventId], closure: &[EventId]) -> DeltaReport {
    let mut selected_ids = selected.to_vec();
    selected_ids.sort();
    selected_ids.dedup();
    let mut closure_ids = closure.to_vec();
    closure_ids.sort();
    closure_ids.dedup();
    let mut delta = Vec::new();
    let mut known = Vec::new();
    for id in selected_ids {
        if closure_ids.binary_search(&id).is_ok() {
            known.push(id);
        } else {
            delta.push(id);
        }
    }
    DeltaReport { delta, known }
}

/// The provable delta: closed selected events outside the recipient closure.
///
/// Carries the recipient head and closure the delta was computed against and
/// the selected events already known, so `delta ∪ known == selected` and
/// `known ⊆ closure` are auditable from the record alone.
#[derive(Clone, Debug, Serialize)]
pub struct Delta {
    context: ContextId,
    recipient_head: Option<EventId>,
    recipient_closure: Vec<EventId>,
    selected_known: Vec<EventId>,
    references: Vec<SourceReference>,
    total_bytes: usize,
    limits: ClosureLimits,
}

impl Delta {
    /// Returns the delta's context.
    #[must_use]
    pub const fn context(&self) -> ContextId {
        self.context
    }

    /// Returns the recipient head the delta was computed against.
    #[must_use]
    pub const fn recipient_head(&self) -> Option<EventId> {
        self.recipient_head
    }

    /// Returns the recipient closure used, in canonical order.
    #[must_use]
    pub fn recipient_closure(&self) -> &[EventId] {
        &self.recipient_closure
    }

    /// Returns the selected events already inside the recipient closure.
    #[must_use]
    pub fn selected_known(&self) -> &[EventId] {
        &self.selected_known
    }

    /// Returns the delta source references in canonical EventId order.
    #[must_use]
    pub fn references(&self) -> &[SourceReference] {
        &self.references
    }

    /// Returns the delta event IDs in canonical order.
    #[must_use]
    pub fn events(&self) -> Vec<EventId> {
        self.references.iter().map(SourceReference::event).collect()
    }

    /// Returns the total exported byte size of the delta references.
    #[must_use]
    pub const fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Returns the limits the selection (and thus the delta) was computed under.
    #[must_use]
    pub const fn limits(&self) -> ClosureLimits {
        self.limits
    }

    /// Returns true when the recipient had empty known history (cold start).
    #[must_use]
    pub const fn is_cold_start(&self) -> bool {
        self.recipient_head.is_none()
    }

    /// Renders the delta as canonical RFC 8785/JCS wire bytes.
    pub fn to_wire(&self) -> Result<Vec<u8>, DeltaError> {
        crate::model::canonicalize(self).map_err(|_| DeltaError::Internal)
    }
}

/// Computes the delta — closed selected events outside the recipient closure.
///
/// Fails closed with `UnknownRecipientHead` when the stated recipient head is
/// not a node of the DAG (unknown recipient state is never assumed), with
/// `WrongContext` when the head belongs to another context, and with
/// `ContextMismatch` when the recipient state and the selection disagree on
/// context. The partition itself is the deterministic pure function
/// [`delta_over`]; the delta record carries the recipient head, the recipient
/// closure used, and the selected events already known, so the delta is
/// provable against the store.
pub async fn compute_delta(
    store: &Store,
    selected: &ClosedSelection,
    recipient: &RecipientState,
) -> Result<Delta, DeltaError> {
    if recipient.context() != selected.context() {
        return Err(DeltaError::ContextMismatch);
    }
    if let Some(head) = recipient.head() {
        match store.event(head).await {
            Ok(Some(event)) => {
                if event.body().context() != selected.context() {
                    return Err(DeltaError::WrongContext { event: head });
                }
            }
            Ok(None) => return Err(DeltaError::UnknownRecipientHead { head }),
            Err(error) => return Err(DeltaError::Store(error)),
        }
    }

    let selected_ids: Vec<EventId> = selected.references().iter().map(|r| r.event()).collect();
    let report = delta_over(&selected_ids, recipient.closure());
    if report.delta.len() > MAX_DELTA_EVENTS {
        return Err(DeltaError::LimitExceeded);
    }

    let mut references = Vec::with_capacity(report.delta.len());
    let mut total_bytes = 0usize;
    for reference in selected.references() {
        if report.delta.binary_search(&reference.event()).is_ok() {
            total_bytes = total_bytes
                .checked_add(reference.payload_bytes())
                .ok_or(DeltaError::LimitExceeded)?;
            references.push(reference.clone());
        }
    }
    if total_bytes > MAX_DELTA_BYTES {
        return Err(DeltaError::LimitExceeded);
    }

    Ok(Delta {
        context: selected.context(),
        recipient_head: recipient.head(),
        recipient_closure: recipient.closure().to_vec(),
        selected_known: report.known,
        references,
        total_bytes,
        limits: selected.limits(),
    })
}
