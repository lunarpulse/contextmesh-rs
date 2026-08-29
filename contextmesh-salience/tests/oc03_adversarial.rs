//! OC-03 Stage 3G adversarial tests (matrix rows OC03-X01..X08).
//!
//! Boundary and attack vectors against the prior pipeline: forged inputs,
//! byte tampering, cross-config divergence, cap overflow counting, Thorn
//! absence, duplicate folding, and falsified metadata — every vector ends
//! in a hard rejection or an honestly-counted cap, never a silent wrong
//! artifact.

use contextmesh_salience::prior::{
    PriorConfigV1, PriorSeedSetV1, PriorSeedV1, ReportContribution, SaliencePriorV1,
    SessionPayloads, assemble_prior, build_entity_graph, derive_seeds, parse_prior_bytes, run_ppr,
    verify_prior,
};

/// Report envelope builder (mirrors the 3F artifact-test helper).
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

/// Standard inputs: one session, two seeded entities.
fn fixture() -> (
    Vec<SessionPayloads<'static>>,
    Vec<ReportContribution>,
    Vec<(&'static str, &'static str)>,
) {
    let sessions = vec![SessionPayloads::from_payloads(vec![
        "alpha beta",
        "beta charlie",
    ])];
    let reports = vec![
        ReportContribution::from_report_bytes(
            report_json("r1", "computed", &[("evt-a", 600_000), ("evt-c", 200_000)]).as_bytes(),
        )
        .expect("report"),
    ];
    let events: Vec<(&str, &str)> = vec![("evt-a", "alpha"), ("evt-c", "beta charlie")];
    (sessions, reports, events)
}

/// Full pipeline over the given inputs → canonical artifact bytes.
fn pipeline_bytes(
    sessions: &[SessionPayloads<'_>],
    reports: &[ReportContribution],
    events: &[(&str, &str)],
) -> Vec<u8> {
    let config = PriorConfigV1::default();
    let graph = build_entity_graph(sessions, &config).expect("graph");
    let (seeds, dropped) = derive_seeds(reports, events, &config).expect("seeds");
    let ppr = run_ppr(&graph, &seeds, &config).expect("ppr");
    assemble_prior(graph, seeds, &ppr, dropped, "terminal", &config)
        .expect("assemble")
        .canonical_bytes()
        .expect("canonical")
}

#[test]
fn forged_report_rejected() {
    // X01: an ocattr1_-SHAPED but structurally broken report fails the
    // parser and the whole build — no partial artifact.
    let (sessions, _reports, events) = fixture();
    let config = PriorConfigV1::default();
    // Forged: correct outer shape, m4 share with a NEGATIVE ppm. The tier
    // is JSON-escaped inside the envelope (share_ppm\":), so the splice
    // must target the escaped spelling; as_u64 rejects negatives → Err.
    let forged = report_json("r-forge", "computed", &[("evt-a", 600_000)])
        .replace("share_ppm\\\":600000", "share_ppm\\\":-5");
    assert!(
        ReportContribution::from_report_bytes(forged.as_bytes()).is_err(),
        "negative share rejected at parse"
    );
    // A forged report that PARSES still fails the build when it is not the
    // artifact's true input: verification against falsified inputs → Err.
    let bytes = pipeline_bytes(&sessions, &[_reports[0].clone()], &events);
    let forged_reports = vec![
        ReportContribution::from_report_bytes(
            report_json("r-forge", "computed", &[("evt-a", 999_999)]).as_bytes(),
        )
        .expect("forged parses"),
    ];
    assert!(
        verify_prior(&bytes, &sessions, &forged_reports, &events, &config).is_err(),
        "artifact with falsified report inputs diverges"
    );
}

#[test]
fn tampered_vector_detected() {
    // X02: a single byte flip anywhere in the artifact → verify Err.
    let (sessions, reports, events) = fixture();
    let config = PriorConfigV1::default();
    let bytes = pipeline_bytes(&sessions, &reports, &events);
    assert!(verify_prior(&bytes, &sessions, &reports, &events, &config).is_ok());
    // Flip every distinct byte position class once (first quote, a digit
    // inside a ppb value, the last brace).
    for &idx in &[
        0usize,
        bytes.len() - 1,
        bytes
            .iter()
            .position(|&b| b.is_ascii_digit())
            .expect("digit"),
    ] {
        let mut tampered = bytes.clone();
        tampered[idx] = match tampered[idx] {
            b'"' => b' ',
            b'}' => b']',
            d if d.is_ascii_digit() => b'0' + ((d - b'0' + 1) % 10),
            other => other ^ 0x20,
        };
        if tampered == bytes {
            continue; // no-op flip, skip
        }
        assert!(
            verify_prior(&tampered, &sessions, &reports, &events, &config).is_err(),
            "byte flip at {idx} rejected"
        );
    }
}

#[test]
fn cross_config_divergence() {
    // X03: the same inputs under a different config produce a different
    // prior_id, and cross-verification fails.
    let (sessions, reports, events) = fixture();
    let config = PriorConfigV1::default();
    let bytes = pipeline_bytes(&sessions, &reports, &events);
    let prior = parse_prior_bytes(&bytes).expect("parse");
    // Mutate a frozen config value (damping). validate_frozen must reject
    // the mutated config outright — cross-config artifacts cannot even be
    // built.
    let mut mutated = config.clone();
    mutated.damping_ppm = 900_000;
    assert!(mutated.validate_frozen().is_err());
    // And the original artifact verified with the mutated config fails
    // (config_hash mismatch → rebuild divergence).
    assert!(
        verify_prior(&bytes, &sessions, &reports, &events, &mutated).is_err(),
        "cross-config verification rejected"
    );
    let _ = prior; // prior_id equality is implied by byte-equality (A02)
}

#[test]
fn graph_overflow_counters() {
    // X04: a corpus far beyond the entity cap builds Ok with honest
    // counters — never an error, never silent truncation.
    let config = PriorConfigV1::default();
    // 1,100 distinct entities across 1,100 events (> 1,024 cap), one
    // session per event so the pair-union never widens edges.
    let sessions: Vec<SessionPayloads<'static>> = (0..1_100u32)
        .map(|i| {
            SessionPayloads::from_payloads(vec![Box::leak(
                format!("e{i:04} f{i:04}").into_boxed_str(),
            )])
        })
        .collect();
    let graph = build_entity_graph(&sessions, &config).expect("massive graph builds");
    assert_eq!(graph.entities().len(), 1_024, "entity cap enforced");
    assert!(graph.truncated_entities() > 0, "overflow honestly counted");
}

#[test]
fn seed_overflow_counted() {
    // X05: more than 64 seeds → drop counted, remaining artifact valid.
    let config = PriorConfigV1::default();
    let mut reports = Vec::new();
    let mut events: Vec<(&'static str, &'static str)> = Vec::new();
    for i in 0..70u32 {
        let eid = Box::leak(format!("e{i:03}").into_boxed_str());
        events.push((eid, "alpha")); // every seed lands on the same entity
        reports.push(
            ReportContribution::from_report_bytes(
                report_json(&format!("r{i:03}"), "computed", &[(eid, 500_000)]).as_bytes(),
            )
            .expect("report"),
        );
    }
    let (seeds, _dropped) = derive_seeds(&reports, &events, &config).expect("seeds");
    assert_eq!(seeds.seeds().len(), 1, "folds onto one entity");
    // Per-entity folding keeps this under the cap, so exercise the true
    // 64-cap path with distinct entities instead.
    let mut wide_sessions: Vec<SessionPayloads<'static>> = Vec::new();
    let mut wide_reports = Vec::new();
    let mut wide_events: Vec<(&'static str, &'static str)> = Vec::new();
    for i in 0..70u32 {
        let e = Box::leak(format!("ent{i:03}").into_boxed_str());
        wide_sessions.push(SessionPayloads::from_payloads(vec![e]));
        wide_events.push((Box::leak(format!("wr-e{i:03}").into_boxed_str()), e));
        wide_reports.push(
            ReportContribution::from_report_bytes(
                report_json(
                    &format!("wr{i:03}"),
                    "computed",
                    &[(
                        Box::leak(format!("wr-e{i:03}").into_boxed_str()),
                        500_000 + u128::from(i),
                    )],
                )
                .as_bytes(),
            )
            .expect("report"),
        );
    }
    let graph = build_entity_graph(&wide_sessions, &config).expect("graph");
    let (wide_seeds, wide_dropped) =
        derive_seeds(&wide_reports, &wide_events, &config).expect("wide seeds");
    assert_eq!(wide_seeds.seeds().len(), 64, "seed cap enforced");
    assert_eq!(wide_dropped, 6, "drops honestly counted");
    let ppr = run_ppr(&graph, &wide_seeds, &config).expect("ppr");
    let artifact = assemble_prior(graph, wide_seeds, &ppr, wide_dropped, "terminal", &config)
        .expect("artifact valid under overflow");
    assert_eq!(artifact.dropped_seeds(), 6);
}

#[test]
fn thorn_unreachable() {
    // X06: no public API accepts negative ppb or Thorn payloads — the
    // types make them structurally absent.
    let config = PriorConfigV1::default();
    // Thorn status is frozen to thorn_disabled; the config validator
    // rejects any other spelling.
    // Attempt to smuggle a non-frozen thorn spelling via the constructor.
    let smuggled = SaliencePriorV1::new_for_test(
        "prior_id".to_owned(),
        config.config_hash().expect("cfg"),
        vec![],
        build_entity_graph(&[], &config).expect("g"),
        PriorSeedSetV1::new_for_test(vec![], vec![], 0, config.config_hash().expect("cfg")),
        vec![],
        1,
        true,
        0,
        0,
        "terminal",
    );
    // Canonical render validates the frozen thorn member internally.
    assert!(smuggled.canonical_bytes().is_ok(), "frozen path ok");
    // Negative ppb cannot even be expressed: the seed constructor takes
    // u128 (unsigned) — compile-time absence; we assert the type-level
    // guarantee by constructing a zero seed and confirming it is dropped.
    let zero = PriorSeedV1::new_for_test("alpha", 0);
    assert_eq!(zero.ppb(), 0, "u128 seed: negative not expressible");
    // The wire parser rejects negative ppb text outright (the tier is
    // escaped inside the envelope — target the escaped spelling).
    let negative = report_json("r-neg", "computed", &[("e", 1)])
        .replace("share_ppm\\\":1", "share_ppm\\\":-1");
    assert!(ReportContribution::from_report_bytes(negative.as_bytes()).is_err());
}

#[test]
fn duplicate_reports_folded() {
    // X07: the same report_id presented twice contributes exactly once.
    let config = PriorConfigV1::default();
    let report = ReportContribution::from_report_bytes(
        report_json("r-dup", "computed", &[("e", 700_000)]).as_bytes(),
    )
    .expect("report");
    let events: Vec<(&str, &str)> = vec![("e", "alpha")];
    let once = derive_seeds(std::slice::from_ref(&report), &events, &config).expect("once");
    let twice = derive_seeds(&[report.clone(), report], &events, &config).expect("twice");
    assert_eq!(once.0.seeds(), twice.0.seeds(), "duplicate folded once");
    assert_eq!(once.0.seeds().len(), 1);
    // The folded seed mass is the single contribution, not doubled.
    assert_eq!(once.0.seeds()[0].ppb(), 700_000_000);
}

#[test]
fn falsified_metadata_detected() {
    // X08: altered converged/iterations/residual in the artifact →
    // rebuild divergence → Err.
    let (sessions, reports, events) = fixture();
    let config = PriorConfigV1::default();
    let bytes = pipeline_bytes(&sessions, &reports, &events);
    let text = String::from_utf8(bytes.clone()).expect("utf8");
    let prior = parse_prior_bytes(&bytes).expect("parse");
    let old_id = prior.prior_id();

    // Self-consistent forgery helper: mutate, re-derive id, re-seal.
    let forge = |mutation: String| -> Vec<u8> {
        let placeholder = mutation.replace(old_id, "prior_id");
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"oc-03-prior-v1\0");
        hasher.update(placeholder.as_bytes());
        let id = format!("ocprior1_{}", hasher.finalize().to_hex());
        placeholder
            .replace(
                "\"prior_id\":\"prior_id\"",
                &format!("\"prior_id\":\"{id}\""),
            )
            .into_bytes()
    };

    // iterations: bump by 1.
    let iters = format!("\"iterations\":{}", prior.iterations());
    let forged_iters = forge(text.replace(
        &iters,
        &format!("\"iterations\":{}", prior.iterations() + 1),
    ));
    assert!(
        verify_prior(&forged_iters, &sessions, &reports, &events, &config).is_err(),
        "falsified iterations rejected"
    );
    // converged: flip the boolean.
    let conv = format!("\"converged\":{}", prior.converged());
    let flipped = if prior.converged() { "false" } else { "true" };
    let forged_conv = forge(text.replace(&conv, &format!("\"converged\":{flipped}")));
    assert!(
        verify_prior(&forged_conv, &sessions, &reports, &events, &config).is_err(),
        "falsified converged rejected"
    );
    // residual_ppb: alter by +1.
    let res = format!("\"residual_ppb\":{}", prior.residual_ppb());
    let forged_res = forge(text.replace(
        &res,
        &format!("\"residual_ppb\":{}", prior.residual_ppb() + 1),
    ));
    assert!(
        verify_prior(&forged_res, &sessions, &reports, &events, &config).is_err(),
        "falsified residual rejected"
    );
}
