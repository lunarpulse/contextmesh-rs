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
//!
//! OB-03 and OB-04 extend that layer with dependency closure and critical/risk
//! coverage, and the recipient-known-history delta for state-safe handoff.
//!
//! OB-05 binds that delta to the recipient head it was computed against: a
//! handoff is valid only against that head, and a stale handoff is rejected
//! and re-derived, never applied.
//!
//! OB-06 makes omissions first-class, challengeable data: every handoff
//! carries an explicit omission list and uncertainty markers, and a
//! challenged omission is re-included in a follow-up handoff with the
//! challenge recorded — no omission is hidden.
//!
//! OB-07 runs a bounded progressive repair loop over that handoff: on
//! comprehension or task failure it iteratively re-includes omitted context
//! and re-handoffs, recording every attempt to a distinct JSON-lines history
//! file and always reporting convergence or non-convergence.
//!
//! OB-08 ships the frozen, offline evaluation suite that makes comprehension
//! measurable: a curated task set with known critical-context annotations,
//! run in two sub-modes (challenge probes and task benchmarks), where the
//! withheld-context case fails and the repaired case passes.
//!
//! OB-10 adds the claim discipline for that selection: sufficiency is claimed
//! only when the frozen B8 evaluation backs it, minimality only when the
//! recorded metric (selected count/bytes against budget) backs it, and any
//! claim beyond the metric is refused.

#![warn(missing_docs)]

/// Stable automation command-line interface.
pub mod cli;
/// Option B dependency closure and critical-risk coverage.
pub mod closure;
/// Option B context compiler: bounded source-reference assembly.
pub mod compiler;
/// Domain-separated signing, hashing, and verification primitives.
pub mod crypto;
/// Option B recipient-known-history delta.
pub mod delta;
/// Typed non-secret contract failures.
pub mod error;
/// Option B frozen comprehension and task-performance evaluation.
pub mod eval;
/// Option B state-bound handoff validity.
pub mod handoff;
/// Authenticated bounded HTTP/1 transport for pull synchronization.
pub mod http;
/// Strict versioned event and wire-format types.
pub mod model;
/// Provider recording boundary and invocation contract.
pub mod provider;
/// Option B agent-experience receipts and derived selection layer.
pub mod receipt;
/// Option B bounded progressive context repair.
pub mod repair;
/// Option B task-conditioned source selection core.
pub mod selection;
/// Embedded Turso persistence, DAG/ref operations, bundles, and verification.
pub mod store;
/// Strict synchronization protocol, pull state machine, and reports.
pub mod sync;
