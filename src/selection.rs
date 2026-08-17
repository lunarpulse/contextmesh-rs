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

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::compiler::{CompiledContext, compile_context};
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
