//! contextmesh provides the frozen OA-01 signed-event contract plus OA-02/OA-03
//! transactional persistence, explicit DAG/ref operations, bounded bundles,
//! deterministic ancestry projection, full-store integrity verification, and
//! OA-04 authenticated pull synchronization.
//!
//! These facilities authenticate and preserve caller-selected history. They do
//! not infer relevance or truth, provide consensus, or perform provider/network
//! operations; those remain explicit later work packages.
//!
//! Option B adds task-conditioned source selection and bounded context
//! compilation over that history: self-contained signed receipts (OB-01) and
//! the deterministic baseline selector with budget enforcement (OB-02).
//! Relevance scoring is a derived, recorded Option B artifact — it never
//! rewrites or extends Option A history.

#![warn(missing_docs)]

/// Stable automation command-line interface.
pub mod cli;
/// Option B dependency closure and critical-risk coverage.
pub mod closure;
/// Option B context compiler: bounded source-reference assembly.
pub mod compiler;
/// Domain-separated signing, hashing, and verification primitives.
pub mod crypto;
/// Typed non-secret contract failures.
pub mod error;
/// Authenticated bounded HTTP/1 transport for pull synchronization.
pub mod http;
/// Strict versioned event and wire-format types.
pub mod model;
/// Provider recording boundary and invocation contract.
pub mod provider;
/// Option B agent-experience receipts and derived selection layer.
pub mod receipt;
/// Option B task-conditioned source selection core.
pub mod selection;
/// Embedded Turso persistence, DAG/ref operations, bundles, and verification.
pub mod store;
/// Strict synchronization protocol, pull state machine, and reports.
pub mod sync;
