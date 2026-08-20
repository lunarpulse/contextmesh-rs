//! OB-03 closure matrix: parent closure, adversarial severance, critical/risk
//! coverage, bounds, determinism, and composition with OB-02 (gate B3).

use contextmesh::closure::{
    ClosureError, ClosureLimits, CriticalPolicy, DanglingEdge, EventNode, close_check, close_over,
    close_selection,
};
use contextmesh::model::{ContextId, EventId};
use contextmesh::receipt::{ReceiptBodyV1, RecipientStateV1, SignedReceiptV1, TaskRecordV1};
use contextmesh::selection::{BaselineSelector, SelectionBudget, select_sources};
use contextmesh::store::{RefExpectation, RefMutation, Store};
use serde_json::json;

mod common;
use common::{context, identity, main_cas, path, provision};

const TASK: &str = "summarize the request chain";
const CREATED_AT: &str = "2026-08-17T00:00:00Z";
const LIMITS: ClosureLimits = ClosureLimits {
    max_events: 100_000,
    max_exported_bytes: 64 * 1024 * 1024,
};

fn critical_policy() -> CriticalPolicy {
    CriticalPolicy::new(vec!["context.critical".to_owned()]).unwrap()
}

/// Builds a deterministic linear chain: genesis, then `depth` single-parent
/// children, each the new main head. Returns (store, author, context, ids).
async fn chain_store(
    depth: usize,
) -> (
    Store,
    contextmesh::crypto::SigningIdentity,
    ContextId,
    Vec<EventId>,
) {
    let db = path("ob03-chain");
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

/// Builds genesis + one `agent.request` child + one `context.critical` event.
async fn critical_store() -> (
    Store,
    contextmesh::crypto::SigningIdentity,
    ContextId,
    EventId,
    EventId,
    EventId,
) {
    let db = path("ob03-critical");
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
    let child = author
        .create_event(
            ctx,
            vec![genesis_event.event_id()],
            "agent.request",
            json!({"note": "summarize the request chain"}),
        )
        .unwrap();
    store
        .admit(
            &child,
            main_cas(ctx, RefExpectation::Absent, child.event_id()),
        )
        .await
        .unwrap();
    let critical = author
        .create_event(
            ctx,
            vec![genesis_event.event_id()],
            "context.critical",
            json!({"note": "withheld critical fact"}),
        )
        .unwrap();
    store
        .admit(
            &critical,
            main_cas(
                ctx,
                RefExpectation::Head(child.event_id()),
                critical.event_id(),
            ),
        )
        .await
        .unwrap();
    (
        store,
        author,
        ctx,
        genesis_event.event_id(),
        child.event_id(),
        critical.event_id(),
    )
}

#[tokio::test]
async fn closure_reports_zero_dangling_on_valid_selection() {
    let (store, _author, ctx, ids) = chain_store(2).await;
    let closed = close_selection(&store, ctx, &ids, &ids, &critical_policy(), &LIMITS)
        .await
        .unwrap();
    assert_eq!(closed.references().len(), ids.len());
    assert!(closed.added_critical().is_empty());
    let mut actual: Vec<EventId> = closed.references().iter().map(|r| r.event()).collect();
    let mut expected = ids.clone();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
fn valid_nodes_report_zero_dangling() {
    let a = EventId::from_bytes([1; 32]);
    let b = EventId::from_bytes([2; 32]);
    let c = EventId::from_bytes([3; 32]);
    let report = close_check(
        &[
            EventNode {
                event: a,
                parents: vec![],
            },
            EventNode {
                event: b,
                parents: vec![a],
            },
            EventNode {
                event: c,
                parents: vec![b],
            },
        ],
        &LIMITS,
    )
    .unwrap();
    assert!(report.dangling.is_empty());
    assert!(!report.cycle);
    assert_eq!(report.closed.len(), 3);
}

#[tokio::test]
async fn closure_includes_all_ancestors() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let last = ids.last().copied().unwrap();
    let closed = close_selection(&store, ctx, &[last], &[last], &critical_policy(), &LIMITS)
        .await
        .unwrap();
    let mut actual: Vec<EventId> = closed.references().iter().map(|r| r.event()).collect();
    let mut expected = ids.clone();
    actual.sort();
    expected.sort();
    assert_eq!(
        actual, expected,
        "closure of the last event must include every ancestor"
    );
}

#[test]
fn deliberately_severed_parent_rejected() {
    let child = EventId::from_bytes([1; 32]);
    let missing = EventId::from_bytes([2; 32]);
    let nodes = [EventNode {
        event: child,
        parents: vec![missing],
    }];
    let err = close_check(&nodes, &LIMITS).unwrap_err();
    assert_eq!(
        err,
        ClosureError::DanglingParent {
            child,
            parent: missing,
        }
    );
    let report = close_over(&nodes, &LIMITS);
    assert_eq!(
        report.dangling,
        vec![DanglingEdge {
            child,
            parent: missing,
        }]
    );
    // The child node itself is still part of the (partial) closed set; only
    // the unresolvable parent edge is flagged.
    assert_eq!(report.closed, vec![child]);
}

#[test]
fn multiple_dangling_edges_all_reported() {
    let a = EventId::from_bytes([1; 32]);
    let b = EventId::from_bytes([2; 32]);
    let x = EventId::from_bytes([3; 32]);
    let y = EventId::from_bytes([4; 32]);
    let report = close_over(
        &[
            EventNode {
                event: a,
                parents: vec![x],
            },
            EventNode {
                event: b,
                parents: vec![y],
            },
        ],
        &LIMITS,
    );
    assert_eq!(report.dangling.len(), 2);
    assert_eq!(
        report.dangling[0],
        DanglingEdge {
            child: a,
            parent: x
        }
    );
    assert_eq!(
        report.dangling[1],
        DanglingEdge {
            child: b,
            parent: y
        }
    );
}

#[test]
fn cycle_rejected() {
    let a = EventId::from_bytes([1; 32]);
    let b = EventId::from_bytes([2; 32]);
    let err = close_check(
        &[
            EventNode {
                event: a,
                parents: vec![b],
            },
            EventNode {
                event: b,
                parents: vec![a],
            },
        ],
        &LIMITS,
    )
    .unwrap_err();
    assert_eq!(err, ClosureError::Cycle);
}

#[tokio::test]
async fn critical_events_are_added() {
    let (store, _author, ctx, genesis, child, critical) = critical_store().await;
    let closed = close_selection(
        &store,
        ctx,
        &[child],
        &[genesis, child, critical],
        &critical_policy(),
        &LIMITS,
    )
    .await
    .unwrap();
    let mut actual: Vec<EventId> = closed.references().iter().map(|r| r.event()).collect();
    let mut expected = vec![genesis, child, critical];
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    assert_eq!(closed.added_critical(), &[critical]);
}

#[tokio::test]
async fn critical_event_in_closure_not_double_counted() {
    let (store, _author, ctx, genesis, _child, critical) = critical_store().await;
    let closed = close_selection(
        &store,
        ctx,
        &[critical],
        &[genesis, critical],
        &critical_policy(),
        &LIMITS,
    )
    .await
    .unwrap();
    // Selecting the critical event itself: it is in the closure, so nothing is
    // reported as added-critical.
    assert!(closed.added_critical().is_empty());
    let mut actual: Vec<EventId> = closed.references().iter().map(|r| r.event()).collect();
    let mut expected = vec![genesis, critical];
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn empty_selection_closes_to_empty() {
    let (store, _author, ctx, _ids) = chain_store(1).await;
    let closed = close_selection(&store, ctx, &[], &[], &critical_policy(), &LIMITS)
        .await
        .unwrap();
    assert!(closed.references().is_empty());
    assert!(closed.added_critical().is_empty());
    assert_eq!(closed.total_bytes(), 0);
}

#[tokio::test]
async fn empty_selection_still_covers_critical() {
    let (store, _author, ctx, _genesis, _child, critical) = critical_store().await;
    let closed = close_selection(&store, ctx, &[], &[critical], &critical_policy(), &LIMITS)
        .await
        .unwrap();
    assert_eq!(closed.references().len(), 1);
    assert_eq!(closed.references()[0].event(), critical);
    assert_eq!(closed.added_critical(), &[critical]);
}

#[tokio::test]
async fn unverifiable_selected_source_fails_closed() {
    let (store, _author, ctx, _ids) = chain_store(1).await;
    let foreign = EventId::from_bytes([0xAB; 32]);
    let err = close_selection(&store, ctx, &[foreign], &[], &critical_policy(), &LIMITS)
        .await
        .unwrap_err();
    assert_eq!(err, ClosureError::UnverifiableSource { event: foreign });
}

#[tokio::test]
async fn wrong_context_selected_event_fails_closed() {
    let db = path("ob03-contexts");
    let store = Store::open(&db).await.unwrap();
    let author = identity(7);
    let ctx_a = context(8);
    let ctx_b = context(9);
    let genesis_a = author
        .create_event(ctx_a, vec![], "context.genesis", json!({"note": "a"}))
        .unwrap();
    let genesis_b = author
        .create_event(ctx_b, vec![], "context.genesis", json!({"note": "b"}))
        .unwrap();
    provision(&store, &genesis_a, vec![author.author()]).await;
    provision(&store, &genesis_b, vec![author.author()]).await;
    store.admit(&genesis_a, RefMutation::None).await.unwrap();
    store.admit(&genesis_b, RefMutation::None).await.unwrap();
    let err = close_selection(
        &store,
        ctx_a,
        &[genesis_b.event_id()],
        &[],
        &critical_policy(),
        &LIMITS,
    )
    .await
    .unwrap_err();
    assert_eq!(
        err,
        ClosureError::WrongContext {
            event: genesis_b.event_id(),
        }
    );
}

#[tokio::test]
async fn closure_respects_event_limit() {
    let (store, _author, ctx, ids) = chain_store(5).await;
    let last = ids.last().copied().unwrap();
    let tight = ClosureLimits {
        max_events: 2,
        max_exported_bytes: 64 * 1024 * 1024,
    };
    let err = close_selection(&store, ctx, &[last], &[], &critical_policy(), &tight)
        .await
        .unwrap_err();
    assert_eq!(err, ClosureError::LimitExceeded);
}

#[tokio::test]
async fn closure_respects_byte_limit() {
    let (store, _author, ctx, ids) = chain_store(2).await;
    let last = ids.last().copied().unwrap();
    let tight = ClosureLimits {
        max_events: 100_000,
        max_exported_bytes: 8,
    };
    let err = close_selection(&store, ctx, &[last], &[], &critical_policy(), &tight)
        .await
        .unwrap_err();
    assert_eq!(err, ClosureError::LimitExceeded);
}

#[tokio::test]
async fn closure_is_deterministic() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let a = close_selection(&store, ctx, &ids, &ids, &critical_policy(), &LIMITS)
        .await
        .unwrap();
    let b = close_selection(&store, ctx, &ids, &ids, &critical_policy(), &LIMITS)
        .await
        .unwrap();
    assert_eq!(a.total_bytes(), b.total_bytes());
    assert_eq!(a.references(), b.references());
    assert_eq!(a.to_wire().unwrap(), b.to_wire().unwrap());
}

#[tokio::test]
async fn selected_set_is_normalized() {
    let (store, _author, ctx, ids) = chain_store(2).await;
    let scrambled = vec![ids[2], ids[1], ids[2], ids[0]];
    let mut sorted = scrambled.clone();
    sorted.sort();
    sorted.dedup();
    let a = close_selection(&store, ctx, &scrambled, &[], &critical_policy(), &LIMITS)
        .await
        .unwrap();
    let b = close_selection(&store, ctx, &sorted, &[], &critical_policy(), &LIMITS)
        .await
        .unwrap();
    assert_eq!(a.selected(), b.selected());
    assert_eq!(a.references(), b.references());
}

#[tokio::test]
async fn closed_references_carry_source_metadata() {
    let (store, _author, ctx, ids) = chain_store(1).await;
    let closed = close_selection(&store, ctx, &ids, &ids, &critical_policy(), &LIMITS)
        .await
        .unwrap();
    for reference in closed.references() {
        assert_eq!(reference.context(), ctx);
        assert!(!reference.kind().is_empty());
        assert!(reference.payload_bytes() > 0);
    }
}

#[tokio::test]
async fn composition_selection_then_closure_then_receipt() {
    let (store, author, ctx, genesis, child, critical) = critical_store().await;
    let budget = SelectionBudget {
        max_selected_events: 4,
        max_exported_bytes: 4096,
    };
    let candidates = vec![genesis, child, critical];
    let selection = select_sources(
        &store,
        TASK,
        None,
        &budget,
        &BaselineSelector::new(),
        &candidates,
    )
    .await
    .unwrap();
    let selected_ids: Vec<EventId> = selection.references().iter().map(|r| r.event()).collect();
    assert_eq!(
        selected_ids,
        vec![child],
        "only the matching child is selected"
    );

    let closed = close_selection(
        &store,
        ctx,
        &selected_ids,
        &candidates,
        &critical_policy(),
        &LIMITS,
    )
    .await
    .unwrap();
    assert_eq!(closed.added_critical(), &[critical]);

    let mut events: Vec<EventId> = closed.references().iter().map(|r| r.event()).collect();
    events.sort();
    let task = TaskRecordV1::from_verbatim(TASK.to_owned(), None).unwrap();
    let body = ReceiptBodyV1::new(
        ctx,
        events,
        task,
        RecipientStateV1::new(genesis),
        selection.selector().clone(),
        Vec::new(),
        selection.uncertainty().to_vec(),
        CREATED_AT.to_owned(),
        author.author(),
    )
    .unwrap();
    let receipt = SignedReceiptV1::issue(&author, body).unwrap();
    let report = receipt.verify_against_dag(&store).await.unwrap();
    assert!(report.valid, "findings: {:?}", report.findings);
    assert_eq!(report.checked_events, 4);
}

#[test]
fn critical_policy_validates() {
    assert_eq!(
        CriticalPolicy::new(vec![]).unwrap_err(),
        ClosureError::InvalidPolicy
    );
    assert_eq!(
        CriticalPolicy::new(vec!["context.critical".repeat(100)]).unwrap_err(),
        ClosureError::InvalidPolicy
    );
    let policy = CriticalPolicy::new(vec![
        "risk.flag".to_owned(),
        "context.critical".to_owned(),
        "context.critical".to_owned(),
    ])
    .unwrap();
    assert_eq!(policy.kinds().len(), 2);
    assert!(policy.is_critical("context.critical"));
    assert!(policy.is_critical("risk.flag"));
    assert!(!policy.is_critical("agent.request"));
}

#[test]
fn closure_limits_validate() {
    assert!(ClosureLimits::new(0, 1024).is_err());
    assert!(ClosureLimits::new(100, 0).is_err());
    assert!(ClosureLimits::new(100, 1024).is_ok());
    assert!(ClosureLimits::new(100_001, 1024).is_err());
}
