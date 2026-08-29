//! OC-03 Stage 3F artifact tests (matrix rows OC03-A01..A10).
use contextmesh_salience::prior::{
    PriorConfigV1, PriorSeedSetV1, PriorSeedV1, ReportContribution, SaliencePriorV1,
    SessionPayloads, assemble_prior, build_entity_graph, derive_seeds, parse_prior_bytes, run_ppr,
    verify_prior,
};

/// Build a minimal report envelope with the given section status and m4
/// shares (adapter tier rendered as an embedded JSON string; mirrors the
/// Stage 3D helper).
fn report_json(report_id: &str, status: &str, shares: &[(&str, u128)]) -> String {
    let m4: Vec<String> = shares
        .iter()
        .map(|(event, ppm)| {
            format!(
                "{{\"event\":\"{event}\",\"judge\":\"j.example\",\"judge_config_hash\":\"h\",\"judge_version\":\"v1\",\"samples\":64,\"share_ppm\":{ppm}}}"
            )
        })
        .collect();
    let tier = format!(
        "{{\"m3\":[],\"m4\":[{}],\"status\":\"{status}\",\"uncertainty_markers\":[]}}",
        m4.join(",")
    );
    format!(
        "{{\"adapter_tier\":\"{}\",\"config_hash\":\"ocattrcfg1_x\",\"ledger_id\":\"ocout1_a\",\"prereg_reference\":\"be20d8fc\",\"report_id\":\"{report_id}\",\"task_fingerprint\":\"t\",\"input_snapshot_fingerprint\":\"i\",\"deterministic_tier\":\"d\",\"terminal_status\":\"terminal\",\"version\":1}}",
        tier.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

/// Inputs of the standard fixture: one session, one computed report.
fn fixture_inputs() -> (
    Vec<SessionPayloads<'static>>,
    Vec<ReportContribution>,
    Vec<(&'static str, &'static str)>,
) {
    let sessions = vec![SessionPayloads::from_payloads(vec![
        "alpha beta",
        "beta charlie",
    ])];
    let report = ReportContribution::from_report_bytes(
        report_json("r1", "computed", &[("evt-a", 600_000), ("evt-c", 200_000)]).as_bytes(),
    )
    .expect("report parses");
    let events: Vec<(&str, &str)> = vec![("evt-a", "alpha"), ("evt-c", "beta charlie")];
    (sessions, vec![report], events)
}

/// Deterministic full-pipeline fixture: graph + real derived seeds + ppr +
/// assembly.
fn build_fixture() -> SaliencePriorV1 {
    let config = PriorConfigV1::default();
    let (sessions, reports, events) = fixture_inputs();
    let graph = build_entity_graph(&sessions, &config).expect("graph");
    let (seeds, dropped) = derive_seeds(&reports, &events, &config).expect("seeds");
    assert_eq!(dropped, 0);
    let ppr = run_ppr(&graph, &seeds, &config).expect("ppr");
    assemble_prior(graph, seeds, &ppr, dropped, "terminal", &config).expect("assemble")
}

#[test]
fn artifact_assembly() {
    // A01: member values equal the inputs.
    let prior = build_fixture();
    assert_eq!(prior.prior_id().len(), "ocprior1_".len() + 64);
    assert!(prior.prior_id().starts_with("ocprior1_"));
    assert_eq!(
        prior.config_hash(),
        PriorConfigV1::default().config_hash().expect("cfg")
    );
    assert_eq!(prior.source_report_ids(), &["r1".to_owned()]);
    assert_eq!(prior.terminal_status(), "terminal");
    assert_eq!(prior.dropped_seeds(), 0);
    assert!(!prior.vector().is_empty(), "derived seeds propagate");
    let entities: Vec<&str> = prior.vector().iter().map(|v| v.entity()).collect();
    let mut sorted = entities.clone();
    sorted.sort_unstable();
    assert_eq!(entities, sorted);
    assert!(prior.vector().iter().all(|v| v.ppb() > 0));
}

#[test]
fn prior_id_derivation() {
    // A02: independent hash over placeholder-normalized bytes.
    let prior = build_fixture();
    let bytes = prior.canonical_bytes().expect("canonical");
    let text = String::from_utf8(bytes.to_vec()).expect("utf8");
    let derived = prior.prior_id();
    let substituted = text.replace(&format!("\"{derived}\""), "\"prior_id\"");
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"oc-03-prior-v1\0");
    hasher.update(substituted.as_bytes());
    let expected = format!("ocprior1_{}", hasher.finalize().to_hex());
    assert_eq!(derived, &expected, "placeholder derivation reproduced");
}

#[test]
fn prior_id_tamper_matrix() {
    // A03: mutating scalar members (config_hash prefix, terminal_status,
    // residual key) yields artifacts that fail rebuild verification.
    let (sessions, reports, events) = fixture_inputs();
    let config = PriorConfigV1::default();
    let prior = build_fixture();
    let bytes = prior.canonical_bytes().expect("canonical");
    let text = String::from_utf8(bytes.to_vec()).expect("utf8");
    let variants = [
        text.replace("ocpriorcfg1_", "ocpriorcfg2_"),
        text.replace("\"terminal\"", "\"unterminated\""),
        text.replace("\"residual_ppb\":", "\"residual_ppbX\":"),
    ];
    for (i, variant) in variants.iter().enumerate() {
        assert_ne!(variant, &text, "variant {i} must differ");
        assert!(
            verify_prior(variant.as_bytes(), &sessions, &reports, &events, &config).is_err(),
            "tampered variant {i} rejected"
        );
    }
}

#[test]
fn verify_recompute() {
    // A04: a well-formed artifact verifies Ok against its true inputs;
    // any byte mutation → Err.
    let (sessions, reports, events) = fixture_inputs();
    let config = PriorConfigV1::default();
    let prior = build_fixture();
    let bytes = prior.canonical_bytes().expect("canonical");
    assert!(verify_prior(&bytes, &sessions, &reports, &events, &config).is_ok());
    let mut mutated = bytes.clone();
    let idx = mutated.iter().position(|&b| b == b'"').expect("quote");
    mutated[idx] = b' ';
    assert!(verify_prior(&mutated, &sessions, &reports, &events, &config).is_err());
}

#[test]
fn verify_no_trust() {
    // A05: self-consistent forgeries fail rebuild verification.
    let (sessions, reports, events) = fixture_inputs();
    let config = PriorConfigV1::default();
    let prior = build_fixture();
    let bytes = prior.canonical_bytes().expect("canonical");
    let text = String::from_utf8(bytes.to_vec()).expect("utf8");

    // Forgery 1: inflated vector ppb with re-derived prior_id (the B1
    // reviewer exploit class — must now be caught by rebuild divergence).
    let old_id = prior.prior_id().to_owned();
    let inflated = text
        .replace(&old_id, "prior_id")
        .replace("\"ppb\":5", "\"ppb\":9");
    // Re-derive the id over the falsified placeholder bytes so the forgery
    // is fully self-consistent. Seal ONLY the value occurrence — the bytes
    // contain "prior_id":"prior_id" (key and placeholder value); a
    // replace-all of the bare token would corrupt the key and fail parsing
    // before reaching the rebuild gate.
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"oc-03-prior-v1\0");
    hasher.update(inflated.as_bytes());
    let forged_id = format!("ocprior1_{}", hasher.finalize().to_hex());
    let sealed = inflated.replace(
        "\"prior_id\":\"prior_id\"",
        &format!("\"prior_id\":\"{forged_id}\""),
    );
    assert_ne!(&sealed, &text);
    assert!(
        verify_prior(sealed.as_bytes(), &sessions, &reports, &events, &config).is_err(),
        "self-consistent forged vector rejected by rebuild"
    );

    // Forgery 2: wrong-but-well-formed prior_id.
    let forged2 = text.replace(
        &format!("\"{old_id}\""),
        "\"ocprior1_0000000000000000000000000000000000000000000000000000000000000000\"",
    );
    assert_ne!(&forged2, &text);
    assert!(
        verify_prior(forged2.as_bytes(), &sessions, &reports, &events, &config).is_err(),
        "forged id rejected"
    );
}

#[test]
fn mixed_terminal_rejected() {
    // A06: mixed terminal statuses across reports → derive_seeds Err
    // (no partial artifact).
    let config = PriorConfigV1::default();
    let terminal =
        ReportContribution::from_report_bytes(report_json("r1", "computed", &[]).as_bytes())
            .unwrap();
    // Craft an unterminated envelope directly (the parser stores
    // terminal_status per report).
    let unterm_json = report_json("r2", "computed", &[]).replace(
        "\"terminal_status\":\"terminal\"",
        "\"terminal_status\":\"unterminated\"",
    );
    let mixed = ReportContribution::from_report_bytes(unterm_json.as_bytes()).unwrap();
    let events: Vec<(&str, &str)> = vec![("e", "alpha")];
    let result = derive_seeds(&[terminal, mixed], &events, &config);
    assert!(result.is_err(), "mixed terminal statuses rejected");
    // And assembly with an unknown spelling is rejected too.
    let sessions = vec![SessionPayloads::from_payloads(vec!["alpha beta"])];
    let graph = build_entity_graph(&sessions, &config).unwrap();
    let seeds = PriorSeedSetV1::new_for_test(
        vec![PriorSeedV1::new_for_test("alpha", 100_000_000)],
        vec![],
        0,
        config.config_hash().expect("cfg"),
    );
    let ppr = run_ppr(&graph, &seeds, &config).expect("ppr");
    assert!(
        assemble_prior(graph, seeds, &ppr, 0, "bogus_status", &config).is_err(),
        "unknown status rejected at assembly"
    );
}

#[test]
fn golden_prior_fixture_immutable() {
    // A07: the committed fixture bytes verify against its true inputs.
    let (sessions, reports, events) = fixture_inputs();
    let config = PriorConfigV1::default();
    let bytes = std::fs::read("tests/fixtures/oc03-prior-v1-golden.json")
        .expect("committed golden fixture");
    assert!(
        verify_prior(&bytes, &sessions, &reports, &events, &config).is_ok(),
        "golden verifies against rebuilt pipeline"
    );
}

#[test]
fn golden_fixture_sha256() {
    // A08: sha256sum of the fixture equals the sidecar.
    let bytes = std::fs::read("tests/fixtures/oc03-prior-v1-golden.json").expect("fixture");
    let sidecar =
        std::fs::read_to_string("tests/fixtures/oc03-prior-v1-golden.sha256").expect("sidecar");
    use std::fmt::Write as _;
    let mut hex = String::new();
    for b in sha256_of(&bytes) {
        write!(hex, "{b:02x}").expect("write");
    }
    assert_eq!(hex, sidecar.trim(), "sidecar matches fixture bytes");
}

/// SHA-256 via the system sha256sum (POSIX), mirroring the OC-02 2H
/// precedent.
fn sha256_of(bytes: &[u8]) -> Vec<u8> {
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sha256sum available");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(bytes)
        .expect("pipe");
    let out = child.wait_with_output().expect("wait");
    let text = String::from_utf8(out.stdout).expect("utf8");
    (0..32)
        .map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

#[test]
fn unverified_report_rejected() {
    // A09: structurally malformed report bytes → parser Err before any
    // artifact exists; verification scope is the OC-02 caller contract
    // (verify_report runs upstream) — this crate's parser fails closed on
    // non-JSON and non-object envelopes.
    let bad = b"{not json";
    assert!(ReportContribution::from_report_bytes(bad).is_err());
    assert!(ReportContribution::from_report_bytes(b"[]").is_err());
    // A valid artifact verified with a WRONG report set (falsified inputs)
    // diverges → Err.
    let (sessions, _reports, events) = fixture_inputs();
    let config = PriorConfigV1::default();
    let prior = build_fixture();
    let bytes = prior.canonical_bytes().expect("canonical");
    let wrong_reports = vec![
        ReportContribution::from_report_bytes(
            report_json("rX", "computed", &[("evt-a", 999_999)]).as_bytes(),
        )
        .unwrap(),
    ];
    assert!(
        verify_prior(&bytes, &sessions, &wrong_reports, &events, &config).is_err(),
        "artifact verified against falsified inputs diverges"
    );
}

#[test]
fn empty_inputs_verification() {
    // Re-check W2: the empty-inputs edge is pinned. An artifact assembled
    // from zero reports/sessions rebuilds deterministically (empty seeds →
    // zero teleport → empty vector) and verifies Ok; the same artifact
    // verified against NON-empty inputs diverges → Err.
    let config = PriorConfigV1::default();
    let graph = build_entity_graph(&[], &config).expect("empty graph");
    let (seeds, dropped) = derive_seeds(&[], &[], &config).expect("empty seeds");
    let ppr = run_ppr(&graph, &seeds, &config).expect("empty ppr");
    let prior =
        assemble_prior(graph, seeds, &ppr, dropped, "terminal", &config).expect("empty assemble");
    assert!(prior.vector().is_empty(), "vacuous artifact has no claims");
    let bytes = prior.canonical_bytes().expect("canonical");
    assert!(verify_prior(&bytes, &[], &[], &[], &config).is_ok());
    let (sessions, reports, events) = fixture_inputs();
    assert!(
        verify_prior(&bytes, &sessions, &reports, &events, &config).is_err(),
        "empty artifact diverges against non-empty inputs"
    );
}

#[test]
fn noncanonical_spelling_rejected() {
    // A10: reordered members parse fine but fail the canonical gate.
    let (sessions, reports, events) = fixture_inputs();
    let config = PriorConfigV1::default();
    let prior = build_fixture();
    let bytes = prior.canonical_bytes().expect("canonical");
    let text = String::from_utf8(bytes.to_vec()).expect("utf8");
    // Top-level members only: "converged" and "dropped_seeds" are adjacent
    // (never nested); swap their segments with a single separating comma.
    let cb = "\"converged\":";
    let pc = text.find(cb).expect("converged");
    let ec = text[pc..].find(',').expect("comma") + pc;
    let pd = ec + 1; // dropped_seeds directly follows
    let db = "\"dropped_seeds\":";
    assert!(text[pd..].starts_with(db), "adjacent members");
    let ed = text[pd..].find(',').expect("comma2") + pd;
    let first = text[pc..ec].to_owned(); // "converged":true (no comma)
    let second = text[pd..ed].to_owned(); // "dropped_seeds":0 (no comma)
    let reordered = format!(
        "{prefix}{second},{first}{suffix}",
        prefix = &text[..pc],
        suffix = &text[ed..]
    );
    assert_ne!(&reordered, &text, "reorder changed bytes");
    assert!(
        parse_prior_bytes(reordered.as_bytes()).is_ok(),
        "parses leniently"
    );
    assert!(
        verify_prior(reordered.as_bytes(), &sessions, &reports, &events, &config).is_err(),
        "non-canonical rejected"
    );
}

/// Golden fixture generator (#[ignore]; regenerate only via explicit
/// `cargo test -- --ignored golden_generator` after change control).
#[test]
#[ignore = "golden fixture: change-control gate; run explicitly"]
fn golden_generator() {
    let prior = build_fixture();
    let bytes = prior.canonical_bytes().expect("canonical");
    std::fs::create_dir_all("tests/fixtures").expect("dir");
    std::fs::write("tests/fixtures/oc03-prior-v1-golden.json", &bytes).expect("write");
    use std::fmt::Write as _;
    let mut hex = String::new();
    for b in sha256_of(&bytes) {
        write!(hex, "{b:02x}").expect("write");
    }
    std::fs::write("tests/fixtures/oc03-prior-v1-golden.sha256", hex).expect("write");
}
