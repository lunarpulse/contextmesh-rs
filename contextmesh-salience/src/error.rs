//! Stable, non-secret OC-01 artifact error categories and the separate
//! operational wrapper used by store-aware and file-aware APIs.
//!
//! The twelve [`OutcomeError`] values are the exact frozen artifact/wire
//! semantic categories of the OC-01 OutcomeLedgerV1 contract. They are never
//! overloaded with operational failures: store and filesystem failures use
//! [`OutcomeOperationError`], whose `Display`, `Debug`, report, and gate
//! surfaces are generic and contain no caller data, while the `source` chain
//! retains the typed cause for programmatic inspection only.

use std::fmt;

use thiserror::Error;

/// Exact frozen artifact/wire semantic categories for OutcomeLedgerV1.
///
/// Display text is the stable category string and includes no path, input
/// fragment, task text, note, mechanism text, payload, key, signature, or
/// provider response.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, PartialEq)]
pub enum OutcomeError {
    /// Structural, typing, duplicate, unknown, or missing JSON failure.
    #[error("malformed")]
    Malformed,
    /// Semantically valid bytes that are not exact canonical JCS.
    #[error("noncanonical")]
    Noncanonical,
    /// The body version is not the frozen v1 version.
    #[error("unsupported-version")]
    UnsupportedVersion,
    /// A caller limit or frozen hard maximum was exceeded.
    #[error("limit-exceeded")]
    LimitExceeded,
    /// A derived ID, fingerprint binding, or author match failed.
    #[error("id-mismatch")]
    IdMismatch,
    /// The domain signature failed strict verification.
    #[error("signature-invalid")]
    SignatureInvalid,
    /// A referenced event is absent from the store.
    #[error("missing-event")]
    MissingEvent,
    /// Reserved: a future approved Store authorization failure.
    #[error("unauthorized-event")]
    UnauthorizedEvent,
    /// A loaded event belongs to another context.
    #[error("context-mismatch")]
    ContextMismatch,
    /// The embedded input-ref snapshot no longer matches a fresh capture.
    #[error("stale-input")]
    StaleInput,
    /// Reserved: a required mechanism is not available.
    #[error("mechanism-unavailable")]
    MechanismUnavailable,
    /// Reserved: caller input is incomplete for the operation.
    #[error("incomplete-input")]
    IncompleteInput,
}

impl OutcomeError {
    /// Returns the exact frozen stable category text for this value.
    #[must_use]
    pub const fn stable_category(self) -> &'static str {
        match self {
            Self::Malformed => "malformed",
            Self::Noncanonical => "noncanonical",
            Self::UnsupportedVersion => "unsupported-version",
            Self::LimitExceeded => "limit-exceeded",
            Self::IdMismatch => "id-mismatch",
            Self::SignatureInvalid => "signature-invalid",
            Self::MissingEvent => "missing-event",
            Self::UnauthorizedEvent => "unauthorized-event",
            Self::ContextMismatch => "context-mismatch",
            Self::StaleInput => "stale-input",
            Self::MechanismUnavailable => "mechanism-unavailable",
            Self::IncompleteInput => "incomplete-input",
        }
    }
}

/// Non-wire wrapper for store-aware and verified-file OC-01 APIs.
///
/// `Display` strings, the custom `Debug` output, verification reports, and
/// gate output are generic (`outcome artifact operation failed`,
/// `outcome store operation failed`, `outcome file operation failed`). The
/// `source` chain retains the typed cause for programmatic inspection.
/// Arbitrary `std::io::Error` source text is not certified non-secret and
/// callers must not log or export traversed I/O sources.
#[derive(Error)]
pub enum OutcomeOperationError {
    /// An artifact/wire semantic failure with a stable category.
    #[error("outcome artifact operation failed")]
    Artifact(#[source] OutcomeError),
    /// An operational store failure, never remapped to a wire category.
    #[error("outcome store operation failed")]
    Store(#[source] contextmesh::error::StoreError),
    /// An import/export filesystem operation failed.
    #[error("outcome file operation failed")]
    Io(#[source] std::io::Error),
}

impl fmt::Debug for OutcomeOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Generic, non-secret surface: variant names only, never inner data.
        match self {
            Self::Artifact(_) => formatter.write_str("OutcomeOperationError::Artifact"),
            Self::Store(_) => formatter.write_str("OutcomeOperationError::Store"),
            Self::Io(_) => formatter.write_str("OutcomeOperationError::Io"),
        }
    }
}

impl From<OutcomeError> for OutcomeOperationError {
    fn from(value: OutcomeError) -> Self {
        Self::Artifact(value)
    }
}

impl From<contextmesh::error::StoreError> for OutcomeOperationError {
    fn from(value: contextmesh::error::StoreError) -> Self {
        Self::Store(value)
    }
}

impl From<std::io::Error> for OutcomeOperationError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Result alias for store-aware and file-aware OC-01 APIs.
pub type OutcomeOperationResult<T> = Result<T, OutcomeOperationError>;
