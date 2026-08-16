//! OA-04 end-to-end pull synchronization evidence.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use contextmesh::crypto::SigningIdentity;
use contextmesh::http::{PeerEndpoint, SyncServer, SyncServerConfig, TokenSource, TransportLimits};
use contextmesh::model::{ContextId, EventId};
use contextmesh::store::{AdvertisedRef, ProjectionLimits, RefNamespace, Store, SyncExportPage};
use contextmesh::sync::{
    ExportResponse, PullClient, PullClientConfig, PullLimits, RefSnapshot, decode_cursor,
    encode_cursor, plan_fingerprint,
};
use serde_json::json;

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "oa04-sync-{tag}-{}-{}.db",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

fn token_file(tag: &str) -> PathBuf {
    let path = temp_path(tag).with_extension("token");
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).expect("entropy");
    std::fs::write(&path, format!("token1_{}", URL_SAFE_NO_PAD.encode(bytes))).expect("write");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("perm");
    }
    path
}

struct Node {
    store: Store,
    identity: SigningIdentity,
    context: ContextId,
    genesis: EventId,
}

async fn node(tag: &str, seed: u8, create: bool) -> Node {
    let store = Store::open(temp_path(tag)).await.expect("store");
    let identity = SigningIdentity::from_fixture_seed([seed; 32]);
    if create {
        let created = store
            .create_context(&identity, "main".parse().expect("name"))
            .await
            .expect("context");
        Node {
            store,
            identity,
            context: created.context,
            genesis: created.branch.head,
        }
    } else {
        Node {
            store,
            identity,
            context: ContextId::from_bytes([0; 32]),
            genesis: EventId::from_bytes([0; 32]),
        }
    }
}

async fn server_history(server: &Node) -> Vec<EventId> {
    let mut chain = Vec::new();
    let mut head = server.genesis;
    chain.push(head);
    for kind in ["demo.first", "demo.second"] {
        let event = server
            .store
            .append(
                &server.identity,
                server.context,
                "main".parse().unwrap(),
                head,
                kind,
                json!({"step":kind}),
            )
            .await
            .expect("append");
        head = event.event_id();
        chain.push(head);
    }
    server
        .store
        .create_branch(server.context, "feature".parse().unwrap(), server.genesis)
        .await
        .expect("branch");
    let feature = server
        .store
        .append(
            &server.identity,
            server.context,
            "feature".parse().unwrap(),
            server.genesis,
            "demo.fork",
            json!({}),
        )
        .await
        .expect("fork")
        .event_id();
    let merge = server
        .store
        .merge(
            &server.identity,
            server.context,
            "main".parse().unwrap(),
            head,
            vec![head, feature],
            json!({}),
        )
        .await
        .expect("merge")
        .event_id();
    chain.push(feature);
    chain.push(merge);
    chain
}

async fn spawn_server(store: Store, token: PathBuf) -> std::net::SocketAddr {
    let server = SyncServer::bind(
        store,
        SyncServerConfig {
            bind: "127.0.0.1:0".parse().expect("bind"),
            acknowledge_non_loopback_plaintext: false,
            token: TokenSource::file(token),
            limits: TransportLimits::default(),
        },
    )
    .await
    .expect("server");
    let address = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve_until(std::future::pending::<()>()).await;
    });
    address
}

fn client_config(
    address: std::net::SocketAddr,
    token: PathBuf,
    context: ContextId,
    max_events: usize,
) -> PullClientConfig {
    PullClientConfig {
        peer: "alpha".parse().expect("peer"),
        endpoint: PeerEndpoint::new(&format!("http://{address}"), false).expect("endpoint"),
        token: TokenSource::file(token),
        context,
        limits: PullLimits::new(max_events, 16 * 1024 * 1024, 100_000).expect("limits"),
        transport: TransportLimits::default(),
    }
}

#[tokio::test]
async fn one_way_pull_paginates_converges_and_preserves_local_refs() {
    let server = node("srv", 31, true).await;
    let history = server_history(&server).await;
    let merge = *history.last().expect("merge");
    let token = token_file("pull");
    let address = spawn_server(server.store.clone(), token.clone()).await;

    let mut client = node("cli", 41, false).await;
    client.context = server.context;
    client.genesis = server.genesis;
    client
        .store
        .join_context(contextmesh::store::ContextProvision {
            context: server.context,
            expected_genesis: server.genesis,
            authorized_authors: vec![server.identity.author()],
        })
        .await
        .expect("join");

    let unrelated = node("other", 51, true).await;
    let unrelated_ref = unrelated
        .store
        .list_local_refs(unrelated.context)
        .await
        .expect("refs");
    assert_eq!(unrelated_ref.len(), 1);

    let before = client
        .store
        .list_local_refs(server.context)
        .await
        .expect("refs");
    assert!(before.is_empty());

    let puller = PullClient::new(
        client.store.clone(),
        client_config(address, token.clone(), server.context, 1),
    )
    .expect("client");
    let report = puller.pull().await.expect("pull");
    assert_eq!(report.pages, 5, "one event per page across five events");
    assert_eq!(report.received, 5);
    assert_eq!(report.inserted, 5);
    assert_eq!(report.already_present, 0);
    assert_eq!(report.remote_refs_updated, 2);

    let after = client
        .store
        .list_local_refs(server.context)
        .await
        .expect("refs");
    assert!(after.is_empty(), "synchronization never creates local refs");
    let remote = client
        .store
        .list_remote_refs(Some(&"alpha".parse().unwrap()), server.context)
        .await
        .expect("remote");
    assert_eq!(remote.len(), 2);
    let projection = client
        .store
        .project(server.context, vec![merge], ProjectionLimits::default())
        .await
        .expect("project");
    assert_eq!(projection.events.len(), 5);
    assert_eq!(projection.events[0].event_id(), server.genesis);
    let server_projection = server
        .store
        .project(server.context, vec![merge], ProjectionLimits::default())
        .await
        .expect("server project");
    assert_eq!(projection.events.len(), server_projection.events.len());

    let unrelated_after = unrelated
        .store
        .list_local_refs(unrelated.context)
        .await
        .expect("refs");
    assert_eq!(unrelated_after, unrelated_ref);

    let repeat = puller.pull().await.expect("repeat");
    assert_eq!(repeat.inserted, 0);
    assert_eq!(repeat.remote_refs_updated, 0);
    let still = client
        .store
        .list_local_refs(server.context)
        .await
        .expect("refs");
    assert!(still.is_empty());
}

#[tokio::test]
async fn pagination_plan_is_immutable_while_refs_move() {
    let server = node("page", 61, true).await;
    let history = server_history(&server).await;
    let merge = *history.last().expect("merge");
    let first: SyncExportPage = server
        .store
        .export_sync_page(
            server.context,
            vec![merge],
            Vec::new(),
            0,
            2,
            16 * 1024 * 1024,
        )
        .await
        .expect("page one");
    assert_eq!(first.bundle.events().len(), 2);
    let next = first.next_offset.expect("more pages");
    assert_eq!(next, 2);

    let moving = server
        .store
        .append(
            &server.identity,
            server.context,
            "main".parse().unwrap(),
            merge,
            "demo.after",
            json!({}),
        )
        .await
        .expect("move ref")
        .event_id();

    let second = server
        .store
        .export_sync_page(
            server.context,
            vec![merge],
            Vec::new(),
            next,
            2,
            16 * 1024 * 1024,
        )
        .await
        .expect("page two after ref movement");
    assert_eq!(second.bundle.events().len(), 2);
    assert_ne!(
        second.bundle.events()[0].event_id(),
        first.bundle.events()[1].event_id(),
        "later page begins after the first page"
    );
    let all: Vec<_> = first
        .bundle
        .events()
        .iter()
        .chain(second.bundle.events())
        .map(|event| event.event_id())
        .collect();
    assert!(
        !all.contains(&moving),
        "plan must exclude post-snapshot events"
    );
    assert!(second.next_offset.is_some());
}

fn http_ok(wire: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        wire.len()
    )
    .into_bytes();
    response.extend_from_slice(wire);
    response
}

fn read_request(stream: &mut TcpStream) -> (String, Vec<u8>) {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                buffer.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buffer);
                if let Some(split) = text.find("\r\n\r\n") {
                    let head = &text[..split];
                    let length = head
                        .to_lowercase()
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .and_then(|value| value.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if buffer.len() >= split + 4 + length {
                        let path = head.split_whitespace().nth(1).unwrap_or("").to_string();
                        return (path, buffer[split + 4..].to_vec());
                    }
                }
            }
        }
    }
    (String::new(), Vec::new())
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_late_page_leaves_refs_unchanged_with_earlier_orphans() {
    let server = node("stub-srv", 71, true).await;
    let _ = server_history(&server).await;
    let mut refs: Vec<AdvertisedRef> = server
        .store
        .sync_local_ref_snapshot(server.context)
        .await
        .expect("snapshot")
        .into_iter()
        .map(|item| AdvertisedRef {
            namespace: RefNamespace::Local,
            name: item.name,
            head: item.head,
        })
        .collect();
    refs.retain(|item| item.name.as_str() == "main");
    let advertised_head = refs.first().expect("main ref").head;
    let snapshot = RefSnapshot::new(server.context, refs.clone()).expect("snapshot value");
    let refs_wire = snapshot.to_wire().expect("refs wire");

    let first: SyncExportPage = server
        .store
        .export_sync_page(
            server.context,
            vec![advertised_head],
            Vec::new(),
            0,
            2,
            16 * 1024 * 1024,
        )
        .await
        .expect("first page");
    let first_cursor = first.next_offset.expect("first cursor");
    let first_page_last = first
        .bundle
        .events()
        .last()
        .expect("first page event")
        .event_id();
    let effective = first.effective_known_heads.clone();
    let limits = PullLimits::new(2, 16 * 1024 * 1024, 100).expect("limits");
    let fingerprint = plan_fingerprint(server.context, &[advertised_head], &effective, limits)
        .expect("fingerprint");
    let first_wire = ExportResponse::new(
        server.context,
        &[advertised_head],
        first.bundle,
        Some(encode_cursor(first_cursor, fingerprint).expect("cursor")),
    )
    .expect("response")
    .to_wire()
    .expect("wire");
    let mut truncated = first_wire.clone();
    truncated.truncate(truncated.len() - 5);

    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let stub_address = listener.local_addr().expect("address");
    let stub_context = server.context;
    let stub_genesis = server.genesis;
    let stub_author = server.identity.author();
    let stub = tokio::task::spawn_blocking(move || {
        let mut page_count = 0;
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let (path, _) = read_request(&mut stream);
            let wire = if path.starts_with("/v1/refs") {
                refs_wire.clone()
            } else {
                page_count += 1;
                if page_count == 1 {
                    first_wire.clone()
                } else {
                    truncated.clone()
                }
            };
            let _ = stream.write_all(&http_ok(&wire));
            let _ = stream.flush();
            if page_count >= 2 {
                break;
            }
        }
    });

    let mut client = node("stub-cli", 81, false).await;
    client.context = stub_context;
    client.genesis = stub_genesis;
    client
        .store
        .join_context(contextmesh::store::ContextProvision {
            context: stub_context,
            expected_genesis: stub_genesis,
            authorized_authors: vec![stub_author],
        })
        .await
        .expect("join");

    let puller = PullClient::new(
        client.store.clone(),
        client_config(stub_address, token_file("stub"), stub_context, 2),
    )
    .expect("client");
    let error = puller.pull().await.expect_err("second page must fail");
    assert!(matches!(
        error,
        contextmesh::error::SyncError::Protocol
            | contextmesh::error::SyncError::Transport
            | contextmesh::error::SyncError::LimitExceeded
    ));
    let remote = client
        .store
        .list_remote_refs(Some(&"alpha".parse().unwrap()), stub_context)
        .await
        .expect("remote");
    assert!(
        remote.is_empty(),
        "invalid page must prevent ref replacement"
    );
    let local = client
        .store
        .list_local_refs(stub_context)
        .await
        .expect("local");
    assert!(local.is_empty());
    let imported = client
        .store
        .event(first_page_last)
        .await
        .expect("event lookup");
    assert!(
        imported.is_some(),
        "verified earlier page events remain as orphans"
    );
    stub.abort();
    let _ = decode_cursor("cursor1_").is_err();
}

#[tokio::test]
async fn unreachable_peer_times_out_boundedly_then_retry_converges() {
    let server = node("retry-srv", 91, true).await;
    let history = server_history(&server).await;
    let merge = *history.last().expect("merge");
    let token = token_file("retry");
    let address = spawn_server(server.store.clone(), token.clone()).await;

    let mut client = node("retry-cli", 92, false).await;
    client.context = server.context;
    client.genesis = server.genesis;
    client
        .store
        .join_context(contextmesh::store::ContextProvision {
            context: server.context,
            expected_genesis: server.genesis,
            authorized_authors: vec![server.identity.author()],
        })
        .await
        .expect("join");

    let dead = PullClient::new(
        client.store.clone(),
        PullClientConfig {
            peer: "beta".parse().expect("peer"),
            endpoint: PeerEndpoint::new("http://10.255.255.1:9", true).expect("endpoint"),
            token: TokenSource::file(token_file("dead")),
            context: server.context,
            limits: PullLimits::default(),
            transport: TransportLimits::default(),
        },
    )
    .expect("client");
    let error = dead.pull().await.expect_err("unreachable peer");
    assert!(matches!(
        error,
        contextmesh::error::SyncError::Timeout | contextmesh::error::SyncError::Transport
    ));
    let untouched = client
        .store
        .list_remote_refs(Some(&"beta".parse().unwrap()), server.context)
        .await
        .expect("remote");
    assert!(untouched.is_empty(), "failed pull must not replace refs");

    let puller = PullClient::new(
        client.store.clone(),
        client_config(address, token, server.context, 4),
    )
    .expect("client");
    let report = puller.pull().await.expect("retry pull");
    assert_eq!(report.inserted, 5);
    assert_eq!(report.received, 5);
    assert!(report.pages >= 2);
    let head = client
        .store
        .event(merge)
        .await
        .expect("event")
        .expect("merged head imported");
    assert_eq!(head.event_id(), merge);
}
