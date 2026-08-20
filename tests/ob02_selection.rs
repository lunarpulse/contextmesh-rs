//! OB-02 selection-core matrix: golden fixture, budget enforcement, task
//! intake, provenance, determinism, and the I/O edge cases (gate B2).

use contextmesh::compiler::CompiledContext;
use contextmesh::model::{ContextId, EventId};
use contextmesh::receipt::{ReceiptBodyV1, RecipientStateV1, SignedReceiptV1, TaskRecordV1};
use contextmesh::selection::{
    BASELINE_IDENTITY, BASELINE_VERSION, BaselineSelector, NO_MATCH_NOTE, NO_SOURCES_NOTE,
    SelectionBudget, SelectionError, SelectionMarker, SelectionResult, Selector, SourceEvent,
    SourceReference, select_sources,
};
use contextmesh::store::{RefExpectation, RefMutation, Store};
use serde_json::{Value, json};

mod common;
use common::{context, genesis, identity, main_cas, path, provision};

const FIXTURE: &str = "tests/fixtures/ob02-selection-golden.json";
const FIXTURE_TASK: &str = "summarize the request chain";
const FIXTURE_CREATED_AT: &str = "2026-08-17T00:00:00Z";
const FIXTURE_BUDGET: SelectionBudget = SelectionBudget {
    max_selected_events: 3,
    max_exported_bytes: 4096,
};

fn fixture_structured() -> Value {
    json!({"query": {"terms": ["summarize", "request", "chain"]}, "mode": "recall"})
}

fn fixture_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)
}

/// The deterministic golden inputs: author A is identity 7, author B is 9,
/// context byte is 8, and B appends two text-bearing children to A's genesis.
struct Golden {
    store: Store,
    author_a: contextmesh::crypto::SigningIdentity,
    context: ContextId,
    genesis: EventId,
    child1: EventId,
    child2: EventId,
}

async fn golden() -> Golden {
    let db = path("ob02");
    let store = Store::open(&db).await.unwrap();
    let author_a = identity(7);
    let author_b = identity(9);
    let ctx = context(8);
    let genesis_event = author_a
        .create_event(
            ctx,
            vec![],
            "context.genesis",
            json!({"note": "project kickoff atlas request chain"}),
        )
        .unwrap();
    let child1_event = author_b
        .create_event(
            ctx,
            vec![genesis_event.event_id()],
            "agent.request",
            json!({"value": 1, "note": "summarize the request chain from genesis"}),
        )
        .unwrap();
    let child2_event = author_b
        .create_event(
            ctx,
            vec![genesis_event.event_id()],
            "agent.request",
            json!({"value": 2, "note": "summarize the request chain and summarize status"}),
        )
        .unwrap();
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
        context: ctx,
        genesis: genesis_event.event_id(),
        child1: child1_event.event_id(),
        child2: child2_event.event_id(),
    }
}

fn golden_candidates(golden: &Golden) -> Vec<EventId> {
    vec![golden.genesis, golden.child1, golden.child2]
}

async fn select_golden(golden: &Golden) -> SelectionResult {
    select_sources(
        &golden.store,
        FIXTURE_TASK,
        Some(&fixture_structured()),
        &FIXTURE_BUDGET,
        &BaselineSelector::new(),
        &golden_candidates(golden),
    )
    .await
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
    let golden = runtime.block_on(golden());
    let result = runtime.block_on(select_golden(&golden));
    std::fs::write(fixture_path(), result.to_wire().unwrap()).unwrap();
}

#[tokio::test]
async fn golden_fixture_matches_reconstruction() {
    let golden = golden().await;
    let expected = select_golden(&golden).await;
    let wire = std::fs::read(fixture_path()).unwrap();
    assert_eq!(expected.to_wire().unwrap(), wire);
}

#[tokio::test]
async fn golden_fixture_ranks_sources_deterministically() {
    let golden = golden().await;
    let result = select_golden(&golden).await;
    let refs = result.references();
    assert_eq!(refs.len(), 3);
    // Term-frequency scores: child2 (6) > child1 (5) > genesis (2).
    assert_eq!(refs[0].event(), golden.child2);
    assert_eq!(refs[1].event(), golden.child1);
    assert_eq!(refs[2].event(), golden.genesis);
    assert!(result.total_bytes() > 0);
    assert!(result.marker().is_none());
    assert!(result.uncertainty().is_empty());
}

#[tokio::test]
async fn selection_respects_event_count_budget() {
    let golden = golden().await;
    let tight = SelectionBudget {
        max_selected_events: 2,
        max_exported_bytes: 4096,
    };
    let err = select_sources(
        &golden.store,
        FIXTURE_TASK,
        None,
        &tight,
        &BaselineSelector::new(),
        &golden_candidates(&golden),
    )
    .await
    .unwrap_err();
    assert_eq!(err, SelectionError::BudgetExceeded);
}

#[tokio::test]
async fn selection_respects_byte_budget() {
    let golden = golden().await;
    let result = select_golden(&golden).await;
    let total = result.total_bytes();
    let tight = SelectionBudget {
        max_selected_events: 3,
        max_exported_bytes: total - 1,
    };
    let err = select_sources(
        &golden.store,
        FIXTURE_TASK,
        None,
        &tight,
        &BaselineSelector::new(),
        &golden_candidates(&golden),
    )
    .await
    .unwrap_err();
    assert_eq!(err, SelectionError::BudgetExceeded);
}

#[tokio::test]
async fn empty_history_produces_no_sources_marker() {
    let golden = golden().await;
    let result = select_sources(
        &golden.store,
        FIXTURE_TASK,
        None,
        &FIXTURE_BUDGET,
        &BaselineSelector::new(),
        &[],
    )
    .await
    .unwrap();
    assert_eq!(result.marker(), Some(SelectionMarker::NoSources));
    assert!(result.references().is_empty());
    assert_eq!(result.total_bytes(), 0);
    assert_eq!(result.uncertainty(), &[NO_SOURCES_NOTE.to_owned()]);
}

#[tokio::test]
async fn empty_task_fails_closed() {
    let golden = golden().await;
    for empty in ["", "   ", "\n\t"] {
        let err = select_sources(
            &golden.store,
            empty,
            None,
            &FIXTURE_BUDGET,
            &BaselineSelector::new(),
            &golden_candidates(&golden),
        )
        .await
        .unwrap_err();
        assert_eq!(
            err,
            SelectionError::EmptyTask,
            "task {empty:?} must fail closed"
        );
    }
}

#[tokio::test]
async fn no_match_produces_empty_selection_with_uncertainty() {
    let golden = golden().await;
    let result = select_sources(
        &golden.store,
        "quantum teleportation protocol",
        None,
        &FIXTURE_BUDGET,
        &BaselineSelector::new(),
        &golden_candidates(&golden),
    )
    .await
    .unwrap();
    assert_eq!(result.marker(), Some(SelectionMarker::NoMatch));
    assert!(result.references().is_empty());
    assert_eq!(result.total_bytes(), 0);
    assert_eq!(result.uncertainty(), &[NO_MATCH_NOTE.to_owned()]);
}

#[tokio::test]
async fn two_selector_versions_produce_distinct_provenance() {
    let golden = golden().await;
    let v1 = BaselineSelector::new();
    let v2 = BaselineSelector::with_version("0.2.0");
    let r1 = select_sources(
        &golden.store,
        FIXTURE_TASK,
        None,
        &FIXTURE_BUDGET,
        &v1,
        &golden_candidates(&golden),
    )
    .await
    .unwrap();
    let r2 = select_sources(
        &golden.store,
        FIXTURE_TASK,
        None,
        &FIXTURE_BUDGET,
        &v2,
        &golden_candidates(&golden),
    )
    .await
    .unwrap();
    assert_eq!(r1.selector().identity(), BASELINE_IDENTITY);
    assert_eq!(r1.selector().version(), BASELINE_VERSION);
    assert_ne!(r1.selector().version(), r2.selector().version());
    assert_eq!(r1.selector().identity(), r2.selector().identity());
    assert_eq!(r1.selector().config_hash(), r2.selector().config_hash());
    // Same history, same ranking: only the recorded provenance differs.
    assert_eq!(r1.references(), r2.references());
    assert_ne!(r1.to_wire().unwrap(), r2.to_wire().unwrap());
}

#[tokio::test]
async fn structured_and_free_text_tasks_both_produce_selections() {
    let golden = golden().await;
    let selector = BaselineSelector::new();
    let free = select_sources(
        &golden.store,
        FIXTURE_TASK,
        None,
        &FIXTURE_BUDGET,
        &selector,
        &golden_candidates(&golden),
    )
    .await
    .unwrap();
    let structured = select_sources(
        &golden.store,
        FIXTURE_TASK,
        Some(&fixture_structured()),
        &FIXTURE_BUDGET,
        &selector,
        &golden_candidates(&golden),
    )
    .await
    .unwrap();
    assert!(!free.references().is_empty());
    assert_eq!(free.references(), structured.references());
    assert_eq!(free.to_wire().unwrap(), structured.to_wire().unwrap());

    // The receipt task record captures the verbatim, the content hash, and the
    // caller-supplied structured canonical form.
    let task =
        TaskRecordV1::from_verbatim(FIXTURE_TASK.to_owned(), Some(fixture_structured())).unwrap();
    assert_eq!(task.verbatim(), FIXTURE_TASK);
    assert!(task.content_hash().starts_with("blake3_"));
    assert_eq!(task.structured(), Some(&fixture_structured()));
}

#[tokio::test]
async fn selection_is_deterministic_across_runs() {
    let golden = golden().await;
    let selector = BaselineSelector::new();
    let a = select_sources(
        &golden.store,
        FIXTURE_TASK,
        Some(&fixture_structured()),
        &FIXTURE_BUDGET,
        &selector,
        &golden_candidates(&golden),
    )
    .await
    .unwrap();
    let b = select_sources(
        &golden.store,
        FIXTURE_TASK,
        Some(&fixture_structured()),
        &FIXTURE_BUDGET,
        &selector,
        &golden_candidates(&golden),
    )
    .await
    .unwrap();
    assert_eq!(a.to_wire().unwrap(), b.to_wire().unwrap());
}

#[tokio::test]
async fn tie_break_is_canonical_event_order() {
    let db = path("ob02-tie");
    let store = Store::open(&db).await.unwrap();
    let author = identity(11);
    let ctx = context(10);
    let genesis_event = genesis(&author, ctx);
    provision(&store, &genesis_event, vec![author.author()]).await;
    store
        .admit(&genesis_event, RefMutation::None)
        .await
        .unwrap();
    let first = author
        .create_event(
            ctx,
            vec![genesis_event.event_id()],
            "agent.request",
            json!({"note": "alpha beta gamma delta"}),
        )
        .unwrap();
    let second = author
        .create_event(
            ctx,
            vec![genesis_event.event_id()],
            "agent.request",
            json!({"note": "alpha beta gamma epsilon"}),
        )
        .unwrap();
    store
        .admit(
            &first,
            main_cas(ctx, RefExpectation::Absent, first.event_id()),
        )
        .await
        .unwrap();
    store
        .admit(
            &second,
            main_cas(
                ctx,
                RefExpectation::Head(first.event_id()),
                second.event_id(),
            ),
        )
        .await
        .unwrap();
    let budget = SelectionBudget {
        max_selected_events: 2,
        max_exported_bytes: 4096,
    };
    let result = select_sources(
        &store,
        "alpha beta gamma",
        None,
        &budget,
        &BaselineSelector::new(),
        &[first.event_id(), second.event_id()],
    )
    .await
    .unwrap();
    let refs = result.references();
    assert_eq!(refs.len(), 2);
    // Equal term-frequency scores: canonical EventId text order breaks the tie.
    let mut sorted = [first.event_id(), second.event_id()];
    sorted.sort_by_key(ToString::to_string);
    assert_eq!(refs[0].event(), sorted[0]);
    assert_eq!(refs[1].event(), sorted[1]);
}

#[tokio::test]
async fn unverifiable_candidate_fails_closed() {
    let golden = golden().await;
    let foreign = EventId::from_bytes([0xAB; 32]);
    let err = select_sources(
        &golden.store,
        FIXTURE_TASK,
        None,
        &FIXTURE_BUDGET,
        &BaselineSelector::new(),
        &[foreign],
    )
    .await
    .unwrap_err();
    assert_eq!(err, SelectionError::UnverifiableSource);
}

/// A selector that always fails, to exercise the fail-closed contract.
struct FailingSelector;

impl Selector for FailingSelector {
    fn identity(&self) -> &str {
        "failing"
    }

    fn version(&self) -> &str {
        "0.0.0"
    }

    fn config_hash(&self) -> &str {
        "blake3_failing"
    }

    fn select(
        &self,
        _task: &TaskRecordV1,
        _sources: &[SourceEvent],
    ) -> Result<Vec<SourceReference>, SelectionError> {
        Err(SelectionError::SelectorError)
    }
}

#[tokio::test]
async fn selector_error_fails_closed_prior_state_intact() {
    let golden = golden().await;
    let main_before = golden
        .store
        .local_ref(golden.context, &"main".parse().unwrap())
        .await
        .unwrap();
    let err = select_sources(
        &golden.store,
        FIXTURE_TASK,
        None,
        &FIXTURE_BUDGET,
        &FailingSelector,
        &golden_candidates(&golden),
    )
    .await
    .unwrap_err();
    assert_eq!(err, SelectionError::SelectorError);
    let main_after = golden
        .store
        .local_ref(golden.context, &"main".parse().unwrap())
        .await
        .unwrap();
    assert_eq!(main_before, main_after, "prior state must remain intact");
}

#[tokio::test]
async fn selection_composes_with_receipt() {
    let golden = golden().await;
    let result = select_golden(&golden).await;
    let mut events: Vec<EventId> = result.references().iter().map(|r| r.event()).collect();
    events.sort();
    let task =
        TaskRecordV1::from_verbatim(FIXTURE_TASK.to_owned(), Some(fixture_structured())).unwrap();
    let body = ReceiptBodyV1::new(
        golden.context,
        events,
        task,
        RecipientStateV1::new(golden.genesis),
        result.selector().clone(),
        Vec::new(),
        result.uncertainty().to_vec(),
        FIXTURE_CREATED_AT.to_owned(),
        golden.author_a.author(),
    )
    .unwrap();
    let receipt = SignedReceiptV1::issue(&golden.author_a, body).unwrap();
    let report = receipt.verify_against_dag(&golden.store).await.unwrap();
    assert!(report.valid, "findings: {:?}", report.findings);
    // Three selected references plus the recipient head.
    assert_eq!(report.checked_events, 4);
}

#[test]
fn compiled_context_exposes_budget_and_totals() {
    let budget = FIXTURE_BUDGET;
    let compiled = CompiledContext::compile(Vec::new(), &budget).unwrap();
    assert_eq!(compiled.references().len(), 0);
    assert_eq!(compiled.total_bytes(), 0);
    assert_eq!(compiled.budget(), budget);
}
