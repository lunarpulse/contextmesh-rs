mod common;
use common::*;
use contextmesh::error::StoreError;
use contextmesh::store::RefExpectation;

#[tokio::test(flavor = "current_thread")]
async fn policy_parent_and_stale_failures_leave_history_unchanged() {
    let db = path("rollback");
    let store = contextmesh::store::Store::open(&db).await.unwrap();
    let alice = identity(3);
    let mallory = identity(4);
    let ctx = context(10);
    let root = genesis(&alice, ctx);
    provision(&store, &root, vec![alice.author()]).await;
    store
        .admit(
            &root,
            main_cas(ctx, RefExpectation::Absent, root.event_id()),
        )
        .await
        .unwrap();
    let unauthorized = child(&mallory, ctx, root.event_id(), 2);
    assert_eq!(
        store
            .admit(
                &unauthorized,
                main_cas(
                    ctx,
                    RefExpectation::Head(root.event_id()),
                    unauthorized.event_id()
                )
            )
            .await
            .unwrap_err(),
        StoreError::UnauthorizedAuthor
    );
    assert!(
        store
            .event(unauthorized.event_id())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .local_ref(ctx, &"main".parse().unwrap())
            .await
            .unwrap(),
        Some(root.event_id())
    );
    let missing = alice
        .create_event(
            ctx,
            vec![contextmesh::model::EventId::from_bytes([99; 32])],
            "agent.request",
            serde_json::json!({}),
        )
        .unwrap();
    assert!(matches!(
        store
            .admit(&missing, contextmesh::store::RefMutation::None)
            .await
            .unwrap_err(),
        StoreError::ParentMissing(_)
    ));
    let good = child(&alice, ctx, root.event_id(), 3);
    let wrong_expected = contextmesh::model::EventId::from_bytes([88; 32]);
    assert!(matches!(
        store
            .admit(
                &good,
                main_cas(ctx, RefExpectation::Head(wrong_expected), good.event_id())
            )
            .await
            .unwrap_err(),
        StoreError::StaleHead { .. }
    ));
    assert!(store.event(good.event_id()).await.unwrap().is_none());
    assert_eq!(
        store
            .local_ref(ctx, &"main".parse().unwrap())
            .await
            .unwrap(),
        Some(root.event_id())
    );
    assert!(
        store
            .admit_wire(b"{bad", contextmesh::store::RefMutation::None)
            .await
            .is_err()
    );
    let _ = std::fs::remove_file(db);
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_genesis_and_cross_context_parent_reject() {
    let db = path("contexts");
    let store = contextmesh::store::Store::open(&db).await.unwrap();
    let a = identity(5);
    let c1 = context(11);
    let c2 = context(12);
    let g1 = genesis(&a, c1);
    let g2 = genesis(&a, c2);
    provision(&store, &g1, vec![a.author()]).await;
    let wrong = a
        .create_event(
            c1,
            vec![],
            "context.genesis",
            serde_json::json!({"wrong":true}),
        )
        .unwrap();
    assert_eq!(
        store
            .admit(&wrong, contextmesh::store::RefMutation::None)
            .await
            .unwrap_err(),
        StoreError::GenesisMismatch
    );
    store
        .admit(&g1, contextmesh::store::RefMutation::None)
        .await
        .unwrap();
    provision(&store, &g2, vec![a.author()]).await;
    store
        .admit(&g2, contextmesh::store::RefMutation::None)
        .await
        .unwrap();
    let cross = a
        .create_event(
            c1,
            vec![g2.event_id()],
            "agent.request",
            serde_json::json!({}),
        )
        .unwrap();
    assert!(matches!(
        store
            .admit(&cross, contextmesh::store::RefMutation::None)
            .await
            .unwrap_err(),
        StoreError::ParentContextMismatch(_)
    ));
    let _ = std::fs::remove_file(db);
}
