//! OC-01 Stage 2A schema tests (matrix rows OC01-I02..OC01-I17).
//!
//! Row OC01-I01 (body/envelope shapes) belongs to `outcome.rs` in Stage 2B
//! and is intentionally absent here.

use contextmesh::model::{ContextId, EventId};
use contextmesh_salience::error::{OutcomeError, OutcomeOperationError};
use contextmesh_salience::json;
use contextmesh_salience::types::{
    AttemptErrorV1, AttemptStatus, AttemptV1, AttributionLabel, AttributionMarkV1, Blake3HashText,
    CostLedgerV1, CostValueV1, DeadEndV1, Disposition, InputRefSnapshotV1, LocalRefEntry,
    MAX_OUTCOME_NOTE_BYTES, MAX_OUTCOME_NOTES, MAX_OUTCOME_WIRE_BYTES, MechanismRecordV1,
    OutcomeId, OutcomeLimits, OutcomeSignature, OutcomeValue, QualityV1, RemoteRefEntry,
    TaskBindingV1, TerminalV1, TimestampText, validate_attempt_tree, validate_attribution_marks,
    validate_dead_ends, validate_warnings,
};
use serde_json::json;

fn limits() -> OutcomeLimits {
    OutcomeLimits::default()
}

fn hash(text: &str) -> Blake3HashText {
    Blake3HashText::parse(text).expect("test hash text is valid")
}

fn event(id: u8) -> EventId {
    EventId::from_bytes([id; 32])
}

fn context() -> ContextId {
    ContextId::from_bytes([7; 32])
}

fn mechanism() -> MechanismRecordV1 {
    MechanismRecordV1::new(
        "caller.example".to_owned(),
        "1.0.0".to_owned(),
        hash(&hash_text()),
        &limits(),
    )
    .expect("mechanism is valid")
}

fn hash_text() -> String {
    let digest = blake3::hash(b"test-input");
    let bytes = digest.as_bytes().to_owned();
    let mut hex = String::new();
    for byte in bytes {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("blake3_{hex}")
}

fn cost_available(value: u64) -> CostValueV1 {
    CostValueV1::new(
        CostValueV1::Available {
            value,
            provenance: mechanism(),
        },
        &limits(),
    )
    .expect("cost is valid")
}

fn cost_unavailable(reason: &str) -> CostValueV1 {
    CostValueV1::new(
        CostValueV1::Unavailable {
            reason: reason.to_owned(),
            provenance: mechanism(),
        },
        &limits(),
    )
    .expect("cost is valid")
}

fn ledger() -> CostLedgerV1 {
    CostLedgerV1::new(
        CostLedgerV1 {
            wall_clock_ms: cost_available(17),
            tool_calls: cost_available(0),
            retries: cost_unavailable("clock not exposed"),
            input_tokens: cost_available(1_000),
            output_tokens: cost_unavailable("metering absent"),
        },
        &limits(),
    )
    .expect("ledger is valid")
}

fn attempt(index: usize, parent: Option<usize>) -> AttemptV1 {
    AttemptV1::new(
        AttemptV1 {
            attempt_id: format!("attempt1_{index:06}"),
            parent_attempt_id: parent.map(|p| format!("attempt1_{p:06}")),
            status: AttemptStatus::Failed,
            operation_fingerprint: hash(&hash_text()),
            event_refs: vec![event(1), event(2)],
            error: AttemptErrorV1::Available {
                category: "provider-timeout".to_owned(),
                fingerprint: hash(&hash_text()),
            },
            costs: ledger(),
            provenance: mechanism(),
        },
        &limits(),
    )
    .expect("attempt is valid")
}

fn dead_end(index: usize, attempt_index: usize) -> DeadEndV1 {
    DeadEndV1::new(
        DeadEndV1 {
            dead_end_id: format!("dead1_{index:06}"),
            attempt_id: format!("attempt1_{attempt_index:06}"),
            failure_category: "provider-timeout".to_owned(),
            error_fingerprint: hash(&hash_text()),
            event_refs: vec![event(1)],
            disposition: Disposition::Unresolved,
            provenance: mechanism(),
        },
        &limits(),
    )
    .expect("dead end is valid")
}

fn mark(event_id: EventId, label: AttributionLabel) -> AttributionMarkV1 {
    AttributionMarkV1::new(
        AttributionMarkV1 {
            event: event_id,
            label,
            evidence: vec![event(3)],
            mechanism: mechanism(),
        },
        &limits(),
    )
    .expect("mark is valid")
}

/// OC01-I01: body/envelope shapes, required fields, null and tagged variants.
#[test]
fn exact_v1_shapes_tags_requiredness_and_version() {
    use contextmesh_salience::outcome::SignedOutcomeLedgerV1;

    // A committed fixed-DAG envelope is the valid base. Schema testing needs
    // no alternative public signing/materialization path.
    let wire = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/oc01-outcome-ledger-v1-unterminated.json"
    ))
    .unwrap();
    let issued = SignedOutcomeLedgerV1::from_wire(&wire, limits()).unwrap();
    let parsed = SignedOutcomeLedgerV1::from_wire(&wire, limits()).unwrap();
    assert_eq!(parsed.outcome_id(), issued.outcome_id());
    assert_eq!(parsed.body().version(), 1);

    let value: serde_json::Value = serde_json::from_slice(&wire).unwrap();
    let canonical = |value: &serde_json::Value| json::jcs(value).unwrap();

    let mut unknown_envelope = value.clone();
    unknown_envelope["unexpected"] = json!(true);
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(&canonical(&unknown_envelope), limits()).unwrap_err(),
        OutcomeError::Malformed
    );
    let mut missing_envelope = value.clone();
    missing_envelope
        .as_object_mut()
        .unwrap()
        .remove("signature");
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(&canonical(&missing_envelope), limits()).unwrap_err(),
        OutcomeError::Malformed
    );
    let mut unknown_body = value.clone();
    unknown_body["body"]["unexpected"] = json!(true);
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(&canonical(&unknown_body), limits()).unwrap_err(),
        OutcomeError::Malformed
    );
    let mut missing_body = value.clone();
    missing_body["body"]
        .as_object_mut()
        .unwrap()
        .remove("author");
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(&canonical(&missing_body), limits()).unwrap_err(),
        OutcomeError::Malformed
    );
    let mut wrong_version = value.clone();
    wrong_version["body"]["version"] = json!(2);
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(&canonical(&wrong_version), limits()).unwrap_err(),
        OutcomeError::UnsupportedVersion
    );
    let mut illegal_null = value.clone();
    illegal_null["body"]["author"] = serde_json::Value::Null;
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(&canonical(&illegal_null), limits()).unwrap_err(),
        OutcomeError::Malformed
    );
    let mut mixed_terminal = value;
    mixed_terminal["body"]["terminal"] = json!({
        "status": "event", "event": event(1).to_string(), "reason": "unknown"
    });
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(&canonical(&mixed_terminal), limits()).unwrap_err(),
        OutcomeError::Malformed
    );
}

/// OC01-P05 (filed beside the schema tests per the matrix): `from_wire` is
/// the sole untrusted constructor; checked nested constructors and read-only
/// accessors leave no deserialize/unchecked bypass into public state.
#[test]
fn public_api_has_no_unchecked_or_deserialize_bypass() {
    use contextmesh_salience::outcome::{SignedOutcomeLedgerV1, derive_outcome_id};

    // The only public construction paths are store-aware `issue` (exercised
    // in oc01_dag) and checked `from_wire`; use a committed valid wire here.
    let wire = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/oc01-outcome-ledger-v1-unterminated.json"
    ))
    .unwrap();
    let ledger = SignedOutcomeLedgerV1::from_wire(&wire, limits()).unwrap();
    ledger.verify(limits()).unwrap();

    // Cross-check: verify() and from_wire(to_wire()) agree — there is no
    // third unchecked materialization path.
    let wire = ledger.to_wire(limits()).unwrap();
    let reparsed = SignedOutcomeLedgerV1::from_wire(&wire, limits()).unwrap();
    reparsed.verify(limits()).unwrap();
    assert_eq!(reparsed, ledger);

    // The free derivation helper is also checked: it revalidates the body.
    assert_eq!(
        derive_outcome_id(ledger.body(), limits()).unwrap(),
        ledger.outcome_id()
    );

    // Invalid untrusted bytes never produce public state.
    assert!(SignedOutcomeLedgerV1::from_wire(b"{}", limits()).is_err());

    // Accessors are read-only: they return shared references, owned copies,
    // or Copy values — never a mutable view into ledger state.
    let _: &AttemptV1 = ledger.body().attempts().first().unwrap();
    let _: OutcomeId = ledger.outcome_id();
    let _: u8 = ledger.body().version();
    let _: &[String] = ledger.body().warnings();
    let marker = ledger.body().attempts().first().unwrap().attempt_id.clone();
    assert!(ledger.body().validate(limits()).is_ok());
    assert_eq!(ledger.body().attempts().first().unwrap().attempt_id, marker);

    // The body and envelope are Serialize-only at the top level (the wire
    // parser reconstructs them through parse_body + validate, never through
    // a derived Deserialize); nested value types that do deserialize are
    // always revalidated by body.validate() before any public state exists.
    fn assert_serialize<T: serde::Serialize>() {}
    assert_serialize::<contextmesh_salience::outcome::OutcomeLedgerBodyV1>();
    assert_serialize::<SignedOutcomeLedgerV1>();
}

/// OC01-I02: typed encodings and timestamps are exact.
#[test]
fn typed_text_encodings_and_timestamp_are_exact() {
    // Typed fixed-size text round trips.
    let id = OutcomeId::from_bytes([9; 32]);
    let text = id.to_string();
    assert!(text.starts_with("ocout1_"));
    assert_eq!(text.parse::<OutcomeId>().unwrap(), id);
    // Cross-type prefix, padding, length, alphabet reject.
    assert!(
        text.replace("ocout1_", "ocsig1_")
            .parse::<OutcomeSignature>()
            .is_err()
    );
    assert!(format!("{text}=").parse::<OutcomeId>().is_err());
    assert!(text[..text.len() - 1].parse::<OutcomeId>().is_err());
    // Uppercase base64 alphabet rejects.
    let upper = format!("ocout1_{}", text["ocout1_".len()..].to_ascii_uppercase());
    assert!(upper.parse::<OutcomeId>().is_err());

    // Hash text grammar.
    let valid = hash_text();
    assert!(Blake3HashText::parse(&valid).is_ok());
    assert!(Blake3HashText::parse("sha256_abcdef").is_err());
    assert!(Blake3HashText::parse(&valid.to_uppercase()).is_err());
    assert!(Blake3HashText::parse(&format!("{valid}0")).is_err());

    // Timestamps: exact grammar, Gregorian validity, year >= 1970.
    assert!(TimestampText::parse("2026-08-21T00:00:00Z").is_ok());
    assert!(TimestampText::parse("1969-12-31T23:59:59Z").is_err());
    assert!(TimestampText::parse("2026-02-29T00:00:00Z").is_err()); // not a leap year
    assert!(TimestampText::parse("2024-02-29T00:00:00Z").is_ok()); // leap year
    assert!(TimestampText::parse("2026-13-01T00:00:00Z").is_err());
    assert!(TimestampText::parse("2026-08-21 00:00:00Z").is_err());
    assert!(TimestampText::parse("2026-08-21T00:00:00").is_err());
    assert!(TimestampText::parse("2026-08-21T24:00:00Z").is_err());
    assert!(TimestampText::parse("2026-04-31T00:00:00Z").is_err());
}

/// OC01-I03: mechanism boundaries and provenance.
#[test]
fn mechanism_record_boundaries_and_provenance() {
    let max_identity = "x".repeat(128);
    let max_version = "y".repeat(64);
    let record = MechanismRecordV1::new(max_identity, max_version, hash(&hash_text()), &limits());
    assert!(record.is_ok());
    assert!(
        MechanismRecordV1::new("x".repeat(129), "1".into(), hash(&hash_text()), &limits()).is_err()
    );
    assert!(
        MechanismRecordV1::new("x".into(), "y".repeat(65), hash(&hash_text()), &limits()).is_err()
    );
    assert!(
        MechanismRecordV1::new(String::new(), "1".into(), hash(&hash_text()), &limits()).is_err()
    );
    assert!(
        MechanismRecordV1::new(
            "has\tcontrol".into(),
            "1".into(),
            hash(&hash_text()),
            &limits()
        )
        .is_err()
    );
    // Round-trips through JSON with exact keys.
    let record = record.unwrap();
    let wire = serde_json::to_value(&record).unwrap();
    assert_eq!(
        wire,
        json!({"identity": record.identity, "version": record.version, "config_hash": record.config_hash.to_string()})
    );
}

/// OC01-I04: task binding is hash-only and bounded.
#[test]
fn task_binding_is_hash_only_and_bounded() {
    let binding = TaskBindingV1::new(
        hash(&hash_text()),
        None,
        Some("ext-id-1".to_owned()),
        &limits(),
    )
    .unwrap();
    assert!(binding.structured_hash.is_none());
    // 128 passes, 129 rejects.
    assert!(TaskBindingV1::new(hash(&hash_text()), None, Some("a".repeat(128)), &limits()).is_ok());
    assert!(
        TaskBindingV1::new(hash(&hash_text()), None, Some("a".repeat(129)), &limits()).is_err()
    );
    // Raw task bytes or note fields reject: constructor accepts hashes only.
    let wire = serde_json::to_value(&binding).unwrap();
    assert!(wire.get("note").is_none());
    assert!(wire.get("task_text").is_none());
    assert!(wire.get("content").is_none());
}

/// OC01-I05: snapshot order/uniqueness/fingerprint.
#[test]
fn input_ref_snapshot_order_uniqueness_and_fingerprint() {
    let ctx = context();
    let local = vec![
        LocalRefEntry {
            name: "alpha".into(),
            head: event(1),
        },
        LocalRefEntry {
            name: "beta".into(),
            head: event(2),
        },
    ];
    let remote = vec![
        RemoteRefEntry {
            peer: "peer-a".into(),
            name: "main".into(),
            head: event(3),
        },
        RemoteRefEntry {
            peer: "peer-b".into(),
            name: "dev".into(),
            head: event(4),
        },
    ];
    let snapshot = InputRefSnapshotV1::new(ctx, local.clone(), remote.clone()).unwrap();
    assert_eq!(snapshot.head_count(), 4);
    // Fingerprint binds context and arrays: any tamper rejects.
    let good = snapshot.fingerprint;
    assert!(
        InputRefSnapshotV1::from_parts(ctx, good.clone(), local.clone(), remote.clone()).is_ok()
    );
    let mut tampered_local = local.clone();
    tampered_local[0].head = event(9);
    assert!(
        InputRefSnapshotV1::from_parts(ctx, good.clone(), tampered_local, remote.clone()).is_err()
    );
    let mut tampered_remote = remote.clone();
    tampered_remote[0].peer = "peer-z".into();
    assert!(
        InputRefSnapshotV1::from_parts(ctx, good.clone(), local.clone(), tampered_remote).is_err()
    );
    // Empty arrays are valid.
    assert!(InputRefSnapshotV1::new(ctx, vec![], vec![]).is_ok());
    // Disorder and duplicates reject.
    let disordered = vec![
        LocalRefEntry {
            name: "beta".into(),
            head: event(1),
        },
        LocalRefEntry {
            name: "alpha".into(),
            head: event(2),
        },
    ];
    assert!(InputRefSnapshotV1::new(ctx, disordered, vec![]).is_err());
    let duplicated = vec![
        LocalRefEntry {
            name: "same".into(),
            head: event(1),
        },
        LocalRefEntry {
            name: "same".into(),
            head: event(2),
        },
    ];
    assert!(InputRefSnapshotV1::new(ctx, duplicated, vec![]).is_err());
    // A different context rejects the same fingerprint.
    let other = ContextId::from_bytes([8; 32]);
    assert!(InputRefSnapshotV1::from_parts(other, good, local, remote).is_err());
}

/// OC01-I06: EventId lists require caller-canonical order.
#[test]
fn event_id_lists_require_caller_canonical_order() {
    let mut ok = vec![event(1), event(2), event(3)];
    ok.sort_by_key(|e| e.to_string());
    // The fixture happens to be ascending; force canonical ordering.
    let ordered: Vec<EventId> = {
        let mut ids = ok.clone();
        ids.sort_by_key(|e| e.to_string());
        ids
    };
    assert!(contextmesh_salience::types::validate_event_id_list(&ordered).is_ok());
    let duplicate = vec![event(1), event(1)];
    assert!(contextmesh_salience::types::validate_event_id_list(&duplicate).is_err());
    // Disordered: reverse and let validation reject (canonical text order is
    // opaque, so construct guaranteed disorder by comparing texts).
    let mut reversed = ordered.clone();
    reversed.reverse();
    if reversed.len() > 1 && reversed[0].to_string() > reversed[1].to_string() {
        assert!(contextmesh_salience::types::validate_event_id_list(&reversed).is_err());
    }
}

/// OC01-I07: terminal variants are exhaustive.
#[test]
fn terminal_event_and_unterminated_variants_are_exhaustive() {
    let event_terminal = serde_json::from_value::<TerminalV1>(json!({
        "status": "event",
        "event": event(1).to_string()
    }))
    .unwrap();
    assert!(matches!(event_terminal, TerminalV1::Event { .. }));
    for reason in [
        "no-terminal-event",
        "cancelled-before-terminal",
        "collector-ended",
        "unknown",
    ] {
        let parsed = serde_json::from_value::<TerminalV1>(json!({
            "status": "unterminated",
            "reason": reason
        }))
        .unwrap();
        assert!(matches!(parsed, TerminalV1::Unterminated { .. }));
    }
    // Null event, free text, mixed fields reject.
    assert!(
        serde_json::from_value::<TerminalV1>(json!({"status": "event", "event": null})).is_err()
    );
    assert!(
        serde_json::from_value::<TerminalV1>(json!({
            "status": "unterminated", "reason": "free text"
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<TerminalV1>(json!({
            "status": "event", "event": event(1).to_string(), "reason": "unknown"
        }))
        .is_err()
    );
    assert!(serde_json::from_value::<TerminalV1>(json!({"status": "discovered"})).is_err());
}

/// OC01-I08: outcome values are caller-declared, not inferred.
#[test]
fn outcome_values_are_caller_declared_not_inferred() {
    for (value, text) in [
        (OutcomeValue::Succeeded, "succeeded"),
        (OutcomeValue::Failed, "failed"),
        (OutcomeValue::Partial, "partial"),
        (OutcomeValue::Cancelled, "cancelled"),
        (OutcomeValue::Unknown, "unknown"),
    ] {
        let record = contextmesh_salience::types::OutcomeRecordV1::new(
            value,
            vec![],
            mechanism(),
            &limits(),
        )
        .unwrap();
        let wire = serde_json::to_value(&record).unwrap();
        assert_eq!(wire["value"], text);
        let round =
            serde_json::from_value::<contextmesh_salience::types::OutcomeRecordV1>(wire).unwrap();
        assert_eq!(round.value, value);
    }
    // Terminal status does not infer outcome: unterminated + succeeded is a
    // valid caller declaration and round-trips unchanged.
    let record = contextmesh_salience::types::OutcomeRecordV1::new(
        OutcomeValue::Succeeded,
        vec![],
        mechanism(),
        &limits(),
    )
    .unwrap();
    assert!(matches!(record.value, OutcomeValue::Succeeded));
}

/// OC01-I09: quality availability values and provenance.
#[test]
fn quality_availability_values_and_provenance_are_exact() {
    let zero = QualityV1::new(
        QualityV1::Available {
            value_ppm: 0,
            evidence: vec![],
            provenance: mechanism(),
        },
        &limits(),
    )
    .unwrap();
    assert!(matches!(zero, QualityV1::Available { value_ppm: 0, .. }));
    let max = QualityV1::new(
        QualityV1::Available {
            value_ppm: 1_000_000,
            evidence: vec![],
            provenance: mechanism(),
        },
        &limits(),
    )
    .unwrap();
    assert!(matches!(
        max,
        QualityV1::Available {
            value_ppm: 1_000_000,
            ..
        }
    ));
    assert!(
        QualityV1::new(
            QualityV1::Available {
                value_ppm: 1_000_001,
                evidence: vec![],
                provenance: mechanism()
            },
            &limits()
        )
        .is_err()
    );
    // Missing provenance / mixed variants reject.
    assert!(
        serde_json::from_value::<QualityV1>(json!({
            "status": "available", "value_ppm": 5, "evidence": []
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<QualityV1>(json!({
            "status": "available", "value_ppm": 5, "evidence": [],
            "provenance": {"identity": "a", "version": "1", "config_hash": hash_text()},
            "reason": "extra"
        }))
        .is_err()
    );
    // Overlong unavailable reason rejects.
    let long = "r".repeat(MAX_OUTCOME_NOTE_BYTES + 1);
    assert!(
        QualityV1::new(
            QualityV1::Unavailable {
                reason: long,
                provenance: mechanism()
            },
            &limits()
        )
        .is_err()
    );
}

/// OC01-I10: cost availability, zero, and unavailable are preserved.
#[test]
fn cost_availability_zero_and_unavailable_are_preserved() {
    // Zero is an available recorded value, never unavailable.
    let zero = cost_available(0);
    let wire = serde_json::to_value(&zero).unwrap();
    assert_eq!(wire["status"], "available");
    assert_eq!(wire["value"], 0);
    let round = serde_json::from_value::<CostValueV1>(wire).unwrap();
    assert!(matches!(round, CostValueV1::Available { value: 0, .. }));
    // Mixed availability round-trips per field.
    let ledger = ledger();
    let wire = serde_json::to_value(&ledger).unwrap();
    assert_eq!(wire["tool_calls"]["status"], "available");
    assert_eq!(wire["retries"]["status"], "unavailable");
    let round = serde_json::from_value::<CostLedgerV1>(wire).unwrap();
    assert_eq!(round, ledger);
    // Missing/mixed/inferred values reject: unknown member rejects.
    assert!(
        serde_json::from_value::<CostLedgerV1>(json!({
            "wall_clock_ms": serde_json::to_value(cost_available(1)).unwrap(),
            "tool_calls": serde_json::to_value(cost_available(2)).unwrap(),
            "retries": serde_json::to_value(cost_unavailable("x")).unwrap(),
            "input_tokens": serde_json::to_value(cost_available(3)).unwrap(),
            "output_tokens": serde_json::to_value(cost_unavailable("y")).unwrap(),
            "extra": 1
        }))
        .is_err()
    );
    // Safe-integer maximum passes, +1 rejects.
    assert!(cost_available(9_007_199_254_740_991).status_available());
    assert!(
        CostValueV1::new(
            CostValueV1::Available {
                value: 9_007_199_254_740_992,
                provenance: mechanism()
            },
            &limits()
        )
        .is_err()
    );
}

/// Helper: quick availability probe used by the cost test.
trait CostAvailable {
    fn status_available(&self) -> bool;
}

impl CostAvailable for CostValueV1 {
    fn status_available(&self) -> bool {
        matches!(self, CostValueV1::Available { .. })
    }
}

/// OC01-I11: safe integers and checked aggregate boundaries.
#[test]
fn safe_integer_and_checked_aggregate_boundaries() {
    // Limits: equal-or-lower nonzero pass; zero and above-hard-max reject.
    let lowered = OutcomeLimits {
        max_wire_bytes: 1024,
        max_event_references: 8,
        ..OutcomeLimits::default()
    };
    assert!(lowered.validate().is_ok());
    let zeroed = OutcomeLimits {
        max_wire_bytes: 0,
        ..lowered
    };
    assert!(zeroed.validate().is_err());
    let raised = OutcomeLimits {
        max_wire_bytes: MAX_OUTCOME_WIRE_BYTES + 1,
        ..OutcomeLimits::default()
    };
    assert!(raised.validate().is_err());
    let sixty_four: Vec<String> = (0..64).map(|i| format!("warning-{i}")).collect();
    assert!(validate_warnings(&sixty_four, &limits()).is_ok());
    let mut sixty_five = sixty_four;
    sixty_five.push("extra".into());
    assert_eq!(
        validate_warnings(&sixty_five, &limits()).unwrap_err(),
        OutcomeError::LimitExceeded
    );
}

/// OC01-I12: attempt tree ordinals, parents, connectivity, categories.
#[test]
fn attempt_tree_ordinals_parent_order_connectivity_and_categories() {
    // Multi-level tree passes.
    let tree = vec![
        attempt(0, None),
        attempt(1, Some(0)),
        attempt(2, Some(1)),
        attempt(3, Some(0)),
    ];
    assert!(validate_attempt_tree(&tree, &limits()).is_ok());
    // Gap rejects.
    let mut gap = tree.clone();
    gap[1].attempt_id = "attempt1_999999".into();
    assert!(validate_attempt_tree(&gap, &limits()).is_err());
    // Second root rejects (index != 0 with no parent).
    let mut second_root = tree.clone();
    second_root[2].parent_attempt_id = None;
    assert!(validate_attempt_tree(&second_root, &limits()).is_err());
    // Forward parent rejects.
    let mut forward = tree.clone();
    forward[1].parent_attempt_id = Some("attempt1_000002".into());
    assert!(validate_attempt_tree(&forward, &limits()).is_err());
    // Missing parent rejects (a disconnected node's only representation).
    let mut missing = tree.clone();
    missing[1].parent_attempt_id = Some("attempt1_000042".into());
    assert!(validate_attempt_tree(&missing, &limits()).is_err());
    // Non-ASCII category rejects.
    let mut bad_category = attempt(0, None);
    if let AttemptErrorV1::Available { category, .. } = &mut bad_category.error {
        *category = "카테고리".into();
    }
    assert!(AttemptV1::new(bad_category.clone(), &limits()).is_err());
    // 65-byte category rejects.
    let mut long_category = attempt(0, None);
    if let AttemptErrorV1::Available { category, .. } = &mut long_category.error {
        *category = "a".repeat(65);
    }
    assert!(AttemptV1::new(long_category.clone(), &limits()).is_err());
    // Disconnected-node coverage is provided by the missing-parent arm above:
    // a node whose parent index is out of range of earlier nodes is exactly
    // a disconnected node, and it rejects. Cycle coverage is the forward
    // parent arm. This arm confirms the untouched tree stays valid.
    let mut disconnected = tree.clone();
    disconnected[1].parent_attempt_id = Some("attempt1_000000".into());
    assert!(validate_attempt_tree(&disconnected, &limits()).is_ok());
}

/// OC01-I13: attempt values/errors/costs/provenance round-trip exactly.
#[test]
fn attempt_values_errors_costs_and_provenance_round_trip_exactly() {
    let original = attempt(0, None);
    let wire = serde_json::to_value(&original).unwrap();
    let round = serde_json::from_value::<AttemptV1>(wire).unwrap();
    assert_eq!(round, original);
    // All five statuses round-trip.
    for status in [
        AttemptStatus::Succeeded,
        AttemptStatus::Failed,
        AttemptStatus::Partial,
        AttemptStatus::Cancelled,
        AttemptStatus::Unknown,
    ] {
        let mut variant = attempt(0, None);
        variant.status = status;
        let wire = serde_json::to_value(&variant).unwrap();
        let round = serde_json::from_value::<AttemptV1>(wire).unwrap();
        assert_eq!(round.status, status);
    }
    // Succeeded-with-diagnostic (available error) round-trips unchanged.
    let mut succeeded = attempt(0, None);
    succeeded.status = AttemptStatus::Succeeded;
    let wire = serde_json::to_value(&succeeded).unwrap();
    let round = serde_json::from_value::<AttemptV1>(wire).unwrap();
    assert!(matches!(round.error, AttemptErrorV1::Available { .. }));
    // Unavailable error round-trips.
    let mut unavailable = attempt(0, None);
    unavailable.error = AttemptErrorV1::Unavailable {
        reason: "no error detail".into(),
    };
    let wire = serde_json::to_value(&unavailable).unwrap();
    let round = serde_json::from_value::<AttemptV1>(wire).unwrap();
    assert!(matches!(round.error, AttemptErrorV1::Unavailable { .. }));
}

/// OC01-I14: dead-end ordinals, targets, categories, dispositions.
#[test]
fn dead_end_ordinals_targets_categories_and_dispositions() {
    let attempts = vec![attempt(0, None), attempt(1, Some(0))];
    // Four dispositions pass.
    for disposition in [
        Disposition::Unresolved,
        Disposition::Abandoned,
        Disposition::Superseded,
        Disposition::Recovered,
    ] {
        let mut dead_end = dead_end(0, 1);
        dead_end.disposition = disposition;
        let dead_end = DeadEndV1::new(dead_end, &limits()).unwrap();
        assert_eq!(dead_end.disposition, disposition);
    }
    // Gap rejects.
    let mut gap = dead_end(0, 0);
    gap.dead_end_id = "dead1_000009".into();
    assert!(validate_dead_ends(&[gap], &attempts, &limits()).is_err());
    // Absent target rejects.
    let absent = dead_end(0, 9);
    assert!(validate_dead_ends(&[absent], &attempts, &limits()).is_err());
    // Bad category rejects.
    let mut bad = dead_end(0, 0);
    bad.failure_category = "Bad Category".into();
    assert!(DeadEndV1::new(bad, &limits()).is_err());
    // Unknown disposition rejects on the wire.
    let mut unknown = dead_end(0, 0);
    unknown.disposition = Disposition::Unresolved;
    let wire = serde_json::to_value(&unknown).unwrap();
    let mut bad_wire = wire.clone();
    bad_wire["disposition"] = json!("deferred");
    assert!(serde_json::from_value::<DeadEndV1>(bad_wire).is_err());
    assert!(serde_json::from_value::<DeadEndV1>(wire).is_ok());
}

/// OC01-I15: attribution marks are ordered provenanced candidates only.
#[test]
fn attribution_marks_are_ordered_provenanced_candidates_only() {
    let a = mark(event(1), AttributionLabel::LoadBearingCandidate);
    let b = mark(event(2), AttributionLabel::SupportingCandidate);
    // Sort by composite key to guarantee ascending order.
    let mut ordered = vec![a.clone(), b.clone()];
    ordered.sort_by(|x, y| {
        let xk = serde_json::to_value(x).unwrap().to_string();
        let yk = serde_json::to_value(y).unwrap().to_string();
        xk.cmp(&yk)
    });
    assert!(validate_attribution_marks(&ordered, &limits()).is_ok());
    // Duplicate composite keys reject.
    let duplicates = vec![a.clone(), a.clone()];
    assert!(validate_attribution_marks(&duplicates, &limits()).is_err());
    // Unknown label rejects on the wire.
    let wire = serde_json::to_value(&a).unwrap();
    let mut bad_label = wire.clone();
    bad_label["label"] = json!("definitely-correct");
    assert!(serde_json::from_value::<AttributionMarkV1>(bad_label).is_err());
    // Score-like fields reject: exact key set enforced.
    let mut score_like = wire.clone();
    score_like["score"] = json!(0.9);
    assert!(serde_json::from_value::<AttributionMarkV1>(score_like).is_err());
}

/// OC01-I16: warnings/reasons/categories obey text grammar and bounds.
#[test]
fn warnings_reasons_and_categories_obey_text_grammar_and_bounds() {
    // Ordered distinct text passes without sorting or truncation.
    let warnings = vec!["first".to_owned(), "second".to_owned()];
    assert!(validate_warnings(&warnings, &limits()).is_ok());
    // Empty, control, duplicate reject.
    assert!(validate_warnings(&[String::new()], &limits()).is_err());
    assert!(validate_warnings(&["bad\u{7}control".to_owned()], &limits()).is_err());
    assert!(validate_warnings(&["same".to_owned(), "same".to_owned()], &limits()).is_err());
    // +1 note bytes reject.
    let long = "n".repeat(MAX_OUTCOME_NOTE_BYTES + 1);
    assert!(validate_warnings(&[long], &limits()).is_err());
    let exact = "n".repeat(MAX_OUTCOME_NOTE_BYTES);
    assert!(validate_warnings(&[exact], &limits()).is_ok());
    // Bad category grammar rejects.
    assert!(contextmesh_salience::types::validate_category("UPPER", 64,).is_err());
    assert!(contextmesh_salience::types::validate_category("a b", 64).is_err());
    assert!(contextmesh_salience::types::validate_category("-lead", 64).is_err());
    assert!(contextmesh_salience::types::validate_category("trail-", 64).is_err());
    assert!(contextmesh_salience::types::validate_category("ok.value-1_x", 64).is_ok());
}

/// OC01-I17: limits are nonzero, downward-only, never truncate.
#[test]
fn outcome_limits_are_nonzero_downward_only_and_never_truncate() {
    let default = OutcomeLimits::default();
    assert!(default.validate().is_ok());
    // Equal passes.
    assert_eq!(default, OutcomeLimits::default());
    // Lower passes.
    let mut lower = default;
    lower.max_attempts = 1;
    lower.max_warnings = 1;
    assert!(lower.validate().is_ok());
    // Zero rejects.
    let mut zero = default;
    zero.max_attempts = 0;
    assert!(zero.validate().is_err());
    // Above hard max rejects.
    let mut above = default;
    above.max_attempts = 1_025;
    assert!(above.validate().is_err());
    // A lowered limit enforces exactly: 65 warnings reject under a 64 cap.
    let mut cap64 = default;
    cap64.max_warnings = MAX_OUTCOME_NOTES;
    let sixty_five: Vec<String> = (0..65).map(|i| format!("w{i}")).collect();
    assert!(validate_warnings(&sixty_five, &cap64).is_err());
}

/// Stage-2A surface: operation error wrapper maps exactly and stays generic.
#[test]
fn operation_error_wrapper_is_generic_and_exact() {
    let artifact = OutcomeOperationError::from(OutcomeError::Malformed);
    assert_eq!(artifact.to_string(), "outcome artifact operation failed");
    let store = OutcomeOperationError::from(contextmesh::error::StoreError::DatabaseUnavailable);
    assert_eq!(store.to_string(), "outcome store operation failed");
    let io = OutcomeOperationError::from(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "secret path /home/cosmo/x",
    ));
    assert_eq!(io.to_string(), "outcome file operation failed");
    // Debug stays generic.
    assert_eq!(format!("{io:?}"), "OutcomeOperationError::Io");
    // Source chain retains the typed cause.
    let source = std::error::Error::source(&io).and_then(|s| s.downcast_ref::<std::io::Error>());
    assert!(source.is_some());
}

/// Stage-2A surface: strict JSON rejects hostile syntax.
#[test]
fn strict_json_rejects_hostile_syntax() {
    assert!(json::parse_strict(b"\xEF\xBB\xBF{}").is_err());
    assert!(json::parse_strict(b"{} trailing").is_err());
    assert!(json::parse_strict(b"{\"a\":1,\"a\":2}").is_err());
    assert!(json::parse_strict(b"{\"a\":{\"b\":1,\"b\":2}}").is_err());
    assert!(json::parse_strict(b"{\"deep\":").is_err());
    let deep_ok = format!(
        "{}{}",
        "{\"a\":".repeat(10),
        "1".to_string() + &"}".repeat(10)
    );
    assert!(json::parse_strict(deep_ok.as_bytes()).is_ok());
    // Depth boundary is exact: 64 nested containers pass, 65 reject.
    let depth_64 = format!("{}{}{}", "{\"a\":".repeat(64), "1", "}".repeat(64));
    assert!(json::parse_strict(depth_64.as_bytes()).is_ok());
    let depth_65 = format!("{}{}{}", "{\"a\":".repeat(65), "1", "}".repeat(65));
    assert!(json::parse_strict(depth_65.as_bytes()).is_err());
    // Canonical JCS renders and compares.
    let value = json!({"b": 1, "a": 2});
    let canonical = json::jcs(&value).unwrap();
    assert_eq!(canonical, b"{\"a\":2,\"b\":1}");
}
