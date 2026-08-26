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

#[test]
fn m1_normalized_numeric_nominates() {
    // A06: "9.5M" in the payload equals "9500000" in the evidence —
    // M1 must nominate (the M0 blind spot, now covered).
    use contextmesh_salience::attribution::m1_nominate;
    let (nom, skipped) = m1_nominate(
        "evt-val",
        "market size 9.5M units",
        "the verified figure is 9500000 exactly",
        &["evt-val"],
        &cfg(),
    )
    .unwrap();
    assert!(skipped.is_empty());
    let nom = nom.expect("9.5M ↔ 9500000 must nominate via M1");
    assert_eq!(nom.event, "evt-val");
    assert_eq!(nom.mechanism.mechanism, Mechanism::M1);
    assert_eq!(nom.mechanism.extractor_version, "oc-1-m1n-v1");
    assert_eq!(nom.evidence_kind, EvidenceKind::Normalized);
    assert!(
        nom.evidence_fingerprint
            .starts_with(EVIDENCE_FINGERPRINT_PREFIX)
    );

    // Cross-check the M0 blind spot still holds on the same vectors.
    let m0 = m0_nominate(
        "evt-val",
        "market size 9.5M units",
        "the verified figure is 9500000 exactly",
        &["evt-val"],
        &cfg(),
    )
    .unwrap();
    assert!(m0.is_none(), "M0 stays blind; only M1 catches it");
}

#[test]
fn m1_magnitude_bound() {
    // A07: 10^18 is the inclusive bound; 10^18+1 normalizes out of
    // range → recorded-skip, no error, no edge.
    use contextmesh_salience::attribution::{
        NUMERIC_MAGNITUDE_LIMIT, NormalizedValue, m1_nominate, parse_normalized,
    };
    assert_eq!(NUMERIC_MAGNITUDE_LIMIT, 1_000_000_000_000_000_000u128);
    assert_eq!(
        parse_normalized("1000000000000000000"),
        Some(NormalizedValue::Number(1_000_000_000_000_000_000))
    );
    // 1000000G = 10^18 exactly — still in range.
    assert_eq!(
        parse_normalized("1000000G"),
        Some(NormalizedValue::Number(1_000_000_000_000_000_000))
    );
    // 10^18 + 1 → out of range.
    assert_eq!(parse_normalized("1000000000000000001"), None);
    // 1000000.1G > 10^18 → out of range.
    assert_eq!(parse_normalized("1000000.1G"), None);

    // Recorded-skip behavior through m1_nominate: the oversized token is
    // reported in `skipped`, nomination proceeds without error.
    let (nom, skipped) = m1_nominate(
        "evt-huge",
        "value 1000000000000000001 also 9.5M",
        "figure 9500000 confirmed",
        &["evt-huge"],
        &cfg(),
    )
    .unwrap();
    assert!(nom.is_some(), "the in-range 9.5M still nominates");
    assert_eq!(skipped, vec!["1000000000000000001".to_string()]);

    // Over the bound with no in-range match: no nomination, skip only.
    let (nom, skipped) = m1_nominate(
        "evt-edge",
        "1000000000000000001",
        "nothing numeric here at all",
        &["evt-edge"],
        &cfg(),
    )
    .unwrap();
    assert!(nom.is_none());
    assert_eq!(skipped.len(), 1);
}

#[test]
fn m1_folding_rules() {
    // A08: case/whitespace/path folding is deterministic — a fixture
    // set of fold pairs must compare equal after normalization.
    use contextmesh_salience::attribution::{NormalizedValue, fold_path, parse_normalized};
    // Unit-suffix case folding: 9.5M == 9.5m == 9500000.
    assert_eq!(
        parse_normalized("9.5M"),
        Some(NormalizedValue::Number(9_500_000))
    );
    assert_eq!(
        parse_normalized("9.5m"),
        Some(NormalizedValue::Number(9_500_000))
    );
    assert_eq!(
        parse_normalized("9500000"),
        Some(NormalizedValue::Number(9_500_000))
    );
    // Percent: "9.5%" == "950bps" canonical.
    assert_eq!(
        parse_normalized("9.5%"),
        Some(NormalizedValue::Percent(950))
    );
    assert_eq!(
        parse_normalized("9.5%").map(|v| v.canonical()),
        Some("950bps".to_string())
    );
    // Path folding: duplicate + trailing slashes and case collapse.
    assert_eq!(fold_path("/A//b/"), "/a/b");
    assert_eq!(fold_path("/a/b"), "/a/b");
    assert_eq!(fold_path("/a/b///c//"), "/a/b/c");
    assert_eq!(
        parse_normalized("/A//b/"),
        Some(NormalizedValue::Path("/a/b".into()))
    );
    assert_eq!(
        parse_normalized("/a/b"),
        Some(NormalizedValue::Path("/a/b".into()))
    );
    // Determinism: folding is a pure function of the token.
    for t in ["/A//b/", "9.5M", "9.5%", "12.5k"] {
        let a = parse_normalized(t);
        let b = parse_normalized(t);
        assert_eq!(a, b);
    }
    // Non-numeric words stay None (no false normalization).
    assert_eq!(parse_normalized("alpha"), None);
    assert_eq!(parse_normalized("M"), None);
    assert_eq!(parse_normalized("-5"), None);
    assert_eq!(parse_normalized("1e9"), None);
}
