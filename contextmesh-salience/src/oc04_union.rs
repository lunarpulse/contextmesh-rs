//! OC-04 Stage 4C — union: prior-arm candidate generation, entity→event
//! reconstruction, and the per-arm union outcome (spec §7.1).
//!
//! The `SaliencePriorV1` artifact records entity names + ppb, NOT EventIds,
//! so reconstruction is NORMATIVE: every candidate `SourceEvent` is matched
//! against the verified prior's positive vector via `derive_entity_keys` on
//! the canonical payload text. An event's raw prior value is the MAX ppb
//! over its matching entities (sum rejected as unbounded-scale); events
//! with no positive match are not prior-arm candidates; orphan prior
//! entities increment the fail-closed counter. Reconstruction iterates the
//! candidate pool in its canonical order — no HashMap iteration.

use crate::error::OutcomeError;
use crate::oc04_selection::{ORPHAN_PRIOR_ENTITIES_MAX, Oc04ConfigV1, VerifiedPrior};
use crate::prior::{self, PriorSeedV1};
use contextmesh::oc04_scored::ScoredSelection;
use contextmesh::selection::SourceEvent;

/// One prior-arm candidate: the raw max-folded ppb over matching entities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriorCandidate {
    event: String,
    raw_ppb: u128,
}

impl PriorCandidate {
    /// Canonical EventId text.
    #[must_use]
    pub fn event(&self) -> &str {
        &self.event
    }

    /// Raw ppb value (max over matching entities, bounded by PRIOR_MAX_PPB).
    #[must_use]
    pub const fn raw_ppb(&self) -> u128 {
        self.raw_ppb
    }
}

/// One EventId-deduplicated union candidate. Raw arm values are retained for
/// 4D's independent normalization; arm presence derives the frozen reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionCandidate {
    event: String,
    lexical_raw: Option<u128>,
    prior_raw: Option<u128>,
}

impl UnionCandidate {
    /// Canonical EventId text.
    #[must_use]
    pub fn event(&self) -> &str {
        &self.event
    }
    /// Raw lexical arm value when this EventId entered that arm.
    #[must_use]
    pub const fn lexical_raw(&self) -> Option<u128> {
        self.lexical_raw
    }
    /// Raw prior arm value when this EventId entered that arm.
    #[must_use]
    pub const fn prior_raw(&self) -> Option<u128> {
        self.prior_raw
    }
    /// Frozen union reason: `lexical`, `prior`, or `both`.
    #[must_use]
    pub fn reason(&self) -> &'static str {
        match (self.lexical_raw.is_some(), self.prior_raw.is_some()) {
            (true, true) => crate::oc04_selection::ENTRY_REASON_BOTH,
            (true, false) => crate::oc04_selection::ENTRY_REASON_LEXICAL,
            (false, true) => crate::oc04_selection::ENTRY_REASON_PRIOR,
            (false, false) => unreachable!("union candidates always have an arm"),
        }
    }
}

/// The per-arm union outcome prior to normalization/rerank (spec §7.1/§7.2
/// inputs). Carries the capped lexical arm references in arm order, capped
/// prior-arm candidates in ppb-descending/EventId-text-ascending order, the
/// EventId-deduplicated union with reasons, and the orphan counter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionOutcomeV1 {
    lexical: Vec<String>,
    prior: Vec<PriorCandidate>,
    entries: Vec<UnionCandidate>,
    orphan_prior_entities: u32,
}

impl UnionOutcomeV1 {
    /// Lexical-arm EventId canonical texts in arm (score-desc) order.
    #[must_use]
    pub fn lexical(&self) -> &[String] {
        &self.lexical
    }

    /// Prior-arm candidates in canonical EventId-text order.
    #[must_use]
    pub fn prior(&self) -> &[PriorCandidate] {
        &self.prior
    }

    /// EventId-deduplicated union in canonical EventId-text order. Entry
    /// reasons are derived from arm membership (`lexical`/`prior`/`both`).
    #[must_use]
    pub fn entries(&self) -> &[UnionCandidate] {
        &self.entries
    }

    /// Orphan prior entities counted during reconstruction (U04).
    #[must_use]
    pub const fn orphan_prior_entities(&self) -> u32 {
        self.orphan_prior_entities
    }
}

/// Unions the capped lexical arm with reconstructed prior-arm candidates
/// (spec §7.1): per-arm caps enforced pre-union (64 lexical / 30 prior),
/// max-fold ppb matching, orphan counting with the fail-closed 1024 bound,
/// deterministic canonical-order reconstruction. The EventId-deduplicated
/// entries and their `lexical`/`prior`/`both` reasons are exposed by the 4C
/// outcome; 4D independently normalizes and reranks their retained raw arms.
///
/// # Errors
/// Returns [`OutcomeError::Malformed`] when orphan prior entities exceed
/// `ORPHAN_PRIOR_ENTITIES_MAX` (fail-closed, X10).
pub fn union_candidates(
    lexical: &[ScoredSelection],
    prior: &VerifiedPrior,
    sources: &[SourceEvent],
    config: &Oc04ConfigV1,
) -> Result<UnionOutcomeV1, OutcomeError> {
    config.validate_frozen()?;

    // Lexical arm: cap pre-union (arm already arrives score-desc; cap keeps
    // the first LEXICAL_ARM_CAP entries).
    let lexical_ids: Vec<String> = lexical
        .iter()
        .take(config.lexical_arm_cap as usize)
        .map(|scored| scored.reference().event().to_string())
        .collect();

    // Reconstruction table: entity key → EventId canonical text, built in
    // the candidate pool's canonical order (no HashMap iteration anywhere;
    // lookups are the only map use).
    let seeds: &[PriorSeedV1] = prior.positive_seeds();
    let mut event_of_entity: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut orphans: u32 = 0;
    // Fold MAX per event: event → raw ppb (max over its entities).
    let mut event_ppb: Vec<(String, u128)> = Vec::new();

    let mut canonical_sources: Vec<&SourceEvent> = sources.iter().collect();
    canonical_sources.sort_by_key(|source| source.event());
    for source in canonical_sources {
        let event_text = source.event().to_string();
        let keys = prior::derive_entity_keys(source.text());
        let mut best: Option<u128> = None;
        for seed in seeds {
            let matched = keys.iter().any(|key| *key == seed.entity());
            if matched {
                let ppb = seed.ppb();
                if ppb > 0 {
                    best = Some(match best {
                        Some(current) if current >= ppb => current,
                        _ => ppb,
                    });
                }
            }
        }
        if let Some(ppb) = best {
            // Duplicate EventId in the pool: keep the max fold.
            if let Some(slot) = event_ppb.iter_mut().find(|(ev, _)| *ev == event_text) {
                if ppb > slot.1 {
                    slot.1 = ppb;
                }
            } else {
                event_ppb.push((event_text.clone(), ppb));
            }
            for key in &keys {
                event_of_entity
                    .entry(key.as_str().to_owned())
                    .or_insert(event_text.clone());
            }
        }
    }

    // Orphan accounting: a positive vector entity that matched no candidate
    // event is orphaned (counted, fail-closed at the 1024 bound).
    for seed in seeds {
        if seed.ppb() > 0 && !event_of_entity.contains_key(seed.entity()) {
            orphans = orphans.checked_add(1).ok_or(OutcomeError::Malformed)?;
            if orphans > ORPHAN_PRIOR_ENTITIES_MAX {
                return Err(OutcomeError::Malformed);
            }
        }
    }

    // Prior arm: max-folded events ranked by ppb descending, then canonical
    // EventId text ascending, and capped before union.
    event_ppb.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    event_ppb.truncate(config.prior_arm_cap as usize);
    let prior_arm = event_ppb
        .into_iter()
        .map(|(event, raw_ppb)| PriorCandidate { event, raw_ppb })
        .collect::<Vec<PriorCandidate>>();

    // Deduplicated union: retain one EventId with both raw arms when present.
    // This is deterministic even if a caller supplied duplicate sources.
    let mut entries: Vec<UnionCandidate> = Vec::new();
    for scored in lexical.iter().take(config.lexical_arm_cap as usize) {
        let event = scored.reference().event().to_string();
        let raw = scored.lexical_raw();
        if let Some(entry) = entries.iter_mut().find(|entry| entry.event == event) {
            entry.lexical_raw = Some(entry.lexical_raw.unwrap_or(0).max(raw));
        } else {
            entries.push(UnionCandidate {
                event,
                lexical_raw: Some(raw),
                prior_raw: None,
            });
        }
    }
    for candidate in &prior_arm {
        if let Some(entry) = entries
            .iter_mut()
            .find(|entry| entry.event == candidate.event)
        {
            entry.prior_raw = Some(candidate.raw_ppb());
        } else {
            entries.push(UnionCandidate {
                event: candidate.event.clone(),
                lexical_raw: None,
                prior_raw: Some(candidate.raw_ppb()),
            });
        }
    }
    entries.sort_by(|left, right| left.event.cmp(&right.event));

    Ok(UnionOutcomeV1 {
        lexical: lexical_ids,
        prior: prior_arm,
        entries,
        orphan_prior_entities: orphans,
    })
}
