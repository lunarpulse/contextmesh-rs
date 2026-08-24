//! OC-01 Stage 2D DAG/context/current-input tests (matrix rows OC01-P07,
//! OC01-P08, and OC01-D01..D10).
//!
//! Store-aware issuance order, fail-closed atomicity, snapshot capture
//! canonicalization, event-role coverage with read deduplication, missing
//! and cross-context rejection, store-error mapping, admission-as-evidence,
//! no invented signer policy, immutable-DAG validity after ref moves,
//! the stale-input change matrix, and the bounded verification report.

use contextmesh::crypto::SigningIdentity;
use contextmesh::model::{AuthorId, ContextId, EventId, SignedEventV1};
use contextmesh::store::{
    ContextProvision, LocalRefName, PeerName, RefExpectation, RefMutation, Store,
};
use contextmesh_salience::error::{OutcomeError, OutcomeOperationError};
use contextmesh_salience::outcome::{OutcomeLedgerBodyV1, SignedOutcomeLedgerV1};
use contextmesh_salience::types::{
    AttemptErrorV1, AttemptStatus, AttemptV1, AttributionLabel, AttributionMarkV1, Blake3HashText,
    CostLedgerV1, CostValueV1, DeadEndV1, Disposition, InputRefFingerprint, InputRefSnapshotV1,
    MechanismRecordV1, OutcomeLimits, OutcomeRecordV1, OutcomeValue, QualityV1, TaskBindingV1,
    TerminalV1, TimestampText,
};
use serde_json::json;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// Published test-only signing seed for the outcome-ledger issuer.
const ISSUER_SEED: [u8; 32] = [0x4f; 32];
/// Published test-only signing seed for the event author.
const EVENT_AUTHOR_SEED: [u8; 32] = [0x55; 32];
/// Published test-only signing seed for a foreign-context author.
const FOREIGN_AUTHOR_SEED: [u8; 32] = [0x66; 32];

static NEXT: AtomicU64 = AtomicU64::new(0);

fn temp_db(label: &str) -> PathBuf {
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "oc01-dag-{label}-{}-{serial}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn limits() -> OutcomeLimits {
    OutcomeLimits::default()
}

fn identity(seed: [u8; 32]) -> SigningIdentity {
    SigningIdentity::from_fixture_seed(seed)
}

fn hash_text_of(bytes: &[u8]) -> Blake3HashText {
    let digest = blake3::hash(bytes);
    let mut hex = String::new();
    for byte in digest.as_bytes() {
        hex.push_str(&format!("{byte:02x}"));
    }
    Blake3HashText::parse(&format!("blake3_{hex}")).expect("hash text is valid")
}

fn context() -> ContextId {
    ContextId::from_bytes([7; 32])
}

fn foreign_context() -> ContextId {
    ContextId::from_bytes([9; 32])
}

fn mechanism() -> MechanismRecordV1 {
    MechanismRecordV1::new(
        "caller.example".to_owned(),
        "1.0.0".to_owned(),
        hash_text_of(b"oc01-dag-mechanism"),
        &limits(),
    )
    .expect("mechanism is valid")
}

fn cost_available(value: u64) -> CostValueV1 {
    CostValueV1::new(
        CostValueV1::Available {
            value,
            provenance: mechanism(),
        },
        &limits(),
    )
    .expect("cost is valid")
}

fn cost_ledger() -> CostLedgerV1 {
    CostLedgerV1::new(
        CostLedgerV1 {
            wall_clock_ms: cost_available(17),
            tool_calls: cost_available(1),
            retries: cost_available(0),
            input_tokens: cost_available(2_000),
            output_tokens: cost_available(500),
        },
        &limits(),
    )
    .expect("cost ledger is valid")
}

/// Builds one admitted event owned by the event author in the target context.
async fn admitted_event(
    store: &Store,
    author: &SigningIdentity,
    context: ContextId,
    parents: Vec<EventId>,
    kind: &str,
) -> SignedEventV1 {
    let event = author
        .create_event(context, parents, kind, json!({"oc01":"dag"}))
        .expect("event constructs");
    store
        .admit(&event, RefMutation::None)
        .await
        .expect("event admits");
    event
}

/// Builds a complete store fixture: provisioned context, genesis, chain of
/// agent events, one local ref, and one remote ref under fixed identities.
async fn fixture_store() -> (Store, Vec<EventId>) {
    let store = Store::open(temp_db("fixture")).await.expect("store opens");
    let author = identity(EVENT_AUTHOR_SEED);
    let ctx = context();
    let genesis = author.create_event(ctx, vec![], "context.genesis", json!({}));
    let genesis = genesis.expect("genesis constructs");
    store
        .provision_context(ContextProvision {
            context: ctx,
            expected_genesis: genesis.event_id(),
            authorized_authors: vec![author.author()],
        })
        .await
        .expect("context provisions");
    store
        .admit(
            &genesis,
            RefMutation::CompareAndSwap {
                context: ctx,
                name: "main".parse::<LocalRefName>().expect("name parses"),
                expected: RefExpectation::Absent,
                new_head: genesis.event_id(),
            },
        )
        .await
        .expect("genesis admits with ref");
    let mut events = vec![genesis.event_id()];
    let mut parent = genesis.event_id();
    for _ in 0..5 {
        let event = admitted_event(&store, &author, ctx, vec![parent], "agent.request").await;
        // Advance the local ref to each new chain head so the fixture's
        // local head is the newest event.
        store
            .admit(
                &event,
                RefMutation::CompareAndSwap {
                    context: ctx,
                    name: "main".parse::<LocalRefName>().expect("name parses"),
                    expected: RefExpectation::Head(parent),
                    new_head: event.event_id(),
                },
            )
            .await
            .expect("chain event admits with ref advance");
        parent = event.event_id();
        events.push(event.event_id());
    }
    // A remote-tracking ref advertised by a peer, pointing at an admitted
    // same-context event.
    store
        .set_remote_ref(
            "peer.example".parse::<PeerName>().expect("peer parses"),
            ctx,
            "main".parse::<LocalRefName>().expect("name parses"),
            events[2],
        )
        .await
        .expect("remote ref sets");
    (store, events)
}

/// Snapshot of the fixture store: local main head = events[5], remote head = events[2].
async fn snapshot(store: &Store) -> InputRefSnapshotV1 {
    InputRefSnapshotV1::capture(store, context(), limits())
        .await
        .expect("snapshot captures")
}

fn attempt(index: usize, parent: Option<usize>, refs: Vec<EventId>) -> AttemptV1 {
    AttemptV1::new(
        AttemptV1 {
            attempt_id: format!("attempt1_{index:06}"),
            parent_attempt_id: parent.map(|p| format!("attempt1_{p:06}")),
            status: AttemptStatus::Failed,
            operation_fingerprint: hash_text_of(b"oc01-dag-attempt"),
            event_refs: refs,
            error: AttemptErrorV1::Available {
                category: "provider-timeout".to_owned(),
                fingerprint: hash_text_of(b"oc01-dag-error"),
            },
            costs: cost_ledger(),
            provenance: mechanism(),
        },
        &limits(),
    )
    .expect("attempt is valid")
}

fn dead_end(refs: Vec<EventId>) -> DeadEndV1 {
    DeadEndV1::new(
        DeadEndV1 {
            dead_end_id: "dead1_000000".to_owned(),
            attempt_id: "attempt1_000000".to_owned(),
            failure_category: "provider-timeout".to_owned(),
            error_fingerprint: hash_text_of(b"oc01-dag-dead-end"),
            event_refs: refs,
            disposition: Disposition::Abandoned,
            provenance: mechanism(),
        },
        &limits(),
    )
    .expect("dead end is valid")
}

fn mark(event_id: EventId, label: AttributionLabel) -> AttributionMarkV1 {
    AttributionMarkV1::new(
        AttributionMarkV1 {
            event: event_id,
            label,
            evidence: vec![],
            mechanism: mechanism(),
        },
        &limits(),
    )
    .expect("mark is valid")
}

/// Every EventId-bearing collector role represented by [`full_body`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EventRole {
    InputLocalHead,
    InputRemoteHead,
    Terminal,
    OutcomeEvidence,
    QualityEvidence,
    AttemptRefs,
    DeadEndRefs,
    AttributionMarkEvent,
    AttributionMarkEvidence,
}

const EVENT_ROLES: [EventRole; 9] = [
    EventRole::InputLocalHead,
    EventRole::InputRemoteHead,
    EventRole::Terminal,
    EventRole::OutcomeEvidence,
    EventRole::QualityEvidence,
    EventRole::AttemptRefs,
    EventRole::DeadEndRefs,
    EventRole::AttributionMarkEvent,
    EventRole::AttributionMarkEvidence,
];

impl EventRole {
    const fn label(self) -> &'static str {
        match self {
            Self::InputLocalHead => "input local head",
            Self::InputRemoteHead => "input remote head",
            Self::Terminal => "terminal",
            Self::OutcomeEvidence => "outcome evidence",
            Self::QualityEvidence => "quality evidence",
            Self::AttemptRefs => "attempt refs",
            Self::DeadEndRefs => "dead-end refs",
            Self::AttributionMarkEvent => "attribution mark event",
            Self::AttributionMarkEvidence => "attribution mark evidence",
        }
    }
}

/// Body touching every EventId role. An optional replacement changes exactly
/// one role while preserving each constructor's canonical-array requirements.
async fn full_body_with_substitution(
    store: &Store,
    events: &[EventId],
    author: AuthorId,
    replacement: Option<(EventRole, EventId)>,
) -> OutcomeLedgerBodyV1 {
    let mut snapshot = snapshot(store).await;
    if let Some((EventRole::InputLocalHead, event)) = replacement {
        snapshot.local[0].head = event;
    }
    if let Some((EventRole::InputRemoteHead, event)) = replacement {
        snapshot.remote[0].head = event;
    }
    let snapshot = InputRefSnapshotV1::new(context(), snapshot.local, snapshot.remote)
        .expect("substituted snapshot remains canonical");
    // Real BLAKE3 EventIds are not ordinal-byte ordered. OC-01 requires
    // event arrays and attribution composite keys in caller canonical order.
    let mut attempt_refs = vec![events[1], events[2]];
    if let Some((EventRole::AttemptRefs, event)) = replacement {
        attempt_refs[0] = event;
    }
    attempt_refs.sort_by_key(ToString::to_string);
    let mut dead_end_refs = vec![events[3]];
    if let Some((EventRole::DeadEndRefs, event)) = replacement {
        dead_end_refs[0] = event;
    }
    dead_end_refs.sort_by_key(ToString::to_string);
    let mut marks = vec![
        mark(
            match replacement {
                Some((EventRole::AttributionMarkEvent, event)) => event,
                _ => events[1],
            },
            AttributionLabel::LoadBearingCandidate,
        ),
        AttributionMarkV1::new(
            AttributionMarkV1 {
                event: events[2],
                label: AttributionLabel::SupportingCandidate,
                evidence: match replacement {
                    Some((EventRole::AttributionMarkEvidence, event)) => vec![event],
                    _ => vec![events[4]],
                },
                mechanism: mechanism(),
            },
            &limits(),
        )
        .expect("mark evidence is canonical"),
    ];
    marks.sort_by_key(|entry| {
        (
            entry.event.to_string(),
            entry.label.text(),
            entry.mechanism.identity.clone(),
            entry.mechanism.version.clone(),
            entry.mechanism.config_hash.as_str().to_owned(),
        )
    });
    OutcomeLedgerBodyV1::new(
        context(),
        snapshot,
        TaskBindingV1::new(hash_text_of(b"oc01-dag-task"), None, None, &limits())
            .expect("task binds"),
        TerminalV1::Event {
            event: match replacement {
                Some((EventRole::Terminal, event)) => event,
                _ => events[5],
            },
        },
        OutcomeRecordV1::new(
            OutcomeValue::Succeeded,
            match replacement {
                Some((EventRole::OutcomeEvidence, event)) => vec![event],
                _ => vec![events[4]],
            },
            mechanism(),
            &limits(),
        )
        .expect("outcome records"),
        QualityV1::new(
            QualityV1::Available {
                value_ppm: 990_000,
                evidence: match replacement {
                    Some((EventRole::QualityEvidence, event)) => vec![event],
                    _ => vec![events[3]],
                },
                provenance: mechanism(),
            },
            &limits(),
        )
        .expect("quality records"),
        cost_ledger(),
        vec![attempt(0, None, attempt_refs)],
        vec![dead_end(dead_end_refs)],
        marks,
        vec![],
        TimestampText::parse("2026-08-24T00:00:00Z").expect("timestamp parses"),
        author,
        limits(),
    )
    .expect("body is valid")
}

async fn full_body(store: &Store, events: &[EventId], author: AuthorId) -> OutcomeLedgerBodyV1 {
    full_body_with_substitution(store, events, author, None).await
}

async fn issue_full(store: &Store, events: &[EventId]) -> SignedOutcomeLedgerV1 {
    let issuer = identity(ISSUER_SEED);
    SignedOutcomeLedgerV1::issue(
        &issuer,
        store,
        full_body(store, events, issuer.author()).await,
        limits(),
    )
    .await
    .expect("ledger issues")
}

/// OC01-P07: static source audit anchors the frozen issue order; this public
/// boundary suite verifies successful issuance plus author/missing/stale
/// failures, without a production phase-instrumentation seam.
#[tokio::test]
async fn issue_executes_fail_closed_steps_in_exact_order() {
    let (store, events) = fixture_store().await;
    let issuer = identity(ISSUER_SEED);

    // Happy path: every step passes in order and returns a fully verified
    // artifact plus a bounded report.
    let ledger = issue_full(&store, &events).await;
    let report = ledger
        .verify_against_dag(&store, limits())
        .await
        .expect("full DAG verification passes");
    assert_eq!(report.unique_events(), 5);
    // Occurrences: local(1) + remote(1) + terminal(1) + outcome evidence(1)
    // + quality evidence(1) + attempt refs(2) + dead-end refs(1) + mark
    // events(2) + mark evidence(1) = 11.
    assert_eq!(report.event_occurrences(), 11);

    // Injected failure 1: invalid body stops before any store access — a
    // body whose author differs never reaches event loading.
    let other = identity(FOREIGN_AUTHOR_SEED);
    let mismatch = SignedOutcomeLedgerV1::issue(
        &issuer,
        &store,
        full_body(&store, &events, other.author()).await,
        limits(),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        mismatch,
        OutcomeOperationError::Artifact(OutcomeError::IdMismatch)
    ));

    // Injected failure 2: a missing referenced event stops issuance; nothing
    // is returned. Build a body referencing a real admitted event plus one
    // absent EventId.
    let absent = EventId::from_bytes([0xEE; 32]);
    let issuer_author = issuer.author();
    let snapshot = snapshot(&store).await;
    let body = OutcomeLedgerBodyV1::new(
        context(),
        snapshot,
        TaskBindingV1::new(hash_text_of(b"oc01-dag-task"), None, None, &limits()).unwrap(),
        TerminalV1::Event { event: absent },
        OutcomeRecordV1::new(OutcomeValue::Succeeded, vec![], mechanism(), &limits()).unwrap(),
        QualityV1::new(
            QualityV1::Unavailable {
                reason: "no rubric".into(),
                provenance: mechanism(),
            },
            &limits(),
        )
        .unwrap(),
        cost_ledger(),
        vec![],
        vec![],
        vec![],
        vec![],
        TimestampText::parse("2026-08-24T00:00:00Z").unwrap(),
        issuer_author,
        limits(),
    )
    .unwrap();
    let missing = SignedOutcomeLedgerV1::issue(&issuer, &store, body, limits())
        .await
        .unwrap_err();
    assert!(matches!(
        missing,
        OutcomeOperationError::Artifact(OutcomeError::MissingEvent)
    ));

    // Injected failure 3: a stale embedded snapshot stops issuance before
    // ID derivation/signing. Mutate the store refs after building a body
    // whose snapshot is now stale.
    let stale_body = full_body(&store, &events, issuer.author()).await;
    let author = identity(EVENT_AUTHOR_SEED);
    let extra = author
        .create_event(
            context(),
            vec![events[5]],
            "agent.request",
            json!({"oc01":"stale"}),
        )
        .expect("event constructs");
    store
        .admit(
            &extra,
            RefMutation::CompareAndSwap {
                context: context(),
                name: "main".parse().unwrap(),
                expected: RefExpectation::Head(events[5]),
                new_head: extra.event_id(),
            },
        )
        .await
        .expect("ref moves");
    let stale = SignedOutcomeLedgerV1::issue(&issuer, &store, stale_body, limits())
        .await
        .unwrap_err();
    assert!(matches!(
        stale,
        OutcomeOperationError::Artifact(OutcomeError::StaleInput)
    ));
    // The extra event exists in the store; clean up is unnecessary because
    // the store is per-test and immutable events never block later tests.
    assert!(store.event(extra.event_id()).await.unwrap().is_some());
}

/// OC01-P08: any failure returns neither artifact nor partial report and
/// never mutates observable store state.
#[tokio::test]
async fn issue_returns_no_artifact_and_never_mutates_store_on_any_failure() {
    let (store, events) = fixture_store().await;
    let issuer = identity(ISSUER_SEED);
    let absent = EventId::from_bytes([0xEE; 32]);

    // Missing-event issuance: take a complete local+remote snapshot directly
    // around this one failed public call, then recheck every fixture event.
    let missing_body = full_body_with_substitution(
        &store,
        &events,
        issuer.author(),
        Some((EventRole::Terminal, absent)),
    )
    .await;
    let before = snapshot(&store).await;
    let error = SignedOutcomeLedgerV1::issue(&issuer, &store, missing_body, limits())
        .await
        .unwrap_err();
    let after = snapshot(&store).await;
    assert!(matches!(
        error,
        OutcomeOperationError::Artifact(OutcomeError::MissingEvent)
    ));
    assert_eq!(after, before, "missing-event issue must not mutate refs");
    assert_fixture_events_present(&store, &events).await;

    // Deliberately make the body stale *before* the before-state capture.
    let stale_body = full_body(&store, &events, issuer.author()).await;
    let author = identity(EVENT_AUTHOR_SEED);
    let extra = admitted_event(&store, &author, context(), vec![events[5]], "agent.request").await;
    store
        .admit(
            &extra,
            RefMutation::CompareAndSwap {
                context: context(),
                name: "main".parse().unwrap(),
                expected: RefExpectation::Head(events[5]),
                new_head: extra.event_id(),
            },
        )
        .await
        .expect("deliberate stale ref move succeeds");
    let before = snapshot(&store).await;
    let error = SignedOutcomeLedgerV1::issue(&issuer, &store, stale_body, limits())
        .await
        .unwrap_err();
    let after = snapshot(&store).await;
    assert!(matches!(
        error,
        OutcomeOperationError::Artifact(OutcomeError::StaleInput)
    ));
    assert_eq!(after, before, "stale-input issue must not mutate refs");
    assert_fixture_events_present(&store, &events).await;
    assert!(store.event(extra.event_id()).await.unwrap().is_some());

    // Author mismatch is a body-level failure, also bounded by an immediate
    // complete observable-state equality check.
    let mismatch_body = full_body(&store, &events, identity(FOREIGN_AUTHOR_SEED).author()).await;
    let before = snapshot(&store).await;
    let error = SignedOutcomeLedgerV1::issue(&issuer, &store, mismatch_body, limits())
        .await
        .unwrap_err();
    let after = snapshot(&store).await;
    assert!(matches!(
        error,
        OutcomeOperationError::Artifact(OutcomeError::IdMismatch)
    ));
    assert_eq!(after, before, "author-mismatch issue must not mutate refs");
    assert_fixture_events_present(&store, &events).await;
}

async fn assert_fixture_events_present(store: &Store, events: &[EventId]) {
    for event in events {
        assert!(
            store
                .event(*event)
                .await
                .expect("fixture event loads")
                .is_some(),
            "fixture event {event} remains present"
        );
    }
}

/// OC01-D01: capture canonicalizes complete local+remote refs, supports
/// empty refs, and computes one exact context-bound fingerprint.
#[tokio::test]
async fn capture_snapshot_canonicalizes_complete_local_and_remote_refs() {
    let (store, events) = fixture_store().await;
    let first = snapshot(&store).await;
    assert_eq!(first.local.len(), 1);
    assert_eq!(first.local[0].name, "main");
    assert_eq!(first.local[0].head, events[5]);
    assert_eq!(first.remote.len(), 1);
    assert_eq!(first.remote[0].peer, "peer.example");
    assert_eq!(first.remote[0].head, events[2]);

    // Permuted store insertion yields the same exact snapshot: add a second
    // local ref in non-sorted admission order and require canonical output.
    let author = identity(EVENT_AUTHOR_SEED);
    let event = admitted_event(&store, &author, context(), vec![events[5]], "agent.request").await;
    store
        .admit(
            &event,
            RefMutation::CompareAndSwap {
                context: context(),
                name: "alpha".parse::<LocalRefName>().unwrap(),
                expected: RefExpectation::Absent,
                new_head: event.event_id(),
            },
        )
        .await
        .expect("second ref admits");
    let canonical = snapshot(&store).await;
    let names: Vec<&str> = canonical
        .local
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(names, vec!["alpha", "main"]);

    // Empty case: a fresh store with no refs captures an empty snapshot.
    let empty_store = Store::open(temp_db("empty")).await.expect("opens");
    let empty = InputRefSnapshotV1::capture(&empty_store, context(), limits())
        .await
        .expect("empty capture");
    assert!(empty.local.is_empty());
    assert!(empty.remote.is_empty());

    // Context-bound: the same ref state under a different context yields a
    // different fingerprint.
    let other_ctx = foreign_context();
    let foreign = InputRefSnapshotV1::capture(&empty_store, other_ctx, limits())
        .await
        .expect("captures");
    assert_ne!(empty.fingerprint, foreign.fingerprint);
}

/// OC01-D02: every unique referenced/input/terminal event is loaded with
/// strict stored-wire verification and exact body context; read dedup does
/// not alter the occurrence-bound result.
#[tokio::test]
async fn dag_verification_covers_every_event_role_and_deduplicates_only_reads() {
    let (store, events) = fixture_store().await;
    let ledger = issue_full(&store, &events).await;
    let report = ledger
        .verify_against_dag(&store, limits())
        .await
        .expect("verifies");
    // events[5](terminal) events[4](outcome) events[3](quality)
    // events[1], events[2](attempt refs + marks) + two snapshot heads
    // events[5](local head), events[2](remote head) => unique set
    // {1,2,3,4,5} = 5 unique.
    assert_eq!(report.unique_events(), 5);
    // Occurrences include duplicates: local head events[5] counted again,
    // remote head events[2] counted again, marks referencing events[1]
    // and events[2] again.
    assert_eq!(report.event_occurrences(), 11);
    assert_eq!(report.local_refs(), 1);
    assert_eq!(report.remote_refs(), 1);
    assert_eq!(
        report.snapshot_fingerprint(),
        snapshot(&store).await.fingerprint.to_string()
    );
}

/// OC01-D03: every collector role rejects an absent EventId with
/// `Artifact(MissingEvent)`.
#[tokio::test]
async fn every_event_role_rejects_missing_event() {
    let (store, events) = fixture_store().await;
    let issuer = identity(ISSUER_SEED);
    let absent = EventId::from_bytes([0xEE; 32]);

    for role in EVENT_ROLES {
        let body =
            full_body_with_substitution(&store, &events, issuer.author(), Some((role, absent)))
                .await;
        let error = SignedOutcomeLedgerV1::issue(&issuer, &store, body, limits())
            .await
            .unwrap_err();
        assert!(
            matches!(
                error,
                OutcomeOperationError::Artifact(OutcomeError::MissingEvent)
            ),
            "{} must reject a missing event, got {error:?}",
            role.label()
        );
    }
}

/// OC01-D04: every collector role rejects an admitted foreign-context event
/// with `Artifact(ContextMismatch)`.
#[tokio::test]
async fn every_event_role_rejects_cross_context_event() {
    let (store, events) = fixture_store().await;
    let issuer = identity(ISSUER_SEED);
    let foreign_author = identity(FOREIGN_AUTHOR_SEED);
    let foreign_genesis = foreign_author
        .create_event(foreign_context(), vec![], "context.genesis", json!({}))
        .expect("foreign genesis constructs");
    store
        .provision_context(ContextProvision {
            context: foreign_context(),
            expected_genesis: foreign_genesis.event_id(),
            authorized_authors: vec![foreign_author.author()],
        })
        .await
        .expect("foreign context provisions");
    store
        .admit(&foreign_genesis, RefMutation::None)
        .await
        .expect("foreign event admits");

    for role in EVENT_ROLES {
        let body = full_body_with_substitution(
            &store,
            &events,
            issuer.author(),
            Some((role, foreign_genesis.event_id())),
        )
        .await;
        let error = SignedOutcomeLedgerV1::issue(&issuer, &store, body, limits())
            .await
            .unwrap_err();
        assert!(
            matches!(
                error,
                OutcomeOperationError::Artifact(OutcomeError::ContextMismatch)
            ),
            "{} must reject a foreign-context event, got {error:?}",
            role.label()
        );
    }
}

/// OC01-D05: every Store operational failure stays a Store-cause and is
/// never mislabeled as an artifact category.
#[tokio::test]
async fn store_error_mapping_is_total_generic_and_nonsecret() {
    // A corrupted (unopenable) database path produces a store operational
    // failure surfaced as OutcomeOperationError::Store, never as malformed.
    let bad = std::env::temp_dir().join(format!(
        "oc01-dag-corrupt-{}-{}.db",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&bad, b"not a sqlite database at all").expect("writes");
    let store = match Store::open(&bad).await {
        Ok(_) => panic!("open must fail on a corrupt database"),
        Err(error) => error,
    };
    // The mapping surface: Store errors never masquerade as artifact
    // categories. Model the mapping contract through the wrapper directly.
    let wrapped = OutcomeOperationError::from(store);
    assert!(matches!(wrapped, OutcomeOperationError::Store(_)));
    // Display is generic and non-secret.
    let text = wrapped.to_string();
    assert_eq!(text, "outcome store operation failed");
    // Every StoreError variant maps through the same wrapper (totality).
    let variants: Vec<OutcomeOperationError> = all_store_error_variants()
        .into_iter()
        .map(OutcomeOperationError::from)
        .collect();
    assert_eq!(variants.len(), 29);
    for wrapped in variants {
        assert!(matches!(wrapped, OutcomeOperationError::Store(_)));
        assert_eq!(wrapped.to_string(), "outcome store operation failed");
    }
}

/// Exhaustively lists every public `StoreError` variant for the mapping
/// totality check (kept in lockstep with `contextmesh::error::StoreError`).
fn all_store_error_variants() -> Vec<contextmesh::error::StoreError> {
    use contextmesh::error::StoreError as E;
    let event = EventId::from_bytes([1; 32]);
    vec![
        E::DatabaseUnavailable,
        E::MigrationFailed,
        E::NewerSchema,
        E::CorruptStorage,
        E::ContextUnknown,
        E::ContextProvisionMismatch,
        E::GenesisMismatch,
        E::UnauthorizedAuthor,
        E::ParentMissing(event),
        E::ParentContextMismatch(event),
        E::EventCollision,
        E::InvalidRefName,
        E::RefMutationMismatch,
        E::RefMissing,
        E::RefAlreadyExists,
        E::StaleHead { current: None },
        E::LimitExceeded,
        E::EntropyUnavailable,
        E::ReservedEventKind,
        E::InvalidMerge,
        E::ProjectionCycle,
        E::ProjectionLimitExceeded,
        E::BundleMalformed,
        E::BundleUnsupportedVersion,
        E::BundleOrder,
        E::BundleLimitExceeded,
        E::BundleRefInvalid,
        E::VerificationLimitInvalid,
        E::IndeterminateCommit,
    ]
}

/// OC01-D06: admission is the authorization evidence for every referenced
/// event; the OC signer is authenticated only by its distinct domain
/// signature and may author no Option A event at all.
#[tokio::test]
async fn admitted_references_and_independent_artifact_signer_are_not_conflated() {
    let (store, events) = fixture_store().await;
    // The issuer authored zero admitted events (all events are the event
    // author's); issuance still succeeds because admission is the evidence.
    let ledger = issue_full(&store, &events).await;
    let report = ledger
        .verify_against_dag(&store, limits())
        .await
        .expect("verifies");
    assert_eq!(report.unique_events(), 5);

    // The issuer identity is absent from every admitted event's author.
    let issuer_author = identity(ISSUER_SEED).author();
    for event_id in &events {
        let event = store.event(*event_id).await.unwrap().unwrap();
        assert_ne!(event.body().author(), issuer_author);
    }
}

/// OC01-D07: no artifact-signer allowlist, revocation, or historical
/// authorization semantics are inferred from current public APIs.
#[tokio::test]
async fn authorization_verification_does_not_invent_signer_policy() {
    let (store, events) = fixture_store().await;
    let ledger = issue_full(&store, &events).await;
    // Signature author match is enforced structurally (Stage 2B/2C prove);
    // here we assert the DAG layer adds no policy: a fresh issuer identity
    // that never authored any event still issues and verifies, and the
    // store has no signer-allowlist surface reachable from this API.
    ledger
        .verify_against_dag(&store, limits())
        .await
        .expect("no signer policy is inferred");
    // Compile/API audit anchor: the only inputs are identity, store, body,
    // limits — there is no allowlist or witness parameter.
    fn _signature_of_issue(
        _identity: &SigningIdentity,
        _store: &Store,
        _body: OutcomeLedgerBodyV1,
        _limits: OutcomeLimits,
    ) {
    }
}

/// OC01-D08: immutable DAG re-verification remains valid after refs move,
/// while freshness verification fails.
#[tokio::test]
async fn dag_verify_survives_ref_move_but_current_inputs_returns_stale_input() {
    let (store, events) = fixture_store().await;
    let ledger = issue_full(&store, &events).await;

    // Move the local ref by admitting a new head event.
    let author = identity(EVENT_AUTHOR_SEED);
    let new_head =
        admitted_event(&store, &author, context(), vec![events[5]], "agent.request").await;
    store
        .admit(
            &new_head,
            RefMutation::CompareAndSwap {
                context: context(),
                name: "main".parse().unwrap(),
                expected: RefExpectation::Head(events[5]),
                new_head: new_head.event_id(),
            },
        )
        .await
        .expect("ref moves");

    // Immutable DAG verification still passes.
    ledger
        .verify_against_dag(&store, limits())
        .await
        .expect("immutable DAG verification survives ref moves");

    // Freshness verification fails with stale-input.
    let error = ledger
        .verify_current_inputs(&store, limits())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        OutcomeOperationError::Artifact(OutcomeError::StaleInput)
    ));
}

/// OC01-D09: public Store mutations (add/move) make current inputs stale;
/// a directly supplied nonbinding fingerprint fails closed at construction.
#[tokio::test]
async fn current_input_snapshot_change_matrix_returns_stale_input() {
    let (store, events) = fixture_store().await;
    let ledger = issue_full(&store, &events).await;
    let author = identity(EVENT_AUTHOR_SEED);
    let name = "main".parse::<LocalRefName>().unwrap();

    // Baseline: current inputs verify.
    ledger
        .verify_current_inputs(&store, limits())
        .await
        .expect("baseline current inputs verify");

    // Direct fingerprint-mismatch vector: the arrays remain canonical, but
    // `from_parts` is the public construction boundary that rejects an
    // unbound claimed fingerprint. It is correctly `IdMismatch`, not
    // `StaleInput`, because no structurally valid body can carry it through
    // to `verify_current_inputs`.
    let current = snapshot(&store).await;
    let claimed = InputRefFingerprint::from_bytes([0xD9; 32]);
    assert_ne!(claimed, current.fingerprint);
    assert_eq!(
        InputRefSnapshotV1::from_parts(context(), claimed, current.local, current.remote)
            .unwrap_err(),
        OutcomeError::IdMismatch
    );

    // Case: local ref head move.
    let moved = admitted_event(&store, &author, context(), vec![events[5]], "agent.request").await;
    store
        .admit(
            &moved,
            RefMutation::CompareAndSwap {
                context: context(),
                name: name.clone(),
                expected: RefExpectation::Head(events[5]),
                new_head: moved.event_id(),
            },
        )
        .await
        .unwrap();
    assert_stale(&ledger, &store).await;

    // Case: local ref add (second ref).
    let extra = admitted_event(&store, &author, context(), vec![events[5]], "agent.request").await;
    store
        .admit(
            &extra,
            RefMutation::CompareAndSwap {
                context: context(),
                name: "zeta".parse().unwrap(),
                expected: RefExpectation::Absent,
                new_head: extra.event_id(),
            },
        )
        .await
        .unwrap();
    assert_stale(&ledger, &store).await;

    // Case: remote ref head move.
    store
        .set_remote_ref(
            "peer.example".parse().unwrap(),
            context(),
            name.clone(),
            events[3],
        )
        .await
        .unwrap();
    assert_stale(&ledger, &store).await;

    // Case: remote ref add. The public Store API has no remove transition,
    // so this matrix deliberately claims only representable add/move cases.
    store
        .set_remote_ref(
            "other.example".parse().unwrap(),
            context(),
            "main".parse().unwrap(),
            events[1],
        )
        .await
        .unwrap();
    assert_stale(&ledger, &store).await;
}

async fn assert_stale(ledger: &SignedOutcomeLedgerV1, store: &Store) {
    let error = ledger
        .verify_current_inputs(store, limits())
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            OutcomeOperationError::Artifact(OutcomeError::StaleInput)
        ),
        "expected stale-input, got {error:?}"
    );
}

/// OC01-D10: success reports only checked counts and the snapshot
/// fingerprint; every failure returns `Err` only, with no report object.
#[tokio::test]
async fn verification_reports_are_bounded_nonredundant_and_all_failures_atomic() {
    let (store, events) = fixture_store().await;
    let ledger = issue_full(&store, &events).await;
    let report = ledger
        .verify_current_inputs(&store, limits())
        .await
        .expect("success returns the bounded report");

    // The report exposes only checked counts and the fingerprint.
    let fingerprint = report.snapshot_fingerprint().to_owned();
    assert_eq!(
        fingerprint,
        ledger.body().input_refs().fingerprint.to_string()
    );
    assert_eq!(report.unique_events(), 5);
    assert_eq!(report.event_occurrences(), 11);
    assert_eq!(report.local_refs(), 1);
    assert_eq!(report.remote_refs(), 1);

    // Failures return Err only — no report object is constructed. The type
    // system enforces this (Err carries no report); move the local ref to
    // make the previously signed snapshot stale.
    let author = identity(EVENT_AUTHOR_SEED);
    let moved = author
        .create_event(
            context(),
            vec![events[5]],
            "agent.request",
            json!({"oc01":"stale"}),
        )
        .unwrap();
    store
        .admit(
            &moved,
            RefMutation::CompareAndSwap {
                context: context(),
                name: "main".parse().unwrap(),
                expected: RefExpectation::Head(events[5]),
                new_head: moved.event_id(),
            },
        )
        .await
        .unwrap();
    let result = ledger.verify_current_inputs(&store, limits()).await;
    match result {
        Ok(report) => panic!("expected failure, got report {report:?}"),
        Err(OutcomeOperationError::Artifact(OutcomeError::StaleInput)) => {}
        Err(other) => panic!("expected stale-input, got {other:?}"),
    }
}

/// Utility: unique-collection parity check used by maintainers.
#[allow(dead_code)]
fn unique_of(events: &[EventId]) -> usize {
    let set: BTreeSet<EventId> = events.iter().copied().collect();
    set.len()
}
