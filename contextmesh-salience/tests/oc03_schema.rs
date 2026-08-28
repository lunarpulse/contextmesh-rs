//! OC-03 Stage 3B: prior schema tests (matrix rows T01–T08).
//!
//! T08 renders a minimal `SaliencePriorV1` canonically and parses it back
//! with `serde_json` to assert the exact 13-member §7.4 top-level set.

use base64::Engine as _;
use contextmesh_salience::prior::{
    CONFIG_HASH_PREFIX, PREREG_SHA256, PRIOR_CONFIG_HASH_DOMAIN, PRIOR_ID_DOMAIN, PRIOR_ID_PREFIX,
    PriorConfigV1, SaliencePriorV1, caps, versions,
};

#[test]
fn prior_version_wire() {
    // T01: constant round-trips as frozen.
    assert_eq!(versions::PRIOR, "oc-3-prior-v1");
    // Wire type sanity: a &'static str usable in tags.
    let wire: &'static str = versions::PRIOR;
    assert_eq!(wire.len(), 13);
}

#[test]
fn thorn_status_marker() {
    // T02: marker literal round-trips; no Thorn type is reachable from
    // this module (no Thorn type exists in the crate at all).
    assert_eq!(versions::THORN_STATUS, "thorn_disabled");
    let marker: &'static str = versions::THORN_STATUS;
    assert_eq!(marker.len(), 14);
}

#[test]
fn caps_frozen_literals() {
    // T03: 1024/32/8/64/64/1e6/850000/1e9 asserted exactly.
    assert_eq!(caps::MAX_ENTITIES, 1024);
    assert_eq!(caps::MAX_EDGES_PER_ENTITY, 32);
    assert_eq!(caps::ENTITIES_PER_EVENT, 8);
    assert_eq!(caps::MAX_SEEDS, 64);
    assert_eq!(caps::MAX_ITERATIONS, 64);
    assert_eq!(caps::EPSILON_PPB, 1_000_000u128);
    assert_eq!(caps::DAMPING_PPM, 850_000u128);
    assert_eq!(caps::PRIOR_MAX_PPB, 1_000_000_000u128);
}

#[test]
fn config_canonical_order() {
    // T04: byte-render equals a manual JCS render.
    let config = PriorConfigV1::default();
    let bytes = config.canonical_bytes().unwrap();
    let manual = concat!(
        "{\"damping_ppm\":850000,\"entities_per_event\":8,",
        "\"epsilon_ppb\":1000000,\"max_edges_per_entity\":32,",
        "\"max_entities\":1024,\"max_iterations\":64,\"max_seeds\":64,",
        "\"prereg_reference\":\"be20d8fc48771098e745038b906dd13456ffcebdeb424cee25e91d52eae784c9\",",
        "\"prior_max_ppb\":1000000000,\"thorn_status\":\"thorn_disabled\",",
        "\"version\":1}"
    );
    assert_eq!(bytes, manual.as_bytes());
}

#[test]
fn config_validate_frozen() {
    // T05: each mutated member → Err; unmutated → Ok.
    let base = PriorConfigV1::default();
    assert!(base.validate_frozen().is_ok());

    let shortlist = [
        PriorConfigV1 {
            version: 2,
            ..base.clone()
        },
        PriorConfigV1 {
            damping_ppm: 850_001,
            ..base.clone()
        },
        PriorConfigV1 {
            epsilon_ppb: 999_999,
            ..base.clone()
        },
        PriorConfigV1 {
            max_iterations: 63,
            ..base.clone()
        },
        PriorConfigV1 {
            max_entities: 1023,
            ..base.clone()
        },
        PriorConfigV1 {
            max_edges_per_entity: 31,
            ..base.clone()
        },
        PriorConfigV1 {
            max_seeds: 63,
            ..base.clone()
        },
        PriorConfigV1 {
            entities_per_event: 9,
            ..base.clone()
        },
        PriorConfigV1 {
            prior_max_ppb: 999_999_999,
            ..base.clone()
        },
        PriorConfigV1 {
            thorn_status: "thorn_enabled",
            ..base.clone()
        },
        PriorConfigV1 {
            prereg_reference: "deadbeef",
            ..base.clone()
        },
    ];
    assert_eq!(shortlist.len(), 11); // one mutation per member.
    for mutated in &shortlist {
        assert!(
            mutated.validate_frozen().is_err(),
            "mutated config must fail closed"
        );
    }

    // canonical_bytes and config_hash also fail closed on every deviation
    // (all 11 mutations, not a spot-check).
    for mutated in &shortlist {
        assert!(
            mutated.canonical_bytes().is_err(),
            "canonical_bytes must fail closed on any deviation"
        );
        assert!(
            mutated.config_hash().is_err(),
            "config_hash must fail closed on any deviation"
        );
    }
}

#[test]
fn config_hash_domain() {
    // T06: hash = BLAKE3(`oc-03-priorcfg1\0` + bytes), prefix `ocpriorcfg1_`;
    // independent re-hash matches. The prior_id domain and prefix are the
    // other frozen half of the domain set (§5).
    let config = PriorConfigV1::default();
    let got = config.config_hash().unwrap();
    assert_eq!(CONFIG_HASH_PREFIX, "ocpriorcfg1_");
    assert_eq!(PRIOR_CONFIG_HASH_DOMAIN, b"oc-03-priorcfg1\0".as_slice());
    assert!(got.starts_with(CONFIG_HASH_PREFIX));
    assert_eq!(PRIOR_ID_DOMAIN, b"oc-03-prior-v1\0".as_slice());
    assert_eq!(PRIOR_ID_PREFIX, "ocprior1_");

    // Independent re-hash over domain + canonical bytes must match. The
    // encoding is base64url(blake3) (spec §7.5) — NOT hex like OC-02.
    let mut hasher = blake3::Hasher::new();
    hasher.update(PRIOR_CONFIG_HASH_DOMAIN);
    hasher.update(config.canonical_bytes().unwrap().as_slice());
    let expected =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize().as_bytes());
    assert_eq!(got.trim_start_matches(CONFIG_HASH_PREFIX), expected);

    // An undomained hash differs (domain separation is real).
    let mut undomained = blake3::Hasher::new();
    undomained.update(config.canonical_bytes().unwrap().as_slice());
    let undomained_b64 =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(undomained.finalize().as_bytes());
    assert_ne!(got.trim_start_matches(CONFIG_HASH_PREFIX), undomained_b64);

    // Deviating config fails closed.
    let bad = PriorConfigV1 {
        max_seeds: 63,
        ..config
    };
    assert!(bad.config_hash().is_err());
}

#[test]
fn prereg_reference_seal() {
    // T07: literal equals the frozen P1 SHA-256 string, byte-for-byte.
    assert_eq!(
        PREREG_SHA256,
        "be20d8fc48771098e745038b906dd13456ffcebdeb424cee25e91d52eae784c9"
    );
    // The config carries the seal verbatim in its canonical bytes.
    let config = PriorConfigV1::default();
    let text = String::from_utf8(config.canonical_bytes().unwrap()).unwrap();
    assert!(text.contains(&format!("\"prereg_reference\":\"{PREREG_SHA256}\"")));
}

#[test]
fn envelope_member_set() {
    // T08: parsed member set equals the exact 13 §7.4 names.
    use contextmesh_salience::prior::{EntityEdgeV1, EntityGraphV1, PriorSeedSetV1, PriorSeedV1};

    let config = PriorConfigV1::default();
    let config_hash = config.config_hash().unwrap();

    let edge = EntityEdgeV1::new_for_test("count:1", "count:2");
    let graph = EntityGraphV1::new_for_test(
        vec!["count:1".to_owned(), "count:2".to_owned()],
        vec![edge],
        0,
        0,
        config_hash.clone(),
    );
    let seed = PriorSeedV1::new_for_test("count:1", 500_000_000);
    let seeds = PriorSeedSetV1::new_for_test(
        vec![seed],
        vec!["ocattr1_x".to_owned()],
        0,
        config_hash.clone(),
    );
    let prior = SaliencePriorV1::new_for_test(
        "ocprior1_deadbeef".to_owned(),
        config_hash,
        vec!["ocattr1_x".to_owned()],
        graph,
        seeds,
        vec![PriorSeedV1::new_for_test("count:1", 425_000_000)],
        3,
        true,
        12,
        0,
        "terminal",
    );

    let bytes = prior.canonical_bytes().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let object = value.as_object().unwrap();
    let mut names: Vec<&str> = object.keys().map(String::as_str).collect();
    names.sort_unstable();
    let mut expected: Vec<&str> = [
        "config_hash",
        "converged",
        "dropped_seeds",
        "graph",
        "iterations",
        "prior_id",
        "residual_ppb",
        "seeds",
        "source_report_ids",
        "terminal_status",
        "thorn_status",
        "vector",
        "version",
    ]
    .to_vec();
    expected.sort_unstable();
    assert_eq!(names, expected);
    assert_eq!(names.len(), 13);
}
