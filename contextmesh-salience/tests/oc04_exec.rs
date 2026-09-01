//! OC-04 Stage 4E execution matrix (spec §7.3/§7.4/§7.5, §8): E-family
//! rows except E07 (delivered at 4F) plus the 4B-deferred S07b
//! canonicalization gate.
//!
//! Test names match the frozen traceability matrix exactly
//! (oc-04-test-traceability-matrix.md): every E-row re-derives the bound
//! body member independently over the §6 domain constants and compares
//! against the recorded envelope — replay proof, never trust.

use blake3::Hasher;
use contextmesh::closure::{ClosureLimits, CriticalPolicy, close_selection};
use contextmesh::crypto::SigningIdentity;
use contextmesh::delta::{RecipientState, compute_delta};
use contextmesh::handoff::Handoff;
use contextmesh::model::{ContextId, EventId};
use contextmesh::repair::{RepairBounds, RepairHistory, TaskOutcome};
use contextmesh::selection::SelectionBudget;
use contextmesh::store::{RefExpectation, RefMutation, Store};
use contextmesh_salience::oc04_exec::{ExecutionChainInputs, bind_execution, verify_execution};
use contextmesh_salience::oc04_exec::{
    parse_execution_body_canonical, parse_execution_body_lenient,
};
use contextmesh_salience::oc04_selection::{
    ENTRY_REASON_BOTH, ENTRY_REASON_LEXICAL, Oc04ConfigV1, SelectionInfluenceEntryV1,
    SelectionInfluenceV1, SignedExecutionV1, derive_execution_id, render_execution_body,
};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

mod support {
    //! Minimal local fixture helpers (no shared common module in this
    //! crate's test layout; mirrors tests/common/mod.rs of the root
    //! crate with the subset oc04_exec needs).

    use contextmesh::crypto::SigningIdentity;
    use contextmesh::model::{AuthorId, ContextId, SignedEventV1};
    use contextmesh::store::{ContextProvision, RefExpectation, RefMutation, Store};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    pub fn path(label: &str) -> PathBuf {
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        let file = std::env::temp_dir().join(format!(
            "contextmesh-{label}-{}-{serial}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&file);
        file
    }

    pub fn identity(seed: u8) -> SigningIdentity {
        SigningIdentity::from_fixture_seed([seed; 32])
    }

    pub fn context(byte: u8) -> ContextId {
        ContextId::from_bytes([byte; 32])
    }

    pub async fn provision(store: &Store, event: &SignedEventV1, authors: Vec<AuthorId>) {
        let mut sorted = authors;
        sorted.sort_by_key(ToString::to_string);
        store
            .provision_context(ContextProvision {
                context: event.body().context(),
                expected_genesis: event.event_id(),
                authorized_authors: sorted,
            })
            .await
            .unwrap();
    }

    pub fn main_cas(context: ContextId, expected: RefExpectation, head: EventId) -> RefMutation {
        RefMutation::CompareAndSwap {
            context,
            name: "main".parse().unwrap(),
            expected,
            new_head: head,
        }
    }

    use contextmesh::model::EventId;
}
use support::{context, identity, main_cas, path, provision};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn history_path(label: &str) -> PathBuf {
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    let file = std::env::temp_dir().join(format!(
        "contextmesh-oc04e-{label}-{}-{serial}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&file);
    file
}

const LIMITS: ClosureLimits = ClosureLimits {
    max_events: 100_000,
    max_exported_bytes: 64 * 1024 * 1024,
};

fn critical_policy() -> CriticalPolicy {
    CriticalPolicy::new(vec!["context.critical".to_owned()]).unwrap()
}

fn budget() -> SelectionBudget {
    SelectionBudget {
        max_selected_events: 100_000,
        max_exported_bytes: 64 * 1024 * 1024,
    }
}

fn bounds() -> RepairBounds {
    RepairBounds {
        max_iterations: 8,
        max_re_included_events: 8,
        max_delta_bytes: 64 * 1024 * 1024,
    }
}

fn config() -> Oc04ConfigV1 {
    Oc04ConfigV1::default()
}

/// Deterministic linear chain (genesis + `depth` children), last child is
/// main head. Mirrors the OB-07 fixture.
async fn chain_store(depth: usize) -> (Store, SigningIdentity, ContextId, Vec<EventId>) {
    let db = path("oc04e-chain");
    let store = Store::open(&db).await.unwrap();
    let author = identity(7);
    let ctx = context(8);
    let genesis_event = author
        .create_event(ctx, vec![], "context.genesis", json!({"note": "root"}))
        .unwrap();
    provision(&store, &genesis_event, vec![author.author()]).await;
    store
        .admit(&genesis_event, RefMutation::None)
        .await
        .unwrap();
    let mut ids = vec![genesis_event.event_id()];
    let mut head = genesis_event.event_id();
    for step in 1..=depth {
        let event = author
            .create_event(
                ctx,
                vec![head],
                "agent.request",
                json!({"value": step, "note": format!("step {step}")}),
            )
            .unwrap();
        let expected = if step == 1 {
            RefExpectation::Absent
        } else {
            RefExpectation::Head(head)
        };
        store
            .admit(&event, main_cas(ctx, expected, event.event_id()))
            .await
            .unwrap();
        ids.push(event.event_id());
        head = event.event_id();
    }
    (store, author, ctx, ids)
}

/// Always-success driver factory + driver, as concrete fn pointers
/// (function items implement Fn/FnMut concretely — no nested impl Trait).
type Driver = fn(&Handoff) -> std::future::Ready<TaskOutcome>;
type DriverFactory = fn() -> Driver;

fn success_driver(_current: &Handoff) -> std::future::Ready<TaskOutcome> {
    std::future::ready(TaskOutcome::Success)
}

fn success_factory() -> Driver {
    success_driver
}

/// A Rig plus its derived shared inputs that the borrow checker needs to
/// keep alive together: constructs the canonical-policy/budget/limits/
/// recipient ONCE so the returned chain inputs borrow from `self`, not
/// from temporaries.
struct Rig {
    store: Store,
    context: ContextId,
    ids: Vec<EventId>,
    signer: SigningIdentity,
    policy: CriticalPolicy,
    limits: ClosureLimits,
    budget: SelectionBudget,
    recipient: RecipientState,
    bounds: RepairBounds,
}

async fn rig(depth: usize) -> Rig {
    let (store, signer, context, ids) = chain_store(depth).await;
    Rig {
        policy: critical_policy(),
        limits: LIMITS,
        budget: budget(),
        recipient: RecipientState::cold_start(context),
        bounds: bounds(),
        store,
        context,
        ids,
        signer,
    }
}

fn chain_inputs<'a>(
    r: &'a Rig,
    scratch: &'a PathBuf,
    history: &'a mut RepairHistory,
) -> ExecutionChainInputs<'a, DriverFactory, Driver, std::future::Ready<TaskOutcome>> {
    ExecutionChainInputs {
        context: &r.context,
        store: &r.store,
        b3_candidates: &r.ids,
        b3_policy: &r.policy,
        b3_limits: &r.limits,
        budget: &r.budget,
        recipient: &r.recipient,
        repair_bounds: &r.bounds,
        repair_driver_factory: success_factory,
        repair_history: history,
        scratch_history_path: scratch,
        critical_ids: &r.ids,
    }
}

// ---------------------------------------------------------------------------
// §6 independent re-derivation helpers (test-local; the production
// fingerprint functions are private — these mirror the frozen §6 table
// verbatim, following the oc04_schema.rs hardcoded-domain precedent).
// ---------------------------------------------------------------------------

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// §6 list fingerprint: hex(BLAKE3(domain + canonical-text-ascending
/// deduplicated ids, comma-joined)).
fn list_fp(domain: &[u8], ids: &[EventId]) -> String {
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
    hex_lower(hasher.finalize().as_bytes())
}

/// §6 policy fingerprint: hex(BLAKE3(`oc-04-b3policy-v1\0` + kinds joined
/// by NUL in the accessor's canonical order)).
fn policy_fp(kinds: &[String]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(b"oc-04-b3policy-v1\0");
    hasher.update(kinds.join("\0").as_bytes());
    hex_lower(hasher.finalize().as_bytes())
}

/// §6 marker fingerprint: hex(BLAKE3(`oc-04-b6warn-v1\0` + each marker
/// NUL-terminated in the handoff's exposed order)).
fn markers_fp(markers: &[String]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(b"oc-04-b6warn-v1\0");
    for marker in markers {
        hasher.update(marker.as_bytes());
        hasher.update(b"\0");
    }
    hex_lower(hasher.finalize().as_bytes())
}

// ---------------------------------------------------------------------------
// E01: execution_binds_preclosure — pre-closure hash/count recomputed over
// the actual set
// ---------------------------------------------------------------------------

#[tokio::test]
async fn execution_binds_preclosure() {
    let r = rig(3).await;
    let bind_history = history_path("e01-bind");
    let mut history = RepairHistory::open(&bind_history).unwrap();
    let scratch = history_path("e01-scratch");
    let mut chain = chain_inputs(&r, &scratch, &mut history);
    let influence = test_influence(&config(), &r.ids);

    let (env, handoff) = bind_execution(&influence, &mut chain, &r.signer, &config())
        .await
        .expect("bind must succeed on a converged chain");

    // §6: pre_closure_ids_hash over the actual pre-closure set (sorted,
    // deduplicated, comma-joined) and the u64 decimal count.
    assert_eq!(
        env.body().pre_closure_ids_hash,
        list_fp(b"oc-04-preclosure-v1\0", &r.ids),
        "E01: pre-closure hash must be the §6 re-derivation over r.ids"
    );
    assert_eq!(
        env.body().pre_closure_count,
        u64::try_from(r.ids.len()).unwrap(),
        "E01: pre-closure count must equal the actual set length"
    );

    // Mutation-negative: a DIFFERENT pre-closure set must NOT re-derive
    // the recorded hash (binding is content, not shape).
    assert_ne!(
        env.body().pre_closure_ids_hash,
        list_fp(b"oc-04-preclosure-v1\0", &r.ids[..r.ids.len() - 1]),
        "E01: a shorter pre-closure must produce a different hash"
    );
    // Mutation-negative (count): the count must be the actual set
    // length, not any other plausible value (mutation-decisive on the
    // count member independently of the hash).
    assert_ne!(
        env.body().pre_closure_count,
        u64::try_from(r.ids.len() - 1).unwrap(),
        "E01: a truncated set length must not match the recorded count"
    );
    assert_ne!(
        env.body().pre_closure_count,
        0,
        "E01: the empty-set count must not match the recorded count"
    );

    // The deliverable handoff carries exactly the two §7.3 markers
    // (empty-arm fixture): sorted exposure, orphan marker first.
    let markers = handoff.uncertainty();
    assert_eq!(markers.len(), 2, "§7.3: exactly two markers");
    assert_eq!(markers[0], "orphan_prior_entities=0");
    assert_eq!(markers[1], "prior_arm_empty");

    // Verify: fresh scratch reservation + replay → Ok.
    let mut history2 = RepairHistory::open(&bind_history).unwrap();
    let scratch2 = history_path("e01-scratch2");
    let mut chain2 = chain_inputs(&r, &scratch2, &mut history2);
    verify_execution(&env, &mut chain2, &config())
        .await
        .expect("verify must replay Ok on identical inputs");
}

// ---------------------------------------------------------------------------
// E02: execution_binds_b3 — B3 policy + candidate fingerprints recomputed
// from the chain output
// ---------------------------------------------------------------------------

#[tokio::test]
async fn execution_binds_b3() {
    let r = rig(3).await;
    let bind_history = history_path("e02-bind");
    let mut history = RepairHistory::open(&bind_history).unwrap();
    let scratch = history_path("e02-scratch");
    let influence = test_influence(&config(), &r.ids);

    let mut chain = chain_inputs(&r, &scratch, &mut history);
    let (env, _handoff) = bind_execution(&influence, &mut chain, &r.signer, &config())
        .await
        .unwrap();

    // §6: both B3 fingerprints recomputed from the SAME chain inputs B3
    // actually consumed — candidate list (B3's own sorted-dedup canonical
    // order) and the policy kinds accessor.
    assert_eq!(
        env.body().b3_candidate_fingerprint,
        list_fp(b"oc-04-b3cand-v1\0", &r.ids),
        "E02: candidate fingerprint must bind the actual B3 candidate set"
    );
    assert_eq!(
        env.body().b3_policy_fingerprint,
        policy_fp(r.policy.kinds()),
        "E02: policy fingerprint must bind the actual B3 policy kinds"
    );

    // Mutation decisiveness: a different candidate set / policy MUST NOT
    // re-derive the recorded fingerprints (the binding is content, not
    // shape).
    let other = list_fp(b"oc-04-b3cand-v1\0", &r.ids[..r.ids.len() - 1]);
    assert_ne!(env.body().b3_candidate_fingerprint, other);
    let other_policy = policy_fp(&["context.other".to_owned()]);
    assert_ne!(env.body().b3_policy_fingerprint, other_policy);
}

// ---------------------------------------------------------------------------
// E03: execution_binds_delta — delta hash + count recomputed; both match
// B4 output
// ---------------------------------------------------------------------------

#[tokio::test]
async fn execution_binds_delta() {
    let r = rig(3).await;
    let bind_history = history_path("e03-bind");
    let mut history = RepairHistory::open(&bind_history).unwrap();
    let scratch = history_path("e03-scratch");
    let influence = test_influence(&config(), &r.ids);

    let mut chain = chain_inputs(&r, &scratch, &mut history);
    let (env, _handoff) = bind_execution(&influence, &mut chain, &r.signer, &config())
        .await
        .unwrap();

    // Independently re-run B3 (same inputs) to obtain the closed set, then
    // B4 over the same cold-start recipient: the recorded delta members
    // must equal this recomputation.
    let closed = close_selection(&r.store, r.context, &r.ids, &r.ids, &r.policy, &r.limits)
        .await
        .expect("recompute B3 closed set");
    let delta = compute_delta(&r.store, &closed, &r.recipient)
        .await
        .expect("recompute B4 delta");
    let wire = delta.to_wire().expect("delta wire");
    let mut hasher = Hasher::new();
    hasher.update(b"oc-04-delta-v1\0");
    hasher.update(&wire);
    assert_eq!(
        env.body().delta_hash,
        hex_lower(hasher.finalize().as_bytes()),
        "E03: delta hash must bind the B4 wire output"
    );
    assert_eq!(
        env.body().delta_count,
        u64::try_from(delta.events().len()).unwrap(),
        "E03: delta count must equal the B4 output length"
    );

    // Mutation-negative: a delta over a DIFFERENT recipient (at genesis —
    // genesis already delivered there, so its wire bytes differ) must NOT
    // hash to the recorded delta_hash.
    let at_genesis = RecipientState::at_head(&r.store, r.context, r.ids[0], &r.limits)
        .await
        .expect("recipient at genesis");
    let other_delta = compute_delta(&r.store, &closed, &at_genesis)
        .await
        .expect("genesis-recipient delta");
    let other_wire = other_delta.to_wire().expect("other delta wire");
    let mut other_hasher = Hasher::new();
    other_hasher.update(b"oc-04-delta-v1\0");
    other_hasher.update(&other_wire);
    assert_ne!(
        env.body().delta_hash,
        hex_lower(other_hasher.finalize().as_bytes()),
        "E03: a different recipient's delta must produce a different hash"
    );
    assert_ne!(
        env.body().delta_count,
        u64::try_from(other_delta.events().len()).unwrap(),
        "E03: delta count must differ for the different recipient"
    );
}

// ---------------------------------------------------------------------------
// E04: execution_binds_recipient_head — the body member equals the
// B5-verified head; a replay against a drifted recipient head → Err
// ---------------------------------------------------------------------------

#[tokio::test]
async fn execution_binds_recipient_head() {
    let r = rig(3).await;

    // B5-verified recipient at the GENESIS event: a real (non-null) head
    // while the delta stays non-empty. The critical set EXCLUDES the
    // genesis event (the recipient already holds it — an event the
    // recipient has never enters the delta and would be counted hidden,
    // the E10 refusal shape). All criticals (the children) are delivered
    // by the delta → B8 passes.
    let genesis = r.ids[0];
    let critical_children: Vec<EventId> = r.ids[1..].to_vec();
    let at_genesis = RecipientState::at_head(&r.store, r.context, genesis, &r.limits)
        .await
        .expect("recipient at genesis");
    let bind_history = history_path("e04-bind");
    let mut history = RepairHistory::open(&bind_history).unwrap();
    let scratch = history_path("e04-scratch");
    let influence = test_influence(&config(), &r.ids);
    let mut chain = ExecutionChainInputs {
        recipient: &at_genesis,
        critical_ids: &critical_children,
        ..chain_inputs(&r, &scratch, &mut history)
    };
    let (env, _handoff) = bind_execution(&influence, &mut chain, &r.signer, &config())
        .await
        .unwrap();
    assert_eq!(
        env.body().recipient_head.as_deref(),
        Some(genesis.to_string()).as_deref(),
        "E04: recipient_head member must equal the B5-verified head text"
    );

    // Isolated member comparison (§7.5): bind the SAME everything EXCEPT
    // the recipient on a twin chain — with the critical set tightened the
    // same way — then diff the two bodies member by member. The intended
    // isolation is recipient_head alone; any additional differing members
    // are exactly the §6-derived members the recipient causally feeds
    // (delta, projection), which the assertion names explicitly.
    let cold_rig = rig(3).await;
    let cold_critical: Vec<EventId> = cold_rig.ids[1..].to_vec();
    let bind_history2 = history_path("e04-bind2");
    let mut history2 = RepairHistory::open(&bind_history2).unwrap();
    let scratch2 = history_path("e04-scratch2");
    let mut chain2 = ExecutionChainInputs {
        recipient: &cold_rig.recipient,
        critical_ids: &cold_critical,
        ..chain_inputs(&cold_rig, &scratch2, &mut history2)
    };
    let (cold_env, _cold_handoff) =
        bind_execution(&influence, &mut chain2, &cold_rig.signer, &config())
            .await
            .unwrap();
    assert!(cold_env.body().recipient_head.is_none());

    // Exactly one member differs, and it is recipient_head.
    let a = env.body();
    let b = cold_env.body();
    let differing: Vec<&str> = [
        (
            "b3_candidate_fingerprint",
            a.b3_candidate_fingerprint == b.b3_candidate_fingerprint,
        ),
        (
            "b3_policy_fingerprint",
            a.b3_policy_fingerprint == b.b3_policy_fingerprint,
        ),
        ("b6_warnings_hash", a.b6_warnings_hash == b.b6_warnings_hash),
        ("budget_max_bytes", a.budget_max_bytes == b.budget_max_bytes),
        (
            "budget_max_events",
            a.budget_max_events == b.budget_max_events,
        ),
        ("closed_count", a.closed_count == b.closed_count),
        ("closed_hash", a.closed_hash == b.closed_hash),
        ("config_hash", a.config_hash == b.config_hash),
        (
            "critical_projection",
            a.critical_projection == b.critical_projection,
        ),
        ("delta_count", a.delta_count == b.delta_count),
        ("delta_hash", a.delta_hash == b.delta_hash),
        ("execution_id", a.execution_id == b.execution_id),
        ("handoff_hash", a.handoff_hash == b.handoff_hash),
        ("influence_id", a.influence_id == b.influence_id),
        (
            "pre_closure_count",
            a.pre_closure_count == b.pre_closure_count,
        ),
        (
            "pre_closure_ids_hash",
            a.pre_closure_ids_hash == b.pre_closure_ids_hash,
        ),
        ("prior_id", a.prior_id == b.prior_id),
        ("recipient_head", a.recipient_head == b.recipient_head),
    ]
    .iter()
    .filter(|(_, equal)| !*equal)
    .map(|(name, _)| *name)
    .collect();
    // The recipient causally feeds: its dedicated member (B5 head), the
    // delta it determines (B4), and transitively the derived
    // ids/hashes. NO other member may diverge — the influence identity,
    // pre-closure binding, policy binding, and critical projection
    // (identical critical set here) are unaffected.
    assert_eq!(
        differing,
        vec![
            "delta_count",
            "delta_hash",
            "execution_id",
            "handoff_hash",
            "recipient_head",
        ],
        "E04: recipient mutation must move ONLY recipient-caused members"
    );

    // End-to-end §7.5: verifying the genesis-bound envelope against the
    // COLD chain inputs re-derives recipient_head = null ≠ recorded
    // Some(genesis) → Err (the replay proof consumes the same isolation).
    let mut history3 = RepairHistory::open(&bind_history2).unwrap();
    let scratch3 = history_path("e04-scratch3");
    let mut chain3 = chain_inputs(&cold_rig, &scratch3, &mut history3);
    assert!(
        verify_execution(&env, &mut chain3, &config())
            .await
            .is_err(),
        "§7.5: a drifted recipient head must fail the replay"
    );
}

// ---------------------------------------------------------------------------
// E04b: execution_binds_b6_warnings — b6_warnings_hash recomputed over the
// Handoff::uncertainty() exposure per the §6 derivation table
// ---------------------------------------------------------------------------

#[tokio::test]
async fn execution_binds_b6_warnings() {
    let r = rig(2).await;
    let bind_history = history_path("e04b-bind");
    let mut history = RepairHistory::open(&bind_history).unwrap();
    let scratch = history_path("e04b-scratch");
    let influence = test_influence(&config(), &r.ids);

    let mut chain = chain_inputs(&r, &scratch, &mut history);
    let (env, handoff) = bind_execution(&influence, &mut chain, &r.signer, &config())
        .await
        .unwrap();

    assert_eq!(
        env.body().b6_warnings_hash,
        markers_fp(handoff.uncertainty()),
        "E04b: b6_warnings_hash must be the §6 re-derivation over the \
         exposed marker list"
    );
    // Decisiveness: a different marker set must not collide.
    assert_ne!(
        env.body().b6_warnings_hash,
        markers_fp(&[
            "prior_arm_used=true".to_owned(),
            "orphan_prior_entities=0".to_owned()
        ])
    );
}

// ---------------------------------------------------------------------------
// E04c: b6_marker_rule_exact — normative exact-two-marker rule, both
// alternatives (prior-arm-used / prior-arm-empty)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn b6_marker_rule_exact() {
    // Empty-arm fixture: markers exactly {prior_arm_empty, orphan=n}.
    let r = rig(2).await;
    let bind_history = history_path("e04c-bind");
    let mut history = RepairHistory::open(&bind_history).unwrap();
    let scratch = history_path("e04c-scratch");
    let influence = test_influence(&config(), &r.ids);
    let mut chain = chain_inputs(&r, &scratch, &mut history);
    let (_env, handoff) = bind_execution(&influence, &mut chain, &r.signer, &config())
        .await
        .unwrap();
    // Lexical sort: "orphan_prior_entities=0" < "prior_arm_empty". The
    // orphan count re-derives from the influence (lexical-only entries
    // → 0 positive prior_ppm).
    assert_eq!(
        handoff.uncertainty(),
        &[
            "orphan_prior_entities=0".to_owned(),
            "prior_arm_empty".to_owned()
        ],
        "E04c: empty-arm fixture — exactly two markers, orphan re-derived"
    );

    // Prior-arm fixture: ≥1 influence entry with prior_ppm > 0 →
    // {orphan=n, prior_arm_used=true} exactly. The influence still names
    // the FULL pre-closure (§7.5) — one entry rides the prior arm
    // (reason `both`, prior_ppm > 0), the rest stay lexical.
    let bind_history2 = history_path("e04c-bind2");
    let mut history2 = RepairHistory::open(&bind_history2).unwrap();
    let scratch2 = history_path("e04c-scratch2");
    let mut prior_entries: Vec<SelectionInfluenceEntryV1> = r
        .ids
        .iter()
        .map(|id| {
            SelectionInfluenceEntryV1::new(id.to_string(), ENTRY_REASON_LEXICAL, 1, 0)
                .expect("coherent lexical entry")
        })
        .collect();
    prior_entries[0] =
        SelectionInfluenceEntryV1::new(r.ids[0].to_string(), ENTRY_REASON_BOTH, 1, 42)
            .expect("coherent both-arm entry");
    prior_entries.sort_by(|a, b| {
        b.score_ppm()
            .cmp(&a.score_ppm())
            .then_with(|| a.event_id_text().cmp(b.event_id_text()))
    });
    let prior_influence = SelectionInfluenceV1::assemble(
        &config(),
        "ocprior1_0000000000000000000000000000000000000000000000000000000000000000",
        "task-fingerprint-fixtures",
        prior_entries,
    )
    .expect("assemble prior-arm influence");
    let mut chain2 = chain_inputs(&r, &scratch2, &mut history2);
    let (_env2, handoff2) = bind_execution(&prior_influence, &mut chain2, &r.signer, &config())
        .await
        .unwrap();
    assert_eq!(
        handoff2.uncertainty(),
        &[
            "orphan_prior_entities=1".to_owned(),
            "prior_arm_used=true".to_owned()
        ],
        "E04c: prior-arm fixture — exactly two markers; the orphan count
         re-derives as the count of entries with positive prior_ppm (1:
         the both-arm entry's prior_ppm=42; the lexical entries carry 0)"
    );
}

// ---------------------------------------------------------------------------
// E05: influence_mismatch_rejected — the execution artifact is BOUND to
// the influence record: a different (tampered) influence cannot produce
// the same artifact. NOTE: §7.5's "entries ≠ actual union/rerank → Err"
// is unconstructible on the frozen surface (SelectionInfluenceV1 has no
// public constructor other than `assemble`, which enforces entry
// coherence, and verify_execution takes no influence parameter) — this
// test asserts the reachable consequence: the influence_id member and the
// derived execution_id are functions of the influence record.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn influence_mismatch_rejected() {
    let r = rig(2).await;
    let influence_a = test_influence(&config(), &r.ids);
    let bind_history_a = history_path("e05-bind-a");
    let mut history_a = RepairHistory::open(&bind_history_a).unwrap();
    let scratch_a = history_path("e05-scratch-a");
    let mut chain_a = chain_inputs(&r, &scratch_a, &mut history_a);
    let (env_a, _h) = bind_execution(&influence_a, &mut chain_a, &r.signer, &config())
        .await
        .unwrap();

    // Bind B over a chain whose critical projection matches B's
    // 1-entry pre-closure (B8 must pass for the bind to issue) — a
    // 1-event rig keeps the ONLY divergence from env_a in the
    // influence record itself.
    let rig_b = rig(1).await;
    let bind_history_b2 = history_path("e05-bind-b2");
    let mut history_b2 = RepairHistory::open(&bind_history_b2).unwrap();
    let scratch_b2 = history_path("e05-scratch-b2");
    let influence_b2 = test_influence(&config(), &rig_b.ids);
    let mut chain_b2 = chain_inputs(&rig_b, &scratch_b2, &mut history_b2);
    let (env_b, _h) = bind_execution(&influence_b2, &mut chain_b2, &rig_b.signer, &config())
        .await
        .unwrap();

    // Each envelope carries ITS OWN influence's id — a mismatched
    // influence cannot be swapped in undetected.
    assert_eq!(env_a.body().influence_id, influence_a.influence_id());
    assert_eq!(env_b.body().influence_id, influence_b2.influence_id());
    // Different influences → different artifacts (execution_id is a
    // function of the whole body, influence_id included).
    assert_ne!(env_a.body().influence_id, env_b.body().influence_id);
    assert_ne!(env_a.body().execution_id, env_b.body().execution_id);
    // The mismatch is visible without any secret: the recorded member is
    // directly comparable to the caller's influence record.
    assert_ne!(env_a.body().influence_id, influence_b2.influence_id());

    // §7.5 core (bind-time): the influence entry set IS the bound
    // pre-closure by construction, so the tamper surface is VERIFY-side:
    // envelope B (bound to influence B over the 2-event chain) replayed
    // against a DIFFERENT (3-event) chain must be refused — the recorded
    // pre_closure_count/pre_closure_ids_hash members cannot match the
    // larger chain.
    let mismatch_rig = rig(3).await;
    let mismatch_history = history_path("e05-verify-mismatch");
    let mut history_m = RepairHistory::open(&mismatch_history).unwrap();
    let scratch_m = history_path("e05-scratch-m");
    let mut chain_m = chain_inputs(&mismatch_rig, &scratch_m, &mut history_m);
    assert!(
        verify_execution(&env_b, &mut chain_m, &config())
            .await
            .is_err(),
        "§7.5: recorded execution vs a different chain must be refused at verify"
    );

    // §7.5 forged-envelope refusal (E05 matrix evidence): a FORGED
    // envelope — canonical bytes (passes the canonical gate), FRESH
    // valid signature, self-consistent §9 id — must be refused by the
    // replay over the chain it claims. The forged closed_count cannot
    // match the recomputed closure. (Full evidence lives in
    // `canonical_extra_member_rejected`; this row asserts its own copy.)
    let mut forged_body = env_b.body().clone();
    forged_body.closed_count += 1;
    forged_body.execution_id = derive_execution_id(&forged_body);
    let forged_env = SignedExecutionV1::issue(forged_body, &r.signer)
        .expect("forged body is self-consistent and canonical");
    let forge_history = history_path("e05-verify-forged");
    let mut history_f = RepairHistory::open(&forge_history).unwrap();
    let scratch_f = history_path("e05-scratch-f");
    let forged_rig = rig(2).await;
    let mut chain_f = chain_inputs(&forged_rig, &scratch_f, &mut history_f);
    assert!(
        verify_execution(&forged_env, &mut chain_f, &config())
            .await
            .is_err(),
        "§7.5: a canonical, freshly-signed forged envelope must be refused by replay"
    );
}

// ---------------------------------------------------------------------------
// E06: budget_events_refusal — max_events exceeded ALONE → deterministic
// refusal (never truncation)
// ---------------------------------------------------------------------------

const EVENTS_CAPPED_BUDGET: SelectionBudget = SelectionBudget {
    max_selected_events: 0,
    max_exported_bytes: 64 * 1024 * 1024,
};

#[tokio::test]
async fn budget_events_refusal() {
    let r = rig(3).await;
    let bind_history = history_path("e06-bind");
    let mut history = RepairHistory::open(&bind_history).unwrap();
    let scratch = history_path("e06-scratch");
    let influence = test_influence(&config(), &r.ids);

    let mut chain = chain_inputs(&r, &scratch, &mut history);
    // Byte budget is generous; ONLY the event cap binds → over-budget
    // closed set → Err, no artifact.
    chain.budget = &EVENTS_CAPPED_BUDGET;
    assert!(
        bind_execution(&influence, &mut chain, &r.signer, &config())
            .await
            .is_err(),
        "E06: zero-events budget must refuse, not truncate"
    );
}

// ---------------------------------------------------------------------------
// E06b: budget_bytes_refusal — max_bytes exceeded ALONE → refusal
// ---------------------------------------------------------------------------

const BYTES_CAPPED_BUDGET: SelectionBudget = SelectionBudget {
    max_selected_events: 100_000,
    max_exported_bytes: 0,
};

#[tokio::test]
async fn budget_bytes_refusal() {
    let r = rig(3).await;
    let bind_history = history_path("e06b-bind");
    let mut history = RepairHistory::open(&bind_history).unwrap();
    let scratch = history_path("e06b-scratch");
    let influence = test_influence(&config(), &r.ids);

    let mut chain = chain_inputs(&r, &scratch, &mut history);
    // Event cap generous; ONLY the byte cap binds (the delta carries
    // non-empty wire bytes) → Err.
    chain.budget = &BYTES_CAPPED_BUDGET;
    assert!(
        bind_execution(&influence, &mut chain, &r.signer, &config())
            .await
            .is_err(),
        "E06b: zero-bytes budget must refuse, not truncate"
    );
}

// ---------------------------------------------------------------------------
// E08: execution_golden — full-pipeline committed fixture + sha256 sidecar
// (the generator run IS the pipeline; deterministic fixture-seed identities
// and a content-only body make the pipeline byte-reproducible)
// ---------------------------------------------------------------------------

const GOLDEN_JSON: &str = "tests/fixtures/oc04-execution-v1-golden.json";
const GOLDEN_SHA: &str = "tests/fixtures/oc04-execution-v1-golden.sha256";

async fn golden_pipeline_body() -> contextmesh_salience::oc04_selection::SelectionExecutionBodyV1 {
    let r = rig(3).await;
    let bind_history = history_path("e08-bind");
    let mut history = RepairHistory::open(&bind_history).unwrap();
    let scratch = history_path("e08-scratch");
    let influence = test_influence(&config(), &r.ids);
    let mut chain = chain_inputs(&r, &scratch, &mut history);
    let (env, _handoff) = bind_execution(&influence, &mut chain, &r.signer, &config())
        .await
        .expect("golden pipeline must bind");
    env.body().clone()
}

fn sha256_of(bytes: &[u8]) -> String {
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sha256sum available");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(bytes)
        .expect("pipe");
    String::from_utf8(child.wait_with_output().expect("wait").stdout)
        .expect("utf8")
        .split_whitespace()
        .next()
        .expect("digest")
        .to_owned()
}

#[test]
fn execution_golden() {
    // E08: the committed fixture IS the pipeline output — a fresh run must
    // reproduce it byte-for-byte, and the sidecar must match the bytes.
    let body = futures_block_on(golden_pipeline_body());
    let bytes = render_execution_body(&body);
    let committed = std::fs::read(GOLDEN_JSON).expect("committed golden fixture");
    assert_eq!(bytes, committed, "E08: pipeline output == committed bytes");
    let sidecar = std::fs::read_to_string(GOLDEN_SHA).expect("sidecar");
    assert_eq!(sha256_of(&bytes), sidecar.trim(), "E08: sidecar matches");
}

/// Minimal single-threaded block-on for the golden test (tokio
/// dev-dependency rt feature not enabled for `macros`-only builds? — use
/// the tokio runtime the other async tests already pull in).
fn futures_block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(fut)
}

/// Golden fixture generator (#[ignore]; regenerate only via explicit
/// `cargo test -- --ignored golden_generator` after change control — the
/// generator run IS the full B3–B8 pipeline).
#[tokio::test]
#[ignore = "golden fixture: change-control gate; run explicitly"]
async fn golden_generator() {
    let body = golden_pipeline_body().await;
    let bytes = render_execution_body(&body);
    std::fs::create_dir_all("tests/fixtures").expect("dir");
    std::fs::write(GOLDEN_JSON, &bytes).expect("write fixture");
    std::fs::write(GOLDEN_SHA, sha256_of(&bytes)).expect("write sidecar");
}

// ---------------------------------------------------------------------------
// E09: b7_nonconvergence_no_artifact — non-converging driver →
// bind_execution Err, no SignedExecutionV1 emitted
// ---------------------------------------------------------------------------

#[tokio::test]
async fn b7_nonconvergence_no_artifact() {
    let r = rig(2).await;
    let bind_history = history_path("e09-bind");
    let mut history = RepairHistory::open(&bind_history).unwrap();
    let scratch = history_path("e09-scratch");
    let influence = test_influence(&config(), &r.ids);

    // Drive the chain through the FAILURE driver by swapping the factory.
    fn fail_driver(_current: &Handoff) -> std::future::Ready<TaskOutcome> {
        std::future::ready(TaskOutcome::Failure {
            note: "hard failure".to_owned(),
        })
    }
    let fail_factory: DriverFactory = || fail_driver;
    let mut chain = ExecutionChainInputs {
        repair_driver_factory: fail_factory,
        ..chain_inputs(&r, &scratch, &mut history)
    };
    // Err — the Result carries no SignedExecutionV1 on failure.
    assert!(
        bind_execution(&influence, &mut chain, &r.signer, &config())
            .await
            .is_err()
    );
}

// ---------------------------------------------------------------------------
// E10: b8_failure_no_artifact — B8 simulate failure → bind_execution Err,
// no artifact. Fixture: a fully-advanced recipient (at head) yields an
// EMPTY delta, so every critical event is hidden (delivered nowhere,
// omitted nowhere) → B8 refuses the bind.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn b8_failure_no_artifact() {
    let r = rig(3).await;
    let head = *r.ids.last().unwrap();
    let advanced = RecipientState::at_head(&r.store, r.context, head, &r.limits)
        .await
        .expect("advanced recipient");
    let bind_history = history_path("e10-bind");
    let mut history = RepairHistory::open(&bind_history).unwrap();
    let scratch = history_path("e10-scratch");
    let influence = test_influence(&config(), &r.ids);

    let mut chain = ExecutionChainInputs {
        recipient: &advanced,
        ..chain_inputs(&r, &scratch, &mut history)
    };
    assert!(
        bind_execution(&influence, &mut chain, &r.signer, &config())
            .await
            .is_err(),
        "E10: hidden criticals (empty delta) must refuse the bind"
    );
}

// ---------------------------------------------------------------------------
// E11: checked_overflow_rejected — the u128 checked-add surface exercised
// at its REACHABLE extreme: maximal u64 arms sum exactly in u128 (a wrap,
// saturate, or clamp implementation fails this assert). NOTE: an actual
// overflow of the u128 checked add is unconstructible on the frozen
// surface (u64 + u64 fits u128 by construction) — same positive-only
// vacuity class as U03; matrix evidence wording change proposed.
// ---------------------------------------------------------------------------

#[test]
fn checked_overflow_rejected() {
    let maximal = SelectionInfluenceEntryV1::new("ev-max", ENTRY_REASON_BOTH, u64::MAX, u64::MAX)
        .expect("maximal coherent both-arm entry");
    // Exact u128 sum: no wraparound, no saturation, no clamping.
    assert_eq!(
        maximal.score_ppm(),
        u128::from(u64::MAX) + u128::from(u64::MAX),
        "E11: checked u128 add must be exact at the reachable extreme"
    );
    assert_eq!(maximal.lexical_ppm(), u64::MAX);
    assert_eq!(maximal.prior_ppm(), u64::MAX);
}

// ---------------------------------------------------------------------------
// ScratchHistoryGuard reservation (4E deliverable consumption; X12/X12b
// own the adversarial placement at 4F) — pre-existing file and same-path
// rejection, fail-closed before any chain step
// ---------------------------------------------------------------------------

#[tokio::test]
async fn scratch_guard_reservation_fail_closed() {
    let production = history_path("guard-prod");

    // A pre-existing file at the scratch path must be rejected.
    std::fs::write(&production, b"occupied\n").unwrap();
    let guard = contextmesh_salience::oc04_selection::ScratchHistoryGuard::reserve(
        &production,
        &history_path("guard-other"),
    );
    assert!(guard.is_err(), "pre-existing scratch file must be rejected");
    std::fs::remove_file(&production).unwrap();

    // A scratch path equal to the production path must be rejected.
    let guard_same = contextmesh_salience::oc04_selection::ScratchHistoryGuard::reserve(
        &production,
        &production,
    );
    assert!(guard_same.is_err(), "same-path scratch must be rejected");
}

// ---------------------------------------------------------------------------
// S07b: canonicalization gate — an execution envelope whose canonical
// serialization gains an extra member must be rejected on verify (4B
// canonicalization discipline, delivered at 4E per v11 §3; matrix places
// the row in oc04_adversarial.rs — placement discrepancy reported)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn canonical_extra_member_rejected() {
    // S07b (matrix evidence): parsed extra-member bytes → canonical gate
    // Err; parsed value-tampered (forged) bytes → verify_execution Err.
    // The lenient parser accepts anything well-formed; the canonical
    // gate and the §7.5 replay are the rejectors.
    let r = rig(1).await;
    let bind_history = history_path("s07b-bind");
    let mut history = RepairHistory::open(&bind_history).unwrap();
    let scratch = history_path("s07b-scratch");
    let influence = test_influence(&config(), &r.ids);
    let mut chain = chain_inputs(&r, &scratch, &mut history);
    let (env, _handoff) = bind_execution(&influence, &mut chain, &r.signer, &config())
        .await
        .unwrap();

    // Re-render the real body, inject an extra member, parse it back
    // leniently (S07 path), then demand the canonical gate reject it.
    let canonical = render_execution_body(env.body());
    let text = std::str::from_utf8(&canonical).unwrap();
    let injected = text.replacen('{', "{\"aardvark_extra_member\":\"sneaky\",", 1);
    // Lenient (S07): parses.
    let _lenient = parse_execution_body_lenient(injected.as_bytes())
        .expect("lenient parser must accept the extra member (S07)");
    // Canonical (S07b): refused — the bytes diverge from the §6 re-render.
    assert!(
        parse_execution_body_canonical(injected.as_bytes()).is_err(),
        "S07b: canonical gate must refuse extra-member bytes"
    );
    // The unmodified canonical bytes pass the gate.
    let unmodified = parse_execution_body_canonical(&canonical)
        .expect("canonical bytes must pass the canonical gate");
    assert_eq!(unmodified.execution_id, env.body().execution_id);

    // Matrix evidence continuation: parsed TAMPERED bytes →
    // verify_execution Err. A member-VALUE tamper survives the canonical
    // parser (the bytes still re-render to themselves) but the §7.5
    // replay refuses it — the recorded execution_id/handoff_hash can no
    // longer match a fresh recomputation over the tampered members.
    let mut tampered_bytes = canonical.clone();
    // Tamper `closed_count` (first frozen value) inside the canonical
    // JSON: locate its key and bump the value.
    let key = b"\"closed_count\":";
    let pos = tampered_bytes
        .windows(key.len())
        .position(|w| w == key)
        .expect("closed_count member present");
    let val_start = pos + key.len();
    assert_eq!(
        tampered_bytes[val_start], b'2',
        "fixture: golden rig(1) closes 2"
    );
    tampered_bytes[val_start] = b'3';
    let tampered = parse_execution_body_canonical(&tampered_bytes)
        .expect("a member-VALUE tamper is still canonical (S07 parser accepts)");
    assert_ne!(
        derive_execution_id(&tampered),
        env.body().execution_id,
        "S07b: the value tamper must shift the §9 id derivation"
    );
    // Re-derive the id over the tampered members so issue() accepts —
    // a self-consistent forged envelope is exactly the §7.5 replay's
    // target: bytes parse canonical, signature is fresh and valid, yet
    // the REPLAY refuses it (closed_count/delta_hash recomputation
    // diverges from the forged members).
    let mut forged = tampered;
    forged.execution_id = derive_execution_id(&forged);
    let tampered_env = SignedExecutionV1::issue(forged, &r.signer)
        .expect("issue signs a self-consistent (forged) body");
    let mut history_v = RepairHistory::open(&bind_history).unwrap();
    let scratch_v = history_path("s07b-verify");
    let mut chain_v = chain_inputs(&r, &scratch_v, &mut history_v);
    assert!(
        verify_execution(&tampered_env, &mut chain_v, &config())
            .await
            .is_err(),
        "S07b: parsed tampered bytes must be refused by verify_execution"
    );
}

// ---------------------------------------------------------------------------
// Shared test constants/helpers
// ---------------------------------------------------------------------------

/// Real influence record via the 4B public `assemble` surface. Entries
/// are LEXICAL (reason `lexical`, prior_ppm = 0) naming exactly the
/// fixture chain's pre-closure ids — §7.5 requires an influence's
/// entries to match the pre-closure it binds, and the lexical reason
/// keeps the marker rule at `prior_arm_empty`.
fn test_influence(config: &Oc04ConfigV1, pre_closure: &[EventId]) -> SelectionInfluenceV1 {
    let mut entries: Vec<SelectionInfluenceEntryV1> = pre_closure
        .iter()
        .map(|id| {
            SelectionInfluenceEntryV1::new(id.to_string(), ENTRY_REASON_LEXICAL, 1, 0)
                .expect("coherent lexical entry")
        })
        .collect();
    entries.sort_by(|a, b| {
        b.score_ppm()
            .cmp(&a.score_ppm())
            .then_with(|| a.event_id_text().cmp(b.event_id_text()))
    });
    SelectionInfluenceV1::assemble(
        config,
        // OC-03 normative prior ID prefix (prior.rs PRIOR_ID_PREFIX).
        "ocprior1_0000000000000000000000000000000000000000000000000000000000000000",
        "task-fingerprint-fixtures",
        entries,
    )
    .unwrap()
}
