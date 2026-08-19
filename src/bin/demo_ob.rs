//! The OB-13 Option B end-to-end demo (phase success signal).
//!
//! A recipient with a partial known history and a task receives a handoff
//! that deliberately omits a critical fact, fails the task, challenges the
//! omission, and succeeds only after the repair loop re-includes it. The
//! demo is deterministic on the structural path (fixture seeds, frozen eval
//! manifest, no network, no wall clock) and prints only public identifiers —
//! never key, token, or seed material.

use std::path::{Path, PathBuf};

use contextmesh::closure::ClosureLimits;
use contextmesh::crypto::SigningIdentity;
use contextmesh::delta::RecipientState;
use contextmesh::eval::{
    EVAL_AUTHOR_SEED, EvalManifest, build_case, build_chain, eval_context, simulate,
};
use contextmesh::repair::{RepairBounds, RepairHistory, TaskOutcome, run_repair};
use contextmesh::store::Store;

const LIMITS: ClosureLimits = ClosureLimits {
    max_events: 100_000,
    max_exported_bytes: 64 * 1024 * 1024,
};
const TASK_ID: &str = "probe-security-constraint";
const REPAIR_MAX_ITERATIONS: usize = 4;
const REPAIR_MAX_RE_INCLUDED: usize = 4;
const REPAIR_MAX_BYTES: usize = 64 * 1024 * 1024;

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ob08-eval-manifest.json")
}

fn runtime_store() -> PathBuf {
    if let Some(root) = std::env::var_os("OB13_DEMO_RUNTIME_ROOT") {
        PathBuf::from(root).join("demo.db")
    } else {
        std::env::temp_dir().join(format!("contextmesh-ob13-demo-{}.db", std::process::id()))
    }
}

fn cleanup(store: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut path = store.as_os_str().to_os_string();
        path.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(path));
    }
}

fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build the demo runtime");
    let exit = runtime.block_on(run());
    std::process::exit(if exit { 0 } else { 1 });
}

async fn run() -> bool {
    println!("OB-13 option-b demo");
    println!("task: {TASK_ID}");

    let store_path = runtime_store();
    cleanup(&store_path);
    let store = match Store::open(&store_path).await {
        Ok(store) => store,
        Err(error) => {
            println!("store open failed: {error}");
            return false;
        }
    };
    let author = SigningIdentity::from_fixture_seed([EVAL_AUTHOR_SEED; 32]);
    let manifest = match EvalManifest::load(manifest_path()) {
        Ok(manifest) => manifest,
        Err(error) => {
            println!("manifest load failed: {error}");
            return false;
        }
    };
    let task = match manifest.tasks.iter().find(|task| task.id == TASK_ID) {
        Some(task) => task,
        None => {
            println!("manifest task missing: {TASK_ID}");
            return false;
        }
    };
    let context = eval_context(0);

    println!("stage 1/5: build the dag with a partial-history recipient");
    let chain = match build_chain(&store, &author, context, task).await {
        Ok(chain) => chain,
        Err(error) => {
            println!("chain build failed: {error}");
            return false;
        }
    };
    println!("  genesis: {}", chain.genesis);
    let critical = chain.critical;
    println!("  critical fact: {}", critical);

    println!("stage 2/5: select and hand off with a deliberate omission");
    let withheld = match build_case(&store, context, task, &chain, true).await {
        Ok(case) => case,
        Err(error) => {
            println!("withheld case failed: {error}");
            return false;
        }
    };
    let repaired = match build_case(&store, context, task, &chain, false).await {
        Ok(case) => case,
        Err(error) => {
            println!("repaired case failed: {error}");
            return false;
        }
    };
    let omitted = withheld
        .handoff
        .omissions()
        .iter()
        .find(|omission| omission.event() == critical);
    println!("  handoff events: {}", withheld.handoff.events().len());
    println!(
        "  omission: {} ({})",
        omitted.map(|o| o.event().to_string()).unwrap_or_default(),
        omitted.map(|o| o.reason().to_string()).unwrap_or_default()
    );

    println!("stage 3/5: recipient benchmark with withheld context");
    let recipient = match RecipientState::at_head(&store, context, chain.genesis, &LIMITS).await {
        Ok(state) => state,
        Err(error) => {
            println!("recipient state failed: {error}");
            return false;
        }
    };
    let before = simulate(&withheld.handoff, &[critical]);
    println!("  completed: {}", before.completed);
    if before.completed {
        println!("  unexpected: the task must fail with withheld context");
        return false;
    }

    println!("stage 4/5: recipient challenges the omission; repair re-includes it");
    let history_path = store_path.with_extension("history.jsonl");
    let mut history = match RepairHistory::open(&history_path) {
        Ok(history) => history,
        Err(error) => {
            println!("repair history failed: {error}");
            return false;
        }
    };
    let bounds = match RepairBounds::new(
        REPAIR_MAX_ITERATIONS,
        REPAIR_MAX_RE_INCLUDED,
        REPAIR_MAX_BYTES,
    ) {
        Ok(bounds) => bounds,
        Err(error) => {
            println!("repair bounds failed: {error}");
            return false;
        }
    };
    let critical_for_driver = critical;
    let repaired_closed = repaired.closed;
    let driver = move |current: &contextmesh::handoff::Handoff| {
        use std::future::Ready;
        let current = current.clone();
        let repaired_closed = repaired_closed.clone();
        let outcome: Ready<contextmesh::repair::TaskOutcome> = std::future::ready(
            if current.events().binary_search(&critical_for_driver).is_ok() {
                TaskOutcome::Success
            } else {
                TaskOutcome::NeedsSource {
                    event: critical_for_driver,
                    note: "the recipient needs the withheld critical fact".to_owned(),
                    closed: repaired_closed,
                }
            },
        );
        outcome
    };
    let report = match run_repair(
        &store,
        &withheld.handoff,
        &recipient,
        &bounds,
        driver,
        &mut history,
    )
    .await
    {
        Ok(report) => report,
        Err(error) => {
            println!("repair failed: {error}");
            return false;
        }
    };
    println!("  repair converged: {}", report.converged());
    println!("  repair iterations: {}", report.iterations());
    println!("  re-included: {}", report.re_included().len());

    println!("stage 5/5: recipient benchmark after repair");
    let after = simulate(report.handoff(), &[critical]);
    println!("  completed: {}", after.completed);
    if !after.completed {
        println!("  unexpected: the task must succeed after repair");
        return false;
    }
    if let Ok(wire) = report.handoff().to_wire() {
        println!("  final handoff wire bytes: {}", wire.len());
    }

    cleanup(&store_path);
    let _ = std::fs::remove_file(&history_path);
    println!("phase success signal: PASS");
    true
}
