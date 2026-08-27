//! OC-02 Stage 2E shortlist policy tests — matrix rows S01–S08.

use contextmesh_salience::attribution::{
    AttributionConfigV1, AttributionMechanismTag, CausalStatus, EvidenceKind, M0Nomination,
    Mechanism, NOMINATION_SCORE_PPM, RecallBasisV1, ShortlistEntryV1, ShortlistV1, build_shortlist,
    evidence_fingerprint, versions,
};
use contextmesh_salience::error::OutcomeError;

fn cfg() -> AttributionConfigV1 {
    AttributionConfigV1::default()
}

fn eid(suffix: char) -> String {
    format!("evt1_{}{}", "A".repeat(42), suffix)
}

fn event_ids(count: usize) -> Vec<String> {
    const SUFFIXES: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";
    SUFFIXES
        .iter()
        .take(count)
        .map(|b| eid(char::from(*b)))
        .collect()
}

fn nomination(event: &str, mechanism: Mechanism) -> M0Nomination {
    let (extractor_version, evidence_kind) = match mechanism {
        Mechanism::M0 => (versions::M0, EvidenceKind::Overlap),
        Mechanism::M1 => (versions::M1, EvidenceKind::Normalized),
        Mechanism::M2 => (versions::M2, EvidenceKind::Citation),
        Mechanism::M3 | Mechanism::M4 => ("future-adapter", EvidenceKind::Citation),
    };
    M0Nomination {
        event: event.to_string(),
        mechanism: AttributionMechanismTag {
            mechanism,
            extractor_version,
            config_hash: cfg().config_hash().expect("frozen config hashes"),
        },
        evidence_kind,
        evidence_fingerprint: evidence_fingerprint(b"test evidence"),
    }
}

fn refs(ids: &[String]) -> Vec<&str> {
    ids.iter().map(String::as_str).collect()
}

#[test]
fn shortlist_union_dedup() {
    let event = eid('a');
    let nominations = vec![
        nomination(&event, Mechanism::M2),
        nomination(&event, Mechanism::M0),
        nomination(&event, Mechanism::M1),
        nomination(&event, Mechanism::M0),
    ];
    let shortlist = build_shortlist(&nominations, &[event.as_str()], &cfg()).unwrap();

    assert_eq!(shortlist.entries.len(), 1);
    assert_eq!(shortlist.entries[0].event, event);
    assert_eq!(
        shortlist.entries[0].nominating_mechanisms,
        vec![Mechanism::M0, Mechanism::M1, Mechanism::M2]
    );
    assert_eq!(shortlist.entries[0].rank, 1);
    assert_eq!(shortlist.entries[0].score_ppm, NOMINATION_SCORE_PPM);
}

#[test]
fn shortlist_cap_boundaries() {
    for count in [0usize, 32] {
        let ids = event_ids(count);
        let nominations: Vec<_> = ids.iter().map(|id| nomination(id, Mechanism::M0)).collect();
        let shortlist = build_shortlist(&nominations, &refs(&ids), &cfg()).unwrap();
        assert_eq!(shortlist.entries.len(), count);
        assert_eq!(shortlist.overflow_count().unwrap(), 0);
    }

    let mut ids = event_ids(33);
    ids.reverse();
    let nominations: Vec<_> = ids.iter().map(|id| nomination(id, Mechanism::M0)).collect();
    let shortlist = build_shortlist(&nominations, &refs(&ids), &cfg()).unwrap();
    let mut expected = ids.clone();
    expected.sort();
    expected.truncate(32);
    assert_eq!(shortlist.entries.len(), 32);
    assert_eq!(shortlist.overflow_count().unwrap(), 1);
    assert_eq!(
        shortlist
            .entries
            .iter()
            .map(|entry| entry.event.clone())
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn shortlist_order_deterministic() {
    let lower = eid('A');
    let higher = eid('z');
    let nominations = vec![
        nomination(&higher, Mechanism::M1),
        nomination(&lower, Mechanism::M2),
    ];
    let shortlist =
        build_shortlist(&nominations, &[higher.as_str(), lower.as_str()], &cfg()).unwrap();

    assert_eq!(shortlist.entries[0].event, lower);
    assert_eq!(shortlist.entries[1].event, higher);
    assert_eq!(shortlist.entries[0].score_ppm, 1_000_000);
    assert_eq!(shortlist.entries[1].score_ppm, 1_000_000);
    assert_eq!(shortlist.entries[0].rank, 1);
    assert_eq!(shortlist.entries[1].rank, 2);
}

#[test]
fn shortlist_empty_recorded() {
    let shortlist = build_shortlist(&[], &[], &cfg()).unwrap();
    assert!(shortlist.entries.is_empty());
    assert_eq!(
        shortlist.causal_status_marker().unwrap(),
        Some(CausalStatus::NoNominations)
    );
    assert_eq!(
        shortlist.causal_status_marker().unwrap().unwrap().as_str(),
        "no_nominations"
    );
}

#[test]
fn shortlist_recall_recorded_separately() {
    let eligible = event_ids(40);
    let nominations: Vec<_> = eligible
        .iter()
        .take(33)
        .map(|id| nomination(id, Mechanism::M0))
        .collect();
    let shortlist = build_shortlist(&nominations, &refs(&eligible), &cfg()).unwrap();

    assert_eq!(shortlist.entries.len(), 32);
    assert_eq!(shortlist.recall_basis.nominated, 33);
    assert_eq!(shortlist.recall_basis.eligible, 40);
    assert_eq!(shortlist.overflow_count().unwrap(), 1);
}

#[test]
fn shortlist_arithmetic_checked() {
    let invalid = ShortlistV1 {
        entries: vec![ShortlistEntryV1 {
            event: eid('a'),
            rank: 1,
            nominating_mechanisms: vec![Mechanism::M0],
            score_ppm: NOMINATION_SCORE_PPM,
        }],
        cap: 32,
        dedup: "EventId",
        order: "score_ppm desc, EventId asc",
        recall_basis: RecallBasisV1 {
            nominated: 0,
            eligible: u128::MAX,
        },
    };
    assert_eq!(invalid.overflow_count(), Err(OutcomeError::Malformed));
    assert_eq!(invalid.causal_status_marker(), Err(OutcomeError::Malformed));

    // Impossible empty/pre-cap state and the inherited eligible bound
    // fail before producing authoritative bytes or markers.
    let impossible_empty = ShortlistV1 {
        entries: vec![],
        cap: 32,
        dedup: "EventId",
        order: "score_ppm desc, EventId asc",
        recall_basis: RecallBasisV1 {
            nominated: u128::MAX,
            eligible: u128::MAX,
        },
    };
    assert_eq!(
        impossible_empty.canonical_bytes(),
        Err(OutcomeError::Malformed)
    );
    assert_eq!(
        impossible_empty.causal_status_marker(),
        Err(OutcomeError::Malformed)
    );

    let ids = event_ids(33);
    let nominations: Vec<_> = ids.iter().map(|id| nomination(id, Mechanism::M2)).collect();
    let shortlist = build_shortlist(&nominations, &refs(&ids), &cfg()).unwrap();
    let nominated: u128 = shortlist.recall_basis.nominated;
    let retained = u128::try_from(shortlist.entries.len()).unwrap();
    assert_eq!(nominated.checked_sub(retained), Some(1));
}

#[test]
fn shortlist_domain_purity() {
    let real = eid('r');
    let foreign = eid('f');
    let nominations = vec![
        nomination(&foreign, Mechanism::M2),
        nomination(&real, Mechanism::M0),
    ];
    let shortlist = build_shortlist(&nominations, &[real.as_str()], &cfg()).unwrap();

    assert_eq!(shortlist.entries.len(), 1);
    assert_eq!(shortlist.entries[0].event, real);
    assert!(!shortlist.entries.iter().any(|entry| entry.event == foreign));

    let malformed = build_shortlist(&[], &["not-an-event-id"], &cfg());
    assert_eq!(malformed, Err(OutcomeError::Malformed));
}

#[test]
fn shortlist_rejects_invalid_nomination_provenance() {
    let event = eid('p');
    let referenced = [event.as_str()];

    let mut wrong_version = nomination(&event, Mechanism::M0);
    wrong_version.mechanism.extractor_version = versions::M1;
    assert_eq!(
        build_shortlist(&[wrong_version], &referenced, &cfg()),
        Err(OutcomeError::Malformed)
    );

    let mut wrong_config = nomination(&event, Mechanism::M1);
    wrong_config.mechanism.config_hash = "ocattrcfg1_forged".into();
    assert_eq!(
        build_shortlist(&[wrong_config], &referenced, &cfg()),
        Err(OutcomeError::Malformed)
    );

    let mut wrong_kind = nomination(&event, Mechanism::M2);
    wrong_kind.evidence_kind = EvidenceKind::Overlap;
    assert_eq!(
        build_shortlist(&[wrong_kind], &referenced, &cfg()),
        Err(OutcomeError::Malformed)
    );

    let mut wrong_fingerprint = nomination(&event, Mechanism::M0);
    wrong_fingerprint.evidence_fingerprint = "ocfp1_not-a-blake3-hash".into();
    assert_eq!(
        build_shortlist(&[wrong_fingerprint], &referenced, &cfg()),
        Err(OutcomeError::Malformed)
    );
}

#[test]
fn shortlist_byte_reproduction() {
    let a = eid('a');
    let b = eid('b');
    let nominations = vec![
        nomination(&b, Mechanism::M2),
        nomination(&a, Mechanism::M1),
        nomination(&a, Mechanism::M0),
    ];
    let reversed: Vec<_> = nominations.iter().cloned().rev().collect();
    let referenced = [a.as_str(), b.as_str()];
    let first = build_shortlist(&nominations, &referenced, &cfg()).unwrap();
    let second = build_shortlist(&reversed, &referenced, &cfg()).unwrap();

    let first_bytes = first.canonical_bytes().unwrap();
    assert_eq!(first_bytes, second.canonical_bytes().unwrap());
    assert_eq!(
        String::from_utf8(first_bytes).unwrap(),
        format!(
            "{{\"cap\":32,\"dedup\":\"EventId\",\"entries\":[{{\"event\":\"{a}\",\"nominating_mechanisms\":[\"M0\",\"M1\"],\"rank\":1,\"score_ppm\":{NOMINATION_SCORE_PPM}}},{{\"event\":\"{b}\",\"nominating_mechanisms\":[\"M2\"],\"rank\":2,\"score_ppm\":{NOMINATION_SCORE_PPM}}}],\"order\":\"score_ppm desc, EventId asc\",\"recall_basis\":{{\"eligible\":2,\"nominated\":2}}}}"
        )
    );

    // Referenced-universe permutation and duplicates cannot alter bytes
    // or inflate the eligible count.
    let permuted_referenced = [b.as_str(), a.as_str(), b.as_str()];
    let third = build_shortlist(&nominations, &permuted_referenced, &cfg()).unwrap();
    assert_eq!(
        first.canonical_bytes().unwrap(),
        third.canonical_bytes().unwrap()
    );
    assert_eq!(third.recall_basis.eligible, 2);
}

use std::cell::{Cell, RefCell};

use contextmesh::model::{ContextId, EventId};
use contextmesh_salience::judge::{
    AblationDeltaV1, AblationRequestV1, AttributionSessionKeyV1, JudgeUnavailable, M3AdapterStatus,
    M3DeltaKind, M3UncertaintyMarker, OutcomeJudge, run_m3,
};
use contextmesh_salience::types::{Blake3HashText, MechanismRecordV1, OutcomeId, OutcomeLimits};

fn typed_event(index: u8) -> EventId {
    EventId::from_bytes([index; 32])
}

fn typed_shortlist(count: usize) -> ShortlistV1 {
    let ids: Vec<String> = (0..count)
        .map(|index| typed_event(u8::try_from(index + 1).unwrap()).to_string())
        .collect();
    let nominations: Vec<_> = ids.iter().map(|id| nomination(id, Mechanism::M0)).collect();
    build_shortlist(&nominations, &refs(&ids), &cfg()).unwrap()
}

fn session(outcome_byte: u8, context_byte: u8) -> AttributionSessionKeyV1 {
    AttributionSessionKeyV1 {
        outcome: OutcomeId::from_bytes([outcome_byte; 32]),
        context: ContextId::from_bytes([context_byte; 32]),
    }
}

fn judge_record(identity: &str, version: &str, hash_byte: u8) -> MechanismRecordV1 {
    MechanismRecordV1::new(
        identity.to_owned(),
        version.to_owned(),
        Blake3HashText::from_digest([hash_byte; 32]),
        &OutcomeLimits::default(),
    )
    .unwrap()
}

struct SpyJudge {
    identity: MechanismRecordV1,
    requests: RefCell<Vec<(AttributionSessionKeyV1, EventId)>>,
    unavailable_at: Option<usize>,
}

impl SpyJudge {
    fn available(identity: MechanismRecordV1) -> Self {
        Self {
            identity,
            requests: RefCell::new(Vec::new()),
            unavailable_at: None,
        }
    }

    fn unavailable_at(identity: MechanismRecordV1, call: usize) -> Self {
        Self {
            identity,
            requests: RefCell::new(Vec::new()),
            unavailable_at: Some(call),
        }
    }
}

impl OutcomeJudge for SpyJudge {
    fn identity(&self) -> MechanismRecordV1 {
        self.identity.clone()
    }

    fn ablate(&self, req: AblationRequestV1<'_>) -> Result<AblationDeltaV1, JudgeUnavailable> {
        let call = self.requests.borrow().len();
        self.requests
            .borrow_mut()
            .push((req.session().clone(), req.event()));
        if self.unavailable_at == Some(call) {
            Err(JudgeUnavailable)
        } else if call.is_multiple_of(2) {
            Ok(AblationDeltaV1::Changed)
        } else {
            Ok(AblationDeltaV1::Unchanged)
        }
    }
}

#[test]
fn m3_shortlist_only_execution() {
    let shortlist = typed_shortlist(3);
    let before = shortlist.canonical_bytes().unwrap();
    let session = session(1, 2);
    let judge = SpyJudge::available(judge_record("judge-spy", "1", 3));

    let section = run_m3(&session, &shortlist, Some(&judge), &cfg()).unwrap();

    let allowed: Vec<EventId> = shortlist
        .entries
        .iter()
        .map(|entry| entry.event.parse().unwrap())
        .collect();
    assert_eq!(section.judge_calls(), 3);
    assert!(
        judge
            .requests
            .borrow()
            .iter()
            .all(|(seen_session, event)| seen_session == &session && allowed.contains(event))
    );
    assert_eq!(shortlist.canonical_bytes().unwrap(), before);
}

#[test]
fn m3_call_cap_boundaries() {
    for count in [0usize, 8] {
        let shortlist = typed_shortlist(count);
        let judge = SpyJudge::available(judge_record("cap-judge", "1", 4));
        let section = run_m3(&session(2, 3), &shortlist, Some(&judge), &cfg()).unwrap();
        assert_eq!(section.judge_calls(), count);
        assert_eq!(judge.requests.borrow().len(), count);
        assert_eq!(section.failure(), None);
        assert!(section.uncertainty_markers().is_empty());
        assert_eq!(
            section.status(),
            if count == 0 {
                M3AdapterStatus::NoNominations
            } else {
                M3AdapterStatus::Complete
            }
        );
    }

    let shortlist = typed_shortlist(9);
    let judge = SpyJudge::available(judge_record("cap-judge", "1", 4));
    let section = run_m3(&session(2, 3), &shortlist, Some(&judge), &cfg()).unwrap();
    assert_eq!(section.judge_calls(), 8);
    assert_eq!(judge.requests.borrow().len(), 8);
    assert_eq!(section.status(), M3AdapterStatus::Unavailable);
    assert_eq!(section.failure(), Some(OutcomeError::MechanismUnavailable));
    assert_eq!(
        section.uncertainty_markers(),
        vec![M3UncertaintyMarker::M3CallCap]
    );
    assert_eq!(M3UncertaintyMarker::M3CallCap.as_str(), "m3_call_cap");
    assert_eq!(section.m3()[8].delta_kind(), M3DeltaKind::Unavailable);
    let expected_events: Vec<EventId> = shortlist
        .entries
        .iter()
        .map(|entry| entry.event.parse().unwrap())
        .collect();
    assert_eq!(
        section
            .m3()
            .iter()
            .map(|delta| delta.event())
            .collect::<Vec<_>>(),
        expected_events
    );
}

#[test]
fn m3_call_provenance() {
    let shortlist = typed_shortlist(2);
    let identity = judge_record("explicit-judge", "v7", 5);
    let judge = SpyJudge::available(identity.clone());
    let section = run_m3(&session(3, 4), &shortlist, Some(&judge), &cfg()).unwrap();

    for delta in section.m3() {
        assert_eq!(delta.judge(), identity.identity.as_str());
        assert_eq!(delta.judge_version(), identity.version.as_str());
        assert_eq!(delta.judge_config_hash(), &identity.config_hash);
    }
}

#[test]
fn judge_none_fail_closed() {
    let shortlist = typed_shortlist(2);
    let before = shortlist.canonical_bytes().unwrap();
    let section = run_m3(&session(4, 5), &shortlist, None, &cfg()).unwrap();

    assert_eq!(section.status(), M3AdapterStatus::Unavailable);
    assert_eq!(section.failure(), Some(OutcomeError::MechanismUnavailable));
    assert_eq!(section.judge_calls(), 0);
    assert_eq!(
        section.uncertainty_markers(),
        vec![M3UncertaintyMarker::JudgeUnavailable]
    );
    assert_eq!(
        M3UncertaintyMarker::JudgeUnavailable.as_str(),
        "judge_unavailable"
    );
    assert!(section.m3().is_empty());
    assert_eq!(shortlist.canonical_bytes().unwrap(), before);
}

#[test]
fn judge_unavailable_midrun() {
    let shortlist = typed_shortlist(4);
    let identity = judge_record("flaky-judge", "2", 6);
    let judge = SpyJudge::unavailable_at(identity.clone(), 2);
    let section = run_m3(&session(5, 6), &shortlist, Some(&judge), &cfg()).unwrap();

    assert_eq!(section.judge_calls(), 3);
    assert_eq!(judge.requests.borrow().len(), 3);
    assert_eq!(section.status(), M3AdapterStatus::Unavailable);
    assert_eq!(section.failure(), Some(OutcomeError::MechanismUnavailable));
    assert_eq!(
        section.uncertainty_markers(),
        vec![M3UncertaintyMarker::JudgeUnavailable]
    );
    assert_eq!(section.m3()[0].delta_kind(), M3DeltaKind::Changed);
    assert_eq!(section.m3()[1].delta_kind(), M3DeltaKind::Unchanged);
    assert!(
        section.m3()[2..]
            .iter()
            .all(|delta| delta.delta_kind() == M3DeltaKind::Unavailable)
    );
    assert!(section.m3().iter().all(|delta| {
        delta.judge() == identity.identity
            && delta.judge_version() == identity.version
            && delta.judge_config_hash() == &identity.config_hash
    }));
    let expected_events: Vec<EventId> = shortlist
        .entries
        .iter()
        .map(|entry| entry.event.parse().unwrap())
        .collect();
    assert_eq!(
        section
            .m3()
            .iter()
            .map(|delta| delta.event())
            .collect::<Vec<_>>(),
        expected_events
    );
}

#[test]
fn unavailable_no_causal_vocabulary() {
    let shortlist = typed_shortlist(1);
    let section = run_m3(&session(6, 7), &shortlist, None, &cfg()).unwrap();

    assert_eq!(section.status().as_str(), "unavailable");
    assert_eq!(
        section.failure().unwrap().stable_category(),
        "mechanism-unavailable"
    );
    assert_eq!(
        section.uncertainty_markers()[0].as_str(),
        "judge_unavailable"
    );
    let typed_surface = format!(
        "{} {} {}",
        section.status().as_str(),
        section.failure().unwrap().stable_category(),
        section.uncertainty_markers()[0].as_str()
    );
    for causal_claim in ["caused", "because", "credit", "load-bearing"] {
        assert!(!typed_surface.contains(causal_claim));
    }
}

struct IdentityCountingJudge {
    identities: Cell<usize>,
    calls: Cell<usize>,
    identity: MechanismRecordV1,
}

impl OutcomeJudge for IdentityCountingJudge {
    fn identity(&self) -> MechanismRecordV1 {
        self.identities.set(self.identities.get() + 1);
        self.identity.clone()
    }

    fn ablate(&self, _req: AblationRequestV1<'_>) -> Result<AblationDeltaV1, JudgeUnavailable> {
        self.calls.set(self.calls.get() + 1);
        Ok(AblationDeltaV1::Changed)
    }
}

#[test]
fn judge_identity_recorded_not_inferred() {
    let identity = judge_record("returned-by-identity-method", "exact-version", 7);
    let judge = IdentityCountingJudge {
        identities: Cell::new(0),
        calls: Cell::new(0),
        identity: identity.clone(),
    };
    let section = run_m3(&session(7, 8), &typed_shortlist(1), Some(&judge), &cfg()).unwrap();

    assert_eq!(judge.identities.get(), 1);
    assert_eq!(judge.calls.get(), 1);
    assert_eq!(section.m3()[0].judge(), identity.identity);
    assert_eq!(section.m3()[0].judge_version(), identity.version);
    assert_eq!(section.m3()[0].judge_config_hash(), &identity.config_hash);

    // A malformed identity is rejected before any ablation call and no
    // partial authoritative section is returned.
    let mut malformed_identity = judge_record("valid-first", "1", 9);
    malformed_identity.identity.clear();
    let malformed_judge = IdentityCountingJudge {
        identities: Cell::new(0),
        calls: Cell::new(0),
        identity: malformed_identity,
    };
    let error = run_m3(
        &session(7, 8),
        &typed_shortlist(1),
        Some(&malformed_judge),
        &cfg(),
    )
    .unwrap_err();
    assert_eq!(error, OutcomeError::Malformed);
    assert_eq!(malformed_judge.identities.get(), 1);
    assert_eq!(malformed_judge.calls.get(), 0);
}

#[test]
fn caps_counted_per_session_definition() {
    let shortlist = typed_shortlist(9);
    let judge = SpyJudge::available(judge_record("session-judge", "1", 8));
    let first_key = session(8, 9);
    let second_key = session(10, 11);

    let first = run_m3(&first_key, &shortlist, Some(&judge), &cfg()).unwrap();
    let second = run_m3(&second_key, &shortlist, Some(&judge), &cfg()).unwrap();

    assert_eq!(first.judge_calls(), 8);
    assert_eq!(second.judge_calls(), 8);
    let requests = judge.requests.borrow();
    assert_eq!(requests.len(), 16);
    assert_eq!(
        requests.iter().filter(|(key, _)| key == &first_key).count(),
        8
    );
    assert_eq!(
        requests
            .iter()
            .filter(|(key, _)| key == &second_key)
            .count(),
        8
    );
}
