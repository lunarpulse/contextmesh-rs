#![allow(dead_code)]
mod common;
use common::*;
use contextmesh::store::{Store, VerificationCategory, VerificationLimits};
use serde_json::json;
use turso::params;

#[tokio::test(flavor = "current_thread")]
async fn full_verify_passes_restart_and_reports_corruption_without_repair() {
    let db = path("oa03-verify");
    let alice = identity(35);
    let store = Store::open(&db).await.unwrap();
    let created = store
        .create_context(&alice, "main".parse().unwrap())
        .await
        .unwrap();
    let ctx = created.context;
    let child = store
        .append(
            &alice,
            ctx,
            "main".parse().unwrap(),
            created.genesis.event_id(),
            "agent.request",
            json!({}),
        )
        .await
        .unwrap();
    drop(store);
    let reopened = Store::open(&db).await.unwrap();
    let good = reopened
        .verify_full(VerificationLimits::default())
        .await
        .unwrap();
    assert!(good.valid);
    assert_eq!(good.checked_contexts, 1);
    assert_eq!(good.checked_events, 2);
    assert_eq!(good.checked_refs, 1);
    let raw = turso::Builder::new_local(db.to_str().unwrap())
        .build()
        .await
        .unwrap();
    let conn = raw.connect().unwrap();
    conn.execute_batch("DROP TRIGGER events_no_update;")
        .await
        .unwrap();
    conn.execute(
        "UPDATE events SET kind='agent.error' WHERE event_id=?1",
        params![child.event_id().to_bytes().to_vec()],
    )
    .await
    .unwrap();
    let report = reopened
        .verify_full(VerificationLimits::default())
        .await
        .unwrap();
    assert!(!report.valid);
    assert!(report.findings.iter().any(|finding| matches!(
        finding.category,
        VerificationCategory::Schema | VerificationCategory::EventColumns
    )));
    assert_eq!(
        reopened.event(child.event_id()).await.unwrap_err(),
        contextmesh::error::StoreError::CorruptStorage
    );
    let _ = std::fs::remove_file(db);
}
