//! OA-05 bounded crash-window recovery queries for recorded invocations.

use turso::params;
use turso::transaction::TransactionBehavior;

use super::*;

/// Maximum pending or detached results returned before failing closed.
pub const MAX_INVOCATION_QUERY_RESULTS: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InvocationKind {
    Request,
    Result,
}

impl InvocationKind {
    fn from_kind(kind: &str) -> Option<Self> {
        match kind {
            "agent.request" => Some(Self::Request),
            "agent.response" | "agent.error" => Some(Self::Result),
            _ => None,
        }
    }
}

struct InvocationRow {
    kind: InvocationKind,
    event: SignedEventV1,
    on_branch: bool,
    linked: bool,
}

impl Store {
    /// Returns requests on the branch ancestry with no linked sole-parent result.
    pub async fn pending_invocations(
        &self,
        context: ContextId,
        branch: LocalRefName,
    ) -> StoreResult<Vec<SignedEventV1>> {
        let rows = self
            .invocation_rows(context, branch, InvocationKind::Request)
            .await?;
        Ok(rows
            .into_iter()
            .filter(|row| row.kind == InvocationKind::Request && !row.linked)
            .map(|row| row.event)
            .collect())
    }

    /// Returns recorded results in the context unreachable from the branch head.
    pub async fn detached_results(
        &self,
        context: ContextId,
        branch: LocalRefName,
    ) -> StoreResult<Vec<SignedEventV1>> {
        let rows = self
            .invocation_rows(context, branch, InvocationKind::Result)
            .await?;
        Ok(rows
            .into_iter()
            .filter(|row| row.kind == InvocationKind::Result && !row.on_branch)
            .map(|row| row.event)
            .collect())
    }

    async fn invocation_rows(
        &self,
        context: ContextId,
        branch: LocalRefName,
        scan: InvocationKind,
    ) -> StoreResult<Vec<InvocationRow>> {
        let head = self
            .local_ref(context, &branch)
            .await?
            .ok_or(StoreError::RefMissing)?;
        let projection = self
            .project(context, vec![head], ProjectionLimits::default())
            .await?;
        let ancestry: std::collections::HashSet<EventId> = projection
            .events
            .iter()
            .map(|event| event.event_id())
            .collect();
        let mut conn = self.connection().await?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .await
            .map_err(map_db)?;
        let result = async {
            let sql = format!(
                "SELECT canonical_wire FROM events WHERE context_id=?1 AND kind IN ('agent.request','agent.response','agent.error') LIMIT {limit}",
                limit = 2 * MAX_INVOCATION_QUERY_RESULTS + 1
            );
            let mut rows = tx
                .query(sql.as_str(), params![context.to_bytes().to_vec()])
                .await
                .map_err(map_db)?;
            let mut parsed = Vec::new();
            while let Some(row) = rows.next().await.map_err(map_db)? {
                let wire = blob_value(row.get_value(0).map_err(|_| StoreError::CorruptStorage)?)?;
                let event =
                    SignedEventV1::from_wire(&wire).map_err(|_| StoreError::CorruptStorage)?;
                let kind = InvocationKind::from_kind(event.body().kind())
                    .ok_or(StoreError::CorruptStorage)?;
                if event.body().context() != context {
                    return Err(StoreError::CorruptStorage);
                }
                parsed.push((kind, event));
            }
            if parsed
                .iter()
                .filter(|(kind, _)| *kind == scan)
                .count()
                > MAX_INVOCATION_QUERY_RESULTS
                || parsed.len() > 2 * MAX_INVOCATION_QUERY_RESULTS
            {
                return Err(StoreError::ProjectionLimitExceeded);
            }
            let answered: std::collections::HashSet<EventId> = parsed
                .iter()
                .filter(|(kind, _)| *kind == InvocationKind::Result)
                .filter_map(|(_, event)| {
                    let parents = event.body().parents();
                    (parents.len() == 1).then_some(parents[0])
                })
                .collect();
            Ok(parsed
                .into_iter()
                .map(|(kind, event)| {
                    let on_branch = ancestry.contains(&event.event_id());
                    let linked = answered.contains(&event.event_id());
                    InvocationRow {
                        kind,
                        event,
                        on_branch,
                        linked,
                    }
                })
                .collect::<Vec<_>>())
        }
        .await;
        finish_transaction(tx, result).await
    }
}
