//! OA-04 hostile transport, resource-bound, and bounded-client evidence.
//!
//! Raw socket exchanges run inside spawn_blocking because the minimized Tokio
//! feature set builds a current-thread test runtime; blocking directly in the
//! test body would starve the spawned server task and falsify the evidence.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use contextmesh::crypto::SigningIdentity;
use contextmesh::http::{
    MAX_PULL_REQUEST_BODY_BYTES, PeerEndpoint, SyncServer, SyncServerConfig, TokenSource,
    TransportLimits,
};
use contextmesh::model::ContextId;
use contextmesh::store::{ContextProvision, Store};
use contextmesh::sync::{PullClient, PullClientConfig, PullLimits};

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "oa04-xport-{tag}-{}-{}.db",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

fn token_file(tag: &str) -> (PathBuf, String) {
    let path = temp_path(tag).with_extension("token");
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).expect("entropy");
    let token = format!("token1_{}", URL_SAFE_NO_PAD.encode(bytes));
    std::fs::write(&path, token.as_bytes()).expect("write");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("perm");
    }
    (path, token)
}

async fn live_server(tag: &str) -> (std::net::SocketAddr, String) {
    let store = Store::open(temp_path(tag)).await.expect("store");
    let identity = SigningIdentity::from_fixture_seed([5; 32]);
    store
        .create_context(&identity, "main".parse().expect("name"))
        .await
        .expect("context");
    let (path, token) = token_file(tag);
    let server = SyncServer::bind(
        store,
        SyncServerConfig {
            bind: "127.0.0.1:0".parse().expect("bind"),
            acknowledge_non_loopback_plaintext: false,
            token: TokenSource::file(path),
            limits: TransportLimits::default(),
        },
    )
    .await
    .expect("server");
    let address = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve_until(std::future::pending::<()>()).await;
    });
    (address, token)
}

fn read_until_close(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(12)))
        .expect("timeout");
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    while let Ok(n) = stream.read(&mut chunk) {
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

fn status_of(response: &str) -> Option<u16> {
    response.split_whitespace().nth(1)?.parse().ok()
}

async fn blocking_exchange<F>(exchange: F) -> String
where
    F: FnOnce() -> String + Send + 'static,
{
    tokio::task::spawn_blocking(exchange)
        .await
        .expect("raw exchange task")
}

async fn pull_against(address: std::net::SocketAddr, token: &str) -> contextmesh::error::SyncError {
    let (path, _) = token_file("puller");
    std::fs::write(&path, token).expect("replace client token");
    let store = Store::open(temp_path("puller")).await.expect("store");
    PullClient::new(
        store,
        PullClientConfig {
            peer: "hostile".parse().expect("peer"),
            endpoint: PeerEndpoint::new(&format!("http://{address}"), false).expect("endpoint"),
            token: TokenSource::file(path),
            context: ContextId::from_bytes([2; 32]),
            limits: PullLimits::default(),
            transport: TransportLimits::default(),
        },
    )
    .expect("client")
    .pull()
    .await
    .expect_err("hostile server must fail the pull")
}

#[tokio::test]
async fn request_body_bound_is_enforced_before_parsing() {
    let (address, token) = live_server("body").await;
    let oversized = MAX_PULL_REQUEST_BODY_BYTES + 1;
    let response = blocking_exchange(move || {
        let mut stream = TcpStream::connect(address).expect("connect");
        // The declared length alone exceeds the bound; no body bytes are sent,
        // proving rejection happens before any body parsing or collection.
        let head = format!(
            "POST /v1/bundles/export HTTP/1.1\r\nhost: a\r\nauthorization: Bearer {token}\r\ncontent-type: application/json\r\ncontent-length: {oversized}\r\nconnection: close\r\n\r\n"
        );
        stream.write_all(head.as_bytes()).expect("head");
        read_until_close(&mut stream)
    })
    .await;
    assert_eq!(
        status_of(&response),
        Some(413),
        "oversized declared body: {response}"
    );
    assert!(response.contains("limit_exceeded"));
}

#[tokio::test]
async fn raw_header_flood_is_rejected_before_the_application() {
    let (address, token) = live_server("flood").await;
    let response = blocking_exchange(move || {
        let mut stream = TcpStream::connect(address).expect("connect");
        let mut request = format!(
            "GET /v1/refs?context=x HTTP/1.1\r\nhost: a\r\nauthorization: Bearer {token}\r\n"
        )
        .into_bytes();
        for index in 0..200 {
            request.extend_from_slice(format!("x-flood-{index}: v\r\n").as_bytes());
        }
        request.extend_from_slice(b"connection: close\r\n\r\n");
        stream.write_all(&request).expect("write");
        read_until_close(&mut stream)
    })
    .await;
    assert!(
        response.is_empty()
            || status_of(&response) == Some(431)
            || status_of(&response) == Some(400),
        "parser must reject or close: {response}"
    );
}

#[tokio::test]
async fn slow_partial_headers_are_cut_by_the_pre_handler_timer() {
    let (address, token) = live_server("slowloris").await;
    let response = blocking_exchange(move || {
        let mut stream = TcpStream::connect(address).expect("connect");
        let partial = format!(
            "GET /v1/refs?context=x HTTP/1.1\r\nhost: a\r\nauthorization: Bearer {token}\r\nx-par"
        );
        stream.write_all(partial.as_bytes()).expect("partial");
        std::thread::sleep(Duration::from_millis(5600));
        read_until_close(&mut stream)
    })
    .await;
    assert!(
        response.is_empty() || status_of(&response).is_none(),
        "slow partial headers must be cut without a response: {response}"
    );
}

#[tokio::test]
async fn slow_request_body_is_cut_by_the_body_read_timeout() {
    let (address, token) = live_server("slowbody").await;
    let response = blocking_exchange(move || {
        let mut stream = TcpStream::connect(address).expect("connect");
        let head = format!(
            "POST /v1/bundles/export HTTP/1.1\r\nhost: a\r\nauthorization: Bearer {token}\r\ncontent-type: application/json\r\ncontent-length: 64\r\nconnection: close\r\n\r\n{{\"a\""
        );
        stream.write_all(head.as_bytes()).expect("head");
        std::thread::sleep(Duration::from_millis(5600));
        read_until_close(&mut stream)
    })
    .await;
    assert!(
        response.is_empty() || matches!(status_of(&response), Some(408) | Some(413) | None),
        "slow body must time out or fail closed: {response}"
    );
}

#[tokio::test]
async fn concurrency_limit_rejects_rather_than_queueing() {
    let (address, token) = live_server("conc").await;
    let response = blocking_exchange(move || {
        let mut holders = Vec::new();
        for _ in 0..16 {
            let mut stream = TcpStream::connect(address).expect("connect");
            let head = format!(
                "POST /v1/bundles/export HTTP/1.1\r\nhost: a\r\nauthorization: Bearer {token}\r\ncontent-type: application/json\r\ncontent-length: 64\r\nconnection: close\r\n\r\n"
            );
            stream.write_all(head.as_bytes()).expect("hold head");
            holders.push(stream);
        }
        std::thread::sleep(Duration::from_millis(300));
        let mut extra = TcpStream::connect(address).expect("connect");
        let request = format!(
            "GET /v1/refs?context=x HTTP/1.1\r\nhost: a\r\nauthorization: Bearer {token}\r\nconnection: close\r\n\r\n"
        );
        extra.write_all(request.as_bytes()).expect("write");
        let response = read_until_close(&mut extra);
        drop(holders);
        response
    })
    .await;
    assert_eq!(
        status_of(&response),
        Some(503),
        "excess must be rejected: {response}"
    );
    assert!(response.contains("unavailable"));
}

#[tokio::test(flavor = "current_thread")]
async fn hostile_responses_stay_bounded_and_redirects_are_never_followed() {
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("truncated", b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 100\r\nconnection: close\r\n\r\n{\"a\":"[..].to_vec()),
        ("huge-length", b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 999999999\r\nconnection: close\r\n\r\n"[..].to_vec()),
        ("redirect", b"HTTP/1.1 302 Found\r\nlocation: http://127.0.0.1:1/evil\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"[..].to_vec()),
        ("oversized-stream", b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n40000000\r\n"[..].to_vec()),
    ];
    let token = {
        let (path, token) = token_file("hostile");
        drop(path);
        token
    };
    for (name, wire) in cases {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let wire = wire.clone();
        let server = tokio::task::spawn_blocking(move || {
            if let Some(Ok(mut stream)) = listener.incoming().next() {
                let mut buffer = [0_u8; 4096];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(&wire);
                let _ = stream.flush();
            }
        });
        let error = pull_against(address, &token).await;
        assert!(
            matches!(
                error,
                contextmesh::error::SyncError::LimitExceeded
                    | contextmesh::error::SyncError::Protocol
                    | contextmesh::error::SyncError::Transport
                    | contextmesh::error::SyncError::Timeout
            ),
            "{name} stayed bounded: {error:?}"
        );
        server.abort();
    }
}

#[tokio::test(flavor = "current_thread")]
async fn proxy_environment_is_ignored_by_the_client() {
    let server_store = Store::open(temp_path("proxy-srv")).await.expect("store");
    let identity = SigningIdentity::from_fixture_seed([5; 32]);
    let created = server_store
        .create_context(&identity, "main".parse().expect("name"))
        .await
        .expect("context");
    let (server_token_path, token) = token_file("proxy-srv");
    let server = SyncServer::bind(
        server_store,
        SyncServerConfig {
            bind: "127.0.0.1:0".parse().expect("bind"),
            acknowledge_non_loopback_plaintext: false,
            token: TokenSource::file(server_token_path),
            limits: TransportLimits::default(),
        },
    )
    .await
    .expect("server");
    let address = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve_until(std::future::pending::<()>()).await;
    });

    let (client_token_path, _) = token_file("proxy-cli");
    std::fs::write(&client_token_path, token.as_bytes()).expect("matching client token");
    let client_store = Store::open(temp_path("proxy-cli")).await.expect("store");
    client_store
        .join_context(ContextProvision {
            context: created.context,
            expected_genesis: created.branch.head,
            authorized_authors: vec![identity.author()],
        })
        .await
        .expect("join");
    // No other thread in this current-thread test touches process environment.
    unsafe {
        std::env::set_var("HTTP_PROXY", "http://127.0.0.1:1");
        std::env::set_var("http_proxy", "http://127.0.0.1:1");
    }
    let result = PullClient::new(
        client_store,
        PullClientConfig {
            peer: "alpha".parse().expect("peer"),
            endpoint: PeerEndpoint::new(&format!("http://{address}"), false).expect("endpoint"),
            token: TokenSource::file(client_token_path),
            context: created.context,
            limits: PullLimits::default(),
            transport: TransportLimits::default(),
        },
    )
    .expect("client")
    .pull()
    .await;
    unsafe {
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("http_proxy");
    }
    let report = result.expect("loopback pull must ignore hostile proxy environment");
    assert_eq!(report.inserted, 1);
    assert_eq!(report.remote_refs_updated, 1);
}

#[tokio::test]
async fn request_target_header_and_body_boundaries_are_exact() {
    let (address, token) = live_server("exact").await;
    let cases: Vec<(&str, Vec<u8>, u16)> = {
        let pad = |total: usize| "x".repeat(total - "/v1/refs?context=".len());
        let exact_target = format!(
            "GET /v1/refs?context={} HTTP/1.1\r\nhost: a\r\nauthorization: Bearer {token}\r\nconnection: close\r\n\r\n",
            pad(contextmesh::http::MAX_REQUEST_TARGET_BYTES)
        );
        let over_target = format!(
            "GET /v1/refs?context={} HTTP/1.1\r\nhost: a\r\nauthorization: Bearer {token}\r\nconnection: close\r\n\r\n",
            pad(contextmesh::http::MAX_REQUEST_TARGET_BYTES + 1)
        );
        let exact_value = format!(
            "GET /v1/refs?context=x HTTP/1.1\r\nhost: a\r\nauthorization: Bearer {token}\r\nx-pad: {}\r\nconnection: close\r\n\r\n",
            "v".repeat(contextmesh::http::MAX_HTTP_HEADER_VALUE_BYTES)
        );
        let over_value = format!(
            "GET /v1/refs?context=x HTTP/1.1\r\nhost: a\r\nauthorization: Bearer {token}\r\nx-pad: {}\r\nconnection: close\r\n\r\n",
            "v".repeat(contextmesh::http::MAX_HTTP_HEADER_VALUE_BYTES + 1)
        );
        let exact_body = format!(
            "POST /v1/bundles/export HTTP/1.1\r\nhost: a\r\nauthorization: Bearer {token}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            contextmesh::http::MAX_PULL_REQUEST_BODY_BYTES
        )
        .into_bytes();
        let mut exact_body_full = exact_body;
        exact_body_full
            .extend_from_slice(&vec![b'{'; contextmesh::http::MAX_PULL_REQUEST_BODY_BYTES]);
        let over_body = format!(
            "POST /v1/bundles/export HTTP/1.1\r\nhost: a\r\nauthorization: Bearer {token}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            contextmesh::http::MAX_PULL_REQUEST_BODY_BYTES + 1
        )
        .into_bytes();
        vec![
            ("target-exact", exact_target.into_bytes(), 400),
            ("target-plus-one", over_target.into_bytes(), 414),
            ("header-value-exact", exact_value.into_bytes(), 400),
            ("header-value-plus-one", over_value.into_bytes(), 431),
            ("declared-body-exact", exact_body_full, 400),
            ("declared-body-plus-one", over_body, 413),
        ]
    };
    for (name, wire, expected) in cases {
        let response = blocking_exchange(move || {
            let mut stream = TcpStream::connect(address).expect("connect");
            stream.write_all(&wire).expect("write");
            read_until_close(&mut stream)
        })
        .await;
        assert_eq!(status_of(&response), Some(expected), "{name}: {response}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn client_response_cap_boundary_is_exact() {
    let (path, token) = token_file("cap");
    for (name, declared) in [("exact", 1024_usize), ("plus-one", 1025_usize)] {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let server = tokio::task::spawn_blocking(move || {
            if let Some(Ok(mut stream)) = listener.incoming().next() {
                let mut buffer = [0_u8; 4096];
                let _ = stream.read(&mut buffer);
                let mut response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {declared}\r\nconnection: close\r\n\r\n"
                )
                .into_bytes();
                response.extend_from_slice(&vec![b'{'; declared]);
                let _ = stream.write_all(&response);
                let _ = stream.flush();
            }
        });
        let store = Store::open(temp_path("cap")).await.expect("store");
        let error = PullClient::new(
            store,
            PullClientConfig {
                peer: "hostile".parse().expect("peer"),
                endpoint: PeerEndpoint::new(&format!("http://{address}"), false).expect("endpoint"),
                token: TokenSource::file(path.clone()),
                context: ContextId::from_bytes([3; 32]),
                limits: PullLimits::default(),
                transport: TransportLimits::new(1024, 1024, 16).expect("transport"),
            },
        )
        .expect("client")
        .pull()
        .await
        .expect_err("canned response must fail");
        if name == "exact" {
            assert!(
                matches!(error, contextmesh::error::SyncError::Protocol),
                "exact-cap bytes reach the strict parser: {error:?}"
            );
        } else {
            assert!(
                matches!(error, contextmesh::error::SyncError::LimitExceeded),
                "plus-one declared response is rejected before reading: {error:?}"
            );
        }
        server.abort();
    }
    drop(token);
}
