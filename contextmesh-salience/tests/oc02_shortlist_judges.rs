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
