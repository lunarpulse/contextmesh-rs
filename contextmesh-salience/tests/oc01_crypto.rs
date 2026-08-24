//! OC-01 Stage 2C crypto tests (matrix rows OC01-P01..P06, P09..P11).
//!
//! Literal ID/signature domains, raw-ID signature coverage, cross-domain
//! and typed-encoding rejection, the field-level tamper matrix, frozen
//! parse/verify precedence, committed golden fixture byte-exact
//! reconstruction, and the ignored fixture generator.
//!
//! Rows OC01-I25..I26 (hostile JSON and JCS canonicality matrices) belong to
//! `oc01_adversarial.rs` in Stage 2E and are intentionally absent here.

use contextmesh::crypto::SigningIdentity;
use contextmesh::model::{AuthorId, ContextId, EventId};
use contextmesh_salience::error::OutcomeError;
use contextmesh_salience::json;
use contextmesh_salience::outcome::{
    OutcomeLedgerBodyV1, SignedOutcomeLedgerV1, derive_outcome_id,
};
use contextmesh_salience::types::{
    AttemptErrorV1, AttemptStatus, AttemptV1, AttributionLabel, AttributionMarkV1, Blake3HashText,
    CostLedgerV1, CostValueV1, DeadEndV1, Disposition, InputRefSnapshotV1, LocalRefEntry,
    MechanismRecordV1, OUTCOME_ID_DOMAIN, OUTCOME_SIGNATURE_DOMAIN, OutcomeId, OutcomeLimits,
    OutcomeRecordV1, OutcomeSignature, OutcomeValue, QualityV1, RemoteRefEntry, TaskBindingV1,
    TerminalV1, TimestampText, UnterminatedReason,
};
use serde_json::{Value, json};

/// Published test-only signing seed for the golden terminal-event ledger.
const GOLDEN_SEED: [u8; 32] = [0x4f; 32];
/// Published test-only signing seed for the golden unterminated ledger.
const UNTERMINATED_SEED: [u8; 32] = [0x55; 32];

const GOLDEN_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/oc01-outcome-ledger-v1-golden.json"
);
const UNTERMINATED_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/oc01-outcome-ledger-v1-unterminated.json"
);

fn limits() -> OutcomeLimits {
    OutcomeLimits::default()
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

fn event(id: u8) -> EventId {
    EventId::from_bytes([id; 32])
}

fn context() -> ContextId {
    ContextId::from_bytes([7; 32])
}

fn mechanism_named(identity_text: &str) -> MechanismRecordV1 {
    MechanismRecordV1::new(
        identity_text.to_owned(),
        "1.0.0".to_owned(),
        hash_text_of(b"oc01-crypto-mechanism"),
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

fn attempt(index: usize, parent: Option<usize>) -> AttemptV1 {
    AttemptV1::new(
        AttemptV1 {
            attempt_id: format!("attempt1_{index:06}"),
            parent_attempt_id: parent.map(|p| format!("attempt1_{p:06}")),
            status: AttemptStatus::Failed,
            operation_fingerprint: hash_text_of(b"oc01-crypto-attempt"),
            event_refs: vec![event(1), event(2)],
            error: AttemptErrorV1::Available {
                category: "provider-timeout".to_owned(),
                fingerprint: hash_text_of(b"oc01-crypto-error"),
            },
            costs: cost_ledger_mixed(),
            provenance: mechanism_named("caller.example"),
        },
        &limits(),
    )
    .expect("attempt is valid")
}

fn attempt_unavailable(index: usize) -> AttemptV1 {
    AttemptV1::new(
        AttemptV1 {
            attempt_id: format!("attempt1_{index:06}"),
            parent_attempt_id: None,
            status: AttemptStatus::Failed,
            operation_fingerprint: hash_text_of(b"oc01-crypto-attempt"),
            event_refs: vec![event(1)],
            error: AttemptErrorV1::Unavailable {
                reason: "error detail not captured".to_owned(),
            },
            costs: cost_ledger_unavailable(),
            provenance: mechanism_named("caller.example"),
        },
        &limits(),
    )
    .expect("attempt is valid")
}

fn dead_end_with(index: usize, attempt_index: usize, disposition: Disposition) -> DeadEndV1 {
    DeadEndV1::new(
        DeadEndV1 {
            dead_end_id: format!("dead1_{index:06}"),
            attempt_id: format!("attempt1_{attempt_index:06}"),
            failure_category: "provider-timeout".to_owned(),
            error_fingerprint: hash_text_of(b"oc01-crypto-dead-end"),
            event_refs: vec![event(2)],
            disposition,
            provenance: mechanism_named("caller.example"),
        },
        &limits(),
    )
    .expect("dead end is valid")
}

fn mark_with(
    event_id: EventId,
    label: AttributionLabel,
    mechanism_identity: &str,
) -> AttributionMarkV1 {
    AttributionMarkV1::new(
        AttributionMarkV1 {
            event: event_id,
            label,
            evidence: vec![event(3)],
            mechanism: mechanism_named(mechanism_identity),
        },
        &limits(),
    )
    .expect("mark is valid")
}

fn input_refs() -> InputRefSnapshotV1 {
    InputRefSnapshotV1::new(
        context(),
        vec![LocalRefEntry {
            name: "main".to_owned(),
            head: event(11),
        }],
        vec![RemoteRefEntry {
            peer: "peer.example".to_owned(),
            name: "main".to_owned(),
            head: event(13),
        }],
    )
    .expect("input refs are valid")
}

/// Fixed terminal-event ledger body: mixed cost availability, multi-level
/// attempt tree, recovered/unresolved/abandoned dead ends, and multiple
/// attribution mechanisms (P09 fixture identity).
fn golden_body(author: AuthorId) -> OutcomeLedgerBodyV1 {
    OutcomeLedgerBodyV1::new(
        context(),
        input_refs(),
        TaskBindingV1::new(
            hash_text_of(b"oc01-golden-task"),
            Some(hash_text_of(b"oc01-golden-structured")),
            None,
            &limits(),
        )
        .expect("task binding is valid"),
        TerminalV1::Event { event: event(5) },
        OutcomeRecordV1::new(
            OutcomeValue::Succeeded,
            vec![event(5), event(6)],
            mechanism_named("caller.example"),
            &limits(),
        )
        .expect("outcome record is valid"),
        QualityV1::new(
            QualityV1::Available {
                value_ppm: 990_000,
                evidence: vec![event(5), event(6)],
                provenance: mechanism_named("caller.example"),
            },
            &limits(),
        )
        .expect("quality is valid"),
        cost_ledger_mixed(),
        vec![
            attempt(0, None),
            attempt(1, Some(0)),
            attempt(2, Some(1)),
            attempt(3, Some(0)),
        ],
        vec![
            dead_end_with(0, 1, Disposition::Unresolved),
            dead_end_with(1, 1, Disposition::Recovered),
            dead_end_with(2, 2, Disposition::Abandoned),
        ],
        vec![
            mark_with(
                event(1),
                AttributionLabel::LoadBearingCandidate,
                "mechanism-a",
            ),
            mark_with(
                event(2),
                AttributionLabel::SupportingCandidate,
                "mechanism-b",
            ),
            mark_with(event(4), AttributionLabel::DeadEndCandidate, "mechanism-c"),
        ],
        vec!["collector warning".to_owned()],
        TimestampText::parse("2026-08-21T00:00:00Z").expect("timestamp is valid"),
        author,
        limits(),
    )
    .expect("golden body is valid")
}

/// Fixed explicit-unterminated ledger body: unavailable clock, calls,
/// retries, and tokens, with the exact terminal reason (P10 fixture
/// identity).
fn unterminated_body(author: AuthorId) -> OutcomeLedgerBodyV1 {
    OutcomeLedgerBodyV1::new(
        context(),
        input_refs(),
        TaskBindingV1::new(
            hash_text_of(b"oc01-unterminated-task"),
            None,
            None,
            &limits(),
        )
        .expect("task binding is valid"),
        TerminalV1::Unterminated {
            reason: UnterminatedReason::CancelledBeforeTerminal,
        },
        OutcomeRecordV1::new(
            OutcomeValue::Unknown,
            vec![],
            mechanism_named("caller.example"),
            &limits(),
        )
        .expect("outcome record is valid"),
        QualityV1::new(
            QualityV1::Unavailable {
                reason: "no recorded rubric".to_owned(),
                provenance: mechanism_named("caller.example"),
            },
            &limits(),
        )
        .expect("quality is valid"),
        cost_ledger_unavailable(),
        vec![attempt_unavailable(0)],
        vec![],
        vec![],
        vec![],
        TimestampText::parse("2026-08-21T00:00:00Z").expect("timestamp is valid"),
        author,
        limits(),
    )
    .expect("unterminated body is valid")
}

fn golden_ledger() -> SignedOutcomeLedgerV1 {
    let identity = identity(GOLDEN_SEED);
    SignedOutcomeLedgerV1::issue(&identity, golden_body(identity.author()), limits())
        .expect("golden ledger issues")
}

fn unterminated_ledger() -> SignedOutcomeLedgerV1 {
    let identity = identity(UNTERMINATED_SEED);
    SignedOutcomeLedgerV1::issue(&identity, unterminated_body(identity.author()), limits())
        .expect("unterminated ledger issues")
}

// ---------------------------------------------------------------------------
// Wire mutation helpers
// ---------------------------------------------------------------------------

fn envelope_value(ledger: &SignedOutcomeLedgerV1) -> Value {
    let wire = ledger.to_wire(limits()).expect("wire renders");
    serde_json::from_slice(&wire).expect("wire is valid JSON")
}

fn render(value: &Value) -> Vec<u8> {
    json::jcs(value).expect("envelope JCS renders")
}

/// Renders `value` as valid strict JSON that is deliberately NOT JCS.
fn render_noncanonical(value: &Value) -> Vec<u8> {
    serde_json::to_vec_pretty(value).expect("pretty JSON renders")
}

fn envelope_with_signature(ledger: &SignedOutcomeLedgerV1, raw: [u8; 64]) -> Vec<u8> {
    let mut value = envelope_value(ledger);
    value["signature"] = json!(OutcomeSignature::from_bytes(raw).to_string());
    render(&value)
}

fn envelope_with_replaced_signature(ledger: &SignedOutcomeLedgerV1, text: &str) -> Vec<u8> {
    let mut value = envelope_value(ledger);
    value["signature"] = json!(text);
    render(&value)
}

/// Parses a top-level replacement and returns the rejection category.
fn parse_replaced(base: &Value, field: &str, replacement: Value) -> OutcomeError {
    let mut value = base.clone();
    value[field] = replacement;
    let rendered = render(&value);
    SignedOutcomeLedgerV1::from_wire(&rendered, limits()).expect_err("mutated envelope must reject")
}

fn pointer_set(value: &mut Value, path: &str, replacement: Value) {
    *value
        .pointer_mut(path)
        .unwrap_or_else(|| panic!("path {path} exists in fixture")) = replacement;
}

/// OC01-P01: the ID is ordinary BLAKE3 over the literal NUL-terminated ID
/// domain plus exact JCS body bytes — never derive-key mode.
#[test]
fn outcome_id_uses_literal_domain_prefix_hashing() {
    let ledger = golden_ledger();
    let canonical_body = json::jcs(ledger.body()).expect("body JCS renders");

    // Literal domain bytes, including the NUL terminator.
    assert_eq!(OUTCOME_ID_DOMAIN.last(), Some(&0));
    assert_eq!(
        OUTCOME_ID_DOMAIN,
        b"org.aaif.contextmesh.oc.outcome-ledger-id.v1\0"
    );
    // The literal backslash-plus-zero text is a different byte string.
    assert_ne!(
        OUTCOME_ID_DOMAIN,
        b"org.aaif.contextmesh.oc.outcome-ledger-id.v1\\0"
    );

    // Published vector: literal prefix hashing over domain || body.
    let mut hasher = blake3::Hasher::new();
    hasher.update(OUTCOME_ID_DOMAIN);
    hasher.update(&canonical_body);
    let expected = OutcomeId::from_bytes(*hasher.finalize().as_bytes());
    assert_eq!(ledger.outcome_id(), expected);
    assert_eq!(
        ledger.outcome_id(),
        derive_outcome_id(ledger.body(), limits()).expect("derivation is valid")
    );

    // Derive-key mode over the same material produces a different ID.
    let context = std::str::from_utf8(OUTCOME_ID_DOMAIN).expect("domain is UTF-8");
    let derive_key_id = OutcomeId::from_bytes(blake3::derive_key(context, &canonical_body));
    assert_ne!(ledger.outcome_id(), derive_key_id);

    // Undomained plain body hashing also differs.
    let undomained = OutcomeId::from_bytes(*blake3::hash(&canonical_body).as_bytes());
    assert_ne!(ledger.outcome_id(), undomained);
}

/// OC01-P02: the signature is Ed25519 over the literal signature domain plus
/// the raw 32-byte ID — never the ID text or body bytes.
#[test]
fn signature_covers_domain_and_raw_id_bytes() {
    let identity = identity(GOLDEN_SEED);
    let ledger = golden_ledger();
    let raw_id = ledger.outcome_id().to_bytes();

    // Literal domain bytes, including the NUL terminator.
    assert_eq!(OUTCOME_SIGNATURE_DOMAIN.last(), Some(&0));
    assert_eq!(
        OUTCOME_SIGNATURE_DOMAIN,
        b"org.aaif.contextmesh.oc.outcome-ledger-signature.v1\0"
    );

    // Published vector: signature over domain || raw ID bytes.
    let signed = identity.sign_domain_message(OUTCOME_SIGNATURE_DOMAIN, &raw_id);
    let exact: [u8; 64] = signed.try_into().expect("signature is 64 bytes");
    assert_eq!(ledger.signature().to_bytes(), exact);
    ledger.verify(limits()).expect("exact vector verifies");

    // ID-text signing alternative produces a different signature that fails.
    let id_text = ledger.outcome_id().to_string();
    let id_text_signed = identity.sign_domain_message(OUTCOME_SIGNATURE_DOMAIN, id_text.as_bytes());
    let id_text_sig: [u8; 64] = id_text_signed.try_into().expect("signature is 64 bytes");
    assert_ne!(ledger.signature().to_bytes(), id_text_sig);
    let envelope = envelope_with_signature(&ledger, id_text_sig);
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(&envelope, limits()).unwrap_err(),
        OutcomeError::SignatureInvalid
    );

    // Body-bytes signing alternative also differs and fails verification.
    let body_bytes = json::jcs(ledger.body()).expect("body JCS renders");
    let body_signed = identity.sign_domain_message(OUTCOME_SIGNATURE_DOMAIN, &body_bytes);
    let body_sig: [u8; 64] = body_signed.try_into().expect("signature is 64 bytes");
    assert_ne!(ledger.signature().to_bytes(), body_sig);
    let envelope = envelope_with_signature(&ledger, body_sig);
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(&envelope, limits()).unwrap_err(),
        OutcomeError::SignatureInvalid
    );
}

/// OC01-P03: cross-type IDs, signatures, prefixes, lengths, alphabets,
/// padding, domains, and authors all reject with stable categories.
#[test]
fn cross_domain_typed_encoding_and_author_mismatch_matrix() {
    let ledger = golden_ledger();
    let base = envelope_value(&ledger);

    // Cross-type ID prefix (event prefix on an outcome ID).
    let mut evt_prefixed = String::from("evt1_");
    evt_prefixed.push_str(&ledger.outcome_id().to_string()["ocout1_".len()..]);
    assert_eq!(
        parse_replaced(&base, "outcome_id", json!(evt_prefixed)),
        OutcomeError::Malformed
    );

    // Wrong length: truncated ID text.
    let full = ledger.outcome_id().to_string();
    assert_eq!(
        parse_replaced(&base, "outcome_id", json!(full[..full.len() - 1])),
        OutcomeError::Malformed
    );

    // Wrong alphabet: uppercase encoding.
    let upper: String = full.chars().map(|c| c.to_ascii_uppercase()).collect();
    assert_eq!(
        parse_replaced(&base, "outcome_id", json!(upper)),
        OutcomeError::Malformed
    );

    // Padding: base64 padding character appended.
    assert_eq!(
        parse_replaced(&base, "outcome_id", json!(format!("{full}="))),
        OutcomeError::Malformed
    );

    // Valid typed but wrong ID bytes bind to a different body.
    let wrong_id = OutcomeId::from_bytes([9; 32]).to_string();
    assert_eq!(
        parse_replaced(&base, "outcome_id", json!(wrong_id)),
        OutcomeError::IdMismatch
    );

    // Signature prefix, length, and alphabet.
    let sig = ledger.signature().to_string();
    let mut wrong_sig_prefix = String::from("ocsig2_");
    wrong_sig_prefix.push_str(&sig["ocsig1_".len()..]);
    assert_eq!(
        parse_replaced(&base, "signature", json!(wrong_sig_prefix)),
        OutcomeError::Malformed
    );
    assert_eq!(
        parse_replaced(&base, "signature", json!(sig[..sig.len() - 2])),
        OutcomeError::Malformed
    );
    let upper_sig: String = sig.chars().map(|c| c.to_ascii_uppercase()).collect();
    assert_eq!(
        parse_replaced(&base, "signature", json!(upper_sig)),
        OutcomeError::Malformed
    );

    // Author mismatch: the declared author is inside the signed body, so
    // issue() refuses to sign a body owned by another identity.
    let signer = identity(GOLDEN_SEED);
    let other = identity(UNTERMINATED_SEED);
    assert_eq!(
        SignedOutcomeLedgerV1::issue(&signer, golden_body(other.author()), limits()).unwrap_err(),
        OutcomeError::IdMismatch
    );

    // Cross-domain: a signature made under the ID domain never verifies
    // under the signature domain.
    let id_domain_signed =
        signer.sign_domain_message(OUTCOME_ID_DOMAIN, &ledger.outcome_id().to_bytes());
    let cross_domain_sig: [u8; 64] = id_domain_signed.try_into().expect("signature is 64 bytes");
    let envelope = envelope_with_signature(&ledger, cross_domain_sig);
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(&envelope, limits()).unwrap_err(),
        OutcomeError::SignatureInvalid
    );
}

/// OC01-P04: tampering with any signed or derived component rejects, and no
/// repaired or re-sorted artifact is ever returned.
#[test]
fn tamper_matrix_rejects_every_signed_or_derived_component() {
    let ledger = golden_ledger();
    let wire = ledger.to_wire(limits()).expect("wire renders");
    let base = envelope_value(&ledger);

    // (pointer path, replacement) covering every P04 component.
    let cases: Vec<(&str, Value)> = vec![
        // Body-level tamper via a nested signed field.
        (
            "/body/task/content_hash",
            json!(hash_text_of(b"tampered-task").to_string()),
        ),
        // Snapshot binding tamper: moved local head invalidates fingerprint.
        (
            "/body/input_refs/local/0/head",
            json!(event(15).to_string()),
        ),
        // Snapshot fingerprint tamper: a correctly-typed fingerprint of a
        // DIFFERENT snapshot, so the declared binding no longer holds.
        (
            "/body/input_refs/fingerprint",
            json!(
                InputRefSnapshotV1::compute_fingerprint(
                    &context(),
                    &[LocalRefEntry {
                        name: "main".to_owned(),
                        head: event(12)
                    },],
                    &[],
                )
                .expect("computes")
                .to_string()
            ),
        ),
        // Derived ID tamper.
        (
            "/outcome_id",
            json!(OutcomeId::from_bytes([9; 32]).to_string()),
        ),
        // Signature tamper (first base64 data character flipped; the last
        // character carries discardable padding bits, so it is not usable).
        (
            "/signature",
            json!(flip_first_data_char(
                &ledger.signature().to_string(),
                "ocsig1_"
            )),
        ),
        // Outcome value tamper.
        ("/body/outcome/value", json!("failed")),
        // Quality tamper.
        ("/body/quality/value_ppm", json!(500_000)),
        // Cost tamper.
        ("/body/costs/tool_calls/value", json!(7)),
        // Attempt tamper.
        ("/body/attempts/1/status", json!("succeeded")),
        // Dead-end tamper.
        ("/body/dead_ends/0/disposition", json!("recovered")),
        // Attribution-mark tamper.
        (
            "/body/attribution_marks/0/label",
            json!("supporting-candidate"),
        ),
        // Author tamper.
        (
            "/body/author",
            json!(identity(UNTERMINATED_SEED).author().to_string()),
        ),
        // Timestamp tamper.
        ("/body/created_at", json!("2026-08-22T00:00:00Z")),
    ];

    for (path, replacement) in cases {
        let mut value = base.clone();
        pointer_set(&mut value, path, replacement);
        let rendered = render(&value);
        assert!(
            SignedOutcomeLedgerV1::from_wire(&rendered, limits()).is_err(),
            "tamper case {path} must reject"
        );
    }

    // No partial artifact: the in-memory ledger is untouched and re-renders
    // to the exact same bytes.
    ledger.verify(limits()).expect("original still verifies");
    assert_eq!(ledger.to_wire(limits()).expect("re-renders"), wire);
}

/// OC01-P06: structural parse/verify is store-free, claim-bounded, and
/// freezes the precedence wire bound -> parse -> schema -> canonicality ->
/// ID -> signature.
#[test]
fn structural_verify_is_store_free_claim_bounded_and_precedence_exact() {
    let ledger = golden_ledger();

    // Store-free: a valid immutable artifact verifies with no Store access;
    // the only inputs are the artifact and caller limits.
    ledger.verify(limits()).expect("valid artifact verifies");
    let wire = ledger.to_wire(limits()).expect("wire renders");
    SignedOutcomeLedgerV1::from_wire(&wire, limits()).expect("valid artifact parses");

    // (a) Wire bound precedes parse: an oversized input whose content is
    //     also unparseable still reports the bound category, never malformed.
    let mut tiny = limits();
    tiny.max_wire_bytes = 16;
    tiny.validate().expect("tiny limits are legal");
    let oversized_garbage = b"{ this garbage input is deliberately longer than sixteen bytes";
    assert!(oversized_garbage.len() > tiny.max_wire_bytes);
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(oversized_garbage, tiny).unwrap_err(),
        OutcomeError::LimitExceeded
    );

    // (b) Parse precedes schema: syntactically invalid JSON is malformed.
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(b"{not json", limits()).unwrap_err(),
        OutcomeError::Malformed
    );

    // (c) Schema precedes canonicality: an unknown member inside non-JCS
    //     bytes reports the schema category, not noncanonical.
    let base = envelope_value(&ledger);
    let mut unknown = base.clone();
    unknown["unexpected"] = json!(true);
    let noncanonical_unknown = render_noncanonical(&unknown);
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(&noncanonical_unknown, limits()).unwrap_err(),
        OutcomeError::Malformed
    );

    // (d) Canonicality precedes ID: non-JCS bytes of a body with a wrong
    //     embedded ID still report noncanonical, not id-mismatch.
    let mut wrong_id = base.clone();
    wrong_id["outcome_id"] = json!(OutcomeId::from_bytes([9; 32]).to_string());
    let noncanonical_wrong_id = render_noncanonical(&wrong_id);
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(&noncanonical_wrong_id, limits()).unwrap_err(),
        OutcomeError::Noncanonical
    );

    // (e) ID precedes signature: an author-swapped body breaks the ID
    //     binding AND the signature, and reports id-mismatch.
    let mut author_swapped = base.clone();
    author_swapped["body"]["author"] = json!(identity(UNTERMINATED_SEED).author().to_string());
    let canonical_swap = render(&author_swapped);
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(&canonical_swap, limits()).unwrap_err(),
        OutcomeError::IdMismatch
    );

    // (f) Signature is last: a correct ID binding with a flipped signature
    //     reports signature-invalid.
    let flipped = envelope_with_replaced_signature(
        &ledger,
        &flip_first_data_char(&ledger.signature().to_string(), "ocsig1_"),
    );
    assert_eq!(
        SignedOutcomeLedgerV1::from_wire(&flipped, limits()).unwrap_err(),
        OutcomeError::SignatureInvalid
    );
}

/// OC01-P09: reconstruction equals the committed terminal-event golden
/// fixture byte-for-byte, including the typed ID and signature.
#[test]
fn terminal_golden_fixture_matches_bytes_id_and_signature() {
    let ledger = golden_ledger();
    let wire = ledger.to_wire(limits()).expect("wire renders");
    let committed = std::fs::read(GOLDEN_FIXTURE)
        .expect("committed golden fixture exists; see the ignored generator for change control");
    assert_eq!(wire, committed);

    let parsed =
        SignedOutcomeLedgerV1::from_wire(&committed, limits()).expect("committed fixture verifies");
    assert_eq!(parsed.outcome_id(), ledger.outcome_id());
    assert_eq!(parsed.signature().to_bytes(), ledger.signature().to_bytes());
    assert_eq!(parsed.body().version(), 1);
}

/// OC01-P10: reconstruction equals the committed unterminated golden
/// fixture byte-for-byte, including the typed ID and signature.
#[test]
fn unterminated_golden_fixture_matches_bytes_id_and_signature() {
    let ledger = unterminated_ledger();
    let wire = ledger.to_wire(limits()).expect("wire renders");
    let committed = std::fs::read(UNTERMINATED_FIXTURE)
        .expect("committed unterminated fixture exists; see the ignored generator");
    assert_eq!(wire, committed);

    let parsed =
        SignedOutcomeLedgerV1::from_wire(&committed, limits()).expect("committed fixture verifies");
    assert_eq!(parsed.outcome_id(), ledger.outcome_id());
    assert_eq!(parsed.signature().to_bytes(), ledger.signature().to_bytes());
    assert_eq!(parsed.body().version(), 1);
}

/// OC01-P11: golden updates are never automatic — committed vectors are
/// normal-test inputs and any fixture drift is detected by reconstruction.
#[test]
fn golden_generator_is_ignored_and_fixtures_are_immutable_inputs() {
    let committed = std::fs::read(GOLDEN_FIXTURE).expect("golden fixture exists");

    // A different test-only identity reconstructs to different bytes, so the
    // byte-exact comparison above detects any fixture drift.
    let other = SignedOutcomeLedgerV1::issue(
        &identity(UNTERMINATED_SEED),
        golden_body(identity(UNTERMINATED_SEED).author()),
        limits(),
    )
    .expect("issues");
    assert_ne!(other.to_wire(limits()).expect("renders"), committed);

    // The two committed fixtures are distinct vectors.
    let unterminated = std::fs::read(UNTERMINATED_FIXTURE).expect("unterminated fixture exists");
    assert_ne!(committed, unterminated);
}

/// Regenerates the two committed golden fixtures.
///
/// This generator is `#[ignore]`d: fixture updates require explicit human
/// change control, never automatic regeneration. The generator and the
/// comparison tests intentionally share one construction path, so a committed
/// fixture can only change when this file changes.
#[test]
#[ignore = "fixture regeneration requires explicit human change control"]
fn generate_golden_fixtures() {
    let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
    std::fs::create_dir_all(directory).expect("fixture directory exists");
    std::fs::write(
        GOLDEN_FIXTURE,
        golden_ledger().to_wire(limits()).expect("renders"),
    )
    .expect("golden fixture written");
    std::fs::write(
        UNTERMINATED_FIXTURE,
        unterminated_ledger().to_wire(limits()).expect("renders"),
    )
    .expect("unterminated fixture written");
}

fn flip_first_data_char(text: &str, prefix: &str) -> String {
    let index = prefix.len();
    let mut chars: Vec<char> = text.chars().collect();
    assert!(
        chars.len() > index + 1,
        "text must have a non-final data char"
    );
    chars[index] = if chars[index] == 'A' { 'B' } else { 'A' };
    chars.into_iter().collect()
}
