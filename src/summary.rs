//! Option B hierarchical and project summaries (gate B9).
//!
//! Derived, verifiable summaries at three hierarchical levels — event → ref →
//! project — as content-addressed references over Option A history, so a
//! recipient can enter a large history at the right altitude. Every summary
//! is a content-addressed record: its `SummaryId` commits to the canonical
//! wire of its payload (level, context, covered events, derived note), and it
//! references exactly the events it summarizes. Verification against the DAG
//! recomputes the content address (a tampered summary is rejected) and checks
//! that every referenced event is present in the DAG under the summary's
//! context (a drifted summary is rejected). Summaries are derived records,
//! never Option A history; the store is read read-only.

use std::fmt;
use std::str::FromStr;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::closure::ClosureLimits;
use crate::delta::{DeltaError, RecipientState};
use crate::error::{ContractError, StoreError};
use crate::model::{ContextId, EventId, SignedEventV1};
use crate::store::{LocalRefName, Store};

/// BLAKE3 derive-key context frozen for version-1 summary IDs.
pub const SUMMARY_ID_DOMAIN: &str = "org.aaif.contextmesh.summary-id.v1";
/// Hard maximum byte size of one derived summary note.
pub const MAX_SUMMARY_NOTE_BYTES: usize = 1024;

/// A BLAKE3-derived immutable summary identifier (sum1_ plus 32 bytes).
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct SummaryId([u8; 32]);

impl SummaryId {
    /// Constructs the typed value from its exact raw bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns a copy of the exact raw bytes.
    #[must_use]
    pub const fn to_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Display for SummaryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("sum1_")?;
        formatter.write_str(&URL_SAFE_NO_PAD.encode(self.0))
    }
}

impl fmt::Debug for SummaryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl FromStr for SummaryId {
    type Err = ContractError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        const ENCODED_LEN: usize = (32usize * 8).div_ceil(6);
        if !text.starts_with("sum1_") || text.len() != "sum1_".len() + ENCODED_LEN {
            return Err(ContractError::InvalidEncoding);
        }
        let encoded = &text["sum1_".len()..];
        let decoded = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| ContractError::InvalidEncoding)?;
        let bytes: [u8; 32] = decoded
            .try_into()
            .map_err(|_| ContractError::InvalidEncoding)?;
        let value = Self(bytes);
        if value.to_string() != text {
            return Err(ContractError::InvalidEncoding);
        }
        Ok(value)
    }
}

impl Serialize for SummaryId {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SummaryId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// The summary hierarchy level (gate B9).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SummaryLevel {
    /// A single-event summary.
    Event,
    /// A local-ref summary covering the ref's ancestry.
    Ref,
    /// A project (context) summary covering the context's events.
    Project,
}

/// The content-addressed payload of a summary (gate B9).
///
/// The payload is what the summary ID commits to; it carries the level, the
/// context, exactly the covered events, and the derived note.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "level", rename_all = "kebab-case")]
pub enum SummaryPayload {
    /// One event, its kind, and the derived note.
    Event {
        /// The context the event belongs to.
        context: ContextId,
        /// The single covered event (content-addressed reference).
        event: EventId,
        /// The event's kind.
        kind: String,
        /// The derived summary note.
        note: String,
    },
    /// A local ref, its head, and the exact ancestry it covers.
    Ref {
        /// The context the ref belongs to.
        context: ContextId,
        /// The local ref name.
        ref_name: String,
        /// The ref's current head.
        head: EventId,
        /// Exactly the covered events (the ref's ancestry, canonical order).
        events: Vec<EventId>,
        /// The derived summary note.
        note: String,
    },
    /// A project (context) and the exact events it covers.
    Project {
        /// The context being summarized.
        context: ContextId,
        /// Exactly the covered events (canonical order).
        events: Vec<EventId>,
        /// The derived summary note.
        note: String,
    },
}

impl SummaryPayload {
    /// Returns the summary's context.
    #[must_use]
    pub const fn context(&self) -> ContextId {
        match self {
            Self::Event { context, .. } => *context,
            Self::Ref { context, .. } => *context,
            Self::Project { context, .. } => *context,
        }
    }

    /// Returns exactly the events this summary references (its covered set).
    #[must_use]
    pub fn covered(&self) -> Vec<EventId> {
        match self {
            Self::Event { event, .. } => vec![*event],
            Self::Ref { events, .. } => events.clone(),
            Self::Project { events, .. } => events.clone(),
        }
    }

    /// Returns the summary level.
    #[must_use]
    pub const fn level(&self) -> SummaryLevel {
        match self {
            Self::Event { .. } => SummaryLevel::Event,
            Self::Ref { .. } => SummaryLevel::Ref,
            Self::Project { .. } => SummaryLevel::Project,
        }
    }
}

/// A content-addressed hierarchical summary (gate B9).
///
/// The summary ID is derived from the payload's canonical wire; the record
/// therefore cannot be tampered with without breaking its own content
/// address, and verification rejects both tampered and drifted records.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// The content-addressed identity of this summary.
    pub summary_id: SummaryId,
    /// The payload the identity commits to.
    pub payload: SummaryPayload,
}

impl Summary {
    /// Derives and returns the content address of a payload.
    fn derive(payload: &SummaryPayload) -> Result<SummaryId, SummaryError> {
        let canonical = crate::model::canonicalize(payload).map_err(|_| SummaryError::Internal)?;
        let mut hasher = blake3::Hasher::new_derive_key(SUMMARY_ID_DOMAIN);
        hasher.update(&canonical);
        Ok(SummaryId::from_bytes(*hasher.finalize().as_bytes()))
    }

    /// Builds a summary from a payload and derives its content address.
    fn from_payload(payload: SummaryPayload) -> Result<Self, SummaryError> {
        let summary_id = Self::derive(&payload)?;
        Ok(Self {
            summary_id,
            payload,
        })
    }

    /// Builds an event-level summary over one event in the DAG.
    ///
    /// The event must be present in the store under the stated context, or
    /// the build fails closed.
    pub async fn event(
        store: &Store,
        context: ContextId,
        event: EventId,
    ) -> Result<Self, SummaryError> {
        let stored = store
            .event(event)
            .await?
            .ok_or(SummaryError::Drifted { event })?;
        if stored.body().context() != context {
            return Err(SummaryError::WrongContext { event });
        }
        let kind = stored.body().kind().to_owned();
        let note = derived_note(&stored);
        Self::from_payload(SummaryPayload::Event {
            context,
            event,
            kind,
            note,
        })
    }

    /// Builds a ref-level summary over a local ref's ancestry.
    ///
    /// The covered set is exactly the ref's verified ancestry (the same
    /// strict walk the B4 recipient state uses), and the ref must exist or
    /// the build fails closed.
    pub async fn ref_summary(
        store: &Store,
        context: ContextId,
        name: &LocalRefName,
        limits: &ClosureLimits,
    ) -> Result<Self, SummaryError> {
        let head = store
            .local_ref(context, name)
            .await?
            .ok_or(SummaryError::UnknownRef {
                context,
                name: name.as_str().to_owned(),
            })?;
        let recipient = RecipientState::at_head(store, context, head, limits).await?;
        let events = recipient.closure().to_vec();
        if events.is_empty() {
            return Err(SummaryError::Empty);
        }
        let note = format!(
            "ref {} at {} covering {} events",
            name.as_str(),
            head,
            events.len()
        );
        Self::from_payload(SummaryPayload::Ref {
            context,
            ref_name: name.as_str().to_owned(),
            head,
            events,
            note,
        })
    }

    /// Builds a project-level summary over a context's events.
    ///
    /// The covered set is the canonical union of every local ref's verified
    /// ancestry in the context; a context with no events fails closed.
    pub async fn project(
        store: &Store,
        context: ContextId,
        limits: &ClosureLimits,
    ) -> Result<Self, SummaryError> {
        let refs = store.list_local_refs(context).await?;
        let mut covered: Vec<EventId> = Vec::new();
        for reference in &refs {
            let recipient = RecipientState::at_head(store, context, reference.head, limits).await?;
            for event in recipient.closure() {
                covered.push(*event);
            }
        }
        covered.sort();
        covered.dedup();
        if covered.is_empty() {
            return Err(SummaryError::Empty);
        }
        let note = format!("project {} covering {} events", context, covered.len());
        Self::from_payload(SummaryPayload::Project {
            context,
            events: covered,
            note,
        })
    }

    /// Returns the summary's content address.
    #[must_use]
    pub const fn summary_id(&self) -> SummaryId {
        self.summary_id
    }

    /// Returns the summary payload.
    #[must_use]
    pub const fn payload(&self) -> &SummaryPayload {
        &self.payload
    }

    /// Returns the summary's context.
    #[must_use]
    pub const fn context(&self) -> ContextId {
        self.payload.context()
    }

    /// Returns the summary level.
    #[must_use]
    pub const fn level(&self) -> SummaryLevel {
        self.payload.level()
    }

    /// Returns exactly the events this summary references.
    #[must_use]
    pub fn covered(&self) -> Vec<EventId> {
        self.payload.covered()
    }

    /// Renders the summary as canonical RFC 8785/JCS wire bytes.
    pub fn to_wire(&self) -> Result<Vec<u8>, SummaryError> {
        crate::model::canonicalize(self).map_err(|_| SummaryError::Internal)
    }

    /// Verifies the summary against the DAG.
    ///
    /// The content address must be derivable from the payload (a tampered
    /// summary is rejected) and every referenced event must be present in the
    /// store under the summary's context (a drifted summary is rejected).
    pub async fn verify_against_dag(
        &self,
        store: &Store,
    ) -> Result<SummaryVerification, SummaryError> {
        let expected = Self::derive(&self.payload)?;
        if expected != self.summary_id {
            return Err(SummaryError::Tampered {
                recorded: self.summary_id,
                recomputed: expected,
            });
        }
        let mut checked = 0usize;
        for event in self.covered() {
            match store.event(event).await? {
                Some(stored) if stored.body().context() == self.context() => checked += 1,
                Some(_) => return Err(SummaryError::WrongContext { event }),
                None => return Err(SummaryError::Drifted { event }),
            }
        }
        Ok(SummaryVerification {
            valid: true,
            checked,
        })
    }
}

/// The outcome of verifying a summary against the DAG.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SummaryVerification {
    /// True when the summary is self-consistent and fully present.
    pub valid: bool,
    /// Number of referenced events verified against the DAG.
    pub checked: usize,
}

/// Stable typed summary failures (gate B9).
#[derive(Debug, Error)]
pub enum SummaryError {
    /// A read-only store operation failed.
    #[error("summary store operation failed")]
    Store(#[from] StoreError),
    /// The ancestry walk used to derive the covered set failed.
    #[error("summary ancestry derivation failed")]
    Delta(#[from] DeltaError),
    /// The summary's content address no longer matches its payload.
    #[error("summary is tampered: recorded {recorded}, recomputed {recomputed}")]
    Tampered {
        /// The recorded content address.
        recorded: SummaryId,
        /// The recomputed content address.
        recomputed: SummaryId,
    },
    /// A referenced event is no longer present in the DAG.
    #[error("summary references a drifted event: {event}")]
    Drifted {
        /// The missing referenced event.
        event: EventId,
    },
    /// A referenced event belongs to a different context.
    #[error("summary references an event outside its context: {event}")]
    WrongContext {
        /// The mis-contexted referenced event.
        event: EventId,
    },
    /// The stated local ref does not exist.
    #[error("summary ref does not exist in context {context}: {name}")]
    UnknownRef {
        /// The context the ref should belong to.
        context: ContextId,
        /// The missing local ref name.
        name: String,
    },
    /// The covered set is empty: nothing to summarize.
    #[error("summary has no covered events")]
    Empty,
    /// An internal checked invariant failed.
    #[error("summary internal failure")]
    Internal,
}

/// Derives a deterministic, bounded summary note from a stored event.
fn derived_note(event: &SignedEventV1) -> String {
    let fallback = event.body().kind();
    let note = event
        .body()
        .payload()
        .get("note")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(fallback);
    if note.len() > MAX_SUMMARY_NOTE_BYTES {
        let mut bound = MAX_SUMMARY_NOTE_BYTES;
        while !note.is_char_boundary(bound) {
            bound -= 1;
        }
        note[..bound].to_owned()
    } else {
        note.to_owned()
    }
}
