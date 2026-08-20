//! Option B comprehension and task-performance evaluation (gate B8).
//!
//! An in-repo, frozen, offline evaluation suite that makes "comprehension"
//! measurable rather than claimed. The suite runs a curated task set from a
//! golden manifest (`tests/fixtures/ob08-eval-manifest.json`) in two
//! sub-modes — challenge probes (does the recipient notice a withheld
//! critical fact?) and task benchmarks (does the recipient complete the
//! downstream task?). Every task carries a known critical-context annotation,
//! so the withheld and repaired cases are deterministic and offline. The
//! evaluation is never human judgment and never self-report: a task succeeds
//! only when the selected context demonstrably carries the critical facts the
//! downstream task needs. External benchmarks are advisory only and never
//! gate acceptance. B7's repair loop consumes this suite's signals for
//! eval-driven convergence.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::closure::{
    ClosedSelection, ClosureError, ClosureLimits, CriticalPolicy, close_selection,
};
use crate::crypto::SigningIdentity;
use crate::delta::{DeltaError, RecipientState, compute_delta};
use crate::error::{ContractError, StoreError};
use crate::handoff::{Handoff, HandoffError, OmissionReason};
use crate::model::{ContextId, EventId};
use crate::selection::{BaselineSelector, SelectionBudget, SelectionError, select_sources};
use crate::store::{ContextProvision, LocalRefName, RefExpectation, RefMutation, Store};
use serde_json::json;

/// Fixture author seed used to build every frozen eval task chain
/// (deterministic across runs and machines).
pub const EVAL_AUTHOR_SEED: u8 = 9;

/// Selection budget applied to every eval task (generous, fixed, offline).
const EVAL_BUDGET: SelectionBudget = SelectionBudget {
    max_selected_events: 64,
    max_exported_bytes: 64 * 1024,
};

/// Closure limits applied to every eval task.
const EVAL_LIMITS: ClosureLimits = ClosureLimits {
    max_events: 100_000,
    max_exported_bytes: 64 * 1024 * 1024,
};

/// The evaluation policy kind: a kind that never appears in eval chains, so
/// the closure adds nothing and the eval measures pure selection
/// load-bearingness.
const EVAL_POLICY_KIND: &str = "eval.no-critical-kind";

/// The two evaluation sub-modes (gate B8).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvalMode {
    /// Challenge probe: does the recipient notice a withheld critical fact?
    ChallengeProbe,
    /// Task benchmark: does the recipient complete the downstream task?
    TaskBenchmark,
}

/// The expected outcome of one case (withheld or repaired).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaseExpectation {
    /// The case is expected to fail (critical context was load-bearing).
    Fails,
    /// The case is expected to pass (critical context was delivered).
    Passes,
}

/// The frozen expected outcomes of one eval task.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExpectedOutcome {
    /// Expected outcome of the withheld-context case.
    pub withheld: CaseExpectation,
    /// Expected outcome of the repaired-context case.
    pub repaired: CaseExpectation,
}

/// The deterministic chain shape of one eval task (the critical annotation).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TaskChainSpec {
    /// Kind of every non-critical child event in the chain.
    pub kind: String,
    /// Notes of the chain children, in order; the critical event is one of
    /// these (see `critical_index`).
    pub steps: Vec<String>,
}

/// One frozen eval task: id, sub-mode, task text, chain shape, and the known
/// critical-context annotation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvalTask {
    /// Unique task id (frozen in the manifest).
    pub id: String,
    /// Evaluation sub-mode.
    pub mode: EvalMode,
    /// Downstream task text (drives the OB-02 baseline selection).
    pub task: String,
    /// Deterministic chain shape the task runs over.
    pub chain: TaskChainSpec,
    /// Index into `chain.steps` of the critical event.
    pub critical_index: usize,
    /// Kind of the critical event (its critical-context annotation).
    pub critical_kind: String,
    /// Note of the critical event; must equal `chain.steps[critical_index]`
    /// and share task terms so the baseline selector can pick it.
    pub critical_note: String,
    /// Frozen expected outcomes for the withheld and repaired cases.
    pub expected: ExpectedOutcome,
}

/// The frozen golden eval manifest (task IDs, critical annotations, expected
/// outcome).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvalManifest {
    /// Must be "ob08-eval-manifest".
    pub schema: String,
    /// Frozen manifest version.
    pub version: String,
    /// The curated, frozen task set.
    pub tasks: Vec<EvalTask>,
}

impl EvalManifest {
    /// Loads and validates a frozen manifest from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, EvalError> {
        let contents = std::fs::read_to_string(path)?;
        let manifest: EvalManifest = serde_json::from_str(&contents)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Fails closed unless the manifest is well-formed: schema and version
    /// pinned, tasks non-empty with unique ids, and every task's critical
    /// annotation consistent with its chain.
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.schema != "ob08-eval-manifest" {
            return Err(EvalError::InvalidManifest);
        }
        if self.version != "1" {
            return Err(EvalError::InvalidManifest);
        }
        if self.tasks.is_empty() {
            return Err(EvalError::InvalidManifest);
        }
        let mut seen = std::collections::HashSet::new();
        for task in &self.tasks {
            if task.id.is_empty() || !seen.insert(&task.id) {
                return Err(EvalError::InvalidManifest);
            }
            if task.task.trim().is_empty() {
                return Err(EvalError::InvalidManifest);
            }
            if task.chain.kind.is_empty() || task.chain.steps.is_empty() {
                return Err(EvalError::InvalidManifest);
            }
            if task.critical_index >= task.chain.steps.len() {
                return Err(EvalError::InvalidManifest);
            }
            if task.critical_kind.is_empty() || task.critical_note.is_empty() {
                return Err(EvalError::InvalidManifest);
            }
            if task.critical_note != task.chain.steps[task.critical_index] {
                return Err(EvalError::InvalidManifest);
            }
        }
        Ok(())
    }
}

/// The deterministic DAG a task runs over: genesis, the chain children, and
/// the annotated critical event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskChain {
    /// The context genesis (the recipient's known history head).
    pub genesis: EventId,
    /// The chain children in insertion order.
    pub children: Vec<EventId>,
    /// The annotated critical event (a member of `children`).
    pub critical: EventId,
}

impl TaskChain {
    /// The candidate set that includes the critical event.
    #[must_use]
    pub fn candidates(&self) -> Vec<EventId> {
        self.children.clone()
    }

    /// The candidate set with the critical event excluded.
    #[must_use]
    pub fn candidates_without_critical(&self) -> Vec<EventId> {
        self.children
            .iter()
            .filter(|event| **event != self.critical)
            .copied()
            .collect()
    }
}

/// The outcome of one case as measured on a simulated recipient (gate B8).
///
/// The recipient is deterministic and offline: it needs the task's critical
/// events, checks the handoff's carried events, and treats a missing critical
/// event as noticed only when it is explicitly listed as an omission — a
/// missing fact that is not listed is a hidden omission and fails the case.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CaseResult {
    /// Critical events the recipient noticed missing (explicit omissions).
    pub noticed: Vec<EventId>,
    /// Critical events missing but not listed as omissions (must be empty).
    pub hidden: Vec<EventId>,
    /// Whether the recipient completed the downstream task.
    pub completed: bool,
}

impl CaseResult {
    /// Returns true when this case matches the frozen expectation: a failing
    /// case is not completed, has at least one noticed omission and nothing
    /// hidden; a passing case is completed with nothing noticed or hidden.
    #[must_use]
    pub fn passes(&self, expectation: CaseExpectation) -> bool {
        match expectation {
            CaseExpectation::Fails => {
                !self.completed && !self.noticed.is_empty() && self.hidden.is_empty()
            }
            CaseExpectation::Passes => {
                self.completed && self.noticed.is_empty() && self.hidden.is_empty()
            }
        }
    }
}

/// One built case: the closed selection and the handoff offered to the
/// simulated recipient.
#[derive(Debug)]
pub struct CaseHandoff {
    /// The closed selection the handoff's delta was computed from.
    pub closed: ClosedSelection,
    /// The handoff offered to the recipient.
    pub handoff: Handoff,
}

/// One task's evaluation result.
#[derive(Clone, Debug, Serialize)]
pub struct EvalResult {
    /// Task id (from the frozen manifest).
    pub id: String,
    /// Evaluation sub-mode.
    pub mode: EvalMode,
    /// The withheld-context case result.
    pub withheld: CaseResult,
    /// The repaired-context case result.
    pub repaired: CaseResult,
    /// Whether both cases matched the frozen expectations.
    pub pass: bool,
}

/// The complete evaluation suite report.
#[derive(Clone, Debug, Serialize)]
pub struct EvalReport {
    /// Whether every task passed its frozen expectations.
    pub passed: bool,
    /// Per-task results in manifest order.
    pub results: Vec<EvalResult>,
}

impl EvalReport {
    /// Renders the report as canonical RFC 8785/JCS wire bytes.
    pub fn to_wire(&self) -> Result<Vec<u8>, EvalError> {
        crate::model::canonicalize(self).map_err(|_| EvalError::Internal)
    }

    /// Returns true when every task passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.passed
    }

    /// Returns the per-task results.
    #[must_use]
    pub fn results(&self) -> &[EvalResult] {
        &self.results
    }
}

/// Stable typed evaluation failures (gate B8).
#[derive(Debug, Error)]
pub enum EvalError {
    /// The eval manifest file could not be read.
    #[error("eval manifest I/O failed")]
    Io(#[from] std::io::Error),
    /// The eval manifest could not be parsed.
    #[error("eval manifest parsing failed")]
    Serialize(#[from] serde_json::Error),
    /// The eval manifest is malformed or inconsistent.
    #[error("eval manifest is malformed")]
    InvalidManifest,
    /// An eval event could not be constructed.
    #[error("eval event construction failed")]
    Contract(#[from] ContractError),
    /// A read-only store operation failed.
    #[error("eval store operation failed")]
    Store(#[from] StoreError),
    /// The task-conditioned selection failed.
    #[error("eval selection failed")]
    Selection(#[from] SelectionError),
    /// The dependency closure failed.
    #[error("eval closure failed")]
    Closure(#[from] ClosureError),
    /// The recipient-known-history delta failed.
    #[error("eval delta computation failed")]
    Delta(#[from] DeltaError),
    /// The handoff construction failed.
    #[error("eval handoff construction failed")]
    Handoff(#[from] HandoffError),
    /// An internal checked invariant failed.
    #[error("eval internal failure")]
    Internal,
}

/// Builds the deterministic DAG for a task: genesis plus the chain children,
/// where the child at `critical_index` carries the critical annotation.
pub async fn build_chain(
    store: &Store,
    author: &SigningIdentity,
    context: ContextId,
    task: &EvalTask,
) -> Result<TaskChain, EvalError> {
    let genesis_event =
        author.create_event(context, vec![], "context.genesis", json!({"note": "root"}))?;
    provision(store, &genesis_event, author).await?;
    store.admit(&genesis_event, RefMutation::None).await?;

    let mut children = Vec::new();
    let mut critical = None;
    let mut head = genesis_event.event_id();
    for (index, note) in task.chain.steps.iter().enumerate() {
        let is_critical = index == task.critical_index;
        let kind = if is_critical {
            task.critical_kind.as_str()
        } else {
            task.chain.kind.as_str()
        };
        // Every child is a sibling of the genesis (parents = [genesis]), so
        // the critical event is never an ancestor of another child and can
        // be cleanly withheld without leaking into the dependency closure.
        let event = author.create_event(
            context,
            vec![genesis_event.event_id()],
            kind,
            json!({"value": index + 1, "note": note}),
        )?;
        let expected = if index == 0 {
            RefExpectation::Absent
        } else {
            RefExpectation::Head(head)
        };
        store
            .admit(&event, main_cas(context, expected, event.event_id()))
            .await?;
        let id = event.event_id();
        if is_critical {
            critical = Some(id);
        }
        children.push(id);
        head = id;
    }
    Ok(TaskChain {
        genesis: genesis_event.event_id(),
        children,
        critical: critical.ok_or(EvalError::InvalidManifest)?,
    })
}

/// Builds one case: selection → closure → delta → handoff, with the critical
/// event withheld (listed as a deliberate omission) or included, and the
/// selection's uncertainty carried onto the handoff.
pub async fn build_case(
    store: &Store,
    context: ContextId,
    task: &EvalTask,
    chain: &TaskChain,
    withheld: bool,
) -> Result<CaseHandoff, EvalError> {
    let candidates = if withheld {
        chain.candidates_without_critical()
    } else {
        chain.candidates()
    };
    let selection = select_sources(
        store,
        &task.task,
        None,
        &EVAL_BUDGET,
        &BaselineSelector::new(),
        &candidates,
    )
    .await?;
    let selected: Vec<EventId> = selection.references().iter().map(|r| r.event()).collect();
    let closed = close_selection(
        store,
        context,
        &selected,
        &candidates,
        &noop_policy(),
        &EVAL_LIMITS,
    )
    .await?;
    let recipient = RecipientState::at_head(store, context, chain.genesis, &EVAL_LIMITS).await?;
    let mut handoff = Handoff::from_delta(compute_delta(store, &closed, &recipient).await?)?;
    if withheld {
        handoff = handoff.with_omission(chain.critical, OmissionReason::Deliberate)?;
    }
    for note in selection.uncertainty() {
        handoff = handoff.with_uncertainty(note.clone())?;
    }
    Ok(CaseHandoff { closed, handoff })
}

/// Measures one handoff on the simulated recipient against the task's
/// critical events.
#[must_use]
pub fn simulate(handoff: &Handoff, critical: &[EventId]) -> CaseResult {
    let events = handoff.events();
    let mut noticed = Vec::new();
    let mut hidden = Vec::new();
    for event in critical {
        if events.binary_search(event).is_ok() {
            continue;
        }
        if handoff.omissions().iter().any(|o| o.event() == *event) {
            noticed.push(*event);
        } else {
            hidden.push(*event);
        }
    }
    let completed = critical
        .iter()
        .all(|event| events.binary_search(event).is_ok());
    CaseResult {
        noticed,
        hidden,
        completed,
    }
}

/// Runs the full frozen suite over the manifest: every task is built
/// deterministically and measured in both the withheld and repaired cases.
pub async fn run_eval_suite(
    store: &Store,
    manifest: &EvalManifest,
) -> Result<EvalReport, EvalError> {
    manifest.validate()?;
    let author = SigningIdentity::from_fixture_seed([EVAL_AUTHOR_SEED; 32]);
    let mut results = Vec::with_capacity(manifest.tasks.len());
    for (index, task) in manifest.tasks.iter().enumerate() {
        let context = eval_context(index);
        let chain = build_chain(store, &author, context, task).await?;
        let withheld = build_case(store, context, task, &chain, true).await?;
        let repaired = build_case(store, context, task, &chain, false).await?;
        let withheld = simulate(&withheld.handoff, &[chain.critical]);
        let repaired = simulate(&repaired.handoff, &[chain.critical]);
        let pass =
            withheld.passes(task.expected.withheld) && repaired.passes(task.expected.repaired);
        results.push(EvalResult {
            id: task.id.clone(),
            mode: task.mode,
            withheld,
            repaired,
            pass,
        });
    }
    let passed = results.iter().all(|result| result.pass);
    Ok(EvalReport { passed, results })
}

/// Deterministic per-task context (derived from the task index).
///
/// The suite builds every task in its own context; the derivation is public
/// so callers (and OB-10) can address the same DAGs.
#[must_use]
pub fn eval_context(index: usize) -> ContextId {
    ContextId::from_bytes([(index as u8).wrapping_add(1); 32])
}

/// The eval closure policy: a kind that never appears in eval chains, so the
/// closure adds nothing and the selection alone is load-bearing.
fn noop_policy() -> CriticalPolicy {
    CriticalPolicy::new(vec![EVAL_POLICY_KIND.to_owned()]).expect("static eval policy")
}

/// Provisions the context with the author as the sole authorized author.
async fn provision(
    store: &Store,
    genesis: &crate::model::SignedEventV1,
    author: &SigningIdentity,
) -> Result<(), EvalError> {
    store
        .provision_context(ContextProvision {
            context: genesis.body().context(),
            expected_genesis: genesis.event_id(),
            authorized_authors: vec![author.author()],
        })
        .await?;
    Ok(())
}

/// The canonical "main" local-ref compare-and-swap mutation.
fn main_cas(context: ContextId, expected: RefExpectation, head: EventId) -> RefMutation {
    RefMutation::CompareAndSwap {
        context,
        name: "main"
            .parse::<LocalRefName>()
            .expect("static local ref name"),
        expected,
        new_head: head,
    }
}
