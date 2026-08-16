mod common;
use common::*;
use contextmesh::store::{AdmissionStatus, PeerName, RefExpectation};

#[tokio::test(flavor = "current_thread")]
async fn lifecycle_idempotence_namespaces_and_restart() {
    let db = path("lifecycle");
    let alice = identity(1);
    let bob = identity(2);
    let ctx = context(9);
    let root = genesis(&alice, ctx);
    let store = contextmesh::store::Store::open(&db).await.unwrap();
    provision(&store, &root, vec![alice.author()]).await;
    assert_eq!(
        store
            .admit(
                &root,
                main_cas(ctx, RefExpectation::Absent, root.event_id())
            )
            .await
            .unwrap(),
        AdmissionStatus::Inserted
    );
    assert_eq!(
        store
            .admit(
                &root,
                main_cas(ctx, RefExpectation::Absent, root.event_id())
            )
            .await
            .unwrap(),
        AdmissionStatus::AlreadyApplied
    );
    assert!(store.authorize_author(ctx, bob.author()).await.unwrap());
    assert!(!store.authorize_author(ctx, bob.author()).await.unwrap());
    store
        .provision_context(contextmesh::store::ContextProvision {
            context: ctx,
            expected_genesis: root.event_id(),
            authorized_authors: sorted_authors(vec![alice.author(), bob.author()]),
        })
        .await
        .unwrap();
    let next = child(&bob, ctx, root.event_id(), 1);
    assert_eq!(
        store
            .admit(
                &next,
                main_cas(ctx, RefExpectation::Head(root.event_id()), next.event_id())
            )
            .await
            .unwrap(),
        AdmissionStatus::Inserted
    );
    assert_eq!(
        store
            .event(next.event_id())
            .await
            .unwrap()
            .unwrap()
            .to_wire()
            .unwrap(),
        next.to_wire().unwrap()
    );
    let peer: PeerName = "node-b".parse().unwrap();
    store
        .set_remote_ref(peer.clone(), ctx, "main".parse().unwrap(), root.event_id())
        .await
        .unwrap();
    assert_eq!(
        store
            .local_ref(ctx, &"main".parse().unwrap())
            .await
            .unwrap(),
        Some(next.event_id())
    );
    assert_eq!(store.list_local_refs(ctx).await.unwrap().len(), 1);
    assert_eq!(
        store.list_remote_refs(Some(&peer), ctx).await.unwrap()[0].head,
        root.event_id()
    );
    drop(store);
    let reopened = contextmesh::store::Store::open(&db).await.unwrap();
    assert_eq!(
        reopened
            .local_ref(ctx, &"main".parse().unwrap())
            .await
            .unwrap(),
        Some(next.event_id())
    );
    assert_eq!(reopened.list_remote_refs(None, ctx).await.unwrap().len(), 1);
    assert!(reopened.event(root.event_id()).await.unwrap().is_some());
    let _ = std::fs::remove_file(db);
}

#[test]
fn names_are_strict_and_bounded() {
    for bad in ["", "Main", "main/branch", "main..x", "main_", "a+b"] {
        assert!(bad.parse::<contextmesh::store::LocalRefName>().is_err());
    }
    assert!(
        format!("a{}", "0".repeat(63))
            .parse::<contextmesh::store::PeerName>()
            .is_ok()
    );
    assert!(
        format!("a{}", "0".repeat(64))
            .parse::<contextmesh::store::PeerName>()
            .is_err()
    );
}
