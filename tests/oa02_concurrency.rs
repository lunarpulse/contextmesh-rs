mod common;
use common::*;
use contextmesh::error::StoreError;
use contextmesh::store::RefExpectation;

#[tokio::test(flavor = "current_thread")]
async fn independently_opened_stores_produce_one_cas_winner() {
    let db = path("race");
    let a = identity(7);
    let ctx = context(13);
    let root = genesis(&a, ctx);
    let first = contextmesh::store::Store::open(&db).await.unwrap();
    provision(&first, &root, vec![a.author()]).await;
    first
        .admit(
            &root,
            main_cas(ctx, RefExpectation::Absent, root.event_id()),
        )
        .await
        .unwrap();
    let second = contextmesh::store::Store::open(&db).await.unwrap();
    let left = child(&a, ctx, root.event_id(), 1);
    let right = child(&a, ctx, root.event_id(), 2);
    let l = first.admit(
        &left,
        main_cas(ctx, RefExpectation::Head(root.event_id()), left.event_id()),
    );
    let r = second.admit(
        &right,
        main_cas(ctx, RefExpectation::Head(root.event_id()), right.event_id()),
    );
    let (lr, rr) = tokio::join!(l, r);
    assert_eq!(usize::from(lr.is_ok()) + usize::from(rr.is_ok()), 1);
    let loser = match (lr, rr) {
        (Err(error), Ok(_)) | (Ok(_), Err(error)) => error,
        _ => panic!("exactly one concurrent admission must succeed"),
    };
    assert!(matches!(loser, StoreError::StaleHead { current: Some(_) }));
    let head = first
        .local_ref(ctx, &"main".parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert!(head == left.event_id() || head == right.event_id());
    let _ = std::fs::remove_file(db);
}
