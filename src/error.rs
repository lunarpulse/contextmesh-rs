//! Typed, non-secret OA-01 failures.

use thiserror::Error;

/// Result type used by the signed-event contract.
pub type Result<T> = std::result::Result<T, ContractError>;

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
