//! Authenticated bounded HTTP/1 transport for OA-04 pull synchronization.

use std::fmt;
use std::future::Future;
use std::io::Read as _;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::extract::{Extension, Request, State};
use axum::http::header::{AUTHORIZATION, CONNECTION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Router, serve::Listener};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::redirect::Policy;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, TryAcquireError};
use tokio::time::{Sleep, timeout};
use zeroize::Zeroizing;

use crate::error::{SyncError, SyncResult};
use crate::model::{ContextId, canonicalize};
use crate::store::{AdvertisedRef, RefNamespace, Store};
use crate::sync::{
    ExportRequest, ExportResponse, RefSnapshot, SYNC_PROTOCOL_VERSION, decode_cursor,
    encode_cursor, plan_fingerprint,
};

/// Exact decoded bearer-token length.
pub const AUTH_TOKEN_BYTES: usize = 32;
/// Hard parsed HTTP header-count limit.
pub const MAX_HTTP_HEADERS: usize = 96;
/// Hard aggregate parsed header-name/value byte limit.
pub const MAX_HTTP_HEADER_BYTES: usize = 16 * 1024;
/// Hard single parsed header-value byte limit.
pub const MAX_HTTP_HEADER_VALUE_BYTES: usize = 8 * 1024;
/// Hard request-target byte limit.
pub const MAX_REQUEST_TARGET_BYTES: usize = 2 * 1024;
/// Hard pull request-body byte limit.
pub const MAX_PULL_REQUEST_BODY_BYTES: usize = 64 * 1024;
/// Hard synchronization response-body byte limit.
pub const MAX_SYNC_RESPONSE_BODY_BYTES: usize = 16 * 1024 * 1024 + 64 * 1024;
/// Hard concurrent handler limit.
pub const MAX_CONCURRENT_HTTP_REQUESTS: usize = 16;
/// Per-body read timeout.
pub const BODY_READ_TIMEOUT: Duration = Duration::from_secs(5);
/// TCP connect timeout.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Whole request and handler timeout.
pub const REQUEST_AND_HANDLER_TIMEOUT: Duration = Duration::from_secs(30);
/// Fixed warning attached to acknowledged non-loopback plaintext operation.
pub const NON_LOOPBACK_PLAINTEXT_WARNING: &str =
    "warning: non-loopback plain HTTP has no confidentiality or server identity";
const AUTH_HASH_CONTEXT: &str = "org.aaif.contextmesh.sync.auth.v1";

/// Explicit bearer-token source. Debug output is always redacted.
#[derive(Clone)]
pub enum TokenSource {
    /// Read the canonical token from this environment variable name.
    Environment(String),
    /// Read the canonical token from this permission-checked file.
    File(PathBuf),
}

impl TokenSource {
    /// Selects an explicit environment-variable source.
    #[must_use]
    pub fn environment(name: impl Into<String>) -> Self {
        Self::Environment(name.into())
    }

    /// Selects an explicit file source.
    #[must_use]
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::File(path.into())
    }

    fn load(&self) -> SyncResult<LoadedToken> {
        let bytes = match self {
            Self::Environment(name) => {
                if name.is_empty() {
                    return Err(SyncError::TokenSource);
                }
                std::env::var_os(name)
                    .and_then(|value| value.into_string().ok())
                    .ok_or(SyncError::TokenSource)?
                    .into_bytes()
            }
            Self::File(path) => load_token_file(path)?,
        };
        LoadedToken::parse(bytes)
    }
}

impl fmt::Debug for TokenSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TokenSource([REDACTED])")
    }
}

struct LoadedToken {
    authorization: Zeroizing<Vec<u8>>,
}

impl LoadedToken {
    fn parse(mut text: Vec<u8>) -> SyncResult<Self> {
        const PREFIX: &[u8] = b"token1_";
        const ENCODED_LEN: usize = 43;
        if text.len() != PREFIX.len() + ENCODED_LEN || !text.starts_with(PREFIX) {
            text.fill(0);
            return Err(SyncError::TokenSource);
        }
        let decoded = Zeroizing::new(
            URL_SAFE_NO_PAD
                .decode(&text[PREFIX.len()..])
                .map_err(|_| SyncError::TokenSource)?,
        );
        let canonical = Zeroizing::new(URL_SAFE_NO_PAD.encode(&decoded));
        if decoded.len() != AUTH_TOKEN_BYTES || canonical.as_bytes() != &text[PREFIX.len()..] {
            text.fill(0);
            return Err(SyncError::TokenSource);
        }
        let mut authorization = Vec::with_capacity(7 + text.len());
        authorization.extend_from_slice(b"Bearer ");
        authorization.extend_from_slice(&text);
        text.fill(0);
        Ok(Self {
            authorization: Zeroizing::new(authorization),
        })
    }

    fn header_value(&self) -> SyncResult<HeaderValue> {
        let mut value =
            HeaderValue::from_bytes(&self.authorization).map_err(|_| SyncError::TokenSource)?;
        value.set_sensitive(true);
        Ok(value)
    }

    fn hash(&self) -> [u8; 32] {
        blake3::derive_key(AUTH_HASH_CONTEXT, &self.authorization)
    }
}

impl fmt::Debug for LoadedToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("LoadedToken([REDACTED])")
    }
}

/// Validated plain-HTTP IP-literal peer endpoint.
#[derive(Clone, Eq, PartialEq)]
pub struct PeerEndpoint {
    address: SocketAddr,
    non_loopback_acknowledged: bool,
}

impl PeerEndpoint {
    /// Parses an absolute HTTP endpoint with an explicit IP-literal port.
    pub fn new(text: &str, acknowledge_non_loopback_plaintext: bool) -> SyncResult<Self> {
        let authority = text
            .strip_prefix("http://")
            .ok_or(SyncError::InvalidEndpoint)?;
        let authority = authority.strip_suffix('/').unwrap_or(authority);
        if authority.is_empty()
            || authority.contains('/')
            || authority.contains('?')
            || authority.contains('#')
            || authority.contains('@')
        {
            return Err(SyncError::InvalidEndpoint);
        }
        let address = SocketAddr::from_str(authority).map_err(|_| SyncError::InvalidEndpoint)?;
        if !address.ip().is_loopback() && !acknowledge_non_loopback_plaintext {
            return Err(SyncError::ExposureNotAcknowledged);
        }
        Ok(Self {
            address,
            non_loopback_acknowledged: acknowledge_non_loopback_plaintext,
        })
    }

    /// Returns the fixed exposure warning only for a non-loopback endpoint.
    #[must_use]
    pub fn exposure_warning(&self) -> Option<&'static str> {
        (!self.address.ip().is_loopback()).then_some(NON_LOOPBACK_PLAINTEXT_WARNING)
    }

    pub(crate) fn validate(&self) -> SyncResult<()> {
        if !self.address.ip().is_loopback() && !self.non_loopback_acknowledged {
            return Err(SyncError::ExposureNotAcknowledged);
        }
        Ok(())
    }

    fn url(&self, suffix: &str) -> String {
        format!("http://{}{}", self.address, suffix)
    }
}

impl fmt::Debug for PeerEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PeerEndpoint")
            .field("loopback", &self.address.ip().is_loopback())
            .field("port", &self.address.port())
            .finish()
    }
}

/// Checked per-server and per-client transport bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportLimits {
    /// Maximum request body accepted by the server.
    pub max_request_body_bytes: usize,
    /// Maximum response body accepted by the client.
    pub max_response_body_bytes: usize,
    /// Maximum concurrent server handlers.
    pub max_concurrent_requests: usize,
}

impl TransportLimits {
    /// Constructs nonzero limits no greater than protocol hard maxima.
    pub fn new(
        max_request_body_bytes: usize,
        max_response_body_bytes: usize,
        max_concurrent_requests: usize,
    ) -> SyncResult<Self> {
        if max_request_body_bytes == 0
            || max_request_body_bytes > MAX_PULL_REQUEST_BODY_BYTES
            || max_response_body_bytes == 0
            || max_response_body_bytes > MAX_SYNC_RESPONSE_BODY_BYTES
            || max_concurrent_requests == 0
            || max_concurrent_requests > MAX_CONCURRENT_HTTP_REQUESTS
        {
            return Err(SyncError::InvalidConfig);
        }
        Ok(Self {
            max_request_body_bytes,
            max_response_body_bytes,
            max_concurrent_requests,
        })
    }
}

impl Default for TransportLimits {
    fn default() -> Self {
        Self {
            max_request_body_bytes: MAX_PULL_REQUEST_BODY_BYTES,
            max_response_body_bytes: MAX_SYNC_RESPONSE_BODY_BYTES,
            max_concurrent_requests: MAX_CONCURRENT_HTTP_REQUESTS,
        }
    }
}

/// Checked server bind and authentication configuration.
#[derive(Clone, Debug)]
pub struct SyncServerConfig {
    /// Listener socket; use a loopback address unless exposure is acknowledged.
    pub bind: SocketAddr,
    /// Explicit acknowledgement for non-loopback plaintext listening.
    pub acknowledge_non_loopback_plaintext: bool,
    /// Bearer-token source loaded at server construction.
    pub token: TokenSource,
    /// Checked resource bounds.
    pub limits: TransportLimits,
}

/// Bound authenticated synchronization server.
pub struct SyncServer {
    listener: TcpListener,
    address: SocketAddr,
    state: AppState,
}

impl SyncServer {
    /// Binds the configured listener and loads the authentication token.
    pub async fn bind(store: Store, config: SyncServerConfig) -> SyncResult<Self> {
        if !config.bind.ip().is_loopback() && !config.acknowledge_non_loopback_plaintext {
            return Err(SyncError::ExposureNotAcknowledged);
        }
        TransportLimits::new(
            config.limits.max_request_body_bytes,
            config.limits.max_response_body_bytes,
            config.limits.max_concurrent_requests,
        )?;
        let token = config.token.load()?;
        let state = AppState::new(store, token.hash(), config.limits)?;
        let listener = TcpListener::bind(config.bind)
            .await
            .map_err(|_| SyncError::Transport)?;
        let address = listener.local_addr().map_err(|_| SyncError::Transport)?;
        Ok(Self {
            listener,
            address,
            state,
        })
    }

    /// Returns the actual bound socket address.
    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.address
    }

    /// Returns the fixed exposure warning only for non-loopback listeners.
    #[must_use]
    pub fn exposure_warning(&self) -> Option<&'static str> {
        (!self.address.ip().is_loopback()).then_some(NON_LOOPBACK_PLAINTEXT_WARNING)
    }

    /// Serves until the supplied shutdown future completes.
    pub async fn serve_until<F>(self, shutdown: F) -> SyncResult<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let state = self.state;
        let router = Router::new()
            .route("/v1/refs", get(get_refs))
            .route("/v1/bundles/export", post(post_export))
            .fallback(not_found)
            .method_not_allowed_fallback(method_not_allowed)
            .layer(middleware::from_fn_with_state(state.clone(), guard))
            .with_state(state);
        let listener = HeaderGuardListener {
            inner: self.listener,
        };
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(|_| SyncError::Transport)
    }
}

impl fmt::Debug for SyncServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SyncServer")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct AppState {
    store: Store,
    auth_hash: [u8; 32],
    request_ids: Arc<RequestIds>,
    permits: Arc<Semaphore>,
    limits: TransportLimits,
}

impl AppState {
    fn new(store: Store, auth_hash: [u8; 32], limits: TransportLimits) -> SyncResult<Self> {
        let mut seed = [0_u8; 32];
        getrandom::fill(&mut seed).map_err(|_| SyncError::Internal)?;
        Ok(Self {
            store,
            auth_hash,
            request_ids: Arc::new(RequestIds {
                seed: Zeroizing::new(seed),
                counter: AtomicU64::new(0),
            }),
            permits: Arc::new(Semaphore::new(limits.max_concurrent_requests)),
            limits,
        })
    }
}

struct RequestIds {
    seed: Zeroizing<[u8; 32]>,
    counter: AtomicU64,
}

impl RequestIds {
    fn next(&self) -> SyncResult<RequestId> {
        let counter = self
            .counter
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| SyncError::Internal)?;
        let hash = blake3::keyed_hash(&self.seed, &counter.to_be_bytes());
        Ok(RequestId(format!(
            "req1_{}",
            URL_SAFE_NO_PAD.encode(&hash.as_bytes()[..16])
        )))
    }
}

#[derive(Clone)]
struct RequestId(String);

async fn guard(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let request_id = match state.request_ids.next() {
        Ok(value) => value,
        Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal", None),
    };
    if request
        .uri()
        .path_and_query()
        .map_or(0, |value| value.as_str().len())
        > MAX_REQUEST_TARGET_BYTES
    {
        return error_response(
            StatusCode::URI_TOO_LONG,
            "limit_exceeded",
            Some(&request_id),
        );
    }
    if headers_exceed(request.headers()) {
        return error_response(
            StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            "limit_exceeded",
            Some(&request_id),
        );
    }
    if !authenticated(request.headers(), state.auth_hash) {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "authentication_failed",
            Some(&request_id),
        );
    }
    let permit = match state.permits.clone().try_acquire_owned() {
        Ok(value) => value,
        Err(TryAcquireError::NoPermits) => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                Some(&request_id),
            );
        }
        Err(TryAcquireError::Closed) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                Some(&request_id),
            );
        }
    };
    request.extensions_mut().insert(request_id.clone());
    let mut response = match timeout(REQUEST_AND_HANDLER_TIMEOUT, next.run(request)).await {
        Ok(value) => value,
        Err(_) => error_response(StatusCode::REQUEST_TIMEOUT, "timeout", Some(&request_id)),
    };
    drop(permit);
    response
        .headers_mut()
        .insert(CONNECTION, HeaderValue::from_static("close"));
    response
}

fn headers_exceed(headers: &HeaderMap) -> bool {
    if headers.len() > MAX_HTTP_HEADERS {
        return true;
    }
    let mut total = 0_usize;
    for (name, value) in headers {
        if value.as_bytes().len() > MAX_HTTP_HEADER_VALUE_BYTES {
            return true;
        }
        let Some(next) = total.checked_add(name.as_str().len()) else {
            return true;
        };
        let Some(next) = next.checked_add(value.as_bytes().len()) else {
            return true;
        };
        total = next;
    }
    total > MAX_HTTP_HEADER_BYTES
}

fn authenticated(headers: &HeaderMap, expected: [u8; 32]) -> bool {
    let values: Vec<_> = headers.get_all(AUTHORIZATION).iter().collect();
    if values.len() != 1 {
        return false;
    }
    let actual = blake3::derive_key(AUTH_HASH_CONTEXT, values[0].as_bytes());
    blake3::Hash::from_bytes(actual) == blake3::Hash::from_bytes(expected)
}

async fn get_refs(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    request: Request,
) -> Response {
    if request.method() != Method::GET || has_body(request.headers()) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "malformed_request",
            Some(&request_id),
        );
    }
    let Some(raw) = request.uri().path_and_query().map(|value| value.as_str()) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "malformed_request",
            Some(&request_id),
        );
    };
    let Some(context_text) = raw.strip_prefix("/v1/refs?context=") else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "malformed_request",
            Some(&request_id),
        );
    };
    if context_text.is_empty() || context_text.contains('&') || context_text.contains('%') {
        return error_response(
            StatusCode::BAD_REQUEST,
            "malformed_request",
            Some(&request_id),
        );
    }
    let context: ContextId = match context_text.parse() {
        Ok(value) => value,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "malformed_request",
                Some(&request_id),
            );
        }
    };
    let refs = match state.store.sync_local_ref_snapshot(context).await {
        Ok(value) => value
            .into_iter()
            .map(|item| AdvertisedRef {
                namespace: RefNamespace::Local,
                name: item.name,
                head: item.head,
            })
            .collect(),
        Err(error) => return store_error(error, &request_id),
    };
    match RefSnapshot::new(context, refs).and_then(|snapshot| snapshot.to_wire()) {
        Ok(wire) => json_response(StatusCode::OK, wire),
        Err(error) => sync_error(error, &request_id),
    }
}

async fn post_export(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    request: Request,
) -> Response {
    if request.method() != Method::POST || !json_content_type(request.headers()) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "malformed_request",
            Some(&request_id),
        );
    }
    if content_length_over(request.headers(), state.limits.max_request_body_bytes) {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "limit_exceeded",
            Some(&request_id),
        );
    }
    let body = match timeout(
        BODY_READ_TIMEOUT,
        to_bytes(
            request.into_body(),
            state.limits.max_request_body_bytes.saturating_add(1),
        ),
    )
    .await
    {
        Err(_) => return error_response(StatusCode::REQUEST_TIMEOUT, "timeout", Some(&request_id)),
        Ok(Err(_)) => {
            return error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "limit_exceeded",
                Some(&request_id),
            );
        }
        Ok(Ok(value)) => value,
    };
    if body.len() > state.limits.max_request_body_bytes {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "limit_exceeded",
            Some(&request_id),
        );
    }
    let export = match ExportRequest::from_wire(&body) {
        Ok(value) => value,
        Err(error) => return sync_error(error, &request_id),
    };
    let (offset, supplied_fingerprint) = match export.cursor.as_deref() {
        Some(value) => match decode_cursor(value) {
            Ok(value) => (value.0, Some(value.1)),
            Err(error) => return sync_error(error, &request_id),
        },
        None => (0, None),
    };
    let page = match state
        .store
        .export_sync_page(
            export.context,
            export.requested_heads.clone(),
            export.known_heads.clone(),
            offset,
            export.limits.max_events,
            export.limits.max_bundle_bytes,
        )
        .await
    {
        Ok(value) => value,
        Err(error) => return store_error(error, &request_id),
    };
    let fingerprint = match plan_fingerprint(
        export.context,
        &export.requested_heads,
        &page.effective_known_heads,
        export.limits,
    ) {
        Ok(value) => value,
        Err(error) => return sync_error(error, &request_id),
    };
    if supplied_fingerprint.is_some_and(|value| value != fingerprint) {
        return error_response(
            StatusCode::CONFLICT,
            "pagination_conflict",
            Some(&request_id),
        );
    }
    let next_cursor = match page
        .next_offset
        .map(|next| encode_cursor(next, fingerprint))
        .transpose()
    {
        Ok(value) => value,
        Err(error) => return sync_error(error, &request_id),
    };
    let response = match ExportResponse::new(
        export.context,
        &export.requested_heads,
        page.bundle,
        next_cursor,
    )
    .and_then(|value| value.to_wire())
    {
        Ok(value) if value.len() <= state.limits.max_response_body_bytes => value,
        Ok(_) => {
            return error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "limit_exceeded",
                Some(&request_id),
            );
        }
        Err(error) => return sync_error(error, &request_id),
    };
    json_response(StatusCode::OK, response)
}

async fn not_found(Extension(request_id): Extension<RequestId>) -> Response {
    error_response(StatusCode::NOT_FOUND, "not_found", Some(&request_id))
}

async fn method_not_allowed(Extension(request_id): Extension<RequestId>) -> Response {
    error_response(
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        Some(&request_id),
    )
}

fn has_body(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|value| value != 0)
        || headers.contains_key("transfer-encoding")
}

fn json_content_type(headers: &HeaderMap) -> bool {
    headers.get_all(CONTENT_TYPE).iter().count() == 1
        && headers.get(CONTENT_TYPE) == Some(&HeaderValue::from_static("application/json"))
}

fn content_length_over(headers: &HeaderMap, limit: usize) -> bool {
    let mut values = headers.get_all(CONTENT_LENGTH).iter();
    let Some(value) = values.next() else {
        return false;
    };
    if values.next().is_some() {
        return true;
    }
    value
        .to_str()
        .ok()
        .and_then(|text| text.parse::<usize>().ok())
        .is_none_or(|size| size > limit)
}

fn error_response(status: StatusCode, code: &str, request_id: Option<&RequestId>) -> Response {
    let request_id = request_id
        .map(|value| value.0.as_str())
        .unwrap_or("req1_AAAAAAAAAAAAAAAAAAAAAA");
    let wire = canonicalize(&serde_json::json!({
        "error":{"code":code,"request_id":request_id},
        "protocol_version":SYNC_PROTOCOL_VERSION
    }))
    .unwrap_or_else(|_| b"{\"error\":{\"code\":\"internal\",\"request_id\":\"req1_AAAAAAAAAAAAAAAAAAAAAA\"},\"protocol_version\":1}".to_vec());
    let mut response = json_response(status, wire);
    if status == StatusCode::UNAUTHORIZED {
        response
            .headers_mut()
            .insert("www-authenticate", HeaderValue::from_static("Bearer"));
    }
    response
}

fn json_response(status: StatusCode, wire: Vec<u8>) -> Response {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .header(CONNECTION, "close")
        .body(Body::from(wire))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

fn sync_error(error: SyncError, request_id: &RequestId) -> Response {
    match error {
        SyncError::UnsupportedVersion => error_response(
            StatusCode::BAD_REQUEST,
            "unsupported_version",
            Some(request_id),
        ),
        SyncError::LimitExceeded | SyncError::InvalidConfig => error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "limit_exceeded",
            Some(request_id),
        ),
        SyncError::PaginationConflict => error_response(
            StatusCode::CONFLICT,
            "pagination_conflict",
            Some(request_id),
        ),
        SyncError::Timeout => {
            error_response(StatusCode::REQUEST_TIMEOUT, "timeout", Some(request_id))
        }
        SyncError::Store(error) => store_error(error, request_id),
        SyncError::Protocol => error_response(
            StatusCode::BAD_REQUEST,
            "malformed_request",
            Some(request_id),
        ),
        _ => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            Some(request_id),
        ),
    }
}

fn store_error(error: crate::error::StoreError, request_id: &RequestId) -> Response {
    use crate::error::StoreError;
    match error {
        StoreError::ContextUnknown | StoreError::ParentMissing(_) => {
            error_response(StatusCode::NOT_FOUND, "not_found", Some(request_id))
        }
        StoreError::BundleLimitExceeded
        | StoreError::ProjectionLimitExceeded
        | StoreError::LimitExceeded => error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "limit_exceeded",
            Some(request_id),
        ),
        StoreError::BundleOrder => error_response(
            StatusCode::CONFLICT,
            "pagination_conflict",
            Some(request_id),
        ),
        StoreError::DatabaseUnavailable | StoreError::IndeterminateCommit => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            Some(request_id),
        ),
        _ => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            Some(request_id),
        ),
    }
}

/// Internal bounded Reqwest transport used by PullClient.
pub(crate) struct HttpClient {
    client: reqwest::Client,
    endpoint: PeerEndpoint,
    token: LoadedToken,
    response_limit: usize,
}

impl HttpClient {
    pub(crate) fn new(
        endpoint: &PeerEndpoint,
        source: &TokenSource,
        response_limit: usize,
    ) -> SyncResult<Self> {
        endpoint.validate()?;
        if response_limit == 0 || response_limit > MAX_SYNC_RESPONSE_BODY_BYTES {
            return Err(SyncError::InvalidConfig);
        }
        let token = source.load()?;
        let client = reqwest::Client::builder()
            .redirect(Policy::none())
            .no_proxy()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_AND_HANDLER_TIMEOUT)
            .http1_only()
            .build()
            .map_err(|_| SyncError::Transport)?;
        Ok(Self {
            client,
            endpoint: endpoint.clone(),
            token,
            response_limit,
        })
    }

    pub(crate) async fn get_refs(&self, context: ContextId) -> SyncResult<Vec<u8>> {
        let suffix = format!("/v1/refs?context={context}");
        let response = self
            .client
            .get(self.endpoint.url(&suffix))
            .header(AUTHORIZATION, self.token.header_value()?)
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response(response, self.response_limit).await
    }

    pub(crate) async fn post_export(&self, wire: &[u8]) -> SyncResult<Vec<u8>> {
        if wire.len() > MAX_PULL_REQUEST_BODY_BYTES {
            return Err(SyncError::LimitExceeded);
        }
        let response = self
            .client
            .post(self.endpoint.url("/v1/bundles/export"))
            .header(AUTHORIZATION, self.token.header_value()?)
            .header(CONTENT_TYPE, "application/json")
            .body(wire.to_vec())
            .send()
            .await
            .map_err(map_reqwest)?;
        read_response(response, self.response_limit).await
    }
}

async fn read_response(response: reqwest::Response, limit: usize) -> SyncResult<Vec<u8>> {
    let status = response.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => SyncError::Authentication,
            408 => SyncError::Timeout,
            409 => SyncError::PaginationConflict,
            413 | 414 | 431 => SyncError::LimitExceeded,
            _ if status.is_redirection() => SyncError::Protocol,
            _ => SyncError::Transport,
        });
    }
    if response.headers().get_all(CONTENT_TYPE).iter().count() != 1
        || response.headers().get(CONTENT_TYPE)
            != Some(&HeaderValue::from_static("application/json"))
    {
        return Err(SyncError::Protocol);
    }
    let mut lengths = response.headers().get_all(CONTENT_LENGTH).iter();
    if let Some(value) = lengths.next()
        && (lengths.next().is_some()
            || value
                .to_str()
                .ok()
                .and_then(|text| text.parse::<usize>().ok())
                .is_none_or(|value| value > limit))
    {
        return Err(SyncError::LimitExceeded);
    }
    timeout(BODY_READ_TIMEOUT, async move {
        let mut response = response;
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(map_reqwest)? {
            let next = body
                .len()
                .checked_add(chunk.len())
                .ok_or(SyncError::LimitExceeded)?;
            if next > limit {
                return Err(SyncError::LimitExceeded);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    })
    .await
    .map_err(|_| SyncError::Timeout)?
}

fn map_reqwest(error: reqwest::Error) -> SyncError {
    if error.is_timeout() {
        SyncError::Timeout
    } else {
        SyncError::Transport
    }
}

fn load_token_file(path: &Path) -> SyncResult<Vec<u8>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
        let before = std::fs::symlink_metadata(path).map_err(|_| SyncError::TokenSource)?;
        if !before.file_type().is_file()
            || before.file_type().is_symlink()
            || before.permissions().mode() & 0o077 != 0
        {
            return Err(SyncError::TokenSource);
        }
        let mut file = std::fs::File::open(path).map_err(|_| SyncError::TokenSource)?;
        let opened = file.metadata().map_err(|_| SyncError::TokenSource)?;
        if !opened.file_type().is_file()
            || opened.dev() != before.dev()
            || opened.ino() != before.ino()
            || opened.permissions().mode() & 0o077 != 0
        {
            return Err(SyncError::TokenSource);
        }
        let expected_len = 7_u64 + 43;
        if opened.len() != expected_len {
            return Err(SyncError::TokenSource);
        }
        let mut bytes = Vec::with_capacity(expected_len as usize);
        file.read_to_end(&mut bytes)
            .map_err(|_| SyncError::TokenSource)?;
        let after = std::fs::symlink_metadata(path).map_err(|_| SyncError::TokenSource)?;
        if !after.file_type().is_file()
            || after.file_type().is_symlink()
            || after.dev() != opened.dev()
            || after.ino() != opened.ino()
            || after.permissions().mode() & 0o077 != 0
        {
            bytes.fill(0);
            return Err(SyncError::TokenSource);
        }
        Ok(bytes)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(SyncError::TokenSource)
    }
}

struct HeaderGuardListener {
    inner: TcpListener,
}

impl Listener for HeaderGuardListener {
    type Io = HeaderGuardStream;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match self.inner.accept().await {
                Ok((stream, address)) => {
                    return (HeaderGuardStream::new(stream), address);
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.inner.local_addr()
    }
}

struct HeaderGuardStream {
    inner: TcpStream,
    timer: Option<Pin<Box<Sleep>>>,
    header_bytes: usize,
    tail: [u8; 3],
    tail_len: usize,
    complete: bool,
}

impl HeaderGuardStream {
    fn new(inner: TcpStream) -> Self {
        Self {
            inner,
            timer: None,
            header_bytes: 0,
            tail: [0; 3],
            tail_len: 0,
            complete: false,
        }
    }

    fn observe(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        if self.complete {
            return Ok(());
        }
        let before = self.header_bytes;
        self.header_bytes = self
            .header_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
        let mut combined = Vec::with_capacity(self.tail_len + bytes.len());
        combined.extend_from_slice(&self.tail[..self.tail_len]);
        combined.extend_from_slice(bytes);
        if let Some(position) = combined.windows(4).position(|window| window == b"\r\n\r\n") {
            let header_bytes = before
                .saturating_sub(self.tail_len)
                .checked_add(position + 4)
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
            if header_bytes > MAX_HTTP_HEADER_BYTES {
                return Err(std::io::ErrorKind::InvalidData.into());
            }
            self.complete = true;
            self.timer = None;
            return Ok(());
        }
        if self.header_bytes > MAX_HTTP_HEADER_BYTES {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        self.tail_len = combined.len().min(3);
        self.tail[..self.tail_len]
            .copy_from_slice(&combined[combined.len().saturating_sub(self.tail_len)..]);
        Ok(())
    }
}

impl AsyncRead for HeaderGuardStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if !self.complete {
            if self.timer.is_none() {
                self.timer = Some(Box::pin(tokio::time::sleep(BODY_READ_TIMEOUT)));
            }
            if self
                .timer
                .as_mut()
                .is_some_and(|timer| timer.as_mut().poll(context).is_ready())
            {
                return Poll::Ready(Err(std::io::ErrorKind::TimedOut.into()));
            }
        }
        let before = buffer.filled().len();
        match Pin::new(&mut self.inner).poll_read(context, buffer) {
            Poll::Ready(Ok(())) => {
                if let Err(error) = self.observe(&buffer.filled()[before..]) {
                    Poll::Ready(Err(error))
                } else {
                    Poll::Ready(Ok(()))
                }
            }
            other => other,
        }
    }
}

impl AsyncWrite for HeaderGuardStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(context, bytes)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(context)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(context)
    }
}
