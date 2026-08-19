use contextmesh::crypto::SigningIdentity;
use contextmesh::model::{AuthorId, ContextId, EventId, SignedEventV1};
use contextmesh::store::{ContextProvision, LocalRefName, RefExpectation, RefMutation, Store};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub fn path(label: &str) -> PathBuf {
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "contextmesh-{label}-{}-{serial}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}
#[allow(dead_code)] // each integration test binary uses a subset of these helpers
pub fn identity(seed: u8) -> SigningIdentity {
    SigningIdentity::from_fixture_seed([seed; 32])
}
#[allow(dead_code)] // each integration test binary uses a subset of these helpers
pub fn sorted_authors(mut authors: Vec<AuthorId>) -> Vec<AuthorId> {
    authors.sort_by_key(ToString::to_string);
    authors
}
#[allow(dead_code)] // each integration test binary uses a subset of these helpers
pub fn context(byte: u8) -> ContextId {
    ContextId::from_bytes([byte; 32])
}
#[allow(dead_code)] // each integration test binary uses a subset of these helpers
pub fn genesis(who: &SigningIdentity, context: ContextId) -> SignedEventV1 {
    who.create_event(context, vec![], "context.genesis", json!({}))
        .unwrap()
}
#[allow(dead_code)] // each integration test binary uses a subset of these helpers
pub fn child(
    who: &SigningIdentity,
    context: ContextId,
    parent: EventId,
    value: i64,
) -> SignedEventV1 {
    who.create_event(
        context,
        vec![parent],
        "agent.request",
        json!({"value":value}),
    )
    .unwrap()
}
#[allow(dead_code)] // each integration test binary uses a subset of these helpers
pub async fn provision(store: &Store, event: &SignedEventV1, authors: Vec<AuthorId>) {
    store
        .provision_context(ContextProvision {
            context: event.body().context(),
            expected_genesis: event.event_id(),
            authorized_authors: sorted_authors(authors),
        })
        .await
        .unwrap();
}
#[allow(dead_code)] // each integration test binary uses a subset of these helpers
pub fn main_cas(context: ContextId, expected: RefExpectation, head: EventId) -> RefMutation {
    RefMutation::CompareAndSwap {
        context,
        name: "main".parse::<LocalRefName>().unwrap(),
        expected,
        new_head: head,
    }
}
