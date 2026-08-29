//! OC-03 Stage 3C graph tests (matrix rows OC03-G01..G08).
//!
//! Entity keys reuse the frozen M0/M1/M2 extractors with one spelling per
//! `NormalizedValue` variant (spec §7.1, clarified 2026-08-29); the graph
//! canonicalizes co-occurrence edges with recorded truncation counters and
//! parent sessions contribute adjacency the same way (spec §7.2).

use contextmesh_salience::prior::{
    PriorConfigV1, SessionPayloads, build_entity_graph, derive_entity_keys,
};

/// A valid canonical evt1_-shaped id (43 base64url chars after prefix).
fn canonical_evt1(seed: u8) -> String {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut body = String::new();
    let mut x = u64::from(seed) * 2654435761 + 0x9E37_79B9_7F4A_7C15;
    for _ in 0..43 {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        body.push(alphabet[(x >> 33) as usize % alphabet.len()] as char);
    }
    format!("evt1_{body}")
}

#[test]
fn entity_key_canonical_id() {
    // G01: evt1_/rcpt1_/ocout1_ 43-char canonical keys extracted as-is.
    for prefix in ["evt1_", "rcpt1_", "ocout1_"] {
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut body = String::new();
        let mut x: u64 = 0x1234_5678_9ABC_DEF0;
        for _ in 0..43 {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            body.push(alphabet[(x >> 33) as usize % alphabet.len()] as char);
        }
        let id = format!("{prefix}{body}");
        let keys = derive_entity_keys(&id);
        assert_eq!(
            keys,
            vec![id.clone()],
            "canonical id must pass through: {prefix}"
        );
    }
}

#[test]
fn entity_key_normalized() {
    // G02: path:/pct:/num: keys — one per NormalizedValue variant.
    let keys = derive_entity_keys("/A//b/ 9.5% 42k");
    assert!(
        keys.contains(&"path:/a/b".to_string()),
        "path folding: {keys:?}"
    );
    assert!(
        keys.contains(&"pct:950bps".to_string()),
        "percent bps: {keys:?}"
    );
    assert!(
        keys.contains(&"num:42000".to_string()),
        "number scaled: {keys:?}"
    );
    // Exactly one spelling per variant — no count:/amt: forms.
    assert!(
        !keys
            .iter()
            .any(|k| k.starts_with("count:") || k.starts_with("amt:"))
    );
}

#[test]
fn entity_key_token() {
    // G03: M0 token fallback; tokens are bounded (skip over-length).
    let keys = derive_entity_keys("zeta alpha");
    assert_eq!(keys, vec!["alpha".to_string(), "zeta".to_string()]);
    // A token longer than 1,024 bytes is skipped by extract_tokens.
    let long = "x".repeat(1_025);
    let keys = derive_entity_keys(&long);
    assert!(keys.is_empty(), "over-length token skipped: {keys:?}");
}

#[test]
fn entity_key_event_cap() {
    // G04: 9 distinct keys → 8 kept, byte-sorted.
    let payload = (0..9)
        .map(|i| format!("tok{i}"))
        .collect::<Vec<_>>()
        .join(" ");
    let keys = derive_entity_keys(&payload);
    assert_eq!(keys.len(), 8, "capped at 8: {keys:?}");
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "byte-sorted");
    // Sorted order means the lexicographically-largest key was dropped.
    assert!(
        !keys.contains(&"tok8".to_string()),
        "tail dropped: {keys:?}"
    );

    // Discriminating case (payload order ≠ byte order): the spec keeps the
    // 8 byte-smallest, so a1 survives even though it appears last.
    let keys = derive_entity_keys("z1 z2 z3 z4 z5 z6 z7 z8 a1");
    assert_eq!(keys.len(), 8);
    assert!(
        keys.contains(&"a1".to_string()),
        "byte-smallest survives: {keys:?}"
    );
    assert!(
        !keys.contains(&"z8".to_string()),
        "sorted tail dropped: {keys:?}"
    );
}

#[test]
fn graph_canonical_edges() {
    // G05: co-occurrence renders a canonical edge list (a<b, sorted).
    let session = SessionPayloads::from_payloads(vec!["delta bravo charlie"]);
    let graph = build_entity_graph(&[session], &PriorConfigV1::default()).unwrap();
    let edges: Vec<(String, String)> = graph
        .edges()
        .iter()
        .map(|e| (e.a().to_string(), e.b().to_string()))
        .collect();
    assert_eq!(
        edges,
        vec![
            ("bravo".to_string(), "charlie".to_string()),
            ("bravo".to_string(), "delta".to_string()),
            ("charlie".to_string(), "delta".to_string()),
        ]
    );
    assert_eq!(graph.entities(), &["bravo", "charlie", "delta"]);
    assert_eq!(graph.truncated_entities(), 0);
    assert_eq!(graph.truncated_edges(), 0);
}

#[test]
fn graph_entity_cap() {
    // G06: 1,025 entities → 1,024 kept, truncated_entities = 1.
    let mut payloads = Vec::new();
    for i in 0..1025u32 {
        // Each payload yields one unique key ("e0000".."e1024"), sorted so
        // the dropped tail is the lexicographically largest.
        payloads.push(format!("e{i:04}"));
    }
    let refs: Vec<&str> = payloads.iter().map(String::as_str).collect();
    // One event per key: the per-event cap (8) applies per payload, so a
    // session of 1,025 single-key events yields 1,025 candidate entities.
    let session = SessionPayloads::from_payloads(refs);
    let graph = build_entity_graph(&[session], &PriorConfigV1::default()).unwrap();
    assert_eq!(graph.entities().len(), 1_024);
    assert_eq!(graph.truncated_entities(), 1);
    assert!(!graph.entities().contains(&"e1024".to_string()));
    // Edges were dropped too: the 1,025 single-key session unions into one
    // large co-occurrence set whose pairs vastly exceed the per-entity cap.
    assert!(graph.truncated_edges() > 0, "edge drops are counted");
}

#[test]
fn graph_edge_cap() {
    // G07: one entity with 33 co-occurrence partners → 32 edges kept.
    // The hub sorts first ("a_hub") so its edges lead the canonical order
    // and the keep-first rule binds to the hub (a late-sorting hub would
    // deterministically lose edges to earlier entities instead).
    let hub = "a_hub";
    let mut payloads = Vec::new();
    for i in 0..33u32 {
        payloads.push(format!("{hub} p{i:03}"));
    }
    let refs: Vec<&str> = payloads.iter().map(String::as_str).collect();
    let session = SessionPayloads::from_payloads(refs);
    let graph = build_entity_graph(&[session], &PriorConfigV1::default()).unwrap();
    let hub_degree = graph
        .edges()
        .iter()
        .filter(|e| e.a() == hub || e.b() == hub)
        .count();
    assert_eq!(hub_degree, 32, "per-entity edge cap: {hub_degree}");
    assert!(graph.truncated_edges() >= 1, "dropped edges counted");
}

#[test]
fn graph_parent_sessions() {
    // G08: parent+child session entity sets union into cross edges.
    let parent = SessionPayloads::from_payloads(vec!["parent_a parent_b"]);
    let child = SessionPayloads::from_payloads(vec!["child_x child_y"]);
    let graph = build_entity_graph(&[parent, child], &PriorConfigV1::default()).unwrap();
    let has = |a: &str, b: &str| {
        graph
            .edges()
            .iter()
            .any(|e| (e.a() == a && e.b() == b) || (e.a() == b && e.b() == a))
    };
    assert!(has("parent_a", "parent_b"), "intra-parent edge");
    assert!(has("child_x", "child_y"), "intra-child edge");
    // No cross edges between disjoint sessions.
    assert!(
        !has("parent_a", "child_x"),
        "disjoint sessions stay disjoint"
    );
    // The union graph contains both sessions' entities.
    assert_eq!(graph.entities().len(), 4);

    // A shared entity bridges the two sessions: the parent-linked entity
    // co-occurs with both sides (propagation over parent edges).
    let parent2 = SessionPayloads::from_payloads(vec!["parent_a parent_b"]);
    let child2 = SessionPayloads::from_payloads(vec!["child_x parent_a"]);
    let graph2 = build_entity_graph(&[parent2, child2], &PriorConfigV1::default()).unwrap();
    let has2 = |a: &str, b: &str| {
        graph2
            .edges()
            .iter()
            .any(|e| (e.a() == a && e.b() == b) || (e.a() == b && e.b() == a))
    };
    assert!(
        has2("child_x", "parent_a"),
        "bridge entity creates cross edge"
    );
}

/// Canonical-id helper stays exercised (used by adversarial stages later).
#[test]
fn canonical_evt1_helper_is_canonical() {
    let id = canonical_evt1(7);
    assert_eq!(id.len(), 48);
    assert!(id.starts_with("evt1_"));
}

// ── Stage 3D: seed derivation (matrix rows G09–G12) ──────────────────────

use contextmesh_salience::prior::{ReportContribution, derive_seeds};

/// Build a minimal report envelope with the given section status and m4
/// shares (adapter tier rendered as an embedded JSON string).
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

#[test]
fn seeds_complete_sections_only() {
    // G09: only `computed` sections yield seeds; unavailable/no_nominations
    // sections and 0-ppm shares yield zero.
    let computed = ReportContribution::from_report_bytes(
        report_json("r1", "computed", &[("evt-a", 500_000)]).as_bytes(),
    )
    .unwrap();
    let unavailable =
        ReportContribution::from_report_bytes(report_json("r2", "unavailable", &[]).as_bytes())
            .unwrap();
    let none =
        ReportContribution::from_report_bytes(report_json("r3", "no_nominations", &[]).as_bytes())
            .unwrap();
    let zero_share = ReportContribution::from_report_bytes(
        report_json("r4", "computed", &[("evt-a", 0)]).as_bytes(),
    )
    .unwrap();
    let payloads = vec![("evt-a", "alpha beta")];
    let (set, dropped) = derive_seeds(
        &[computed, unavailable, none, zero_share],
        &payloads,
        &PriorConfigV1::default(),
    )
    .unwrap();
    assert_eq!(dropped, 0);
    assert_eq!(set.unavailable_reports(), 1, "unavailable counted");
    // Only r1's 500,000 ppm → 500,000,000 ppb on evt-a's two entity keys.
    let seeds: Vec<(String, u128)> = set
        .seeds()
        .iter()
        .map(|s| (s.entity().to_owned(), s.ppb()))
        .collect();
    assert_eq!(seeds.len(), 2, "{seeds:?}");
    for (_, ppb) in &seeds {
        assert_eq!(*ppb, 500_000_000);
    }
    assert_eq!(set.source_report_ids(), &["r1", "r2", "r3", "r4"]);
}

#[test]
fn seed_ppb_conversion() {
    // G10: share_ppm ×1,000 → ppb, clamp at 1e9, checked math.
    // 1,000,000 ppm (max share) ×1000 = 1e9 ppb exactly at the clamp.
    let report = ReportContribution::from_report_bytes(
        report_json("r1", "computed", &[("evt-a", 1_000_000)]).as_bytes(),
    )
    .unwrap();
    let (set, _) =
        derive_seeds(&[report], &[("evt-a", "alpha")], &PriorConfigV1::default()).unwrap();
    assert_eq!(set.seeds().len(), 1);
    assert_eq!(set.seeds()[0].ppb(), 1_000_000_000);
    // Clamping: two max shares on the same single-key event would sum to
    // 2e9, clamped to 1e9.
    let r1 = ReportContribution::from_report_bytes(
        report_json("r1", "computed", &[("evt-a", 1_000_000)]).as_bytes(),
    )
    .unwrap();
    let r2 = ReportContribution::from_report_bytes(
        report_json("r2", "computed", &[("evt-a", 1_000_000)]).as_bytes(),
    )
    .unwrap();
    let (set, _) =
        derive_seeds(&[r1, r2], &[("evt-a", "alpha")], &PriorConfigV1::default()).unwrap();
    assert_eq!(set.seeds()[0].ppb(), 1_000_000_000, "clamped at 1e9");
}

#[test]
fn seeds_unavailable_marker() {
    // G11: unavailable report → zero seeds, unavailable_reports +1.
    let report =
        ReportContribution::from_report_bytes(report_json("r9", "unavailable", &[]).as_bytes())
            .unwrap();
    let (set, _) = derive_seeds(
        std::slice::from_ref(&report),
        &[("e", "p")],
        &PriorConfigV1::default(),
    )
    .unwrap();
    assert!(set.seeds().is_empty());
    assert_eq!(set.unavailable_reports(), 1);
    // Two unavailable reports → 2 (explicit warning count, no error).
    let (set, _) = derive_seeds(
        &[report.clone(), report.clone()],
        &[("e", "p")],
        &PriorConfigV1::default(),
    )
    .unwrap();
    assert_eq!(set.unavailable_reports(), 1, "duplicate id folds once");
}
#[test]
fn seed_cap_ordering() {
    // G12: 65 seeds → 64 kept by descending ppb then entity asc; drop lands
    // in the returned dropped count (envelope `dropped_seeds`).
    let mut contributions = Vec::new();
    let mut payloads = Vec::new();
    for i in 0..65u32 {
        let event = format!("ev{i:03}");
        let ppm = 100_000 + u128::from(64 - i); // ev000 largest … ev064 smallest
        contributions.push(
            ReportContribution::from_report_bytes(
                report_json(&format!("r{i:03}"), "computed", &[(event.as_str(), ppm)]).as_bytes(),
            )
            .unwrap(),
        );
        payloads.push((
            Box::leak(event.clone().into_boxed_str()),
            Box::leak(format!("k{i:03}").into_boxed_str()),
        ));
    }
    let payloads_ref: Vec<(&str, &str)> = payloads.iter().map(|(e, p)| (&**e, &**p)).collect();
    let (set, dropped) =
        derive_seeds(&contributions, &payloads_ref, &PriorConfigV1::default()).unwrap();
    assert_eq!(dropped, 1, "65th seed dropped and counted");
    assert_eq!(set.seeds().len(), 64);
    assert_eq!(set.seeds()[0].entity(), "k000", "highest ppb kept");
    assert!(
        !set.seeds().iter().any(|s| s.entity() == "k064"),
        "lowest ppb dropped"
    );
    // Rendered order is entity byte order after cap selection.
    let entities: Vec<&str> = set.seeds().iter().map(|s| s.entity()).collect();
    let mut sorted = entities.clone();
    sorted.sort();
    assert_eq!(entities, sorted, "byte order after cap");
}
