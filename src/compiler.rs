//! Option B context compiler (gate B2).
//!
//! Assembles the bounded set of source references from selector output. The
//! selection budget — maximum selected event count plus maximum exported byte
//! size — is enforced here at handoff time: over-budget selections are
//! refused with a typed `BudgetExceeded` error, never silently truncated.

use serde::Serialize;

use crate::selection::{
    MAX_SELECTED_BYTES, MAX_SELECTED_EVENTS, SelectionBudget, SelectionError, SourceReference,
};

/// A bounded, compiled set of source references for one handoff.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompiledContext {
    references: Vec<SourceReference>,
    budget: SelectionBudget,
    total_bytes: usize,
}

impl CompiledContext {
    /// Compiles the given references under the budget.
    ///
    /// Refuses (never truncates) when the reference count or the total
    /// exported byte size exceeds either the stated budget or the hard
    /// ceilings.
    pub fn compile(
        references: Vec<SourceReference>,
        budget: &SelectionBudget,
    ) -> Result<Self, SelectionError> {
        if references.len() > MAX_SELECTED_EVENTS || references.len() > budget.max_selected_events {
            return Err(SelectionError::BudgetExceeded);
        }
        let total_bytes = references
            .iter()
            .map(SourceReference::payload_bytes)
            .sum::<usize>();
        if total_bytes > MAX_SELECTED_BYTES || total_bytes > budget.max_exported_bytes {
            return Err(SelectionError::BudgetExceeded);
        }
        Ok(Self {
            references,
            budget: *budget,
            total_bytes,
        })
    }

    /// Returns the compiled source references in selector-ranked order.
    #[must_use]
    pub fn references(&self) -> &[SourceReference] {
        &self.references
    }

    /// Returns the budget the compilation was performed under.
    #[must_use]
    pub const fn budget(&self) -> SelectionBudget {
        self.budget
    }

    /// Returns the total exported byte size of the compiled references.
    #[must_use]
    pub const fn total_bytes(&self) -> usize {
        self.total_bytes
    }
}

/// Compiles the given references under the budget.
///
/// Convenience form of [`CompiledContext::compile`].
pub fn compile_context(
    references: Vec<SourceReference>,
    budget: &SelectionBudget,
) -> Result<CompiledContext, SelectionError> {
    CompiledContext::compile(references, budget)
}
