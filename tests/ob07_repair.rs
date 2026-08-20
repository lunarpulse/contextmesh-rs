//! OB-07 repair matrix: on comprehension or task failure, a bounded repair
//! sequence iteratively re-includes omitted context and re-handoffs, records
//! the attempt history to a distinct JSON-lines file, and always reports
//! convergence or non-convergence — with the original handoff left intact on
//! non-convergence (gate B7).

use contextmesh::closure::{ClosedSelection, ClosureLimits, CriticalPolicy, close_selection};
use contextmesh::crypto::SigningIdentity;
use contextmesh::delta::{RecipientState, compute_delta};
use contextmesh::handoff::{Handoff, HandoffError, OmissionReason};
use contextmesh::model::{ContextId, EventId};
use contextmesh::repair::{
    NonConvergence, RepairBounds, RepairError, RepairHistory, TaskOutcome, TerminalRecord,
    run_repair,
};
use contextmesh::store::{RefExpectation, RefMutation, Store};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

mod common;
use common::{context, identity, main_cas, path, provision};

const LIMITS: ClosureLimits = ClosureLimits {
    max_events: 100_000,
    max_exported_bytes: 64 * 1024 * 1024,
};

static HISTORY_NEXT: AtomicU64 = AtomicU64::new(0);

fn history_path(label: &str) -> PathBuf {
    let serial = HISTORY_NEXT.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "contextmesh-ob07-{label}-{}-{serial}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn critical_policy() -> CriticalPolicy {
    CriticalPolicy::new(vec!["context.critical".to_owned()]).unwrap()
}

/// Builds a deterministic linear chain: genesis, then `depth` single-parent
/// children, each the new main head. Returns (store, author, context, ids).
async fn chain_store(depth: usize) -> (Store, SigningIdentity, ContextId, Vec<EventId>) {
    let db = path("ob07-chain");
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

/// The scripted-challenge driver used by the convergence tests: ask for `c3`
/// until it lands in the offered handoff, then report success. The closed
/// selection that carries `c3` is precomputed, so the driver is synchronous
/// (its future is `Ready`) and stays callable (`FnMut`).
fn ask_c3_driver(
    c3: EventId,
    closed_c3: ClosedSelection,
) -> impl FnMut(&Handoff) -> std::future::Ready<TaskOutcome> {
    move |current: &Handoff| {
        let current = current.clone();
        std::future::ready(if current.events().binary_search(&c3).is_ok() {
            TaskOutcome::Success
        } else {
            TaskOutcome::NeedsSource {
                event: c3,
                note: "c3 is required for the task".to_owned(),
                closed: closed_c3.clone(),
            }
        })
    }
}

#[tokio::test]
async fn repair_converges_within_the_bound_and_records_attempt_history() {
    let (store, _author, ctx, ids) = chain_store(4).await;
    let (genesis, c1, c2, c3, _c4) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    let closed = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff = Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap())
        .unwrap()
        .with_omission(c3, OmissionReason::NotSelected)
        .unwrap();
    let original_wire = handoff.to_wire().unwrap();
    let history = history_path("converge");
    let mut history = RepairHistory::open(&history).unwrap();
    let bounds = RepairBounds::new(4, 4, 64 * 1024 * 1024).unwrap();
    let closed_c3 = closed_for(&store, ctx, &[c1, c2, c3]).await;

    let report = run_repair(
        &store,
        &handoff,
        &recipient,
        &bounds,
        ask_c3_driver(c3, closed_c3),
        &mut history,
    )
    .await
    .unwrap();

    // The sequence converges within the bound with the re-inclusion recorded.
    assert!(report.converged());
    assert_eq!(report.iterations(), 2);
    assert_eq!(report.re_included(), &[c3]);
    assert!(report.non_convergence().is_none());
    let mut expected = vec![c1, c2, c3];
    expected.sort();
    assert_eq!(report.handoff().events(), expected);
    // The convergent handoff is still state-bound (B5 composes into B7).
    report
        .handoff()
        .verify_valid(&store, Some(genesis))
        .await
        .unwrap();
    // The original handoff record is left intact.
    assert_eq!(handoff.to_wire().unwrap(), original_wire);

    // The attempt history records both steps and the convergent terminal.
    let attempts = RepairHistory::read_attempts(history.path()).unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].sequence, 0);
    assert_eq!(attempts[0].iteration, 0);
    assert_eq!(attempts[0].re_included, Some(c3));
    assert_eq!(
        attempts[0].note.as_deref(),
        Some("c3 is required for the task")
    );
    assert_eq!(attempts[0].terminal, None);
    assert_eq!(attempts[0].omissions, vec![c3]);
    let mut expected0 = vec![c1, c2];
    expected0.sort();
    assert_eq!(attempts[0].events, expected0);
    assert_eq!(attempts[1].sequence, 1);
    assert_eq!(attempts[1].iteration, 1);
    assert_eq!(attempts[1].re_included, None);
    assert_eq!(attempts[1].terminal, Some(TerminalRecord::Converged));
    assert!(attempts[1].events.binary_search(&c3).is_ok());
    assert!(attempts[1].omissions.is_empty());
}

#[tokio::test]
async fn repair_converges_immediately_when_the_task_succeeds() {
    let (store, _author, ctx, ids) = chain_store(2).await;
    let (genesis, c1, _c2) = (ids[0], ids[1], ids[2]);
    let closed = closed_for(&store, ctx, &[c1]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff =
        Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap()).unwrap();
    let history = history_path("immediate");
    let mut history = RepairHistory::open(&history).unwrap();
    let bounds = RepairBounds::new(4, 4, 64 * 1024 * 1024).unwrap();

    let report = run_repair(
        &store,
        &handoff,
        &recipient,
        &bounds,
        |_current: &Handoff| async { TaskOutcome::Success },
        &mut history,
    )
    .await
    .unwrap();

    assert!(report.converged());
    assert_eq!(report.iterations(), 1);
    assert!(report.re_included().is_empty());
    assert_eq!(report.handoff().events(), handoff.events());
    let attempts = RepairHistory::read_attempts(history.path()).unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].iteration, 0);
    assert_eq!(attempts[0].re_included, None);
    assert_eq!(attempts[0].terminal, Some(TerminalRecord::Converged));
}

#[tokio::test]
async fn repair_reports_non_convergence_when_the_iteration_budget_is_exhausted() {
    let (store, _author, ctx, ids) = chain_store(4).await;
    let (genesis, c1, c2, c3, _c4) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    let closed = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff = Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap())
        .unwrap()
        .with_omission(c3, OmissionReason::NotSelected)
        .unwrap();
    let original_wire = handoff.to_wire().unwrap();
    let history = history_path("iteration-bound");
    let mut history = RepairHistory::open(&history).unwrap();
    let bounds = RepairBounds::new(1, 4, 64 * 1024 * 1024).unwrap();
    let closed_c3 = closed_for(&store, ctx, &[c1, c2, c3]).await;

    let report = run_repair(
        &store,
        &handoff,
        &recipient,
        &bounds,
        ask_c3_driver(c3, closed_c3),
        &mut history,
    )
    .await
    .unwrap();

    // Non-convergence is reported, and the original handoff is left intact.
    assert!(!report.converged());
    assert_eq!(report.iterations(), 1);
    assert_eq!(report.re_included(), &[c3]);
    assert_eq!(
        report.non_convergence(),
        Some(&NonConvergence::IterationBudgetExceeded { iterations: 1 })
    );
    assert_eq!(report.handoff().to_wire().unwrap(), original_wire);

    // The history records the re-inclusion attempt and the terminal record.
    let attempts = RepairHistory::read_attempts(history.path()).unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].re_included, Some(c3));
    assert_eq!(attempts[0].terminal, None);
    assert_eq!(
        attempts[1].terminal,
        Some(TerminalRecord::NonConverged(
            NonConvergence::IterationBudgetExceeded { iterations: 1 }
        ))
    );
    // The terminal record describes the last offered handoff (with c3).
    assert!(attempts[1].events.binary_search(&c3).is_ok());
}

#[tokio::test]
async fn repair_reports_non_convergence_when_the_driver_reports_failure() {
    let (store, _author, ctx, ids) = chain_store(2).await;
    let (genesis, c1, _c2) = (ids[0], ids[1], ids[2]);
    let closed = closed_for(&store, ctx, &[c1]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff =
        Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap()).unwrap();
    let original_wire = handoff.to_wire().unwrap();
    let history = history_path("driver-failure");
    let mut history = RepairHistory::open(&history).unwrap();
    let bounds = RepairBounds::new(4, 4, 64 * 1024 * 1024).unwrap();

    let report = run_repair(
        &store,
        &handoff,
        &recipient,
        &bounds,
        |_current: &Handoff| async {
            TaskOutcome::Failure {
                note: "no source will help".to_owned(),
            }
        },
        &mut history,
    )
    .await
    .unwrap();

    assert!(!report.converged());
    assert_eq!(report.iterations(), 1);
    assert!(report.re_included().is_empty());
    assert_eq!(
        report.non_convergence(),
        Some(&NonConvergence::OutcomeFailure {
            note: "no source will help".to_owned(),
        })
    );
    assert_eq!(report.handoff().to_wire().unwrap(), original_wire);
    let attempts = RepairHistory::read_attempts(history.path()).unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].note.as_deref(), Some("no source will help"));
    assert_eq!(
        attempts[0].terminal,
        Some(TerminalRecord::NonConverged(
            NonConvergence::OutcomeFailure {
                note: "no source will help".to_owned(),
            }
        ))
    );
}

#[tokio::test]
async fn repair_re_inclusion_budget_is_bounded_and_converges_within_it() {
    let (store, _author, ctx, ids) = chain_store(4).await;
    let (genesis, c1, c2, c3, c4) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    let closed = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff = Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap())
        .unwrap()
        .with_omission(c3, OmissionReason::NotSelected)
        .unwrap()
        .with_omission(c4, OmissionReason::Deliberate)
        .unwrap();
    let original_wire = handoff.to_wire().unwrap();
    let closed_c3 = closed_for(&store, ctx, &[c1, c2, c3]).await;
    let closed_c4 = closed_for(&store, ctx, &[c1, c2, c3, c4]).await;

    // The two-step driver: ask for c3 first, then c4, then report success.
    let two_step = |current: &Handoff| {
        let current = current.clone();
        std::future::ready(
            if current.events().binary_search(&c3).is_ok()
                && current.events().binary_search(&c4).is_ok()
            {
                TaskOutcome::Success
            } else if current.events().binary_search(&c3).is_err() {
                TaskOutcome::NeedsSource {
                    event: c3,
                    note: "need c3".to_owned(),
                    closed: closed_c3.clone(),
                }
            } else {
                TaskOutcome::NeedsSource {
                    event: c4,
                    note: "need c4".to_owned(),
                    closed: closed_c4.clone(),
                }
            },
        )
    };
    let two_step_again = |current: &Handoff| {
        let current = current.clone();
        std::future::ready(
            if current.events().binary_search(&c3).is_ok()
                && current.events().binary_search(&c4).is_ok()
            {
                TaskOutcome::Success
            } else if current.events().binary_search(&c3).is_err() {
                TaskOutcome::NeedsSource {
                    event: c3,
                    note: "need c3".to_owned(),
                    closed: closed_c3.clone(),
                }
            } else {
                TaskOutcome::NeedsSource {
                    event: c4,
                    note: "need c4".to_owned(),
                    closed: closed_c4.clone(),
                }
            },
        )
    };

    // A re-inclusion bound of 1: the second re-inclusion request exceeds it.
    let history = history_path("reinclude-bound");
    let mut history = RepairHistory::open(&history).unwrap();
    let bounds = RepairBounds::new(4, 1, 64 * 1024 * 1024).unwrap();
    let report = run_repair(
        &store,
        &handoff,
        &recipient,
        &bounds,
        two_step,
        &mut history,
    )
    .await
    .unwrap();
    assert!(!report.converged());
    assert_eq!(report.re_included(), &[c3]);
    assert_eq!(
        report.non_convergence(),
        Some(&NonConvergence::ReInclusionBudgetExceeded { re_included: 1 })
    );
    assert_eq!(report.handoff().to_wire().unwrap(), original_wire);
    let attempts = RepairHistory::read_attempts(history.path()).unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(
        attempts[1].terminal,
        Some(TerminalRecord::NonConverged(
            NonConvergence::ReInclusionBudgetExceeded { re_included: 1 }
        ))
    );

    // A re-inclusion bound of 2 admits the same sequence to convergence.
    let history2 = history_path("reinclude-ok");
    let mut history2 = RepairHistory::open(&history2).unwrap();
    let bounds2 = RepairBounds::new(4, 2, 64 * 1024 * 1024).unwrap();
    let report = run_repair(
        &store,
        &handoff,
        &recipient,
        &bounds2,
        two_step_again,
        &mut history2,
    )
    .await
    .unwrap();
    assert!(report.converged());
    assert_eq!(report.re_included(), &[c3, c4]);
    let mut expected = vec![c1, c2, c3, c4];
    expected.sort();
    assert_eq!(report.handoff().events(), expected);
}

#[tokio::test]
async fn repair_byte_budget_is_bounded() {
    let (store, _author, ctx, ids) = chain_store(4).await;
    let (genesis, c1, c2, c3, _c4) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    let closed = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff = Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap())
        .unwrap()
        .with_omission(c3, OmissionReason::NotSelected)
        .unwrap();
    let original_wire = handoff.to_wire().unwrap();
    let initial_bytes = handoff.delta().total_bytes();

    // The follow-up delta that re-includes c3 is strictly larger.
    let closed3 = closed_for(&store, ctx, &[c1, c2, c3]).await;
    let follow_up_bytes = compute_delta(&store, &closed3, &recipient)
        .await
        .unwrap()
        .total_bytes();
    assert!(follow_up_bytes > initial_bytes);
    let closed_c3 = closed_for(&store, ctx, &[c1, c2, c3]).await;

    // A byte bound equal to the initial delta: re-inclusion exceeds it.
    let history = history_path("byte-bound");
    let mut history = RepairHistory::open(&history).unwrap();
    let bounds = RepairBounds::new(4, 4, initial_bytes).unwrap();
    let report = run_repair(
        &store,
        &handoff,
        &recipient,
        &bounds,
        ask_c3_driver(c3, closed_c3.clone()),
        &mut history,
    )
    .await
    .unwrap();
    assert!(!report.converged());
    assert_eq!(
        report.non_convergence(),
        Some(&NonConvergence::ByteBudgetExceeded {
            bytes: follow_up_bytes,
        })
    );
    assert_eq!(report.handoff().to_wire().unwrap(), original_wire);

    // The same sequence converges when the bound admits the follow-up delta.
    let history2 = history_path("byte-ok");
    let mut history2 = RepairHistory::open(&history2).unwrap();
    let bounds2 = RepairBounds::new(4, 4, follow_up_bytes).unwrap();
    let report = run_repair(
        &store,
        &handoff,
        &recipient,
        &bounds2,
        ask_c3_driver(c3, closed_c3),
        &mut history2,
    )
    .await
    .unwrap();
    assert!(report.converged());
    assert_eq!(report.re_included(), &[c3]);
}

#[tokio::test]
async fn repair_fails_closed_for_a_source_that_was_never_omitted_or_never_lands() {
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
    let bounds = RepairBounds::new(4, 4, 64 * 1024 * 1024).unwrap();

    // A driver asking for a source that was never omitted fails closed.
    let history = history_path("never-omitted");
    let mut history = RepairHistory::open(&history).unwrap();
    let closed_c4 = closed_for(&store, ctx, &[c1, c2, c4]).await;
    let driver = |_current: &Handoff| {
        std::future::ready(TaskOutcome::NeedsSource {
            event: c4,
            note: "c4 was never omitted".to_owned(),
            closed: closed_c4.clone(),
        })
    };
    let error = run_repair(&store, &handoff, &recipient, &bounds, driver, &mut history)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        RepairError::Handoff(HandoffError::UnknownOmission { event }) if event == c4
    ));
    assert!(
        RepairHistory::read_attempts(history.path())
            .unwrap()
            .is_empty()
    );

    // A driver whose re-inclusion never lands in the delta fails closed.
    let history2 = history_path("never-lands");
    let mut history2 = RepairHistory::open(&history2).unwrap();
    let closed_c1c2 = closed_for(&store, ctx, &[c1, c2]).await;
    let driver2 = |_current: &Handoff| {
        // The closed selection does not contain c3: the re-inclusion is not
        // real, so the B6 follow-up fails closed.
        std::future::ready(TaskOutcome::NeedsSource {
            event: c3,
            note: "need c3".to_owned(),
            closed: closed_c1c2.clone(),
        })
    };
    let error = run_repair(
        &store,
        &handoff,
        &recipient,
        &bounds,
        driver2,
        &mut history2,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error,
        RepairError::Handoff(HandoffError::InvalidState)
    ));
    assert!(
        RepairHistory::read_attempts(history2.path())
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn repair_history_file_is_independent_of_option_a_db() {
    let db = path("ob07-independence");
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
    let mut head = genesis_event.event_id();
    let mut ids = vec![head];
    for step in 1..=3 {
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
    let (genesis, c1, c2, c3) = (ids[0], ids[1], ids[2], ids[3]);

    let history = history_path("independence");
    let mut history = RepairHistory::open(&history).unwrap();
    let closed = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff = Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap())
        .unwrap()
        .with_omission(c3, OmissionReason::NotSelected)
        .unwrap();
    let bounds = RepairBounds::new(4, 4, 64 * 1024 * 1024).unwrap();
    let closed_c3 = closed_for(&store, ctx, &[c1, c2, c3]).await;
    let report = run_repair(
        &store,
        &handoff,
        &recipient,
        &bounds,
        ask_c3_driver(c3, closed_c3),
        &mut history,
    )
    .await
    .unwrap();
    assert!(report.converged());

    // The history is a distinct JSON-lines file, never the Option A DB.
    assert_ne!(history.path(), &db);
    assert!(!db.ends_with("jsonl"));
    let contents = std::fs::read_to_string(history.path()).unwrap();
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 2);
    for line in &lines {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(value.is_object());
    }
    // The store is untouched and still answers; no sqlite artifacts exist
    // alongside the history file.
    assert!(store.event(genesis).await.unwrap().is_some());
    assert!(store.event(c3).await.unwrap().is_some());
    for suffix in [".db", ".db-wal", ".db-shm"] {
        assert!(!history.path().to_string_lossy().ends_with(suffix));
    }
    // read_attempts round-trips the records from the file alone.
    let attempts = RepairHistory::read_attempts(history.path()).unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[1].terminal, Some(TerminalRecord::Converged));
}

#[tokio::test]
async fn repair_history_is_deterministic_on_the_wire() {
    let run_once = |label: String| async move {
        let (store, _author, ctx, ids) = chain_store(4).await;
        let (genesis, c1, c2, c3, _c4) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
        let closed = closed_for(&store, ctx, &[c1, c2]).await;
        let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
            .await
            .unwrap();
        let handoff =
            Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap())
                .unwrap()
                .with_omission(c3, OmissionReason::NotSelected)
                .unwrap();
        let history = history_path(&label);
        let mut history = RepairHistory::open(&history).unwrap();
        let bounds = RepairBounds::new(4, 4, 64 * 1024 * 1024).unwrap();
        let closed_c3 = closed_for(&store, ctx, &[c1, c2, c3]).await;
        let report = run_repair(
            &store,
            &handoff,
            &recipient,
            &bounds,
            ask_c3_driver(c3, closed_c3),
            &mut history,
        )
        .await
        .unwrap();
        (report, std::fs::read(history.path()).unwrap())
    };
    let (first_report, first_wire) = run_once("determinism-a".to_owned()).await;
    let (second_report, second_wire) = run_once("determinism-b".to_owned()).await;
    assert!(first_report.converged());
    assert!(second_report.converged());
    // Identical inputs produce a byte-identical repair history.
    assert_eq!(first_wire, second_wire);
}

#[tokio::test]
async fn repair_never_negotiates_a_stale_handoff() {
    let (store, _author, ctx, ids) = chain_store(4).await;
    let (genesis, c1, c2, c3, _c4) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    let closed = closed_for(&store, ctx, &[c1, c2]).await;
    let recipient = RecipientState::at_head(&store, ctx, genesis, &LIMITS)
        .await
        .unwrap();
    let handoff = Handoff::from_delta(compute_delta(&store, &closed, &recipient).await.unwrap())
        .unwrap()
        .with_omission(c3, OmissionReason::NotSelected)
        .unwrap();
    let history = history_path("stale");
    let mut history = RepairHistory::open(&history).unwrap();
    let bounds = RepairBounds::new(4, 4, 64 * 1024 * 1024).unwrap();
    let closed_c3 = closed_for(&store, ctx, &[c1, c2, c3]).await;

    // The recipient advances to c1 before the repair runs: the handoff is
    // stale, and a stale handoff is never re-negotiated (B5 composes into
    // B7 through the B6 follow-up's validity gate).
    let advanced = RecipientState::at_head(&store, ctx, c1, &LIMITS)
        .await
        .unwrap();
    let error = run_repair(
        &store,
        &handoff,
        &advanced,
        &bounds,
        ask_c3_driver(c3, closed_c3),
        &mut history,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error,
        RepairError::Handoff(HandoffError::Stale {
            computed: Some(computed),
            current: Some(current),
        }) if computed == genesis && current == c1
    ));
    // The fail-closed negotiation recorded nothing and left the handoff intact.
    assert_eq!(history.attempts(), 0);
    assert!(
        RepairHistory::read_attempts(history.path())
            .unwrap()
            .is_empty()
    );
}
