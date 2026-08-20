//! Option B task-conditioned source selection core (gate B2).
//!
//! This module owns the selection contract: task intake in both accepted
//! forms (free text and a caller-supplied structured query), budget
//! enforcement (maximum selected event count plus maximum exported byte
//! size), the deterministic baseline selector (lexical/term-frequency
//! matching over event payloads with no new dependencies), and the spec's
//! I/O edge-case matrix:
//!
//! - empty history → empty selection with a `NoSources` marker;
//! - empty or absent task → fail closed (`EmptyTask`), no selection produced;
//! - no matching source → empty selection plus an uncertainty marker
//!   (`NoMatch`);
//! - selector error → fail closed (`SelectorError`), prior state intact.
//!
//! Selection is recorded, not re-derived: the receipt — not a model rerun —
//! is the record of a selection, and selector provenance (identity, version,
//! configuration hash) is recorded in every result. Changing the selector
//! version changes the recorded version, never the history. Option A modules
//! are untouched; this module reads events read-only through the store.
//!
//! Gate B10 adds the sufficiency/minimality claim discipline over that
//! selection: sufficiency is claimed only when the frozen B8 evaluation backs
//! it (the task succeeds with the selected context), minimality only when a
//! recorded metric (selected count/bytes against budget) backs it, and any
//! claim beyond the metric is refused.

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::closure::{
    ClosedSelection, ClosureError, ClosureLimits, CriticalPolicy, close_selection,
};
use crate::compiler::{CompiledContext, compile_context};
use crate::delta::{DeltaError, RecipientState, compute_delta};
use crate::eval::simulate;
use crate::handoff::{Handoff, HandoffError};
use crate::model::{AuthorId, ContextId, EventId, SignedEventV1, canonical_payload_bytes};
use crate::receipt::{SelectorRecordV1, TaskRecordV1, task_content_hash};
use crate::store::Store;

/// Hard ceiling on the candidate event set one selection may read.
pub const MAX_SELECTION_CANDIDATES: usize = 65_536;
/// Hard ceiling on selected event references (matches the receipt bound).
pub const MAX_SELECTED_EVENTS: usize = 4096;
/// Hard ceiling on the total exported byte size of a selection.
pub const MAX_SELECTED_BYTES: usize = 2_097_152;
/// Uncertainty note recorded when the context history is empty.
pub const NO_SOURCES_NOTE: &str = "no sources in context history";
/// Uncertainty note recorded when no source matches the task.
pub const NO_MATCH_NOTE: &str = "no source matches the task";
/// Selector identity of the baseline lexical/term-frequency selector.
pub const BASELINE_IDENTITY: &str = "ob-baseline-lexical-tf";
/// Selector version of the baseline lexical/term-frequency selector.
pub const BASELINE_VERSION: &str = "0.1.0";
/// The baseline selector's empty configuration, hashed for provenance.
const BASELINE_CONFIG: &[u8] = b"{}";

/// Stable typed selection failures (gate B2).
///
/// Option A's error module is untouched; this module owns its fail-closed
/// contract. Variants carry no input, payload, or secret material, so
/// displaying an error cannot disclose caller-controlled or secret data.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SelectionError {
    /// The task is empty or absent: fail closed, no selection is produced.
    #[error("task is empty or absent")]
    EmptyTask,
    /// The task could not be recorded (oversized or invalid structured form).
    #[error("task is invalid or exceeds the task bound")]
    InvalidTask,
    /// The selection exceeds the stated budget: refused, never truncated.
    #[error("selection exceeds the stated budget")]
    BudgetExceeded,
    /// A candidate source is missing or unverifiable in the store.
    #[error("candidate source is missing or unverifiable")]
    UnverifiableSource,
    /// The candidate set exceeds the selection bound.
    #[error("candidate set exceeds the selection bound")]
    TooManyCandidates,
    /// The selector failed: fail closed, prior state intact.
    #[error("selector failed")]
    SelectorError,
}

/// The selection budget: maximum selected event count plus maximum exported
/// byte size, enforced at handoff time by the context compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SelectionBudget {
    /// Maximum number of selected event references.
    pub max_selected_events: usize,
    /// Maximum total exported byte size (sum of canonical payload bytes).
    pub max_exported_bytes: usize,
}

impl Default for SelectionBudget {
    fn default() -> Self {
        Self {
            max_selected_events: MAX_SELECTED_EVENTS,
            max_exported_bytes: MAX_SELECTED_BYTES,
        }
    }
}

/// Selection markers for the I/O edge-case matrix.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum SelectionMarker {
    /// Empty history: no sources exist to select.
    NoSources,
    /// The task matched no source: empty selection plus an uncertainty marker.
    NoMatch,
}

/// The read-only view of an Option A event a selector may read.
///
/// Carries the verified immutable content plus the recorded byte size and
/// canonical text of the event's payload — the basis for lexical scoring and
/// for the exported-byte budget.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceEvent {
    event: EventId,
    context: ContextId,
    kind: String,
    author: AuthorId,
    payload: Value,
    payload_text: String,
    payload_bytes: usize,
}

impl SourceEvent {
    /// Builds the selector's read-only view from a verified stored event.
    pub fn from_signed(event: &SignedEventV1) -> Result<Self, SelectionError> {
        let payload_bytes = canonical_payload_bytes(event.body().payload())
            .map_err(|_| SelectionError::UnverifiableSource)?;
        let payload_text = String::from_utf8_lossy(&payload_bytes).into_owned();
        Ok(Self {
            event: event.event_id(),
            context: event.body().context(),
            kind: event.body().kind().to_owned(),
            author: event.body().author(),
            payload: event.body().payload().clone(),
            payload_text,
            payload_bytes: payload_bytes.len(),
        })
    }

    /// Returns the referenced event ID.
    #[must_use]
    pub const fn event(&self) -> EventId {
        self.event
    }

    /// Returns the event's context.
    #[must_use]
    pub const fn context(&self) -> ContextId {
        self.context
    }

    /// Returns the event kind.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Returns the signing author identity.
    #[must_use]
    pub const fn author(&self) -> AuthorId {
        self.author
    }

    /// Returns the immutable JSON payload.
    #[must_use]
    pub fn payload(&self) -> &Value {
        &self.payload
    }

    /// Returns the canonical payload byte size.
    #[must_use]
    pub const fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }

    /// Returns the canonical payload text used for lexical scoring.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.payload_text
    }
}

/// A bounded reference to one selected Option A event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceReference {
    event: EventId,
    context: ContextId,
    kind: String,
    author: AuthorId,
    payload_bytes: usize,
}

impl SourceReference {
    pub(crate) fn from_source(source: &SourceEvent) -> Self {
        Self {
            event: source.event,
            context: source.context,
            kind: source.kind.clone(),
            author: source.author,
            payload_bytes: source.payload_bytes,
        }
    }

    /// Returns the referenced event ID.
    #[must_use]
    pub const fn event(&self) -> EventId {
        self.event
    }

    /// Returns the event's context.
    #[must_use]
    pub const fn context(&self) -> ContextId {
        self.context
    }

    /// Returns the event kind.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Returns the signing author identity.
    #[must_use]
    pub const fn author(&self) -> AuthorId {
        self.author
    }

    /// Returns the canonical payload byte size.
    #[must_use]
    pub const fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }
}

/// Task-conditioned source selection over a context's event history.
///
/// Implementations are deterministic: identical task and source sets always
/// produce the identical ranked selection. The trait is object-safe so
/// different selectors (baseline now, heavier semantic mechanisms later under
/// the Ask-First rule) can be swapped without changing callers.
pub trait Selector {
    /// Returns the selector identity recorded as provenance.
    fn identity(&self) -> &str;
    /// Returns the selector version recorded as provenance.
    fn version(&self) -> &str;
    /// Returns the selector configuration hash recorded as provenance.
    fn config_hash(&self) -> &str;
    /// Ranks the given sources by relevance to the task.
    ///
    /// Returns matching sources ranked best-first; a source with no matching
    /// terms is omitted. Failures fail closed with `SelectionError`.
    fn select(
        &self,
        task: &TaskRecordV1,
        sources: &[SourceEvent],
    ) -> Result<Vec<SourceReference>, SelectionError>;
}

/// Deterministic lexical/term-frequency selector (baseline, gate B2).
///
/// Requires no new dependencies: tokenization is a simple ASCII word split
/// over the canonical JSON text of each event payload plus its kind, and the
/// score is the sum of task-term frequencies in that text. Determinism:
/// identical task and source set always produce the identical ranked
/// selection, with canonical EventId text order breaking ties.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BaselineSelector {
    version: &'static str,
}

impl BaselineSelector {
    /// Constructs the baseline selector at the current default version.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            version: BASELINE_VERSION,
        }
    }

    /// Constructs the baseline selector at an explicit version.
    ///
    /// Changing the version changes the recorded provenance, never history.
    #[must_use]
    pub const fn with_version(version: &'static str) -> Self {
        Self { version }
    }

    /// Returns the selector version.
    #[must_use]
    pub const fn version(&self) -> &'static str {
        self.version
    }
}

impl Default for BaselineSelector {
    fn default() -> Self {
        Self::new()
    }
}

/// BLAKE3 content hash of the baseline's (empty) configuration.
fn baseline_config_hash() -> &'static str {
    static CONFIG_HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CONFIG_HASH.get_or_init(|| task_content_hash(BASELINE_CONFIG))
}

impl Selector for BaselineSelector {
    fn identity(&self) -> &str {
        BASELINE_IDENTITY
    }

    fn version(&self) -> &str {
        self.version
    }

    fn config_hash(&self) -> &str {
        baseline_config_hash()
    }

    fn select(
        &self,
        task: &TaskRecordV1,
        sources: &[SourceEvent],
    ) -> Result<Vec<SourceReference>, SelectionError> {
        let task_terms = tokenize(task.verbatim());
        if task_terms.is_empty() {
            return Err(SelectionError::EmptyTask);
        }
        let mut scored: Vec<(usize, &SourceEvent)> = sources
            .iter()
            .map(|source| (score_source(&task_terms, source), source))
            .collect();
        scored.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.event().to_string().cmp(&right.1.event().to_string()))
        });
        Ok(scored
            .into_iter()
            .filter(|(score, _)| *score > 0)
            .map(|(_, source)| SourceReference::from_source(source))
            .collect())
    }
}

/// Splits ASCII text into lowercase alphanumeric terms.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Scores one source as the sum of task-term frequencies in its text.
fn score_source(task_terms: &[String], source: &SourceEvent) -> usize {
    let tokens = tokenize(&format!("{} {}", source.kind(), source.text()));
    task_terms
        .iter()
        .map(|term| tokens.iter().filter(|token| *token == term).count())
        .sum()
}

/// The complete bounded outcome of a selection over a candidate history.
#[derive(Clone, Debug, Serialize)]
pub struct SelectionResult {
    context: CompiledContext,
    marker: Option<SelectionMarker>,
    uncertainty: Vec<String>,
    selector: SelectorRecordV1,
}

impl SelectionResult {
    /// Returns the compiled, budget-checked source references.
    #[must_use]
    pub fn context(&self) -> &CompiledContext {
        &self.context
    }

    /// Returns the I/O edge-case marker, if any.
    #[must_use]
    pub const fn marker(&self) -> Option<SelectionMarker> {
        self.marker
    }

    /// Returns the explicit uncertainty notes.
    #[must_use]
    pub fn uncertainty(&self) -> &[String] {
        &self.uncertainty
    }

    /// Returns the recorded selector provenance.
    #[must_use]
    pub const fn selector(&self) -> &SelectorRecordV1 {
        &self.selector
    }

    /// Returns the selected source references in ranked order.
    #[must_use]
    pub fn references(&self) -> &[SourceReference] {
        self.context.references()
    }

    /// Returns the total exported byte size of the selection.
    #[must_use]
    pub const fn total_bytes(&self) -> usize {
        self.context.total_bytes()
    }

    /// Renders the result as canonical RFC 8785/JCS wire bytes.
    pub fn to_wire(&self) -> Result<Vec<u8>, SelectionError> {
        crate::model::canonicalize(self).map_err(|_| SelectionError::SelectorError)
    }
}

/// Selects and compiles a bounded source set for a task over a candidate set.
///
/// The candidate set is the caller-selected history to select from (for
/// example the events reachable from a local ref). Every candidate must exist
/// in the store and be verifiable, or selection fails closed
/// (`UnverifiableSource`). The empty-history, empty-task, no-match, and
/// selector-error edge cases follow the spec's I/O matrix.
pub async fn select_sources(
    store: &Store,
    task_text: &str,
    structured: Option<&Value>,
    budget: &SelectionBudget,
    selector: &dyn Selector,
    candidates: &[EventId],
) -> Result<SelectionResult, SelectionError> {
    if task_text.trim().is_empty() {
        return Err(SelectionError::EmptyTask);
    }
    let task = TaskRecordV1::from_verbatim(task_text.to_owned(), structured.cloned())
        .map_err(|_| SelectionError::InvalidTask)?;

    let mut candidates: Vec<EventId> = candidates.to_vec();
    candidates.sort();
    candidates.dedup();
    if candidates.len() > MAX_SELECTION_CANDIDATES {
        return Err(SelectionError::TooManyCandidates);
    }

    let selector_provenance = SelectorRecordV1::new(
        selector.identity().to_owned(),
        selector.version().to_owned(),
        selector.config_hash().to_owned(),
    )
    .map_err(|_| SelectionError::SelectorError)?;

    if candidates.is_empty() {
        let context = compile_context(Vec::new(), budget)?;
        return Ok(SelectionResult {
            context,
            marker: Some(SelectionMarker::NoSources),
            uncertainty: vec![NO_SOURCES_NOTE.to_owned()],
            selector: selector_provenance,
        });
    }

    let mut sources = Vec::with_capacity(candidates.len());
    for id in &candidates {
        match store.event(*id).await {
            Ok(Some(event)) => sources.push(SourceEvent::from_signed(&event)?),
            Ok(None) => return Err(SelectionError::UnverifiableSource),
            Err(_) => return Err(SelectionError::SelectorError),
        }
    }

    let ranked = selector.select(&task, &sources)?;
    let (marker, uncertainty) = if ranked.is_empty() {
        (
            Some(SelectionMarker::NoMatch),
            vec![NO_MATCH_NOTE.to_owned()],
        )
    } else {
        (None, Vec::new())
    };
    let context = compile_context(ranked, budget)?;
    Ok(SelectionResult {
        context,
        marker,
        uncertainty,
        selector: selector_provenance,
    })
}

// ---------------------------------------------------------------------------
// OB-10 — minimal-sufficient-context claim discipline (gate B10).
//
// Sufficiency is claimed only when the frozen B8 evaluation backs it (the
// task succeeds with the selected context); minimality is claimed only when a
// recorded metric (selected count/bytes against budget) backs it; any claim
// beyond the metric is refused.
// ---------------------------------------------------------------------------

/// A recorded selection metric: selected count and exported bytes against
/// the budget the selection was computed under (gate B10).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SelectionMetric {
    /// Number of selected sources.
    pub selected_events: usize,
    /// Total canonical exported bytes of the selection.
    pub exported_bytes: usize,
    /// Budget cap on selected events.
    pub max_selected_events: usize,
    /// Budget cap on exported bytes.
    pub max_exported_bytes: usize,
}

impl SelectionMetric {
    /// Records the metric from a closed selection and its budget.
    #[must_use]
    pub fn record(closed: &ClosedSelection, budget: &SelectionBudget) -> Self {
        Self {
            selected_events: closed.selected().len(),
            exported_bytes: closed.total_bytes(),
            max_selected_events: budget.max_selected_events,
            max_exported_bytes: budget.max_exported_bytes,
        }
    }

    /// Returns true when the recorded selection stays within its budget.
    #[must_use]
    pub fn within_budget(&self) -> bool {
        self.selected_events <= self.max_selected_events
            && self.exported_bytes <= self.max_exported_bytes
    }
}

/// The recorded evidence that backs a sufficiency or minimality claim
/// (gate B10).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClaimBasis {
    /// The frozen B8 evaluation demonstrated task success.
    B8Evaluation,
    /// A recorded selection metric (selected count/bytes against budget).
    Metric,
}

/// A typed sufficiency claim: sufficiency is claimed only when the frozen B8
/// evaluation backs it, and the claim carries its basis so the backing is
/// auditable from the claim alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SufficiencyClaim {
    /// Whether the selection is sufficient for the task.
    pub sufficient: bool,
    /// The recorded evidence backing the claim.
    pub basis: ClaimBasis,
}

/// A typed minimality claim: removal-minimality is claimed only when the
/// recorded metric backs it, and the claim carries both the metric and its
/// basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct MinimalityClaim {
    /// Whether the selection is removal-minimal (every selected source is
    /// load-bearing: removing any one breaks sufficiency).
    pub minimal: bool,
    /// The recorded metric backing the claim.
    pub metric: SelectionMetric,
    /// The recorded evidence backing the claim.
    pub basis: ClaimBasis,
}

/// The claim-scope vocabulary (gate B10).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimRequest {
    /// Sufficiency: the task succeeds with the selected context.
    Sufficiency,
    /// Removal-minimality: every selected source is load-bearing.
    RemovalMinimality,
    /// Global minimality across the candidate set: never backed by the
    /// recorded metric.
    GlobalMinimality,
}

/// A refused claim: beyond what the recorded evidence shows (gate B10).
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ClaimRefusal {
    /// The requested claim that was refused.
    pub requested: String,
    /// Why the recorded evidence cannot back it.
    pub reason: String,
    /// The recorded metric, when one was offered.
    pub metric: Option<SelectionMetric>,
}

impl ClaimRefusal {
    /// Refuses a claim the recorded evidence cannot back. Sufficiency
    /// without the B8 evaluation and minimality beyond the metric are always
    /// refused.
    #[must_use]
    pub fn refuse(request: ClaimRequest, metric: Option<SelectionMetric>) -> Self {
        let (requested, reason) = match request {
            ClaimRequest::Sufficiency => (
                "sufficiency",
                "sufficiency is claimed only when the frozen B8 evaluation backs it",
            ),
            ClaimRequest::RemovalMinimality => (
                "removal-minimality",
                "removal-minimality is claimed only when the recorded metric backs it",
            ),
            ClaimRequest::GlobalMinimality => (
                "global-minimality",
                "the recorded metric proves removal-minimality only, never global minimality",
            ),
        };
        Self {
            requested: requested.to_owned(),
            reason: reason.to_owned(),
            metric,
        }
    }
}

/// Stable typed sufficiency/minimality check failures (gate B10).
#[derive(Debug, Error)]
pub enum ClaimError {
    /// The recipient-known-history delta failed during the check.
    #[error("sufficiency/minimality delta computation failed")]
    Delta(#[from] DeltaError),
    /// The handoff construction failed during the check.
    #[error("sufficiency/minimality handoff construction failed")]
    Handoff(#[from] HandoffError),
    /// The dependency closure failed during the check.
    #[error("sufficiency/minimality closure failed")]
    Closure(#[from] ClosureError),
    /// An internal checked invariant failed.
    #[error("sufficiency/minimality check internal failure")]
    Internal,
}

/// Checks sufficiency of a closed selection against the frozen B8 evaluation:
/// builds the handoff the selection delivers and runs the eval's simulated
/// recipient against the task's critical events. The claim's basis is the B8
/// evaluation; a selection that hides a critical fact is never sufficient.
pub async fn check_sufficiency(
    store: &Store,
    context: ContextId,
    genesis: EventId,
    critical: &[EventId],
    closed: &ClosedSelection,
    limits: &ClosureLimits,
) -> Result<SufficiencyClaim, ClaimError> {
    let recipient = RecipientState::at_head(store, context, genesis, limits).await?;
    let handoff = Handoff::from_delta(compute_delta(store, closed, &recipient).await?)?;
    let result = simulate(&handoff, critical);
    let sufficient = result.completed && result.hidden.is_empty();
    Ok(SufficiencyClaim {
        sufficient,
        basis: ClaimBasis::B8Evaluation,
    })
}

/// Checks removal-minimality of a selection: every selected source must be
/// load-bearing — removing any one of them must make the selection
/// insufficient under the B8 evaluation. The claim's basis is the recorded
/// metric (selected count/bytes against the budget).
pub async fn check_minimality(
    store: &Store,
    context: ContextId,
    genesis: EventId,
    critical: &[EventId],
    closed: &ClosedSelection,
    budget: &SelectionBudget,
    limits: &ClosureLimits,
) -> Result<MinimalityClaim, ClaimError> {
    let metric = SelectionMetric::record(closed, budget);
    let mut minimal = true;
    for removed in closed.selected() {
        let reduced: Vec<EventId> = closed
            .selected()
            .iter()
            .filter(|event| *event != removed)
            .copied()
            .collect();
        let reduced_closed =
            close_selection(store, context, &reduced, &reduced, &no_add_policy(), limits).await?;
        let claim =
            check_sufficiency(store, context, genesis, critical, &reduced_closed, limits).await?;
        if claim.sufficient {
            minimal = false;
            break;
        }
    }
    Ok(MinimalityClaim {
        minimal,
        metric,
        basis: ClaimBasis::Metric,
    })
}

/// The check closure policy: a kind that never appears in the B8 eval chains,
/// so the closure adds nothing and the checks measure the selection alone.
fn no_add_policy() -> CriticalPolicy {
    CriticalPolicy::new(vec!["ob10.no-critical-kind".to_owned()]).expect("static no-add policy")
}
