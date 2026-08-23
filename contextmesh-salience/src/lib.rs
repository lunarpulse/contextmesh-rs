//! Option C salience-provenance primitives for ContextMesh.
//!
//! OC-01 Stages 1A and 2A established the crate boundary, frozen errors,
//! strict JSON/JCS helpers, and checked value types. Stage 2B adds the
//! OutcomeLedgerV1 body/envelope with structural ID and signature verification.
//! The crate does not yet verify a DAG or current refs, perform I/O, assign
//! causal credit, infer task outcome, select context, invoke a model or judge,
//! access a network, or add storage.

#![warn(missing_docs)]

pub mod error;
pub mod json;
pub mod outcome;
pub mod types;
