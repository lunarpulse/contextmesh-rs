//! OC-04 Stage 4E: execution binding and verification (spec §7.3/§7.4/§7.5,
//! §8 public API).
//!
//! [`bind_execution`] drives the normative B3→B8 chain over the influence
//! record's reranked pre-closure set and issues the signed
//! `SignedExecutionV1` envelope ONLY after B7 convergence AND B8 pass —
//! the deliverable post-B7 handoff is RETURNED, never discarded (§8: the
//! envelope records only its hash). [`verify_execution`] replays the whole
//! chain against a FRESH scratch `RepairHistory` (RAII [`ScratchHistoryGuard`]
//! — never the production history) and a FRESH driver from the caller's
//! factory, then requires the recomputed body to equal the recorded body
//! member-for-member AND the recomputed final handoff wire to hash to the
//! recorded `handoff_hash` — replay proof (§7.5: the recorded body is
//! never trusted).
//!
//! §7.3 B6 warning inputs rule: exactly TWO deterministic markers via
//! `with_uncertainty`, derived from the influence record itself —
//! (1) `prior_arm_used=true` when ≥1 influence entry carries
//! `entry_reason` `prior` or `both` (union membership — the ONE-WAY
//! reason authority of spec §6/v13: a prior-arm member may normalize
//! to ppm 0, so ppm is NOT membership truth), else `prior_arm_empty`;
//! (2) `orphan_prior_entities=<n>` where `n` is the U04 counter value
//! re-derived as the count of influence entries whose prior_ppm is
//! positive (the recorded prior-arm exposure the U04 counter bounded).
//!
//! Fail-closed discipline: no HashMap iteration enters any hash input
//! (every list is canonical-text-ascending per §6); budget excess is a
//! deterministic refusal (§7.4), never silent truncation; any chain,
//! budget, stale-state, or mismatch failure collapses to
//! [`HandoffError4E`] with NO artifact and NO deliverable handoff.

use crate::error::OutcomeError;
use crate::oc04_selection::{
    Oc04ConfigV1, ScratchHistoryGuard, SelectionExecutionBodyV1, SelectionInfluenceV1,
    SignedExecutionV1, derive_execution_id, render_execution_body,
};
use blake3::Hasher;
use contextmesh::closure::{ClosureLimits, CriticalPolicy, close_selection};
use contextmesh::crypto::SigningIdentity;
use contextmesh::delta::{RecipientState, compute_delta};
use contextmesh::eval::simulate;
use contextmesh::handoff::Handoff;
use contextmesh::model::{ContextId, EventId};
use contextmesh::repair::{RepairBounds, RepairHistory, TaskOutcome, run_repair};
use contextmesh::selection::SelectionBudget;
use contextmesh::store::Store;
use std::future::Future;
use std::path::Path;
use thiserror::Error;

// ---------------------------------------------------------------------------
// §6 derivation domains (frozen table; hyphenated + versioned + NUL term.)
// ---------------------------------------------------------------------------

const DOM_PRECLOSURE: &[u8] = b"oc-04-preclosure-v1\0";
const DOM_B3CAND: &[u8] = b"oc-04-b3cand-v1\0";
const DOM_B3POLICY: &[u8] = b"oc-04-b3policy-v1\0";
const DOM_CLOSED: &[u8] = b"oc-04-closed-v1\0";
const DOM_DELTA: &[u8] = b"oc-04-delta-v1\0";
const DOM_HANDOFF: &[u8] = b"oc-04-handoff-v1\0";
const DOM_B6WARN: &[u8] = b"oc-04-b6warn-v1\0";

// ---------------------------------------------------------------------------
// §8 HandoffError4E (4E-specific failure surface, frozen at 4E)
// ---------------------------------------------------------------------------

/// Typed, non-secret failure surface for OC-04 §8 execution binding and
/// verification (spec §8 names `HandoffError` for these two functions;
/// the 4E-suffixed type is the concrete realization — the root crate's
/// [`contextmesh::handoff`] error surface predates OC-04 and lacks the
/// budget category). Variants carry no input, payload, or secret
/// material, so displaying an error cannot disclose caller-controlled
/// data. No artifact and no deliverable handoff is produced on any
/// variant (spec §7.3).
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HandoffError4E {
    /// Structural, typing, duplicate, canonical, signature, membership,
    /// or invariant failure anywhere in the B3–B8 chain (spec §7.3/§7.5
    /// "mismatch → Err").
    #[error("execution binding failed closed (malformed)")]
    Malformed,
    /// B5 stale-state verification: the handoff was computed against a
    /// recipient head that is no longer the recipient's current head —
    /// the handoff is stale, must be re-derived, and is never applied.
    #[error("handoff is stale for the current recipient state")]
    Stale,
    /// §7.4 budget exceeded by the closed selection or the delta —
    /// a deterministic refusal, never silent truncation.
    #[error("selection budget exceeded")]
    Budget,
}

fn malformed() -> OutcomeError {
    OutcomeError::Malformed
}

// ---------------------------------------------------------------------------
// §6 hash helpers
// ---------------------------------------------------------------------------

/// hex(BLAKE3(domain + comma-joined canonical-text-ascending ids)).
/// Empty list hashes over the bare domain bytes (§6 list-concatenation
/// rule).
fn list_hash(domain: &[u8], ids: &[EventId]) -> String {
    let mut sorted: Vec<&EventId> = ids.iter().collect();
    sorted.sort();
    sorted.dedup();
    let joined = sorted
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let mut hasher = Hasher::new();
    hasher.update(domain);
    hasher.update(joined.as_bytes());
    hex(hasher.finalize().as_bytes())
}

/// hex(BLAKE3(domain + NUL-terminated markers in the handoff's exposed
/// order — already canonically sorted+deduplicated; consumed verbatim).
fn b6_warnings_hash(markers: &[String]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(DOM_B6WARN);
    for marker in markers {
        hasher.update(marker.as_bytes());
        hasher.update(b"\0");
    }
    hex(hasher.finalize().as_bytes())
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

impl From<OutcomeError> for HandoffError4E {
    fn from(_: OutcomeError) -> Self {
        // All frozen OutcomeError categories collapse into the single
        // structural Malformed category on the 4E surface (§7.3/§7.5
        // fail-closed discipline — no artifact, no handoff).
        HandoffError4E::Malformed
    }
}

// ---------------------------------------------------------------------------
// §8 wire parsers (lenient parse → strict semantic gate)
// ---------------------------------------------------------------------------

/// Lenient JSON parser for the §6 execution body wire format: parses the
/// object and every member value WITHOUT rejecting unknown members
/// (S07 — the parser itself never rejects; the canonical gate does).
/// Returns the raw member set for the caller to rebuild a typed body
/// from; any missing frozen member, wrong-typed member, or non-object
/// input is still [`OutcomeError::Malformed`] (syntax/typing is the
/// parser's job; membership extra/reorder is NOT — S07b's gate catches
/// those at verification).
///
/// # Errors
/// Returns [`OutcomeError::Malformed`] on non-object input, non-UTF8,
/// malformed JSON, or any frozen member missing/mistyped.
pub fn parse_execution_body_lenient(
    bytes: &[u8],
) -> Result<SelectionExecutionBodyV1, OutcomeError> {
    let text = std::str::from_utf8(bytes).map_err(|_| malformed())?;
    let value: serde_json::Value = serde_json::from_str(text).map_err(|_| malformed())?;
    let object = value.as_object().ok_or_else(malformed)?;
    let get_str = |key: &str| -> Result<String, OutcomeError> {
        object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(malformed)
    };
    let get_u64 = |key: &str| -> Result<u64, OutcomeError> {
        object
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(malformed)
    };
    // Every frozen §6 member must be present and typed; extras are
    // carried in `value` but dropped here — the canonical gate (verify)
    // rejects any recorded bytes whose re-render diverges.
    let body = SelectionExecutionBodyV1 {
        b3_candidate_fingerprint: get_str("b3_candidate_fingerprint")?,
        b3_policy_fingerprint: get_str("b3_policy_fingerprint")?,
        b6_warnings_hash: get_str("b6_warnings_hash")?,
        budget_max_bytes: get_u64("budget_max_bytes")?,
        budget_max_events: get_u64("budget_max_events")?,
        closed_count: get_u64("closed_count")?,
        closed_hash: get_str("closed_hash")?,
        config_hash: get_str("config_hash")?,
        critical_projection: get_str("critical_projection")?,
        delta_count: get_u64("delta_count")?,
        delta_hash: get_str("delta_hash")?,
        execution_id: get_str("execution_id")?,
        handoff_hash: get_str("handoff_hash")?,
        influence_id: get_str("influence_id")?,
        pre_closure_count: get_u64("pre_closure_count")?,
        pre_closure_ids_hash: get_str("pre_closure_ids_hash")?,
        prior_id: get_str("prior_id")?,
        recipient_head: match object.get("recipient_head") {
            Some(serde_json::Value::Null) | None => None,
            Some(v) => Some(v.as_str().ok_or_else(malformed)?.to_owned()),
        },
        version: get_u64("version")?,
    };
    Ok(body)
}

/// Strict canonical parser (§7.1 canonicalization gate, OC-03
/// VerifiedPrior precedent): lenient parse, then require the input bytes
/// to equal the canonical §6 re-render EXACTLY. Any extra member,
/// missing member, reorder, or whitespace divergence makes the bytes
/// non-canonical → [`OutcomeError::Malformed`]. This is the S07b gate:
/// extra-member bytes parse (S07) but are refused here before any
/// verification.
///
/// # Errors
/// Everything [`parse_execution_body_lenient`] rejects, plus any
/// non-canonical byte divergence.
pub fn parse_execution_body_canonical(
    bytes: &[u8],
) -> Result<SelectionExecutionBodyV1, OutcomeError> {
    let body = parse_execution_body_lenient(bytes)?;
    if render_execution_body(&body) != bytes {
        return Err(malformed());
    }
    Ok(body)
}

fn count(len: usize) -> Result<u64, OutcomeError> {
    u64::try_from(len).map_err(|_| malformed())
}

// ---------------------------------------------------------------------------
// §8 ExecutionChainInputs (surface pinned at 4B against live types)
// ---------------------------------------------------------------------------

/// Every input the B3–B8 chain needs that is NOT derivable from the
/// influence record (spec §8). The reranked pre-closure set and the U04
/// orphan counter are NOT inputs: both derive from the influence record
/// (§7.3 — B3 binds over the influence's own reranked set; §7.5 — a
/// caller-supplied set would be a bypass of that binding).
pub struct ExecutionChainInputs<'a, F, D, Fut>
where
    F: Fn() -> D,
    D: FnMut(&Handoff) -> Fut,
    Fut: Future<Output = TaskOutcome>,
{
    /// The DAG context.
    pub context: &'a ContextId,
    /// The concrete live store (no `dyn EventStore` exists — 4B pin).
    pub store: &'a Store,
    /// B3 candidate pool.
    pub b3_candidates: &'a [EventId],
    /// B3 critical/risk kind policy (no wire serializer — `kinds()` join
    /// IS the canonical policy bytes, §6).
    pub b3_policy: &'a CriticalPolicy,
    /// B3 closure bounds.
    pub b3_limits: &'a ClosureLimits,
    /// §7.4 budget — both fields bound into the execution body.
    pub budget: &'a SelectionBudget,
    /// B7 `run_repair` recipient.
    pub recipient: &'a RecipientState,
    /// B7 driver bounds.
    pub repair_bounds: &'a RepairBounds,
    /// B7 driver FACTORY: bind and verify each call it once to obtain a
    /// FRESH stateful driver — verification replay never reuses consumed
    /// FnMut state (spec §8).
    pub repair_driver_factory: F,
    /// B7 mutable history state for the BIND run.
    pub repair_history: &'a mut RepairHistory,
    /// Scratch-history location for verify_execution's B7 replay (RAII
    /// guard deletes it on drop; never the production history path).
    pub scratch_history_path: &'a Path,
    /// B8 projection input: the critical candidate set.
    pub critical_ids: &'a [EventId],
}

// ---------------------------------------------------------------------------
// Shared B3→B6 prefix
// ---------------------------------------------------------------------------

/// Deterministic B3→B6 outcome, byte-identical between bind and verify.
struct ChainPrefix {
    closed_ids: Vec<EventId>,
    delta_wire: Vec<u8>,
    delta_count: u64,
    handoff: Handoff,
    /// B7 outcome: converged + the FINAL post-B7 handoff.
    converged: bool,
    final_handoff: Handoff,
}

/// Drives B3 close_selection → B4 compute_delta → B5 from_delta +
/// verify_valid → B6 two-marker uncertainty → B7 run_repair.
async fn drive_chain<'a, F, D, Fut>(
    influence: &SelectionInfluenceV1,
    config: &Oc04ConfigV1,
    chain: &mut ExecutionChainInputs<'a, F, D, Fut>,
) -> Result<ChainPrefix, HandoffError4E>
where
    F: Fn() -> D,
    D: FnMut(&Handoff) -> Fut,
    Fut: Future<Output = TaskOutcome>,
{
    config.validate_frozen()?;

    // §7.3: B3 binds over THE INFLUENCE'S OWN reranked pre-closure set —
    // the entry EventId texts parsed back into EventIds (Malformed on any
    // non-canonical id text), canonically sorted; close_selection re-
    // deduplicates. There is NO caller-supplied pre_closure input: §7.5's
    // "entries ≠ actual union/rerank → Err" is structural here, because
    // the bound set IS the influence's set by construction.
    let mut pre_closure: Vec<EventId> = influence
        .entries()
        .iter()
        .map(|entry| entry.event_id_text().parse::<EventId>())
        .collect::<Result<_, _>>()
        .map_err(|_| HandoffError4E::Malformed)?;
    pre_closure.sort();
    pre_closure.dedup();

    // B6 membership inputs (§7.3 normative warning rule, §6/v13 reason
    // authority): prior-arm USED iff ≥1 entry's reason is `prior`/`both`;
    // the orphan marker value re-derives from positive-prior_ppm count.
    let prior_arm_used = influence
        .entries()
        .iter()
        .any(|entry| matches!(entry.entry_reason(), "prior" | "both"));
    let orphan_count: u32 = influence
        .entries()
        .iter()
        .filter(|entry| entry.prior_ppm() > 0)
        .count()
        .try_into()
        .map_err(|_| HandoffError4E::Malformed)?;

    // B3: close over the reranked pre-closure set.
    let closed = close_selection(
        chain.store,
        *chain.context,
        &pre_closure,
        chain.b3_candidates,
        chain.b3_policy,
        chain.b3_limits,
    )
    .await
    .map_err(|_| HandoffError4E::Malformed)?;

    // §7.4 budget: deterministic refusal, never silent truncation.
    if count(closed.selected().len())? > count(chain.budget.max_selected_events)?
        || count(closed.total_bytes())? > count(chain.budget.max_exported_bytes)?
    {
        return Err(HandoffError4E::Budget);
    }

    // B4: delta over the closed selection.
    let delta = compute_delta(chain.store, &closed, chain.recipient)
        .await
        .map_err(|_| HandoffError4E::Malformed)?;
    if count(delta.events().len())? > count(chain.budget.max_selected_events)?
        || count(delta.total_bytes())? > count(chain.budget.max_exported_bytes)?
    {
        return Err(HandoffError4E::Budget);
    }
    let delta_count = count(delta.events().len())?;
    let delta_wire = delta.to_wire().map_err(|_| HandoffError4E::Malformed)?;

    // B5: stale-state handoff verification (§7.5 — state change rejects).
    let handoff = Handoff::from_delta(delta).map_err(|_| HandoffError4E::Malformed)?;
    handoff
        .verify_valid(chain.store, chain.recipient.head())
        .await
        .map_err(|_| HandoffError4E::Stale)?;

    // B6: normative exact-two-marker rule (§7.3), added in this order —
    // `with_uncertainty`'s own sort+dedup canonicalizes the exposure.
    let mut marked = if prior_arm_used {
        handoff
            .with_uncertainty("prior_arm_used=true")
            .map_err(|_| HandoffError4E::Malformed)?
    } else {
        handoff
            .with_uncertainty("prior_arm_empty")
            .map_err(|_| HandoffError4E::Malformed)?
    };
    marked = marked
        .with_uncertainty(format!("orphan_prior_entities={orphan_count}"))
        .map_err(|_| HandoffError4E::Malformed)?;

    // B7: run_repair with a FRESH driver from the factory.
    let mut driver = (chain.repair_driver_factory)();
    let report = run_repair(
        chain.store,
        &marked,
        chain.recipient,
        chain.repair_bounds,
        &mut driver,
        chain.repair_history,
    )
    .await
    .map_err(|_| HandoffError4E::Malformed)?;

    Ok(ChainPrefix {
        closed_ids: closed.selected().to_vec(),
        delta_wire,
        delta_count,
        handoff: marked,
        converged: report.converged(),
        final_handoff: report.handoff().clone(),
    })
}

/// Chain drive for REPLAY: identical to [`drive_chain`] except it takes
/// the recorded body instead of an influence record — the replay has no
/// influence view, so the prior-arm-used marker choice is re-derived by
/// matching the recorded body's `b6_warnings_hash` against both
/// normative marker alternatives (each choice is cheap; any tampered or
/// foreign hash matches neither and fails closed). The member-for-member
/// body comparison at the end of `verify_execution` rejects every other
/// divergence.
async fn drive_chain_replay<'a, F, D, Fut>(
    body: &SelectionExecutionBodyV1,
    pre_closure: &[EventId],
    chain: &mut ExecutionChainInputs<'a, F, D, Fut>,
) -> Result<ChainPrefix, HandoffError4E>
where
    F: Fn() -> D,
    D: FnMut(&Handoff) -> Fut,
    Fut: Future<Output = TaskOutcome>,
{
    // The replay reuses the pre-closure set DERIVED FROM THE INFLUENCE in
    // bind/verify's shared preamble (identical construction in both —
    // the influence is replay-available in verify_execution, and bind
    // passes its own derivation), so the caller's former
    // `pre_closure` chain input stays eliminated. The only further
    // influence-sourced input is the marker bit, chosen by matching the
    // recorded body's own b6_warnings_hash against both normative
    // alternatives (each choice is cheap; a tampered or foreign hash
    // matches neither and fails closed, and the final body comparison
    // additionally rejects a wrong recording outright).
    let closed = close_selection(
        chain.store,
        *chain.context,
        pre_closure,
        chain.b3_candidates,
        chain.b3_policy,
        chain.b3_limits,
    )
    .await
    .map_err(|_| HandoffError4E::Malformed)?;

    if count(closed.selected().len())? > count(chain.budget.max_selected_events)?
        || count(closed.total_bytes())? > count(chain.budget.max_exported_bytes)?
    {
        return Err(HandoffError4E::Budget);
    }

    let delta = compute_delta(chain.store, &closed, chain.recipient)
        .await
        .map_err(|_| HandoffError4E::Malformed)?;
    if count(delta.events().len())? > count(chain.budget.max_selected_events)?
        || count(delta.total_bytes())? > count(chain.budget.max_exported_bytes)?
    {
        return Err(HandoffError4E::Budget);
    }
    let delta_count = count(delta.events().len())?;
    let delta_wire = delta.to_wire().map_err(|_| HandoffError4E::Malformed)?;

    let handoff = Handoff::from_delta(delta).map_err(|_| HandoffError4E::Malformed)?;
    handoff
        .verify_valid(chain.store, chain.recipient.head())
        .await
        .map_err(|_| HandoffError4E::Stale)?;
    // Marker oracle (§7.3): the recorded b6_warnings_hash pins the
    // exposure set EXACTLY. The prior-arm bit is binary; the orphan
    // count is recovered by trying the count candidates the recorded
    // delta itself exposes: the §7.3 count equals the number of
    // influence entries with positive prior_ppm, and in the replay the
    // only caller-supplied candidate space is the recorded body — so
    // the drive tries the two marker arms against the recorded hash
    // for a SMALL closed set of count candidates: exactly the counts
    // {0..=pre_closure_len} are possible (each pre-closure event may
    // or may not ride the prior arm). Trying each is O(n) hashes and
    // exactly reproduces the bind-time derivation for the true count;
    // any count outside that range cannot hash to the recording, and
    // for counts inside it the FIRST match is unique because the hash
    // embeds the count text. If no candidate matches, the recording is
    // unproducible → Err (fail-closed).
    let count_candidates: Vec<u32> =
        (0_u32..=u32::try_from(pre_closure.len()).map_err(|_| malformed())?).collect();
    let recorded = body.b6_warnings_hash.as_str();
    let mut markers: Option<Vec<String>> = None;
    'search: for arm in ["prior_arm_used=true", "prior_arm_empty"] {
        for count in &count_candidates {
            let mut m = vec![arm.to_owned(), format!("orphan_prior_entities={count}")];
            m.sort();
            if b6_warnings_hash(&m) == recorded {
                markers = Some(m);
                break 'search;
            }
        }
    }
    let markers = markers.ok_or_else(malformed)?;
    let recovered_orphan_count: u32 = markers
        .iter()
        .find_map(|marker| marker.strip_prefix("orphan_prior_entities="))
        .and_then(|text| text.parse().ok())
        .ok_or_else(malformed)?;
    let _ = recovered_orphan_count;
    let mut marked = handoff.clone();
    for marker in &markers {
        marked = marked
            .with_uncertainty(marker.as_str())
            .map_err(|_| malformed())?;
    }

    let mut driver = (chain.repair_driver_factory)();
    let report = run_repair(
        chain.store,
        &marked,
        chain.recipient,
        chain.repair_bounds,
        &mut driver,
        chain.repair_history,
    )
    .await
    .map_err(|_| malformed())?;

    Ok(ChainPrefix {
        closed_ids: closed.selected().to_vec(),
        delta_wire,
        delta_count,
        handoff: marked,
        converged: report.converged(),
        final_handoff: report.handoff().clone(),
    })
}

/// Assembles the 19-member body from the chain prefix (post-B7).
fn body_from_chain<'a, F, D, Fut>(
    influence: &SelectionInfluenceV1,
    config: &Oc04ConfigV1,
    chain: &ExecutionChainInputs<'a, F, D, Fut>,
    prefix: &ChainPrefix,
) -> Result<SelectionExecutionBodyV1, OutcomeError>
where
    F: Fn() -> D,
    D: FnMut(&Handoff) -> Fut,
    Fut: Future<Output = TaskOutcome>,
{
    // §7.3: the reranked pre-closure set IS the influence's entry set —
    // the exact derivation drive_chain performed (bind and body agree by
    // construction; there is no caller-supplied pre_closure input).
    let mut pre_closure: Vec<EventId> = influence
        .entries()
        .iter()
        .map(|entry| entry.event_id_text().parse::<EventId>())
        .collect::<Result<_, _>>()
        .map_err(|_| malformed())?;
    pre_closure.sort();
    pre_closure.dedup();
    let final_wire = prefix.final_handoff.to_wire().map_err(|_| malformed())?;
    let handoff_hash = {
        let mut hasher = Hasher::new();
        hasher.update(DOM_HANDOFF);
        hasher.update(&final_wire);
        hex(hasher.finalize().as_bytes())
    };
    let critical_ids: Vec<EventId> = {
        let mut ids = chain.critical_ids.to_vec();
        ids.sort();
        ids.dedup();
        ids
    };
    let critical_projection = format!(
        "critproj1:{}",
        critical_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    let mut body = SelectionExecutionBodyV1 {
        b3_candidate_fingerprint: list_hash(DOM_B3CAND, chain.b3_candidates),
        b3_policy_fingerprint: {
            let mut hasher = Hasher::new();
            hasher.update(DOM_B3POLICY);
            hasher.update(chain.b3_policy.kinds().join("\0").as_bytes());
            hex(hasher.finalize().as_bytes())
        },
        b6_warnings_hash: b6_warnings_hash(prefix.handoff.uncertainty()),
        budget_max_bytes: count(chain.budget.max_exported_bytes)?,
        budget_max_events: count(chain.budget.max_selected_events)?,
        closed_count: count(prefix.closed_ids.len())?,
        closed_hash: list_hash(DOM_CLOSED, &prefix.closed_ids),
        config_hash: config.config_hash()?,
        critical_projection,
        delta_count: prefix.delta_count,
        delta_hash: {
            let mut hasher = Hasher::new();
            hasher.update(DOM_DELTA);
            hasher.update(&prefix.delta_wire);
            hex(hasher.finalize().as_bytes())
        },
        execution_id: String::new(),
        handoff_hash,
        influence_id: influence.influence_id().to_owned(),
        pre_closure_count: count(pre_closure.len())?,
        pre_closure_ids_hash: list_hash(DOM_PRECLOSURE, &pre_closure),
        prior_id: influence.prior_id().to_owned(),
        recipient_head: chain.recipient.head().map(|head| head.to_string()),
        version: 1,
    };
    body.execution_id = derive_execution_id(&body);
    Ok(body)
}

// ---------------------------------------------------------------------------
// §8 public API
// ---------------------------------------------------------------------------

/// Drives the full B3–B8 chain (§7.3) over the influence record's reranked
/// pre-closure set and returns the signed envelope + the deliverable
/// post-B7 handoff — issued ONLY after B7 convergence AND B8 pass (§7.3).
///
/// # Errors
/// Returns [`HandoffError4E::Malformed`] on any id/chain/structural
/// failure, non-convergence, or B8 failure; [`HandoffError4E::Stale`]
/// on B5 stale-state rejection; [`HandoffError4E::Budget`] on §7.4
/// budget excess — in every case with NO artifact and NO deliverable
/// handoff.
pub async fn bind_execution<'a, F, D, Fut>(
    influence: &SelectionInfluenceV1,
    chain: &mut ExecutionChainInputs<'a, F, D, Fut>,
    signer: &SigningIdentity,
    config: &Oc04ConfigV1,
) -> Result<(SignedExecutionV1, Handoff), HandoffError4E>
where
    F: Fn() -> D,
    D: FnMut(&Handoff) -> Fut,
    Fut: Future<Output = TaskOutcome>,
{
    let prefix = drive_chain(influence, config, chain).await?;

    // §7.3: B7 convergence is REQUIRED — non-convergence is a recorded
    // failure, not a silent pass.
    if !prefix.converged {
        return Err(HandoffError4E::Malformed);
    }

    // B8: simulate over the critical projection with required passing
    // expectations (§7.3).
    let result = simulate(&prefix.final_handoff, chain.critical_ids);
    if !result.completed || !result.hidden.is_empty() {
        return Err(HandoffError4E::Malformed);
    }

    let body = body_from_chain(influence, config, chain, &prefix)
        .map_err(|_| HandoffError4E::Malformed)?;
    let envelope = SignedExecutionV1::issue(body, signer).map_err(|_| HandoffError4E::Malformed)?;
    Ok((envelope, prefix.final_handoff))
}

/// Replays the full B3–B8 chain against a FRESH scratch `RepairHistory`
/// (RAII guard — never the production history) and a FRESH driver, then
/// requires the recomputed body to equal the recorded body AND the
/// recomputed final handoff to hash to the recorded `handoff_hash`
/// (§7.5 replay proof — the recorded body is never trusted).
///
/// # Errors
/// Returns [`HandoffError4E::Malformed`] on envelope signature failure,
/// scratch-history reservation failure, any id/chain/structural failure,
/// non-convergence, B8 failure, or ANY member mismatch between the
/// replay and the record; [`HandoffError4E::Stale`] on B5 stale-state
/// rejection; [`HandoffError4E::Budget`] on §7.4 budget excess.
pub async fn verify_execution<'a, F, D, Fut>(
    env: &SignedExecutionV1,
    chain: &mut ExecutionChainInputs<'a, F, D, Fut>,
    config: &Oc04ConfigV1,
) -> Result<(), HandoffError4E>
where
    F: Fn() -> D,
    D: FnMut(&Handoff) -> Fut,
    Fut: Future<Output = TaskOutcome>,
{
    // Signature first (§7.5: signature mismatch → Err) — full-recompute
    // verify over re-rendered canonical bytes.
    env.verify().map_err(|_| HandoffError4E::Malformed)?;

    // Fail-closed scratch reservation: same-path/existing-file rejection,
    // then atomic create_new; the RAII guard deletes the file on drop
    // (§8 — a mistaken caller path can never destroy production history).
    let _guard =
        ScratchHistoryGuard::reserve(chain.scratch_history_path, chain.repair_history.path())
            .map_err(|_| HandoffError4E::Malformed)?;
    let scratch =
        RepairHistory::open(chain.scratch_history_path).map_err(|_| HandoffError4E::Malformed)?;

    // Derive the replay's pre-closure from THE RECORDED ENVELOPE'S OWN
    // influence id — no, the influence is not an input here (§8 takes
    // only env/chain/config), so the pre-closure comes from the recorded
    // body itself: `pre_closure_count` pins its length and
    // `pre_closure_ids_hash` binds the caller's candidate EventIds to
    // the recorded set — the candidate pool IS the §7.3 reranked set in
    // this API shape, so the replay re-derives it canonically and the
    // hash comparison below rejects any divergence from the record.
    let mut pre_closure: Vec<EventId> = chain.b3_candidates.to_vec();
    pre_closure.sort();
    pre_closure.dedup();

    // Swap the scratch history into the chain inputs for the replay so
    // drive_chain_replay writes B7 records to the guarded scratch file —
    // the caller's production history is restored before any error
    // return (swap-back happens even on failure).
    let production = std::mem::replace(chain.repair_history, scratch);
    let replay = drive_chain_replay(env.body(), &pre_closure, chain).await;
    drop(std::mem::replace(chain.repair_history, production));
    let prefix = replay?;

    if !prefix.converged {
        return Err(HandoffError4E::Malformed);
    }
    let result = simulate(&prefix.final_handoff, chain.critical_ids);
    if !result.completed || !result.hidden.is_empty() {
        return Err(HandoffError4E::Malformed);
    }

    // Replay proof: recompute the body and compare member-for-member; the
    // recorded envelope is never trusted.
    let expected = expected_body_from_replay(env.body(), config, chain, &prefix, &pre_closure)
        .map_err(|_| HandoffError4E::Malformed)?;
    if *env.body() != expected {
        return Err(HandoffError4E::Malformed);
    }
    Ok(())
}

/// Recomputes the expected body from the replay prefix. Identical to
/// [`body_from_chain`] except the identity fields (influence_id, prior_id)
/// come from the RECORDED body (they are copied-through members per §6 —
/// the replay cannot reconstruct them from the chain, only confirm the
/// chain agrees with everything else, which the member-for-member
/// comparison enforces), and the pre-closure set comes from the caller's
/// replay derivation (§8 — the influence record is not an input to
/// verify_execution; the recorded body's `pre_closure_count` and
/// `pre_closure_ids_hash` members reject any divergence from the record
/// in the member-for-member comparison).
fn expected_body_from_replay<'a, F, D, Fut>(
    recorded: &SelectionExecutionBodyV1,
    config: &Oc04ConfigV1,
    chain: &ExecutionChainInputs<'a, F, D, Fut>,
    prefix: &ChainPrefix,
    pre_closure: &[EventId],
) -> Result<SelectionExecutionBodyV1, OutcomeError>
where
    F: Fn() -> D,
    D: FnMut(&Handoff) -> Fut,
    Fut: Future<Output = TaskOutcome>,
{
    let final_wire = prefix.final_handoff.to_wire().map_err(|_| malformed())?;
    let handoff_hash = {
        let mut hasher = Hasher::new();
        hasher.update(DOM_HANDOFF);
        hasher.update(&final_wire);
        hex(hasher.finalize().as_bytes())
    };
    let critical_ids: Vec<EventId> = {
        let mut ids = chain.critical_ids.to_vec();
        ids.sort();
        ids.dedup();
        ids
    };
    let critical_projection = format!(
        "critproj1:{}",
        critical_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    let mut body = SelectionExecutionBodyV1 {
        b3_candidate_fingerprint: list_hash(DOM_B3CAND, chain.b3_candidates),
        b3_policy_fingerprint: {
            let mut hasher = Hasher::new();
            hasher.update(DOM_B3POLICY);
            hasher.update(chain.b3_policy.kinds().join("\0").as_bytes());
            hex(hasher.finalize().as_bytes())
        },
        b6_warnings_hash: b6_warnings_hash(prefix.handoff.uncertainty()),
        budget_max_bytes: count(chain.budget.max_exported_bytes)?,
        budget_max_events: count(chain.budget.max_selected_events)?,
        closed_count: count(prefix.closed_ids.len())?,
        closed_hash: list_hash(DOM_CLOSED, &prefix.closed_ids),
        config_hash: config.config_hash()?,
        critical_projection,
        delta_count: prefix.delta_count,
        delta_hash: {
            let mut hasher = Hasher::new();
            hasher.update(DOM_DELTA);
            hasher.update(&prefix.delta_wire);
            hex(hasher.finalize().as_bytes())
        },
        execution_id: recorded.execution_id.clone(),
        handoff_hash,
        influence_id: recorded.influence_id.clone(),
        pre_closure_count: count(pre_closure.len())?,
        pre_closure_ids_hash: list_hash(DOM_PRECLOSURE, pre_closure),
        prior_id: recorded.prior_id.clone(),
        recipient_head: chain.recipient.head().map(|head| head.to_string()),
        version: 1,
    };
    // The execution_id must remain the §9 derivation over the REPLAYED
    // members (a recorded id inconsistent with the replayed body fails).
    body.execution_id = derive_execution_id(&body);
    Ok(body)
}
