//! Store-aware DAG, context, and current-input verification for the signed
//! OutcomeLedgerV1 artifact (OC-01 Stage 2D).
//!
//! This module is read-only toward the store: verification loads admitted
//! events and refs, never mutates storage, and returns a bounded report only
//! on full success. Every failure fails closed with `Err` and no partial
//! report.

use std::collections::BTreeSet;

use contextmesh::model::{ContextId, EventId};
use contextmesh::store::Store;

use crate::error::{OutcomeError, OutcomeOperationError, OutcomeOperationResult};
use crate::outcome::{OutcomeLedgerBodyV1, SignedOutcomeLedgerV1};
use crate::types::{InputRefSnapshotV1, LocalRefEntry, OutcomeLimits, RemoteRefEntry, TerminalV1};

/// Bounded verification report returned only on full success.
///
/// Contains checked event-occurrence, unique-event, local-ref, and
/// remote-ref counts plus the verified snapshot fingerprint. There is no
/// redundant `valid` boolean, findings list, or arbitrary input text: the
/// methods fail closed, so only a fully valid operation returns a report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutcomeVerification {
    event_occurrences: usize,
    unique_events: usize,
    local_refs: usize,
    remote_refs: usize,
    snapshot_fingerprint: String,
}

impl OutcomeVerification {
    /// Returns the total checked EventId-valued body occurrences.
    #[must_use]
    pub fn event_occurrences(&self) -> usize {
        self.event_occurrences
    }

    /// Returns the count of unique events loaded and strictly verified.
    #[must_use]
    pub fn unique_events(&self) -> usize {
        self.unique_events
    }

    /// Returns the count of local refs in the verified snapshot.
    #[must_use]
    pub fn local_refs(&self) -> usize {
        self.local_refs
    }

    /// Returns the count of remote refs in the verified snapshot.
    #[must_use]
    pub fn remote_refs(&self) -> usize {
        self.remote_refs
    }

    /// Returns the verified input-ref snapshot fingerprint text.
    #[must_use]
    pub fn snapshot_fingerprint(&self) -> &str {
        &self.snapshot_fingerprint
    }
}

impl InputRefSnapshotV1 {
    /// Captures the complete canonical local+remote ref snapshot for one
    /// context and computes the exact context-bound fingerprint.
    ///
    /// The capture reads local refs and all-peer remote refs (already in
    /// canonical order from the store), supports an empty snapshot, and never
    /// mutates the store.
    ///
    /// # Errors
    /// Returns [`OutcomeOperationError::Store`] on any store operational
    /// failure and [`OutcomeError::LimitExceeded`] when the captured
    /// snapshot-head count exceeds the caller's event-reference bound.
    pub async fn capture(
        store: &Store,
        context: ContextId,
        limits: OutcomeLimits,
    ) -> OutcomeOperationResult<Self> {
        limits.validate()?;
        let locals = store
            .list_local_refs(context)
            .await
            .map_err(OutcomeOperationError::Store)?;
        let remotes = store
            .list_remote_refs(None, context)
            .await
            .map_err(OutcomeOperationError::Store)?;
        if locals.len() + remotes.len() > limits.max_event_references {
            return Err(OutcomeError::LimitExceeded.into());
        }
        let local: Vec<LocalRefEntry> = locals
            .iter()
            .map(|r| LocalRefEntry {
                name: r.name.as_str().to_owned(),
                head: r.head,
            })
            .collect();
        let remote: Vec<RemoteRefEntry> = remotes
            .iter()
            .map(|r| RemoteRefEntry {
                peer: r.peer.as_str().to_owned(),
                name: r.name.as_str().to_owned(),
                head: r.head,
            })
            .collect();
        InputRefSnapshotV1::new(context, local, remote).map_err(OutcomeOperationError::Artifact)
    }
}

impl SignedOutcomeLedgerV1 {
    /// Verifies the artifact structurally, then against the admitted DAG.
    ///
    /// Every unique referenced EventId is loaded through `Store::event` with
    /// strict stored-wire verification. A missing event returns
    /// `Artifact(MissingEvent)`; a successfully loaded event whose body
    /// context differs returns `Artifact(ContextMismatch)`. Any store
    /// operational failure, including `CorruptStorage` produced for
    /// unverifiable stored wire, returns `Store(e)` without remapping.
    /// Presence in the admitted append-only store is the authorization
    /// evidence for referenced events; no signer allowlist, revocation, or
    /// historical policy is inferred.
    ///
    /// # Errors
    /// Fails closed with [`OutcomeOperationError`]; never returns a partial
    /// report.
    pub async fn verify_against_dag(
        &self,
        store: &Store,
        limits: OutcomeLimits,
    ) -> OutcomeOperationResult<OutcomeVerification> {
        self.verify(limits)?;
        let (occurrences, unique) = verify_references(self.body(), store, limits).await?;
        Ok(report_from(self.body(), occurrences, unique))
    }

    /// Performs full DAG verification, then checks input freshness.
    ///
    /// The current local+remote ref snapshot is captured after DAG
    /// verification. Any observed snapshot difference—name/head/peer addition
    /// or movement, externally observable removal, or fingerprint mismatch—
    /// returns `Artifact(StaleInput)`. Public Store v1 has no removal
    /// transition; executable removal-transition coverage is therefore deferred.
    ///
    /// # Errors
    /// Fails closed; returns no partial report.
    pub async fn verify_current_inputs(
        &self,
        store: &Store,
        limits: OutcomeLimits,
    ) -> OutcomeOperationResult<OutcomeVerification> {
        let verification = self.verify_against_dag(store, limits).await?;
        let fresh = InputRefSnapshotV1::capture(store, self.body().context(), limits).await?;
        if fresh != *self.body().input_refs() {
            return Err(OutcomeError::StaleInput.into());
        }
        Ok(verification)
    }
}

/// Loads every unique referenced event and checks the exact body context.
///
/// Returns `(total occurrences, unique count)`. Reads are deduplicated so
/// the occurrence-bound result is unchanged while every unique event is
/// loaded and strictly verified exactly once.
pub(crate) async fn verify_references(
    body: &OutcomeLedgerBodyV1,
    store: &Store,
    limits: OutcomeLimits,
) -> Result<(usize, usize), OutcomeOperationError> {
    let unique = collect_unique_events(body);
    for event_id in &unique {
        let loaded = store
            .event(*event_id)
            .await
            .map_err(OutcomeOperationError::Store)?;
        match loaded {
            None => return Err(OutcomeError::MissingEvent.into()),
            Some(event) => {
                if event.body().context() != body.context() {
                    return Err(OutcomeError::ContextMismatch.into());
                }
            }
        }
    }
    let occurrences = count_event_occurrences(body, limits)?;
    Ok((occurrences, unique.len()))
}

/// Collects every EventId-valued role into canonical unique order.
fn collect_unique_events(body: &OutcomeLedgerBodyV1) -> BTreeSet<EventId> {
    let mut collection = BTreeSet::new();
    collect_events(body, &mut collection);
    collection
}

fn collect_events(body: &OutcomeLedgerBodyV1, collection: &mut BTreeSet<EventId>) {
    collection.extend(body.input_refs().local.iter().map(|r| r.head));
    collection.extend(body.input_refs().remote.iter().map(|r| r.head));
    if let TerminalV1::Event { event } = body.terminal() {
        collection.insert(*event);
    }
    collection.extend(body.outcome().evidence.iter().copied());
    if let crate::types::QualityV1::Available { evidence, .. } = body.quality() {
        collection.extend(evidence.iter().copied());
    }
    for attempt in body.attempts() {
        collection.extend(attempt.event_refs.iter().copied());
    }
    for dead_end in body.dead_ends() {
        collection.extend(dead_end.event_refs.iter().copied());
    }
    for mark in body.attribution_marks() {
        collection.insert(mark.event);
        collection.extend(mark.evidence.iter().copied());
    }
}

/// Counts every EventId-valued occurrence (duplicates included).
fn count_event_occurrences(
    body: &OutcomeLedgerBodyV1,
    limits: OutcomeLimits,
) -> Result<usize, OutcomeOperationError> {
    let mut total = 0_usize;
    let mut add = |count: usize| -> Result<(), OutcomeOperationError> {
        total = total
            .checked_add(count)
            .ok_or(OutcomeError::LimitExceeded)?;
        if total > limits.max_event_references {
            return Err(OutcomeError::LimitExceeded.into());
        }
        Ok(())
    };
    add(body.input_refs().local.len())?;
    add(body.input_refs().remote.len())?;
    if let TerminalV1::Event { .. } = body.terminal() {
        add(1)?;
    }
    add(body.outcome().evidence.len())?;
    if let crate::types::QualityV1::Available { evidence, .. } = body.quality() {
        add(evidence.len())?;
    }
    for attempt in body.attempts() {
        add(attempt.event_refs.len())?;
    }
    for dead_end in body.dead_ends() {
        add(dead_end.event_refs.len())?;
    }
    for mark in body.attribution_marks() {
        add(1)?;
        add(mark.evidence.len())?;
    }
    Ok(total)
}

/// Computes the bounded report from a fully verified body.
fn report_from(
    body: &OutcomeLedgerBodyV1,
    occurrences: usize,
    unique: usize,
) -> OutcomeVerification {
    OutcomeVerification {
        event_occurrences: occurrences,
        unique_events: unique,
        local_refs: body.input_refs().local.len(),
        remote_refs: body.input_refs().remote.len(),
        snapshot_fingerprint: body.input_refs().fingerprint.to_string(),
    }
}
