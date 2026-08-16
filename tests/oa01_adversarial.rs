//! Adversarial, malformed, mutation, and boundary coverage for OA-01.

use std::error::Error;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::str::FromStr;

use contextmesh::crypto::{SigningIdentity, derive_event_id};
use contextmesh::error::ContractError;
use contextmesh::model::{
    AuthorId, ContextId, EventBodyV1, EventId, EventSignature, MAX_CANONICAL_BODY_BYTES,
    MAX_CANONICAL_PAYLOAD_BYTES, MAX_PAYLOAD_DEPTH, MAX_RAW_WIRE_BYTES, SignedEventV1,
    canonical_payload_bytes,
};
use ed25519_dalek::{Signer as _, SigningKey};
use serde_json::{Value, json};

const SEED: [u8; 32] = [7; 32];

fn identity() -> SigningIdentity {
    SigningIdentity::from_fixture_seed(SEED)
}

fn base_event() -> SignedEventV1 {
    identity()
        .create_event(
            ContextId::from_bytes([3; 32]),
            Vec::new(),
            "agent.request",
            json!({"nested": {"value": 1}, "text": "hello"}),
        )
        .unwrap()
}

fn wire_value(event: &SignedEventV1) -> Value {
    serde_json::from_slice(&event.to_wire().unwrap()).unwrap()
}

fn parse_value(value: &Value) -> Result<SignedEventV1, ContractError> {
    SignedEventV1::from_wire(&serde_json::to_vec(value).unwrap())
}

#[test]
fn generated_identity_signs_and_round_trips() -> Result<(), Box<dyn Error>> {
    let generated = SigningIdentity::generate()?;
    let event = generated.create_event(
        ContextId::from_bytes([4; 32]),
        Vec::new(),
        "agent.response",
        json!({"ok": true}),
    )?;
    event.verify()?;
    SignedEventV1::from_wire(&event.to_wire()?)?.verify()?;
    Ok(())
}

#[test]
fn every_signed_field_mutation_is_rejected() {
    let event = base_event();
    let valid = wire_value(&event);

    let mut mutations = Vec::new();
    let mut version = valid.clone();
    version["body"]["version"] = json!(2);
    mutations.push((version, ContractError::UnsupportedVersion));

    let mut context = valid.clone();
    context["body"]["context"] = json!(ContextId::from_bytes([5; 32]).to_string());
    mutations.push((context, ContractError::IdMismatch));

    let mut parents = valid.clone();
    parents["body"]["parents"] = json!([EventId::from_bytes([0; 32]).to_string()]);
    mutations.push((parents, ContractError::IdMismatch));

    let mut kind = valid.clone();
    kind["body"]["kind"] = json!("agent.response");
    mutations.push((kind, ContractError::IdMismatch));

    let mut author = valid.clone();
    author["body"]["author"] = json!(identity_with_seed([8; 32]).author().to_string());
    mutations.push((author, ContractError::IdMismatch));

    let mut payload = valid.clone();
    payload["body"]["payload"]["text"] = json!("tampered");
    mutations.push((payload, ContractError::IdMismatch));

    let mut id = valid.clone();
    let mut id_bytes = event.event_id().to_bytes();
    id_bytes[0] ^= 1;
    id["event_id"] = json!(EventId::from_bytes(id_bytes).to_string());
    mutations.push((id, ContractError::IdMismatch));

    let mut signature = valid;
    let mut signature_bytes = event.signature().to_bytes();
    signature_bytes[0] ^= 1;
    signature["signature"] = json!(EventSignature::from_bytes(signature_bytes).to_string());
    mutations.push((signature, ContractError::SignatureInvalid));

    for (mutation, expected) in mutations {
        assert_eq!(parse_value(&mutation).unwrap_err(), expected);
    }
}

fn identity_with_seed(seed: [u8; 32]) -> SigningIdentity {
    SigningIdentity::from_fixture_seed(seed)
}

#[test]
fn author_mismatch_and_wrong_signature_domain_are_typed_failures() -> Result<(), Box<dyn Error>> {
    let signer = identity();
    let other = identity_with_seed([9; 32]);
    let body = EventBodyV1::new(
        ContextId::from_bytes([1; 32]),
        Vec::new(),
        "agent.request",
        other.author(),
        json!({}),
    )?;
    assert_eq!(
        signer.sign_body(body).unwrap_err(),
        ContractError::AuthorMismatch
    );

    let direct_key = SigningKey::from_bytes(&SEED);
    let body = EventBodyV1::new(
        ContextId::from_bytes([1; 32]),
        Vec::new(),
        "agent.request",
        signer.author(),
        json!({}),
    )?;
    let id = derive_event_id(&body)?;
    let wrong_signature = direct_key.sign(b"org.aaif.contextmesh.signature.v1-WRONG");
    let wire = json!({
        "event_id": id.to_string(),
        "body": serde_json::from_slice::<Value>(&body.canonical_bytes()?)?,
        "signature": EventSignature::from_bytes(wrong_signature.to_bytes()).to_string()
    });
    assert_eq!(
        parse_value(&wire).unwrap_err(),
        ContractError::SignatureInvalid
    );
    Ok(())
}

#[test]
fn malformed_author_and_signature_are_rejected() -> Result<(), Box<dyn Error>> {
    let body = EventBodyV1::new(
        ContextId::from_bytes([1; 32]),
        Vec::new(),
        "agent.request",
        AuthorId::from_bytes([255; 32]),
        json!({}),
    )?;
    let id = derive_event_id(&body)?;
    let wire = json!({
        "event_id": id.to_string(),
        "body": serde_json::from_slice::<Value>(&body.canonical_bytes()?)?,
        "signature": EventSignature::from_bytes([0; 64]).to_string()
    });
    assert_eq!(
        parse_value(&wire).unwrap_err(),
        ContractError::SignatureInvalid
    );

    let mut valid = wire_value(&base_event());
    valid["signature"] = json!("sig1_short");
    assert_eq!(
        parse_value(&valid).unwrap_err(),
        ContractError::InvalidEncoding
    );
    Ok(())
}

#[test]
fn strict_ed25519_rejects_noncanonical_s_and_small_order_keys() -> Result<(), Box<dyn Error>> {
    // Add the Ed25519 subgroup order L to the valid scalar S. The resulting
    // signature is mathematically related but noncanonical and must be rejected.
    const L: [u8; 32] = [
        0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde,
        0x14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10,
    ];
    let event = base_event();
    let mut signature = event.signature().to_bytes();
    let mut carry = 0_u16;
    for (byte, addend) in signature[32..].iter_mut().zip(L) {
        let sum = u16::from(*byte) + u16::from(addend) + carry;
        *byte = sum as u8;
        carry = sum >> 8;
    }
    assert_eq!(carry, 0);
    let mut wire = wire_value(&event);
    wire["signature"] = json!(EventSignature::from_bytes(signature).to_string());
    assert_eq!(
        parse_value(&wire).unwrap_err(),
        ContractError::SignatureInvalid
    );

    // The identity point is a valid compressed Edwards encoding but a weak,
    // small-order public key. Strict verification must reject it.
    let mut identity_point = [0_u8; 32];
    identity_point[0] = 1;
    let body = EventBodyV1::new(
        ContextId::from_bytes([1; 32]),
        Vec::new(),
        "agent.request",
        AuthorId::from_bytes(identity_point),
        json!({}),
    )?;
    let id = derive_event_id(&body)?;
    let wire = json!({
        "event_id": id.to_string(),
        "body": serde_json::from_slice::<Value>(&body.canonical_bytes()?)?,
        "signature": EventSignature::from_bytes([0; 64]).to_string()
    });
    assert_eq!(
        parse_value(&wire).unwrap_err(),
        ContractError::SignatureInvalid
    );
    Ok(())
}

#[test]
fn duplicate_keys_are_rejected_at_every_depth() {
    let event = base_event();
    let wire = String::from_utf8(event.to_wire().unwrap()).unwrap();
    let envelope = wire.replacen(
        &format!("\"event_id\":\"{}\"", event.event_id()),
        &format!(
            "\"event_id\":\"{}\",\"event_id\":\"{}\"",
            event.event_id(),
            event.event_id()
        ),
        1,
    );
    assert_eq!(
        SignedEventV1::from_wire(envelope.as_bytes()).unwrap_err(),
        ContractError::DuplicateKey
    );

    let body = wire.replacen("\"version\":1", "\"version\":1,\"version\":1", 1);
    assert_eq!(
        SignedEventV1::from_wire(body.as_bytes()).unwrap_err(),
        ContractError::DuplicateKey
    );

    let payload = wire.replacen("\"value\":1", "\"value\":1,\"value\":1", 1);
    assert_eq!(
        SignedEventV1::from_wire(payload.as_bytes()).unwrap_err(),
        ContractError::DuplicateKey
    );

    let mut deepest_payload = r#"{"x":1,"x":2}"#.to_owned();
    for _ in 0..63 {
        deepest_payload = format!(r#"{{"nested":{deepest_payload}}}"#);
    }
    let deepest_body = format!(
        r#"{{"version":1,"context":"{}","parents":[],"kind":"agent.request","author":"{}","payload":{deepest_payload}}}"#,
        ContextId::from_bytes([3; 32]),
        identity().author()
    );
    assert_eq!(
        EventBodyV1::from_json(deepest_body.as_bytes()).unwrap_err(),
        ContractError::DuplicateKey
    );
}

#[test]
fn malformed_field_sets_versions_bom_and_trailing_data_are_typed() {
    let valid = wire_value(&base_event());

    let mut unknown_envelope = valid.clone();
    unknown_envelope["extra"] = json!(true);
    assert_eq!(
        parse_value(&unknown_envelope).unwrap_err(),
        ContractError::UnknownField
    );

    let mut unknown_body = valid.clone();
    unknown_body["body"]["extra"] = json!(true);
    assert_eq!(
        parse_value(&unknown_body).unwrap_err(),
        ContractError::UnknownField
    );

    let mut missing = valid.clone();
    missing.as_object_mut().unwrap().remove("signature");
    assert_eq!(
        parse_value(&missing).unwrap_err(),
        ContractError::MissingField
    );

    let mut missing_body = valid.clone();
    missing_body["body"]
        .as_object_mut()
        .unwrap()
        .remove("payload");
    assert_eq!(
        parse_value(&missing_body).unwrap_err(),
        ContractError::MissingField
    );

    let mut wrong_type = valid.clone();
    wrong_type["body"]["parents"] = json!({});
    assert_eq!(
        parse_value(&wrong_type).unwrap_err(),
        ContractError::JsonSyntax
    );

    for version in [0, 2] {
        let mut unsupported = valid.clone();
        unsupported["body"]["version"] = json!(version);
        assert_eq!(
            parse_value(&unsupported).unwrap_err(),
            ContractError::UnsupportedVersion
        );
    }

    let canonical = base_event().to_wire().unwrap();
    let mut bom = vec![0xef, 0xbb, 0xbf];
    bom.extend_from_slice(&canonical);
    assert_eq!(
        SignedEventV1::from_wire(&bom).unwrap_err(),
        ContractError::JsonSyntax
    );
    let mut trailing = canonical;
    trailing.extend_from_slice(b" true");
    assert_eq!(
        SignedEventV1::from_wire(&trailing).unwrap_err(),
        ContractError::JsonSyntax
    );
    assert_eq!(
        SignedEventV1::from_wire(br#"{"event_id":"#).unwrap_err(),
        ContractError::JsonSyntax
    );
}

#[test]
fn typed_text_encodings_are_canonical_and_exact() {
    let id = EventId::from_bytes([11; 32]);
    let canonical = id.to_string();
    assert_eq!(EventId::from_str(&canonical).unwrap(), id);
    assert_eq!(
        EventId::from_str(&(canonical.clone() + "=")).unwrap_err(),
        ContractError::InvalidEncoding
    );
    assert_eq!(
        EventId::from_str(&canonical.replacen("evt1_", "ctx1_", 1)).unwrap_err(),
        ContractError::InvalidEncoding
    );
    assert_eq!(
        EventId::from_str("evt1_short").unwrap_err(),
        ContractError::InvalidEncoding
    );

    let mut bad_alphabet = canonical.clone().into_bytes();
    bad_alphabet[10] = b'!';
    assert_eq!(
        EventId::from_str(std::str::from_utf8(&bad_alphabet).unwrap()).unwrap_err(),
        ContractError::InvalidEncoding
    );

    // A 32-byte base64url value has four significant bits in its last
    // character. Alter only the two unused low bits to create an alias for the
    // same bytes; strict decoding or decode/re-encode equality must reject it.
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut alias = canonical.into_bytes();
    let last = alias.len() - 1;
    let index = ALPHABET
        .iter()
        .position(|candidate| *candidate == alias[last])
        .unwrap();
    assert_eq!(index % 4, 0);
    alias[last] = ALPHABET[index + 1];
    assert_eq!(
        EventId::from_str(std::str::from_utf8(&alias).unwrap()).unwrap_err(),
        ContractError::InvalidEncoding
    );

    macro_rules! assert_other_encoding {
        ($ty:ty, $value:expr, $prefix:literal, $wrong_prefix:literal, $alias_step:expr) => {{
            let canonical = $value.to_string();
            assert_eq!(<$ty>::from_str(&canonical).unwrap(), $value);
            assert_eq!(
                <$ty>::from_str(&(canonical.clone() + "=")).unwrap_err(),
                ContractError::InvalidEncoding
            );
            assert_eq!(
                <$ty>::from_str(&canonical.replacen($prefix, $wrong_prefix, 1)).unwrap_err(),
                ContractError::InvalidEncoding
            );
            let mut alias = canonical.into_bytes();
            let last = alias.len() - 1;
            let index = ALPHABET
                .iter()
                .position(|candidate| *candidate == alias[last])
                .unwrap();
            assert_eq!(index % $alias_step, 0);
            alias[last] = ALPHABET[index + 1];
            assert_eq!(
                <$ty>::from_str(std::str::from_utf8(&alias).unwrap()).unwrap_err(),
                ContractError::InvalidEncoding
            );
        }};
    }
    assert_other_encoding!(
        ContextId,
        ContextId::from_bytes([12; 32]),
        "ctx1_",
        "evt1_",
        4
    );
    assert_other_encoding!(
        AuthorId,
        AuthorId::from_bytes([13; 32]),
        "ed25519_",
        "author__",
        4
    );
    assert_other_encoding!(
        EventSignature,
        EventSignature::from_bytes([14; 64]),
        "sig1_",
        "bad1_",
        16
    );
}

#[test]
fn parent_kind_and_depth_boundaries_are_enforced() -> Result<(), Box<dyn Error>> {
    let signer = identity();
    let context = ContextId::from_bytes([6; 32]);
    signer.create_event(
        context,
        vec![EventId::from_bytes([0; 32])],
        "agent.request",
        json!({}),
    )?;

    let mut parents: Vec<EventId> = (0_u8..64)
        .map(|value| EventId::from_bytes([value; 32]))
        .collect();
    parents.sort();
    signer.create_event(
        context,
        parents.clone(),
        format!("a{}", "0".repeat(63)),
        nested_payload(MAX_PAYLOAD_DEPTH - 1),
    )?;

    let mut duplicate = parents.clone();
    duplicate[1] = duplicate[0];
    assert_eq!(
        signer
            .create_event(context, duplicate, "agent.request", json!({}))
            .unwrap_err(),
        ContractError::ParentOrder
    );
    let mut unsorted = parents.clone();
    unsorted.swap(0, 1);
    assert_eq!(
        signer
            .create_event(context, unsorted, "agent.request", json!({}))
            .unwrap_err(),
        ContractError::ParentOrder
    );
    parents.push(EventId::from_bytes([200; 32]));
    assert_eq!(
        signer
            .create_event(context, parents, "agent.request", json!({}))
            .unwrap_err(),
        ContractError::LimitExceeded
    );

    for kind in [
        "",
        "Agent.request",
        "agent..request",
        "agent_",
        "a+b",
        &"a".repeat(65),
    ] {
        assert_eq!(
            signer
                .create_event(context, Vec::new(), kind, json!({}))
                .unwrap_err(),
            ContractError::InvalidKind
        );
    }
    assert_eq!(
        signer
            .create_event(
                context,
                Vec::new(),
                "agent.request",
                nested_payload(MAX_PAYLOAD_DEPTH)
            )
            .unwrap_err(),
        ContractError::LimitExceeded
    );
    Ok(())
}

fn nested_payload(wrappers: usize) -> Value {
    let mut value = json!(0);
    for _ in 0..wrappers {
        value = Value::Array(vec![value]);
    }
    value
}

#[test]
fn payload_body_and_wire_size_boundaries_are_enforced() -> Result<(), Box<dyn Error>> {
    let exact_payload = Value::String("x".repeat(MAX_CANONICAL_PAYLOAD_BYTES - 2));
    assert_eq!(
        canonical_payload_bytes(&exact_payload)?.len(),
        MAX_CANONICAL_PAYLOAD_BYTES
    );
    let mut maximum_parents: Vec<EventId> = (0_u8..64)
        .map(|value| EventId::from_bytes([value; 32]))
        .collect();
    maximum_parents.sort();
    let event = identity().create_event(
        ContextId::from_bytes([2; 32]),
        maximum_parents,
        format!("a{}", "0".repeat(63)),
        exact_payload,
    )?;
    let maximum_reachable_body = event.body().canonical_bytes()?;
    assert_eq!(
        maximum_reachable_body.len(),
        MAX_CANONICAL_PAYLOAD_BYTES + 3_498
    );
    assert!(maximum_reachable_body.len() <= MAX_CANONICAL_BODY_BYTES);

    let oversized_payload = Value::String("x".repeat(MAX_CANONICAL_PAYLOAD_BYTES - 1));
    assert_eq!(
        canonical_payload_bytes(&oversized_payload).unwrap_err(),
        ContractError::LimitExceeded
    );

    let mut exact_wire = base_event().to_wire()?;
    exact_wire.resize(MAX_RAW_WIRE_BYTES, b' ');
    SignedEventV1::from_wire(&exact_wire)?.verify()?;
    exact_wire.push(b' ');
    assert_eq!(
        SignedEventV1::from_wire(&exact_wire).unwrap_err(),
        ContractError::WireTooLarge
    );
    Ok(())
}

#[test]
fn number_boundaries_negative_zero_and_exponent_aliases_are_enforced() -> Result<(), Box<dyn Error>>
{
    assert!(serde_json::Number::from_f64(f64::NAN).is_none());
    assert!(serde_json::Number::from_f64(f64::INFINITY).is_none());
    assert!(serde_json::Number::from_f64(f64::NEG_INFINITY).is_none());

    let context = ContextId::from_bytes([3; 32]);
    let author = identity().author();
    let make = |payload: &str| {
        format!(
            r#"{{"version":1,"context":"{context}","parents":[],"kind":"agent.request","author":"{author}","payload":{payload}}}"#
        )
    };
    EventBodyV1::from_json(make(r#"[-9007199254740991,9007199254740991]"#).as_bytes())?;
    assert_eq!(
        EventBodyV1::from_json(make("9007199254740992").as_bytes()).unwrap_err(),
        ContractError::UnsafeNumber
    );
    assert_eq!(
        EventBodyV1::from_json(make("-9007199254740992").as_bytes()).unwrap_err(),
        ContractError::UnsafeNumber
    );
    assert_eq!(
        EventBodyV1::new(
            context,
            Vec::new(),
            "agent.request",
            author,
            json!(9_007_199_254_740_992_u64)
        )
        .unwrap_err(),
        ContractError::UnsafeNumber
    );

    let zero = EventBodyV1::from_json(make("-0").as_bytes())?;
    let zero_alias = EventBodyV1::from_json(make("0.0e+10").as_bytes())?;
    assert_eq!(zero.canonical_bytes()?, zero_alias.canonical_bytes()?);
    let exponent = EventBodyV1::from_json(make("1e0").as_bytes())?;
    let integer = EventBodyV1::from_json(make("1").as_bytes())?;
    assert_eq!(exponent.canonical_bytes()?, integer.canonical_bytes()?);
    Ok(())
}

#[test]
fn malformed_inputs_never_panic_or_return_partial_events() {
    let cases: Vec<Vec<u8>> = vec![
        Vec::new(),
        b"null".to_vec(),
        b"[]".to_vec(),
        br#"{"a":1,"a":2}"#.to_vec(),
        vec![0xff, 0xfe, 0xfd],
        br#"{"event_id":false,"body":null,"signature":0}"#.to_vec(),
        b"[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[0]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]".to_vec(),
    ];
    for input in cases {
        let result = catch_unwind(AssertUnwindSafe(|| SignedEventV1::from_wire(&input)));
        assert!(result.is_ok());
        assert!(result.unwrap().is_err());
    }
}
