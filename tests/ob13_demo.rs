//! OB-13 demo matrix: the end-to-end Option B phase success signal — a
//! recipient with partial known history fails its task when a critical fact
//! is withheld, challenges the omission, and succeeds only after the repair
//! loop re-includes it (gate B13).

use contextmesh::closure::ClosureLimits;
use contextmesh::crypto::SigningIdentity;
use contextmesh::delta::RecipientState;
use contextmesh::eval::{
    EVAL_AUTHOR_SEED, EvalManifest, build_case, build_chain, eval_context, simulate,
};
use contextmesh::handoff::{Handoff, Omission, OmissionReason};
use contextmesh::repair::{RepairBounds, RepairHistory, TaskOutcome, run_repair};
use contextmesh::store::Store;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

mod common;
use common::path;

const LIMITS: ClosureLimits = ClosureLimits {
    max_events: 100_000,
    max_exported_bytes: 64 * 1024 * 1024,
};
const TASK_ID: &str = "probe-security-constraint";

static RUNTIME_NEXT: AtomicU64 = AtomicU64::new(0);

fn runtime_root(label: &str) -> PathBuf {
    let serial = RUNTIME_NEXT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "contextmesh-ob13-{label}-{}-{serial}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ob08-eval-manifest.json")
}

fn load_manifest() -> EvalManifest {
    EvalManifest::load(manifest_path()).unwrap()
}

async fn run_scenario(
    store: &Store,
    label: &str,
) -> (
    contextmesh::eval::CaseHandoff,
    contextmesh::repair::RepairReport,
    PathBuf,
) {
    let manifest = load_manifest();
    let author = SigningIdentity::from_fixture_seed([EVAL_AUTHOR_SEED; 32]);
    let task = manifest
        .tasks
        .iter()
        .find(|task| task.id == TASK_ID)
        .unwrap();
    let context = eval_context(0);
    let chain = build_chain(store, &author, context, task).await.unwrap();
    let withheld = build_case(store, context, task, &chain, true)
        .await
        .unwrap();
    let repaired = build_case(store, context, task, &chain, false)
        .await
        .unwrap();
    let recipient = RecipientState::at_head(store, context, chain.genesis, &LIMITS)
        .await
        .unwrap();
    let critical = chain.critical;
    let serial = RUNTIME_NEXT.fetch_add(1, Ordering::Relaxed);
    let history_path = std::env::temp_dir().join(format!(
        "contextmesh-ob13-history-{label}-{}-{serial}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&history_path);
    let mut history = RepairHistory::open(&history_path).unwrap();
    let bounds = RepairBounds::new(4, 4, 64 * 1024 * 1024).unwrap();
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
        store,
        &withheld.handoff,
        &recipient,
        &bounds,
        driver,
        &mut history,
    )
    .await
    .unwrap();
    (withheld, report, history_path)
}

#[tokio::test]
async fn phase_success_signal_fails_with_withheld_context_and_succeeds_after_repair() {
    let db = path("ob13-signal");
    let store = Store::open(&db).await.unwrap();
    let (withheld, report, history_path) = run_scenario(&store, "signal").await;
    let _ = std::fs::remove_file(&history_path);

    // Before repair: the withheld handoff's benchmark does not complete.
    let critical = report.re_included()[0];
    let before = simulate(&withheld.handoff, &[critical]);
    assert!(!before.completed);
    assert_eq!(before.noticed, vec![critical]);
    // After repair: the converged handoff's benchmark completes.
    assert!(report.converged());
    assert_eq!(report.iterations(), 2);
    assert_eq!(report.re_included(), &[critical]);
    let after = simulate(report.handoff(), &[critical]);
    assert!(after.completed);
    assert!(after.hidden.is_empty());
    // The converged handoff still verifies (B5 composes into B13).
    report
        .handoff()
        .verify_valid(&store, withheld.handoff.recipient_head())
        .await
        .unwrap();
}

#[tokio::test]
async fn recipient_partial_history_is_respected() {
    let db = path("ob13-partial");
    let store = Store::open(&db).await.unwrap();
    let manifest = load_manifest();
    let author = SigningIdentity::from_fixture_seed([EVAL_AUTHOR_SEED; 32]);
    let task = manifest
        .tasks
        .iter()
        .find(|task| task.id == TASK_ID)
        .unwrap();
    let context = eval_context(0);
    let chain = build_chain(&store, &author, context, task).await.unwrap();
    // The recipient's known history is partial: exactly the genesis.
    let recipient = RecipientState::at_head(&store, context, chain.genesis, &LIMITS)
        .await
        .unwrap();
    assert_eq!(recipient.head(), Some(chain.genesis));
    assert_eq!(recipient.closure(), &[chain.genesis]);
    // The withheld handoff's delta does not repeat the known history.
    let withheld = build_case(&store, context, task, &chain, true)
        .await
        .unwrap();
    assert!(
        withheld
            .handoff
            .events()
            .binary_search(&chain.genesis)
            .is_err()
    );
}

#[tokio::test]
async fn withheld_critical_fact_is_a_deliberate_explicit_omission_and_is_challengeable() {
    let db = path("ob13-omission");
    let store = Store::open(&db).await.unwrap();
    let (withheld, report, history_path) = run_scenario(&store, "omission").await;
    let _ = std::fs::remove_file(&history_path);
    let critical = report.re_included()[0];
    // The withheld fact is an explicit deliberate omission, never hidden.
    assert_eq!(
        withheld.handoff.omissions(),
        &[Omission::new(critical, OmissionReason::Deliberate)]
    );
    // The omission is a first-class B6 entry point: it can be challenged.
    let challenge = withheld
        .handoff
        .challenge(critical, "the recipient needs this critical fact")
        .unwrap();
    assert_eq!(challenge.event(), critical);
    // The re-included fact is recorded on the converged handoff.
    assert!(report.handoff().events().binary_search(&critical).is_ok());
}

#[tokio::test]
async fn repair_loop_is_bounded_and_records_attempt_history() {
    let db = path("ob13-bounded");
    let store = Store::open(&db).await.unwrap();
    let (_, report, history_path) = run_scenario(&store, "bounded").await;
    assert!(report.converged());
    assert!(report.iterations() <= 4);
    assert!(report.re_included().len() <= 4);
    // The repair history records the re-inclusion and the convergent record.
    let attempts = RepairHistory::read_attempts(&history_path).unwrap();
    assert_eq!(attempts.len(), 2);
    assert!(
        attempts[1]
            .events
            .binary_search(&report.re_included()[0])
            .is_ok()
    );
    let _ = std::fs::remove_file(&history_path);
}

#[test]
fn demo_binary_prints_the_phase_success_signal() {
    let root = runtime_root("bin");
    let output = Command::new(env!("CARGO_BIN_EXE_demo_ob"))
        .env("OB13_DEMO_RUNTIME_ROOT", &root)
        .output()
        .unwrap();
    let transcript = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(transcript.contains("phase success signal: PASS"));
    assert!(transcript.contains("completed: false"));
    assert!(transcript.contains("completed: true"));
    assert!(transcript.contains("repair converged: true"));
}

#[test]
fn demo_transcript_is_deterministic() {
    let run = || {
        let root = runtime_root("determinism");
        let output = Command::new(env!("CARGO_BIN_EXE_demo_ob"))
            .env("OB13_DEMO_RUNTIME_ROOT", &root)
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    };
    let first = run();
    let second = run();
    assert_eq!(first, second);
}

#[test]
fn demo_transcript_leaks_no_secrets() {
    let root = runtime_root("secrets");
    let output = Command::new(env!("CARGO_BIN_EXE_demo_ob"))
        .env("OB13_DEMO_RUNTIME_ROOT", &root)
        .output()
        .unwrap();
    let transcript = String::from_utf8(output.stdout).unwrap();
    for forbidden in [
        "token1_",
        "secret",
        "seed",
        "private key",
        "ed25519_",
        "ctx1_",
    ] {
        assert!(
            !transcript.contains(forbidden),
            "transcript must not contain {forbidden:?}"
        );
    }
    // Only public evt1_ identifiers and stage markers appear.
    assert!(transcript.contains("critical fact: evt1_"));
    assert!(!transcript.contains("critical fact: evt1_ed25519_"));
}

#[test]
fn completion_evidence_matrix_covers_every_b_gate() {
    let evidence = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("_bmad-output/verification-artifacts/ob-completion-evidence.md"),
    )
    .unwrap();
    for gate in [
        "| B1", "| B2", "| B3", "| B4", "| B5", "| B6", "| B7", "| B8", "| B9", "| B10", "| B11",
    ] {
        assert!(evidence.contains(gate), "matrix must cover {gate}");
    }
    for verifier in [
        "verify-ob01.sh",
        "verify-ob02.sh",
        "verify-ob03.sh",
        "verify-ob04.sh",
        "verify-ob05.sh",
        "verify-ob06.sh",
        "verify-ob07.sh",
        "verify-ob08.sh",
        "verify-ob09.sh",
        "verify-ob10.sh",
        "verify-ob11.sh",
        "verify-ob12.sh",
        "verify-ob13.sh",
    ] {
        assert!(evidence.contains(verifier), "matrix must name {verifier}");
    }
}
