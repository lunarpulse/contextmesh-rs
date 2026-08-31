//! OC-04 Stage 4D — preregistered normalization + rerank (spec §7.2) and
//! the ordered `SelectionInfluenceV1` record.
//!
//! Per-arm min-max normalization to `[0, 1_000_000]` ppm over each arm's
//! candidates, `score_ppm = lexical_ppm + prior_ppm`, rank by score desc
//! then canonical EventId text ascending (= `EventId::Ord`, canonical-text
//! order, frozen). TF=0 events enter via the prior arm with
//! `lexical_ppm = 0`.
//!
//! Membership-truth separation (founder-approved change control at the 4D
//! gate): `entry_reason` records UNION MEMBERSHIP (§7.1) — derived from the
//! retained raw-arm presence, never from normalized ppm — while the ppm
//! values record normalized relative magnitude. Min-max normalization
//! legitimately collapses an arm's minimum member to 0 ppm; that collapse
//! must not alter the recorded reason. The constructor enforces the
//! exact-string enum, the §6 ONE-WAY non-member-zero rule (`lexical` ⇒
//! `prior_ppm = 0`, `prior` ⇒ `lexical_ppm = 0`), and the checked score
//! sum; it deliberately does NOT police member-zero collapses —
//! membership-vs-arm consistency is asserted HERE against the union
//! outcome, and re-verified in the §7.5 chain at 4E.
//!
//! Multiplication/division and the u64 conversion are checked u128,
//! fail-closed per the prereg overflow policy; `max − min` and `raw −
//! min` use plain subtraction, which is safe by the extrema invariant
//! (every raw lies in [min, max]). No HashMap iteration: `entries` arrives
//! in canonical EventId-text order (the lexical/prior arm ARRAYS are
//! score-/ppb-ordered respectively and are only consumed via
//! set-membership lookups) and every map here is a lookup-only BTreeMap.

use crate::error::OutcomeError;
use crate::oc04_selection::{
    ENTRY_REASON_BOTH, ENTRY_REASON_LEXICAL, ENTRY_REASON_PRIOR, Oc04ConfigV1,
    SelectionInfluenceEntryV1, SelectionInfluenceV1, VerifiedPrior,
};
use crate::oc04_union::UnionOutcomeV1;
use std::collections::BTreeMap;

/// Normalizes one arm's raw values by per-arm min-max to
/// `[0, clip_above_ppm]` ppm (§7.2): `ppm = (raw − min) × 1e6 / (max −
/// min)`, clipped, checked u128. Degenerate single-value arm (min = max):
/// every member maps to `clip_above_ppm` if raw > 0, else `0` (the
/// arm-carrying case cannot fire in practice — both arms are positive-only
/// by construction — but the rule is implemented fail-safe as written).
fn normalize_arm(
    raws: &[(String, u128)],
    clip_above_ppm: u64,
) -> Result<BTreeMap<String, u64>, OutcomeError> {
    let mut ppm = BTreeMap::new();
    let Some(&min) = raws.iter().map(|(_, raw)| raw).min() else {
        return Ok(ppm);
    };
    let max = raws
        .iter()
        .map(|(_, raw)| raw)
        .max()
        .expect("non-empty checked above");
    if min == *max {
        // Degenerate arm (§7.2): single distinct value.
        let degenerate = if min > 0 { clip_above_ppm } else { 0 };
        for (event, _) in raws {
            ppm.insert(event.clone(), degenerate);
        }
        return Ok(ppm);
    }
    let span = max - min; // u128, > 0
    for (event, raw) in raws {
        let offset = raw - min; // >= 0
        let scaled = offset
            .checked_mul(u128::from(clip_above_ppm))
            .ok_or(OutcomeError::Malformed)?;
        let value = scaled.checked_div(span).ok_or(OutcomeError::Malformed)?;
        // Clamp to the clip bounds (raw in [min, max] keeps value in
        // [0, 1e6]; the clip is normative defense, not decoration).
        let value = value.min(u128::from(clip_above_ppm));
        ppm.insert(
            event.clone(),
            u64::try_from(value).map_err(|_| OutcomeError::Malformed)?,
        );
    }
    Ok(ppm)
}

/// Reranks the EventId-deduplicated union (spec §7.2): independently
/// normalizes the lexical arm (raw TF of the lexical members) and the prior
/// arm (raw ppb of the prior members), combines `score_ppm =
/// lexical_ppm + prior_ppm` (checked), ranks by score desc then canonical
/// EventId text ascending, and assembles the ordered `SelectionInfluenceV1`
/// with `entry_reason` derived from UNION MEMBERSHIP (retained raw-arm
/// presence in the outcome — never from normalized ppm).
///
/// `prior_id` is copied structurally from the verified token (spec §6:
/// "copied from the verified token") — the only prior_id source is the
/// `VerifiedPrior` itself. `task_fingerprint` is copied verbatim from the
/// OC-02 report (spec §6). Change control (§8 surface, founder-approved at
/// the 4D gate): the frozen §8 signature `rerank(union, config)` could not
/// populate the frozen §6 members, so the signature carries the verified
/// prior and the verbatim task fingerprint explicitly.
///
/// # Errors
/// Returns [`OutcomeError::Malformed`] on checked-arithmetic overflow, on
/// any reason-vs-membership inconsistency (fail-closed §7.5 discipline), or
/// when the influence record fails assembly validation.
pub fn rerank(
    union: &UnionOutcomeV1,
    prior: &VerifiedPrior,
    task_fingerprint: &str,
    config: &Oc04ConfigV1,
) -> Result<SelectionInfluenceV1, OutcomeError> {
    config.validate_frozen()?;

    // Raw lexical values: the lexical arm's retained raw TF per member.
    // The union's entries carry lexical_raw for exactly the lexical
    // members; assert set consistency with the capped arm list
    // (fail-closed). The arm list is in lexical score-desc order, NOT
    // canonical-text order, so both sides are sorted by EventId text
    // before comparison (re-review QB1 fix — a valid union whose score
    // order differs from text order must not be rejected).
    let mut lexical_raws: Vec<(String, u128)> = Vec::new();
    for candidate in union.entries() {
        if let Some(raw) = candidate.lexical_raw() {
            lexical_raws.push((candidate.event().to_owned(), raw));
        }
    }
    lexical_raws.sort_by(|a, b| a.0.cmp(&b.0));
    // Arm-side view: (event, raw) for each arm member. The arm list holds
    // EventIds only; raws come from the union entries — but ONLY accept an
    // entry as an arm member if its EventId appears in the arm list
    // (position lookup), otherwise a hand-built outcome could smuggle an
    // entry that is absent from the arm.
    let mut lexical_arm: Vec<(&str, u128)> = union
        .lexical()
        .iter()
        .map(|event| {
            let raw = union
                .entries()
                .iter()
                .find(|c| c.event() == event.as_str())
                .and_then(|c| c.lexical_raw())
                .ok_or(OutcomeError::Malformed)?;
            Ok((event.as_str(), raw))
        })
        .collect::<Result<Vec<(&str, u128)>, OutcomeError>>()?;
    lexical_arm.sort_by(|a, b| a.0.cmp(b.0));
    if lexical_raws.len() != lexical_arm.len()
        || lexical_raws.iter().zip(lexical_arm.iter()).any(
            |((entry, entry_raw), (arm, arm_raw))| entry.as_str() != *arm || entry_raw != arm_raw,
        )
    {
        return Err(OutcomeError::Malformed);
    }

    // Raw prior values: the prior arm's retained raw ppb per member.
    // Prior arm IS canonical-text-ordered (4C normative sort), so the
    // sorted-entries comparison is direct; raw values must match exactly.
    let mut prior_raws: Vec<(String, u128)> = union
        .entries()
        .iter()
        .filter_map(|candidate| {
            candidate
                .prior_raw()
                .map(|raw| (candidate.event().to_owned(), raw))
        })
        .collect();
    prior_raws.sort_by(|a, b| a.0.cmp(&b.0));
    // Duplicate EventIds in the entries list would let one member mask a
    // missing one under any per-element lookup, so uniqueness is checked
    // FIRST (adjacent pairs after the sort).
    if prior_raws.windows(2).any(|w| w[0].0 == w[1].0) {
        return Err(OutcomeError::Malformed);
    }
    // Prior-arm bidirectional set + value equality: the union's prior
    // members must be exactly the capped prior arm with identical raw
    // ppbs. The arm list is ppb-desc/text-asc (4C normative sort), NOT
    // text-ascending, so compare as a multiset of exact (EventId, raw)
    // pairs: (a) every arm member has an entry-side exact match, and
    // (b) counts match per EventId — with uniqueness this makes a
    // duplicate-masked-missing-member forgery impossible (round-2
    // blocker fix: [A, A] vs {A, B} now rejected by (b)).
    if prior_raws.len() != union.prior().len()
        || union.prior().iter().any(|arm| {
            !prior_raws
                .iter()
                .any(|(event, raw)| event.as_str() == arm.event() && *raw == arm.raw_ppb())
        })
    {
        return Err(OutcomeError::Malformed);
    }
    let mut arm_events: Vec<&str> = union.prior().iter().map(|arm| arm.event()).collect();
    arm_events.sort_unstable();
    arm_events.dedup();
    if arm_events.len() != prior_raws.len() {
        return Err(OutcomeError::Malformed);
    }

    // Per-arm min-max normalization (§7.2), independent per arm.
    let lexical_ppm = normalize_arm(&lexical_raws, config.clip_above_ppm)?;
    let prior_ppm = normalize_arm(&prior_raws, config.clip_above_ppm)?;

    // Entries: membership-derived reason + normalized ppms + checked sum.
    let mut entries = Vec::with_capacity(union.entries().len());
    for candidate in union.entries() {
        let lexical_member = candidate.lexical_raw().is_some();
        let prior_member = candidate.prior_raw().is_some();
        let reason = match (lexical_member, prior_member) {
            (true, true) => ENTRY_REASON_BOTH,
            (true, false) => ENTRY_REASON_LEXICAL,
            (false, true) => ENTRY_REASON_PRIOR,
            // Unreachable from union_candidates (every entry has an arm);
            // fail-closed rather than unreachable!().
            (false, false) => return Err(OutcomeError::Malformed),
        };
        let lexical = if lexical_member {
            lexical_ppm
                .get(candidate.event())
                .copied()
                .ok_or(OutcomeError::Malformed)?
        } else {
            // Non-member of the lexical arm: §6 mandates lexical_ppm = 0.
            0
        };
        let prior = if prior_member {
            prior_ppm
                .get(candidate.event())
                .copied()
                .ok_or(OutcomeError::Malformed)?
        } else {
            0
        };
        entries.push(SelectionInfluenceEntryV1::new(
            candidate.event(),
            reason,
            lexical,
            prior,
        )?);
    }

    // Rerank order: score desc, then canonical EventId text ascending.
    entries.sort_by(|a, b| {
        b.score_ppm()
            .cmp(&a.score_ppm())
            .then_with(|| a.event_id_text().cmp(b.event_id_text()))
    });

    SelectionInfluenceV1::assemble(config, prior.prior_id(), task_fingerprint, entries)
}
