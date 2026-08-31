//! Option C salience-provenance primitives for ContextMesh.
//!
//! OC-01 Stages 1A and 2A established the crate boundary, frozen errors,
//! strict JSON/JCS helpers, and checked value types. Stage 2B added the
//! OutcomeLedgerV1 body/envelope with structural ID and signature
//! verification. Stage 2C committed the golden fixtures and crypto/tamper
//! vectors. Stage 2D added store-aware issuance plus DAG, context, and
//! current-input verification. Stage 2E adds bounded regular-file
//! import/export. OC-02 Stage 2F adds deterministic orchestration across a
//! caller-supplied judge trait boundary; concrete model inference remains
//! out-of-tree.
//! The crate does not select context, embed a model client, access a network,
//! read wall-clock time, or add storage.

#![warn(missing_docs)]

pub mod attribution;
pub mod attribution_report;
pub mod error;
pub mod io;
pub mod json;
pub mod judge;
pub mod oc04_rerank;
pub mod oc04_selection;
pub mod oc04_union;
pub mod outcome;
pub mod prior;
pub mod types;
pub mod verify;
