//! OB-05 validity matrix: a handoff binds a B4 delta to the recipient head it
//! was computed against; it is valid only while the recipient's current
//! stated head is that head in the same DAG. A stale handoff is rejected and
//! re-derived, never applied; unknown recipient state fails closed (gate B5).

use contextmesh::closure::{ClosedSelection, ClosureLimits, CriticalPolicy, close_selection};
use contextmesh::crypto::SigningIdentity;
use contextmesh::delta::{RecipientState, compute_delta};
use contextmesh::handoff::{Handoff, HandoffError};
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
    let db = path("ob05-chain");
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

/// Closes an explicit selected set under the standard critical policy.
async fn closed_for(store: &Store, ctx: ContextId, selected: &[EventId]) -> ClosedSelection {
    close_selection(store, ctx, selected, selected, &critical_policy(), &LIMITS)
        .await
        .unwrap()
}

/// A deterministic EventId that is not a node of any test DAG.
fn ghost_id(seed: u8) -> EventId {
    let mut bytes = [0u8; 32];
    bytes[0] = seed;
    EventId::from_bytes(bytes)
}

#[tokio::test]
async fn handoff_is_valid_against_its_stated_head() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let (genesis, c1, c2, c3) = (ids[0], ids[1], ids[2], ids[3]);
    let closed = closed_for(&store, ctx, &[c1, c2, c3]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff =
        Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap()).unwrap();

    assert_eq!(handoff.context(), ctx);
    assert_eq!(handoff.recipient_head(), Some(genesis));
    assert!(!handoff.is_cold_start());
    let mut expected = vec![c1, c2, c3];
    expected.sort();
    assert_eq!(handoff.events(), expected);

    handoff.verify_valid(&store, Some(genesis)).await.unwrap();
    let verified = handoff.verified_delta(&store, Some(genesis)).await.unwrap();
    assert_eq!(verified.events(), expected);
}

#[tokio::test]
async fn stale_handoff_is_rejected_when_recipient_advances() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let (genesis, c1, ..) = (ids[0], ids[1], ids[2]);
    let closed = closed_for(&store, ctx, &ids[1..]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff =
        Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap()).unwrap();

    // The recipient applied an earlier handoff and advanced its head to c1:
    // the handoff computed against genesis is now stale.
    let error = handoff.verify_valid(&store, Some(c1)).await.unwrap_err();
    assert_eq!(
        error,
        HandoffError::Stale {
            computed: Some(genesis),
            current: Some(c1),
        }
    );
    // A stale handoff is never applied: the delta is not obtainable.
    assert!(matches!(
        handoff.verified_delta(&store, Some(c1)).await,
        Err(HandoffError::Stale { .. })
    ));
}

#[tokio::test]
async fn re_deriving_against_the_new_head_succeeds_and_original_is_intact() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let (genesis, c1, c2, c3) = (ids[0], ids[1], ids[2], ids[3]);
    let closed = closed_for(&store, ctx, &[c1, c2, c3]).await;

    let first = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let original =
        Handoff::from_delta(compute_delta(&store, &closed, &first).await.unwrap()).unwrap();
    let original_wire = original.to_wire().unwrap();

    // The recipient advanced to c1: the original handoff is stale.
    assert!(matches!(
        original.verify_valid(&store, Some(c1)).await,
        Err(HandoffError::Stale { .. })
    ));

    // Re-derive against c1; the re-derived handoff is valid.
    let advanced = RecipientState::at_head(&store, ctx, c1, &LIMITS)
        .await
        .unwrap();
    let rederived =
        Handoff::from_delta(compute_delta(&store, &closed, &advanced).await.unwrap()).unwrap();
    assert_eq!(rederived.recipient_head(), Some(c1));
    let mut expected = vec![c2, c3];
    expected.sort();
    assert_eq!(rederived.events(), expected);
    rederived.verify_valid(&store, Some(c1)).await.unwrap();

    // The original handoff was left intact.
    assert_eq!(original.to_wire().unwrap(), original_wire);
}

#[tokio::test]
async fn cold_start_handoff_is_valid_until_the_recipient_advances() {
    let (store, _author, ctx, ids) = chain_store(2).await;
    let (genesis, c1, c2) = (ids[0], ids[1], ids[2]);
    let closed = closed_for(&store, ctx, &[c1, c2]).await;

    let cold = RecipientState::cold_start(ctx);
    let handoff =
        Handoff::from_delta(compute_delta(&store, &closed, &cold).await.unwrap()).unwrap();
    assert!(handoff.is_cold_start());
    assert_eq!(handoff.recipient_head(), None);
    // Cold-start delta == the full closed selection, ancestors included.
    let mut expected = vec![genesis, c1, c2];
    expected.sort();
    assert_eq!(handoff.events(), expected);

    handoff.verify_valid(&store, None).await.unwrap();

    let error = handoff
        .verify_valid(&store, Some(genesis))
        .await
        .unwrap_err();
    assert_eq!(
        error,
        HandoffError::Stale {
            computed: None,
            current: Some(genesis),
        }
    );
}

#[tokio::test]
async fn unknown_recipient_head_fails_closed() {
    let (store, _author, ctx, ids) = chain_store(2).await;
    let (genesis, c1, c2) = (ids[0], ids[1], ids[2]);
    let closed = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff =
        Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap()).unwrap();

    // The recipient's current stated head is not a node of the DAG.
    let ghost = ghost_id(42);
    let error = handoff.verify_valid(&store, Some(ghost)).await.unwrap_err();
    assert_eq!(error, HandoffError::UnknownRecipientHead { head: ghost });

    // The handoff's embedded head is not a node of this (empty) DAG either:
    // unknown recipient state fails closed and is never assumed.
    let other_db = path("ob05-empty");
    let other_store = Store::open(&other_db).await.unwrap();
    let error = handoff
        .verify_valid(&other_store, Some(genesis))
        .await
        .unwrap_err();
    assert_eq!(error, HandoffError::UnknownRecipientHead { head: genesis });
}

#[tokio::test]
async fn head_from_another_context_fails_closed() {
    let (store, author, ctx, ids) = chain_store(2).await;
    let (genesis, c1, c2) = (ids[0], ids[1], ids[2]);
    let closed = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff =
        Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap()).unwrap();

    // A genesis event in another context, admitted to the same DAG: the
    // current stated head is present but outside the handoff's context.
    let other_ctx = context(9);
    let other_genesis = author
        .create_event(
            other_ctx,
            vec![],
            "context.genesis",
            json!({"note": "other"}),
        )
        .unwrap();
    provision(&store, &other_genesis, vec![author.author()]).await;
    store
        .admit(&other_genesis, RefMutation::None)
        .await
        .unwrap();

    let error = handoff
        .verify_valid(&store, Some(other_genesis.event_id()))
        .await
        .unwrap_err();
    assert_eq!(
        error,
        HandoffError::WrongContext {
            event: other_genesis.event_id(),
        }
    );
}

#[tokio::test]
async fn verification_is_idempotent_while_the_head_is_unchanged() {
    let (store, _author, ctx, ids) = chain_store(2).await;
    let (genesis, c1, c2) = (ids[0], ids[1], ids[2]);
    let closed = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff =
        Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap()).unwrap();

    handoff.verify_valid(&store, Some(genesis)).await.unwrap();
    handoff.verify_valid(&store, Some(genesis)).await.unwrap();

    let first = handoff.verify_valid(&store, Some(c1)).await.unwrap_err();
    let second = handoff.verify_valid(&store, Some(c1)).await.unwrap_err();
    assert_eq!(first, second);
    assert_eq!(
        first,
        HandoffError::Stale {
            computed: Some(genesis),
            current: Some(c1),
        }
    );
}

#[tokio::test]
async fn handoff_wire_is_deterministic() {
    let (store, _author, ctx, ids) = chain_store(2).await;
    let (genesis, c1, c2) = (ids[0], ids[1], ids[2]);
    let closed = closed_for(&store, ctx, &[c1, c2]).await;

    let first = Handoff::from_delta(
        compute_delta(
            &store,
            &closed,
            &RecipientState::at_head(&store, ctx, genesis, &LIMITS)
                .await
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    let second = Handoff::from_delta(
        compute_delta(
            &store,
            &closed,
            &RecipientState::at_head(&store, ctx, genesis, &LIMITS)
                .await
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .unwrap();

    let wire1 = first.to_wire().unwrap();
    let wire2 = second.to_wire().unwrap();
    assert_eq!(wire1, wire2);
    let parsed: serde_json::Value = serde_json::from_slice(&wire1).unwrap();
    assert!(parsed.is_object());
}

#[tokio::test]
async fn handoff_composition_selection_closure_delta_receipt() {
    // The OB-03 critical-store shape: genesis + agent.request child +
    // context.critical.
    let db = path("ob05-critical");
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
            json!({"note": TASK}),
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
    let genesis = genesis_event.event_id();
    let candidates = vec![genesis, child.event_id(), critical.event_id()];

    let budget = SelectionBudget {
        max_selected_events: 4,
        max_exported_bytes: 4096,
    };
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
    assert_eq!(selected_ids, vec![child.event_id()]);

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
    assert_eq!(closed.added_critical(), &[critical.event_id()]);

    // The recipient knows only up to genesis: the handoff carries exactly the
    // closed set minus genesis (child plus the critical event).
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff =
        Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap()).unwrap();
    assert_eq!(handoff.recipient_head(), Some(genesis));
    let mut expected = vec![child.event_id(), critical.event_id()];
    expected.sort();
    assert_eq!(handoff.events(), expected);

    handoff.verify_valid(&store, Some(genesis)).await.unwrap();

    // The delivered delta verifies against the DAG as a receipt.
    let mut events = handoff.events();
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

    // Once the recipient advances to the child, the same handoff is stale.
    let error = handoff
        .verify_valid(&store, Some(child.event_id()))
        .await
        .unwrap_err();
    assert_eq!(
        error,
        HandoffError::Stale {
            computed: Some(genesis),
            current: Some(child.event_id()),
        }
    );
}
