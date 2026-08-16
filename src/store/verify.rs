//! Bounded read-snapshot full-store integrity verification.

use std::collections::{HashMap, HashSet};

use turso::transaction::TransactionBehavior;

use super::*;

/// Default maximum recorded verification findings.
pub const MAX_VERIFICATION_FINDINGS: usize = 256;
/// Hard maximum recorded verification findings.
pub const MAX_VERIFICATION_FINDINGS_HARD: usize = 1_024;

/// Stable non-secret class of a full-store integrity finding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationCategory {
    /// Schema version, object, or foreign-key structure is invalid.
    Schema,
    /// Context provisioning or active-state invariant is invalid.
    Context,
    /// Author allowlist or event authorization is invalid.
    Authorization,
    /// Canonical event envelope is malformed or unverifiable.
    EventWire,
    /// Event row identifier differs from canonical wire identity.
    EventIdentity,
    /// Denormalized event columns differ from canonical wire.
    EventColumns,
    /// Stored edge ordinals or parent sequence differ from signed parents.
    EdgeSet,
    /// A signed parent is absent.
    ParentMissing,
    /// A signed parent belongs to another context.
    ParentContext,
    /// Genesis/root cardinality or identity is invalid.
    Genesis,
    /// The stored directed graph contains a cycle.
    Cycle,
    /// A local ref has an invalid or absent/cross-context target.
    LocalRef,
    /// A remote ref has an invalid or absent/cross-context target.
    RemoteRef,
    /// A stored local, remote, or peer name is noncanonical.
    RefName,
    /// Projection from a ref exceeded its verification resource bound.
    ProjectionLimit,
}

/// One bounded non-secret integrity finding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationFinding {
    /// Stable finding class.
    pub category: VerificationCategory,
    /// Safely decoded related context, when available.
    pub context: Option<ContextId>,
    /// Safely decoded primary event, when available.
    pub event: Option<EventId>,
    /// Safely decoded related parent or target, when available.
    pub related_event: Option<EventId>,
}

/// Complete bounded outcome of a full-store verification scan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationReport {
    /// True only when the complete scan found no inconsistency.
    pub valid: bool,
    /// Number of safely decoded context rows checked.
    pub checked_contexts: usize,
    /// Number of event rows inspected.
    pub checked_events: usize,
    /// Number of local and remote ref rows inspected.
    pub checked_refs: usize,
    /// Bounded non-secret findings.
    pub findings: Vec<VerificationFinding>,
    /// True when additional findings existed or interpretation became unsafe.
    pub truncated: bool,
}

/// Resource limits used by full-store verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerificationLimits {
    /// Maximum findings retained in the report.
    pub max_findings: usize,
    /// Bound applied to projection from each stored ref.
    pub projection: ProjectionLimits,
}

impl VerificationLimits {
    /// Constructs checked limits.
    pub fn new(max_findings: usize, projection: ProjectionLimits) -> StoreResult<Self> {
        if max_findings == 0 || max_findings > MAX_VERIFICATION_FINDINGS_HARD {
            return Err(StoreError::VerificationLimitInvalid);
        }
        validate_projection_limits(projection)?;
        Ok(Self {
            max_findings,
            projection,
        })
    }
}

impl Default for VerificationLimits {
    fn default() -> Self {
        Self {
            max_findings: MAX_VERIFICATION_FINDINGS,
            projection: ProjectionLimits::default(),
        }
    }
}

#[derive(Clone, Copy)]
struct ContextCheck {
    expected: EventId,
    genesis: Option<EventId>,
    state: i64,
}

struct Findings {
    max: usize,
    values: Vec<VerificationFinding>,
    truncated: bool,
}

impl Findings {
    fn push(
        &mut self,
        category: VerificationCategory,
        context: Option<ContextId>,
        event: Option<EventId>,
        related_event: Option<EventId>,
    ) {
        if self.values.len() < self.max {
            self.values.push(VerificationFinding {
                category,
                context,
                event,
                related_event,
            });
        } else {
            self.truncated = true;
        }
    }
}

impl Store {
    /// Verifies the complete store from canonical wire in one read snapshot without repair.
    pub async fn verify_full(&self, limits: VerificationLimits) -> StoreResult<VerificationReport> {
        VerificationLimits::new(limits.max_findings, limits.projection)?;
        let mut conn = self.connection().await?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .await
            .map_err(map_db)?;
        let result = verify_on(&tx, limits).await;
        match result {
            Ok(report) => {
                tx.commit().await.map_err(map_db)?;
                Ok(report)
            }
            Err(error) => {
                tx.rollback().await.map_err(map_db)?;
                Err(error)
            }
        }
    }
}

async fn verify_on(
    conn: &Connection,
    limits: VerificationLimits,
) -> StoreResult<VerificationReport> {
    let mut findings = Findings {
        max: limits.max_findings,
        values: Vec::new(),
        truncated: false,
    };
    if verify_objects(conn).await.is_err() {
        findings.push(VerificationCategory::Schema, None, None, None);
    }

    let mut contexts = HashMap::new();
    let mut context_rows = conn.query("SELECT context_id,expected_genesis_id,genesis_event_id,state FROM contexts ORDER BY context_id", ()).await.map_err(map_db)?;
    while let Some(row) = context_rows.next().await.map_err(map_db)? {
        let context = context_value(row.get_value(0).map_err(|_| StoreError::CorruptStorage)?);
        let expected = id_value(row.get_value(1).map_err(|_| StoreError::CorruptStorage)?);
        let genesis_value = row.get_value(2).map_err(|_| StoreError::CorruptStorage)?;
        let state: Result<i64, _> = row.get(3);
        match (context, expected, genesis_value, state) {
            (Ok(context), Ok(expected), genesis_value, Ok(state)) => {
                let genesis = match genesis_value {
                    Value::Null => Ok(None),
                    value => id_value(value).map(Some),
                };
                match genesis {
                    Ok(genesis) => {
                        if !matches!(state, 0 | 1)
                            || (state == 0 && genesis.is_some())
                            || (state == 1 && genesis != Some(expected))
                        {
                            findings.push(
                                VerificationCategory::Context,
                                Some(context),
                                None,
                                genesis,
                            );
                        }
                        contexts.insert(
                            context,
                            ContextCheck {
                                expected,
                                genesis,
                                state,
                            },
                        );
                    }
                    Err(_) => {
                        findings.push(VerificationCategory::Context, Some(context), None, None)
                    }
                }
            }
            _ => {
                findings.push(VerificationCategory::Context, None, None, None);
                findings.truncated = true;
            }
        }
    }

    let mut authors = HashSet::new();
    let mut author_rows_query = conn
        .query(
            "SELECT context_id,author_id FROM authorized_authors ORDER BY context_id,author_id",
            (),
        )
        .await
        .map_err(map_db)?;
    while let Some(row) = author_rows_query.next().await.map_err(map_db)? {
        let context = context_value(row.get_value(0).map_err(|_| StoreError::CorruptStorage)?);
        let author = author_value(row.get_value(1).map_err(|_| StoreError::CorruptStorage)?);
        match (context, author) {
            (Ok(context), Ok(author)) => {
                authors.insert((context, author));
            }
            _ => findings.push(VerificationCategory::Authorization, None, None, None),
        }
    }
    for context in contexts.keys() {
        if !authors.iter().any(|(candidate, _)| candidate == context) {
            findings.push(
                VerificationCategory::Authorization,
                Some(*context),
                None,
                None,
            );
        }
    }

    let mut events = HashMap::new();
    let mut event_rows=conn.query("SELECT event_id,context_id,author_id,kind,canonical_wire FROM events ORDER BY event_id",()).await.map_err(map_db)?;
    let mut checked_events = 0_usize;
    while let Some(row) = event_rows.next().await.map_err(map_db)? {
        checked_events = checked_events.saturating_add(1);
        let row_id = id_value(row.get_value(0).map_err(|_| StoreError::CorruptStorage)?);
        let row_context = context_value(row.get_value(1).map_err(|_| StoreError::CorruptStorage)?);
        let row_author = author_value(row.get_value(2).map_err(|_| StoreError::CorruptStorage)?);
        let kind: Result<String, _> = row.get(3);
        let wire = blob_value(row.get_value(4).map_err(|_| StoreError::CorruptStorage)?);
        let safe_id = row_id.as_ref().ok().copied();
        let safe_context = row_context.as_ref().ok().copied();
        let (Ok(row_id), Ok(row_context), Ok(row_author), Ok(kind), Ok(wire)) =
            (row_id, row_context, row_author, kind, wire)
        else {
            findings.push(
                VerificationCategory::EventColumns,
                safe_context,
                safe_id,
                None,
            );
            continue;
        };
        let event = match SignedEventV1::from_wire(&wire) {
            Ok(event) => event,
            Err(_) => {
                findings.push(
                    VerificationCategory::EventWire,
                    Some(row_context),
                    Some(row_id),
                    None,
                );
                continue;
            }
        };
        if event.to_wire().map_err(StoreError::Contract)? != wire {
            findings.push(
                VerificationCategory::EventWire,
                Some(row_context),
                Some(row_id),
                None,
            );
        }
        if event.event_id() != row_id {
            findings.push(
                VerificationCategory::EventIdentity,
                Some(row_context),
                Some(row_id),
                Some(event.event_id()),
            );
        }
        if event.body().context() != row_context
            || event.body().author() != row_author
            || event.body().kind() != kind
        {
            findings.push(
                VerificationCategory::EventColumns,
                Some(row_context),
                Some(row_id),
                None,
            );
        }
        if !authors.contains(&(event.body().context(), event.body().author())) {
            findings.push(
                VerificationCategory::Authorization,
                Some(event.body().context()),
                Some(row_id),
                None,
            );
        }
        events.insert(row_id, event);
    }

    let mut edges: HashMap<EventId, Vec<(i64, EventId)>> = HashMap::new();
    let mut edge_rows_query = conn
        .query(
            "SELECT child_id,ordinal,parent_id FROM parent_edges ORDER BY child_id,ordinal",
            (),
        )
        .await
        .map_err(map_db)?;
    while let Some(row) = edge_rows_query.next().await.map_err(map_db)? {
        let child = id_value(row.get_value(0).map_err(|_| StoreError::CorruptStorage)?);
        let ordinal: Result<i64, _> = row.get(1);
        let parent = id_value(row.get_value(2).map_err(|_| StoreError::CorruptStorage)?);
        match (child, ordinal, parent) {
            (Ok(child), Ok(ordinal), Ok(parent)) => {
                edges.entry(child).or_default().push((ordinal, parent))
            }
            _ => findings.push(VerificationCategory::EdgeSet, None, None, None),
        }
    }
    for (id, event) in &events {
        let stored = edges.get(id).map(Vec::as_slice).unwrap_or(&[]);
        let exact = stored.len() == event.body().parents().len()
            && stored.iter().enumerate().all(|(index, (ordinal, parent))| {
                usize::try_from(*ordinal).ok() == Some(index)
                    && event.body().parents().get(index) == Some(parent)
            });
        if !exact {
            findings.push(
                VerificationCategory::EdgeSet,
                Some(event.body().context()),
                Some(*id),
                None,
            );
        }
        for parent in event.body().parents() {
            match events.get(parent) {
                None => findings.push(
                    VerificationCategory::ParentMissing,
                    Some(event.body().context()),
                    Some(*id),
                    Some(*parent),
                ),
                Some(parent_event) if parent_event.body().context() != event.body().context() => {
                    findings.push(
                        VerificationCategory::ParentContext,
                        Some(event.body().context()),
                        Some(*id),
                        Some(*parent),
                    )
                }
                Some(_) => {}
            }
        }
    }
    for child in edges.keys() {
        if !events.contains_key(child) {
            findings.push(VerificationCategory::EdgeSet, None, Some(*child), None);
        }
    }

    for (context, check) in &contexts {
        if check.state == 1 {
            match events.get(&check.expected) {
                Some(event)
                    if check.genesis == Some(check.expected)
                        && event.body().context() == *context
                        && event.body().kind() == "context.genesis"
                        && event.body().parents().is_empty() => {}
                _ => findings.push(
                    VerificationCategory::Genesis,
                    Some(*context),
                    Some(check.expected),
                    None,
                ),
            }
        }
    }
    for (id, event) in &events {
        let expected = contexts
            .get(&event.body().context())
            .map(|check| check.expected);
        let context_pending = contexts
            .get(&event.body().context())
            .is_some_and(|check| check.state == 0);
        if context_pending
            || (event.body().parents().is_empty() && expected != Some(*id))
            || (event.body().kind() == "context.genesis" && expected != Some(*id))
        {
            findings.push(
                VerificationCategory::Genesis,
                Some(event.body().context()),
                Some(*id),
                None,
            );
        }
    }
    if graph_has_cycle(&events) {
        findings.push(VerificationCategory::Cycle, None, None, None);
    }

    let mut ref_heads = Vec::new();
    let mut checked_refs = 0_usize;
    let mut local_rows = conn
        .query(
            "SELECT context_id,name,event_id FROM local_refs ORDER BY context_id,name",
            (),
        )
        .await
        .map_err(map_db)?;
    while let Some(row) = local_rows.next().await.map_err(map_db)? {
        checked_refs = checked_refs.saturating_add(1);
        let context = context_value(row.get_value(0).map_err(|_| StoreError::CorruptStorage)?);
        let name: Result<String, _> = row.get(1);
        let head = id_value(row.get_value(2).map_err(|_| StoreError::CorruptStorage)?);
        match (context, name, head) {
            (Ok(context), Ok(name), Ok(head)) => {
                if name.parse::<LocalRefName>().is_err() {
                    findings.push(
                        VerificationCategory::RefName,
                        Some(context),
                        None,
                        Some(head),
                    );
                }
                if events.get(&head).map(|event| event.body().context()) != Some(context) {
                    findings.push(
                        VerificationCategory::LocalRef,
                        Some(context),
                        None,
                        Some(head),
                    );
                } else {
                    ref_heads.push((context, head));
                }
            }
            _ => findings.push(VerificationCategory::LocalRef, None, None, None),
        }
    }
    let mut remote_rows = conn
        .query(
            "SELECT peer,context_id,name,event_id FROM remote_refs ORDER BY peer,context_id,name",
            (),
        )
        .await
        .map_err(map_db)?;
    while let Some(row) = remote_rows.next().await.map_err(map_db)? {
        checked_refs = checked_refs.saturating_add(1);
        let peer: Result<String, _> = row.get(0);
        let context = context_value(row.get_value(1).map_err(|_| StoreError::CorruptStorage)?);
        let name: Result<String, _> = row.get(2);
        let head = id_value(row.get_value(3).map_err(|_| StoreError::CorruptStorage)?);
        match (peer, context, name, head) {
            (Ok(peer), Ok(context), Ok(name), Ok(head)) => {
                if peer.parse::<PeerName>().is_err() || name.parse::<LocalRefName>().is_err() {
                    findings.push(
                        VerificationCategory::RefName,
                        Some(context),
                        None,
                        Some(head),
                    );
                }
                if events.get(&head).map(|event| event.body().context()) != Some(context) {
                    findings.push(
                        VerificationCategory::RemoteRef,
                        Some(context),
                        None,
                        Some(head),
                    );
                } else {
                    ref_heads.push((context, head));
                }
            }
            _ => findings.push(VerificationCategory::RemoteRef, None, None, None),
        }
    }
    for (context, head) in ref_heads {
        if let Err(error) = project_on(conn, context, &[head], limits.projection).await {
            let category = match error {
                StoreError::ProjectionLimitExceeded => VerificationCategory::ProjectionLimit,
                StoreError::ProjectionCycle => VerificationCategory::Cycle,
                _ => VerificationCategory::EventWire,
            };
            findings.push(category, Some(context), Some(head), None);
        }
    }
    let valid = findings.values.is_empty() && !findings.truncated;
    Ok(VerificationReport {
        valid,
        checked_contexts: contexts.len(),
        checked_events,
        checked_refs,
        findings: findings.values,
        truncated: findings.truncated,
    })
}

fn graph_has_cycle(events: &HashMap<EventId, SignedEventV1>) -> bool {
    #[derive(Clone, Copy)]
    enum Frame {
        Enter(EventId),
        Exit(EventId),
    }
    let mut state = HashMap::<EventId, u8>::new();
    let mut ids: Vec<_> = events.keys().copied().collect();
    ids.sort();
    for root in ids {
        if state.get(&root) == Some(&2) {
            continue;
        }
        let mut stack = vec![Frame::Enter(root)];
        while let Some(frame) = stack.pop() {
            match frame {
                Frame::Enter(id) => match state.get(&id).copied().unwrap_or(0) {
                    2 => {}
                    1 => return true,
                    _ => {
                        state.insert(id, 1);
                        stack.push(Frame::Exit(id));
                        if let Some(event) = events.get(&id) {
                            for parent in event.body().parents().iter().rev() {
                                if events.contains_key(parent) {
                                    stack.push(Frame::Enter(*parent));
                                }
                            }
                        }
                    }
                },
                Frame::Exit(id) => {
                    state.insert(id, 2);
                }
            }
        }
    }
    false
}
