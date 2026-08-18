//! OB-04 delta matrix: recipient-known-history delta, cold-start semantics,
//! fail-closed recipient states, provability, bounds, determinism, and
//! composition with OB-01/OB-02/OB-03 (gate B4).

use contextmesh::closure::{ClosureLimits, CriticalPolicy, close_selection};
use contextmesh::crypto::SigningIdentity;
use contextmesh::delta::{DeltaError, RecipientState, compute_delta, delta_over};
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
async fn chain_store(depth: usize) -> (Store, SigningIdentity, ContextId, Vec<EventId>) {
    let db = path("ob04-chain");
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

/// Builds genesis + one text-bearing `agent.request` child + one
/// `context.critical` event (the OB-03 critical store shape).
async fn critical_store() -> (Store, SigningIdentity, ContextId, EventId, EventId, EventId) {
    let db = path("ob04-critical");
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

/// Closes the full ancestry of the chain tip.
async fn close_all(
    store: &Store,
    ctx: ContextId,
    ids: &[EventId],
) -> contextmesh::closure::ClosedSelection {
    let tip = ids[ids.len() - 1];
    close_selection(store, ctx, &[tip], &[tip], &critical_policy(), &LIMITS)
        .await
        .unwrap()
}

/// A deterministic distinct `EventId` for a seed (canonical order == seed order).
fn distinct_id(seed: u32) -> EventId {
    let mut bytes = [0u8; 32];
    bytes[0] = (seed & 0xff) as u8;
    bytes[1] = ((seed >> 8) & 0xff) as u8;
    bytes[2] = ((seed >> 16) & 0xff) as u8;
    bytes[3] = ((seed >> 24) & 0xff) as u8;
    EventId::from_bytes(bytes)
}

#[tokio::test]
async fn cold_start_recipient_produces_full_selection() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let closed = close_all(&store, ctx, &ids).await;
    let recipient = RecipientState::cold_start(ctx);
    let delta = compute_delta(&store, &closed, &recipient).await.unwrap();
    assert!(delta.is_cold_start());
    assert_eq!(delta.recipient_head(), None);
    assert!(delta.recipient_closure().is_empty());
    assert!(delta.selected_known().is_empty());
    let mut expected = ids.clone();
    expected.sort();
    assert_eq!(delta.events(), expected);
    assert_eq!(delta.total_bytes(), closed.total_bytes());
    assert_eq!(delta.references(), closed.references());
}

#[tokio::test]
async fn delta_matches_selected_minus_recipient_closure() {
    let (store, _author, ctx, ids) = chain_store(3).await; // [genesis, c1, c2, c3]
    let closed = close_all(&store, ctx, &ids).await;
    let recipient = RecipientState::at_head(&store, ctx, ids[1], &LIMITS)
        .await
        .unwrap();
    let mut expected_closure = vec![ids[0], ids[1]];
    expected_closure.sort();
    assert_eq!(recipient.head(), Some(ids[1]));
    assert_eq!(recipient.closure(), expected_closure.as_slice());

    let delta = compute_delta(&store, &closed, &recipient).await.unwrap();
    assert_eq!(delta.recipient_head(), Some(ids[1]));
    assert_eq!(delta.recipient_closure(), expected_closure.as_slice());
    let mut expected_delta = vec![ids[2], ids[3]];
    expected_delta.sort();
    assert_eq!(delta.events(), expected_delta);
    assert_eq!(delta.selected_known(), expected_closure.as_slice());
}

#[tokio::test]
async fn recipient_head_at_tip_produces_empty_delta() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let closed = close_all(&store, ctx, &ids).await;
    let recipient = RecipientState::at_head(&store, ctx, ids[3], &LIMITS)
        .await
        .unwrap();
    let delta = compute_delta(&store, &closed, &recipient).await.unwrap();
    assert!(delta.references().is_empty());
    assert_eq!(delta.total_bytes(), 0);
    let mut expected = ids.clone();
    expected.sort();
    assert_eq!(delta.selected_known(), expected.as_slice());
}

#[tokio::test]
async fn unknown_recipient_head_fails_closed() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let closed = close_all(&store, ctx, &ids).await;
    // A stated head that was never admitted into the DAG.
    let ghost = distinct_id(99);
    let recipient = RecipientState::from_closure(ctx, Some(ghost), vec![ghost], 0).unwrap();
    assert_eq!(
        compute_delta(&store, &closed, &recipient)
            .await
            .unwrap_err(),
        DeltaError::UnknownRecipientHead { head: ghost }
    );
}

#[tokio::test]
async fn recipient_head_wrong_context_fails_closed() {
    let (store, author, ctx, ids) = chain_store(3).await;
    let other_ctx = context(9);
    let other_genesis = author
        .create_event(other_ctx, vec![], "context.genesis", json!({}))
        .unwrap();
    provision(&store, &other_genesis, vec![author.author()]).await;
    store
        .admit(&other_genesis, RefMutation::None)
        .await
        .unwrap();

    let closed = close_all(&store, ctx, &ids).await;
    // The record claims `ctx`, but its head is an event from another context.
    let head = other_genesis.event_id();
    let recipient = RecipientState::from_closure(ctx, Some(head), vec![head], 0).unwrap();
    assert_eq!(
        compute_delta(&store, &closed, &recipient)
            .await
            .unwrap_err(),
        DeltaError::WrongContext { event: head }
    );
}

#[tokio::test]
async fn context_mismatch_fails_closed() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let closed = close_all(&store, ctx, &ids).await;
    let recipient = RecipientState::cold_start(context(9));
    assert_eq!(
        compute_delta(&store, &closed, &recipient)
            .await
            .unwrap_err(),
        DeltaError::ContextMismatch
    );
}

#[tokio::test]
async fn delta_is_deterministic_across_runs() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let closed = close_all(&store, ctx, &ids).await;
    let recipient = RecipientState::at_head(&store, ctx, ids[1], &LIMITS)
        .await
        .unwrap();
    let first = compute_delta(&store, &closed, &recipient).await.unwrap();
    let second = compute_delta(&store, &closed, &recipient).await.unwrap();
    assert_eq!(first.to_wire().unwrap(), second.to_wire().unwrap());
    assert_eq!(first.references(), second.references());
    assert_eq!(first.selected_known(), second.selected_known());
}

#[tokio::test]
async fn delta_is_provable_from_store() {
    let (store, _author, ctx, ids) = chain_store(4).await;
    let closed = close_all(&store, ctx, &ids).await;
    let recipient = RecipientState::at_head(&store, ctx, ids[2], &LIMITS)
        .await
        .unwrap();
    let delta = compute_delta(&store, &closed, &recipient).await.unwrap();

    // Independent re-derivation from the store reproduces the same closure,
    // and the recorded partition is exactly selected minus that closure.
    let rederived = RecipientState::at_head(&store, ctx, ids[2], &LIMITS)
        .await
        .unwrap();
    assert_eq!(delta.recipient_closure(), rederived.closure());
    let selected_events: Vec<EventId> = closed.references().iter().map(|r| r.event()).collect();
    let report = delta_over(&selected_events, rederived.closure());
    assert_eq!(delta.selected_known(), report.known.as_slice());
    assert_eq!(delta.events(), report.delta);

    // Every delta event is absent from the re-derived closure; every known
    // event is present in it.
    for event in delta.events() {
        assert!(rederived.closure().binary_search(&event).is_err());
    }
    for event in report.known {
        assert!(rederived.closure().binary_search(&event).is_ok());
    }
}

#[test]
fn delta_over_pure_partition_is_canonical() {
    let selected = vec![
        distinct_id(3),
        distinct_id(1),
        distinct_id(1),
        distinct_id(2),
    ];
    let closure = vec![distinct_id(1), distinct_id(4), distinct_id(4)];
    let report = delta_over(&selected, &closure);
    let mut expected_delta = vec![distinct_id(2), distinct_id(3)];
    let mut expected_known = vec![distinct_id(1)];
    expected_delta.sort();
    expected_known.sort();
    assert_eq!(report.delta, expected_delta);
    assert_eq!(report.known, expected_known);
}

#[tokio::test]
async fn delta_respects_recipient_closure_bounds() {
    let (store, _author, ctx, ids) = chain_store(4).await;
    let tight_events = ClosureLimits::new(2, 64 * 1024 * 1024).unwrap();
    assert_eq!(
        RecipientState::at_head(&store, ctx, ids[3], &tight_events)
            .await
            .unwrap_err(),
        DeltaError::LimitExceeded
    );
    let tight_bytes = ClosureLimits::new(100_000, 1).unwrap();
    assert_eq!(
        RecipientState::at_head(&store, ctx, ids[3], &tight_bytes)
            .await
            .unwrap_err(),
        DeltaError::LimitExceeded
    );
    // The delta partition is bounded even for a fabricated over-budget state.
    let mut huge = Vec::with_capacity(contextmesh::delta::MAX_DELTA_EVENTS + 1);
    for seed in 0..=contextmesh::delta::MAX_DELTA_EVENTS as u32 {
        huge.push(distinct_id(seed));
    }
    assert_eq!(
        RecipientState::from_closure(ctx, Some(distinct_id(0)), huge, 0).unwrap_err(),
        DeltaError::LimitExceeded
    );
}

#[test]
fn recipient_state_from_closure_validates() {
    // The head must be a member of its own closure.
    assert_eq!(
        RecipientState::from_closure(context(8), Some(distinct_id(5)), vec![], 0).unwrap_err(),
        DeltaError::InvalidState
    );
    // Normalization: scrambled, duplicated closure collapses to canonical order.
    let head = distinct_id(0);
    let state = RecipientState::from_closure(
        context(8),
        Some(head),
        vec![
            distinct_id(3),
            distinct_id(0),
            distinct_id(1),
            distinct_id(1),
            distinct_id(2),
        ],
        123,
    )
    .unwrap();
    assert_eq!(state.context(), context(8));
    assert_eq!(state.head(), Some(head));
    let mut expected = vec![
        distinct_id(0),
        distinct_id(1),
        distinct_id(2),
        distinct_id(3),
    ];
    expected.sort();
    assert_eq!(state.closure(), expected.as_slice());
    assert_eq!(state.total_bytes(), 123);
}

#[tokio::test]
async fn delta_total_bytes_match_references() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let closed = close_all(&store, ctx, &ids).await;
    let recipient = RecipientState::at_head(&store, ctx, ids[1], &LIMITS)
        .await
        .unwrap();
    let delta = compute_delta(&store, &closed, &recipient).await.unwrap();
    let sum: usize = delta.references().iter().map(|r| r.payload_bytes()).sum();
    assert_eq!(delta.total_bytes(), sum);
    assert!(delta.total_bytes() > 0);
    assert_eq!(delta.limits(), LIMITS);
}

#[tokio::test]
async fn composition_selection_closure_delta_receipt() {
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

    // The recipient knows only up to genesis: the delta is the closed set
    // minus genesis, i.e. the child plus the critical event.
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let delta = compute_delta(&store, &closed, &recipient).await.unwrap();
    assert!(!delta.is_cold_start());
    assert_eq!(delta.recipient_head(), Some(genesis));
    let mut expected = vec![child, critical];
    expected.sort();
    assert_eq!(delta.events(), expected);
    assert_eq!(delta.selected_known(), &[genesis]);

    // The delta is what a handoff must deliver: receipt over the delta events
    // verifies against the DAG.
    let mut events = delta.events();
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
    // The verification walk counts the delta events plus the recipient head.
    assert_eq!(report.checked_events, 3);
}
