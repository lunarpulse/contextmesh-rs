//! Frozen OA-01 vectors and RFC 8785 interoperability checks.

use std::error::Error;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use contextmesh::crypto::{EVENT_ID_DOMAIN, SIGNATURE_DOMAIN, SigningIdentity, derive_event_id};
use contextmesh::model::{ContextId, EventBodyV1, SignedEventV1, canonical_payload_bytes};
use ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey};
use serde_json::{Value, json};

const FIXTURE_SEED: [u8; 32] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31,
];
const FIXTURE_CONTEXT: [u8; 32] = [
    128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146,
    147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159,
];

fn fixture_vector() -> Result<Value, Box<dyn Error>> {
    let identity = SigningIdentity::from_fixture_seed(FIXTURE_SEED);
    let event = identity.create_event(
        ContextId::from_bytes(FIXTURE_CONTEXT),
        Vec::new(),
        "agent.request",
        json!({}),
    )?;
    let body = event.body().canonical_bytes()?;
    let envelope = event.to_wire()?;
    let mut signing_message = SIGNATURE_DOMAIN.to_vec();
    signing_message.extend_from_slice(&event.event_id().to_bytes());

    Ok(json!({
        "schema_version": 1,
        "provenance": {
            "spec": "_bmad-output/implementation-artifacts/spec-oa-01-signed-event-contract.md",
            "baseline_commit": "53777ce3668708a5f1b668d25c2a461d04b9985e",
            "rfc_8785": "https://www.rfc-editor.org/rfc/rfc8785",
            "jcs_upstream_testdata": "https://github.com/cyberphone/json-canonicalization/tree/master/testdata",
            "generator": "tests/oa01_golden.rs::fixture_vector",
            "warning": "The fixed seed is public test material and MUST NOT be used in production."
        },
        "vectors": [{
            "name": "empty-object-zero-parents",
            "expected_outcome": "valid",
            "fixed_seed_base64url": URL_SAFE_NO_PAD.encode(FIXTURE_SEED),
            "context_bytes_base64url": URL_SAFE_NO_PAD.encode(FIXTURE_CONTEXT),
            "context": event.body().context().to_string(),
            "canonical_body_utf8": String::from_utf8(body.clone())?,
            "canonical_body_base64url": URL_SAFE_NO_PAD.encode(&body),
            "event_id_domain": EVENT_ID_DOMAIN,
            "event_id_bytes_base64url": URL_SAFE_NO_PAD.encode(event.event_id().to_bytes()),
            "event_id": event.event_id().to_string(),
            "author_public_key_base64url": URL_SAFE_NO_PAD.encode(event.body().author().to_bytes()),
            "author": event.body().author().to_string(),
            "signature_domain_with_nul_base64url": URL_SAFE_NO_PAD.encode(SIGNATURE_DOMAIN),
            "signing_message_base64url": URL_SAFE_NO_PAD.encode(&signing_message),
            "signature_bytes_base64url": URL_SAFE_NO_PAD.encode(event.signature().to_bytes()),
            "signature": event.signature().to_string(),
            "canonical_envelope_utf8": String::from_utf8(envelope.clone())?,
            "canonical_envelope_base64url": URL_SAFE_NO_PAD.encode(&envelope)
        }]
    }))
}

#[test]
fn checked_in_fixture_is_deterministically_reproducible() -> Result<(), Box<dyn Error>> {
    let checked_in: Value = serde_json::from_str(include_str!("fixtures/oa01-v1-golden.json"))?;
    assert_eq!(checked_in, fixture_vector()?);
    Ok(())
}

#[test]
fn fixed_vector_recomputes_and_verifies_independently() -> Result<(), Box<dyn Error>> {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/oa01-v1-golden.json"))?;
    let vector = &fixture["vectors"][0];
    assert_eq!(
        vector["author"],
        "ed25519_A6EHv_POEL4dcN0Y50vAmWfk1jCbpQ1fHdyGZBJVMbg"
    );
    assert_eq!(
        vector["event_id"],
        "evt1_hxBPEEVXvPN-15l_wZU2h7KsakjEdxKPO0yzY4eOXnY"
    );
    let body_bytes =
        URL_SAFE_NO_PAD.decode(vector["canonical_body_base64url"].as_str().unwrap())?;
    let body = EventBodyV1::from_json(&body_bytes)?;

    let mut hasher = blake3::Hasher::new_derive_key(EVENT_ID_DOMAIN);
    hasher.update(&body_bytes);
    let independently_hashed = *hasher.finalize().as_bytes();
    assert_eq!(independently_hashed, derive_event_id(&body)?.to_bytes());
    assert_eq!(
        URL_SAFE_NO_PAD.encode(independently_hashed),
        vector["event_id_bytes_base64url"]
    );

    let fixture_signing_key = SigningKey::from_bytes(&FIXTURE_SEED);
    assert_eq!(
        fixture_signing_key.verifying_key().to_bytes(),
        body.author().to_bytes()
    );
    let author = VerifyingKey::from_bytes(&body.author().to_bytes())?;
    let signature_bytes = event_signature_bytes(vector)?;
    let signature = Signature::from_bytes(&signature_bytes);
    let message = URL_SAFE_NO_PAD.decode(vector["signing_message_base64url"].as_str().unwrap())?;
    let mut independently_built_message = SIGNATURE_DOMAIN.to_vec();
    independently_built_message.extend_from_slice(&independently_hashed);
    assert_eq!(message, independently_built_message);
    assert_eq!(
        fixture_signing_key.sign(&message).to_bytes(),
        signature_bytes
    );
    author.verify_strict(&message, &signature)?;

    let wire = URL_SAFE_NO_PAD.decode(vector["canonical_envelope_base64url"].as_str().unwrap())?;
    let parsed = SignedEventV1::from_wire(&wire)?;
    assert_eq!(parsed.to_wire()?, wire);
    Ok(())
}

fn event_signature_bytes(vector: &Value) -> Result<[u8; 64], Box<dyn Error>> {
    Ok(URL_SAFE_NO_PAD
        .decode(vector["signature_bytes_base64url"].as_str().unwrap())?
        .try_into()
        .map_err(|_| "fixture signature length")?)
}

#[test]
fn equivalent_json_produces_identical_body_id_and_signature() -> Result<(), Box<dyn Error>> {
    let identity = SigningIdentity::from_fixture_seed(FIXTURE_SEED);
    let context = ContextId::from_bytes(FIXTURE_CONTEXT);
    let author = identity.author();
    let first = format!(
        r#"{{"version":1,"context":"{context}","parents":[],"kind":"agent.request","author":"{author}","payload":{{"text":"é","nested":{{"b":2,"a":1}}}}}}"#
    );
    let second = format!(
        r#" {{ "payload" : {{"nested":{{"a":1e0,"b":2.0}},"text":"\u00e9"}}, "author":"{author}", "kind":"agent.request", "parents":[], "context":"{context}", "version":1 }} "#
    );
    let body_one = EventBodyV1::from_json(first.as_bytes())?;
    let body_two = EventBodyV1::from_json(second.as_bytes())?;
    assert_eq!(body_one.canonical_bytes()?, body_two.canonical_bytes()?);
    let event_one = identity.sign_body(body_one)?;
    let event_two = identity.sign_body(body_two)?;
    assert_eq!(event_one.event_id(), event_two.event_id());
    assert_eq!(event_one.signature(), event_two.signature());
    assert_eq!(event_one.to_wire()?, event_two.to_wire()?);
    Ok(())
}

#[test]
fn rfc_8785_serialization_example_matches() -> Result<(), Box<dyn Error>> {
    let input: Value = serde_json::from_str(
        r#"{"numbers":[333333333.33333329,1E30,4.50,2e-3,0.000000000000000000000000001],"string":"€$\u000f\nA'B\"\\\"/","literals":[null,true,false]}"#,
    )?;
    let expected = r#"{"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"string":"€$\u000f\nA'B\"\\\"/"}"#;
    // 1e30 is an integer-valued number outside contextmesh's additional safe
    // range. Verify the RFC serializer itself; contract limits are separate.
    assert_eq!(serde_jcs::to_vec(&input)?, expected.as_bytes());
    Ok(())
}

#[test]
fn rfc_8785_utf16_property_order_matches() -> Result<(), Box<dyn Error>> {
    let input = json!({
        "€": "Euro Sign",
        "\r": "Carriage Return",
        "דּ": "Hebrew Letter Dalet With Dagesh",
        "1": "One",
        "😀": "Emoji: Grinning Face",
        "ö": "Latin Small Letter O With Diaeresis",
        "": "Control"
    });
    let expected = format!(
        r#"{{"\r":"Carriage Return","1":"One","{control}":"Control","{diaeresis}":"Latin Small Letter O With Diaeresis","{euro}":"Euro Sign","{emoji}":"Emoji: Grinning Face","{hebrew}":"Hebrew Letter Dalet With Dagesh"}}"#,
        control = '\u{80}',
        diaeresis = '\u{f6}',
        euro = '\u{20ac}',
        emoji = '\u{1f600}',
        hebrew = '\u{fb33}'
    );
    assert_eq!(canonical_payload_bytes(&input)?, expected.as_bytes());
    Ok(())
}

#[test]
fn unicode_is_not_normalized_but_escape_aliases_are() -> Result<(), Box<dyn Error>> {
    let decomposed: Value = serde_json::from_str(r#"{"value":"e\u0301"}"#)?;
    let decomposed_literal = json!({"value": "e\u{301}"});
    let composed = json!({"value": "é"});
    assert_eq!(
        canonical_payload_bytes(&decomposed)?,
        canonical_payload_bytes(&decomposed_literal)?
    );
    assert_ne!(
        canonical_payload_bytes(&decomposed)?,
        canonical_payload_bytes(&composed)?
    );
    Ok(())
}

#[test]
#[ignore = "prints the deterministic fixture for an explicitly approved vector update"]
fn print_fixture_for_approved_update() -> Result<(), Box<dyn Error>> {
    println!("{}", serde_json::to_string_pretty(&fixture_vector()?)?);
    Ok(())
}
