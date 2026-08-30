//! OC-04 Stage 4C — scored baseline carrier (spec §8, additive).
//!
//! `ScoredSelection` exposes the raw lexical TF score that
//! [`Selector::select`] discards, plus the best-first rank within the arm,
//! so the OC-04 union/rerank pipeline can normalize per arm. `select` is
//! UNTOUCHED (X04 invariant): `select_scored` reuses the exact scoring and
//! ordering discipline of `select` and its output, filtered to score > 0,
//! is byte-equal to `select`'s on the same inputs.

use crate::receipt::TaskRecordV1;
use crate::selection::{
    BaselineSelector, SelectionError, SourceEvent, SourceReference, score_source_u128, tokenize,
};

/// One scored candidate in the lexical arm: reference + raw TF + rank.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoredSelection {
    reference: SourceReference,
    lexical_raw: u128,
    lexical_rank: usize,
}

impl ScoredSelection {
    /// Returns the bounded source reference.
    #[must_use]
    pub const fn reference(&self) -> &SourceReference {
        &self.reference
    }

    /// Returns the raw term-frequency score (checked u128 accumulate).
    #[must_use]
    pub const fn lexical_raw(&self) -> u128 {
        self.lexical_raw
    }

    /// Returns the best-first rank within the lexical arm (0-based).
    #[must_use]
    pub const fn lexical_rank(&self) -> usize {
        self.lexical_rank
    }
}

impl BaselineSelector {
    /// Scores and ranks sources exactly as [`crate::selection::Selector::select`]
    /// does, but carries the raw score and arm rank instead of discarding
    /// them (spec §8 — additive, `select` untouched).
    ///
    /// # Errors
    /// Returns [`SelectionError::EmptyTask`] under the same conditions as
    /// `select`.
    pub fn select_scored(
        &self,
        task: &TaskRecordV1,
        sources: &[SourceEvent],
    ) -> Result<Vec<ScoredSelection>, SelectionError> {
        let task_terms = tokenize(task.verbatim());
        if task_terms.is_empty() {
            return Err(SelectionError::EmptyTask);
        }
        let mut scored: Vec<(u128, &SourceEvent)> = sources
            .iter()
            .map(|source| score_source_u128(&task_terms, source).map(|score| (score, source)))
            .collect::<Result<_, _>>()?;
        scored.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.event().to_string().cmp(&right.1.event().to_string()))
        });
        let mut ranked = Vec::new();
        for (score, source) in scored {
            if score == 0 {
                continue;
            }
            let lexical_raw = score;
            ranked.push(ScoredSelection {
                reference: SourceReference::from_source(source),
                lexical_raw,
                lexical_rank: ranked.len(),
            });
        }
        Ok(ranked)
    }
}
