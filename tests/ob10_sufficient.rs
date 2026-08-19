//! OB-10 sufficiency/minimality matrix: a selection is claimed sufficient
//! only when the frozen B8 evaluation backs it, minimal only when the
//! recorded metric (selected count/bytes against budget) backs it, and any
//! claim beyond the metric is refused (gate B10).

use contextmesh::closure::{ClosureLimits, CriticalPolicy, close_selection};
use contextmesh::crypto::SigningIdentity;
use contextmesh::eval::{EvalManifest, TaskChain, build_case, build_chain, eval_context};
use contextmesh::model::{ContextId, EventId};
use contextmesh::selection::{
    ClaimBasis, ClaimRefusal, ClaimRequest, SelectionBudget, SelectionMetric, check_minimality,
    check_sufficiency,
};
use contextmesh::store::Store;
use std::path::PathBuf;

mod common;
use common::path;

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
    CriticalPolicy::new(vec!["ob10-test.no-critical".to_owned()]).unwrap()
}

/// A deterministic eval task fixture: store, context, chain, and both the
/// withheld and repaired cases.
struct TaskFixture {
    store: Store,
    context: ContextId,
    chain: TaskChain,
    withheld: contextmesh::eval::CaseHandoff,
    repaired: contextmesh::eval::CaseHandoff,
}

async fn fixture(task_id: &str) -> TaskFixture {
    let db = path("ob10-fixture");
    let store = Store::open(&db).await.unwrap();
    let manifest = load_manifest();
    let author = eval_author();
    let (index, task) = manifest
        .tasks
        .iter()
        .enumerate()
        .find(|(_, task)| task.id == task_id)
        .unwrap();
    let context = eval_context(index);
    let chain = build_chain(&store, &author, context, task).await.unwrap();
    let withheld = build_case(&store, context, task, &chain, true)
        .await
        .unwrap();
    let repaired = build_case(&store, context, task, &chain, false)
        .await
        .unwrap();
    TaskFixture {
        store,
        context,
        chain,
        withheld,
        repaired,
    }
}

#[tokio::test]
async fn sufficiency_claim_is_backed_by_the_b8_evaluation() {
    let fx = fixture("probe-security-constraint").await;
    // The repaired selection delivers the critical fact: the B8 evaluation
    // completes the task, and the claim carries the B8 basis.
    let claim = check_sufficiency(
        &fx.store,
        fx.context,
        fx.chain.genesis,
        &[fx.chain.critical],
        &fx.repaired.closed,
        &LIMITS,
    )
    .await
    .unwrap();
    assert!(claim.sufficient);
    assert_eq!(claim.basis, ClaimBasis::B8Evaluation);
    // The withheld selection hides the critical fact: not sufficient.
    let claim = check_sufficiency(
        &fx.store,
        fx.context,
        fx.chain.genesis,
        &[fx.chain.critical],
        &fx.withheld.closed,
        &LIMITS,
    )
    .await
    .unwrap();
    assert!(!claim.sufficient);
    assert_eq!(claim.basis, ClaimBasis::B8Evaluation);
}

#[tokio::test]
async fn sufficiency_check_works_on_arbitrary_closed_selections() {
    let fx = fixture("probe-security-constraint").await;
    // A hand-built selection of only the non-critical children is not
    // sufficient: the critical fact is missing.
    let non_critical: Vec<EventId> = fx
        .chain
        .children
        .iter()
        .filter(|event| **event != fx.chain.critical)
        .copied()
        .collect();
    assert_eq!(non_critical.len(), 2);
    let closed = close_selection(
        &fx.store,
        fx.context,
        &non_critical,
        &non_critical,
        &no_add_policy(),
        &LIMITS,
    )
    .await
    .unwrap();
    let claim = check_sufficiency(
        &fx.store,
        fx.context,
        fx.chain.genesis,
        &[fx.chain.critical],
        &closed,
        &LIMITS,
    )
    .await
    .unwrap();
    assert!(!claim.sufficient);
    assert_eq!(claim.basis, ClaimBasis::B8Evaluation);
}

#[tokio::test]
async fn minimality_claim_is_backed_by_the_recorded_metric() {
    let fx = fixture("probe-security-constraint").await;
    // The repaired selection is sufficient but NOT removal-minimal: it also
    // carries the non-critical request-chain children, and removing either of
    // them still leaves the critical fact delivered.
    let claim = check_minimality(
        &fx.store,
        fx.context,
        fx.chain.genesis,
        &[fx.chain.critical],
        &fx.repaired.closed,
        &BUDGET,
        &LIMITS,
    )
    .await
    .unwrap();
    assert!(!claim.minimal);
    assert_eq!(claim.metric.selected_events, 3);
    assert_eq!(claim.basis, ClaimBasis::Metric);
    assert!(claim.metric.within_budget());

    // The critical-only selection is sufficient and removal-minimal: removing
    // the one load-bearing source breaks sufficiency.
    let critical_only = close_selection(
        &fx.store,
        fx.context,
        &[fx.chain.critical],
        &[fx.chain.critical],
        &no_add_policy(),
        &LIMITS,
    )
    .await
    .unwrap();
    let claim = check_minimality(
        &fx.store,
        fx.context,
        fx.chain.genesis,
        &[fx.chain.critical],
        &critical_only,
        &BUDGET,
        &LIMITS,
    )
    .await
    .unwrap();
    assert!(claim.minimal);
    assert_eq!(claim.metric.selected_events, 1);
    assert!(claim.metric.within_budget());
    assert_eq!(claim.basis, ClaimBasis::Metric);
}

#[tokio::test]
async fn selection_metric_records_count_and_bytes_against_budget() {
    let fx = fixture("probe-security-constraint").await;
    let metric = SelectionMetric::record(&fx.repaired.closed, &BUDGET);
    assert_eq!(metric.selected_events, 3);
    assert!(metric.exported_bytes > 0);
    assert_eq!(metric.max_selected_events, BUDGET.max_selected_events);
    assert_eq!(metric.max_exported_bytes, BUDGET.max_exported_bytes);
    assert!(metric.within_budget());

    let metric = SelectionMetric::record(&fx.withheld.closed, &BUDGET);
    assert_eq!(metric.selected_events, 2);
    assert!(metric.within_budget());

    // A budget smaller than the selection fails the within-budget check.
    let tiny = SelectionBudget {
        max_selected_events: 1,
        max_exported_bytes: 1,
    };
    let metric = SelectionMetric::record(&fx.repaired.closed, &tiny);
    assert!(!metric.within_budget());
}

#[tokio::test]
async fn claims_beyond_the_metric_are_refused() {
    let fx = fixture("probe-security-constraint").await;
    let metric = SelectionMetric::record(&fx.repaired.closed, &BUDGET);

    // Sufficiency without the B8 evaluation is refused.
    let refusal = ClaimRefusal::refuse(ClaimRequest::Sufficiency, None);
    assert_eq!(refusal.requested, "sufficiency");
    assert!(refusal.reason.contains("B8 evaluation"));
    assert_eq!(refusal.metric, None);

    // Minimality without the recorded metric is refused.
    let refusal = ClaimRefusal::refuse(ClaimRequest::RemovalMinimality, None);
    assert_eq!(refusal.requested, "removal-minimality");
    assert!(refusal.reason.contains("recorded metric"));

    // Global minimality is refused even with a metric: the metric proves
    // removal-minimality only, never global minimality.
    let refusal = ClaimRefusal::refuse(ClaimRequest::GlobalMinimality, Some(metric));
    assert_eq!(refusal.requested, "global-minimality");
    assert!(refusal.reason.contains("removal-minimality only"));
    assert_eq!(refusal.metric, Some(metric));
}

#[tokio::test]
async fn sufficiency_check_is_deterministic_on_the_structural_path() {
    let run_once = || async {
        let fx = fixture("probe-security-constraint").await;
        let claim = check_sufficiency(
            &fx.store,
            fx.context,
            fx.chain.genesis,
            &[fx.chain.critical],
            &fx.repaired.closed,
            &LIMITS,
        )
        .await
        .unwrap();
        let minimal = check_minimality(
            &fx.store,
            fx.context,
            fx.chain.genesis,
            &[fx.chain.critical],
            &fx.repaired.closed,
            &BUDGET,
            &LIMITS,
        )
        .await
        .unwrap();
        (claim, minimal)
    };
    let (first_sufficient, first_minimal) = run_once().await;
    let (second_sufficient, second_minimal) = run_once().await;
    assert_eq!(first_sufficient, second_sufficient);
    assert_eq!(first_minimal, second_minimal);
    assert!(first_sufficient.sufficient);
    assert!(!first_minimal.minimal);
    // The claim types carry their basis on the wire (auditable from the
    // claim alone).
    let wire = serde_json::to_value(first_minimal).unwrap();
    assert_eq!(wire["basis"], serde_json::json!("metric"));
    assert_eq!(wire["metric"]["selected_events"], serde_json::json!(3));
}

#[tokio::test]
async fn minimality_holds_across_every_frozen_task() {
    // Every frozen eval task's critical-only selection is removal-minimal,
    // and the full repaired selection is at least sufficient.
    let db = path("ob10-all");
    let store = Store::open(&db).await.unwrap();
    let manifest = load_manifest();
    let author = eval_author();
    for (index, task) in manifest.tasks.iter().enumerate() {
        let context = eval_context(index);
        let chain = build_chain(&store, &author, context, task).await.unwrap();
        let repaired = build_case(&store, context, task, &chain, false)
            .await
            .unwrap();
        let claim = check_sufficiency(
            &store,
            context,
            chain.genesis,
            &[chain.critical],
            &repaired.closed,
            &LIMITS,
        )
        .await
        .unwrap();
        assert!(claim.sufficient, "task {} must be sufficient", task.id);
        let critical_only = close_selection(
            &store,
            context,
            &[chain.critical],
            &[chain.critical],
            &no_add_policy(),
            &LIMITS,
        )
        .await
        .unwrap();
        let claim = check_minimality(
            &store,
            context,
            chain.genesis,
            &[chain.critical],
            &critical_only,
            &BUDGET,
            &LIMITS,
        )
        .await
        .unwrap();
        assert!(
            claim.minimal,
            "task {} critical-only selection must be minimal",
            task.id
        );
    }
}
