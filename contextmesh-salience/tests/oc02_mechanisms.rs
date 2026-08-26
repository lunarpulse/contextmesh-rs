//! OC-02 mechanism tests — Stage 2B coverage: matrix rows A01–A05 (M0 raw
//! string-overlap core). M1/M2 rows (A06+) land with Stages 2C/2D.

use contextmesh_salience::attribution::{
    AttributionConfigV1, EVIDENCE_FINGERPRINT_PREFIX, EvidenceKind, Mechanism, extract_tokens,
    m0_nominate, m2_nominate, versions,
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

// ---- Stage 2D: M2 explicit structural extractor (A09–A17) ----

/// Deterministic canonical EventId-shaped string for tests (valid shape:
/// `evt1_` + 43 base64url chars). Same trick for rcpt/ocout.
fn eid(n: u8) -> String {
    format!("evt1_{}{}", "A".repeat(42), n)
}
fn rid(n: u8) -> String {
    format!("rcpt1_{}{}", "B".repeat(42), n)
}
fn aid(n: u8) -> String {
    format!("ocout1_{}{}", "C".repeat(42), n)
}

#[test]
fn m2_eventid_citation_positive() {
    // A09: a canonical EventId present in the referenced universe,
    // appearing in the payload → citation edge.
    use contextmesh_salience::attribution::{M2Structure, m2_extract, m2_nominate};
    let cited = eid(1);
    let ext = m2_extract(
        &format!("analysis based on {cited} and prior work"),
        &[cited.as_str()],
        &[],
        &[],
    );
    assert_eq!(ext.forged, Vec::<String>::new());
    assert_eq!(
        ext.structures,
        vec![M2Structure::EventIdCitation(cited.clone())]
    );
    let edges = m2_nominate("evt-a", &ext, &cfg()).unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].mechanism.mechanism, Mechanism::M2);
    assert_eq!(edges[0].mechanism.extractor_version, "oc-2-m2-v1");
    assert_eq!(edges[0].evidence_kind, EvidenceKind::Citation);
}

#[test]
fn m2_forged_link_negative() {
    // A10: a canonical-shaped EventId NOT in the referenced universe is
    // recorded as forged, rejected, and produces no edge.
    use contextmesh_salience::attribution::{m2_extract, m2_nominate};
    let real = eid(1);
    let fake = eid(9);
    let ext = m2_extract(
        &format!("see {real} and also {fake}"),
        &[real.as_str()],
        &[],
        &[],
    );
    assert_eq!(ext.forged, vec![fake.clone()]);
    // Only the real citation became a structure; the forged one did not.
    assert_eq!(ext.structures.len(), 1);
    let edges = m2_nominate("evt-b", &ext, &cfg()).unwrap();
    assert_eq!(edges.len(), 1); // forged → no edge
}

#[test]
fn m2_provider_linkage_positive() {
    // A11: linkage comes ONLY from core public metadata pairs, never
    // from prose. Prose mention of ids yields no linkage.
    use contextmesh_salience::attribution::{M2Structure, m2_extract};
    // Prose-only: no metadata pairs → no linkage structure.
    let ext = m2_extract("request req-42 produced result res-99", &[], &[], &[]);
    assert!(
        ext.structures
            .iter()
            .all(|s| !matches!(s, M2Structure::ProviderLinkage { .. }))
    );

    // Metadata pair supplied → exactly one linkage structure.
    let ext = m2_extract("irrelevant prose", &[], &[("req-42", "res-99")], &[]);
    assert_eq!(
        ext.structures,
        vec![M2Structure::ProviderLinkage {
            request_id: "req-42".into(),
            result_id: "res-99".into()
        }]
    );
}

#[test]
fn m2_receipt_reference_positive() {
    // A12: Option B receipt reference (`rcpt1_…`) → receipt edge.
    use contextmesh_salience::attribution::{M2Structure, m2_extract, m2_nominate};
    let receipt = rid(7);
    let ext = m2_extract(&format!("handed off per {receipt}"), &[], &[], &[]);
    assert_eq!(
        ext.structures,
        vec![M2Structure::ReceiptReference(receipt.clone())]
    );
    let edges = m2_nominate("evt-c", &ext, &cfg()).unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].evidence_kind, EvidenceKind::Receipt);
}

#[test]
fn m2_summary_coverage_positive() {
    // A13: enumeration covering listed (referenced) events → summary.
    use contextmesh_salience::attribution::{M2Structure, m2_extract};
    let e1 = eid(1);
    let e2 = eid(2);
    let ext = m2_extract(
        "summary covers:",
        &[e1.as_str(), e2.as_str()],
        &[],
        &[e1.as_str(), e2.as_str()],
    );
    assert_eq!(
        ext.structures,
        vec![M2Structure::SummaryCoverage(vec![e1.clone(), e2.clone()])]
    );

    // Negative: an enumeration naming a non-referenced event records
    // nothing (all-entries-must-be-referenced rule).
    let ghost = eid(3);
    let ext = m2_extract("", &[e1.as_str()], &[], &[e1.as_str(), ghost.as_str()]);
    assert!(ext.structures.is_empty());
}

#[test]
fn m2_signed_artifact_reference_positive() {
    // A14: `ocout1_…` reference → artifact edge.
    use contextmesh_salience::attribution::{M2Structure, m2_extract, m2_nominate};
    let artifact = aid(5);
    let ext = m2_extract(&format!("output recorded as {artifact}"), &[], &[], &[]);
    assert_eq!(
        ext.structures,
        vec![M2Structure::ArtifactReference(artifact.clone())]
    );
    let edges = m2_nominate("evt-d", &ext, &cfg()).unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].evidence_kind, EvidenceKind::Artifact);
}

#[test]
fn m2_exactly_five_structures() {
    // A15: exactly the five structures recognize; near-miss text does
    // not. Five positives + paraphrase/citation-like negatives.
    use contextmesh_salience::attribution::{canonical_id_kind, m2_extract};
    let e1 = eid(1);
    // Positive pass: all five in one extraction.
    let ext = m2_extract(
        &format!("{} {} {}", e1, rid(2), aid(3)),
        &[e1.as_str()],
        &[("rq", "rs")],
        &[e1.as_str()],
    );
    assert_eq!(ext.structures.len(), 5);
    let kinds: Vec<_> = ext.structures.iter().map(|s| s.evidence_kind()).collect();
    for k in [
        EvidenceKind::Citation,
        EvidenceKind::Linkage,
        EvidenceKind::Receipt,
        EvidenceKind::Summary,
        EvidenceKind::Artifact,
    ] {
        assert!(kinds.contains(&k), "missing kind {k:?}");
    }

    // Near-miss negatives: prose wrapping, wrong length, wrong prefix.
    assert_eq!(canonical_id_kind(&format!("cites {e1}")), None);
    assert_eq!(canonical_id_kind("evt1_short"), None);
    assert_eq!(canonical_id_kind("evv1_AAAA"), None);
    assert_eq!(canonical_id_kind(&format!("{}x", e1)), None);
    // Paraphrased citation prose recognizes nothing.
    let ext = m2_extract(
        "the analysis cites the earlier event informally",
        &[],
        &[],
        &[],
    );
    assert!(ext.structures.is_empty());
    assert!(ext.forged.is_empty());
}

#[test]
fn m2_provenance_on_every_edge() {
    // A16: every edge carries extractor identity, version, config hash.
    use contextmesh_salience::attribution::m2_extract;
    let e1 = eid(1);
    let ext = m2_extract(
        &format!("{e1} {} {}", rid(2), aid(3)),
        &[e1.as_str()],
        &[("rq", "rs")],
        &[e1.as_str()],
    );
    let edges = m2_nominate("evt-e", &ext, &cfg()).unwrap();
    assert_eq!(edges.len(), 5);
    for edge in &edges {
        assert_eq!(edge.mechanism.mechanism, Mechanism::M2);
        assert_eq!(edge.mechanism.extractor_version, "oc-2-m2-v1");
        assert!(edge.mechanism.config_hash.starts_with("ocattrcfg1_"));
        assert!(
            edge.evidence_fingerprint
                .starts_with(EVIDENCE_FINGERPRINT_PREFIX)
        );
    }
}

#[test]
fn m2_no_rederivation_during_verify() {
    // A17: the verify path reads recorded edges only — m2_nominate
    // consumes a stored extraction and never re-scans payload text
    // (it takes no payload argument at all). D-C-07: LLM-inferred
    // citations are a future adapter; nothing re-derives them here.
    use contextmesh_salience::attribution::m2_extract;
    let e1 = eid(1);
    let ext = m2_extract(&format!("based on {e1}"), &[e1.as_str()], &[], &[]);
    // "Verify": rebuild edges from the RECORDED extraction only.
    let first = m2_nominate("evt-f", &ext, &cfg()).unwrap();
    let second = m2_nominate("evt-f", &ext, &cfg()).unwrap();
    assert_eq!(first.len(), second.len());
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }
    // The nomination API surface itself cannot re-derive: no payload
    // parameter exists to re-scan (compile-time guarantee).
}
