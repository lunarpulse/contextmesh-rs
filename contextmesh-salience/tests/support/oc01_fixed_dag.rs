//! Deterministic fixed admitted DAG for OC-01 committed golden fixtures.
//!
//! This is test support, not an integration test target. It creates signed
//! core events from published test-only seeds, provisions/adopts them into an
//! embedded store, and installs exactly one local and one all-peer remote ref.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use contextmesh::crypto::SigningIdentity;
use contextmesh::model::{ContextId, EventId};
use contextmesh::store::{
    ContextProvision, LocalRefName, PeerName, RefExpectation, RefMutation, Store,
};
use serde_json::json;

use contextmesh_salience::types::{InputRefSnapshotV1, OutcomeLimits};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// Fixed context seed for the committed OC-01 golden DAG.
pub const CONTEXT_BYTE: u8 = 0x31;
/// Fixed core-event author seed for the committed OC-01 golden DAG.
pub const EVENT_AUTHOR_SEED: [u8; 32] = [0x61; 32];

/// Deterministic admitted-DAG fixture returned to P09/P10 tests.
pub struct FixedDag {
    /// Store containing the fixed admitted DAG and refs.
    pub store: Store,
    /// Context owning every event and ref.
    pub context: ContextId,
    /// Genesis plus five admitted descendant event IDs in chain order.
    pub events: Vec<EventId>,
}

/// Returns the frozen test context.
pub fn context() -> ContextId {
    ContextId::from_bytes([CONTEXT_BYTE; 32])
}

fn path() -> PathBuf {
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "oc01-fixed-golden-dag-{}-{serial}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

/// Constructs the deterministic admitted DAG and exactly two fixed refs.
pub async fn build() -> FixedDag {
    let store = Store::open(path())
        .await
        .expect("fixed fixture store opens");
    let author = SigningIdentity::from_fixture_seed(EVENT_AUTHOR_SEED);
    let context = context();
    let genesis = author
        .create_event(
            context,
            vec![],
            "context.genesis",
            json!({"fixture":"oc01"}),
        )
        .expect("fixed genesis constructs");
    store
        .provision_context(ContextProvision {
            context,
            expected_genesis: genesis.event_id(),
            authorized_authors: vec![author.author()],
        })
        .await
        .expect("fixed context provisions");
    store
        .admit(
            &genesis,
            RefMutation::CompareAndSwap {
                context,
                name: "main"
                    .parse::<LocalRefName>()
                    .expect("fixed local ref parses"),
                expected: RefExpectation::Absent,
                new_head: genesis.event_id(),
            },
        )
        .await
        .expect("fixed genesis admits");

    let mut events = vec![genesis.event_id()];
    let mut parent = genesis.event_id();
    for ordinal in 1..=5_u8 {
        let event = author
            .create_event(
                context,
                vec![parent],
                "agent.request",
                json!({"fixture":"oc01", "ordinal":ordinal}),
            )
            .expect("fixed descendant constructs");
        store
            .admit(
                &event,
                RefMutation::CompareAndSwap {
                    context,
                    name: "main"
                        .parse::<LocalRefName>()
                        .expect("fixed local ref parses"),
                    expected: RefExpectation::Head(parent),
                    new_head: event.event_id(),
                },
            )
            .await
            .expect("fixed descendant admits");
        parent = event.event_id();
        events.push(parent);
    }
    store
        .set_remote_ref(
            "peer.example"
                .parse::<PeerName>()
                .expect("fixed peer parses"),
            context,
            "main"
                .parse::<LocalRefName>()
                .expect("fixed remote ref parses"),
            events[2],
        )
        .await
        .expect("fixed remote ref installs");
    FixedDag {
        store,
        context,
        events,
    }
}

/// Captures the exact fixed local + all-peer remote ref snapshot.
pub async fn snapshot(dag: &FixedDag) -> InputRefSnapshotV1 {
    InputRefSnapshotV1::capture(&dag.store, dag.context, OutcomeLimits::default())
        .await
        .expect("fixed snapshot captures")
}
