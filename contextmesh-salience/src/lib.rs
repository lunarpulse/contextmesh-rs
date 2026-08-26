//! Option C salience-provenance primitives for ContextMesh.
//!
//! OC-01 Stages 1A and 2A established the crate boundary, frozen errors,
//! strict JSON/JCS helpers, and checked value types. Stage 2B added the
//! OutcomeLedgerV1 body/envelope with structural ID and signature
//! verification. Stage 2C committed the golden fixtures and crypto/tamper
//! vectors. Stage 2D added store-aware issuance plus DAG, context, and
//! current-input verification. Stage 2E adds bounded regular-file
//! import/export.
//! The crate does not assign causal credit, infer task outcome, select
//! context, invoke a model or judge, access a network, or add storage.

#![warn(missing_docs)]

pub mod attribution;
pub mod error;
pub mod io;
pub mod json;
pub mod outcome;
pub mod types;
pub mod verify;
