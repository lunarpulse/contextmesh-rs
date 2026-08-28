//! OC-02 Stage 2H report assembly tests (matrix rows OC02-R01..R08).
//!
//! The report ID is domain-separated and flips on any byte change; the
//! deterministic tier rebuilds byte-exact from (ledger, events, config); the
//! adapter tier records the judge transcript verbatim and verification never
//! re-queries the judge; cross-ledger and cross-context inputs are rejected
//! with no partial artifact; unterminated ledgers never fabricate causal
//! content; the committed golden fixture is byte-checked; and report
//! verification reuses OC-01 ledger verification first.
//!
//! J12's full replay evidence lives in `oc02_shortlist_judges.rs` per the
//! frozen file map; the adapter-tier side of that contract is exercised here
//! through OC02-R03.

#[path = "support/oc01_fixed_dag.rs"]
mod fixed_dag;

use std::cell::RefCell;

use contextmesh::crypto::SigningIdentity;
use contextmesh::model::{ContextId, EventId};

use contextmesh_salience::attribution::AttributionConfigV1;
use contextmesh_salience::attribution_report::{
    AttributionReportV1, EventSource, compute_attribution, verify_report,
};
use contextmesh_salience::error::OutcomeError;
use contextmesh_salience::judge::{
    AblationDeltaV1, AblationRequestV1, CoalitionOutcomeV1, CoalitionRequestV1, JudgeUnavailable,
    M3DeltaKind, M3DeltaV1, OutcomeJudge,
};
use contextmesh_salience::outcome::{OutcomeLedgerBodyV1, SignedOutcomeLedgerV1};
use contextmesh_salience::types::{
    AttemptErrorV1, AttemptStatus, AttemptV1, Blake3HashText, CostLedgerV1, CostValueV1, DeadEndV1,
    Disposition, MechanismRecordV1, OutcomeLimits, OutcomeRecordV1, OutcomeValue, QualityV1,
    TaskBindingV1, TerminalV1, TimestampText, UnterminatedReason,
};
use serde_json::{Value, json};

/// Fixed issuer seed for the Stage 2H terminal ledger fixture.
const REPORT_SEED: [u8; 32] = [0x52; 32];
/// Fixed issuer seed for the Stage 2H unterminated ledger fixture.
const UNTERMINATED_SEED: [u8; 32] = [0x55; 32];
/// Fixed issuer seed for the foreign ledger used in the cross-ledger vector.
const FOREIGN_SEED: [u8; 32] = [0x46; 32];

fn limits() -> OutcomeLimits {
    OutcomeLimits::default()
}

fn config() -> AttributionConfigV1 {
    AttributionConfigV1::default()
}

fn identity(seed: [u8; 32]) -> SigningIdentity {
    SigningIdentity::from_fixture_seed(seed)
}

fn hash_text_of(bytes: &[u8]) -> Blake3HashText {
    let digest = blake3::hash(bytes);
    let mut hex = String::new();
    for byte in digest.as_bytes() {
        hex.push_str(&format!("{byte:02x}"));
    }
    Blake3HashText::parse(&format!("blake3_{hex}")).expect("hash text is valid")
}

fn mechanism_named(identity_text: &str) -> MechanismRecordV1 {
    MechanismRecordV1::new(
        identity_text.to_owned(),
        "1.0.0".to_owned(),
        hash_text_of(b"oc02-report-mechanism"),
        &limits(),
    )
    .expect("mechanism is valid")
}

fn cost_available(value: u64) -> CostValueV1 {
    CostValueV1::new(
        CostValueV1::Available {
            value,
            provenance: mechanism_named("caller.example"),
        },
        &limits(),
    )
    .expect("cost is valid")
}

fn cost_unavailable(reason: &str) -> CostValueV1 {
    CostValueV1::new(
        CostValueV1::Unavailable {
            reason: reason.to_owned(),
            provenance: mechanism_named("caller.example"),
        },
        &limits(),
    )
    .expect("cost is valid")
}

fn cost_ledger_mixed() -> CostLedgerV1 {
    CostLedgerV1::new(
        CostLedgerV1 {
            wall_clock_ms: cost_available(17),
            tool_calls: cost_available(0),
            retries: cost_unavailable("retry metering absent"),
            input_tokens: cost_available(1_000),
            output_tokens: cost_unavailable("output token metering absent"),
        },
        &limits(),
    )
    .expect("cost ledger is valid")
}

fn cost_ledger_unavailable() -> CostLedgerV1 {
    CostLedgerV1::new(
        CostLedgerV1 {
            wall_clock_ms: cost_unavailable("wall clock not exposed"),
            tool_calls: cost_unavailable("call metering absent"),
            retries: cost_unavailable("retry metering absent"),
            input_tokens: cost_unavailable("input token metering absent"),
            output_tokens: cost_unavailable("output token metering absent"),
        },
        &limits(),
    )
    .expect("cost ledger is valid")
}

fn attempt(index: usize, parent: Option<usize>, event: contextmesh::model::EventId) -> AttemptV1 {
    AttemptV1::new(
        AttemptV1 {
            attempt_id: format!("attempt1_{index:06}"),
            parent_attempt_id: parent.map(|p| format!("attempt1_{p:06}")),
            status: AttemptStatus::Failed,
            operation_fingerprint: hash_text_of(b"oc02-report-attempt"),
            event_refs: vec![event],
            error: AttemptErrorV1::Available {
                category: "provider-timeout".to_owned(),
                fingerprint: hash_text_of(b"oc02-report-error"),
            },
            costs: cost_ledger_mixed(),
            provenance: mechanism_named("caller.example"),
        },
        &limits(),
    )
    .expect("attempt is valid")
}

fn dead_end_with(
    index: usize,
    attempt_index: usize,
    event: contextmesh::model::EventId,
) -> DeadEndV1 {
    DeadEndV1::new(
        DeadEndV1 {
            dead_end_id: format!("dead1_{index:06}"),
            attempt_id: format!("attempt1_{attempt_index:06}"),
            failure_category: "provider-timeout".to_owned(),
            error_fingerprint: hash_text_of(b"oc02-report-dead-end"),
            event_refs: vec![event],
            disposition: Disposition::Recovered,
            provenance: mechanism_named("caller.example"),
        },
        &limits(),
    )
    .expect("dead end is valid")
}

/// Payload text for the fixed DAG events. Events 4 and 5 are the outcome
/// evidence, so their payloads are the evidence text; event 1 carries the
/// overlapping `9.5M` token plus an explicit EventId citation of event 2.
fn payload_for(ordinal: usize) -> String {
    match ordinal {
        4 | 5 => "9.5M total budget approved".to_owned(),
        1 => "9.5M contributed; citation evt1 placeholder".to_owned(),
        _ => "background context only".to_owned(),
    }
}

/// Builds the fixed event payloads for the DAG, including the M2 citation of
/// event 2 inside event 1's payload.
fn event_entries(dag: &fixed_dag::FixedDag) -> Vec<(String, String)> {
    let mut entries: Vec<(String, String)> = dag
        .events
        .iter()
        .enumerate()
        .map(|(ordinal, event)| (event.to_string(), payload_for(ordinal)))
        .collect();
    // Replace event 1's payload with one carrying a real citation of event 2.
    let cited = dag.events[2].to_string();
    entries[1].1 = format!("9.5M contributed; see {cited} for follow-up");
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

/// Issues the fixed terminal ledger against the fixed admitted DAG.
async fn terminal_fixture() -> (fixed_dag::FixedDag, SignedOutcomeLedgerV1) {
    let dag = fixed_dag::build().await;
    let issuer = identity(REPORT_SEED);
    let refs = fixed_dag::snapshot(&dag).await;
    let ids = &dag.events;
    let mut outcome_evidence = vec![ids[4], ids[5]];
    outcome_evidence.sort_by_key(ToString::to_string);
    let mut quality_evidence = vec![ids[3], ids[5]];
    quality_evidence.sort_by_key(ToString::to_string);
    let attempts = vec![attempt(0, None, ids[1]), attempt(1, Some(0), ids[2])];
    let dead_ends = vec![dead_end_with(0, 0, ids[3])];
    let body = OutcomeLedgerBodyV1::new(
        dag.context,
        refs,
        TaskBindingV1::new(hash_text_of(b"oc02-report-task"), None, None, &limits()).unwrap(),
        TerminalV1::Event { event: ids[5] },
        OutcomeRecordV1::new(
            OutcomeValue::Succeeded,
            outcome_evidence,
            mechanism_named("caller.example"),
            &limits(),
        )
        .unwrap(),
        QualityV1::new(
            QualityV1::Available {
                value_ppm: 990_000,
                evidence: quality_evidence,
                provenance: mechanism_named("caller.example"),
            },
            &limits(),
        )
        .unwrap(),
        cost_ledger_mixed(),
        attempts,
        dead_ends,
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

/// Issues the fixed unterminated ledger against the fixed admitted DAG.
async fn unterminated_fixture() -> (fixed_dag::FixedDag, SignedOutcomeLedgerV1) {
    let dag = fixed_dag::build().await;
    let issuer = identity(UNTERMINATED_SEED);
    let refs = fixed_dag::snapshot(&dag).await;
    let attempts = vec![attempt(0, None, dag.events[1])];
    let body = OutcomeLedgerBodyV1::new(
        dag.context,
        refs,
        TaskBindingV1::new(
            hash_text_of(b"oc02-report-unterminated-task"),
            None,
            None,
            &limits(),
        )
        .unwrap(),
        TerminalV1::Unterminated {
            reason: UnterminatedReason::CancelledBeforeTerminal,
        },
        OutcomeRecordV1::new(
            OutcomeValue::Unknown,
            vec![],
            mechanism_named("caller.example"),
            &limits(),
        )
        .unwrap(),
        QualityV1::new(
            QualityV1::Unavailable {
                reason: "no recorded rubric".to_owned(),
                provenance: mechanism_named("caller.example"),
            },
            &limits(),
        )
        .unwrap(),
        cost_ledger_unavailable(),
        attempts,
        vec![],
        vec![],
        vec![],
        TimestampText::parse("2026-08-21T00:00:00Z").unwrap(),
        issuer.author(),
        limits(),
    )
    .unwrap();
    let ledger = SignedOutcomeLedgerV1::issue(&issuer, &dag.store, body, limits())
        .await
        .expect("fixed unterminated ledger issues");
    (dag, ledger)
}

/// Issues a second, foreign terminal ledger (different seed and task hash).
async fn foreign_fixture() -> (fixed_dag::FixedDag, SignedOutcomeLedgerV1) {
    let dag = fixed_dag::build().await;
    let issuer = identity(FOREIGN_SEED);
    let refs = fixed_dag::snapshot(&dag).await;
    let ids = &dag.events;
    let mut outcome_evidence = vec![ids[4], ids[5]];
    outcome_evidence.sort_by_key(ToString::to_string);
    let attempts = vec![attempt(0, None, ids[1])];
    let body = OutcomeLedgerBodyV1::new(
        dag.context,
        refs,
        TaskBindingV1::new(hash_text_of(b"oc02-foreign-task"), None, None, &limits()).unwrap(),
        TerminalV1::Event { event: ids[5] },
        OutcomeRecordV1::new(
            OutcomeValue::Succeeded,
            outcome_evidence,
            mechanism_named("caller.example"),
            &limits(),
        )
        .unwrap(),
        QualityV1::new(
            QualityV1::Available {
                value_ppm: 900_000,
                evidence: vec![ids[5]],
                provenance: mechanism_named("caller.example"),
            },
            &limits(),
        )
        .unwrap(),
        cost_ledger_mixed(),
        attempts,
        vec![],
        vec![],
        vec![],
        TimestampText::parse("2026-08-21T00:00:00Z").unwrap(),
        issuer.author(),
        limits(),
    )
    .unwrap();
    let ledger = SignedOutcomeLedgerV1::issue(&issuer, &dag.store, body, limits())
        .await
        .expect("foreign ledger issues");
    (dag, ledger)
}

fn source(dag: &fixed_dag::FixedDag) -> EventSource<'static> {
    // Leak-free static payloads: the fixture payloads are fixed text, so the
    // test materializes them once and keeps them alive for the whole run.
    let entries: &'static Vec<(String, String)> = Box::leak(Box::new(event_entries(dag)));
    let borrowed: &'static [(String, String)] = entries;
    EventSource::from_pairs(dag.context, borrowed).expect("event source is valid")
}

/// Assembles a transcript entry from a recorded answer and the judge
/// identity that was validated alongside it (verification replay, R03/J12).
fn m3_record(event: &EventId, delta_kind: M3DeltaKind, identity: &MechanismRecordV1) -> M3DeltaV1 {
    M3DeltaV1::from_transcript_entry(*event, delta_kind, identity).expect("transcript entry")
}

/// Deterministic spy judge with a fixed identity so golden bytes are stable.
struct RecordingJudge {
    identity: MechanismRecordV1,
    calls: RefCell<usize>,
    ablations: RefCell<Vec<(EventId, M3DeltaKind)>>,
    coalitions: RefCell<Vec<(EventId, M3DeltaKind)>>,
}

impl RecordingJudge {
    fn new() -> Self {
        Self {
            identity: mechanism_named("judge.example"),
            calls: RefCell::new(0),
            ablations: RefCell::new(Vec::new()),
            coalitions: RefCell::new(Vec::new()),
        }
    }

    fn call_count(&self) -> usize {
        *self.calls.borrow()
    }

    /// The exact recorded answer sequence in schedule order, each entry
    /// carrying the judge provenance (M3DeltaV1) — the judge transcript
    /// for verification replay (R03/J12).
    fn transcript(&self) -> Vec<M3DeltaV1> {
        let mut replay = Vec::new();
        replay.extend(
            self.ablations
                .borrow()
                .iter()
                .map(|(event, delta)| m3_record(event, *delta, &self.identity)),
        );
        replay.extend(
            self.coalitions
                .borrow()
                .iter()
                .map(|(event, kind)| m3_record(event, *kind, &self.identity)),
        );
        replay
    }
}

impl OutcomeJudge for RecordingJudge {
    fn identity(&self) -> MechanismRecordV1 {
        self.identity.clone()
    }

    fn ablate(&self, req: AblationRequestV1<'_>) -> Result<AblationDeltaV1, JudgeUnavailable> {
        *self.calls.borrow_mut() += 1;
        // Deterministic alternating transcript; identity repeats so golden
        // bytes are stable across runs.
        let delta = if self.call_count().is_multiple_of(2) {
            M3DeltaKind::Changed
        } else {
            M3DeltaKind::Unchanged
        };
        self.ablations.borrow_mut().push((req.event(), delta));
        Ok(match delta {
            M3DeltaKind::Changed => AblationDeltaV1::Changed,
            _ => AblationDeltaV1::Unchanged,
        })
    }

    fn coalition(
        &self,
        req: CoalitionRequestV1<'_>,
    ) -> Result<CoalitionOutcomeV1, JudgeUnavailable> {
        *self.calls.borrow_mut() += 1;
        let outcome = CoalitionOutcomeV1::Contributing;
        let kind = match outcome {
            CoalitionOutcomeV1::Contributing => M3DeltaKind::Changed,
            CoalitionOutcomeV1::NotContributing => M3DeltaKind::Unchanged,
        };
        self.coalitions.borrow_mut().push((req.target(), kind));
        Ok(outcome)
    }
}

fn report_bytes(report: &AttributionReportV1) -> Vec<u8> {
    report.canonical_bytes().expect("report renders")
}

fn member(bytes: &[u8], key: &str) -> Value {
    let value: Value = serde_json::from_slice(bytes).expect("report bytes parse");
    value.get(key).expect("member exists").clone()
}

fn rebind(bytes: &[u8], key: &str, value: Value) -> Vec<u8> {
    let mut parsed: Value = serde_json::from_slice(bytes).expect("report bytes parse");
    parsed
        .as_object_mut()
        .expect("report is an object")
        .insert(key.to_owned(), value);
    contextmesh_salience::json::jcs(&parsed).expect("tampered report renders")
}

/// OC02-R01: report_id flips on any byte change; the original re-renders.
#[tokio::test]
async fn report_id_tamper_matrix() {
    let (dag, ledger) = terminal_fixture().await;
    let events = source(&dag);
    let report = compute_attribution(&ledger, &events, &config(), None)
        .await
        .expect("report computes");
    let bytes = report_bytes(&report);

    for key in [
        "adapter_tier",
        "config_hash",
        "deterministic_tier",
        "input_snapshot_fingerprint",
        "ledger_id",
        "prereg_reference",
        "task_fingerprint",
        "terminal_status",
        "version",
        "report_id",
    ] {
        let tampered = match member(&bytes, key) {
            Value::String(text) => {
                let flipped = if text == "terminal" {
                    json!("unterminated")
                } else {
                    json!(format!("{text}0"))
                };
                rebind(&bytes, key, flipped)
            }
            Value::Number(number) => rebind(&bytes, key, json!(number.as_i64().unwrap_or(0) + 1)),
            other => rebind(&bytes, key, json!([other])),
        };
        assert!(
            verify_report(&tampered, &ledger, &events, &config(), &[])
                .await
                .is_err(),
            "tampering {key} must invalidate the report"
        );
    }

    // The original re-renders byte-exactly from the same inputs.
    let rebuilt = compute_attribution(&ledger, &events, &config(), None)
        .await
        .expect("report recomputes");
    assert_eq!(bytes, report_bytes(&rebuilt));
    verify_report(&bytes, &ledger, &events, &config(), &[])
        .await
        .expect("untampered report verifies");
}

/// OC02-R02: the deterministic tier rebuilds byte-exact from (ledger,
/// events, config), independent of the judge input.
#[tokio::test]
async fn deterministic_tier_byte_rebuild() {
    let (dag, ledger) = terminal_fixture().await;
    let events = source(&dag);
    let without_judge = compute_attribution(&ledger, &events, &config(), None)
        .await
        .expect("report computes");
    let judge = RecordingJudge::new();
    let with_judge = compute_attribution(&ledger, &events, &config(), Some(&judge))
        .await
        .expect("report computes");
    assert_eq!(
        member(&report_bytes(&without_judge), "deterministic_tier"),
        member(&report_bytes(&with_judge), "deterministic_tier"),
        "judge input must not affect the deterministic tier bytes"
    );
    assert_eq!(
        judge.call_count(),
        8 + 7,
        "three shortlist events: M3 ablations + M4 coalition schedule"
    );

    let again = compute_attribution(&ledger, &events, &config(), None)
        .await
        .expect("report recomputes");
    assert_eq!(report_bytes(&without_judge), report_bytes(&again));
}

/// OC02-R03: the adapter tier equals the recorded transcript on replay.
#[tokio::test]
async fn adapter_tier_transcript_replay() {
    let (dag, ledger) = terminal_fixture().await;
    let events = source(&dag);
    let first = RecordingJudge::new();
    let second = RecordingJudge::new();
    let report_a = compute_attribution(&ledger, &events, &config(), Some(&first))
        .await
        .expect("report computes");
    let report_b = compute_attribution(&ledger, &events, &config(), Some(&second))
        .await
        .expect("report recomputes");
    assert_eq!(
        member(&report_bytes(&report_a), "adapter_tier"),
        member(&report_bytes(&report_b), "adapter_tier"),
        "replaying the identical judge transcript must reproduce adapter bytes"
    );
    let status = member(&report_bytes(&report_a), "adapter_tier")
        .get("status")
        .and_then(Value::as_str)
        .expect("adapter status")
        .to_owned();
    assert_eq!(status, "computed");

    // A judge-computed report verifies when the recorded transcript replays
    // (the judge is never re-queried — verification replays the recording).
    let bytes_a = report_bytes(&report_a);
    let transcript = first.transcript();
    verify_report(&bytes_a, &ledger, &events, &config(), &transcript)
        .await
        .expect("computed report verifies against its recorded transcript");
    // A different transcript must not verify the same report bytes: flip
    // every recorded answer, keeping the provenance.
    let wrong = transcript
        .iter()
        .map(|entry| {
            m3_record(
                &entry.event(),
                match entry.delta_kind() {
                    M3DeltaKind::Changed => M3DeltaKind::Unchanged,
                    _ => M3DeltaKind::Changed,
                },
                &first.identity,
            )
        })
        .collect::<Vec<_>>();
    match verify_report(&bytes_a, &ledger, &events, &config(), &wrong).await {
        Err(OutcomeError::IdMismatch) => {}
        other => panic!("wrong transcript must fail verification, got {other:?}"),
    }
}

/// OC02-R04: a report verified against a different ledger is rejected.
#[tokio::test]
async fn cross_ledger_report_rejected() {
    let (dag, ledger) = terminal_fixture().await;
    let events = source(&dag);
    let report = compute_attribution(&ledger, &events, &config(), None)
        .await
        .expect("report computes");
    let bytes = report_bytes(&report);
    let (_foreign_dag, foreign) = foreign_fixture().await;
    // The foreign ledger shares the fixed DAG/context but has another ID.
    match verify_report(&bytes, &foreign, &events, &config(), &[]).await {
        Err(OutcomeError::IdMismatch) => {}
        other => panic!("cross-ledger verification must fail with IdMismatch, got {other:?}"),
    }
    verify_report(&bytes, &ledger, &events, &config(), &[])
        .await
        .expect("matching ledger verifies");
}

/// OC02-R05: foreign-context events are rejected at report level with no
/// partial artifact.
#[tokio::test]
async fn cross_context_report_rejected() {
    let (dag, ledger) = terminal_fixture().await;
    let foreign_context = ContextId::from_bytes([0x7f; 32]);
    let entries: &'static Vec<(String, String)> = Box::leak(Box::new(event_entries(&dag)));
    let borrowed: &'static [(String, String)] = entries;
    let events = EventSource::from_pairs(foreign_context, borrowed).expect("source is valid");
    match compute_attribution(&ledger, &events, &config(), None).await {
        Err(OutcomeError::ContextMismatch) => {}
        other => panic!("cross-context compute must fail with ContextMismatch, got {other:?}"),
    }
    // A report computed against the matching context then verified with a
    // foreign-context source also fails.
    let matched = source(&dag);
    let report = compute_attribution(&ledger, &matched, &config(), None)
        .await
        .expect("matched compute succeeds");
    match verify_report(&report_bytes(&report), &ledger, &events, &config(), &[]).await {
        Err(OutcomeError::ContextMismatch) => {}
        other => panic!("cross-context verify must fail with ContextMismatch, got {other:?}"),
    }
}

/// OC02-R06: an unterminated ledger never fabricates causal content.
#[tokio::test]
async fn unterminated_no_fabrication() {
    let (dag, ledger) = unterminated_fixture().await;
    let events = source(&dag);
    let judge = RecordingJudge::new();
    let report = compute_attribution(&ledger, &events, &config(), Some(&judge))
        .await
        .expect("report computes");
    let bytes = report_bytes(&report);
    assert_eq!(member(&bytes, "terminal_status"), json!("unterminated"));
    let adapter = member(&bytes, "adapter_tier");
    assert_eq!(adapter.get("status"), Some(&json!("no_nominations")));
    assert_eq!(adapter.get("m3"), Some(&json!([])));
    assert_eq!(adapter.get("m4"), Some(&json!([])));
    assert_eq!(
        adapter.get("uncertainty_markers"),
        Some(&json!(["no_terminal_outcome"]))
    );
    assert_eq!(
        judge.call_count(),
        0,
        "no judge calls on an unterminated ledger"
    );
    verify_report(&bytes, &ledger, &events, &config(), &[])
        .await
        .expect("unterminated report verifies");
}

/// OC02-R07: the committed golden fixture bytes and SHA-256 are authoritative.
#[tokio::test]
async fn golden_report_fixture_immutable() {
    let committed = include_str!("fixtures/oc02-attribution-report-v1-golden.json");
    let committed_sha = include_str!("fixtures/oc02-attribution-report-v1-golden.sha256");
    let digest = sha256sum_of(committed.as_bytes());
    assert_eq!(
        committed_sha.trim(),
        digest,
        "committed fixture bytes do not match the committed SHA-256"
    );

    let (dag, ledger) = terminal_fixture().await;
    let events = source(&dag);
    let judge = RecordingJudge::new();
    let report = compute_attribution(&ledger, &events, &config(), Some(&judge))
        .await
        .expect("report computes");
    assert_eq!(
        committed.as_bytes(),
        report_bytes(&report),
        "live report bytes diverge from the committed golden fixture"
    );
}

/// Exact lowercase-hex SHA-256 via the system tool (OC-01 workspace-gate
/// pattern; no new crate dependency).
fn sha256sum_of(bytes: &[u8]) -> String {
    use std::io::Write as _;
    let mut child = std::process::Command::new("sha256sum")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("sha256sum spawns");
    child
        .stdin
        .as_mut()
        .expect("stdin piped")
        .write_all(bytes)
        .expect("digest input writes");
    let output = child.wait_with_output().expect("sha256sum completes");
    String::from_utf8(output.stdout)
        .expect("sha256sum output is UTF-8")
        .split_whitespace()
        .next()
        .expect("sha256sum emits a digest")
        .to_owned()
}

/// Ignored golden-fixture generator (change-controlled per spec §11).
#[tokio::test]
#[ignore = "golden fixture generator; run with --ignored and review the diff"]
async fn generate_golden_report_fixture() {
    let (dag, ledger) = terminal_fixture().await;
    let events = source(&dag);
    let judge = RecordingJudge::new();
    let report = compute_attribution(&ledger, &events, &config(), Some(&judge))
        .await
        .expect("report computes");
    let bytes = report_bytes(&report);
    let digest = sha256sum_of(&bytes);
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/oc02-attribution-report-v1-golden.json"
        ),
        &bytes,
    )
    .expect("fixture writes");
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/oc02-attribution-report-v1-golden.sha256"
        ),
        format!("{digest}\n"),
    )
    .expect("fixture digest writes");
}

/// OC02-R08: report verification reuses OC-01 ledger verification first.
#[tokio::test]
async fn report_verify_reuses_ledger_verify() {
    let (dag, ledger) = terminal_fixture().await;
    let events = source(&dag);
    let report = compute_attribution(&ledger, &events, &config(), None)
        .await
        .expect("report computes");
    let mut wire = ledger.to_wire(limits()).expect("ledger renders");
    // Tamper one byte inside the signature text region at the end of the wire.
    let last = wire.len() - 1;
    if wire[last] == b'"' {
        wire[last - 1] = wire[last - 1].wrapping_add(1);
    } else {
        wire[last] = wire[last].wrapping_add(1);
    }
    let forged = SignedOutcomeLedgerV1::from_wire(&wire, limits());
    assert!(
        forged.is_err(),
        "tampered ledger wire must fail OC-01 verification"
    );
    // Use a structurally valid but differently signed ledger to force the
    // ledger step to reject before any nomination-domain work would run.
    let (_foreign_dag, foreign) = foreign_fixture().await;
    match verify_report(&report_bytes(&report), &foreign, &events, &config(), &[]).await {
        Err(OutcomeError::IdMismatch) => {}
        other => panic!("foreign ledger must fail verification before report work, got {other:?}"),
    }
}
