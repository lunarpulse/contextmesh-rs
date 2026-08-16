//! Strict OA-04 protocol values, deterministic pull state machine, and reports.

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Map, Value, json};

use crate::error::{SyncError, SyncResult};
use crate::http::{PeerEndpoint, TokenSource, TransportLimits};
use crate::model::{
    ContextId, EventId, canonicalize, into_object, into_string, strict_json, take_required,
};
use crate::store::{
    AdvertisedRef, BundleV1, MAX_BUNDLE_CANONICAL_BYTES, MAX_BUNDLE_EVENTS, MAX_BUNDLE_REFS,
    PeerName, RefNamespace, Store,
};

/// Frozen authenticated synchronization protocol version.
pub const SYNC_PROTOCOL_VERSION: u8 = 1;
/// Maximum pages allowed in one pull.
pub const MAX_PULL_PAGES: usize = 100_000;
const REFS_HASH_CONTEXT: &str = "org.aaif.contextmesh.sync.refs.v1";
const HEADS_HASH_CONTEXT: &str = "org.aaif.contextmesh.sync.heads.v1";
const PLAN_HASH_CONTEXT: &str = "org.aaif.contextmesh.sync.plan.v1";

/// Checked synchronization page and total limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PullLimits {
    /// Maximum events requested per page.
    pub max_events: usize,
    /// Maximum canonical Bundle v1 bytes per page.
    pub max_bundle_bytes: usize,
    /// Maximum pages accepted in one pull.
    pub max_pages: usize,
}

impl PullLimits {
    /// Constructs limits no greater than the frozen protocol maxima.
    pub fn new(max_events: usize, max_bundle_bytes: usize, max_pages: usize) -> SyncResult<Self> {
        if max_events == 0
            || max_events > MAX_BUNDLE_EVENTS
            || max_bundle_bytes == 0
            || max_bundle_bytes > MAX_BUNDLE_CANONICAL_BYTES
            || max_pages == 0
            || max_pages > MAX_PULL_PAGES
        {
            return Err(SyncError::InvalidConfig);
        }
        Ok(Self {
            max_events,
            max_bundle_bytes,
            max_pages,
        })
    }
}

impl Default for PullLimits {
    fn default() -> Self {
        Self {
            max_events: MAX_BUNDLE_EVENTS,
            max_bundle_bytes: MAX_BUNDLE_CANONICAL_BYTES,
            max_pages: MAX_PULL_PAGES,
        }
    }
}

/// One immutable pull request page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportRequest {
    /// Context whose immutable ancestry is requested.
    pub context: ContextId,
    /// Sorted unique immutable requested heads.
    pub requested_heads: Vec<EventId>,
    /// Sorted unique locally known frontier hint.
    pub known_heads: Vec<EventId>,
    /// Opaque continuation cursor, or none for the first page.
    pub cursor: Option<String>,
    /// Checked page limits.
    pub limits: PullLimits,
}

impl ExportRequest {
    /// Constructs a checked request with canonically sorted unique heads.
    pub fn new(
        context: ContextId,
        requested_heads: Vec<EventId>,
        known_heads: Vec<EventId>,
        cursor: Option<String>,
        limits: PullLimits,
    ) -> SyncResult<Self> {
        validate_sorted_heads(&requested_heads, false)?;
        validate_sorted_heads(&known_heads, true)?;
        if requested_heads.len() > MAX_BUNDLE_REFS || known_heads.len() > MAX_BUNDLE_REFS {
            return Err(SyncError::LimitExceeded);
        }
        Ok(Self {
            context,
            requested_heads,
            known_heads,
            cursor,
            limits,
        })
    }

    /// Returns exact canonical JCS request bytes.
    pub fn to_wire(&self) -> SyncResult<Vec<u8>> {
        canonicalize(&json!({
            "context":self.context.to_string(),
            "cursor":self.cursor,
            "known_heads":self.known_heads.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "limits":{"max_bundle_bytes":self.limits.max_bundle_bytes,"max_events":self.limits.max_events},
            "protocol_version":SYNC_PROTOCOL_VERSION,
            "requested_heads":self.requested_heads.iter().map(ToString::to_string).collect::<Vec<_>>()
        }))
        .map_err(|_| SyncError::Protocol)
    }

    /// Strictly parses canonical request bytes.
    pub fn from_wire(input: &[u8]) -> SyncResult<Self> {
        let value = strict_protocol(input)?;
        let mut object = object_exact(
            value,
            &[
                "protocol_version",
                "context",
                "requested_heads",
                "known_heads",
                "cursor",
                "limits",
            ],
        )?;
        version(&mut object)?;
        let context = text::<ContextId>(&mut object, "context")?;
        let requested_heads = ids(take(&mut object, "requested_heads")?)?;
        let known_heads = ids(take(&mut object, "known_heads")?)?;
        let cursor = match take(&mut object, "cursor")? {
            Value::Null => None,
            Value::String(value) => Some(value),
            _ => return Err(SyncError::Protocol),
        };
        let mut limits = object_exact(
            take(&mut object, "limits")?,
            &["max_events", "max_bundle_bytes"],
        )?;
        let max_events = usize_value(&mut limits, "max_events")?;
        let max_bundle_bytes = usize_value(&mut limits, "max_bundle_bytes")?;
        let request = Self::new(
            context,
            requested_heads,
            known_heads,
            cursor,
            PullLimits::new(max_events, max_bundle_bytes, MAX_PULL_PAGES)?,
        )?;
        if request.to_wire()? != input {
            return Err(SyncError::Protocol);
        }
        Ok(request)
    }
}

/// Canonical remote local-ref snapshot.
#[derive(Clone, Debug)]
pub struct RefSnapshot {
    /// Context that owns the advertised refs.
    pub context: ContextId,
    /// Sorted unique unsigned peer claims.
    pub refs: Vec<AdvertisedRef>,
    /// Domain-separated BLAKE3 fingerprint of the canonical snapshot.
    pub fingerprint: String,
}

impl RefSnapshot {
    /// Constructs a checked snapshot with its canonical fingerprint.
    pub fn new(context: ContextId, refs: Vec<AdvertisedRef>) -> SyncResult<Self> {
        validate_refs(context, &refs)?;
        let value = refs_fingerprint_value(context, &refs);
        let fingerprint = encode_hash("refs1_", derive(REFS_HASH_CONTEXT, &value)?);
        Ok(Self {
            context,
            refs,
            fingerprint,
        })
    }

    /// Returns exact canonical JCS snapshot bytes.
    pub fn to_wire(&self) -> SyncResult<Vec<u8>> {
        canonicalize(
            &refs_fingerprint_value(self.context, &self.refs)
                .as_object()
                .map(|object| {
                    let mut object = object.clone();
                    object.insert(
                        "snapshot_fingerprint".into(),
                        Value::String(self.fingerprint.clone()),
                    );
                    Value::Object(object)
                })
                .ok_or(SyncError::Internal)?,
        )
        .map_err(|_| SyncError::Protocol)
    }

    /// Strictly parses canonical snapshot bytes.
    pub fn from_wire(input: &[u8]) -> SyncResult<Self> {
        let value = strict_protocol(input)?;
        let mut object = object_exact(
            value,
            &[
                "protocol_version",
                "context",
                "refs",
                "snapshot_fingerprint",
            ],
        )?;
        version(&mut object)?;
        let context = text::<ContextId>(&mut object, "context")?;
        let refs = parse_refs(context, take(&mut object, "refs")?)?;
        let fingerprint = string(&mut object, "snapshot_fingerprint")?;
        let snapshot = Self::new(context, refs)?;
        if snapshot.fingerprint != fingerprint || snapshot.to_wire()? != input {
            return Err(SyncError::Protocol);
        }
        Ok(snapshot)
    }
}

/// Canonical response containing one independently valid Bundle v1 page.
#[derive(Clone, Debug)]
pub struct ExportResponse {
    /// Context shared by the response and embedded bundle.
    pub context: ContextId,
    /// Domain-separated fingerprint of the requested heads.
    pub requested_head_fingerprint: String,
    /// Independently valid Bundle v1 page with no advertised refs.
    pub bundle: BundleV1,
    /// Continuation cursor when more pages remain.
    pub next_cursor: Option<String>,
    /// True exactly when this is the final page.
    pub complete: bool,
}

impl ExportResponse {
    /// Constructs a checked page response for one bundle page.
    pub fn new(
        context: ContextId,
        requested_heads: &[EventId],
        bundle: BundleV1,
        next_cursor: Option<String>,
    ) -> SyncResult<Self> {
        if bundle.context() != context || !bundle.refs().is_empty() {
            return Err(SyncError::Protocol);
        }
        Ok(Self {
            context,
            requested_head_fingerprint: requested_head_fingerprint(context, requested_heads)?,
            bundle,
            complete: next_cursor.is_none(),
            next_cursor,
        })
    }

    /// Returns exact canonical JCS response bytes.
    pub fn to_wire(&self) -> SyncResult<Vec<u8>> {
        let bundle: Value =
            serde_json::from_slice(&self.bundle.to_wire().map_err(SyncError::Store)?)
                .map_err(|_| SyncError::Internal)?;
        canonicalize(&json!({
            "bundle":bundle,"complete":self.complete,"context":self.context.to_string(),
            "next_cursor":self.next_cursor,"protocol_version":SYNC_PROTOCOL_VERSION,
            "requested_head_fingerprint":self.requested_head_fingerprint
        }))
        .map_err(|_| SyncError::Protocol)
    }

    /// Strictly parses canonical page-response bytes.
    pub fn from_wire(input: &[u8]) -> SyncResult<Self> {
        let value = strict_protocol(input)?;
        let mut object = object_exact(
            value,
            &[
                "protocol_version",
                "context",
                "requested_head_fingerprint",
                "bundle",
                "next_cursor",
                "complete",
            ],
        )?;
        version(&mut object)?;
        let context = text::<ContextId>(&mut object, "context")?;
        let requested_head_fingerprint = string(&mut object, "requested_head_fingerprint")?;
        let bundle_wire =
            canonicalize(&take(&mut object, "bundle")?).map_err(|_| SyncError::Protocol)?;
        let bundle = BundleV1::from_wire(&bundle_wire).map_err(SyncError::Store)?;
        let next_cursor = match take(&mut object, "next_cursor")? {
            Value::Null => None,
            Value::String(value) => Some(value),
            _ => return Err(SyncError::Protocol),
        };
        let complete = take(&mut object, "complete")?
            .as_bool()
            .ok_or(SyncError::Protocol)?;
        let response = Self {
            context,
            requested_head_fingerprint,
            bundle,
            next_cursor,
            complete,
        };
        if response.bundle.context() != context
            || !response.bundle.refs().is_empty()
            || response.complete != response.next_cursor.is_none()
            || response.to_wire()? != input
        {
            return Err(SyncError::Protocol);
        }
        Ok(response)
    }
}

/// Non-secret aggregate counters from one pull.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PullReport {
    /// Successfully imported pages.
    pub pages: usize,
    /// Verified event envelopes received across pages.
    pub received: usize,
    /// Newly inserted immutable events.
    pub inserted: usize,
    /// Existing identical events.
    pub already_present: usize,
    /// Inserted, changed, or deleted final remote refs.
    pub remote_refs_updated: usize,
}

/// Checked configuration for one pull client.
#[derive(Debug)]
pub struct PullClientConfig {
    /// Explicit remote peer namespace.
    pub peer: PeerName,
    /// Validated plain-HTTP endpoint.
    pub endpoint: PeerEndpoint,
    /// Explicit bearer-token source.
    pub token: TokenSource,
    /// Context to synchronize.
    pub context: ContextId,
    /// Page and total limits.
    pub limits: PullLimits,
    /// Checked client response and shared transport bounds.
    pub transport: TransportLimits,
}

/// Authenticated pull client bound to one local store.
#[derive(Clone)]
pub struct PullClient {
    store: Store,
    config: Arc<PullClientConfig>,
}

impl fmt::Debug for PullClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PullClient([REDACTED])")
    }
}

impl PullClient {
    /// Constructs a checked pull client without network I/O.
    pub fn new(store: Store, config: PullClientConfig) -> SyncResult<Self> {
        config.endpoint.validate()?;
        PullLimits::new(
            config.limits.max_events,
            config.limits.max_bundle_bytes,
            config.limits.max_pages,
        )?;
        TransportLimits::new(
            config.transport.max_request_body_bytes,
            config.transport.max_response_body_bytes,
            config.transport.max_concurrent_requests,
        )?;
        Ok(Self {
            store,
            config: Arc::new(config),
        })
    }

    /// Pulls immutable history and atomically replaces only the selected peer refs.
    pub async fn pull(&self) -> SyncResult<PullReport> {
        let transport = crate::http::HttpClient::new(
            &self.config.endpoint,
            &self.config.token,
            self.config.transport.max_response_body_bytes,
        )?;
        let refs_wire = transport.get_refs(self.config.context).await?;
        let snapshot = RefSnapshot::from_wire(&refs_wire)?;
        if snapshot.context != self.config.context {
            return Err(SyncError::Protocol);
        }
        let mut requested_heads: Vec<_> = snapshot.refs.iter().map(|item| item.head).collect();
        requested_heads.sort();
        requested_heads.dedup();
        let local = self.store.list_local_refs(self.config.context).await?;
        let remote = self
            .store
            .list_remote_refs(Some(&self.config.peer), self.config.context)
            .await?;
        let mut known_heads: Vec<_> = local
            .iter()
            .map(|item| item.head)
            .chain(remote.iter().map(|item| item.head))
            .collect();
        known_heads.sort();
        known_heads.dedup();
        if known_heads.len() > MAX_BUNDLE_REFS {
            return Err(SyncError::LimitExceeded);
        }
        if requested_heads.is_empty() {
            let updated = self
                .store
                .replace_remote_ref_snapshot(
                    self.config.peer.clone(),
                    self.config.context,
                    snapshot.refs,
                )
                .await?;
            return Ok(PullReport {
                pages: 0,
                received: 0,
                inserted: 0,
                already_present: 0,
                remote_refs_updated: updated,
            });
        }
        let expected_heads = requested_head_fingerprint(self.config.context, &requested_heads)?;
        let mut report = PullReport {
            pages: 0,
            received: 0,
            inserted: 0,
            already_present: 0,
            remote_refs_updated: 0,
        };
        let mut cursor = None;
        let mut expected_offset = 0_usize;
        let mut cursor_fingerprint = None;
        loop {
            if report.pages >= self.config.limits.max_pages {
                return Err(SyncError::LimitExceeded);
            }
            let request = ExportRequest::new(
                self.config.context,
                requested_heads.clone(),
                known_heads.clone(),
                cursor.clone(),
                self.config.limits,
            )?;
            let request_wire = request.to_wire()?;
            let response_wire = transport.post_export(&request_wire).await?;
            let response = ExportResponse::from_wire(&response_wire)?;
            if response.context != self.config.context
                || response.requested_head_fingerprint != expected_heads
            {
                return Err(SyncError::Protocol);
            }
            if response.bundle.events().is_empty() && !response.complete {
                return Err(SyncError::Protocol);
            }
            let next = if response.complete {
                None
            } else {
                let next_cursor = response.next_cursor.as_deref().ok_or(SyncError::Protocol)?;
                let (next_offset, fingerprint) = decode_cursor(next_cursor)?;
                let expected_next = expected_offset
                    .checked_add(response.bundle.events().len())
                    .ok_or(SyncError::LimitExceeded)?;
                if next_offset != expected_next
                    || next_offset <= expected_offset
                    || cursor_fingerprint.is_some_and(|value| value != fingerprint)
                {
                    return Err(SyncError::Protocol);
                }
                Some((next_offset, fingerprint, next_cursor.to_owned()))
            };
            let bundle_wire = response.bundle.to_wire().map_err(SyncError::Store)?;
            let imported = self
                .store
                .import_bundle(
                    self.config.peer.clone(),
                    &bundle_wire,
                    crate::store::BundleLimits::new(
                        self.config.limits.max_events,
                        self.config.limits.max_bundle_bytes,
                        MAX_BUNDLE_REFS,
                    )
                    .map_err(SyncError::Store)?,
                )
                .await?;
            report.pages = checked(report.pages, 1)?;
            report.received = checked(report.received, response.bundle.events().len())?;
            report.inserted = checked(report.inserted, imported.inserted)?;
            report.already_present = checked(report.already_present, imported.already_present)?;
            let Some((next_offset, fingerprint, next_cursor)) = next else {
                break;
            };
            expected_offset = next_offset;
            cursor_fingerprint = Some(fingerprint);
            cursor = Some(next_cursor);
        }
        report.remote_refs_updated = self
            .store
            .replace_remote_ref_snapshot(
                self.config.peer.clone(),
                self.config.context,
                snapshot.refs,
            )
            .await?;
        Ok(report)
    }
}

pub(crate) fn requested_head_fingerprint(
    context: ContextId,
    heads: &[EventId],
) -> SyncResult<String> {
    validate_sorted_heads(heads, false)?;
    Ok(encode_hash(
        "heads1_",
        derive(
            HEADS_HASH_CONTEXT,
            &json!({"context":context.to_string(),"protocol_version":SYNC_PROTOCOL_VERSION,"requested_heads":heads.iter().map(ToString::to_string).collect::<Vec<_>>() }),
        )?,
    ))
}

/// Domain-separated plan fingerprint binding the immutable pagination plan.
pub fn plan_fingerprint(
    context: ContextId,
    requested: &[EventId],
    known: &[EventId],
    limits: PullLimits,
) -> SyncResult<[u8; 32]> {
    derive(
        PLAN_HASH_CONTEXT,
        &json!({"context":context.to_string(),"effective_known_heads":known.iter().map(ToString::to_string).collect::<Vec<_>>(),"limits":{"max_bundle_bytes":limits.max_bundle_bytes,"max_events":limits.max_events},"protocol_version":SYNC_PROTOCOL_VERSION,"requested_heads":requested.iter().map(ToString::to_string).collect::<Vec<_>>() }),
    )
}

/// Renders a checked opaque continuation cursor.
pub fn encode_cursor(offset: usize, fingerprint: [u8; 32]) -> SyncResult<String> {
    let offset = u64::try_from(offset).map_err(|_| SyncError::LimitExceeded)?;
    let mut bytes = [0_u8; 40];
    bytes[..8].copy_from_slice(&offset.to_be_bytes());
    bytes[8..].copy_from_slice(&fingerprint);
    Ok(format!("cursor1_{}", URL_SAFE_NO_PAD.encode(bytes)))
}

/// Strictly decodes and canonicalizes an opaque continuation cursor.
pub fn decode_cursor(text: &str) -> SyncResult<(usize, [u8; 32])> {
    let encoded = text
        .strip_prefix("cursor1_")
        .ok_or(SyncError::PaginationConflict)?;
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| SyncError::PaginationConflict)?;
    let bytes: [u8; 40] = decoded
        .try_into()
        .map_err(|_| SyncError::PaginationConflict)?;
    if encode_cursor(
        usize::try_from(u64::from_be_bytes(
            bytes[..8]
                .try_into()
                .map_err(|_| SyncError::PaginationConflict)?,
        ))
        .map_err(|_| SyncError::PaginationConflict)?,
        bytes[8..]
            .try_into()
            .map_err(|_| SyncError::PaginationConflict)?,
    )? != text
    {
        return Err(SyncError::PaginationConflict);
    }
    Ok((
        usize::try_from(u64::from_be_bytes(
            bytes[..8]
                .try_into()
                .map_err(|_| SyncError::PaginationConflict)?,
        ))
        .map_err(|_| SyncError::PaginationConflict)?,
        bytes[8..]
            .try_into()
            .map_err(|_| SyncError::PaginationConflict)?,
    ))
}

fn strict_protocol(input: &[u8]) -> SyncResult<Value> {
    let value = strict_json(input).map_err(|_| SyncError::Protocol)?;
    if canonicalize(&value).map_err(|_| SyncError::Protocol)? != input {
        return Err(SyncError::Protocol);
    }
    Ok(value)
}
fn object_exact(value: Value, keys: &[&str]) -> SyncResult<Map<String, Value>> {
    let object = into_object(value).map_err(|_| SyncError::Protocol)?;
    if object.len() != keys.len() || object.keys().any(|key| !keys.contains(&key.as_str())) {
        return Err(SyncError::Protocol);
    }
    Ok(object)
}
fn take(object: &mut Map<String, Value>, key: &str) -> SyncResult<Value> {
    take_required(object, key).map_err(|_| SyncError::Protocol)
}
fn string(object: &mut Map<String, Value>, key: &str) -> SyncResult<String> {
    into_string(take(object, key)?).map_err(|_| SyncError::Protocol)
}
fn text<T: FromStr>(object: &mut Map<String, Value>, key: &str) -> SyncResult<T> {
    string(object, key)?
        .parse()
        .map_err(|_| SyncError::Protocol)
}
fn version(object: &mut Map<String, Value>) -> SyncResult<()> {
    if take(object, "protocol_version")?.as_u64() != Some(u64::from(SYNC_PROTOCOL_VERSION)) {
        return Err(SyncError::UnsupportedVersion);
    }
    Ok(())
}
fn usize_value(object: &mut Map<String, Value>, key: &str) -> SyncResult<usize> {
    usize::try_from(take(object, key)?.as_u64().ok_or(SyncError::Protocol)?)
        .map_err(|_| SyncError::LimitExceeded)
}
fn ids(value: Value) -> SyncResult<Vec<EventId>> {
    let Value::Array(values) = value else {
        return Err(SyncError::Protocol);
    };
    values
        .into_iter()
        .map(|value| {
            into_string(value)
                .map_err(|_| SyncError::Protocol)?
                .parse()
                .map_err(|_| SyncError::Protocol)
        })
        .collect()
}
fn validate_sorted_heads(heads: &[EventId], empty: bool) -> SyncResult<()> {
    if (!empty && heads.is_empty()) || heads.windows(2).any(|p| p[0] >= p[1]) {
        return Err(SyncError::Protocol);
    }
    Ok(())
}
fn validate_refs(context: ContextId, refs: &[AdvertisedRef]) -> SyncResult<()> {
    if refs.len() > MAX_BUNDLE_REFS
        || refs.windows(2).any(|p| p[0].name >= p[1].name)
        || refs.iter().any(|r| r.namespace != RefNamespace::Local)
    {
        return Err(SyncError::Protocol);
    }
    let _ = context;
    Ok(())
}
fn parse_refs(context: ContextId, value: Value) -> SyncResult<Vec<AdvertisedRef>> {
    let Value::Array(values) = value else {
        return Err(SyncError::Protocol);
    };
    let mut refs = Vec::with_capacity(values.len());
    for value in values {
        let mut object = object_exact(value, &["namespace", "name", "head"])?;
        if string(&mut object, "namespace")? != "local" {
            return Err(SyncError::Protocol);
        }
        refs.push(AdvertisedRef {
            namespace: RefNamespace::Local,
            name: string(&mut object, "name")?
                .parse()
                .map_err(|_| SyncError::Protocol)?,
            head: text(&mut object, "head")?,
        });
    }
    validate_refs(context, &refs)?;
    Ok(refs)
}
fn refs_fingerprint_value(context: ContextId, refs: &[AdvertisedRef]) -> Value {
    json!({"context":context.to_string(),"protocol_version":SYNC_PROTOCOL_VERSION,"refs":refs.iter().map(|r|json!({"head":r.head.to_string(),"name":r.name.as_str(),"namespace":"local"})).collect::<Vec<_>>()})
}
fn derive(domain: &str, value: &Value) -> SyncResult<[u8; 32]> {
    let bytes = canonicalize(value).map_err(|_| SyncError::Protocol)?;
    Ok(blake3::derive_key(domain, &bytes))
}
fn encode_hash(prefix: &str, bytes: [u8; 32]) -> String {
    format!("{prefix}{}", URL_SAFE_NO_PAD.encode(bytes))
}
fn checked(left: usize, right: usize) -> SyncResult<usize> {
    left.checked_add(right).ok_or(SyncError::LimitExceeded)
}
