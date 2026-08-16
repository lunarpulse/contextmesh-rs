//! Strict bounded Bundle v1 model, deterministic export, and atomic import.

use std::collections::HashSet;

use serde_json::{Map, Value as JsonValue, json};
use turso::params;

use super::*;
use crate::model::{canonicalize, into_object, strict_json, take_required};

/// Independently versioned bundle wire version.
pub const BUNDLE_VERSION: u8 = 1;
/// Hard maximum events in one bundle.
pub const MAX_BUNDLE_EVENTS: usize = 1_024;
/// Hard maximum canonical or raw bytes in one bundle.
pub const MAX_BUNDLE_CANONICAL_BYTES: usize = 16 * 1024 * 1024;
/// Hard maximum advertised refs in one bundle.
pub const MAX_BUNDLE_REFS: usize = 256;

/// Namespace represented by an advertised Bundle v1 ref.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RefNamespace {
    /// A branch local to the advertising peer.
    Local,
}

/// Unsigned peer claim about one advertised branch head.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdvertisedRef {
    /// Explicit Bundle v1 namespace.
    pub namespace: RefNamespace,
    /// Canonical branch name.
    pub name: LocalRefName,
    /// Claimed immutable event head.
    pub head: EventId,
}

/// Checked per-operation limits no greater than Bundle v1 hard bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BundleLimits {
    /// Maximum event count.
    pub max_events: usize,
    /// Maximum raw and canonical byte count.
    pub max_canonical_bytes: usize,
    /// Maximum advertised-ref count.
    pub max_refs: usize,
}

impl BundleLimits {
    /// Constructs nonzero limits no greater than the hard Bundle v1 bounds.
    pub fn new(
        max_events: usize,
        max_canonical_bytes: usize,
        max_refs: usize,
    ) -> StoreResult<Self> {
        if max_events == 0
            || max_events > MAX_BUNDLE_EVENTS
            || max_canonical_bytes == 0
            || max_canonical_bytes > MAX_BUNDLE_CANONICAL_BYTES
            || max_refs == 0
            || max_refs > MAX_BUNDLE_REFS
        {
            return Err(StoreError::BundleLimitExceeded);
        }
        Ok(Self {
            max_events,
            max_canonical_bytes,
            max_refs,
        })
    }
}

impl Default for BundleLimits {
    fn default() -> Self {
        Self {
            max_events: MAX_BUNDLE_EVENTS,
            max_canonical_bytes: MAX_BUNDLE_CANONICAL_BYTES,
            max_refs: MAX_BUNDLE_REFS,
        }
    }
}

/// Immutable, independently validated Bundle v1 value.
#[derive(Clone, Debug)]
pub struct BundleV1 {
    context: ContextId,
    events: Vec<SignedEventV1>,
    refs: Vec<AdvertisedRef>,
}

impl BundleV1 {
    /// Strictly parses and verifies a Bundle v1 under the hard bounds.
    pub fn from_wire(input: &[u8]) -> StoreResult<Self> {
        if input.len() > MAX_BUNDLE_CANONICAL_BYTES {
            return Err(StoreError::BundleLimitExceeded);
        }
        let value = strict_json(input).map_err(|_| StoreError::BundleMalformed)?;
        let mut object = into_object(value).map_err(|_| StoreError::BundleMalformed)?;
        if object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "bundle_version" | "context" | "events" | "refs"
            )
        }) {
            return Err(StoreError::BundleMalformed);
        }
        let version = take_required(&mut object, "bundle_version")
            .map_err(|_| StoreError::BundleMalformed)?;
        if version.as_u64() != Some(u64::from(BUNDLE_VERSION)) {
            return Err(StoreError::BundleUnsupportedVersion);
        }
        let context: ContextId = take_string(&mut object, "context")?
            .parse()
            .map_err(|_| StoreError::BundleMalformed)?;
        let event_values =
            take_required(&mut object, "events").map_err(|_| StoreError::BundleMalformed)?;
        let ref_values =
            take_required(&mut object, "refs").map_err(|_| StoreError::BundleMalformed)?;
        let JsonValue::Array(event_values) = event_values else {
            return Err(StoreError::BundleMalformed);
        };
        let JsonValue::Array(ref_values) = ref_values else {
            return Err(StoreError::BundleMalformed);
        };
        if event_values.len() > MAX_BUNDLE_EVENTS || ref_values.len() > MAX_BUNDLE_REFS {
            return Err(StoreError::BundleLimitExceeded);
        }
        let mut events = Vec::with_capacity(event_values.len());
        for value in event_values {
            let wire = canonicalize(&value).map_err(|_| StoreError::BundleMalformed)?;
            let event = SignedEventV1::from_wire(&wire).map_err(|_| StoreError::BundleMalformed)?;
            events.push(event);
        }
        let mut refs = Vec::with_capacity(ref_values.len());
        for value in ref_values {
            refs.push(parse_ref(value)?);
        }
        let bundle = Self::from_parts(context, events, refs)?;
        if bundle.to_wire()?.len() > MAX_BUNDLE_CANONICAL_BYTES {
            return Err(StoreError::BundleLimitExceeded);
        }
        Ok(bundle)
    }

    /// Constructs a checked bundle, allowing parents outside the bundle as frontier.
    pub fn from_parts(
        context: ContextId,
        events: Vec<SignedEventV1>,
        refs: Vec<AdvertisedRef>,
    ) -> StoreResult<Self> {
        if events.len() > MAX_BUNDLE_EVENTS || refs.len() > MAX_BUNDLE_REFS {
            return Err(StoreError::BundleLimitExceeded);
        }
        let all: HashSet<EventId> = events.iter().map(SignedEventV1::event_id).collect();
        if all.len() != events.len() {
            return Err(StoreError::BundleOrder);
        }
        let mut seen = HashSet::with_capacity(events.len());
        for event in &events {
            event.verify().map_err(|_| StoreError::BundleMalformed)?;
            if event.body().context() != context {
                return Err(StoreError::BundleMalformed);
            }
            for parent in event.body().parents() {
                if all.contains(parent) && !seen.contains(parent) {
                    return Err(StoreError::BundleOrder);
                }
            }
            seen.insert(event.event_id());
        }
        if refs
            .windows(2)
            .any(|pair| ref_key(&pair[0]) >= ref_key(&pair[1]))
        {
            return Err(StoreError::BundleOrder);
        }
        Ok(Self {
            context,
            events,
            refs,
        })
    }

    /// Returns exact canonical JCS Bundle v1 bytes.
    pub fn to_wire(&self) -> StoreResult<Vec<u8>> {
        let mut events = Vec::with_capacity(self.events.len());
        for event in &self.events {
            let wire = event.to_wire().map_err(|_| StoreError::BundleMalformed)?;
            events.push(
                serde_json::from_slice::<JsonValue>(&wire)
                    .map_err(|_| StoreError::BundleMalformed)?,
            );
        }
        let refs: Vec<JsonValue> = self.refs.iter().map(|item| json!({"namespace":"local","name":item.name.as_str(),"head":item.head.to_string()})).collect();
        let value = json!({"bundle_version":BUNDLE_VERSION,"context":self.context.to_string(),"events":events,"refs":refs});
        let wire = canonicalize(&value).map_err(|_| StoreError::BundleMalformed)?;
        if wire.len() > MAX_BUNDLE_CANONICAL_BYTES {
            return Err(StoreError::BundleLimitExceeded);
        }
        Ok(wire)
    }

    /// Returns the single bundle context.
    #[must_use]
    pub const fn context(&self) -> ContextId {
        self.context
    }
    /// Returns parent-first verified events.
    #[must_use]
    pub fn events(&self) -> &[SignedEventV1] {
        &self.events
    }
    /// Returns sorted unique advertised refs.
    #[must_use]
    pub fn refs(&self) -> &[AdvertisedRef] {
        &self.refs
    }
}

/// Counts produced by one atomic idempotent bundle import.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImportReport {
    /// Newly inserted immutable events.
    pub inserted: usize,
    /// Existing identical events fully rechecked.
    pub already_present: usize,
    /// Inserted or changed remote-tracking refs.
    pub remote_refs_updated: usize,
}

impl Store {
    /// Exports a deterministic bounded ancestry difference plus a caller snapshot of refs.
    pub async fn export_bundle(
        &self,
        context: ContextId,
        requested_heads: Vec<EventId>,
        known_frontier: Vec<EventId>,
        advertised_ref_snapshot: Vec<LocalRef>,
        limits: BundleLimits,
    ) -> StoreResult<BundleV1> {
        validate_bundle_limits(limits)?;
        let requested = normalize_bundle_heads(requested_heads)?;
        let known = if known_frontier.is_empty() {
            Vec::new()
        } else {
            normalize_bundle_heads(known_frontier)?
        };
        let mut conn = self.connection().await?;
        let tx = conn.transaction().await.map_err(map_db)?;
        let result = async {
            let (requested_events, _) =
                project_on(&tx, context, &requested, ProjectionLimits::default()).await?;
            let known_ids = if known.is_empty() {
                HashSet::new()
            } else {
                project_on(&tx, context, &known, ProjectionLimits::default())
                    .await?
                    .0
                    .into_iter()
                    .map(|event| event.event_id())
                    .collect()
            };
            let events: Vec<_> = requested_events
                .into_iter()
                .filter(|event| !known_ids.contains(&event.event_id()))
                .collect();
            if events.len() > limits.max_events {
                return Err(StoreError::BundleLimitExceeded);
            }
            let mut refs = Vec::with_capacity(advertised_ref_snapshot.len());
            if advertised_ref_snapshot.len() > limits.max_refs {
                return Err(StoreError::BundleLimitExceeded);
            }
            for item in advertised_ref_snapshot {
                if item.context != context
                    || authoritative_event_context(&tx, item.head).await? != Some(context)
                {
                    return Err(StoreError::BundleRefInvalid);
                }
                refs.push(AdvertisedRef {
                    namespace: RefNamespace::Local,
                    name: item.name,
                    head: item.head,
                });
            }
            refs.sort_by(|a, b| ref_key(a).cmp(&ref_key(b)));
            let bundle = BundleV1::from_parts(context, events, refs)?;
            if bundle.to_wire()?.len() > limits.max_canonical_bytes {
                return Err(StoreError::BundleLimitExceeded);
            }
            Ok(bundle)
        }
        .await;
        match result {
            Ok(bundle) => {
                tx.commit().await.map_err(map_db)?;
                Ok(bundle)
            }
            Err(error) => {
                tx.rollback().await.map_err(map_db)?;
                Err(error)
            }
        }
    }

    /// Strictly parses and atomically imports one bundle into an explicit peer namespace.
    pub async fn import_bundle(
        &self,
        peer: PeerName,
        wire: &[u8],
        limits: BundleLimits,
    ) -> StoreResult<ImportReport> {
        validate_bundle_limits(limits)?;
        if wire.len() > limits.max_canonical_bytes {
            return Err(StoreError::BundleLimitExceeded);
        }
        let bundle = BundleV1::from_wire(wire)?;
        if bundle.events.len() > limits.max_events
            || bundle.refs.len() > limits.max_refs
            || bundle.to_wire()?.len() > limits.max_canonical_bytes
        {
            return Err(StoreError::BundleLimitExceeded);
        }
        self.import_bundle_value(peer, bundle).await
    }

    async fn import_bundle_value(
        &self,
        peer: PeerName,
        bundle: BundleV1,
    ) -> StoreResult<ImportReport> {
        self.write(move |tx| Box::pin(async move {
            let (expected_genesis, mut state) = context_row(tx, bundle.context).await?.ok_or(StoreError::ContextUnknown)?;
            let mut inserted = 0_usize; let mut already_present = 0_usize;
            for event in &bundle.events {
                let id=event.event_id(); let body=event.body(); let parents=body.parents();
                let is_genesis=id==expected_genesis && body.kind()=="context.genesis" && parents.is_empty();
                if state==0 && !is_genesis { return Err(StoreError::GenesisMismatch); }
                if state==1 && !is_genesis && (parents.is_empty() || body.kind()=="context.genesis") { return Err(StoreError::GenesisMismatch); }
                if !is_authorized(tx,bundle.context,body.author()).await? { return Err(StoreError::UnauthorizedAuthor); }
                for parent in parents { let parent_context=authoritative_event_context(tx,*parent).await?.ok_or(StoreError::ParentMissing(*parent))?; if parent_context!=bundle.context{return Err(StoreError::ParentContextMismatch(*parent));} }
                let event_bytes=event.to_wire().map_err(StoreError::Contract)?;
                match event_wire(tx,id).await? {
                    None if state == 1 && is_genesis => {
                        return Err(StoreError::CorruptStorage);
                    }
                    None => {
                        tx.execute("INSERT INTO events(event_id,context_id,author_id,kind,canonical_wire) VALUES(?1,?2,?3,?4,?5)",params![id.to_bytes().to_vec(),bundle.context.to_bytes().to_vec(),body.author().to_bytes().to_vec(),body.kind(),event_bytes]).await.map_err(map_db)?;
                        for (ordinal,parent) in parents.iter().enumerate(){tx.execute("INSERT INTO parent_edges(child_id,ordinal,parent_id) VALUES(?1,?2,?3)",params![id.to_bytes().to_vec(),i64::try_from(ordinal).map_err(|_|StoreError::BundleLimitExceeded)?,parent.to_bytes().to_vec()]).await.map_err(map_db)?;}
                        inserted=inserted.checked_add(1).ok_or(StoreError::BundleLimitExceeded)?;
                    }
                    Some(_) if state == 0 && is_genesis => {
                        return Err(StoreError::CorruptStorage);
                    }
                    Some(existing) if existing==event_bytes => { validate_stored_event(tx,event).await?; already_present=already_present.checked_add(1).ok_or(StoreError::BundleLimitExceeded)?; }
                    Some(_) => return Err(StoreError::EventCollision),
                }
                if state==0 { let changed=tx.execute("UPDATE contexts SET genesis_event_id=?1,state=1 WHERE context_id=?2 AND state=0 AND expected_genesis_id=?1",params![id.to_bytes().to_vec(),bundle.context.to_bytes().to_vec()]).await.map_err(map_db)?; if changed!=1{return Err(StoreError::GenesisMismatch);} state=1; }
            }
            let mut remote_refs_updated=0_usize;
            for item in &bundle.refs {
                if authoritative_event_context(tx,item.head).await? != Some(bundle.context) { return Err(StoreError::BundleRefInvalid); }
                let current=query_optional_id(tx,"SELECT event_id FROM remote_refs WHERE peer=?1 AND context_id=?2 AND name=?3",params![peer.as_str(),bundle.context.to_bytes().to_vec(),item.name.as_str()]).await?;
                if current != Some(item.head) { tx.execute("INSERT INTO remote_refs(peer,context_id,name,event_id) VALUES(?1,?2,?3,?4) ON CONFLICT(peer,context_id,name) DO UPDATE SET event_id=excluded.event_id",params![peer.as_str(),bundle.context.to_bytes().to_vec(),item.name.as_str(),item.head.to_bytes().to_vec()]).await.map_err(map_db)?; remote_refs_updated=remote_refs_updated.checked_add(1).ok_or(StoreError::BundleLimitExceeded)?; }
            }
            Ok(ImportReport{inserted,already_present,remote_refs_updated})
        })).await
    }
}

fn normalize_bundle_heads(mut heads: Vec<EventId>) -> StoreResult<Vec<EventId>> {
    if heads.is_empty() || heads.len() > MAX_BUNDLE_EVENTS {
        return Err(StoreError::BundleLimitExceeded);
    }
    heads.sort();
    heads.dedup();
    Ok(heads)
}

fn validate_bundle_limits(limits: BundleLimits) -> StoreResult<()> {
    BundleLimits::new(
        limits.max_events,
        limits.max_canonical_bytes,
        limits.max_refs,
    )
    .map(|_| ())
}
fn ref_key(item: &AdvertisedRef) -> (&str, &str) {
    ("local", item.name.as_str())
}
fn take_string(object: &mut Map<String, JsonValue>, key: &str) -> StoreResult<String> {
    match take_required(object, key).map_err(|_| StoreError::BundleMalformed)? {
        JsonValue::String(text) => Ok(text),
        _ => Err(StoreError::BundleMalformed),
    }
}
fn parse_ref(value: JsonValue) -> StoreResult<AdvertisedRef> {
    let mut object = into_object(value).map_err(|_| StoreError::BundleMalformed)?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "namespace" | "name" | "head"))
    {
        return Err(StoreError::BundleMalformed);
    }
    if take_string(&mut object, "namespace")? != "local" {
        return Err(StoreError::BundleRefInvalid);
    }
    let name = take_string(&mut object, "name")?
        .parse()
        .map_err(|_| StoreError::BundleRefInvalid)?;
    let head = take_string(&mut object, "head")?
        .parse()
        .map_err(|_| StoreError::BundleRefInvalid)?;
    Ok(AdvertisedRef {
        namespace: RefNamespace::Local,
        name,
        head,
    })
}
