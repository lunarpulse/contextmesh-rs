//! Option C salience-provenance primitives for ContextMesh.
//!
//! OC-01 Stage 1A established the crate and dependency boundary. Stage 2A
//! adds the frozen artifact error categories, the local strict JSON/JCS
//! helpers, and the checked OutcomeLedgerV1 value types with their typed
//! text encodings and limits. The crate still does not compose a body or
//! envelope, assign causal credit, infer task outcome, select context,
//! invoke a model or judge, access a network, or add storage.

#![warn(missing_docs)]

pub mod error;
pub mod json;
pub mod types;
