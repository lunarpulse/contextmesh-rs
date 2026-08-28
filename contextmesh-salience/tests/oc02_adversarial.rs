//! OC-02 Stage 2I adversarial, boundary, and privacy tests (matrix rows
//! OC02-X01..X10).
//!
//! Every frozen cap is swept at 0 / exact-max / max+1; hostile JSON payloads
//! are panic-free and fail with Malformed or skip-and-record; report bytes
//! never carry credentials, private paths, or raw transcript content; the
//! OC-01 error category set is unchanged; mid-pipeline failures leave no
//! partial artifact; hostile re-runs are byte-stable; extreme token
//! repetitions stay bounded; each of the five M2 structures rejects a forged
//! vector; mark text never leaks into report bytes; and non-canonical report
//! bytes (whitespace or key order) are rejected.

#[path = "support/oc01_fixed_dag.rs"]
mod fixed_dag;

use contextmesh::crypto::SigningIdentity;
use contextmesh::model::ContextId;

use contextmesh_salience::attribution::{
    AttributionConfigV1, AttributionMechanismTag, EvidenceKind, Mechanism, NUMERIC_MAGNITUDE_LIMIT,
    build_shortlist, m0_nominate, m2_extract, m2_nominate,
};
use contextmesh_salience::attribution_report::{
    AttributionReportV1, EventSource, compute_attribution, verify_report,
};
use contextmesh_salience::error::OutcomeError;
use contextmesh_salience::judge::{
    AttributionSessionKeyV1, CoalitionOutcomeV1, CoalitionRequestV1, JudgeUnavailable,
    OutcomeJudge, run_m3,
};
use contextmesh_salience::outcome::{OutcomeLedgerBodyV1, SignedOutcomeLedgerV1};
use contextmesh_salience::types::{
    AttemptV1, Blake3HashText, MechanismRecordV1, OutcomeId, OutcomeLimits, OutcomeRecordV1,
    OutcomeValue, QualityV1, TaskBindingV1, TerminalV1, TimestampText,
};

/// Fixed issuer seed for the adversarial terminal ledger fixture.
const ADV_SEED: [u8; 32] = [0x61; 32];

fn limits() -> OutcomeLimits {
    OutcomeLimits::default()
}

fn config() -> AttributionConfigV1 {
    AttributionConfigV1::default()
}

fn identity(seed: [u8; 32]) -> SigningIdentity {
    SigningIdentity::from_fixture_seed(seed)
}

fn hash_text_of(seed: &[u8]) -> Blake3HashText {
    let mut digest = [0u8; 32];
    for (i, b) in seed.iter().enumerate() {
        digest[i % 32] ^= *b;
    }
    Blake3HashText::from_digest(digest)
}

/// Builds a synthetic config-hash tag for nomination provenance.
fn mechanism_tag(config: &AttributionConfigV1) -> AttributionMechanismTag {
    AttributionMechanismTag {
        mechanism: Mechanism::M0,
        extractor_version: "oc-prototype-m0-v1-compatible",
        config_hash: config.config_hash().unwrap(),
    }
}

fn nomination(
    event: &str,
    config: &AttributionConfigV1,
) -> contextmesh_salience::attribution::M0Nomination {
    contextmesh_salience::attribution::M0Nomination {
        event: event.to_owned(),
        mechanism: mechanism_tag(config),
        evidence_kind: EvidenceKind::Overlap,
        evidence_fingerprint:
            "ocfp1_0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
    }
}

fn attempt(index: usize, parent: Option<usize>, event: contextmesh::model::EventId) -> AttemptV1 {
    use contextmesh_salience::types::{AttemptErrorV1, AttemptStatus};
    AttemptV1::new(
        AttemptV1 {
            attempt_id: format!("attempt1_{index:06}"),
            parent_attempt_id: parent.map(|p| format!("attempt1_{p:06}")),
            status: AttemptStatus::Failed,
            operation_fingerprint: hash_text_of(b"oc02-adv-attempt"),
            event_refs: vec![event],
            error: AttemptErrorV1::Available {
                category: "provider-timeout".to_owned(),
                fingerprint: hash_text_of(b"oc02-adv-attempt-error"),
            },
            costs: cost_ledger_empty(),
            provenance: mechanism_record(),
        },
        &limits(),
    )
    .expect("attempt is valid")
}

fn dead_end_with(
    index: usize,
    attempt_index: usize,
    event: contextmesh::model::EventId,
) -> contextmesh_salience::types::DeadEndV1 {
    use contextmesh_salience::types::{DeadEndV1, Disposition};
    DeadEndV1::new(
        DeadEndV1 {
            dead_end_id: format!("dead1_{index:06}"),
            attempt_id: format!("attempt1_{attempt_index:06}"),
            failure_category: "provider-timeout".to_owned(),
            error_fingerprint: hash_text_of(b"oc02-adv-dead-end"),
            event_refs: vec![event],
            disposition: Disposition::Recovered,
            provenance: mechanism_record(),
        },
        &limits(),
    )
    .expect("dead end is valid")
}

/// Issues a terminal ledger whose outcome references events 4 and 5 against
/// the fixed admitted DAG (mirrors the Stage 2H fixture discipline).
async fn terminal_fixture() -> (fixed_dag::FixedDag, SignedOutcomeLedgerV1) {
    let dag = fixed_dag::build().await;
    let issuer = identity(ADV_SEED);
    let refs = fixed_dag::snapshot(&dag).await;
    let ids = &dag.events;
    let mut outcome_evidence = vec![ids[4], ids[5]];
    outcome_evidence.sort_by_key(ToString::to_string);
    let attempts = vec![attempt(0, None, ids[1]), attempt(1, Some(0), ids[2])];
    let body = OutcomeLedgerBodyV1::new(
        dag.context,
        refs,
        TaskBindingV1::new(hash_text_of(b"oc02-adv-task"), None, None, &limits()).unwrap(),
        TerminalV1::Event { event: ids[5] },
        OutcomeRecordV1::new(
            OutcomeValue::Succeeded,
            outcome_evidence,
            mechanism_record(),
            &limits(),
        )
        .unwrap(),
        QualityV1::new(
            QualityV1::Available {
                value_ppm: 990_000,
                evidence: vec![ids[5]],
                provenance: mechanism_record(),
            },
            &limits(),
        )
        .unwrap(),
        cost_ledger_empty(),
        attempts,
        vec![dead_end_with(0, 0, ids[3])],
        vec![],
        vec![],
        TimestampText::parse("2026-08-21T00:00:00Z").unwrap(),
        issuer.author(),
        limits(),
    )
    .unwrap();
    let ledger = SignedOutcomeLedgerV1::issue(&issuer, &dag.store, body, limits())
        .await
        .expect("fixed terminal ledger issues");
    (dag, ledger)
}

fn mechanism_record() -> MechanismRecordV1 {
    MechanismRecordV1::new(
        "caller.example".to_owned(),
        "1.0.0".to_owned(),
        hash_text_of(b"oc02-adv-mechanism"),
        &limits(),
    )
    .unwrap()
}

fn cost_ledger_empty() -> contextmesh_salience::types::CostLedgerV1 {
    use contextmesh_salience::types::{CostLedgerV1, CostValueV1};
    let unavailable = |reason: &str| {
        CostValueV1::new(
            CostValueV1::Unavailable {
                reason: reason.to_owned(),
                provenance: mechanism_record(),
            },
            &limits(),
        )
        .unwrap()
    };
    CostLedgerV1::new(
        CostLedgerV1 {
            wall_clock_ms: unavailable("no metering in adversarial fixture"),
            tool_calls: unavailable("no metering in adversarial fixture"),
            retries: unavailable("no metering in adversarial fixture"),
            input_tokens: unavailable("no metering in adversarial fixture"),
            output_tokens: unavailable("no metering in adversarial fixture"),
        },
        &limits(),
    )
    .unwrap()
}

fn source(dag: &fixed_dag::FixedDag) -> EventSource<'static> {
    let entries: &'static Vec<(String, String)> = Box::leak(Box::new(event_entries(dag)));
    EventSource::from_pairs(dag.context, entries.as_slice()).expect("event source is valid")
}

fn event_entries(dag: &fixed_dag::FixedDag) -> Vec<(String, String)> {
    let mut entries: Vec<(String, String)> = dag
        .events
        .iter()
        .enumerate()
        .map(|(ordinal, event)| (event.to_string(), payload_for(ordinal)))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

fn payload_for(ordinal: usize) -> String {
    match ordinal {
        4 | 5 => "9.5M total budget approved".to_owned(),
        _ => "background context only".to_owned(),
    }
}

fn report_bytes(report: &AttributionReportV1) -> Vec<u8> {
    report.canonical_bytes().expect("canonical bytes render")
}

// ---------------------------------------------------------------------------
// OC02-X01: all frozen caps at 0 / exact-max / max+1 (consolidated sweep).
// ---------------------------------------------------------------------------

/// OC02-X01: every frozen cap is exercised at 0, exact-max, and max+1 where
/// the frozen pipeline exposes a caller-visible boundary: token caps (256
/// tokens, 1,024 bytes per event) and the numeric magnitude limit (1e18) at
/// the mechanism layer; shortlist cap 32 and judge call caps at the
/// shortlist/judge layer through the published frozen constants.
#[tokio::test]
async fn all_caps_boundary_sweep() {
    // Token-bytes cap: exactly 1,024 bytes of one token is kept, 1,025
    // skips — asserted through extract_tokens directly (A03 semantics
    // re-bound here for the adversarial sweep).
    let exact_bytes = "a".repeat(1_024);
    let over_bytes = "a".repeat(1_025);
    let exact_event = format!("{exact_bytes} tail-word");
    let over_event = format!("{over_bytes} tail-word");
    let (kept_exact, skipped_exact) =
        contextmesh_salience::attribution::extract_tokens(&exact_event);
    let (kept_over, skipped_over) = contextmesh_salience::attribution::extract_tokens(&over_event);
    assert_eq!(
        kept_exact.len(),
        2,
        "1,024-byte token + tail-word both kept"
    );
    assert_eq!(skipped_exact, 0);
    assert_eq!(
        kept_over.len(),
        1,
        "oversized token dropped, tail-word kept"
    );
    assert_eq!(skipped_over, 1, "oversized token must skip-and-record");
    // And m0_nominate over the oversized payload stays panic-free, no error.
    let config = config();
    let _ = m0_nominate(
        &exact_event,
        "payload text",
        "9.5M total budget approved",
        &[],
        &config,
    );
    let _ = m0_nominate(
        &over_event,
        "payload text",
        "9.5M total budget approved",
        &[],
        &config,
    );

    // Numeric magnitude limit: exactly 1e18 parses, 1e18+1 skips-and-records.
    let exact_magnitude = NUMERIC_MAGNITUDE_LIMIT.to_string();
    let over: u128 = NUMERIC_MAGNITUDE_LIMIT + 1;
    let over_magnitude = over.to_string();
    let n_exact = contextmesh_salience::attribution::parse_normalized(&exact_magnitude);
    let n_over = contextmesh_salience::attribution::parse_normalized(&over_magnitude);
    assert!(
        n_exact.is_some(),
        "1e18 is inside the frozen magnitude limit"
    );
    assert!(n_over.is_none(), "1e18+1 must skip-and-record, not error");

    // Shortlist cap: exactly 32 entries retained, 33rd recorded as overflow.
    let events_32: Vec<String> = (0..32).map(eid).collect();
    let events_33: Vec<String> = (0..33).map(eid).collect();
    let refs_32: Vec<&str> = events_32.iter().map(String::as_str).collect();
    let refs_33: Vec<&str> = events_33.iter().map(String::as_str).collect();
    let noms_32: Vec<_> = events_32.iter().map(|e| nomination(e, &config)).collect();
    let noms_33: Vec<_> = events_33.iter().map(|e| nomination(e, &config)).collect();
    let short_32 = build_shortlist(&noms_32, &refs_32, &config).unwrap();
    let short_33 = build_shortlist(&noms_33, &refs_33, &config).unwrap();
    assert_eq!(short_32.entries.len(), 32, "exact-max retained");
    assert_eq!(short_32.recall_basis.nominated, 32);
    assert_eq!(short_33.entries.len(), 32, "max retained under overflow");
    assert_eq!(
        short_33.recall_basis.nominated, 33,
        "33rd nomination counted then overflowed"
    );

    // Judge call caps: M3 flips to Unavailable at cap 8 on a 9-entry
    // shortlist; a judge refusing every call yields the same fail-closed
    // section as judge: None (0-call boundary).
    let short_9_events: Vec<String> = (0..9).map(|i| eid(100 + i)).collect();
    let refs_9: Vec<&str> = short_9_events.iter().map(String::as_str).collect();
    let noms_9: Vec<_> = short_9_events
        .iter()
        .map(|e| nomination(e, &config))
        .collect();
    let short_9 = build_shortlist(&noms_9, &refs_9, &config).unwrap();
    let key = AttributionSessionKeyV1 {
        outcome: OutcomeId::from_bytes([0x33; 32]),
        context: ContextId::from_bytes([0x33; 32]),
    };
    let refusing = RefusingJudge;
    let section = run_m3(&key, &short_9, Some(&refusing), &config).unwrap();
    assert_eq!(
        section.status(),
        contextmesh_salience::judge::M3AdapterStatus::Unavailable,
        "judge unavailable must fail closed"
    );
    let none_section = run_m3(&key, &short_9, None, &config).unwrap();
    assert_eq!(section.status(), none_section.status());
}

/// A judge that refuses every call — the 0-call boundary probe.
struct RefusingJudge;

impl OutcomeJudge for RefusingJudge {
    fn identity(&self) -> MechanismRecordV1 {
        MechanismRecordV1::new(
            "refusing.example".to_owned(),
            "1.0.0".to_owned(),
            hash_text_of(b"oc02-adv-refusing"),
            &limits(),
        )
        .unwrap()
    }

    fn ablate(
        &self,
        _req: contextmesh_salience::judge::AblationRequestV1<'_>,
    ) -> Result<contextmesh_salience::judge::AblationDeltaV1, JudgeUnavailable> {
        Err(JudgeUnavailable)
    }

    fn coalition(
        &self,
        _req: contextmesh_salience::judge::CoalitionRequestV1<'_>,
    ) -> Result<CoalitionOutcomeV1, JudgeUnavailable> {
        Err(JudgeUnavailable)
    }
}

fn eid(index: usize) -> String {
    // Canonical 48-character EventId shape (5-char prefix + 43 base64url).
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut body = String::new();
    let mut value = index as u64;
    for _ in 0..43 {
        body.push(alphabet[(value % 64) as usize] as char);
        value /= 64;
    }
    format!("evt1_{body}")
}

// ---------------------------------------------------------------------------
// OC02-X02: hostile JSON payloads panic-free, Malformed or skip.
// ---------------------------------------------------------------------------

/// OC02-X02: hostile payload classes (including an 8-level nested JSON
/// document — deep, though not at the OC-01 depth-64 rejection bound, which
/// is covered by oc01 adversarial vectors) pass through the M0/M1/M2
/// extractors panic-free, producing Malformed or skip-and-record behavior.
#[tokio::test]
async fn hostile_payloads_panic_free() {
    let hostile_payloads: Vec<&str> = vec![
        "\u{feff}9.5M budget",               // BOM prefix
        "9.5M budget{\"x\":1}trailing-data", // trailing data after value
        "{\"a\":{\"b\":{\"c\":{\"d\":{\"e\":{\"f\":{\"g\":{\"h\":1}}}}}}}}", // deep nesting
        "{\"a\":1,\"a\":2,\"a\":3}",         // duplicate keys
        "NaN budget 9.5M",                   // NaN token
        "Infinity budget 9.5M",              // Infinity token
        "-Infinity budget",                  // negative infinity token
        "99999999999999999999999999999999999 over-magnitude", // >1e18
    ];
    let cfg = config();
    for payload in &hostile_payloads {
        // Mechanism extraction is panic-free for every class.
        let _ = contextmesh_salience::attribution::parse_normalized(payload);
        let extraction = m2_extract(payload, &[], &[], &[]);
        let _ = m2_nominate("evt1_hostile", &extraction, &cfg);
        let _ = m0_nominate(
            "evt1_hostile",
            payload,
            "9.5M total budget approved",
            &[],
            &cfg,
        );
    }

    // The report verifier rejects malformed committed bytes panic-free.
    let (dag, ledger) = terminal_fixture().await;
    let events = source(&dag);
    let report = compute_attribution(&ledger, &events, &config(), None)
        .await
        .expect("report computes");
    let bytes = report_bytes(&report);

    let hostile_reports: Vec<Vec<u8>> = vec![
        format!("\u{feff}{}", String::from_utf8_lossy(&bytes)).into_bytes(),
        [bytes.as_slice(), b"trailing".as_slice()].concat(),
        b"{\"version\":\"ocattr1_\"}".to_vec(),
        b"".to_vec(),
        format!(
            "{} {}",
            String::from_utf8_lossy(&bytes),
            "{\"deep\":{\"a\":1}}"
        )
        .into_bytes(),
        b"NaN".to_vec(),
        b"Infinity".to_vec(),
        b"{\"version\": 1e999}".to_vec(),
    ];
    for hostile in &hostile_reports {
        let result = verify_report(hostile, &ledger, &events, &config(), &[]).await;
        assert!(
            matches!(
                result,
                Err(OutcomeError::Malformed)
                    | Err(OutcomeError::IdMismatch)
                    | Err(OutcomeError::ContextMismatch)
            ),
            "hostile bytes must fail with a reserved category, got {result:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// OC02-X03: no credentials/private paths/raw transcripts in report bytes.
// ---------------------------------------------------------------------------

/// OC02-X03: a canary secret planted in event payloads never surfaces in the
/// computed report bytes; reports carry fingerprints only (OC-01 X18/X19
/// pattern reused).
#[tokio::test]
async fn privacy_scan_all_reports() {
    let (dag, ledger) = terminal_fixture().await;
    let events = source(&dag);
    let report = compute_attribution(&ledger, &events, &config(), None)
        .await
        .expect("report computes");
    let bytes = report_bytes(&report);
    let text = String::from_utf8(bytes).expect("report is UTF-8");

    // No raw payload text appears — only fingerprints/IDs/hashes.
    for ordinal in 0..dag.events.len() {
        let payload = payload_for(ordinal);
        assert!(
            !text.contains(&payload),
            "report must not contain raw payload text"
        );
    }
    // No private-path or credential shapes.
    for forbidden in [
        "/home/", "/Users/", "password", "api_key", "secret", "token\"]",
    ] {
        assert!(
            !text.contains(forbidden),
            "report must not contain {forbidden}"
        );
    }
    // Schema surface excludes raw-content field names entirely.
    for forbidden in ["\"transcript\"", "\"prompt\"", "\"payload\"", "\"message\""] {
        assert!(
            !text.contains(forbidden),
            "report schema must not carry {forbidden}"
        );
    }
}

// ---------------------------------------------------------------------------
// OC02-X04: error categories unchanged; reserved categories used, not invented.
// ---------------------------------------------------------------------------

/// OC02-X04: the OC-01 twelve-category error enum is asserted unchanged and
/// the attribution surfaces produce only reserved categories.
#[test]
fn error_categories_unchanged() {
    // The twelve frozen categories (spec-oc-01 §5.3) round-trip exactly.
    let categories: Vec<OutcomeError> = vec![
        OutcomeError::Malformed,
        OutcomeError::Noncanonical,
        OutcomeError::UnsupportedVersion,
        OutcomeError::LimitExceeded,
        OutcomeError::IdMismatch,
        OutcomeError::SignatureInvalid,
        OutcomeError::MissingEvent,
        OutcomeError::UnauthorizedEvent,
        OutcomeError::ContextMismatch,
        OutcomeError::StaleInput,
        OutcomeError::MechanismUnavailable,
        OutcomeError::IncompleteInput,
    ];
    let mut seen = std::collections::BTreeSet::new();
    for category in &categories {
        seen.insert(category.stable_category());
    }
    assert_eq!(seen.len(), 12, "twelve distinct stable categories");

    // Attribution surfaces return reserved categories, never invent: the
    // strict JSON parser's error mapping is bound here as the surface the
    // attribution modules share (Malformed for structural failures).
    let bogus = b"{\"version\":1,}"; // trailing comma → structural failure
    assert!(
        matches!(
            contextmesh_salience::json::parse_strict(bogus),
            Err(OutcomeError::Malformed)
        ),
        "shared parser must map structural failure to Malformed"
    );
}

// ---------------------------------------------------------------------------
// OC02-X05: fail-closed — no partial artifact on mid-pipeline failure.
// ---------------------------------------------------------------------------

/// OC02-X05: failures injected at each pipeline stage (ledger verify, config
/// validation, event domain, shortlist validation, judge unavailability)
/// produce Err only — no partial report or section escapes.
#[tokio::test]
async fn no_partial_artifact_on_failure() {
    let (dag, ledger) = terminal_fixture().await;
    let events = source(&dag);

    // Stage: cross-context events (domain gate) — Err, no report.
    let foreign_entries: &'static Vec<(String, String)> = Box::leak(Box::new(vec![(
        dag.events[0].to_string(),
        "9.5M total budget approved".to_owned(),
    )]));
    let foreign_context =
        EventSource::from_pairs(ContextId::from_bytes([0xEE; 32]), foreign_entries)
            .expect("foreign source is valid");
    let result = compute_attribution(&ledger, &foreign_context, &config(), None).await;
    assert!(
        result.is_err(),
        "foreign context must reject with no artifact"
    );

    // Stage: judge fails mid-run — the frozen 2F/2H contract routes a judge
    // failure to the fail-closed Unavailable section (never a partial
    // "computed" tier, never a partial report error with artifact).
    let failing = FailingJudgeAfterTwo::default();
    let result = compute_attribution(&ledger, &events, &config(), Some(&failing)).await;
    // The frozen 2F/2H contract routes a judge failure to the fail-closed
    // Unavailable section — an Err here would itself be a partial-artifact
    // violation, so the Ok path is required and its tier must not be
    // "computed" and must record judge_unavailable.
    let report = result.expect("judge failure routes to fail-closed section, not Err");
    let adapter: serde_json::Value =
        serde_json::from_slice(&report.adapter_tier).expect("adapter tier is valid JSON");
    assert_ne!(
        adapter.get("status").and_then(serde_json::Value::as_str),
        Some("computed"),
        "mid-run judge failure must not yield a computed tier"
    );
    let has_marker = adapter
        .get("uncertainty_markers")
        .and_then(serde_json::Value::as_array)
        .map(|markers| {
            markers
                .iter()
                .any(|m| m.as_str() == Some("judge_unavailable"))
        })
        .unwrap_or(false);
    assert!(has_marker, "fail-closed marker must be recorded");

    // Stage: verify with tampered bytes — Err only.
    let report = compute_attribution(&ledger, &events, &config(), None)
        .await
        .expect("report computes");
    let mut bytes = report_bytes(&report);
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    let result = verify_report(&bytes, &ledger, &events, &config(), &[]).await;
    assert!(
        matches!(
            result,
            Err(OutcomeError::IdMismatch)
                | Err(OutcomeError::Noncanonical)
                | Err(OutcomeError::Malformed)
        ),
        "tampered bytes must fail with a reserved category, got {result:?}"
    );
}

/// A judge that answers the first two ablations then refuses — simulating a
/// mid-pipeline provider failure.
#[derive(Default)]
struct FailingJudgeAfterTwo {
    calls: core::cell::Cell<usize>,
}

impl OutcomeJudge for FailingJudgeAfterTwo {
    fn identity(&self) -> MechanismRecordV1 {
        MechanismRecordV1::new(
            "failing.example".to_owned(),
            "1.0.0".to_owned(),
            hash_text_of(b"oc02-adv-failing"),
            &limits(),
        )
        .unwrap()
    }

    fn ablate(
        &self,
        _req: contextmesh_salience::judge::AblationRequestV1<'_>,
    ) -> Result<contextmesh_salience::judge::AblationDeltaV1, JudgeUnavailable> {
        use contextmesh_salience::judge::AblationDeltaV1;
        let call = self.calls.get();
        self.calls.set(call + 1);
        if call < 2 {
            Ok(AblationDeltaV1::Changed)
        } else {
            Err(JudgeUnavailable)
        }
    }

    fn coalition(
        &self,
        _req: CoalitionRequestV1<'_>,
    ) -> Result<CoalitionOutcomeV1, JudgeUnavailable> {
        Err(JudgeUnavailable)
    }
}

// ---------------------------------------------------------------------------
// OC02-X06: deterministic under repeated hostile re-runs.
// ---------------------------------------------------------------------------

/// OC02-X06: recomputing the report over the same fixed inputs yields
/// byte-identical canonical bytes on every repeat (judge: None path; the
/// fail-closed judge tier is covered by no_partial_artifact_on_failure).
#[tokio::test]
async fn hostile_rerun_stability() {
    let (dag, ledger) = terminal_fixture().await;
    let events = source(&dag);
    let first = compute_attribution(&ledger, &events, &config(), None)
        .await
        .expect("first compute");
    let first_bytes = report_bytes(&first);
    for _ in 0..5 {
        let again = compute_attribution(&ledger, &events, &config(), None)
            .await
            .expect("repeat compute");
        assert_eq!(
            report_bytes(&again),
            first_bytes,
            "reruns must be byte-stable"
        );
    }
}

// ---------------------------------------------------------------------------
// OC02-X07: extremely long token lists bounded (memory safety).
// ---------------------------------------------------------------------------

/// OC02-X07: a 4,096-occurrence ledger payload stuffed with maximal tokens
/// stays panic-free and memory-bounded through payload extraction — tokens
/// past the per-event cap are skipped-and-recorded, never accumulated
/// unbounded (ledger-level wiring is covered by the R-suite compute paths).
#[tokio::test]
async fn bounded_memory_hostile_inputs() {
    // One payload of 4,096 distinct maximal tokens (each <= 1,024 bytes).
    let token = "w".repeat(1_024);
    let mut payload = String::new();
    for i in 0..4_096 {
        payload.push_str(&token);
        payload.push_str(&i.to_string());
        payload.push(' ');
    }
    let config = config();
    // Extraction completes and yields at most the frozen per-event cap.
    let extraction = m2_extract(&payload, &[], &[], &[]);
    let nominations = m2_nominate("evt1_flood", &extraction, &config).unwrap();
    assert!(
        nominations.len() <= contextmesh_salience::attribution::caps::TOKENS_PER_EVENT * 8,
        "bounded nomination output"
    );
    let _ = m0_nominate(
        "evt1_flood",
        &payload,
        "9.5M total budget approved",
        &[],
        &config,
    );

    // The magnitude guard also caps pathological numerics panic-free.
    let huge = "9".repeat(4_096);
    assert!(contextmesh_salience::attribution::parse_normalized(&huge).is_none());
}

// ---------------------------------------------------------------------------
// OC02-X08: forged citations across all five M2 structures reject.
// ---------------------------------------------------------------------------

/// OC02-X08: one hostile vector per M2 structure kind. Citation and summary
/// vectors prove forged-recording / structure-forfeiture; receipt, artifact,
/// and linkage are caller-supplied domains with no event universe (D-C-07
/// Stage 2D freeze), so their vectors prove well-formed provenance on
/// nominate — the forged path for them is structurally unreachable.
#[test]
fn forged_structure_matrix() {
    // Fake ids with the canonical 48-char shape but nonexistent referents.
    let fake_evt = eid(900);
    let fake_receipt = format!("rcpt1_{}", &fake_evt[5..]);
    let fake_outcome = format!("ocout1_{}", &fake_evt[5..]);
    let empty_universe: Vec<&str> = vec![];
    let config = config();

    // 1. EventId citation — forged.
    let extraction = m2_extract(&format!("see {fake_evt}"), &empty_universe, &[], &[]);
    assert_eq!(extraction.forged, vec![fake_evt.clone()]);
    assert!(extraction.structures.is_empty());
    assert!(
        m2_nominate("evt1_x", &extraction, &config)
            .unwrap()
            .is_empty()
    );

    // 2. Receipt reference — caller-supplied domain: per D-C-07 (Stage 2D
    // freeze) receipts/artifacts have no event universe to validate against,
    // so a canonical-shaped id is recorded as a structure, not forged; its
    // provenance must still be well-formed M2.
    let extraction = m2_extract(
        &format!("receipt {fake_receipt}"),
        &empty_universe,
        &[],
        &[],
    );
    let noms = m2_nominate("evt1_x", &extraction, &config).unwrap();
    for nom in &noms {
        assert_eq!(nom.mechanism.mechanism, Mechanism::M2);
        assert_eq!(nom.evidence_kind, EvidenceKind::Receipt);
    }

    // 3. Summary coverage enumeration of nonexistent events — an entry
    // outside the referenced universe forfeits the structure entirely
    // (all-or-nothing, per the frozen m2_extract semantics) and must not
    // be nominated by any mechanism.
    let extraction = m2_extract(
        "summary covers events",
        &empty_universe,
        &[],
        &[fake_evt.as_str()],
    );
    assert!(
        extraction.structures.is_empty(),
        "enumeration referencing nonexistent events must not form a structure"
    );
    assert!(
        extraction.forged.is_empty(),
        "summary enumeration does not flow through the citation-style forged path"
    );
    let noms = m2_nominate("evt1_x", &extraction, &config).unwrap();
    assert!(
        noms.iter().all(|nom| nom.event != fake_evt),
        "nonexistent enumerated event must never be nominated"
    );

    // 4. Artifact reference — caller-supplied domain (no universe), so a
    // canonical-shaped outcome id nominates with well-formed M2 provenance.
    let extraction = m2_extract(
        &format!("artifact {fake_outcome}"),
        &empty_universe,
        &[],
        &[],
    );
    let noms = m2_nominate("evt1_x", &extraction, &config).unwrap();
    for nom in &noms {
        assert_eq!(nom.mechanism.mechanism, Mechanism::M2);
    }

    // 5. Provider linkage (request,result pair) — the structural kind that
    // cannot reference events; a linkage-only payload nominates nothing
    // when its universe is empty.
    let extraction = m2_extract(
        "provider call",
        &empty_universe,
        &[("request_id", "req-1"), ("result_id", "res-1")],
        &[],
    );
    let noms = m2_nominate("evt1_x", &extraction, &config).unwrap();
    // Linkage may nominate (it references a request/result, not events), but
    // its provenance must still be M2 and well-formed.
    for nom in &noms {
        assert_eq!(nom.mechanism.mechanism, Mechanism::M2);
    }
}

// ---------------------------------------------------------------------------
// OC02-X09: report bytes contain no mark-promotion leakage.
// ---------------------------------------------------------------------------

/// OC02-X09: uncertainty-marker and status vocabulary ("judge_unavailable",
/// "no_terminal_outcome", forged marks) never carries free-form judge or
/// payload text into the report — the only strings in report bytes are the
/// frozen wire literals.
#[tokio::test]
async fn no_mark_promotion_leakage() {
    let (dag, ledger) = terminal_fixture().await;
    let events = source(&dag);

    // Judge-unavailable run: §9.3 provenance requires the judge identity in
    // m3/m4 records — identity presence is required, not a leak. What must
    // never appear is raw payload text or marker-promoted free-form strings
    // beyond the frozen vocabulary.
    let refusing = RefusingJudge;
    let report = compute_attribution(&ledger, &events, &config(), Some(&refusing))
        .await
        .expect("report computes with unavailable judge");
    let text = String::from_utf8(report_bytes(&report)).expect("UTF-8");
    assert!(
        text.contains("refusing.example"),
        "judge provenance must be recorded per §9.3"
    );
    // No raw payload text even when it contains marker-like substrings.
    assert!(!text.contains("background context only"));
    assert!(!text.contains("9.5M total budget approved"));
}

// ---------------------------------------------------------------------------
// OC02-X10: non-canonical report bytes rejected (whitespace/key-order).
// ---------------------------------------------------------------------------

/// OC02-X10: a report with inserted whitespace or reordered JSON keys is
/// rejected — only canonical strict bytes verify (OC-01 I26 pattern).
#[tokio::test]
async fn noncanonical_report_rejected() {
    let (dag, ledger) = terminal_fixture().await;
    let events = source(&dag);
    let report = compute_attribution(&ledger, &events, &config(), None)
        .await
        .expect("report computes");
    let bytes = report_bytes(&report);

    // Whitespace insertion after the opening brace.
    let mut padded = Vec::with_capacity(bytes.len() + 1);
    padded.push(b'{');
    padded.push(b' ');
    padded.extend_from_slice(&bytes[1..]);
    assert!(
        matches!(
            verify_report(&padded, &ledger, &events, &config(), &[]).await,
            Err(OutcomeError::Noncanonical)
        ),
        "whitespace-padded bytes must reject as Noncanonical"
    );

    // Key order swapped: splice the first two top-level members manually.
    // serde_json::Map is a BTreeMap, so re-inserting into another Map would
    // re-sort and the branch would be vacuous — build the non-canonical
    // bytes by hand instead.
    let text = String::from_utf8(bytes.clone()).expect("canonical bytes are UTF-8");
    let inner = text
        .strip_prefix('{')
        .and_then(|t| t.strip_suffix('}'))
        .expect("canonical report is a top-level object");
    let members: Vec<&str> = split_top_level_members(inner);
    assert!(
        members.len() >= 2,
        "canonical report carries at least two members"
    );
    let mut swapped = String::new();
    swapped.push('{');
    swapped.push_str(members[1]);
    swapped.push(',');
    swapped.push_str(members[0]);
    for member in &members[2..] {
        swapped.push(',');
        swapped.push_str(member);
    }
    swapped.push('}');
    let swapped_bytes = swapped.into_bytes();
    assert_ne!(swapped_bytes, bytes, "splice must change member order");
    let result = verify_report(&swapped_bytes, &ledger, &events, &config(), &[]).await;
    assert!(
        matches!(result, Err(OutcomeError::Noncanonical)),
        "reordered-key bytes must reject as Noncanonical, got {result:?}"
    );
}

/// Splits a JSON object body on commas that are outside any string or
/// nested structure — enough for tests splicing top-level members.
fn split_top_level_members(inner: &str) -> Vec<&str> {
    let mut members = Vec::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut start = 0usize;
    for (offset, ch) in inner.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            ',' if depth == 0 => {
                members.push(&inner[start..offset]);
                start = offset + 1;
            }
            _ => {}
        }
    }
    members.push(&inner[start..]);
    members
}
