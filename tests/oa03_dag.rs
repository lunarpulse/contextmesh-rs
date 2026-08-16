#![allow(dead_code)]
mod common;
use common::*;
use contextmesh::error::StoreError;
use contextmesh::store::{ProjectionLimits, RefMutation, Store};
use serde_json::json;

#[tokio::test(flavor = "current_thread")]
async fn create_join_append_fork_merge_project_and_restart() {
    let db = path("oa03-dag");
    let alice = identity(31);
    let store = Store::open(&db).await.unwrap();
    let created = store
        .create_context(&alice, "main".parse().unwrap())
        .await
        .unwrap();
    let ctx = created.context;
    let root = created.genesis.event_id();
    assert_eq!(created.branch.head, root);
    store
        .create_branch(ctx, "side".parse().unwrap(), root)
        .await
        .unwrap();
    let left = store
        .append(
            &alice,
            ctx,
            "main".parse().unwrap(),
            root,
            "agent.request",
            json!({"side":"left"}),
        )
        .await
        .unwrap();
    let right = store
        .append(
            &alice,
            ctx,
            "side".parse().unwrap(),
            root,
            "agent.request",
            json!({"side":"right"}),
        )
        .await
        .unwrap();
    let merged = store
        .merge(
            &alice,
            ctx,
            "main".parse().unwrap(),
            left.event_id(),
            vec![right.event_id(), left.event_id()],
            json!({}),
        )
        .await
        .unwrap();
    let projection = store
        .project(ctx, vec![merged.event_id()], ProjectionLimits::default())
        .await
        .unwrap();
    assert_eq!(projection.events.len(), 4);
    assert_eq!(projection.events[0].event_id(), root);
    assert_eq!(projection.events[3].event_id(), merged.event_id());
    let mut middle = [left.event_id(), right.event_id()];
    middle.sort();
    assert_eq!(projection.events[1].event_id(), middle[0]);
    assert_eq!(projection.events[2].event_id(), middle[1]);
    drop(store);
    let reopened = Store::open(&db).await.unwrap();
    assert_eq!(
        reopened
            .project(ctx, vec![merged.event_id()], ProjectionLimits::default())
            .await
            .unwrap()
            .events
            .len(),
        4
    );
    let join_db = path("oa03-join");
    let join = Store::open(&join_db).await.unwrap();
    join.join_context(contextmesh::store::ContextProvision {
        context: ctx,
        expected_genesis: root,
        authorized_authors: vec![alice.author()],
    })
    .await
    .unwrap();
    assert!(join.event(root).await.unwrap().is_none());
    let _ = std::fs::remove_file(db);
    let _ = std::fs::remove_file(join_db);
}

#[tokio::test(flavor = "current_thread")]
async fn merge_boundaries_and_invalid_shapes_are_atomic() {
    let db = path("oa03-merge");
    let alice = identity(32);
    let store = Store::open(&db).await.unwrap();
    let created = store
        .create_context(&alice, "main".parse().unwrap())
        .await
        .unwrap();
    let ctx = created.context;
    let root = created.genesis.event_id();
    let mut parents = vec![root];
    for value in 0..63 {
        let event = alice
            .create_event(ctx, vec![root], "agent.request", json!({"value":value}))
            .unwrap();
        store.admit(&event, RefMutation::None).await.unwrap();
        parents.push(event.event_id());
    }
    let merged = store
        .merge(
            &alice,
            ctx,
            "main".parse().unwrap(),
            root,
            parents.clone(),
            json!({}),
        )
        .await
        .unwrap();
    assert_eq!(merged.body().parents().len(), 64);
    let before = store
        .local_ref(ctx, &"main".parse().unwrap())
        .await
        .unwrap();
    assert_eq!(
        store
            .merge(
                &alice,
                ctx,
                "main".parse().unwrap(),
                merged.event_id(),
                vec![root],
                json!({})
            )
            .await
            .unwrap_err(),
        StoreError::InvalidMerge
    );
    assert_eq!(
        store
            .merge(
                &alice,
                ctx,
                "main".parse().unwrap(),
                merged.event_id(),
                vec![root, root],
                json!({})
            )
            .await
            .unwrap_err(),
        StoreError::InvalidMerge
    );
    assert_eq!(
        store
            .local_ref(ctx, &"main".parse().unwrap())
            .await
            .unwrap(),
        before
    );
    assert_eq!(
        store
            .append(
                &alice,
                ctx,
                "main".parse().unwrap(),
                merged.event_id(),
                "context.merge",
                json!({})
            )
            .await
            .unwrap_err(),
        StoreError::ReservedEventKind
    );
    let _ = std::fs::remove_file(db);
}
