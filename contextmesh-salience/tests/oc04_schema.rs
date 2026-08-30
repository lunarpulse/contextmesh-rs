//! OC-04 Stage 4B: schema tests (matrix rows S01–S06, S08–S10).
//!
//! S07b (`canonical_extra_member_rejected`) is delivered at 4E — it needs
//! `verify_execution`. The S-row gate for 4B is S01–S06 + S08–S10.

use contextmesh::crypto::SigningIdentity;
use contextmesh_salience::oc04_selection::{
    CLIP_ABOVE_PPM, CLIP_BELOW_PPM, ENTRY_REASON_BOTH, ENTRY_REASON_LEXICAL, ENTRY_REASON_PRIOR,
    LEXICAL_ARM_CAP, OC04_EXEC_SIGNATURE_DOMAIN, Oc04ConfigV1, PRIOR_ARM_CAP,
    SelectionExecutionBodyV1, SelectionInfluenceEntryV1, SelectionInfluenceV1, SignedExecutionV1,
    derive_execution_id, render_execution_body,
};

/// S03/S03b: every frozen constant equals the P1 prereg JSON value loaded
/// at test time (selection_pipeline.per_arm_caps +
/// evaluation.score_normalization).
const PREREG_JSON: &str =
    include_str!("../../_bmad-output/implementation-artifacts/p1-prereg-config.json");

#[test]
fn config_prereg_verbatim() {
    // S03: caps equal the prereg selection_pipeline.per_arm_caps values.
    let prereg: serde_json::Value = serde_json::from_str(PREREG_JSON).unwrap();
    let caps = &prereg["selection_pipeline"]["per_arm_caps"];
    assert_eq!(LEXICAL_ARM_CAP, caps["lexical_arm_cap"].as_u64().unwrap());
    assert_eq!(PRIOR_ARM_CAP, caps["prior_arm_cap"].as_u64().unwrap());
    // Overflow-policy text is the prereg's verbatim contract.
    assert!(
        prereg["selection_pipeline"]["overflow_policy"]
            .as_str()
            .unwrap()
            .contains("fail closed")
    );
    // The config struct itself carries exactly these values.
    let config = Oc04ConfigV1::default();
    assert_eq!(config.lexical_arm_cap, LEXICAL_ARM_CAP);
    assert_eq!(config.prior_arm_cap, PRIOR_ARM_CAP);
}

#[test]
fn config_score_normalization_verbatim() {
    // S03b: normalization constants equal the prereg
    // evaluation.score_normalization values.
    let prereg: serde_json::Value = serde_json::from_str(PREREG_JSON).unwrap();
    let norm = &prereg["evaluation"]["score_normalization"];
    assert_eq!(CLIP_ABOVE_PPM, norm["clip_above_ppm"].as_u64().unwrap());
    assert_eq!(CLIP_BELOW_PPM, norm["clip_below_ppm"].as_u64().unwrap());
    assert_eq!(
        norm["method"].as_str().unwrap(),
        "per-arm min-max to [0, 1000000] ppm"
    );
    let config = Oc04ConfigV1::default();
    assert_eq!(config.clip_above_ppm, CLIP_ABOVE_PPM);
    assert_eq!(config.clip_below_ppm, CLIP_BELOW_PPM);
}

#[test]
fn config_validate_rejects_mutation() {
    // S04: EVERY config member mutation → Err (exhaustive loop).
    let base = Oc04ConfigV1::default();
    let mutations: Vec<Oc04ConfigV1> = vec![
        Oc04ConfigV1 {
            version: 2,
            ..base.clone()
        },
        Oc04ConfigV1 {
            lexical_arm_cap: 65,
            ..base.clone()
        },
        Oc04ConfigV1 {
            prior_arm_cap: 31,
            ..base.clone()
        },
        Oc04ConfigV1 {
            clip_above_ppm: 999_999,
            ..base.clone()
        },
        Oc04ConfigV1 {
            clip_below_ppm: 1,
            ..base.clone()
        },
        Oc04ConfigV1 {
            prereg_reference: "0000000000000000000000000000000000000000000000000000000000000000",
            ..base.clone()
        },
    ];
    assert!(base.validate_frozen().is_ok());
    for mutated in mutations {
        assert!(
            mutated.validate_frozen().is_err(),
            "mutation accepted: {mutated:?}"
        );
    }
}

#[test]
fn influence_jcs_render() {
    // S01: exact byte compare vs hand-rendered canonical JSON.
    let config = Oc04ConfigV1::default();
    let entries = vec![
        SelectionInfluenceEntryV1::new("ev-b", ENTRY_REASON_BOTH, 500_000, 250_000).unwrap(),
        SelectionInfluenceEntryV1::new("ev-a", ENTRY_REASON_LEXICAL, 100_000, 0).unwrap(),
    ];
    let influence =
        SelectionInfluenceV1::assemble(&config, "ocprior1_x", "task-fp", entries).unwrap();
    let bytes = influence.canonical_bytes().unwrap();
    // Hand render: members lexicographic (config_hash, entries,
    // influence_id, prior_id, task_fingerprint, version); entries carry
    // their five members; entry order = rerank order (750000 > 100000).
    let expected = format!(
        concat!(
            "{{\"config_hash\":\"{ch}\",",
            "\"entries\":[{{\"entry_reason\":\"both\",\"event_id\":\"ev-b\",",
            "\"lexical_ppm\":500000,\"prior_ppm\":250000,\"score_ppm\":750000}},",
            "{{\"entry_reason\":\"lexical\",\"event_id\":\"ev-a\",",
            "\"lexical_ppm\":100000,\"prior_ppm\":0,\"score_ppm\":100000}}],",
            "\"influence_id\":\"{iid}\",\"prior_id\":\"ocprior1_x\",",
            "\"task_fingerprint\":\"task-fp\",\"version\":1}}"
        ),
        ch = config.config_hash().unwrap(),
        iid = influence.influence_id(),
    );
    assert_eq!(std::str::from_utf8(&bytes).unwrap(), expected);
    // Entries must be in rerank order in the record itself.
    assert_eq!(influence.entries()[0].event_id_text(), "ev-b");
    // §6 reason↔ppm coherence (part of the S01 render contract: the wire
    // enum and its ppm semantics render together) + duplicate EventIds
    // rejected at assemble (schema-level union dedup). These assertions
    // guard the same entry-wire rule S01 renders; the matrix's
    // one-row/one-assertion discipline treats them as enumerated cases of
    // the entry-rule, not separate rows (v12 row-atomicity convention).
    assert!(SelectionInfluenceEntryV1::new("ev", ENTRY_REASON_BOTH, 5, 0).is_err());
    assert!(SelectionInfluenceEntryV1::new("ev", ENTRY_REASON_BOTH, 0, 5).is_err());
    assert!(SelectionInfluenceEntryV1::new("ev", ENTRY_REASON_LEXICAL, 0, 5).is_err());
    assert!(SelectionInfluenceEntryV1::new("ev", ENTRY_REASON_PRIOR, 5, 0).is_err());
    assert!(SelectionInfluenceEntryV1::new("ev", "unknown", 5, 5).is_err());
    let dup = SelectionInfluenceEntryV1::new("ev-x", ENTRY_REASON_LEXICAL, 7, 0).unwrap();
    let dup_assemble =
        SelectionInfluenceV1::assemble(&config, "ocprior1_x", "task-fp", vec![dup.clone(), dup]);
    assert!(dup_assemble.is_err());
    // Reversed score order rejected (entries must arrive in rerank order).
    let hi = SelectionInfluenceEntryV1::new("ev-hi", ENTRY_REASON_LEXICAL, 900, 0).unwrap();
    let lo = SelectionInfluenceEntryV1::new("ev-lo", ENTRY_REASON_LEXICAL, 100, 0).unwrap();
    let reversed = SelectionInfluenceV1::assemble(&config, "ocprior1_x", "task-fp", vec![lo, hi]);
    assert!(reversed.is_err(), "reversed score order accepted");
    // Equal-score tie must arrive in canonical EventId text ascending.
    let tie_b = SelectionInfluenceEntryV1::new("ev-b", ENTRY_REASON_LEXICAL, 50, 0).unwrap();
    let tie_a = SelectionInfluenceEntryV1::new("ev-a", ENTRY_REASON_LEXICAL, 50, 0).unwrap();
    let bad_tie =
        SelectionInfluenceV1::assemble(&config, "ocprior1_x", "task-fp", vec![tie_b, tie_a]);
    assert!(bad_tie.is_err(), "wrong tie order accepted");
}

#[test]
fn execution_jcs_render() {
    // S02: exact byte compare vs hand-rendered canonical JSON (19 members,
    // lexicographic).
    let config = Oc04ConfigV1::default();
    let body = SelectionExecutionBodyV1 {
        b3_candidate_fingerprint: "aa".to_owned(),
        b3_policy_fingerprint: "bb".to_owned(),
        b6_warnings_hash: "cc".to_owned(),
        budget_max_bytes: 4096,
        budget_max_events: 16,
        closed_count: 3,
        closed_hash: "dd".to_owned(),
        config_hash: config.config_hash().unwrap(),
        critical_projection: "critproj1:ev-a".to_owned(),
        delta_count: 2,
        delta_hash: "ee".to_owned(),
        execution_id: "oc04exec1_placeholder".to_owned(),
        handoff_hash: "ff".to_owned(),
        influence_id: "oc04inf1_x".to_owned(),
        pre_closure_count: 4,
        pre_closure_ids_hash: "11".to_owned(),
        prior_id: "ocprior1_x".to_owned(),
        recipient_head: Some("ev-h".to_owned()),
        version: 1,
    };
    let bytes = render_execution_body(&body);
    let expected = format!(
        concat!(
            "{{\"b3_candidate_fingerprint\":\"aa\",",
            "\"b3_policy_fingerprint\":\"bb\",\"b6_warnings_hash\":\"cc\",",
            "\"budget_max_bytes\":4096,\"budget_max_events\":16,",
            "\"closed_count\":3,\"closed_hash\":\"dd\",",
            "\"config_hash\":\"{ch}\",",
            "\"critical_projection\":\"critproj1:ev-a\",",
            "\"delta_count\":2,\"delta_hash\":\"ee\",",
            "\"execution_id\":\"oc04exec1_placeholder\",",
            "\"handoff_hash\":\"ff\",\"influence_id\":\"oc04inf1_x\",",
            "\"pre_closure_count\":4,\"pre_closure_ids_hash\":\"11\",",
            "\"prior_id\":\"ocprior1_x\",\"recipient_head\":\"ev-h\",",
            "\"version\":1}}"
        ),
        ch = config.config_hash().unwrap(),
    );
    assert_eq!(std::str::from_utf8(&bytes).unwrap(), expected);
    // recipient_head=None renders JSON null (not absent, not "").
    let mut body_null = body.clone();
    body_null.recipient_head = None;
    let null_bytes = render_execution_body(&body_null);
    let text = std::str::from_utf8(&null_bytes).unwrap();
    assert!(text.contains("\"recipient_head\":null"));
    // Member count sanity: exactly 19 `":"` key-value separators (keys
    // are plain identifiers, so `":"` never occurs inside string values
    // of this render — values there are hex/text/numbers/null).
    assert_eq!(text.matches("\":").count(), 19);
}

#[test]
fn id_placeholder_derivation() {
    // S05: influence ID = BLAKE3 over placeholder-substituted canonical
    // bytes; execution ID likewise; both base64url no-pad with the §5
    // prefixes.
    let config = Oc04ConfigV1::default();
    let entries = vec![SelectionInfluenceEntryV1::new("ev-a", ENTRY_REASON_PRIOR, 0, 42).unwrap()];
    let influence =
        SelectionInfluenceV1::assemble(&config, "ocprior1_x", "task-fp", entries).unwrap();
    assert!(influence.influence_id().starts_with("oc04inf1_"));
    // base64url no-pad alphabet (no '=' padding, no '+'/').
    let id = &influence.influence_id()["oc04inf1_".len()..];
    assert!(!id.contains('='));
    assert!(
        id.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    );
    // Determinism: reassembly reproduces the exact ID.
    let entries2 = vec![SelectionInfluenceEntryV1::new("ev-a", ENTRY_REASON_PRIOR, 0, 42).unwrap()];
    let again = SelectionInfluenceV1::assemble(&config, "ocprior1_x", "task-fp", entries2).unwrap();
    assert_eq!(again.influence_id(), influence.influence_id());
    // Independent placeholder-domain recomputation for the INFLUENCE id:
    // BLAKE3 over oc-04-inf-v1-id\0 + canonical bytes with the id member
    // = literal "influence_id" (S05 covers both IDs, not just execution).
    let mut ih = blake3::Hasher::new();
    ih.update(b"oc-04-inf-v1-id\0");
    ih.update(&influence.placeholder_bytes_for_test());
    use base64::Engine as _;
    let expect_inf = format!(
        "oc04inf1_{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(ih.finalize().as_bytes())
    );
    assert_eq!(influence.influence_id(), expect_inf);
    // Execution ID derivation: placeholder discipline (id member = literal
    // "execution_id" at hash time), prefix + base64url no-pad.
    let body = SelectionExecutionBodyV1 {
        b3_candidate_fingerprint: "aa".to_owned(),
        b3_policy_fingerprint: "bb".to_owned(),
        b6_warnings_hash: "cc".to_owned(),
        budget_max_bytes: 1,
        budget_max_events: 1,
        closed_count: 0,
        closed_hash: String::new(),
        config_hash: config.config_hash().unwrap(),
        critical_projection: String::new(),
        delta_count: 0,
        delta_hash: String::new(),
        execution_id: "PLACEHOLDER".to_owned(),
        handoff_hash: String::new(),
        influence_id: influence.influence_id().to_owned(),
        pre_closure_count: 0,
        pre_closure_ids_hash: String::new(),
        prior_id: "ocprior1_x".to_owned(),
        recipient_head: None,
        version: 1,
    };
    let exec_id = derive_execution_id(&body);
    assert!(exec_id.starts_with("oc04exec1_"));
    // Independent recomputation of the placeholder hash.
    let mut ph = body.clone();
    ph.execution_id = "execution_id".to_owned();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"oc-04-exec-v1-id\0");
    hasher.update(&render_execution_body(&ph));
    let expect = {
        use base64::Engine as _;
        format!(
            "oc04exec1_{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize().as_bytes())
        )
    };
    assert_eq!(exec_id, expect);
    // Config hash domain separation: changing the domain changes the hash.
    let ch = config.config_hash().unwrap();
    let mut hasher2 = blake3::Hasher::new();
    hasher2.update(b"oc-04-config-v1\0");
    hasher2.update(&config.canonical_bytes().unwrap());
    assert_eq!(ch, hasher2.finalize().to_hex().to_string());
}

#[test]
fn id_prefix_rejected() {
    // S06: forged/placeholder/non-derived IDs are rejected. assemble()
    // always derives its own ID; the enforceable surfaces are (a) the
    // placeholder execution id and (b) an ARBITRARY non-derived id —
    // both rejected at issue (§9 placeholder discipline enforced).
    let signer = SigningIdentity::from_seed([7_u8; 32]);
    let config = Oc04ConfigV1::default();
    for exec_id in ["execution_id", "wrongprefix_self_chosen"] {
        let body = SelectionExecutionBodyV1 {
            execution_id: exec_id.to_owned(),
            ..placeholder_body(&config)
        };
        assert!(
            SignedExecutionV1::issue(body, &signer).is_err(),
            "non-derived execution_id accepted: {exec_id}"
        );
    }
    // version != 1 rejected at issue even WITH a v2-DERIVED id: the id is
    // computed over the version=2 body itself, so this is the strongest
    // self-consistent-v2 forgery shape (B2 fix: the frozen §6 decimal
    // integer is enforced BEFORE id checking, making the forgery
    // impossible).
    let mut v2 = placeholder_body(&config);
    v2.version = 2;
    v2.execution_id = derive_execution_id(&v2);
    assert!(
        SignedExecutionV1::issue(v2, &signer).is_err(),
        "version=2 with v2-derived id accepted"
    );
    // QB3: semantically invalid members rejected at issue even with a
    // derived id — non-hex fingerprint, wrong prior/influence prefixes,
    // non-versioned projection.
    for mutate in [
        |b: &mut SelectionExecutionBodyV1| b.b3_candidate_fingerprint = "NOTHEX!!".to_owned(),
        |b: &mut SelectionExecutionBodyV1| b.prior_id = "wrong-prior".to_owned(),
        |b: &mut SelectionExecutionBodyV1| b.influence_id = "wrong-inf".to_owned(),
        |b: &mut SelectionExecutionBodyV1| b.critical_projection = "unversioned".to_owned(),
        |b: &mut SelectionExecutionBodyV1| {
            // config_hash must be 64 lowercase hex — a prefixed non-hex
            // value must not bypass the hex check (QB3 re-check finding).
            b.config_hash = "oc04inf1_not_hex_at_all".to_owned();
        },
    ] {
        let mut bad = placeholder_body(&config);
        mutate(&mut bad);
        bad.execution_id = derive_execution_id(&bad);
        assert!(
            SignedExecutionV1::issue(bad, &signer).is_err(),
            "invalid body accepted at issue"
        );
    }
}

#[test]
fn no_float_tokens() {
    // S08: no f32/f64 tokens in the OC-04 module (include_str! scan).
    let source = include_str!("../../contextmesh-salience/src/oc04_selection.rs");
    for banned in [
        "f32", "f64", "as float", ".sqrt()", ".powf(", ".exp(", ".ln(",
    ] {
        assert!(
            !source.contains(banned),
            "banned float token {banned:?} found in oc04_selection.rs"
        );
    }
}

#[test]
fn signature_roundtrip() {
    // S09: sign body → verify Ok.
    let signer = SigningIdentity::from_seed([9_u8; 32]);
    let config = Oc04ConfigV1::default();
    let body = SelectionExecutionBodyV1 {
        execution_id: derive_execution_id(&placeholder_body(&config)),
        ..placeholder_body(&config)
    };
    let signed = SignedExecutionV1::issue(body, &signer).unwrap();
    assert!(signed.verify().is_ok());
    // Signer bytes round-trip as the author identity.
    assert_eq!(signed.signer().len(), 32);
    assert_eq!(signed.signature().len(), 64);
}

#[test]
fn signature_domain_isolated() {
    // S10: the same body signed over a DIFFERENT domain fails verification
    // (domain separation is enforced by the verify-side recompute, which
    // always uses oc-04-exec-v1\0).
    use contextmesh::crypto::verify_domain_message;
    let signer = SigningIdentity::from_seed([11_u8; 32]);
    let config = Oc04ConfigV1::default();
    let body = SelectionExecutionBodyV1 {
        execution_id: derive_execution_id(&placeholder_body(&config)),
        ..placeholder_body(&config)
    };
    let canonical = render_execution_body(&body);
    // Issue normally, then prove the domain is load-bearing: re-signing
    // over a foreign domain must NOT verify under the OC-04 domain.
    let foreign_sig = signer.sign_domain_message(b"oc-03-prior-v1\0", &canonical);
    let signed = SignedExecutionV1::issue(body, &signer).unwrap();
    let mut author = [0_u8; 32];
    author.copy_from_slice(signed.signer());
    let author = contextmesh::model::AuthorId::from_bytes(author);
    assert!(
        verify_domain_message(
            author,
            OC04_EXEC_SIGNATURE_DOMAIN,
            &canonical,
            signed.signature()
        )
        .is_ok()
    );
    assert!(
        verify_domain_message(author, OC04_EXEC_SIGNATURE_DOMAIN, &canonical, &foreign_sig)
            .is_err()
    );
}

/// Minimal well-formed body for signature tests.
fn placeholder_body(config: &Oc04ConfigV1) -> SelectionExecutionBodyV1 {
    SelectionExecutionBodyV1 {
        b3_candidate_fingerprint: String::new(),
        b3_policy_fingerprint: String::new(),
        b6_warnings_hash: String::new(),
        budget_max_bytes: 0,
        budget_max_events: 0,
        closed_count: 0,
        closed_hash: String::new(),
        config_hash: config.config_hash().unwrap(),
        critical_projection: String::new(),
        delta_count: 0,
        delta_hash: String::new(),
        execution_id: "PLACEHOLDER".to_owned(),
        handoff_hash: String::new(),
        influence_id: "oc04inf1_x".to_owned(),
        pre_closure_count: 0,
        pre_closure_ids_hash: String::new(),
        prior_id: "ocprior1_x".to_owned(),
        recipient_head: None,
        version: 1,
    }
}
