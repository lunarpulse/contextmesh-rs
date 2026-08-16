//! OA-04 read-snapshot export paging and atomic remote-ref replacement.

use std::collections::HashSet;

use turso::params;
use turso::transaction::TransactionBehavior;

use super::*;

/// One bounded page selected from an immutable ancestry-difference plan.
#[derive(Clone, Debug)]
pub struct SyncExportPage {
    /// Independently valid OA-03 Bundle v1 with an empty refs array.
    pub bundle: BundleV1,
    /// Same-context known heads that affected this snapshot's plan.
    pub effective_known_heads: Vec<EventId>,
    /// Next event offset when another page remains.
    pub next_offset: Option<usize>,
}

impl Store {
    /// Returns a checked local-ref snapshot for one active context.
    pub async fn sync_local_ref_snapshot(&self, context: ContextId) -> StoreResult<Vec<LocalRef>> {
        let mut conn = self.connection().await?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .await
            .map_err(map_db)?;
        let result = async {
            let (_, state) = context_row(&tx, context)
                .await?
                .ok_or(StoreError::ContextUnknown)?;
            if state != 1 {
                return Err(StoreError::ContextUnknown);
            }
            let mut rows = tx
                .query(
                    "SELECT name,event_id FROM local_refs WHERE context_id=?1 ORDER BY name",
                    params![context.to_bytes().to_vec()],
                )
                .await
                .map_err(map_db)?;
            let mut refs = Vec::new();
            while let Some(row) = rows.next().await.map_err(map_db)? {
                let name: String = row.get(0).map_err(|_| StoreError::CorruptStorage)?;
                let head = id_value(row.get_value(1).map_err(|_| StoreError::CorruptStorage)?)?;
                if authoritative_event_context(&tx, head).await? != Some(context) {
                    return Err(StoreError::CorruptStorage);
                }
                refs.push(LocalRef {
                    context,
                    name: name.parse()?,
                    head,
                });
            }
            if refs.len() > MAX_BUNDLE_REFS {
                return Err(StoreError::BundleLimitExceeded);
            }
            Ok(refs)
        }
        .await;
        finish_transaction(tx, result).await
    }

    /// Selects one deterministic parent-first page in a deferred read snapshot.
    pub async fn export_sync_page(
        &self,
        context: ContextId,
        requested_heads: Vec<EventId>,
        known_heads: Vec<EventId>,
        offset: usize,
        max_events: usize,
        max_bundle_bytes: usize,
    ) -> StoreResult<SyncExportPage> {
        if max_events == 0
            || max_events > MAX_BUNDLE_EVENTS
            || max_bundle_bytes == 0
            || max_bundle_bytes > MAX_BUNDLE_CANONICAL_BYTES
        {
            return Err(StoreError::BundleLimitExceeded);
        }
        let requested = normalize_heads(requested_heads)?;
        let mut known = known_heads;
        if known.len() > MAX_BUNDLE_REFS {
            return Err(StoreError::BundleLimitExceeded);
        }
        known.sort();
        known.dedup();

        let mut conn = self.connection().await?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .await
            .map_err(map_db)?;
        let result = async {
            let (_, state) = context_row(&tx, context)
                .await?
                .ok_or(StoreError::ContextUnknown)?;
            if state != 1 {
                return Err(StoreError::ContextUnknown);
            }
            let mut effective_known = Vec::with_capacity(known.len());
            for head in known {
                if authoritative_event_context(&tx, head).await? == Some(context) {
                    effective_known.push(head);
                }
            }
            let (requested_events, _) =
                project_on(&tx, context, &requested, ProjectionLimits::default()).await?;
            let known_ids: HashSet<EventId> = if effective_known.is_empty() {
                HashSet::new()
            } else {
                project_on(&tx, context, &effective_known, ProjectionLimits::default())
                    .await?
                    .0
                    .into_iter()
                    .map(|event| event.event_id())
                    .collect()
            };
            let plan: Vec<SignedEventV1> = requested_events
                .into_iter()
                .filter(|event| !known_ids.contains(&event.event_id()))
                .collect();
            if offset > plan.len() || (offset == plan.len() && offset != 0) {
                return Err(StoreError::BundleOrder);
            }
            if plan.is_empty() {
                return Ok(SyncExportPage {
                    bundle: BundleV1::from_parts(context, Vec::new(), Vec::new())?,
                    effective_known_heads: effective_known,
                    next_offset: None,
                });
            }
            let remaining = plan.len() - offset;
            let upper = remaining.min(max_events);
            let mut low = 1_usize;
            let mut high = upper;
            let mut best: Option<(usize, BundleV1)> = None;
            while low <= high {
                let count = low + (high - low) / 2;
                let bundle = BundleV1::from_parts(
                    context,
                    plan[offset..offset + count].to_vec(),
                    Vec::new(),
                )?;
                if bundle.to_wire()?.len() <= max_bundle_bytes {
                    best = Some((count, bundle));
                    low = count.saturating_add(1);
                } else {
                    high = count.saturating_sub(1);
                }
            }
            let (count, bundle) = best.ok_or(StoreError::BundleLimitExceeded)?;
            let next = offset
                .checked_add(count)
                .ok_or(StoreError::BundleLimitExceeded)?;
            Ok(SyncExportPage {
                bundle,
                effective_known_heads: effective_known,
                next_offset: (next < plan.len()).then_some(next),
            })
        }
        .await;
        finish_transaction(tx, result).await
    }

    /// Atomically replaces exactly one peer/context remote-ref namespace.
    pub async fn replace_remote_ref_snapshot(
        &self,
        peer: PeerName,
        context: ContextId,
        refs: Vec<AdvertisedRef>,
    ) -> StoreResult<usize> {
        if refs.len() > MAX_BUNDLE_REFS
            || refs.windows(2).any(|pair| pair[0].name >= pair[1].name)
            || refs
                .iter()
                .any(|item| item.namespace != RefNamespace::Local)
        {
            return Err(StoreError::BundleRefInvalid);
        }
        self.write(move |tx| {
            Box::pin(async move {
                let (_, state) = context_row(tx, context)
                    .await?
                    .ok_or(StoreError::ContextUnknown)?;
                if state != 1 {
                    return Err(StoreError::ContextUnknown);
                }
                for item in &refs {
                    if authoritative_event_context(tx, item.head).await? != Some(context) {
                        return Err(StoreError::BundleRefInvalid);
                    }
                }
                let mut rows = tx
                    .query(
                        "SELECT name,event_id FROM remote_refs WHERE peer=?1 AND context_id=?2 ORDER BY name",
                        params![peer.as_str(), context.to_bytes().to_vec()],
                    )
                    .await
                    .map_err(map_db)?;
                let mut existing = Vec::new();
                while let Some(row) = rows.next().await.map_err(map_db)? {
                    let name: String = row.get(0).map_err(|_| StoreError::CorruptStorage)?;
                    let head = id_value(
                        row.get_value(1)
                            .map_err(|_| StoreError::CorruptStorage)?,
                    )?;
                    existing.push((name.parse::<LocalRefName>()?, head));
                }
                let desired: Vec<_> = refs
                    .iter()
                    .map(|item| (item.name.clone(), item.head))
                    .collect();
                let changed = namespace_change_count(&existing, &desired)?;
                if changed == 0 {
                    return Ok(0);
                }
                tx.execute(
                    "DELETE FROM remote_refs WHERE peer=?1 AND context_id=?2",
                    params![peer.as_str(), context.to_bytes().to_vec()],
                )
                .await
                .map_err(map_db)?;
                for item in refs {
                    tx.execute(
                        "INSERT INTO remote_refs(peer,context_id,name,event_id) VALUES(?1,?2,?3,?4)",
                        params![
                            peer.as_str(),
                            context.to_bytes().to_vec(),
                            item.name.as_str(),
                            item.head.to_bytes().to_vec()
                        ],
                    )
                    .await
                    .map_err(map_db)?;
                }
                Ok(changed)
            })
        })
        .await
    }
}

fn namespace_change_count(
    existing: &[(LocalRefName, EventId)],
    desired: &[(LocalRefName, EventId)],
) -> StoreResult<usize> {
    let old: std::collections::HashMap<&LocalRefName, EventId> =
        existing.iter().map(|(name, head)| (name, *head)).collect();
    let new: std::collections::HashMap<&LocalRefName, EventId> =
        desired.iter().map(|(name, head)| (name, *head)).collect();
    let deleted = old.keys().filter(|name| !new.contains_key(*name)).count();
    let inserted_or_changed = new
        .iter()
        .filter(|(name, head)| old.get(*name) != Some(*head))
        .count();
    deleted
        .checked_add(inserted_or_changed)
        .ok_or(StoreError::BundleLimitExceeded)
}
