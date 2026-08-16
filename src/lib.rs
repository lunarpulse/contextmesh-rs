//! contextmesh provides the frozen OA-01 signed-event contract plus OA-02/OA-03
//! transactional persistence, explicit DAG/ref operations, bounded bundles,
//! deterministic ancestry projection, and full-store integrity verification.
//!
//! These facilities authenticate and preserve caller-selected history. They do
//! not infer relevance or truth, provide consensus, or perform provider/network
//! operations; those remain explicit later work packages.

#![warn(missing_docs)]

/// Domain-separated signing, hashing, and verification primitives.
pub mod crypto;
/// Typed non-secret contract failures.
pub mod error;
/// Authenticated bounded HTTP/1 transport for pull synchronization.
pub mod http;
/// Strict versioned event and wire-format types.
pub mod model;
/// Transport-neutral provider recording boundary (reserved for OA-05).
pub mod provider;
/// Embedded Turso persistence, DAG/ref operations, bundles, and verification.
pub mod store;
/// Strict synchronization protocol, pull state machine, and reports.
pub mod sync;
