//! OA-04 strict protocol canonical vectors, fingerprints, cursors, and parser
//! adversarial boundaries.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use contextmesh::crypto::SigningIdentity;
use contextmesh::error::SyncError;
use contextmesh::model::{ContextId, EventId};
use contextmesh::store::{
    AdvertisedRef, BundleLimits, BundleV1, MAX_BUNDLE_CANONICAL_BYTES, MAX_BUNDLE_EVENTS,
    MAX_BUNDLE_REFS, RefNamespace,
};
use contextmesh::sync::{
    ExportRequest, ExportResponse, PullLimits, RefSnapshot, decode_cursor, encode_cursor,
};
use serde_json::json;

const ERROR_401: &str = "{\"error\":{\"code\":\"authentication_failed\",\"request_id\":\"req1_AAAAAAAAAAAAAAAAAAAAAA\"},\"protocol_version\":1}";

fn fixture_values() -> (ContextId, BundleV1, Vec<EventId>) {
    let context = ContextId::from_bytes([7; 32]);
    let identity = SigningIdentity::from_fixture_seed([9; 32]);
    let genesis = identity
        .create_event(context, Vec::new(), "context.genesis", json!({}))
        .unwrap();
    let child = identity
        .create_event(
            context,
            vec![genesis.event_id()],
            "demo.note",
            json!({"note":"fixture"}),
        )
        .unwrap();
    let bundle =
        BundleV1::from_parts(context, vec![genesis.clone(), child.clone()], vec![]).unwrap();
    let mut heads = vec![genesis.event_id(), child.event_id()];
    heads.sort();
    (context, bundle, heads)
}

fn fixture_parts() -> (
    ContextId,
    Vec<AdvertisedRef>,
    Vec<EventId>,
    PullLimits,
    String,
) {
    let (context, _, heads) = fixture_values();
    let refs = vec![AdvertisedRef {
        namespace: RefNamespace::Local,
        name: "main".parse().unwrap(),
        head: heads[1],
    }];
    let limits = PullLimits::new(2, 1_048_576, 100).unwrap();
    let cursor = encode_cursor(2, [3; 32]).unwrap();
    (context, refs, heads, limits, cursor)
}

fn canonical_wires() -> Vec<(&'static str, Vec<u8>)> {
    let (context, bundle, heads) = fixture_values();
    let (context2, refs, heads2, limits, cursor) = fixture_parts();
    assert_eq!(context, context2);
    let snapshot = RefSnapshot::new(context, refs.clone()).unwrap();
    let request = ExportRequest::new(
        context,
        heads2.clone(),
        vec![heads[0]],
        Some(cursor.clone()),
        limits,
    )
    .unwrap();
    let response = ExportResponse::new(context, &heads2, bundle, Some(cursor)).unwrap();
    vec![
        ("refs", snapshot.to_wire().unwrap()),
        ("request", request.to_wire().unwrap()),
        ("response", response.to_wire().unwrap()),
        ("error401", ERROR_401.as_bytes().to_vec()),
    ]
}

#[test]
fn protocol_fixture_is_frozen_canonical_and_reproducible() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/oa04-protocol-golden.json"
    );
    let fixture: std::collections::BTreeMap<String, String> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for (name, wire) in canonical_wires() {
        assert_eq!(
            fixture[name],
            String::from_utf8(wire).unwrap(),
            "frozen {name} wire changed"
        );
    }
}

#[test]
fn parser_rejects_equivalent_but_noncanonical_and_hostile_input() {
    let (_, _, heads) = fixture_values();
    let (context, refs, heads2, limits, cursor) = fixture_parts();
    let snapshot = RefSnapshot::new(context, refs.clone()).unwrap();
    let good = snapshot.to_wire().unwrap();

    let mut bom = good.clone();
    bom.splice(0..0, [0xef, 0xbb, 0xbf]);
    let mut trailing = good.clone();
    trailing.extend_from_slice(b" {}");
    let spaced = String::from_utf8(good.clone()).unwrap().replace(':', ": ");
    for hostile in [&bom, &trailing, spaced.as_bytes()] {
        assert!(RefSnapshot::from_wire(hostile).is_err());
    }

    let text = String::from_utf8(good.clone()).unwrap();
    let wrong_version = text.replacen(r#""protocol_version":1"#, r#""protocol_version":2"#, 1);
    assert!(matches!(
        RefSnapshot::from_wire(wrong_version.as_bytes()),
        Err(SyncError::UnsupportedVersion)
    ));
    let unknown = text.replacen(r#""context""#, r#""extra":1,"context""#, 1);
    assert!(RefSnapshot::from_wire(unknown.as_bytes()).is_err());
    let missing = text.replacen(r#""refs":"#, r#""hidden":"#, 1);
    assert!(RefSnapshot::from_wire(missing.as_bytes()).is_err());
    let duplicate = text.replacen(r#"{"context""#, r#"{"context":1,"context""#, 1);
    assert!(RefSnapshot::from_wire(duplicate.as_bytes()).is_err());
    let unordered = RefSnapshot::new(
        context,
        vec![
            AdvertisedRef {
                namespace: RefNamespace::Local,
                name: "zeta".parse().unwrap(),
                head: heads[0],
            },
            refs[0].clone(),
        ],
    );
    assert!(unordered.is_err());

    let mut descending = heads2.clone();
    descending.sort();
    descending.reverse();
    assert!(ExportRequest::new(context, descending, vec![], None, limits).is_err());
    assert!(PullLimits::new(0, 100, 10).is_err());
    assert!(PullLimits::new(10, MAX_BUNDLE_CANONICAL_BYTES + 1, 10).is_err());
    assert!(PullLimits::new(10, 100, 0).is_err());

    assert_eq!(decode_cursor(&cursor).unwrap().0, 2);
    assert!(decode_cursor("cursor1_AAAA").is_err());
    assert!(decode_cursor(&format!("x{cursor}")).is_err());

    let (_, bundle, _) = fixture_values();
    let full = ExportResponse::new(context, &heads2, bundle.clone(), None)
        .unwrap()
        .requested_head_fingerprint;
    let one = ExportResponse::new(context, &heads2[..1], bundle.clone(), None)
        .unwrap()
        .requested_head_fingerprint;
    assert_ne!(full, one);
    assert!(full.starts_with("heads1_") && full.len() == "heads1_".len() + 43);

    let request =
        ExportRequest::new(context, heads2.clone(), vec![heads[0]], None, limits).unwrap();
    let wire = request.to_wire().unwrap();
    let round = ExportRequest::from_wire(&wire).unwrap();
    assert_eq!(round.context, request.context);
    assert_eq!(round.requested_heads, request.requested_heads);
    assert_eq!(round.known_heads, request.known_heads);
    assert_eq!(round.cursor, request.cursor);
    assert_eq!(round.limits.max_events, request.limits.max_events);
    assert_eq!(
        round.limits.max_bundle_bytes,
        request.limits.max_bundle_bytes
    );
    assert_eq!(round.to_wire().unwrap(), wire);
    assert!(ExportRequest::from_wire(&bom).is_err());
    let zero_limit = String::from_utf8(wire.clone())
        .unwrap()
        .replace(r#""max_events":2"#, r#""max_events":0"#);
    assert!(ExportRequest::from_wire(zero_limit.as_bytes()).is_err());

    let response = ExportResponse::new(context, &heads2, bundle, None).unwrap();
    let wire = response.to_wire().unwrap();
    assert!(response.complete);
    let reparsed = ExportResponse::from_wire(&wire).unwrap();
    assert!(reparsed.complete);
    assert_eq!(
        reparsed.bundle.to_wire().unwrap(),
        response.bundle.to_wire().unwrap()
    );
    assert!(
        BundleLimits::new(
            MAX_BUNDLE_EVENTS,
            MAX_BUNDLE_CANONICAL_BYTES,
            MAX_BUNDLE_REFS
        )
        .is_ok()
    );
}

#[test]
fn protocol_cardinality_boundaries_are_exact() {
    let (context, _, heads) = fixture_values();
    let limits = PullLimits::new(2, 1_048_576, 100).unwrap();
    let make_refs = |count: usize| {
        (0..count)
            .map(|index| AdvertisedRef {
                namespace: RefNamespace::Local,
                name: format!("r{index:04}").parse().unwrap(),
                head: heads[0],
            })
            .collect::<Vec<_>>()
    };
    assert!(RefSnapshot::new(context, make_refs(MAX_BUNDLE_REFS)).is_ok());
    assert!(RefSnapshot::new(context, make_refs(MAX_BUNDLE_REFS + 1)).is_err());

    let make_heads = |count: usize| {
        let mut values: Vec<EventId> = (0..count)
            .map(|index| {
                let mut bytes = [9_u8; 32];
                bytes[..8].copy_from_slice(&(index as u64).to_be_bytes());
                EventId::from_bytes(bytes)
            })
            .collect();
        values.sort();
        values
    };
    let exact = make_heads(MAX_BUNDLE_REFS);
    let over = make_heads(MAX_BUNDLE_REFS + 1);
    assert!(ExportRequest::new(context, exact.clone(), vec![], None, limits).is_ok());
    assert!(ExportRequest::new(context, over, vec![], None, limits).is_err());
    assert!(ExportRequest::new(context, vec![heads[0]], exact, None, limits).is_ok());
    assert!(
        ExportRequest::new(
            context,
            vec![heads[0]],
            make_heads(MAX_BUNDLE_REFS + 1),
            None,
            limits
        )
        .is_err()
    );

    let cursor = encode_cursor(usize::from(u8::MAX), [4; 32]).unwrap();
    let (offset, fingerprint) = decode_cursor(&cursor).unwrap();
    assert_eq!(offset, usize::from(u8::MAX));
    assert_eq!(fingerprint, [4; 32]);
    let mut noncanonical = URL_SAFE_NO_PAD.encode([9_u8; 40]);
    noncanonical.push('=');
    assert!(decode_cursor(&format!("cursor1_{noncanonical}")).is_err());
}

#[test]
#[ignore = "prints the deterministic fixture for an explicitly approved vector update"]
fn print_fixture_for_approved_update() {
    for (name, wire) in canonical_wires() {
        println!("{name}: {}", String::from_utf8(wire).unwrap());
    }
}
