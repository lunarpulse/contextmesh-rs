#![allow(dead_code)]
mod common;
use common::*;
use contextmesh::error::StoreError;
use contextmesh::store::{BundleLimits, BundleV1, ContextProvision, PeerName, Store};
use serde_json::json;

#[tokio::test(flavor = "current_thread")]
async fn bundle_round_trip_parent_first_atomic_idempotent_and_remote_only() {
    let source_db = path("oa03-source");
    let target_db = path("oa03-target");
    let alice = identity(34);
    let source = Store::open(&source_db).await.unwrap();
    let created = source
        .create_context(&alice, "main".parse().unwrap())
        .await
        .unwrap();
    let ctx = created.context;
    let root = created.genesis.event_id();
    let child = source
        .append(
            &alice,
            ctx,
            "main".parse().unwrap(),
            root,
            "agent.request",
            json!({"value":1}),
        )
        .await
        .unwrap();
    let refs = source.list_local_refs(ctx).await.unwrap();
    let bundle = source
        .export_bundle(
            ctx,
            vec![child.event_id()],
            vec![],
            refs,
            BundleLimits::default(),
        )
        .await
        .unwrap();
    let wire = bundle.to_wire().unwrap();
    assert_eq!(BundleV1::from_wire(&wire).unwrap().to_wire().unwrap(), wire);
    assert_eq!(bundle.events().len(), 2);
    assert_eq!(bundle.events()[0].event_id(), root);
    let target = Store::open(&target_db).await.unwrap();
    target
        .join_context(ContextProvision {
            context: ctx,
            expected_genesis: root,
            authorized_authors: vec![alice.author()],
        })
        .await
        .unwrap();
    let peer: PeerName = "node-a".parse().unwrap();
    let first = target
        .import_bundle(peer.clone(), &wire, BundleLimits::default())
        .await
        .unwrap();
    assert_eq!(first.inserted, 2);
    assert_eq!(first.remote_refs_updated, 1);
    assert!(target.list_local_refs(ctx).await.unwrap().is_empty());
    let second = target
        .import_bundle(peer.clone(), &wire, BundleLimits::default())
        .await
        .unwrap();
    assert_eq!(second.inserted, 0);
    assert_eq!(second.already_present, 2);
    assert_eq!(second.remote_refs_updated, 0);
    let missing = Store::open(path("oa03-missing")).await.unwrap();
    missing
        .join_context(ContextProvision {
            context: ctx,
            expected_genesis: root,
            authorized_authors: vec![alice.author()],
        })
        .await
        .unwrap();
    let child_only = BundleV1::from_parts(ctx, vec![child.clone()], vec![])
        .unwrap()
        .to_wire()
        .unwrap();
    assert!(matches!(
        missing
            .import_bundle(peer, &child_only, BundleLimits::default())
            .await,
        Err(StoreError::GenesisMismatch) | Err(StoreError::ParentMissing(_))
    ));
    assert!(missing.event(child.event_id()).await.unwrap().is_none());
    let mut bad = wire.clone();
    let pos = bad.iter().position(|byte| *byte == b'1').unwrap();
    bad[pos] = b'9';
    assert!(BundleV1::from_wire(&bad).is_err());

    let rollback_db = path("oa03-atomic-rollback");
    let rollback = Store::open(&rollback_db).await.unwrap();
    rollback
        .join_context(ContextProvision {
            context: ctx,
            expected_genesis: root,
            authorized_authors: vec![alice.author()],
        })
        .await
        .unwrap();
    rollback
        .admit(&created.genesis, contextmesh::store::RefMutation::None)
        .await
        .unwrap();
    let valid = alice
        .create_event(ctx, vec![root], "agent.request", json!({"batch":1}))
        .unwrap();
    let mallory = identity(99);
    let unauthorized = mallory
        .create_event(
            ctx,
            vec![valid.event_id()],
            "agent.request",
            json!({"batch":2}),
        )
        .unwrap();
    let atomic_wire = BundleV1::from_parts(ctx, vec![valid.clone(), unauthorized.clone()], vec![])
        .unwrap()
        .to_wire()
        .unwrap();
    assert_eq!(
        rollback
            .import_bundle(
                "node-a".parse().unwrap(),
                &atomic_wire,
                BundleLimits::default()
            )
            .await
            .unwrap_err(),
        StoreError::UnauthorizedAuthor
    );
    assert!(rollback.event(valid.event_id()).await.unwrap().is_none());
    assert!(
        rollback
            .event(unauthorized.event_id())
            .await
            .unwrap()
            .is_none()
    );

    let _ = std::fs::remove_file(source_db);
    let _ = std::fs::remove_file(target_db);
    let _ = std::fs::remove_file(rollback_db);
}

#[test]
fn canonical_bundle_fixture_is_frozen_and_independently_verified() {
    let fixture = include_bytes!("fixtures/oa03-bundle-v1-golden.json");
    let bundle = BundleV1::from_wire(fixture).unwrap();
    assert_eq!(bundle.to_wire().unwrap(), fixture);
    assert_eq!(bundle.events().len(), 2);
    assert!(bundle.events().iter().all(|event| event.verify().is_ok()));
}

#[test]
fn bundle_parser_rejects_unknown_duplicate_version_order_and_limits() {
    assert_eq!(
        BundleV1::from_wire(br#"{"bundle_version":2,"context":"x","events":[],"refs":[]}"#)
            .unwrap_err(),
        StoreError::BundleUnsupportedVersion
    );
    assert_eq!(
        BundleV1::from_wire(
            br#"{"bundle_version":1,"bundle_version":1,"context":"x","events":[],"refs":[]}"#
        )
        .unwrap_err(),
        StoreError::BundleMalformed
    );
    assert_eq!(
        contextmesh::store::BundleLimits::new(0, 1, 1).unwrap_err(),
        StoreError::BundleLimitExceeded
    );
}
