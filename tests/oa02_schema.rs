#![allow(dead_code)]
mod common;

use common::*;
use contextmesh::error::StoreError;
use contextmesh::model::EventId;
use contextmesh::store::{ContextProvision, RefMutation, Store};
use turso::params;

#[tokio::test(flavor = "current_thread")]
async fn newer_and_incomplete_schemas_fail_closed() {
    let newer = path("newer-schema");
    let db = turso::Builder::new_local(newer.to_str().unwrap())
        .build()
        .await
        .unwrap();
    let conn = db.connect().unwrap();
    conn.execute_batch("CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL); INSERT INTO metadata VALUES('schema_version','99');").await.unwrap();
    drop(conn);
    drop(db);
    assert!(matches!(
        Store::open(&newer).await,
        Err(StoreError::NewerSchema)
    ));

    let incomplete = path("incomplete-schema");
    let db = turso::Builder::new_local(incomplete.to_str().unwrap())
        .build()
        .await
        .unwrap();
    let conn = db.connect().unwrap();
    conn.execute_batch("CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL); INSERT INTO metadata VALUES('schema_version','1');").await.unwrap();
    drop(conn);
    drop(db);
    assert!(matches!(
        Store::open(&incomplete).await,
        Err(StoreError::MigrationFailed)
    ));
    let _ = std::fs::remove_file(newer);
    let _ = std::fs::remove_file(incomplete);
}

#[tokio::test(flavor = "current_thread")]
async fn database_triggers_protect_immutable_rows() {
    let file = path("immutable");
    let alice = identity(21);
    let ctx = context(21);
    let root = genesis(&alice, ctx);
    let store = Store::open(&file).await.unwrap();
    provision(&store, &root, vec![alice.author()]).await;
    store.admit(&root, RefMutation::None).await.unwrap();
    drop(store);

    let db = turso::Builder::new_local(file.to_str().unwrap())
        .build()
        .await
        .unwrap();
    let conn = db.connect().unwrap();
    conn.execute("PRAGMA foreign_keys=ON", ()).await.unwrap();
    assert!(
        conn.execute(
            "UPDATE events SET kind='changed' WHERE event_id=?1",
            params![root.event_id().to_bytes().to_vec()]
        )
        .await
        .is_err()
    );
    assert!(
        conn.execute(
            "DELETE FROM events WHERE event_id=?1",
            params![root.event_id().to_bytes().to_vec()]
        )
        .await
        .is_err()
    );
    assert!(
        conn.execute(
            "DELETE FROM authorized_authors WHERE context_id=?1",
            params![ctx.to_bytes().to_vec()]
        )
        .await
        .is_err()
    );
    let _ = std::fs::remove_file(file);
}

#[tokio::test(flavor = "current_thread")]
async fn provisioning_mismatch_and_external_collision_are_typed() {
    let file = path("collision");
    let alice = identity(22);
    let _bob = identity(23);
    let ctx = context(22);
    let root = genesis(&alice, ctx);
    let store = Store::open(&file).await.unwrap();
    provision(&store, &root, vec![alice.author()]).await;
    let mismatch = ContextProvision {
        context: ctx,
        expected_genesis: EventId::from_bytes([77; 32]),
        authorized_authors: sorted_authors(vec![alice.author()]),
    };
    assert_eq!(
        store.provision_context(mismatch).await.unwrap_err(),
        StoreError::ContextProvisionMismatch
    );

    let bad_ref = contextmesh::store::RefMutation::CompareAndSwap {
        context: ctx,
        name: "main".parse().unwrap(),
        expected: contextmesh::store::RefExpectation::Absent,
        new_head: EventId::from_bytes([66; 32]),
    };
    assert_eq!(
        store.admit(&root, bad_ref).await.unwrap_err(),
        StoreError::RefMutationMismatch
    );

    let too_many = contextmesh::store::ContextProvision {
        context: context(33),
        expected_genesis: EventId::from_bytes([33; 32]),
        authorized_authors: (0..=contextmesh::store::MAX_AUTHORIZED_AUTHORS)
            .map(|index| {
                contextmesh::model::AuthorId::from_bytes(
                    (index as u64).to_be_bytes().repeat(4).try_into().unwrap(),
                )
            })
            .collect(),
    };
    assert_eq!(
        store.provision_context(too_many).await.unwrap_err(),
        StoreError::LimitExceeded
    );

    let db = turso::Builder::new_local(file.to_str().unwrap())
        .build()
        .await
        .unwrap();
    let conn = db.connect().unwrap();
    conn.execute("PRAGMA foreign_keys=ON", ()).await.unwrap();
    conn.execute("INSERT INTO events(event_id,context_id,author_id,kind,canonical_wire) VALUES(?1,?2,?3,?4,?5)", params![root.event_id().to_bytes().to_vec(), ctx.to_bytes().to_vec(), alice.author().to_bytes().to_vec(), "context.genesis", b"not-json".to_vec()]).await.unwrap();
    assert_eq!(
        store.admit(&root, RefMutation::None).await.unwrap_err(),
        StoreError::EventCollision
    );
    assert!(matches!(
        store.event(root.event_id()).await,
        Err(StoreError::CorruptStorage)
    ));
    let _ = std::fs::remove_file(file);
}
