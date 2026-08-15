//! contextmesh is the library boundary for Option A's verifiable distributed
//! agent-history work.
//!
//! OA-00 deliberately provides only documented module boundaries. Event,
//! cryptographic, persistence, synchronization, HTTP, and provider behavior is
//! reserved for OA-01 and later work packages.

#![warn(missing_docs)]

/// Signing, hashing, and verification primitives (reserved for OA-01).
pub mod crypto;
/// Typed library failures (reserved for OA-01).
pub mod error;
/// Authenticated HTTP transport (reserved for OA-04).
pub mod http;
/// Versioned event and wire-format types (reserved for OA-01).
pub mod model;
/// Transport-neutral provider recording boundary (reserved for OA-05).
pub mod provider;
/// Embedded Turso persistence and DAG/ref operations (reserved for OA-02).
pub mod store;
/// Signed-event anti-entropy synchronization (reserved for OA-04).
pub mod sync;
