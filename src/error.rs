//! Typed, non-secret failures for the OA-01 contract and OA-02 store.

use thiserror::Error;

use crate::model::EventId;

/// Result type used by the signed-event contract.
pub type Result<T> = std::result::Result<T, ContractError>;

/// Result type used by the transactional store.
pub type StoreResult<T> = std::result::Result<T, StoreError>;

/// Stable validation categories returned for all external-input failures.
///
/// Variants deliberately carry no input, payload, key, or signature material,
/// so displaying an error cannot disclose caller-controlled or secret data.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ContractError {
    /// Raw JSON wire input exceeds the 2 MiB bound.
    #[error("wire input exceeds the allowed size")]
    WireTooLarge,
    /// JSON is malformed, has a wrong field type, a BOM, or trailing data.
    #[error("invalid JSON syntax or field type")]
    JsonSyntax,
    /// An object contains the same member name more than once.
    #[error("duplicate JSON object member")]
    DuplicateKey,
    /// The envelope or body contains a field outside the v1 contract.
    #[error("unknown v1 field")]
    UnknownField,
    /// The envelope or body omits a required v1 field.
    #[error("missing required v1 field")]
    MissingField,
    /// The body version is not exactly version 1.
    #[error("unsupported event body version")]
    UnsupportedVersion,
    /// A typed text value has a bad prefix, length, alphabet, or canonical form.
    #[error("invalid canonical typed encoding")]
    InvalidEncoding,
    /// The event kind is outside the frozen v1 grammar or byte bound.
    #[error("invalid event kind")]
    InvalidKind,
    /// Parents are not in strictly ascending canonical EventId text order.
    #[error("parents are not strictly ordered and unique")]
    ParentOrder,
    /// A parent, nesting, payload, or body limit was exceeded.
    #[error("event contract limit exceeded")]
    LimitExceeded,
    /// An integer-valued JSON number is outside the I-JSON safe range.
    #[error("unsafe or non-finite JSON number")]
    UnsafeNumber,
    /// RFC 8785 canonicalization failed.
    #[error("RFC 8785 canonicalization failed")]
    Canonicalization,
    /// The operating system could not provide signing-key entropy.
    #[error("operating-system entropy unavailable")]
    Entropy,
    /// A supplied body author does not match the signing identity.
    #[error("event author does not match signing identity")]
    AuthorMismatch,
    /// The supplied event ID does not match the canonical body.
    #[error("event ID does not match canonical body")]
    IdMismatch,
    /// The Ed25519 public key or signature is malformed or does not verify.
    #[error("event signature is invalid")]
    SignatureInvalid,
}

/// Stable, non-secret OA-02 storage and admission failures.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StoreError {
    /// The local database could not be opened or accessed.
    #[error("local database unavailable")]
    DatabaseUnavailable,
    /// A migration failed or an existing schema is incomplete.
    #[error("database migration failed")]
    MigrationFailed,
    /// The database schema is newer than this binary supports.
    #[error("database schema is newer than supported")]
    NewerSchema,
    /// Stored rows contradict the canonical event or schema invariants.
    #[error("stored data is corrupt")]
    CorruptStorage,
    /// The context has not been explicitly provisioned.
    #[error("context is not provisioned")]
    ContextUnknown,
    /// The context already exists with different provisioning data.
    #[error("context provisioning conflicts with existing policy")]
    ContextProvisionMismatch,
    /// The pending context received the wrong genesis event.
    #[error("event does not match the provisioned genesis")]
    GenesisMismatch,
    /// The event author is not in the context's local allowlist.
    #[error("event author is not authorized for this context")]
    UnauthorizedAuthor,
    /// A required parent event is absent.
    #[error("event parent is missing: {0}")]
    ParentMissing(EventId),
    /// A parent belongs to another context.
    #[error("event parent belongs to another context: {0}")]
    ParentContextMismatch(EventId),
    /// An existing EventId maps to different canonical wire bytes.
    #[error("event identifier collides with different canonical bytes")]
    EventCollision,
    /// A local-ref or peer name is noncanonical or out of bounds.
    #[error("invalid ref or peer name")]
    InvalidRefName,
    /// A requested ref mutation does not target the admitted event and context.
    #[error("ref mutation does not match admitted event")]
    RefMutationMismatch,
    /// A referenced ref is absent.
    #[error("ref does not exist")]
    RefMissing,
    /// A ref expected to be absent already exists.
    #[error("ref already exists")]
    RefAlreadyExists,
    /// Compare-and-swap observed a different current head.
    #[error("stale ref head")]
    StaleHead {
        /// The current head, or None if the ref is absent.
        current: Option<EventId>,
    },
    /// A store-specific count or allocation bound was exceeded.
    #[error("store limit exceeded")]
    LimitExceeded,
    /// Operating-system entropy for context creation was unavailable.
    #[error("operating-system entropy unavailable")]
    EntropyUnavailable,
    /// An append attempted to use a helper-reserved event kind.
    #[error("event kind is reserved for a dedicated DAG operation")]
    ReservedEventKind,
    /// A merge request has an invalid parent shape.
    #[error("invalid merge shape")]
    InvalidMerge,
    /// A cycle was detected while projecting stored ancestry.
    #[error("stored event graph contains a cycle")]
    ProjectionCycle,
    /// A deterministic projection exceeded its requested bound.
    #[error("projection limit exceeded")]
    ProjectionLimitExceeded,
    /// Bundle JSON, fields, or embedded values are malformed.
    #[error("bundle is malformed")]
    BundleMalformed,
    /// The bundle version is unsupported.
    #[error("bundle version is unsupported")]
    BundleUnsupportedVersion,
    /// Bundle events or advertised refs are not in canonical order.
    #[error("bundle order is invalid")]
    BundleOrder,
    /// A bundle exceeded an event, ref, raw, or canonical byte bound.
    #[error("bundle limit exceeded")]
    BundleLimitExceeded,
    /// An advertised bundle ref is invalid for the bundle context.
    #[error("bundle advertised ref is invalid")]
    BundleRefInvalid,
    /// A full-verification limit is zero or above its hard maximum.
    #[error("verification limit is invalid")]
    VerificationLimitInvalid,
    /// Commit acknowledgement was indeterminate; a safe retry is required.
    #[error("database commit outcome is indeterminate")]
    IndeterminateCommit,
    /// OA-01 rejected the event before storage admission.
    #[error(transparent)]
    Contract(#[from] ContractError),
}
