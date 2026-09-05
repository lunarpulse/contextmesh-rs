//! OC-04 4G-harness-extension: fixture-backed acceptance tests (v5.2 item 8).
//!
//! Exactly the eight fixtures §7.2 requires:
//! F1 valid mini-corpus passes · F2 manifest hash mismatch fails ·
//! F3 schema error fails · F4 duplicate binding fails ·
//! F5 unresolved `uncertain` fails · F6 unknown HMAC/index fails ·
//! F7 bootstrap deterministic for seed 20260820 ·
//! F8 judge-call cap breach fails closed.
//!
//! All fixtures are labeled by fixture design — NOT real human labels
//! (the NOT-REAL-DATA discipline of `oc04_gold.rs` carries over).

use contextmesh_salience::oc04_gold_realdata::{
    FROZEN_BOOTSTRAP_SEED, family_cluster_bootstrap, load_corpus, manifest_hash_hex,
};

// ---------------------------------------------------------------------------
// Fixture mini-corpus (4 sessions, 3 families: fA={s1,s2}, fB={s3}, fC={s4})
// ---------------------------------------------------------------------------

fn fixture_manifest() -> String {
    // Canonical JSON document (committed sampling manifest byte-shape).
    r#"{"families":{"s1":"fA","s2":"fA","s3":"fB","s4":"fC"},"sessions":{"s1":"hmac_s1","s2":"hmac_s2","s3":"hmac_s3","s4":"hmac_s4"}}"#
        .to_owned()
}

fn fixture_labels() -> String {
    [
        r#"{"adjudication":null,"judgment":"required","session":"s1"}"#,
        r#"{"adjudication":null,"judgment":"supporting","session":"s2"}"#,
        r#"{"adjudication":null,"judgment":"dead_end","session":"s3"}"#,
        // `uncertain` WITH a structured adjudication (§4): passes, and the
        // resolution becomes the effective label with an audit trail.
        r#"{"adjudication":{"reason_code":"insufficient_context","resolution":"supporting"},"judgment":"uncertain","session":"s4"}"#,
    ]
    .join("\n")
}

fn fixture_bindings() -> String {
    [
        r#"{"candidates":[0,1,2],"comparator":{"lexical_ppm":[0,500000,0],"prior_ppm":[120000,0,90000]},"family_hmac":"hmac_fA","session_hmac":"hmac_s1","shortlist32":[0,2]}"#,
        r#"{"candidates":[0,1,2],"comparator":{"lexical_ppm":[300000,0,0],"prior_ppm":[0,60000,0]},"family_hmac":"hmac_fA","session_hmac":"hmac_s2","shortlist32":[1]}"#,
        r#"{"candidates":[0,1],"comparator":{"lexical_ppm":[0,0],"prior_ppm":[40000,0]},"family_hmac":"hmac_fB","session_hmac":"hmac_s3","shortlist32":[0]}"#,
        r#"{"candidates":[0,1,2],"comparator":{"lexical_ppm":[0,0,250000],"prior_ppm":[0,0,0]},"family_hmac":"hmac_fC","session_hmac":"hmac_s4","shortlist32":[2]}"#,
    ]
    .join("\n")
}

fn valid_manifest_hash() -> String {
    manifest_hash_hex(fixture_manifest().as_bytes())
}

fn load_valid() -> contextmesh_salience::oc04_gold_realdata::GoldCorpus {
    load_corpus(
        fixture_manifest().as_bytes(),
        fixture_labels().as_bytes(),
        fixture_bindings().as_bytes(),
        &valid_manifest_hash(),
    )
    .expect("valid fixture corpus loads")
}

// ---------------------------------------------------------------------------
// F1: valid mini-corpus passes end-to-end
// ---------------------------------------------------------------------------

#[test]
fn f1_valid_mini_corpus_passes() {
    let corpus = load_valid();
    assert_eq!(corpus.sessions().len(), 4, "4 manifest sessions load");
    assert_eq!(
        corpus.gold_set(),
        &[
            "s1".to_owned(),
            "s2".to_owned(),
            "s3".to_owned(),
            "s4".to_owned()
        ],
        "gold set = all labeled non-irrelevant sessions (uncertain adjudicated)"
    );
    assert_eq!(
        corpus.families().get("s1").map(String::as_str),
        Some("fA"),
        "family cluster resolved per session"
    );
    assert_eq!(
        corpus.families().get("s4").map(String::as_str),
        Some("fC"),
        "family resolution covers every session"
    );
    // The adjudicated `uncertain` ships its audit trail (§4).
    let adj = corpus.adjudications().get("s4").expect("audit trail");
    assert_eq!(adj.0, "uncertain");
    assert_eq!(adj.1, "insufficient_context");
    assert_eq!(adj.2, "supporting");
    // Effective label for s4 is the adjudicated resolution.
    assert_eq!(
        corpus.judgments().get("s4").map(String::as_str),
        Some("supporting")
    );
    // Binding surfaces the replay substrate fields (§7.1).
    let b = corpus.bindings().get("s1").expect("binding");
    assert_eq!(b.session_hmac, "hmac_s1");
    assert_eq!(b.candidates, vec![0, 1, 2]);
    assert_eq!(b.shortlist32, vec![0, 2]);
    assert_eq!(b.lexical_ppm, vec![0, 500_000, 0]);
    // Bootstrap runs sane and non-degenerate over the fixture families.
    let boot = family_cluster_bootstrap(&corpus, FROZEN_BOOTSTRAP_SEED, 1_000);
    assert!(boot.ci_lo_ppm <= boot.point_ppm && boot.point_ppm <= boot.ci_hi_ppm);
    assert!(boot.ci_hi_ppm > boot.ci_lo_ppm, "non-degenerate CI");
}

// ---------------------------------------------------------------------------
// F2: manifest hash mismatch fails closed
// ---------------------------------------------------------------------------

#[test]
fn f2_manifest_hash_mismatch_fails() {
    let wrong = manifest_hash_hex(b"not-the-manifest");
    let err = load_corpus(
        fixture_manifest().as_bytes(),
        fixture_labels().as_bytes(),
        fixture_bindings().as_bytes(),
        &wrong,
    )
    .expect_err("hash mismatch must fail closed");
    let msg = format!("{err}");
    assert!(
        msg.contains("manifest hash mismatch"),
        "specific marker, got: {msg}"
    );
    assert!(
        msg.contains("have=") && msg.contains("want="),
        "digest pair recorded: {msg}"
    );
}

// ---------------------------------------------------------------------------
// F3: schema validation error fails closed
// ---------------------------------------------------------------------------

#[test]
fn f3_schema_error_fails() {
    // A judgment outside the frozen 5-label scheme.
    let labels = r#"{"adjudication":null,"judgment":"bogus-label","session":"s1"}"#;
    let err = load_corpus(
        fixture_manifest().as_bytes(),
        labels.as_bytes(),
        fixture_bindings().as_bytes(),
        &valid_manifest_hash(),
    )
    .expect_err("unknown label must fail closed");
    assert!(format!("{err}").contains("unknown label"), "got: {err}");

    // A malformed JSON line.
    let err2 = load_corpus(
        fixture_manifest().as_bytes(),
        b"{not json",
        fixture_bindings().as_bytes(),
        &valid_manifest_hash(),
    )
    .expect_err("malformed JSON must fail closed");
    assert!(format!("{err2}").contains("labels schema"), "got: {err2}");

    // A bindings comparator length mismatch (schema shape error).
    let bad_bindings = r#"{"candidates":[0,1],"comparator":{"lexical_ppm":[0],"prior_ppm":[0,0]},"family_hmac":"hmac_fA","session_hmac":"hmac_s1","shortlist32":[0]}"#;
    let err3 = load_corpus(
        fixture_manifest().as_bytes(),
        fixture_labels().as_bytes(),
        bad_bindings.as_bytes(),
        &valid_manifest_hash(),
    )
    .expect_err("comparator length mismatch must fail closed");
    assert!(
        format!("{err3}").contains("comparator `lexical_ppm` length"),
        "got: {err3}"
    );
}

// ---------------------------------------------------------------------------
// F4: duplicate session binding fails closed
// ---------------------------------------------------------------------------

#[test]
fn f4_duplicate_binding_fails() {
    let mut bindings = fixture_bindings();
    let s1_line = bindings
        .lines()
        .find(|l| l.contains("hmac_s1"))
        .expect("s1 line exists")
        .to_owned();
    bindings.push('\n');
    bindings.push_str(&s1_line);
    let err = load_corpus(
        fixture_manifest().as_bytes(),
        fixture_labels().as_bytes(),
        bindings.as_bytes(),
        &valid_manifest_hash(),
    )
    .expect_err("duplicate session binding must fail closed");
    assert!(
        format!("{err}").contains("duplicate session binding"),
        "got: {err}"
    );
}

// ---------------------------------------------------------------------------
// F5: unresolved `uncertain` in the gold set fails closed (§4)
// ---------------------------------------------------------------------------

#[test]
fn f5_unresolved_uncertain_fails() {
    // No adjudication object at all.
    let labels = [
        r#"{"adjudication":null,"judgment":"required","session":"s1"}"#,
        r#"{"adjudication":null,"judgment":"uncertain","session":"s2"}"#,
    ]
    .join("\n");
    let err = load_corpus(
        fixture_manifest().as_bytes(),
        labels.as_bytes(),
        fixture_bindings().as_bytes(),
        &valid_manifest_hash(),
    )
    .expect_err("unresolved uncertain (no adjudication) must fail closed");
    assert!(
        format!("{err}").contains("unresolved `uncertain`"),
        "got: {err}"
    );

    // Adjudication present but the resolution is not decisive.
    let labels2 = r#"{"adjudication":{"reason_code":"codebook_gap","resolution":"uncertain"},"judgment":"uncertain","session":"s2"}"#;
    let err2 = load_corpus(
        fixture_manifest().as_bytes(),
        labels2.as_bytes(),
        fixture_bindings().as_bytes(),
        &valid_manifest_hash(),
    )
    .expect_err("uncertain→uncertain resolution must fail closed");
    assert!(format!("{err2}").contains("not decisive"), "got: {err2}");
}

// ---------------------------------------------------------------------------
// F6: unknown HMAC / unknown candidate index fails closed
// ---------------------------------------------------------------------------

#[test]
fn f6_unknown_hmac_or_index_fails() {
    // A bindings row whose session_hmac is absent from the frozen manifest.
    let unknown_hmac = fixture_bindings()
        .lines()
        .map(|line| {
            if line.contains("hmac_s3") {
                line.replace("hmac_s3", "hmac_UNKNOWN")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let err = load_corpus(
        fixture_manifest().as_bytes(),
        fixture_labels().as_bytes(),
        unknown_hmac.as_bytes(),
        &valid_manifest_hash(),
    )
    .expect_err("unknown session HMAC must fail closed");
    assert!(
        format!("{err}").contains("unknown session binding"),
        "got: {err}"
    );

    // A shortlist index outside the session's candidate set.
    let unknown_idx = fixture_bindings().replacen("[0,2]", "[0,7]", 1);
    let err2 = load_corpus(
        fixture_manifest().as_bytes(),
        fixture_labels().as_bytes(),
        unknown_idx.as_bytes(),
        &valid_manifest_hash(),
    )
    .expect_err("unknown candidate index must fail closed");
    assert!(
        format!("{err2}").contains("unknown candidate index"),
        "got: {err2}"
    );
}

// ---------------------------------------------------------------------------
// F7: bootstrap determinism for the pinned seed 20260820
// ---------------------------------------------------------------------------

#[test]
fn f7_bootstrap_deterministic_seed_20260820() {
    let corpus = load_valid();
    let a = family_cluster_bootstrap(&corpus, FROZEN_BOOTSTRAP_SEED, 2_000);
    let b = family_cluster_bootstrap(&corpus, FROZEN_BOOTSTRAP_SEED, 2_000);
    assert_eq!(
        (a.point_ppm, a.ci_lo_ppm, a.ci_hi_ppm),
        (b.point_ppm, b.ci_lo_ppm, b.ci_hi_ppm),
        "bootstrap CI must be deterministic for the pinned seed"
    );
    // And the seed genuinely drives the generator (unit-level: different
    // seeds produce different streams — the CI itself may legitimately
    // coincide on a coarse 3-family fixture, but the RNG must not).
    let mut r1 = contextmesh_salience::oc04_gold_realdata::Lfg::new(FROZEN_BOOTSTRAP_SEED);
    let mut r2 = contextmesh_salience::oc04_gold_realdata::Lfg::new(FROZEN_BOOTSTRAP_SEED + 1);
    assert_ne!(
        r1.next_raw(),
        r2.next_raw(),
        "seed must change the RNG stream"
    );
}

// ---------------------------------------------------------------------------
// F8: judge-call cap breach fails closed (§5.5 verbatim caps)
// ---------------------------------------------------------------------------

#[test]
fn f8_judge_call_cap_breach_fails_closed() {
    use contextmesh_salience::oc04_gold_realdata::JudgeCallAccounting;

    let mut acct = JudgeCallAccounting::new(&[("m3", 8), ("m4", 128)]);
    acct.record("s1", "m3", 8);
    acct.record("s1", "m4", 128);
    let caps = acct.emit_caps();
    assert!(caps["s1"], "exactly at cap is NOT a breach");

    // A single extra m3 call (e.g. one retry — retries count, §5.5).
    acct.record("s1", "m3", 1);
    let caps = acct.emit_caps();
    assert!(!caps["s1"], "cap breach → false (fail-closed)");

    // A second session stays independently clean.
    acct.record("s2", "m4", 5);
    let caps = acct.emit_caps();
    assert!(!caps["s1"], "breached session stays breached");
    assert!(caps["s2"], "clean session unaffected");
}
