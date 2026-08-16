//! Semantic-free OA-03 DAG operations and deterministic ancestry projection.

use std::collections::HashMap;

use serde_json::{Value, json};
use turso::transaction::TransactionBehavior;
use turso::{Connection, params};

use super::*;
use crate::crypto::SigningIdentity;
use crate::store::MAX_BUNDLE_EVENTS;

/// Hard maximum number of events in one ancestry projection.
pub const MAX_PROJECTION_EVENTS: usize = 100_000;
/// Hard maximum sum of canonical event-envelope bytes in one projection.
pub const MAX_PROJECTION_WIRE_BYTES: usize = 64 * 1024 * 1024;
const MAX_PROJECTION_HEADS: usize = 256;

/// Checked resource bounds for deterministic ancestry projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionLimits {
    /// Maximum unique events returned.
    pub max_events: usize,
    /// Maximum sum of canonical event-envelope bytes returned.
    pub max_wire_bytes: usize,
}

impl ProjectionLimits {
    /// Constructs limits no greater than the OA-03 hard bounds.
    pub fn new(max_events: usize, max_wire_bytes: usize) -> StoreResult<Self> {
        if max_events == 0
            || max_events > MAX_PROJECTION_EVENTS
            || max_wire_bytes == 0
            || max_wire_bytes > MAX_PROJECTION_WIRE_BYTES
        {
            return Err(StoreError::ProjectionLimitExceeded);
        }
        Ok(Self {
            max_events,
            max_wire_bytes,
        })
    }
}

impl Default for ProjectionLimits {
    fn default() -> Self {
        Self {
            max_events: MAX_PROJECTION_EVENTS,
            max_wire_bytes: MAX_PROJECTION_WIRE_BYTES,
        }
    }
}

/// Result of atomically creating and activating a local context.
#[derive(Clone, Debug)]
pub struct CreatedContext {
    /// Newly generated opaque context identifier.
    pub context: ContextId,
    /// Signed and admitted zero-parent genesis event.
    pub genesis: SignedEventV1,
    /// Initial local branch pointing at the genesis event.
    pub branch: LocalRef,
}

/// Complete, deterministic, parent-first ancestry selected by explicit heads.
#[derive(Clone, Debug)]
pub struct Projection {
    /// Context shared by all selected events.
    pub context: ContextId,
    /// Canonically sorted unique requested heads.
    pub heads: Vec<EventId>,
    /// Verified unique events in deterministic parent-first order.
    pub events: Vec<SignedEventV1>,
    /// Sum of exact canonical envelope byte lengths.
    pub canonical_wire_bytes: usize,
}

impl Store {
    /// Atomically creates, provisions, activates, and branches a new context.
    pub async fn create_context(
        &self,
        identity: &SigningIdentity,
        branch: LocalRefName,
    ) -> StoreResult<CreatedContext> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| StoreError::EntropyUnavailable)?;
        let context = ContextId::from_bytes(bytes);
        let genesis = identity
            .create_event(context, Vec::new(), "context.genesis", json!({}))
            .map_err(StoreError::Contract)?;
        let event = genesis.clone();
        let author = identity.author();
        let name = branch.clone();
        let wire = genesis.to_wire().map_err(StoreError::Contract)?;
        let id = genesis.event_id();
        self.write(move |tx| Box::pin(async move {
            if context_row(tx, context).await?.is_some() { return Err(StoreError::ContextProvisionMismatch); }
            tx.execute("INSERT INTO contexts(context_id,expected_genesis_id,genesis_event_id,state) VALUES(?1,?2,NULL,0)", params![context.to_bytes().to_vec(), id.to_bytes().to_vec()]).await.map_err(map_db)?;
            tx.execute("INSERT INTO authorized_authors(context_id,author_id) VALUES(?1,?2)", params![context.to_bytes().to_vec(), author.to_bytes().to_vec()]).await.map_err(map_db)?;
            tx.execute("INSERT INTO events(event_id,context_id,author_id,kind,canonical_wire) VALUES(?1,?2,?3,'context.genesis',?4)", params![id.to_bytes().to_vec(), context.to_bytes().to_vec(), author.to_bytes().to_vec(), wire]).await.map_err(map_db)?;
            let changed = tx.execute("UPDATE contexts SET genesis_event_id=?1,state=1 WHERE context_id=?2 AND state=0 AND expected_genesis_id=?1", params![id.to_bytes().to_vec(), context.to_bytes().to_vec()]).await.map_err(map_db)?;
            if changed != 1 { return Err(StoreError::GenesisMismatch); }
            tx.execute("INSERT INTO local_refs(context_id,name,event_id) VALUES(?1,?2,?3)", params![context.to_bytes().to_vec(), name.as_str(), id.to_bytes().to_vec()]).await.map_err(map_db)?;
            Ok(())
        })).await?;
        Ok(CreatedContext {
            context,
            genesis: event,
            branch: LocalRef {
                context,
                name: branch,
                head: id,
            },
        })
    }

    /// Provisions explicit pending local policy without trusting or contacting a peer.
    pub async fn join_context(&self, provision: ContextProvision) -> StoreResult<()> {
        validate_authors(&provision.authorized_authors)?;
        self.write(move |tx| Box::pin(async move {
            if let Some((expected, state)) = context_row(tx, provision.context).await? {
                if state != 0 || expected != provision.expected_genesis || author_rows(tx, provision.context).await? != provision.authorized_authors { return Err(StoreError::ContextProvisionMismatch); }
                return Ok(());
            }
            tx.execute("INSERT INTO contexts(context_id,expected_genesis_id,genesis_event_id,state) VALUES(?1,?2,NULL,0)", params![provision.context.to_bytes().to_vec(), provision.expected_genesis.to_bytes().to_vec()]).await.map_err(map_db)?;
            for author in provision.authorized_authors {
                tx.execute("INSERT INTO authorized_authors(context_id,author_id) VALUES(?1,?2)", params![provision.context.to_bytes().to_vec(), author.to_bytes().to_vec()]).await.map_err(map_db)?;
            }
            Ok(())
        })).await
    }

    /// Signs and atomically appends one single-parent event to an expected branch head.
    pub async fn append(
        &self,
        identity: &SigningIdentity,
        context: ContextId,
        branch: LocalRefName,
        expected: EventId,
        kind: impl Into<String>,
        payload: Value,
    ) -> StoreResult<SignedEventV1> {
        let kind = kind.into();
        if matches!(kind.as_str(), "context.genesis" | "context.merge") {
            return Err(StoreError::ReservedEventKind);
        }
        let event = identity
            .create_event(context, vec![expected], kind, payload)
            .map_err(StoreError::Contract)?;
        self.admit(
            &event,
            RefMutation::CompareAndSwap {
                context,
                name: branch,
                expected: RefExpectation::Head(expected),
                new_head: event.event_id(),
            },
        )
        .await?;
        Ok(event)
    }

    /// Atomically creates an absent local branch at an existing same-context event.
    pub async fn create_branch(
        &self,
        context: ContextId,
        name: LocalRefName,
        from_head: EventId,
    ) -> StoreResult<LocalRef> {
        let result_name = name.clone();
        self.write(move |tx| {
            Box::pin(async move {
                let event_context = authoritative_event_context(tx, from_head)
                    .await?
                    .ok_or(StoreError::ParentMissing(from_head))?;
                if event_context != context {
                    return Err(StoreError::ParentContextMismatch(from_head));
                }
                let current = query_optional_id(
                    tx,
                    "SELECT event_id FROM local_refs WHERE context_id=?1 AND name=?2",
                    params![context.to_bytes().to_vec(), name.as_str()],
                )
                .await?;
                match current {
                    Some(head) if head == from_head => Ok(()),
                    Some(_) => Err(StoreError::RefAlreadyExists),
                    None => {
                        tx.execute(
                            "INSERT INTO local_refs(context_id,name,event_id) VALUES(?1,?2,?3)",
                            params![
                                context.to_bytes().to_vec(),
                                name.as_str(),
                                from_head.to_bytes().to_vec()
                            ],
                        )
                        .await
                        .map_err(map_db)?;
                        Ok(())
                    }
                }
            })
        })
        .await?;
        Ok(LocalRef {
            context,
            name: result_name,
            head: from_head,
        })
    }

    /// Signs an explicit 2-to-64-parent merge and CAS-moves its target branch.
    pub async fn merge(
        &self,
        identity: &SigningIdentity,
        context: ContextId,
        branch: LocalRefName,
        expected: EventId,
        mut parents: Vec<EventId>,
        payload: Value,
    ) -> StoreResult<SignedEventV1> {
        if !(2..=crate::model::MAX_PARENTS).contains(&parents.len()) {
            return Err(StoreError::InvalidMerge);
        }
        parents.sort();
        if parents.windows(2).any(|pair| pair[0] == pair[1])
            || parents.binary_search(&expected).is_err()
        {
            return Err(StoreError::InvalidMerge);
        }
        let event = identity
            .create_event(context, parents, "context.merge", payload)
            .map_err(StoreError::Contract)?;
        self.admit(
            &event,
            RefMutation::CompareAndSwap {
                context,
                name: branch,
                expected: RefExpectation::Head(expected),
                new_head: event.event_id(),
            },
        )
        .await?;
        Ok(event)
    }

    /// Returns deterministic unique parent-first ancestry for explicit immutable heads.
    pub async fn project(
        &self,
        context: ContextId,
        heads: Vec<EventId>,
        limits: ProjectionLimits,
    ) -> StoreResult<Projection> {
        validate_projection_limits(limits)?;
        let heads = normalize_heads(heads)?;
        let mut conn = self.connection().await?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .await
            .map_err(map_db)?;
        match project_on(&tx, context, &heads, limits).await {
            Ok((events, canonical_wire_bytes)) => {
                tx.commit().await.map_err(map_db)?;
                Ok(Projection {
                    context,
                    heads,
                    events,
                    canonical_wire_bytes,
                })
            }
            Err(error) => {
                tx.rollback().await.map_err(map_db)?;
                Err(error)
            }
        }
    }
}

pub(super) fn validate_projection_limits(limits: ProjectionLimits) -> StoreResult<()> {
    ProjectionLimits::new(limits.max_events, limits.max_wire_bytes).map(|_| ())
}

pub(super) fn normalize_heads(mut heads: Vec<EventId>) -> StoreResult<Vec<EventId>> {
    if heads.is_empty() || heads.len() > MAX_PROJECTION_HEADS {
        return Err(StoreError::ProjectionLimitExceeded);
    }
    heads.sort();
    heads.dedup();
    Ok(heads)
}

pub(super) async fn load_verified_event(
    conn: &Connection,
    id: EventId,
) -> StoreResult<SignedEventV1> {
    let wire = event_wire(conn, id)
        .await?
        .ok_or(StoreError::ParentMissing(id))?;
    let event = SignedEventV1::from_wire(&wire).map_err(|_| StoreError::CorruptStorage)?;
    if event.event_id() != id {
        return Err(StoreError::CorruptStorage);
    }
    validate_stored_event(conn, &event).await?;
    Ok(event)
}

pub(super) async fn project_on(
    conn: &Connection,
    context: ContextId,
    heads: &[EventId],
    limits: ProjectionLimits,
) -> StoreResult<(Vec<SignedEventV1>, usize)> {
    validate_projection_limits(limits)?;
    if heads.is_empty() || heads.len() > MAX_BUNDLE_EVENTS {
        return Err(StoreError::ProjectionLimitExceeded);
    }
    #[derive(Clone, Copy)]
    enum Frame {
        Enter(EventId),
        Exit(EventId),
    }
    let mut state: HashMap<EventId, u8> = HashMap::new();
    let mut loaded: HashMap<EventId, SignedEventV1> = HashMap::new();
    let mut stack = Vec::with_capacity(heads.len());
    for head in heads.iter().rev() {
        stack.push(Frame::Enter(*head));
    }
    let mut wire_bytes = 0_usize;
    let mut ordered = Vec::new();
    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Enter(id) => match state.get(&id).copied().unwrap_or(0) {
                2 => continue,
                1 => return Err(StoreError::ProjectionCycle),
                _ => {
                    let event = load_verified_event(conn, id).await?;
                    if event.body().context() != context {
                        return Err(StoreError::ParentContextMismatch(id));
                    }
                    let event_wire_bytes = event
                        .to_wire()
                        .map_err(|_| StoreError::CorruptStorage)?
                        .len();
                    let next_count = loaded
                        .len()
                        .checked_add(ordered.len())
                        .and_then(|v| v.checked_add(1))
                        .ok_or(StoreError::ProjectionLimitExceeded)?;
                    let next_bytes = wire_bytes
                        .checked_add(event_wire_bytes)
                        .ok_or(StoreError::ProjectionLimitExceeded)?;
                    if next_count > limits.max_events || next_bytes > limits.max_wire_bytes {
                        return Err(StoreError::ProjectionLimitExceeded);
                    }
                    wire_bytes = next_bytes;
                    state.insert(id, 1);
                    let parents = event.body().parents().to_vec();
                    loaded.insert(id, event);
                    stack.push(Frame::Exit(id));
                    for parent in parents.iter().rev() {
                        stack.push(Frame::Enter(*parent));
                    }
                }
            },
            Frame::Exit(id) => {
                if state.get(&id) == Some(&1) {
                    state.insert(id, 2);
                    ordered.push(loaded.remove(&id).ok_or(StoreError::CorruptStorage)?);
                }
            }
        }
    }
    Ok((ordered, wire_bytes))
}
