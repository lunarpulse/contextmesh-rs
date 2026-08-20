//! OB-08 eval matrix: the frozen, offline comprehension and task-performance
//! evaluation suite. Every task carries a known critical-context annotation;
//! the withheld-context case fails and the repaired case passes, proving the
//! selection was load-bearing (gate B8).

use contextmesh::closure::ClosureLimits;
use contextmesh::crypto::SigningIdentity;
use contextmesh::delta::RecipientState;
use contextmesh::eval::{
    CaseExpectation, CaseHandoff, CaseResult, EvalError, EvalManifest, EvalMode, TaskChain,
    build_case, build_chain, eval_context, run_eval_suite, simulate,
};
use contextmesh::handoff::{Handoff, Omission, OmissionReason};
use contextmesh::repair::{RepairBounds, RepairHistory, TaskOutcome, run_repair};
use contextmesh::store::Store;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

mod common;
use common::path;

const LIMITS: ClosureLimits = ClosureLimits {
    max_events: 100_000,
    max_exported_bytes: 64 * 1024 * 1024,
};

static HISTORY_NEXT: AtomicU64 = AtomicU64::new(0);

fn history_path(label: &str) -> PathBuf {
    let serial = HISTORY_NEXT.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "contextmesh-ob08-{label}-{}-{serial}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn manifest_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ob08-eval-manifest.json")
}

fn load_manifest() -> EvalManifest {
    EvalManifest::load(manifest_path()).unwrap()
}

/// The eval suite's deterministic author identity.
fn eval_author() -> SigningIdentity {
    SigningIdentity::from_fixture_seed([contextmesh::eval::EVAL_AUTHOR_SEED; 32])
}

#[tokio::test]
async fn manifest_fixture_is_frozen_and_valid() {
    let manifest = load_manifest();
    assert_eq!(manifest.schema, "ob08-eval-manifest");
    assert_eq!(manifest.version, "1");
    assert_eq!(manifest.tasks.len(), 4);
    assert!(manifest.validate().is_ok());
    // Every task carries a known critical-context annotation: the annotated
    // note is the critical step, and the expected outcomes are the load-
    // bearing pattern (withheld fails, repaired passes).
    let mut ids = Vec::new();
    for task in &manifest.tasks {
        assert_eq!(task.critical_note, task.chain.steps[task.critical_index]);
        assert_eq!(task.expected.withheld, CaseExpectation::Fails);
        assert_eq!(task.expected.repaired, CaseExpectation::Passes);
        ids.push(task.id.clone());
    }
    ids.sort();
    let mut expected_ids = vec![
        "benchmark-escalation-policy".to_owned(),
        "benchmark-retention-rule".to_owned(),
        "probe-approval-requirement".to_owned(),
        "probe-security-constraint".to_owned(),
    ];
    expected_ids.sort();
    assert_eq!(ids, expected_ids);
    // The manifest is a plain JSON file that reads back byte-identically.
    let raw: Value =
        serde_json::from_str(&std::fs::read_to_string(manifest_path()).unwrap()).unwrap();
    assert_eq!(raw["schema"], json!("ob08-eval-manifest"));
}

#[tokio::test]
async fn withheld_context_case_fails_and_repaired_case_passes() {
    // The OB-08 acceptance: the withheld-context case fails and the repaired
    // case passes, proving the selection was load-bearing.
    let db = path("ob08-suite");
    let store = Store::open(&db).await.unwrap();
    let manifest = load_manifest();
    let report = run_eval_suite(&store, &manifest).await.unwrap();

    assert!(report.passed());
    assert_eq!(report.results().len(), 4);
    for result in report.results() {
        assert!(
            result.pass,
            "task {} failed its frozen expectations",
            result.id
        );
        // The withheld case fails: the critical context was load-bearing.
        assert!(!result.withheld.completed);
        assert!(result.withheld.passes(CaseExpectation::Fails));
        // The repaired case passes: the delivered critical context completes
        // the task.
        assert!(result.repaired.completed);
        assert!(result.repaired.passes(CaseExpectation::Passes));
    }
}

#[tokio::test]
async fn challenge_probe_notices_a_withheld_critical_fact() {
    let db = path("ob08-probe");
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
    let case = build_case(&store, context, task, &chain, true)
        .await
        .unwrap();

    // The recipient notices the withheld critical fact because it is an
    // explicit omission — never hidden.
    let result = simulate(&case.handoff, &[chain.critical]);
    assert_eq!(result.noticed, vec![chain.critical]);
    assert!(result.hidden.is_empty());
    assert!(!result.completed);
    assert!(result.passes(CaseExpectation::Fails));
    // The handoff's omission list explicitly names the withheld critical
    // fact with the deliberate reason recorded.
    assert_eq!(
        case.handoff.omissions(),
        &[Omission::new(chain.critical, OmissionReason::Deliberate)]
    );
    // The critical event is not carried by the withheld handoff.
    assert!(
        case.handoff
            .events()
            .binary_search(&chain.critical)
            .is_err()
    );
}

#[tokio::test]
async fn task_benchmark_completes_when_the_critical_fact_is_included() {
    let db = path("ob08-benchmark");
    let store = Store::open(&db).await.unwrap();
    let manifest = load_manifest();
    let author = eval_author();
    let task = manifest
        .tasks
        .iter()
        .find(|task| task.id == "benchmark-retention-rule")
        .unwrap();
    let context = eval_context(3);
    let chain = build_chain(&store, &author, context, task).await.unwrap();
    let case = build_case(&store, context, task, &chain, false)
        .await
        .unwrap();

    // The repaired handoff carries the critical fact and the recipient
    // completes the downstream task.
    let result = simulate(&case.handoff, &[chain.critical]);
    assert!(result.completed);
    assert!(result.noticed.is_empty());
    assert!(result.hidden.is_empty());
    assert!(result.passes(CaseExpectation::Passes));
    assert!(case.handoff.events().binary_search(&chain.critical).is_ok());
    // The repaired handoff verifies against the DAG (B5 still holds).
    case.handoff
        .verify_valid(&store, Some(chain.genesis))
        .await
        .unwrap();
}

#[tokio::test]
async fn eval_suite_is_deterministic_on_the_structural_path() {
    let run_once = || async {
        let db = path("ob08-determinism");
        let store = Store::open(&db).await.unwrap();
        let manifest = load_manifest();
        let report = run_eval_suite(&store, &manifest).await.unwrap();
        report.to_wire().unwrap()
    };
    let first = run_once().await;
    let second = run_once().await;
    assert_eq!(first, second);
    let parsed: Value = serde_json::from_slice(&first).unwrap();
    assert_eq!(parsed["passed"], json!(true));
    assert_eq!(parsed["results"].as_array().map(|a| a.len()), Some(4));
}

#[tokio::test]
async fn eval_signal_drives_repair_convergence() {
    // B7 + B8 compose: the eval's challenge signal (a noticed withheld
    // critical fact) drives the OB-07 repair loop, and the converged handoff
    // passes the eval benchmark — comprehension operationalized as
    // "fails-then-recovers when critical context is withheld".
    let db = path("ob08-repair");
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
    let withheld = build_case(&store, context, task, &chain, true)
        .await
        .unwrap();
    let repaired = build_case(&store, context, task, &chain, false)
        .await
        .unwrap();
    let recipient = RecipientState::at_head(&store, context, chain.genesis, &LIMITS)
        .await
        .unwrap();

    // Withheld: the eval measures the failure and names the missing critical
    // event — the challenge signal.
    let withheld_result = simulate(&withheld.handoff, &[chain.critical]);
    assert_eq!(withheld_result.noticed, vec![chain.critical]);
    assert!(!withheld_result.completed);

    // The repair loop re-includes the challenged source using the eval's
    // repaired closed selection.
    let history = history_path("repair");
    let mut history = RepairHistory::open(&history).unwrap();
    let bounds = RepairBounds::new(4, 4, 64 * 1024 * 1024).unwrap();
    let critical = chain.critical;
    let repaired_closed = repaired.closed;
    let driver = move |current: &Handoff| {
        let current = current.clone();
        let repaired_closed = repaired_closed.clone();
        std::future::ready(if current.events().binary_search(&critical).is_ok() {
            TaskOutcome::Success
        } else {
            TaskOutcome::NeedsSource {
                event: critical,
                note: "the recipient needs the withheld critical fact".to_owned(),
                closed: repaired_closed,
            }
        })
    };
    let report = run_repair(
        &store,
        &withheld.handoff,
        &recipient,
        &bounds,
        driver,
        &mut history,
    )
    .await
    .unwrap();
    assert!(report.converged());

    // The converged handoff passes the eval benchmark.
    let converged = simulate(report.handoff(), &[critical]);
    assert!(converged.completed);
    assert!(converged.noticed.is_empty());
    assert!(converged.hidden.is_empty());
    assert!(converged.passes(CaseExpectation::Passes));
    // The attempt history records the re-inclusion and the convergent record.
    let attempts = RepairHistory::read_attempts(history.path()).unwrap();
    assert_eq!(attempts.len(), 2);
    assert!(attempts[1].events.binary_search(&critical).is_ok());
}

#[tokio::test]
async fn eval_manifest_rejects_malformed_tasks_fail_closed() {
    let raw: Value =
        serde_json::from_str(&std::fs::read_to_string(manifest_path()).unwrap()).unwrap();

    // Wrong schema.
    let mut bad = raw.clone();
    bad["schema"] = json!("not-the-eval-manifest");
    let manifest: EvalManifest = serde_json::from_value(bad).unwrap();
    assert!(matches!(
        manifest.validate(),
        Err(EvalError::InvalidManifest)
    ));

    // Duplicate task ids.
    let mut bad = raw.clone();
    let first_id = bad["tasks"][0]["id"].clone();
    bad["tasks"][1]["id"] = first_id;
    let manifest: EvalManifest = serde_json::from_value(bad).unwrap();
    assert!(matches!(
        manifest.validate(),
        Err(EvalError::InvalidManifest)
    ));

    // Critical index outside the chain.
    let mut bad = raw.clone();
    bad["tasks"][0]["critical_index"] = json!(99);
    let manifest: EvalManifest = serde_json::from_value(bad).unwrap();
    assert!(matches!(
        manifest.validate(),
        Err(EvalError::InvalidManifest)
    ));

    // Critical note inconsistent with the annotated chain step.
    let mut bad = raw.clone();
    bad["tasks"][0]["critical_note"] = json!("a different note");
    let manifest: EvalManifest = serde_json::from_value(bad).unwrap();
    assert!(matches!(
        manifest.validate(),
        Err(EvalError::InvalidManifest)
    ));

    // Empty task set.
    let mut bad = raw.clone();
    bad["tasks"] = json!([]);
    let manifest: EvalManifest = serde_json::from_value(bad).unwrap();
    assert!(matches!(
        manifest.validate(),
        Err(EvalError::InvalidManifest)
    ));
}

#[tokio::test]
async fn challenge_probe_and_benchmark_cover_every_frozen_task() {
    // Every manifest task must exhibit the load-bearing pattern in both
    // sub-modes: the challenge probe notices the withheld fact, and the task
    // benchmark completes when it is repaired.
    let db = path("ob08-cover");
    let store = Store::open(&db).await.unwrap();
    let manifest = load_manifest();
    let author = eval_author();
    for (index, task) in manifest.tasks.iter().enumerate() {
        let context = eval_context(index);
        let chain = build_chain(&store, &author, context, task).await.unwrap();
        let withheld = build_case(&store, context, task, &chain, true)
            .await
            .unwrap();
        let repaired = build_case(&store, context, task, &chain, false)
            .await
            .unwrap();
        let withheld_result = simulate(&withheld.handoff, &[chain.critical]);
        let repaired_result = simulate(&repaired.handoff, &[chain.critical]);
        assert_eq!(
            withheld_result.noticed,
            vec![chain.critical],
            "task {}: the probe must notice the withheld fact",
            task.id
        );
        assert!(withheld_result.hidden.is_empty());
        assert!(!withheld_result.completed);
        assert!(repaired_result.completed);
        assert!(repaired_result.hidden.is_empty());
        // Sub-mode classification matches the manifest.
        match task.mode {
            EvalMode::ChallengeProbe => assert!(task.id.starts_with("probe-")),
            EvalMode::TaskBenchmark => assert!(task.id.starts_with("benchmark-")),
        }
    }
}

/// Builds a fresh store and returns it plus the first manifest task's chain.
async fn fresh_probe_chain() -> (Store, TaskChain, CaseHandoff) {
    let db = path("ob08-assist");
    let store = Store::open(&db).await.unwrap();
    let manifest = load_manifest();
    let author = eval_author();
    let task = manifest.tasks.first().unwrap();
    let context = eval_context(0);
    let chain = build_chain(&store, &author, context, task).await.unwrap();
    let case = build_case(&store, context, task, &chain, true)
        .await
        .unwrap();
    (store, chain, case)
}

#[tokio::test]
async fn case_result_passes_checks_both_directions() {
    // The `passes` verdict is exact: a failing case with no noticed omission
    // is a hidden failure, and a passing case that notices anything fails.
    let (store, chain, case) = fresh_probe_chain().await;
    let critical = chain.critical;
    let result = simulate(&case.handoff, &[critical]);
    assert!(result.passes(CaseExpectation::Fails));
    assert!(!result.passes(CaseExpectation::Passes));

    // A manufactured hidden omission (critical missing, not listed) fails.
    let hidden = CaseResult {
        noticed: vec![],
        hidden: vec![critical],
        completed: false,
    };
    assert!(!hidden.passes(CaseExpectation::Fails));
    assert!(!hidden.passes(CaseExpectation::Passes));

    // A manufactured completed case passes the Passes expectation.
    let completed = CaseResult {
        noticed: vec![],
        hidden: vec![],
        completed: true,
    };
    assert!(completed.passes(CaseExpectation::Passes));
    assert!(!completed.passes(CaseExpectation::Fails));
    // The store is still usable and the handoff still verifies.
    case.handoff
        .verify_valid(&store, Some(chain.genesis))
        .await
        .unwrap();
}
