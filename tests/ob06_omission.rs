//! OB-06 omission matrix: every handoff carries an explicit omission list and
//! uncertainty markers; a recipient can challenge a listed omission, and the
//! challenged source is re-included in a follow-up handoff with the challenge
//! recorded — no omission is hidden (gate B6).

use contextmesh::closure::{ClosedSelection, ClosureLimits, CriticalPolicy, close_selection};
use contextmesh::crypto::SigningIdentity;
use contextmesh::delta::{RecipientState, compute_delta};
use contextmesh::handoff::{
    Handoff, HandoffError, Omission, OmissionChallenge, OmissionReason, ReIncluded,
};
use contextmesh::model::{ContextId, EventId};
use contextmesh::receipt::{ReceiptBodyV1, RecipientStateV1, SignedReceiptV1, TaskRecordV1};
use contextmesh::selection::{
    BaselineSelector, NO_MATCH_NOTE, SelectionBudget, SelectionMarker, select_sources,
};
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
    let db = path("ob06-chain");
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

#[tokio::test]
async fn every_handoff_carries_an_explicit_omission_list() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let (genesis, c1, c2, _c3) = (ids[0], ids[1], ids[2], ids[3]);
    let closed = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff =
        Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap()).unwrap();

    // No omission is hidden: the omission list, the uncertainty markers, and
    // the re-inclusion list are present on every handoff, even when empty.
    assert!(handoff.omissions().is_empty());
    assert!(handoff.uncertainty().is_empty());
    assert!(handoff.re_included().is_empty());
}

#[tokio::test]
async fn omissions_and_uncertainty_are_recorded_explicitly() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let (genesis, c1, c2, c3) = (ids[0], ids[1], ids[2], ids[3]);
    let closed = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff = Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap())
        .unwrap()
        .with_omission(c3, OmissionReason::NotSelected)
        .unwrap()
        .with_uncertainty(NO_MATCH_NOTE)
        .unwrap();

    assert_eq!(
        handoff.omissions(),
        &[Omission::new(c3, OmissionReason::NotSelected)]
    );
    assert_eq!(handoff.uncertainty(), &[NO_MATCH_NOTE.to_owned()]);
    // An omission never names an event the handoff already carries.
    assert!(handoff.events().binary_search(&c3).is_err());
    // The canonical wire carries the negotiation fields explicitly.
    let parsed: serde_json::Value = serde_json::from_slice(&handoff.to_wire().unwrap()).unwrap();
    assert_eq!(parsed["omissions"][0]["event"], json!(c3.to_string()));
    assert_eq!(parsed["uncertainty"][0], json!(NO_MATCH_NOTE));
}

#[tokio::test]
async fn with_omission_fails_closed_for_an_included_event() {
    let (store, _author, ctx, ids) = chain_store(3).await;
    let (genesis, c1, c2, _c3) = (ids[0], ids[1], ids[2], ids[3]);
    let closed = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    // c1 is carried by the handoff's delta; listing it as an omission would
    // hide a carried source, so the record fails closed as malformed.
    let error = Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap())
        .unwrap()
        .with_omission(c1, OmissionReason::NotSelected)
        .unwrap_err();
    assert_eq!(error, HandoffError::InvalidState);
}

#[tokio::test]
async fn a_listed_omission_can_be_challenged_and_an_unlisted_one_cannot() {
    let (store, _author, ctx, ids) = chain_store(4).await;
    let (genesis, c1, c2, c3, c4) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    let closed = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff = Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap())
        .unwrap()
        .with_omission(c3, OmissionReason::NotSelected)
        .unwrap();

    // The recipient challenges a listed omission: the typed challenge record
    // names the event and carries the recipient's stated reason.
    let note = "the recipient needs c3 to complete the task";
    let challenge = handoff.challenge(c3, note).unwrap();
    assert_eq!(challenge, OmissionChallenge::new(c3, note.to_owned()));
    assert_eq!(challenge.event(), c3);
    assert_eq!(challenge.note(), note);

    // Challenging an unlisted omission fails closed — a recipient cannot
    // invent an omission that was never stated.
    let error = handoff.challenge(c4, "why was this withheld").unwrap_err();
    assert_eq!(error, HandoffError::UnknownOmission { event: c4 });

    // An empty challenge note fails closed.
    let error = handoff.challenge(c3, "").unwrap_err();
    assert_eq!(error, HandoffError::InvalidState);
}

#[tokio::test]
async fn challenged_omission_is_re_included_in_the_follow_up_handoff_with_the_challenge_recorded() {
    let (store, _author, ctx, ids) = chain_store(4).await;
    let (genesis, c1, c2, c3, c4) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    let closed1 = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff = Handoff::from_delta(compute_delta(&store, &closed1, &recipient).await.unwrap())
        .unwrap()
        .with_omission(c3, OmissionReason::NotSelected)
        .unwrap()
        .with_omission(c4, OmissionReason::Deliberate)
        .unwrap();
    let original_wire = handoff.to_wire().unwrap();

    let challenge = handoff
        .challenge(c3, "c3 is required for the task")
        .unwrap();

    // The negotiation re-runs selection/closure with the challenged source
    // included; the follow-up handoff re-includes it with the challenge
    // recorded, and the original handoff is left intact.
    let closed2 = closed_for(&store, ctx, &[c1, c2, c3]).await;
    let follow_up = handoff
        .follow_up(&store, &closed2, &recipient, &challenge)
        .await
        .unwrap();

    // The re-included source lands in the follow-up handoff's delta.
    let mut expected = vec![c1, c2, c3];
    expected.sort();
    assert_eq!(follow_up.events(), expected);
    // The challenge is recorded, tied to the re-included event.
    assert_eq!(
        follow_up.re_included(),
        &[ReIncluded::new(c3, challenge.clone())]
    );
    // No omission is hidden: c3 is no longer listed (it is now carried), and
    // the still-withheld c4 remains explicitly listed.
    assert_eq!(
        follow_up.omissions(),
        &[Omission::new(c4, OmissionReason::Deliberate)]
    );
    // The follow-up handoff is still state-bound (B5): it verifies against
    // the same recipient head.
    follow_up.verify_valid(&store, Some(genesis)).await.unwrap();
    // The original handoff record was left intact.
    assert_eq!(handoff.to_wire().unwrap(), original_wire);
}

#[tokio::test]
async fn follow_up_fails_closed_when_the_re_inclusion_does_not_land_in_the_delta() {
    let (store, _author, ctx, ids) = chain_store(4).await;
    let (genesis, c1, c2, c3, c4) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    let closed1 = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff = Handoff::from_delta(compute_delta(&store, &closed1, &recipient).await.unwrap())
        .unwrap()
        .with_omission(c3, OmissionReason::NotSelected)
        .unwrap();
    let challenge = handoff.challenge(c3, "c3 is required").unwrap();

    // The negotiation supplies a selection that still does not contain c3:
    // the re-inclusion is not real, so the negotiation fails closed.
    let error = handoff
        .follow_up(&store, &closed1, &recipient, &challenge)
        .await
        .unwrap_err();
    assert_eq!(error, HandoffError::InvalidState);

    // A challenge naming an event that is not a listed omission of this
    // handoff also fails closed — a recipient cannot force a re-inclusion of
    // a source that was never stated as omitted.
    let foreign = OmissionChallenge::new(c4, "not from this handoff".to_owned());
    let error = handoff
        .follow_up(&store, &closed1, &recipient, &foreign)
        .await
        .unwrap_err();
    assert_eq!(error, HandoffError::UnknownOmission { event: c4 });
}

#[tokio::test]
async fn follow_up_fails_closed_when_the_original_handoff_is_stale() {
    let (store, _author, ctx, ids) = chain_store(4).await;
    let (genesis, c1, c2, c3, _c4) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    let closed1 = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff = Handoff::from_delta(compute_delta(&store, &closed1, &recipient).await.unwrap())
        .unwrap()
        .with_omission(c3, OmissionReason::NotSelected)
        .unwrap();
    let challenge = handoff.challenge(c3, "c3 is required").unwrap();

    // The recipient advanced to c1: the original handoff is stale, and a
    // stale handoff is never negotiated (B5 composes into B6).
    let advanced = RecipientState::at_head(&store, ctx, c1, &LIMITS)
        .await
        .unwrap();
    let error = handoff
        .follow_up(&store, &closed1, &advanced, &challenge)
        .await
        .unwrap_err();
    assert_eq!(
        error,
        HandoffError::Stale {
            computed: Some(genesis),
            current: Some(c1),
        }
    );
}

#[tokio::test]
async fn uncertainty_markers_flow_from_a_no_match_selection() {
    let (store, _author, ctx, ids) = chain_store(2).await;
    let budget = SelectionBudget {
        max_selected_events: 4,
        max_exported_bytes: 4096,
    };
    // A task that matches no source yields an empty selection plus an
    // explicit uncertainty marker, never a hallucinated mapping.
    let selection = select_sources(
        &store,
        "zzzz qqqq no such source",
        None,
        &budget,
        &BaselineSelector::new(),
        &ids,
    )
    .await
    .unwrap();
    assert_eq!(selection.marker(), Some(SelectionMarker::NoMatch));
    assert_eq!(selection.uncertainty(), &[NO_MATCH_NOTE.to_owned()]);

    let closed = closed_for(&store, ctx, &[]).await;
    let cold = RecipientState::cold_start(ctx);
    let handoff = Handoff::from_delta(compute_delta(&store, &closed, &cold).await.unwrap())
        .unwrap()
        .with_uncertainty(selection.uncertainty()[0].clone())
        .unwrap();

    // The handoff carries the selection's uncertainty marker and no sources.
    assert_eq!(handoff.uncertainty(), &[NO_MATCH_NOTE.to_owned()]);
    assert!(handoff.events().is_empty());
    handoff.verify_valid(&store, None).await.unwrap();
}

#[tokio::test]
async fn omission_notes_and_uncertainty_feed_the_receipt() {
    // The OB-03 critical-store shape plus a deliberately omitted candidate:
    // genesis + agent.request child (matching the task) + context.critical +
    // an unrelated candidate the handoff omits.
    let db = path("ob06-critical");
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
    // Deliberately unrelated: the kind and note contain no task term, so the
    // baseline selector never matches it and it stays a candidate omission.
    let extra = author
        .create_event(
            ctx,
            vec![genesis_event.event_id()],
            "agent.observation",
            json!({"note": "unrelated"}),
        )
        .unwrap();
    store
        .admit(
            &extra,
            main_cas(
                ctx,
                RefExpectation::Head(critical.event_id()),
                extra.event_id(),
            ),
        )
        .await
        .unwrap();
    let genesis = genesis_event.event_id();
    let candidates = vec![
        genesis,
        child.event_id(),
        critical.event_id(),
        extra.event_id(),
    ];

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

    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff = Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap())
        .unwrap()
        .with_omission(extra.event_id(), OmissionReason::NotSelected)
        .unwrap()
        .with_uncertainty("selector capped the candidate set".to_owned())
        .unwrap();
    assert_eq!(
        handoff.omissions(),
        &[Omission::new(extra.event_id(), OmissionReason::NotSelected)]
    );

    // The handoff's explicit omission list and uncertainty markers populate
    // the receipt's omission/uncertainty notes (OB-01 receipts carry them
    // from B6 onward), and the delivered delta still verifies against the DAG.
    let mut events = handoff.events();
    events.sort();
    let mut omission_notes: Vec<String> = handoff
        .omissions()
        .iter()
        .map(|omission| format!("omitted {}: {}", omission.event(), omission.reason()))
        .collect();
    omission_notes.sort();
    let task = TaskRecordV1::from_verbatim(TASK.to_owned(), None).unwrap();
    let body = ReceiptBodyV1::new(
        ctx,
        events,
        task,
        RecipientStateV1::new(genesis),
        selection.selector().clone(),
        omission_notes,
        handoff.uncertainty().to_vec(),
        CREATED_AT.to_owned(),
        author.author(),
    )
    .unwrap();
    assert_eq!(body.omissions().len(), 1);
    let receipt = SignedReceiptV1::issue(&author, body).unwrap();
    let report = receipt.verify_against_dag(&store).await.unwrap();
    assert!(report.valid, "findings: {:?}", report.findings);
    // The verification walk counts the delta events plus the recipient head.
    assert_eq!(report.checked_events, 3);
}

#[tokio::test]
async fn negotiation_fields_are_deterministic_on_the_wire() {
    let build = async || {
        let (store, _author, ctx, ids) = chain_store(4).await;
        let (genesis, c1, c2, c3, c4) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
        let closed1 = closed_for(&store, ctx, &[c1, c2]).await;
        let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
            .await
            .unwrap();
        let handoff =
            Handoff::from_delta(compute_delta(&store, &closed1, &recipient).await.unwrap())
                .unwrap()
                .with_omission(c3, OmissionReason::NotSelected)
                .unwrap()
                .with_omission(c4, OmissionReason::CapabilityMismatch)
                .unwrap()
                .with_uncertainty(NO_MATCH_NOTE)
                .unwrap();
        let challenge = handoff.challenge(c3, "need c3").unwrap();
        let closed2 = closed_for(&store, ctx, &[c1, c2, c3]).await;
        handoff
            .follow_up(&store, &closed2, &recipient, &challenge)
            .await
            .unwrap()
    };
    let first = build().await;
    let second = build().await;
    assert_eq!(first.to_wire().unwrap(), second.to_wire().unwrap());
    assert_eq!(first.events(), second.events());
    assert_eq!(first.re_included(), second.re_included());
    assert_eq!(first.omissions(), second.omissions());
}
