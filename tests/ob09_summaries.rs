//! OB-09 summary matrix: derived, verifiable hierarchical summaries (event →
//! ref → project) as content-addressed references over Option A history. A
//! summary references exactly the events it summarizes, verifies against the
//! DAG, and a tampered or drifted summary is rejected (gate B9).

use contextmesh::closure::ClosureLimits;
use contextmesh::crypto::SigningIdentity;
use contextmesh::model::{ContextId, EventId};
use contextmesh::store::{LocalRefName, RefExpectation, RefMutation, Store};
use contextmesh::summary::{Summary, SummaryError, SummaryLevel, SummaryPayload};
use serde_json::json;

mod common;
use common::{context, identity, main_cas, path, provision};

const LIMITS: ClosureLimits = ClosureLimits {
    max_events: 100_000,
    max_exported_bytes: 64 * 1024 * 1024,
};

/// Builds a deterministic linear chain: genesis, then `depth` single-parent
/// children, each the new main head. Returns (store, author, context, ids).
async fn chain_store(depth: usize) -> (Store, SigningIdentity, ContextId, Vec<EventId>) {
    let db = path("ob09-chain");
    let store = Store::open(&db).await.unwrap();
    let author = identity(7);
    let ctx = context(8);
    let genesis_event = author
        .create_event(ctx, vec![], "context.genesis", json!({"note": "root"}))
        .unwrap();
    provision(&store, &genesis_event, vec![author.author()]).await;
    store
        .admit(&genesis_event, RefMutation::None)
        .await
        .unwrap();
    let mut ids = vec![genesis_event.event_id()];
    let mut head = genesis_event.event_id();
    for step in 1..=depth {
        let event = author
            .create_event(
                ctx,
                vec![head],
                "agent.request",
                json!({"value": step, "note": format!("step {step}")}),
            )
            .unwrap();
        let expected = if step == 1 {
            RefExpectation::Absent
        } else {
            RefExpectation::Head(head)
        };
        store
            .admit(&event, main_cas(ctx, expected, event.event_id()))
            .await
            .unwrap();
        ids.push(event.event_id());
        head = event.event_id();
    }
    (store, author, ctx, ids)
}

/// A compare-and-swap mutation against a named local ref.
fn ref_cas(context: ContextId, name: &str, expected: RefExpectation, head: EventId) -> RefMutation {
    RefMutation::CompareAndSwap {
        context,
        name: name.parse::<LocalRefName>().unwrap(),
        expected,
        new_head: head,
    }
}

fn main_ref() -> LocalRefName {
    "main".parse().unwrap()
}

#[tokio::test]
async fn event_summary_verifies_and_references_exactly_its_event() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let (_genesis, c1, _c2, _c3) = (ids[0], ids[1], ids[2], ids[3]);

    let summary = Summary::event(&store, ctx, c1).await.unwrap();
    assert_eq!(summary.level(), SummaryLevel::Event);
    assert_eq!(summary.context(), ctx);
    // The summary references exactly the event it summarizes.
    assert_eq!(summary.covered(), vec![c1]);
    // The payload carries the event and its derived note.
    let SummaryPayload::Event {
        event, kind, note, ..
    } = &summary.payload
    else {
        panic!("expected an event summary payload");
    };
    assert_eq!(*event, c1);
    assert_eq!(kind, "agent.request");
    assert_eq!(note, "step 1");

    let report = summary.verify_against_dag(&store).await.unwrap();
    assert!(report.valid);
    assert_eq!(report.checked, 1);
    // The content address is self-consistent and canonical on the wire.
    assert!(summary.to_wire().is_ok());
}

#[tokio::test]
async fn ref_summary_verifies_and_references_exactly_its_ancestry() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let (genesis, c1, c2, c3) = (ids[0], ids[1], ids[2], ids[3]);

    let summary = Summary::ref_summary(&store, ctx, &main_ref(), &LIMITS)
        .await
        .unwrap();
    assert_eq!(summary.level(), SummaryLevel::Ref);
    // The ref summary covers exactly the main ref's ancestry.
    let mut expected = vec![genesis, c1, c2, c3];
    expected.sort();
    assert_eq!(summary.covered(), expected);
    let SummaryPayload::Ref {
        ref_name,
        head,
        events,
        ..
    } = &summary.payload
    else {
        panic!("expected a ref summary payload");
    };
    assert_eq!(ref_name, "main");
    assert_eq!(*head, c3);
    assert_eq!(events, &expected);

    let report = summary.verify_against_dag(&store).await.unwrap();
    assert!(report.valid);
    assert_eq!(report.checked, 4);
}

#[tokio::test]
async fn project_summary_verifies_and_references_exactly_the_context() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let (genesis, c1, c2, c3) = (ids[0], ids[1], ids[2], ids[3]);

    let summary = Summary::project(&store, ctx, &LIMITS).await.unwrap();
    assert_eq!(summary.level(), SummaryLevel::Project);
    let mut expected = vec![genesis, c1, c2, c3];
    expected.sort();
    assert_eq!(summary.covered(), expected);
    let SummaryPayload::Project { events, note, .. } = &summary.payload else {
        panic!("expected a project summary payload");
    };
    assert_eq!(events, &expected);
    assert!(note.starts_with("project ctx1_"));

    let report = summary.verify_against_dag(&store).await.unwrap();
    assert!(report.valid);
    assert_eq!(report.checked, 4);
}

#[tokio::test]
async fn tampered_summary_is_rejected() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let c1 = ids[1];
    let summary = Summary::event(&store, ctx, c1).await.unwrap();

    // Tamper the content address: the payload no longer matches the id.
    let mut tampered = summary.clone();
    let mut bytes = tampered.summary_id.to_bytes();
    bytes[0] ^= 0xFF;
    tampered.summary_id = contextmesh::summary::SummaryId::from_bytes(bytes);
    let error = tampered.verify_against_dag(&store).await.unwrap_err();
    assert!(matches!(error, SummaryError::Tampered { .. }));

    // Tamper the payload (the derived note): the id no longer matches.
    let mut tampered = summary.clone();
    tampered.payload = SummaryPayload::Event {
        context: ctx,
        event: c1,
        kind: "agent.request".to_owned(),
        note: "tampered note".to_owned(),
    };
    let error = tampered.verify_against_dag(&store).await.unwrap_err();
    assert!(matches!(error, SummaryError::Tampered { .. }));
}

#[tokio::test]
async fn drifted_summary_is_rejected() {
    let (store, _author, ctx, _ids) = chain_store(3).await;
    let summary = Summary::ref_summary(&store, ctx, &main_ref(), &LIMITS)
        .await
        .unwrap();

    // Verify against a fresh store that has none of the referenced events:
    // the content address is self-consistent, but the references drifted.
    let db = path("ob09-empty");
    let empty = Store::open(&db).await.unwrap();
    let error = summary.verify_against_dag(&empty).await.unwrap_err();
    assert!(matches!(error, SummaryError::Drifted { .. }));
}

#[tokio::test]
async fn summaries_are_content_addressed_and_deterministic() {
    let run_once = || async {
        let (store, _author, ctx, _ids) = chain_store(3).await;
        let summary = Summary::ref_summary(&store, ctx, &main_ref(), &LIMITS)
            .await
            .unwrap();
        (summary.summary_id(), summary.to_wire().unwrap())
    };
    let (first_id, first_wire) = run_once().await;
    let (second_id, second_wire) = run_once().await;
    assert_eq!(first_id, second_id);
    assert_eq!(first_wire, second_wire);
    // The content address is a stable base64url text identity.
    let text = first_id.to_string();
    assert!(text.starts_with("sum1_"));
    assert_eq!(
        text.parse::<contextmesh::summary::SummaryId>().unwrap(),
        first_id
    );
}

#[tokio::test]
async fn project_summary_covers_the_union_of_local_refs() {
    let (store, author, ctx, ids) = chain_store(3).await;
    let (genesis, c1, _c2, _c3) = (ids[0], ids[1], ids[2], ids[3]);

    // A second branch ("backup") with its own head outside the main ancestry.
    let backup = author
        .create_event(
            ctx,
            vec![genesis],
            "agent.request",
            json!({"note": "backup head"}),
        )
        .unwrap();
    store
        .admit(
            &backup,
            ref_cas(ctx, "backup", RefExpectation::Absent, backup.event_id()),
        )
        .await
        .unwrap();

    let summary = Summary::project(&store, ctx, &LIMITS).await.unwrap();
    let mut expected = vec![genesis, c1, backup.event_id()];
    // The project also covers main's full ancestry.
    expected.extend_from_slice(&ids[2..]);
    expected.sort();
    expected.dedup();
    assert_eq!(summary.covered(), expected);
    assert_eq!(summary.covered().len(), 5);
    summary.verify_against_dag(&store).await.unwrap();
}

#[tokio::test]
async fn hierarchy_event_ref_project_nest_by_coverage() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let (genesis, c1, c2, c3) = (ids[0], ids[1], ids[2], ids[3]);

    let event = Summary::event(&store, ctx, c1).await.unwrap();
    let reference = Summary::ref_summary(&store, ctx, &main_ref(), &LIMITS)
        .await
        .unwrap();
    let project = Summary::project(&store, ctx, &LIMITS).await.unwrap();

    let event_covered = event.covered();
    let ref_covered = reference.covered();
    let project_covered = project.covered();
    // The hierarchy nests: event ⊆ ref ⊆ project.
    assert!(ref_covered.iter().all(|e| project_covered.contains(e)));
    assert!(event_covered.iter().all(|e| ref_covered.contains(e)));
    assert_eq!(event_covered, vec![c1]);
    let mut expected = vec![genesis, c1, c2, c3];
    expected.sort();
    assert_eq!(ref_covered, expected);
    assert_eq!(project_covered, expected);
}

#[tokio::test]
async fn summary_wire_round_trips() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let c1 = ids[1];
    let summary = Summary::event(&store, ctx, c1).await.unwrap();

    let wire = summary.to_wire().unwrap();
    let parsed: Summary = serde_json::from_slice(&wire).unwrap();
    assert_eq!(parsed, summary);
    parsed.verify_against_dag(&store).await.unwrap();
}

#[tokio::test]
async fn unknown_ref_and_empty_context_fail_closed() {
    let (store, _author, ctx, _ids) = chain_store(2).await;

    // A ref that does not exist fails closed.
    let error = Summary::ref_summary(&store, ctx, &"nope".parse().unwrap(), &LIMITS)
        .await
        .unwrap_err();
    assert!(matches!(error, SummaryError::UnknownRef { .. }));

    // A context with no refs has nothing to summarize and fails closed.
    let empty_ctx = context(9);
    let error = Summary::project(&store, empty_ctx, &LIMITS)
        .await
        .unwrap_err();
    assert!(matches!(error, SummaryError::Empty));
}
