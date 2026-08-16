//! OA-04 authentication, listener policy, and token non-disclosure evidence.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use contextmesh::crypto::SigningIdentity;
use contextmesh::http::{
    NON_LOOPBACK_PLAINTEXT_WARNING, PeerEndpoint, SyncServer, SyncServerConfig, TokenSource,
    TransportLimits,
};
use contextmesh::store::Store;
use contextmesh::sync::{PullClient, PullClientConfig, PullLimits};
use serde_json::{Value, json};
use tokio::net::TcpListener;

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn unique(tag: &str) -> String {
    format!(
        "oa04-auth-{}-{}.db",
        tag,
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(unique(tag))
}

fn token_text() -> String {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).expect("entropy");
    format!("token1_{}", URL_SAFE_NO_PAD.encode(bytes))
}

fn token_file(tag: &str) -> (PathBuf, String) {
    let path = temp_path(tag).with_extension("token");
    let token = token_text();
    std::fs::write(&path, token.as_bytes()).expect("write token");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("permissions");
    }
    (path, token)
}

async fn server_with_token(tag: &str) -> (SyncServer, PathBuf, String) {
    let store = Store::open(temp_path(tag)).await.expect("store");
    let identity = SigningIdentity::from_fixture_seed([21; 32]);
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
            token: TokenSource::file(path.clone()),
            limits: TransportLimits::default(),
        },
    )
    .await
    .expect("server");
    (server, path, token)
}

fn read_response(stream: &mut TcpStream) -> (u16, Option<String>) {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                buffer.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buffer);
                if let Some(split) = text.find("\r\n\r\n") {
                    let head = &text[..split];
                    let status = head
                        .split_whitespace()
                        .nth(1)
                        .and_then(|code| code.parse::<u16>().ok())
                        .unwrap_or_default();
                    let headers = head.to_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .and_then(|value| value.trim().parse::<usize>().ok());
                    let body_start = split + 4;
                    if let Some(length) = length
                        && buffer.len() >= body_start + length
                    {
                        return (
                            status,
                            Some(String::from_utf8_lossy(&buffer[body_start..]).into_owned()),
                        );
                    }
                }
            }
        }
    }
    let text = String::from_utf8_lossy(&buffer).into_owned();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or_default();
    (status, None)
}

fn assert_generic_401(status: u16, body: Option<&str>) {
    assert_eq!(status, 401);
    let body = body.expect("body");
    let value: Value = serde_json::from_str(body).expect("json");
    assert_eq!(
        value["error"]["code"], "authentication_failed",
        "code must be generic"
    );
    assert_eq!(value["protocol_version"], 1);
    let request_id = value["error"]["request_id"].as_str().expect("id");
    assert!(request_id.starts_with("req1_") && request_id.len() == 27);
}

#[tokio::test]
async fn loopback_is_default_and_non_loopback_needs_acknowledgement_and_warning() {
    let (server, _, _) = server_with_token("loopback").await;
    assert!(server.local_addr().ip().is_loopback());
    assert_eq!(server.exposure_warning(), None);

    assert!(PeerEndpoint::new("http://192.0.2.7:8123", false).is_err());
    let endpoint = PeerEndpoint::new("http://192.0.2.7:8123", true).expect("ack");
    assert_eq!(
        endpoint.exposure_warning(),
        Some(NON_LOOPBACK_PLAINTEXT_WARNING)
    );
    assert!(PeerEndpoint::new("https://127.0.0.1:9", true).is_err());
    assert!(PeerEndpoint::new("http://localhost:9", true).is_err());
    assert!(PeerEndpoint::new("http://user@127.0.0.1:9", true).is_err());
    assert!(PeerEndpoint::new("http://127.0.0.1", true).is_err());
}

#[tokio::test]
async fn sync_server_rejects_unacknowledged_non_loopback_bind() {
    let store = Store::open(temp_path("nonloop")).await.expect("store");
    let (path, _) = token_file("nonloop");
    let error = SyncServer::bind(
        store,
        SyncServerConfig {
            bind: "192.0.2.9:0".parse().expect("bind"),
            acknowledge_non_loopback_plaintext: false,
            token: TokenSource::file(path),
            limits: TransportLimits::default(),
        },
    )
    .await
    .expect_err("must fail");
    assert!(matches!(
        error,
        contextmesh::error::SyncError::ExposureNotAcknowledged
    ));
}

#[tokio::test]
async fn authentication_matrix_returns_one_generic_shape() {
    let (server, token_path, token) = server_with_token("matrix").await;
    let address = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve_until(std::future::pending::<()>()).await;
    });

    let client = reqwest::Client::new();
    let url = format!("http://{address}/v1/refs?context=x");

    let missing = client.get(&url).send().await.expect("send");
    assert_eq!(missing.status(), 401);
    assert_eq!(
        missing
            .headers()
            .get("www-authenticate")
            .map(|v| v.to_str().unwrap()),
        Some("Bearer")
    );
    let body = missing.text().await.expect("body");
    assert_generic_401(401, Some(&body));

    let wrong_scheme = client
        .get(&url)
        .header("authorization", "Basic abc")
        .send()
        .await
        .expect("send");
    assert_eq!(wrong_scheme.status(), 401);
    let body = wrong_scheme.text().await.expect("body");
    assert_generic_401(401, Some(&body));

    let short = client
        .get(&url)
        .header("authorization", "Bearer token1_AAAA")
        .send()
        .await
        .expect("send");
    assert_eq!(short.status(), 401);
    let body = short.text().await.expect("body");
    assert_generic_401(401, Some(&body));

    let wrong = client
        .get(&url)
        .header("authorization", format!("Bearer {token}x"))
        .send()
        .await
        .expect("send");
    assert_eq!(wrong.status(), 401);
    let wrong_body = wrong.text().await.expect("body");
    assert_generic_401(401, Some(&wrong_body));

    let duplicate_token = token.clone();
    let (status, body) = tokio::task::spawn_blocking(move || {
        let mut stream = TcpStream::connect(address).expect("connect");
        let request = format!(
            "GET /v1/refs?context=x HTTP/1.1\r\nhost: a\r\nauthorization: Bearer {duplicate_token}\r\nauthorization: Bearer {duplicate_token}\r\nconnection: close\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).expect("write");
        read_response(&mut stream)
    })
    .await
    .expect("duplicate-auth task");
    assert_generic_401(status, body.as_deref());

    let good = client
        .get(&url)
        .header("authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("send");
    assert_eq!(good.status(), 400, "authenticated but invalid context");

    drop(token_path);
}

#[tokio::test]
async fn token_sources_are_validated_and_never_disclosed() {
    let permissive = temp_path("perm");
    std::fs::write(&permissive, token_text()).expect("write");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&permissive, std::fs::Permissions::from_mode(0o644))
            .expect("permissions");
    }
    let store = Store::open(temp_path("disclose")).await.expect("store");
    let error = SyncServer::bind(
        store.clone(),
        SyncServerConfig {
            bind: "127.0.0.1:0".parse().expect("bind"),
            acknowledge_non_loopback_plaintext: false,
            token: TokenSource::file(permissive),
            limits: TransportLimits::default(),
        },
    )
    .await
    .expect_err("must reject group/other readable token");
    assert!(matches!(error, contextmesh::error::SyncError::TokenSource));

    let link = temp_path("link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&link, &link).ok();
    let error = SyncServer::bind(
        store,
        SyncServerConfig {
            bind: "127.0.0.1:0".parse().expect("bind"),
            acknowledge_non_loopback_plaintext: false,
            token: TokenSource::file(link),
            limits: TransportLimits::default(),
        },
    )
    .await
    .expect_err("must reject symlink");
    assert!(matches!(error, contextmesh::error::SyncError::TokenSource));

    let (path, token) = token_file("debug");
    let debug = format!("{:?}", TokenSource::file(&path));
    assert!(debug.contains("[REDACTED]") && !debug.contains(&token));
    let endpoint = PeerEndpoint::new("http://127.0.0.1:9", false).expect("endpoint");
    let debug = format!("{endpoint:?}");
    assert!(!debug.contains(&token));

    let store = Store::open(temp_path("client-debug")).await.expect("store");
    let config = PullClientConfig {
        peer: "alpha".parse().expect("peer"),
        endpoint: endpoint.clone(),
        token: TokenSource::file(&path),
        context: contextmesh::model::ContextId::from_bytes([1; 32]),
        limits: PullLimits::default(),
        transport: TransportLimits::default(),
    };
    let debug = format!("{config:?}");
    assert!(!debug.contains(&token));
    let client = PullClient::new(store, config).expect("client");
    let debug = format!("{client:?}");
    assert!(!debug.contains(&token));

    assert!(PeerEndpoint::new("http://192.0.2.3:9", false).is_err());
    drop(path);
}

#[tokio::test(flavor = "current_thread")]
async fn environment_token_source_is_supported() {
    let store = Store::open(temp_path("envtoken")).await.expect("store");
    let name = unique("envvar");
    let token = token_text();
    unsafe { std::env::set_var(&name, &token) };
    let server = SyncServer::bind(
        store,
        SyncServerConfig {
            bind: "127.0.0.1:0".parse().expect("bind"),
            acknowledge_non_loopback_plaintext: false,
            token: TokenSource::environment(name.clone()),
            limits: TransportLimits::default(),
        },
    )
    .await
    .expect("server from environment token");
    let address = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve_until(std::future::pending::<()>()).await;
    });
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{address}/v1/refs?context=x"))
        .header("authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("send");
    assert_eq!(response.status(), 400);
    unsafe { std::env::remove_var(&name) };
}

#[tokio::test]
async fn no_route_mutates_or_serves_unknown_paths() {
    let (server, _, token) = server_with_token("routes").await;
    let address = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve_until(std::future::pending::<()>()).await;
    });
    let client = reqwest::Client::new();
    for path in ["/", "/v1", "/v1/bundles", "/v1/events"] {
        let response = client
            .get(format!("http://{address}{path}"))
            .header("authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("send");
        assert_eq!(response.status(), 404, "{path} must not exist");
    }
    let response = client
        .get(format!("http://{address}/v1/refs"))
        .header("authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("send");
    assert_eq!(
        response.status(),
        400,
        "known route requires its exact target"
    );
    let response = client
        .delete(format!("http://{address}/v1/refs?context=x"))
        .header("authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("send");
    assert_eq!(response.status(), 405);
    let body = response.text().await.expect("body");
    let value: Value = serde_json::from_str(&body).expect("json");
    assert_eq!(value["error"]["code"], "method_not_allowed");
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    drop(listener);
    let _ = json!({});
}
