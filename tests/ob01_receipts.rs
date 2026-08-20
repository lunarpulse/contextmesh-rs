//! OB-01 receipt artifact matrix: golden fixture, round-trip, tamper
//! rejection, and DAG binding (gate B1).

use std::path::PathBuf;

use contextmesh::model::{AuthorId, ContextId, EventId};
use contextmesh::receipt::{
    ReceiptBodyV1, ReceiptDagReport, RecipientStateV1, SelectorRecordV1, SignedReceiptV1,
    TaskRecordV1, export_receipt, import_receipt,
};
use contextmesh::store::{RefExpectation, RefMutation, Store};

mod common;
use common::{child, context, genesis, identity, main_cas, path, provision};

const FIXTURE: &str = "tests/fixtures/ob01-receipt-golden.json";
const FIXTURE_TASK: &str = "summarize the request chain";
const FIXTURE_CREATED_AT: &str = "2026-08-17T00:00:00Z";
const FIXTURE_SELECTOR_IDENTITY: &str = "ob-baseline";
const FIXTURE_SELECTOR_VERSION: &str = "0.1.0";
const FIXTURE_SELECTOR_CONFIG_HASH: &str = "0123456789abcdef";

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)
}

/// The deterministic golden inputs: author A is identity 7, author B is 9,
/// context byte is 8, and B appends two children to A's genesis.
struct Golden {
    store: Store,
    author_a: contextmesh::crypto::SigningIdentity,
    author_b: contextmesh::crypto::SigningIdentity,
    context: ContextId,
    genesis: EventId,
    child1: EventId,
    child2: EventId,
}

async fn build_golden() -> Golden {
    let db = path("ob01");
    let store = Store::open(&db).await.unwrap();
    let author_a = identity(7);
    let author_b = identity(9);
    let ctx = context(8);
    let genesis_event = genesis(&author_a, ctx);
    let child1_event = child(&author_b, ctx, genesis_event.event_id(), 1);
    let child2_event = child(&author_b, ctx, genesis_event.event_id(), 2);
    provision(
        &store,
        &genesis_event,
        vec![author_a.author(), author_b.author()],
    )
    .await;
    store
        .admit(&genesis_event, RefMutation::None)
        .await
        .unwrap();
    store
        .admit(
            &child1_event,
            main_cas(ctx, RefExpectation::Absent, child1_event.event_id()),
        )
        .await
        .unwrap();
    store
        .admit(
            &child2_event,
            main_cas(
                ctx,
                RefExpectation::Head(child1_event.event_id()),
                child2_event.event_id(),
            ),
        )
        .await
        .unwrap();
    Golden {
        store,
        author_a,
        author_b,
        context: ctx,
        genesis: genesis_event.event_id(),
        child1: child1_event.event_id(),
        child2: child2_event.event_id(),
    }
}

fn golden_body(author: AuthorId, golden: &Golden) -> ReceiptBodyV1 {
    let mut events = vec![golden.genesis, golden.child1, golden.child2];
    events.sort();
    ReceiptBodyV1::new(
        golden.context,
        events,
        TaskRecordV1::from_verbatim(FIXTURE_TASK.to_owned(), None).unwrap(),
        RecipientStateV1::new(golden.genesis),
        SelectorRecordV1::new(
            FIXTURE_SELECTOR_IDENTITY.to_owned(),
            FIXTURE_SELECTOR_VERSION.to_owned(),
            FIXTURE_SELECTOR_CONFIG_HASH.to_owned(),
        )
        .unwrap(),
        Vec::new(),
        Vec::new(),
        FIXTURE_CREATED_AT.to_owned(),
        author,
    )
    .unwrap()
}

/// Regenerates the committed golden fixture from the deterministic inputs.
///
/// Ignored in CI: the fixture is committed and the non-ignored
/// `golden_fixture_matches_reconstruction` test asserts the committed bytes
/// still match this exact reconstruction.
#[test]
#[ignore]
fn regenerate_golden_fixture() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let golden = runtime.block_on(build_golden());
    let body = golden_body(golden.author_a.author(), &golden);
    let receipt = SignedReceiptV1::issue(&golden.author_a, body).unwrap();
    let wire = receipt.to_wire().unwrap();
    std::fs::write(fixture_path(), wire).unwrap();
}

#[tokio::test]
async fn golden_fixture_matches_reconstruction() {
    let golden = build_golden().await;
    let body = golden_body(golden.author_a.author(), &golden);
    let expected = SignedReceiptV1::issue(&golden.author_a, body).unwrap();
    let wire = std::fs::read(fixture_path()).unwrap();
    let fixture = SignedReceiptV1::from_wire(&wire).unwrap();
    assert_eq!(expected.receipt_id(), fixture.receipt_id());
    assert_eq!(expected.to_wire().unwrap(), wire);
}

#[tokio::test]
async fn golden_fixture_verifies_against_rebuilt_dag() {
    let golden = build_golden().await;
    let wire = std::fs::read(fixture_path()).unwrap();
    let receipt = SignedReceiptV1::from_wire(&wire).unwrap();
    let report: ReceiptDagReport = receipt.verify_against_dag(&golden.store).await.unwrap();
    assert!(report.valid, "findings: {:?}", report.findings);
    // Three referenced events plus the recipient head (genesis, checked twice).
    assert_eq!(report.checked_events, 4);
}

#[tokio::test]
async fn round_trip_preserves_receipt() {
    let golden = build_golden().await;
    let body = golden_body(golden.author_a.author(), &golden);
    let receipt = SignedReceiptV1::issue(&golden.author_a, body).unwrap();
    let wire = receipt.to_wire().unwrap();
    let parsed = SignedReceiptV1::from_wire(&wire).unwrap();
    parsed.verify().unwrap();
    assert_eq!(parsed.receipt_id(), receipt.receipt_id());
    assert_eq!(parsed.body().task().verbatim(), FIXTURE_TASK);
    assert_eq!(
        parsed.body().selector().identity(),
        FIXTURE_SELECTOR_IDENTITY
    );
    assert_eq!(parsed.body().created_at(), FIXTURE_CREATED_AT);
    let report = parsed.verify_against_dag(&golden.store).await.unwrap();
    assert!(report.valid);
}

#[tokio::test]
async fn export_import_round_trip() {
    let golden = build_golden().await;
    let body = golden_body(golden.author_a.author(), &golden);
    let receipt = SignedReceiptV1::issue(&golden.author_a, body).unwrap();
    let out = path("ob01-export");
    export_receipt(&receipt, &out).unwrap();
    let imported = import_receipt(&out).unwrap();
    assert_eq!(imported.receipt_id(), receipt.receipt_id());
    imported.verify().unwrap();
    let _ = std::fs::remove_file(&out);
}

fn tamper_wire(mut wire: Vec<u8>, target: &str, offset_from_end: usize) -> Vec<u8> {
    let text = String::from_utf8(wire.clone()).unwrap();
    let position = text.find(target).unwrap() + target.len();
    let byte_position = wire[..position].len();
    let index = byte_position + offset_from_end;
    let replacement = if wire[index] == b'A' { b'B' } else { b'A' };
    wire[index] = replacement;
    wire
}

#[tokio::test]
async fn tampered_signature_rejected() {
    let golden = build_golden().await;
    let _ = &golden;
    let wire = std::fs::read(fixture_path()).unwrap();
    let tampered = tamper_wire(wire, "\"signature\":\"rsig1_", 5);
    let parsed = SignedReceiptV1::from_wire(&tampered);
    assert!(parsed.is_err(), "mutated signature must fail verification");
}

#[tokio::test]
async fn tampered_receipt_id_rejected() {
    let golden = build_golden().await;
    let _ = &golden;
    let wire = std::fs::read(fixture_path()).unwrap();
    let tampered = tamper_wire(wire, "\"receipt_id\":\"rcpt1_", 5);
    let parsed = SignedReceiptV1::from_wire(&tampered);
    assert!(parsed.is_err(), "mutated receipt id must fail verification");
}

#[tokio::test]
async fn tampered_task_rejected() {
    let golden = build_golden().await;
    let _ = &golden;
    let wire = std::fs::read(fixture_path()).unwrap();
    // Mutate a letter inside the task verbatim string.
    let tampered = tamper_wire(wire, FIXTURE_TASK, 0);
    let parsed = SignedReceiptV1::from_wire(&tampered);
    assert!(parsed.is_err(), "mutated task must fail verification");
}

#[tokio::test]
async fn missing_event_reference_rejected() {
    let golden = build_golden().await;
    let body = golden_body(golden.author_a.author(), &golden);
    // Reference an event that is never admitted to the DAG.
    let phantom = child(&golden.author_b, golden.context, golden.genesis, 99);
    let mut events = vec![
        golden.genesis,
        golden.child1,
        golden.child2,
        phantom.event_id(),
    ];
    events.sort();
    let body = ReceiptBodyV1::new(
        golden.context,
        events,
        body.task().clone(),
        RecipientStateV1::new(golden.genesis),
        body.selector().clone(),
        Vec::new(),
        Vec::new(),
        body.created_at().to_owned(),
        golden.author_a.author(),
    )
    .unwrap();
    let receipt = SignedReceiptV1::issue(&golden.author_a, body).unwrap();
    let report = receipt.verify_against_dag(&golden.store).await.unwrap();
    assert!(!report.valid);
    assert!(report.findings.iter().any(|f| f.reason == "missing"));
}

#[tokio::test]
async fn cross_context_event_reference_rejected() {
    let golden = build_golden().await;
    // A second context admitted into the same store, so the foreign event
    // exists in the DAG but belongs to a different context.
    let other_author = identity(11);
    let other_ctx = context(10);
    let other_genesis = genesis(&other_author, other_ctx);
    provision(&golden.store, &other_genesis, vec![other_author.author()]).await;
    golden
        .store
        .admit(&other_genesis, RefMutation::None)
        .await
        .unwrap();
    let foreign = other_genesis.event_id();

    // A receipt in the golden context referencing the foreign event.
    let body = golden_body(golden.author_a.author(), &golden);
    let mut events = vec![golden.genesis, golden.child1, golden.child2, foreign];
    events.sort();
    let body = ReceiptBodyV1::new(
        golden.context,
        events,
        body.task().clone(),
        RecipientStateV1::new(golden.genesis),
        body.selector().clone(),
        Vec::new(),
        Vec::new(),
        body.created_at().to_owned(),
        golden.author_a.author(),
    )
    .unwrap();
    let receipt = SignedReceiptV1::issue(&golden.author_a, body).unwrap();
    let report = receipt.verify_against_dag(&golden.store).await.unwrap();
    assert!(!report.valid);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.reason == "wrong-context" && f.event == foreign)
    );
}

#[tokio::test]
async fn unknown_recipient_head_fails_closed() {
    let golden = build_golden().await;
    let body = golden_body(golden.author_a.author(), &golden);
    let phantom = child(&golden.author_b, golden.context, golden.genesis, 100);
    let body = ReceiptBodyV1::new(
        golden.context,
        body.events().to_vec(),
        body.task().clone(),
        RecipientStateV1::new(phantom.event_id()),
        body.selector().clone(),
        Vec::new(),
        Vec::new(),
        body.created_at().to_owned(),
        golden.author_a.author(),
    )
    .unwrap();
    let receipt = SignedReceiptV1::issue(&golden.author_a, body).unwrap();
    let report = receipt.verify_against_dag(&golden.store).await.unwrap();
    assert!(!report.valid);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.reason == "recipient-missing")
    );
}

#[tokio::test]
async fn duplicate_events_rejected() {
    let golden = build_golden().await;
    let body = golden_body(golden.author_a.author(), &golden);
    let mut events = vec![golden.genesis, golden.genesis];
    events.sort();
    let result = ReceiptBodyV1::new(
        golden.context,
        events,
        body.task().clone(),
        RecipientStateV1::new(golden.genesis),
        body.selector().clone(),
        Vec::new(),
        Vec::new(),
        body.created_at().to_owned(),
        golden.author_a.author(),
    );
    assert!(result.is_err());
}

#[tokio::test]
async fn unordered_events_rejected() {
    let golden = build_golden().await;
    let body = golden_body(golden.author_a.author(), &golden);
    let mut events = vec![golden.child1, golden.genesis];
    events.sort_by_key(|event| std::cmp::Reverse(event.to_string()));
    let result = ReceiptBodyV1::new(
        golden.context,
        events,
        body.task().clone(),
        RecipientStateV1::new(golden.genesis),
        body.selector().clone(),
        Vec::new(),
        Vec::new(),
        body.created_at().to_owned(),
        golden.author_a.author(),
    );
    assert!(result.is_err());
}

#[tokio::test]
async fn oversized_task_rejected() {
    let golden = build_golden().await;
    let _ = golden;
    let oversized = "x".repeat(contextmesh::receipt::MAX_TASK_BYTES + 1);
    let task = TaskRecordV1::from_verbatim(oversized, None);
    assert!(task.is_err());
}

#[tokio::test]
async fn malformed_created_at_rejected() {
    let golden = build_golden().await;
    let body = golden_body(golden.author_a.author(), &golden);
    let result = ReceiptBodyV1::new(
        golden.context,
        body.events().to_vec(),
        body.task().clone(),
        RecipientStateV1::new(golden.genesis),
        body.selector().clone(),
        Vec::new(),
        Vec::new(),
        "not-a-timestamp".to_owned(),
        golden.author_a.author(),
    );
    assert!(result.is_err());
}
