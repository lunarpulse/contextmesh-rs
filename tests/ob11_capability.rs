//! OB-11 capability matrix: a recorded, versioned capability model per
//! recipient shapes the handoff so an event the recipient cannot act on is
//! flagged in the omission/uncertainty list, never silently handed off or
//! dropped (gate B11).

use contextmesh::capability::{
    Capability, CapabilityError, RecipientCapabilities, shape_handoff, verify_handoff,
};
use contextmesh::closure::{ClosureLimits, CriticalPolicy, close_selection};
use contextmesh::crypto::SigningIdentity;
use contextmesh::delta::{RecipientState, compute_delta};
use contextmesh::eval::{EvalManifest, TaskChain, build_case, build_chain, eval_context};
use contextmesh::handoff::{Handoff, Omission, OmissionReason};
use contextmesh::model::{ContextId, EventId};
use contextmesh::selection::{BaselineSelector, SelectionBudget, select_sources};
use contextmesh::store::Store;
use std::path::PathBuf;

mod common;
use common::{identity, path};

const LIMITS: ClosureLimits = ClosureLimits {
    max_events: 100_000,
    max_exported_bytes: 64 * 1024 * 1024,
};
const BUDGET: SelectionBudget = SelectionBudget {
    max_selected_events: 64,
    max_exported_bytes: 64 * 1024,
};

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ob08-eval-manifest.json")
}

fn load_manifest() -> EvalManifest {
    EvalManifest::load(manifest_path()).unwrap()
}

fn eval_author() -> SigningIdentity {
    SigningIdentity::from_fixture_seed([contextmesh::eval::EVAL_AUTHOR_SEED; 32])
}

fn no_add_policy() -> CriticalPolicy {
    CriticalPolicy::new(vec!["ob11-test.no-critical".to_owned()]).unwrap()
}

/// A capability model covering only request handling.
fn request_only_model() -> RecipientCapabilities {
    let recipient = identity(7).author();
    RecipientCapabilities::new(
        recipient,
        1,
        vec![Capability::new("handle.requests", vec!["agent.request".to_owned()]).unwrap()],
    )
    .unwrap()
}

/// A capability model covering request handling and critical facts.
fn full_model() -> RecipientCapabilities {
    let recipient = identity(7).author();
    RecipientCapabilities::new(
        recipient,
        2,
        vec![
            Capability::new("handle.requests", vec!["agent.request".to_owned()]).unwrap(),
            Capability::new("handle.critical", vec!["context.critical".to_owned()]).unwrap(),
        ],
    )
    .unwrap()
}

/// A deterministic eval task fixture: store, context, chain, and the repaired
/// (full) handoff that carries both agent.request and context.critical kinds.
struct EvalFixture {
    store: Store,
    context: ContextId,
    chain: TaskChain,
    task_text: String,
    repaired: contextmesh::eval::CaseHandoff,
}

async fn eval_fixture() -> EvalFixture {
    let db = path("ob11-fixture");
    let store = Store::open(&db).await.unwrap();
    let manifest = load_manifest();
    let author = eval_author();
    let task = manifest
        .tasks
        .iter()
        .find(|task| task.id == "probe-security-constraint")
        .unwrap();
    let context = eval_context(0);
    let chain = build_chain(&store, &author, context, task).await.unwrap();
    let repaired = build_case(&store, context, task, &chain, false)
        .await
        .unwrap();
    EvalFixture {
        store,
        context,
        chain,
        task_text: task.task.clone(),
        repaired,
    }
}

#[tokio::test]
async fn capability_model_is_recorded_and_versioned() {
    let model = request_only_model();
    assert_eq!(model.recipient(), identity(7).author());
    assert_eq!(model.version(), 1);
    assert_eq!(model.capabilities().len(), 1);
    assert_eq!(model.capabilities()[0].name(), "handle.requests");
    assert_eq!(
        model.capabilities()[0].kinds(),
        &["agent.request".to_owned()]
    );
    // The model is recorded: canonical wire that round-trips.
    let wire = model.to_wire().unwrap();
    let parsed: RecipientCapabilities = serde_json::from_slice(&wire).unwrap();
    assert_eq!(parsed, model);
}

#[tokio::test]
async fn capability_model_covers_declared_kinds_only() {
    let request_only = request_only_model();
    assert!(request_only.covers("agent.request"));
    assert!(!request_only.covers("context.critical"));
    assert!(!request_only.covers("anything.else"));
    let full = full_model();
    assert!(full.covers("agent.request"));
    assert!(full.covers("context.critical"));
    // A model with no declared capabilities covers nothing.
    let empty = RecipientCapabilities::new(identity(7).author(), 0, vec![]).unwrap();
    assert!(!empty.covers("agent.request"));
}

#[tokio::test]
async fn shape_handoff_flags_uncovered_carried_events_in_the_uncertainty_list() {
    let fx = eval_fixture().await;
    let model = request_only_model();
    let shaped = shape_handoff(&fx.repaired.handoff, &model).unwrap();

    // The critical event (context.critical) is carried but the recipient
    // cannot act on it: the mismatch is flagged, not assumed.
    assert!(shaped.report.flagged);
    assert_eq!(shaped.report.mismatches.len(), 1);
    assert_eq!(shaped.report.mismatches[0].event, fx.chain.critical);
    assert_eq!(shaped.report.mismatches[0].kind, "context.critical");
    // The flag is an explicit uncertainty marker on the shaped handoff.
    let marker = format!(
        "capability mismatch: event {} kind context.critical",
        fx.chain.critical
    );
    assert!(shaped.handoff.uncertainty().contains(&marker));
    // Shaping is additive to knowledge: the critical event stays carried.
    assert!(
        shaped
            .handoff
            .events()
            .binary_search(&fx.chain.critical)
            .is_ok()
    );
    // The discipline verifies on the shaped handoff.
    let verification = verify_handoff(&fx.store, &shaped.handoff, &model)
        .await
        .unwrap();
    assert!(verification.valid);
    assert_eq!(verification.mismatches, shaped.report.mismatches);
}

#[tokio::test]
async fn shape_handoff_respects_stated_capabilities() {
    let fx = eval_fixture().await;
    let full = full_model();
    let shaped = shape_handoff(&fx.repaired.handoff, &full).unwrap();
    assert!(!shaped.report.flagged);
    assert!(shaped.report.mismatches.is_empty());
    // The shaped handoff is byte-identical: no flags were added.
    assert_eq!(
        shaped.handoff.to_wire().unwrap(),
        fx.repaired.handoff.to_wire().unwrap()
    );
}

#[tokio::test]
async fn capability_mismatch_omissions_are_consistent_and_challengeable() {
    // Build a handoff that deliberately withholds the critical event because
    // the recipient cannot act on it: recorded as a B6 CapabilityMismatch
    // omission.
    let fx = eval_fixture().await;
    let candidates_without: Vec<EventId> = fx
        .chain
        .children
        .iter()
        .filter(|event| **event != fx.chain.critical)
        .copied()
        .collect();
    let selection = select_sources(
        &fx.store,
        &fx.task_text,
        None,
        &BUDGET,
        &BaselineSelector::new(),
        &candidates_without,
    )
    .await
    .unwrap();
    let selected: Vec<EventId> = selection.references().iter().map(|r| r.event()).collect();
    let closed = close_selection(
        &fx.store,
        fx.context,
        &selected,
        &candidates_without,
        &no_add_policy(),
        &LIMITS,
    )
    .await
    .unwrap();
    let recipient = RecipientState::at_head(&fx.store, fx.context, fx.chain.genesis, &LIMITS)
        .await
        .unwrap();
    let handoff = Handoff::from_delta(compute_delta(&fx.store, &closed, &recipient).await.unwrap())
        .unwrap()
        .with_omission(fx.chain.critical, OmissionReason::CapabilityMismatch)
        .unwrap();
    assert_eq!(
        handoff.omissions(),
        &[Omission::new(
            fx.chain.critical,
            OmissionReason::CapabilityMismatch
        )]
    );

    // The omission is consistent: the withheld event is genuinely uncovered.
    let model = request_only_model();
    let verification = verify_handoff(&fx.store, &handoff, &model).await.unwrap();
    assert!(verification.valid);
    assert!(verification.mismatches.is_empty());
    assert_eq!(verification.checked, 3);

    // The omission is a first-class B6 omission: it can be challenged.
    let challenge = handoff
        .challenge(fx.chain.critical, "the recipient needs this critical fact")
        .unwrap();
    assert_eq!(challenge.event(), fx.chain.critical);
}

#[tokio::test]
async fn verify_handoff_rejects_dishonest_capability_flags() {
    let fx = eval_fixture().await;
    // Withhold the critical event as a capability-mismatch omission, then
    // verify against a model that actually covers critical facts: the flag is
    // dishonest and rejected.
    let candidates_without: Vec<EventId> = fx
        .chain
        .children
        .iter()
        .filter(|event| **event != fx.chain.critical)
        .copied()
        .collect();
    let selection = select_sources(
        &fx.store,
        &fx.task_text,
        None,
        &BUDGET,
        &BaselineSelector::new(),
        &candidates_without,
    )
    .await
    .unwrap();
    let selected: Vec<EventId> = selection.references().iter().map(|r| r.event()).collect();
    let closed = close_selection(
        &fx.store,
        fx.context,
        &selected,
        &candidates_without,
        &no_add_policy(),
        &LIMITS,
    )
    .await
    .unwrap();
    let recipient = RecipientState::at_head(&fx.store, fx.context, fx.chain.genesis, &LIMITS)
        .await
        .unwrap();
    let handoff = Handoff::from_delta(compute_delta(&fx.store, &closed, &recipient).await.unwrap())
        .unwrap()
        .with_omission(fx.chain.critical, OmissionReason::CapabilityMismatch)
        .unwrap();

    let error = verify_handoff(&fx.store, &handoff, &full_model())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        CapabilityError::DishonestFlag { event, .. } if event == fx.chain.critical
    ));
}

#[tokio::test]
async fn verify_handoff_rejects_unflagged_mismatches() {
    let fx = eval_fixture().await;
    let model = request_only_model();
    // The UNshaped repaired handoff carries an uncovered critical event with
    // no flag: the silent mismatch is rejected.
    let error = verify_handoff(&fx.store, &fx.repaired.handoff, &model)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        CapabilityError::UnflaggedMismatch { event, kind } if event == fx.chain.critical && kind == "context.critical"
    ));
}

#[tokio::test]
async fn capability_models_and_shaping_are_deterministic() {
    let fx = eval_fixture().await;
    let model_a = request_only_model();
    let model_b = request_only_model();
    assert_eq!(model_a.to_wire().unwrap(), model_b.to_wire().unwrap());

    let shaped_a = shape_handoff(&fx.repaired.handoff, &model_a).unwrap();
    let shaped_b = shape_handoff(&fx.repaired.handoff, &model_b).unwrap();
    assert_eq!(
        shaped_a.handoff.to_wire().unwrap(),
        shaped_b.handoff.to_wire().unwrap()
    );
    assert_eq!(shaped_a.report, shaped_b.report);
}

#[tokio::test]
async fn shaped_handoff_still_verifies_against_the_dag() {
    let fx = eval_fixture().await;
    let model = request_only_model();
    let shaped = shape_handoff(&fx.repaired.handoff, &model).unwrap();
    // B5 composes into B11: the shaped handoff is still state-bound against
    // the recipient head.
    shaped
        .handoff
        .verify_valid(&fx.store, Some(fx.chain.genesis))
        .await
        .unwrap();
    // The shaped handoff still delivers the full context (knowledge additive).
    assert!(
        shaped
            .handoff
            .events()
            .binary_search(&fx.chain.critical)
            .is_ok()
    );
}

#[tokio::test]
async fn invalid_capability_models_fail_closed() {
    // Duplicate capability names are rejected.
    let error = RecipientCapabilities::new(
        identity(7).author(),
        1,
        vec![
            Capability::new("dup", vec!["agent.request".to_owned()]).unwrap(),
            Capability::new("dup", vec!["context.critical".to_owned()]).unwrap(),
        ],
    )
    .unwrap_err();
    assert!(matches!(error, CapabilityError::InvalidState));

    // A capability with no covered kinds is rejected.
    let error = Capability::new("empty", vec![]).unwrap_err();
    assert!(matches!(error, CapabilityError::InvalidState));

    // An empty capability name is rejected.
    let error = Capability::new("", vec!["agent.request".to_owned()]).unwrap_err();
    assert!(matches!(error, CapabilityError::InvalidState));
}
