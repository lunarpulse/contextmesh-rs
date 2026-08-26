//! OC-02 mechanism tests — Stage 2B coverage: matrix rows A01–A05 (M0 raw
//! string-overlap core). M1/M2 rows (A06+) land with Stages 2C/2D.

use contextmesh_salience::attribution::{
    AttributionConfigV1, EVIDENCE_FINGERPRINT_PREFIX, EvidenceKind, Mechanism, extract_tokens,
    m0_nominate, versions,
};
use contextmesh_salience::error::OutcomeError;

fn cfg() -> AttributionConfigV1 {
    AttributionConfigV1::default()
}

#[test]
fn m0_exact_overlap_nominates() {
    // A01: lone-carrier — a payload token appears verbatim in the
    // outcome evidence; M0 edge exists with tag+version+config hash.
    let nom = m0_nominate(
        "evt-carrier-1",
        "revenue was 9500000 in Q3 per the ledger",
        "The final answer reports revenue 9500000 for Q3.",
        &["evt-carrier-1"],
        &cfg(),
    )
    .unwrap()
    .expect("overlap must nominate");
    assert_eq!(nom.event, "evt-carrier-1");
    assert_eq!(nom.mechanism.mechanism, Mechanism::M0);
    assert_eq!(nom.mechanism.extractor_version, versions::M0);
    assert_eq!(
        nom.mechanism.extractor_version,
        "oc-prototype-m0-v1-compatible"
    );
    assert!(nom.mechanism.config_hash.starts_with("ocattrcfg1_"));
    assert_eq!(nom.evidence_kind, EvidenceKind::Overlap);
    assert!(
        nom.evidence_fingerprint
            .starts_with(EVIDENCE_FINGERPRINT_PREFIX)
    );

    // A01 negative half: disjoint vocabularies yield no nomination.
    let none = m0_nominate(
        "evt-carrier-1",
        "completely unrelated tokens here",
        "nothing matching at all",
        &["evt-carrier-1"],
        &cfg(),
    )
    .unwrap();
    assert!(none.is_none());
}

#[test]
fn m0_reformat_blind_spot_held() {
    // A02: documented blind spot — "9.5M" vs "9500000" shares no raw
    // token (vectors share ONLY the reformatted value), so M0 must NOT
    // nominate (M1 fixes this in Stage 2C).
    let nom = m0_nominate(
        "evt-reformatted",
        "9.5M",
        "the verified figure is 9500000 exactly",
        &["evt-reformatted"],
        &cfg(),
    )
    .unwrap();
    assert!(nom.is_none(), "M0 must be blind to reformatted values");
}

#[test]
fn m0_token_bounds_enforced() {
    // A03: 256 distinct tokens/event kept, 257th skipped; 1,024-byte
    // token kept, 1,025-byte skipped (recorded, not an error).
    let mk = |n: usize| {
        (0..n)
            .map(|i| format!("tok{i}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let exact = mk(256);
    let (kept, skipped) = extract_tokens(&exact);
    assert_eq!(kept.len(), 256);
    assert_eq!(skipped, 0);

    let over = mk(257);
    let (kept, skipped) = extract_tokens(&over);
    assert_eq!(kept.len(), 256);
    assert_eq!(skipped, 1);

    let ok_token = "x".repeat(1_024);
    let (kept, skipped) = extract_tokens(&ok_token);
    assert_eq!(kept.len(), 1);
    assert_eq!(skipped, 0);

    let long_token = "x".repeat(1_025);
    let (kept, skipped) = extract_tokens(&long_token);
    assert!(kept.is_empty());
    assert_eq!(skipped, 1);

    // Bounds also hold through m0_nominate (no error on oversized input).
    let nom = m0_nominate("evt", &long_token, "any evidence", &["evt"], &cfg()).unwrap();
    assert!(nom.is_none());
}

#[test]
fn m0_outside_ledger_refs_rejected() {
    // A04: nomination domain limited to ledger-referenced events — an
    // event absent from the caller-provided referenced set is rejected
    // with the OC-01 reserved category `UnauthorizedEvent` and no edge.
    let err = m0_nominate(
        "evt-not-in-ledger",
        "revenue 9500000",
        "revenue 9500000 confirmed",
        &["evt-real-1", "evt-real-2"],
        &cfg(),
    )
    .unwrap_err();
    assert_eq!(err, OutcomeError::UnauthorizedEvent);

    // The same event, once referenced, proceeds normally.
    let nom = m0_nominate(
        "evt-real-1",
        "revenue 9500000",
        "revenue 9500000 confirmed",
        &["evt-real-1", "evt-real-2"],
        &cfg(),
    )
    .unwrap();
    assert!(nom.is_some());
}

#[test]
fn m0_deterministic_reproduction() {
    // A05: identical inputs produce byte-identical nominations — compare
    // the derived debug of both runs (includes the fingerprint bytes).
    let a = m0_nominate(
        "evt-x",
        "alpha beta gamma",
        "result mentions beta and omega",
        &["evt-x"],
        &cfg(),
    )
    .unwrap();
    let b = m0_nominate(
        "evt-x",
        "alpha beta gamma",
        "result mentions beta and omega",
        &["evt-x"],
        &cfg(),
    )
    .unwrap();
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    assert!(a.is_some());
}
