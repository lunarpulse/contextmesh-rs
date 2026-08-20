//! Option B dependency closure and critical-risk coverage (gate B3).
//!
//! Given a selected reference set, this module computes the DAG parent closure
//! (every ancestor reachable over Option A parent edges) using read-only store
//! access — mirroring the deterministic Enter/Exit walker of OA-03's
//! projection machinery — and fails closed with a typed error on any dangling
//! parent reference, cross-context edge, cycle, or bound violation. It then
//! covers flagged critical/risk events: any candidate event whose kind matches
//! the critical policy is added to the closed set, never silently dropped.
//!
//! The closure is references, not copies: `ClosedSelection` carries bounded
//! [`SourceReference`]s exactly as the selection layer does, so later gates
//! (B4 delta, B5 handoff) consume a single reference vocabulary. Option A
//! modules are untouched; every store access is read-only.

use std::collections::HashMap;

use serde::Serialize;
use thiserror::Error;

use crate::error::StoreError;
use crate::model::{ContextId, EventId, MAX_KIND_BYTES, canonical_payload_bytes};
use crate::selection::{SourceEvent, SourceReference};
use crate::store::Store;

/// Hard maximum number of events in one dependency closure.
pub const MAX_CLOSURE_EVENTS: usize = 100_000;
/// Hard maximum total exported byte size of one dependency closure.
pub const MAX_CLOSURE_BYTES: usize = 64 * 1024 * 1024;
/// Hard maximum candidate events scanned for critical/risk coverage.
pub const MAX_CLOSURE_CANDIDATES: usize = 65_536;
/// Hard maximum distinct critical/risk event kinds in one policy.
pub const MAX_CRITICAL_KINDS: usize = 64;

/// Stable typed closure failures (gate B3).
///
/// Option A's error module is untouched; this module owns its fail-closed
/// contract. Variants carry no secret material, so displaying an error cannot
/// disclose caller-controlled or secret data.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ClosureError {
    /// A selected or candidate event is missing or unverifiable.
    #[error("closure source is missing or unverifiable")]
    UnverifiableSource {
        /// The affected event.
        event: EventId,
    },
    /// A parent reference cannot be resolved: reject, never silently drop.
    #[error("dangling parent reference")]
    DanglingParent {
        /// The child event whose parent edge is unresolvable.
        child: EventId,
        /// The referenced parent that is absent.
        parent: EventId,
    },
    /// An event or parent lies outside the stated closure context.
    #[error("closure event is outside the stated context")]
    WrongContext {
        /// The affected event.
        event: EventId,
    },
    /// The dependency graph contains a cycle.
    #[error("dependency graph contains a cycle")]
    Cycle,
    /// The closure exceeds its checked bounds.
    #[error("closure limit exceeded")]
    LimitExceeded,
    /// The critical/risk policy is invalid.
    #[error("critical-risk policy is invalid")]
    InvalidPolicy,
    /// A read-only store operation failed.
    #[error("closure store operation failed")]
    Store(#[from] StoreError),
    /// An internal checked invariant failed.
    #[error("closure internal failure")]
    Internal,
}

/// Checked resource bounds for dependency closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ClosureLimits {
    /// Maximum unique events in the closed set.
    pub max_events: usize,
    /// Maximum total exported byte size (sum of canonical payload bytes).
    pub max_exported_bytes: usize,
}

impl ClosureLimits {
    /// Constructs limits no greater than the B3 hard bounds.
    pub fn new(max_events: usize, max_exported_bytes: usize) -> Result<Self, ClosureError> {
        if max_events == 0
            || max_events > MAX_CLOSURE_EVENTS
            || max_exported_bytes == 0
            || max_exported_bytes > MAX_CLOSURE_BYTES
        {
            return Err(ClosureError::LimitExceeded);
        }
        Ok(Self {
            max_events,
            max_exported_bytes,
        })
    }
}

impl Default for ClosureLimits {
    fn default() -> Self {
        Self {
            max_events: MAX_CLOSURE_EVENTS,
            max_exported_bytes: MAX_CLOSURE_BYTES,
        }
    }
}

/// One unresolvable parent edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DanglingEdge {
    /// The child event whose parent edge is unresolvable.
    pub child: EventId,
    /// The referenced parent that is absent.
    pub parent: EventId,
}

/// The read-only view of one DAG node for closure computation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventNode {
    /// The node's own event ID.
    pub event: EventId,
    /// The node's parent event IDs (order normalized by the checker).
    pub parents: Vec<EventId>,
}

/// Deterministic outcome of a pure closure computation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosureReport {
    /// The closed event set in canonical (ascending) EventId order.
    pub closed: Vec<EventId>,
    /// Every unresolvable parent edge, in canonical order.
    pub dangling: Vec<DanglingEdge>,
    /// True when the input graph contains a cycle.
    pub cycle: bool,
    /// True when the event-count bound was exceeded.
    pub exceeded: bool,
}

/// The critical/risk coverage policy: which event kinds are flagged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriticalPolicy {
    kinds: Vec<String>,
}

impl CriticalPolicy {
    /// Constructs a validated critical/risk kind policy.
    ///
    /// Kind strings are normalized to canonical ascending order; duplicates
    /// are collapsed.
    pub fn new(kinds: Vec<String>) -> Result<Self, ClosureError> {
        let mut normalized = kinds;
        if normalized.is_empty() || normalized.len() > MAX_CRITICAL_KINDS {
            return Err(ClosureError::InvalidPolicy);
        }
        for kind in &normalized {
            if kind.is_empty() || kind.len() > MAX_KIND_BYTES {
                return Err(ClosureError::InvalidPolicy);
            }
        }
        normalized.sort();
        normalized.dedup();
        Ok(Self { kinds: normalized })
    }

    /// Returns whether the given event kind is flagged critical/risk.
    #[must_use]
    pub fn is_critical(&self, kind: &str) -> bool {
        self.kinds.iter().any(|flagged| flagged == kind)
    }

    /// Returns the flagged kinds in canonical order.
    #[must_use]
    pub fn kinds(&self) -> &[String] {
        &self.kinds
    }
}

/// Computes the deterministic parent closure over explicit DAG nodes.
///
/// Pure and deterministic: the closed set and the dangling-edge list are
/// returned in canonical order regardless of input order. Parent lists are
/// normalized (sorted, deduplicated) before walking. A cycle is reported in
/// `cycle` rather than failing, so callers can choose how to fail closed.
#[must_use]
pub fn close_over(nodes: &[EventNode], limits: &ClosureLimits) -> ClosureReport {
    enum Frame {
        Enter(EventId, Option<EventId>),
        Exit(EventId),
    }
    let mut by_id: HashMap<EventId, Vec<EventId>> = HashMap::with_capacity(nodes.len());
    for node in nodes {
        let mut parents = node.parents.clone();
        parents.sort();
        parents.dedup();
        by_id.insert(node.event, parents);
    }
    let mut starts: Vec<EventId> = by_id.keys().copied().collect();
    starts.sort();
    let mut state: HashMap<EventId, u8> = HashMap::with_capacity(nodes.len());
    let mut closed: Vec<EventId> = Vec::with_capacity(nodes.len());
    let mut dangling: Vec<DanglingEdge> = Vec::new();
    let mut stack: Vec<Frame> = starts
        .iter()
        .rev()
        .map(|id| Frame::Enter(*id, None))
        .collect();
    let mut count = 0usize;
    let mut cycle = false;
    let mut exceeded = false;
    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Exit(id) => {
                state.insert(id, 2);
            }
            Frame::Enter(id, _child) => match state.get(&id).copied().unwrap_or(0) {
                2 => continue,
                1 => {
                    cycle = true;
                }
                _ => {
                    if count >= limits.max_events {
                        exceeded = true;
                        break;
                    }
                    count += 1;
                    state.insert(id, 1);
                    closed.push(id);
                    let parents = by_id.get(&id).cloned().unwrap_or_default();
                    stack.push(Frame::Exit(id));
                    for parent in parents.iter().rev() {
                        if by_id.contains_key(parent) {
                            stack.push(Frame::Enter(*parent, Some(id)));
                        } else {
                            dangling.push(DanglingEdge {
                                child: id,
                                parent: *parent,
                            });
                        }
                    }
                }
            },
        }
    }
    closed.sort();
    closed.dedup();
    dangling.sort_by_key(|edge| (edge.child.to_string(), edge.parent.to_string()));
    dangling.dedup();
    ClosureReport {
        closed,
        dangling,
        cycle,
        exceeded,
    }
}

/// Checks a node set and fails closed on the first structural violation.
///
/// Returns `Cycle`, `LimitExceeded`, or `DanglingParent` (in that precedence)
/// when the input is not a sound DAG, and the deterministic report otherwise.
pub fn close_check(
    nodes: &[EventNode],
    limits: &ClosureLimits,
) -> Result<ClosureReport, ClosureError> {
    let report = close_over(nodes, limits);
    if report.cycle {
        return Err(ClosureError::Cycle);
    }
    if report.exceeded {
        return Err(ClosureError::LimitExceeded);
    }
    if let Some(edge) = report.dangling.first() {
        return Err(ClosureError::DanglingParent {
            child: edge.child,
            parent: edge.parent,
        });
    }
    Ok(report)
}

/// The complete bounded outcome of closing a selection over DAG parent edges.
#[derive(Clone, Debug, Serialize)]
pub struct ClosedSelection {
    context: ContextId,
    references: Vec<SourceReference>,
    selected: Vec<EventId>,
    added_critical: Vec<EventId>,
    total_bytes: usize,
    limits: ClosureLimits,
}

impl ClosedSelection {
    /// Returns the closure's context.
    #[must_use]
    pub const fn context(&self) -> ContextId {
        self.context
    }

    /// Returns the closed source references in canonical EventId order.
    #[must_use]
    pub fn references(&self) -> &[SourceReference] {
        &self.references
    }

    /// Returns the originally selected event set in canonical order.
    #[must_use]
    pub fn selected(&self) -> &[EventId] {
        &self.selected
    }

    /// Returns the critical/risk events added beyond the parent closure.
    #[must_use]
    pub fn added_critical(&self) -> &[EventId] {
        &self.added_critical
    }

    /// Returns the total exported byte size of the closed references.
    #[must_use]
    pub const fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Returns the limits the closure was computed under.
    #[must_use]
    pub const fn limits(&self) -> ClosureLimits {
        self.limits
    }

    /// Renders the closed selection as canonical RFC 8785/JCS wire bytes.
    pub fn to_wire(&self) -> Result<Vec<u8>, ClosureError> {
        crate::model::canonicalize(self).map_err(|_| ClosureError::Internal)
    }
}

/// Closes a selected event set over DAG parent edges and covers critical events.
///
/// Every selected event and every ancestor must exist in the store, belong to
/// the stated context, and reparse with strict verification, or the closure
/// fails closed (`UnverifiableSource`, `WrongContext`, `DanglingParent`,
/// `Cycle`). Candidate events matching the critical policy are added to the
/// closed set; the added set is reported separately so the coverage decision
/// is never silent.
pub async fn close_selection(
    store: &Store,
    context: ContextId,
    selected: &[EventId],
    candidates: &[EventId],
    policy: &CriticalPolicy,
    limits: &ClosureLimits,
) -> Result<ClosedSelection, ClosureError> {
    let limits = ClosureLimits::new(limits.max_events, limits.max_exported_bytes)?;
    let mut selected_ids = selected.to_vec();
    selected_ids.sort();
    selected_ids.dedup();
    let mut candidate_ids = candidates.to_vec();
    candidate_ids.sort();
    candidate_ids.dedup();
    if candidate_ids.len() > MAX_CLOSURE_CANDIDATES {
        return Err(ClosureError::LimitExceeded);
    }

    let nodes = load_closure_nodes(store, context, &selected_ids, &limits).await?;
    let report = close_over(&nodes, &limits);
    if report.cycle {
        return Err(ClosureError::Cycle);
    }
    if report.exceeded {
        return Err(ClosureError::LimitExceeded);
    }
    if let Some(edge) = report.dangling.first() {
        return Err(ClosureError::DanglingParent {
            child: edge.child,
            parent: edge.parent,
        });
    }

    let mut closed_ids = report.closed;
    let mut added_critical = Vec::new();
    for id in &candidate_ids {
        if closed_ids.binary_search(id).is_ok() {
            continue;
        }
        let event = match store.event(*id).await? {
            Some(event) => event,
            None => return Err(ClosureError::UnverifiableSource { event: *id }),
        };
        if event.body().context() != context {
            return Err(ClosureError::WrongContext { event: *id });
        }
        if policy.is_critical(event.body().kind()) {
            closed_ids.push(*id);
            added_critical.push(*id);
        }
    }
    closed_ids.sort();
    closed_ids.dedup();
    added_critical.sort();
    added_critical.dedup();

    let mut references = Vec::with_capacity(closed_ids.len());
    let mut total_bytes = 0usize;
    for id in &closed_ids {
        let event = match store.event(*id).await? {
            Some(event) => event,
            None => return Err(ClosureError::UnverifiableSource { event: *id }),
        };
        let source = SourceEvent::from_signed(&event)
            .map_err(|_| ClosureError::UnverifiableSource { event: *id })?;
        total_bytes = total_bytes
            .checked_add(source.payload_bytes())
            .ok_or(ClosureError::LimitExceeded)?;
        references.push(SourceReference::from_source(&source));
    }

    Ok(ClosedSelection {
        context,
        references,
        selected: selected_ids,
        added_critical,
        total_bytes,
        limits,
    })
}

/// Loads every closure node (selected events plus all ancestors) read-only.
///
/// Mirrors OA-03's `project_on` Enter/Exit walker: parents are visited in
/// canonical order, in-progress nodes indicate a cycle, and every event is
/// reparsed and strictly verified by the store on read. Fails closed on a
/// missing event (`DanglingParent` when the missing id is a referenced parent,
/// `UnverifiableSource` when it is a selected source), a cross-context edge,
/// or a bound violation.
async fn load_closure_nodes(
    store: &Store,
    context: ContextId,
    selected: &[EventId],
    limits: &ClosureLimits,
) -> Result<Vec<EventNode>, ClosureError> {
    enum Frame {
        Enter(EventId, Option<EventId>),
        Exit(EventId),
    }
    let mut state: HashMap<EventId, u8> = HashMap::new();
    let mut nodes: Vec<EventNode> = Vec::new();
    let mut stack: Vec<Frame> = selected
        .iter()
        .rev()
        .map(|id| Frame::Enter(*id, None))
        .collect();
    let mut count = 0usize;
    let mut total_bytes = 0usize;
    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Exit(id) => {
                state.insert(id, 2);
            }
            Frame::Enter(id, child) => match state.get(&id).copied().unwrap_or(0) {
                2 => continue,
                1 => return Err(ClosureError::Cycle),
                _ => {
                    if count >= limits.max_events {
                        return Err(ClosureError::LimitExceeded);
                    }
                    let event = match store.event(id).await {
                        Ok(Some(event)) => event,
                        Ok(None) => {
                            return Err(match child {
                                Some(referenced_by) => ClosureError::DanglingParent {
                                    child: referenced_by,
                                    parent: id,
                                },
                                None => ClosureError::UnverifiableSource { event: id },
                            });
                        }
                        Err(error) => return Err(ClosureError::Store(error)),
                    };
                    if event.body().context() != context {
                        return Err(ClosureError::WrongContext { event: id });
                    }
                    let payload_bytes = canonical_payload_bytes(event.body().payload())
                        .map_err(|_| ClosureError::UnverifiableSource { event: id })?
                        .len();
                    total_bytes = total_bytes
                        .checked_add(payload_bytes)
                        .ok_or(ClosureError::LimitExceeded)?;
                    if total_bytes > limits.max_exported_bytes {
                        return Err(ClosureError::LimitExceeded);
                    }
                    count += 1;
                    state.insert(id, 1);
                    let parents = event.body().parents().to_vec();
                    nodes.push(EventNode {
                        event: id,
                        parents: parents.clone(),
                    });
                    stack.push(Frame::Exit(id));
                    for parent in parents.iter().rev() {
                        stack.push(Frame::Enter(*parent, Some(id)));
                    }
                }
            },
        }
    }
    Ok(nodes)
}
