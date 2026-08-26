//! OC-02 schema/tag/configuration tests (matrix rows T01–T04 and the
//! domain half of T06; full T06 `report_id_domain_separation_exact`
//! lands with report assembly in Stage 2H).

use contextmesh_salience::attribution::{
    ATTRIBUTION_CONFIG_HASH_DOMAIN, ATTRIBUTION_REPORT_ID_DOMAIN, AttributionConfigV1,
    CONFIG_HASH_PREFIX, Mechanism, PREREG_SHA256, REPORT_ID_PREFIX, caps, versions,
};
use contextmesh_salience::error::OutcomeError;

#[test]
fn mechanism_enum_is_exact_and_roundtrips() {
    // T01: exactly five variants; round-trip through as_str/from_name.
    let all = [
        Mechanism::M0,
        Mechanism::M1,
        Mechanism::M2,
        Mechanism::M3,
        Mechanism::M4,
    ];
    for m in all {
        assert_eq!(Mechanism::from_name(m.as_str()).unwrap(), m);
    }
    assert_eq!(Mechanism::from_name("M5"), Err(OutcomeError::Malformed));
    assert_eq!(Mechanism::from_name("m0"), Err(OutcomeError::Malformed));
    assert_eq!(Mechanism::from_name(""), Err(OutcomeError::Malformed));
}

#[test]
fn extractor_versions_match_frozen_prereg() {
    // T02: literals equal the frozen prereg strings verbatim.
    assert_eq!(versions::M0, "oc-prototype-m0-v1-compatible");
    assert_eq!(versions::M1, "oc-1-m1n-v1");
    assert_eq!(versions::M2, "oc-2-m2-v1");
    assert_eq!(versions::PRIOR, "oc-3-prior-v1");
    // Frozen caps are the prereg budget values verbatim.
    assert_eq!(caps::SHORTLIST, 32);
    assert_eq!(caps::M3_JUDGE_CALLS_PER_SESSION, 8);
    assert_eq!(caps::M4_SAMPLES_PER_CANDIDATE, 64);
    assert_eq!(caps::M4_JUDGE_CALLS_PER_SESSION, 128);
    // The frozen prereg reference hash is carried verbatim (shape; literal
    // equality is asserted by T10 at report-assembly stage).
    assert_eq!(PREREG_SHA256.len(), 64);
    assert!(PREREG_SHA256.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn config_canonical_bytes_are_deterministic() {
    // T03: two serializations byte-equal; fixed JCS key order.
    let a = AttributionConfigV1::default();
    let b = AttributionConfigV1::default();
    assert_eq!(a.canonical_bytes().unwrap(), b.canonical_bytes().unwrap());
    let s = String::from_utf8(a.canonical_bytes().unwrap()).unwrap();
    let starts = s.find("\"m3_judge_calls_per_session\"").unwrap();
    let mid = s.find("\"m4_judge_calls_per_session\"").unwrap();
    let tail = s.find("\"tokens_per_event\"").unwrap();
    assert!(starts < mid && mid < tail);
}

#[test]
fn config_hash_domain_and_prefix_exact() {
    // T04: domain includes the NUL terminator; typed prefix correct;
    // a different domain produces a different hash (OC-01 P01 pattern).
    let cfg = AttributionConfigV1::default();
    let got = cfg.config_hash().unwrap();
    assert!(got.starts_with(CONFIG_HASH_PREFIX));
    assert_eq!(CONFIG_HASH_PREFIX, "ocattrcfg1_");

    // Domain bytes asserted explicitly (NUL included).
    assert_eq!(
        ATTRIBUTION_CONFIG_HASH_DOMAIN,
        b"oc-02-attr-config-v1\0".as_slice()
    );

    // Undomained hash differs.
    let mut undomained = blake3::Hasher::new();
    undomained.update(cfg.canonical_bytes().unwrap().as_slice());
    let undomained_hex = undomained.finalize().to_hex().to_string();
    assert_ne!(got.trim_start_matches(CONFIG_HASH_PREFIX), undomained_hex);

    // Deviating config fails closed.
    let bad = AttributionConfigV1 {
        shortlist_cap: 33,
        ..Default::default()
    };
    assert!(bad.config_hash().is_err());
}

#[test]
fn report_id_domain_is_frozen() {
    // T06 (domain half only): literal domain with NUL; prefix typed. The
    // full matrix row `report_id_domain_separation_exact` — including
    // report-ID derivation and the derive-key comparison — lands with
    // report assembly (Stage 2H).
    assert_eq!(
        ATTRIBUTION_REPORT_ID_DOMAIN,
        b"oc-02-attr-report-v1\0".as_slice()
    );
    assert_eq!(REPORT_ID_PREFIX, "ocattr1_");
}
