//! contextmesh provides the OA-01 frozen, persistence-independent signed-event
//! contract for Option A's verifiable distributed agent history.
//!
//! OA-01 authenticates canonical immutable event bodies. Persistence, parent
//! existence, context membership, authorization, refs, networking, provider
//! execution, and semantic context selection remain later work packages.

#![warn(missing_docs)]

/// Domain-separated signing, hashing, and verification primitives.
pub mod crypto;
/// Typed non-secret contract failures.
pub mod error;
/// Authenticated HTTP transport (reserved for OA-04).
pub mod http;
/// Strict versioned event and wire-format types.
pub mod model;
/// Transport-neutral provider recording boundary (reserved for OA-05).
pub mod provider;
/// Embedded Turso persistence and DAG/ref operations (reserved for OA-02).
pub mod store;
/// Signed-event anti-entropy synchronization (reserved for OA-04).
pub mod sync;
