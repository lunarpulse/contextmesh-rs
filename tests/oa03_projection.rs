#![allow(dead_code)]
mod common;
use common::*;
use contextmesh::error::StoreError;
use contextmesh::store::{ProjectionLimits, Store};
use serde_json::json;

#[tokio::test(flavor = "current_thread")]
async fn iterative_projection_is_unique_and_strictly_bounded() {
    let db = path("oa03-deep");
    let alice = identity(33);
    let store = Store::open(&db).await.unwrap();
    let created = store
        .create_context(&alice, "main".parse().unwrap())
        .await
        .unwrap();
    let ctx = created.context;
    let mut head = created.genesis.event_id();
    for value in 0..256 {
        head = store
            .append(
                &alice,
                ctx,
                "main".parse().unwrap(),
                head,
                "agent.request",
                json!({"value":value}),
            )
            .await
            .unwrap()
            .event_id();
    }
    let full = store
        .project(ctx, vec![head], ProjectionLimits::default())
        .await
        .unwrap();
    assert_eq!(full.events.len(), 257);
    let exact = ProjectionLimits::new(full.events.len(), full.canonical_wire_bytes).unwrap();
    assert_eq!(
        store
            .project(ctx, vec![head], exact)
            .await
            .unwrap()
            .events
            .len(),
        257
    );
    let too_few = ProjectionLimits::new(full.events.len() - 1, full.canonical_wire_bytes).unwrap();
    assert_eq!(
        store.project(ctx, vec![head], too_few).await.unwrap_err(),
        StoreError::ProjectionLimitExceeded
    );
    let too_small =
        ProjectionLimits::new(full.events.len(), full.canonical_wire_bytes - 1).unwrap();
    assert_eq!(
        store.project(ctx, vec![head], too_small).await.unwrap_err(),
        StoreError::ProjectionLimitExceeded
    );
    assert_eq!(
        store
            .project(ctx, vec![], ProjectionLimits::default())
            .await
            .unwrap_err(),
        StoreError::ProjectionLimitExceeded
    );
    let _ = std::fs::remove_file(db);
}
