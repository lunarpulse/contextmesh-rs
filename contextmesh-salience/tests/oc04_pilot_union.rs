//! OC-04 pilot candidate-union generation over imported transcripts.
//!
//! LOCAL-ONLY tool (run with `cargo test -p contextmesh-salience --test
//! oc04_pilot_union -- --nocapture`, env-driven): for each pilot session,
//! imports the transcript via `oc04_import`, runs the FROZEN arms
//! (lexical cap 64 + OC-03 prior), and writes the union manifest to
//! `$OC04_UNION_OUT`. Raw session ids stay in the env/local paths only;
//! the committed manifest carries EventIds (payload-derived, deterministic).
//!
//! Disclosed boundary (plan §8): the prior arm's seeds come from a
//! pseudo-report whose m4_shares are EMPTY in this first cut — the pilot
//! union therefore equals the lexical arm, recorded honestly as
//! prior_count = 0. Real prior-arm nomination needs the replay's M4 share
//! extraction and is a separate step.
use contextmesh::crypto::SigningIdentity;
use contextmesh::receipt::TaskRecordV1;
use contextmesh::selection::{BaselineSelector, SourceEvent};
use contextmesh_salience::oc04_import::{ImportMessage, import_session};
use contextmesh_salience::oc04_selection::{Oc04ConfigV1, VerifiedPrior};
use contextmesh_salience::oc04_union::union_candidates;
use contextmesh_salience::prior::{
    PriorConfigV1, ReportContribution, SessionPayloads, assemble_prior, build_entity_graph,
    derive_seeds, run_ppr,
};

/// HMAC-SHA256(key, domain ++ raw_session_id) — MUST match the
/// sampling-manifest scheme exactly (earlier blake3 side-hash was
/// unjoinable; fixed).
fn session_hmac(key: &[u8; 32], raw_session_id: &str) -> String {
    use sha2::{Digest, Sha256};
    const IPAD: u8 = 0x36;
    const OPAD: u8 = 0x5c;
    // RFC 2104: the key is zero-padded to 64 bytes, then XORed with the pad
    // constant. (i % key.len() repeats the key — non-RFC, broke the join
    // with the sampling manifests; fixed to zero-padding.)
    let ipad: [u8; 64] = std::array::from_fn(|i| IPAD ^ key.get(i).copied().unwrap_or(0));
    let opad: [u8; 64] = std::array::from_fn(|i| OPAD ^ key.get(i).copied().unwrap_or(0));
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(b"oc04-gold/session/v1");
    inner.update(raw_session_id.as_bytes());
    let inner_out = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_out);
    hex::encode(outer.finalize())
}

#[test]
fn generate_pilot_unions() {
    let db = std::env::var("OC04_PILOT_DB").expect("OC04_PILOT_DB (session db path)");
    let ids_json = std::env::var("OC04_PILOT_IDS").expect("OC04_PILOT_IDS (json array)");
    let key_hex = std::env::var("OC04_PILOT_KEY").expect("OC04_PILOT_KEY (hex64)");
    let out_path = std::env::var("OC04_UNION_OUT").expect("OC04_UNION_OUT");
    let session_ids: Vec<String> = serde_json::from_str(&ids_json).unwrap();
    let mut key = [0u8; 32];
    key.copy_from_slice(&hex::decode(key_hex.trim()).unwrap());

    let identity = SigningIdentity::from_fixture_seed([42u8; 32]);
    let conn =
        rusqlite::Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let config4 = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let prior_config = PriorConfigV1::default();

    let mut sessions_out = Vec::new();
    for sid in &session_ids {
        let mut stmt = conn
            .prepare(
                "SELECT timestamp, id, role, content, tool_name FROM messages \
                 WHERE session_id = ?1 AND active = 1 ORDER BY timestamp, id",
            )
            .unwrap();
        let rows: Vec<(f64, i64, String, String, Option<String>)> = stmt
            .query_map([sid], |r| {
                Ok((
                    r.get::<_, f64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    // NULL content = placeholder row (e.g. tool-result shells):
                    // skipped downstream by the empty-content rule below.
                    r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(4)?,
                ))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let rows: Vec<_> = rows
            .into_iter()
            .filter(|(.., content, _)| !content.is_empty())
            .collect();
        assert!(!rows.is_empty(), "{sid}: no transcript rows");
        let messages: Vec<ImportMessage> = rows
            .into_iter()
            .map(|(ts, id, role, content, tool)| ImportMessage {
                order: ((ts * 1_000_000.0) as i64, id),
                role,
                tool_name: tool,
                content,
            })
            .collect();

        // 1) import → verified signed events (frozen pipeline admission gate)
        let events = import_session(&identity, sid, &key, &messages).unwrap();

        // 2) lexical arm
        let sources: Vec<SourceEvent> = events
            .iter()
            .map(SourceEvent::from_signed)
            .collect::<Result<_, _>>()
            .unwrap();
        let task_text = messages
            .iter()
            .find(|m| m.role == "user")
            .map(|m| m.content.clone())
            .unwrap_or_default();
        let task = TaskRecordV1::from_verbatim(task_text, None).unwrap();
        let scored = selector.select_scored(&task, &sources).unwrap();
        let scored: Vec<_> = scored.into_iter().take(64).collect(); // lexical_arm_cap

        // 3) prior arm with the disclosed empty-m4 pseudo-report.
        // The prior gate needs canonical payload TEXT: re-derive each
        // event's canonical text from its payload (byte-exact round-trip).
        let payload_refs: Vec<String> = events
            .iter()
            .map(|e| {
                let payload = e.body().payload();
                let canonical = contextmesh::model::canonical_payload_bytes(payload).unwrap();
                String::from_utf8(canonical).unwrap()
            })
            .collect();
        let payload_slices: Vec<&str> = payload_refs.iter().map(String::as_str).collect();
        let session_payloads = vec![SessionPayloads::from_payloads(payload_slices)];
        let report = ReportContribution::from_report_bytes(
            br#"{"report_id":"oc04-pilot","ledger_id":"oc04-pilot","terminal_status":"terminal","adapter_tier":"{\"status\":\"computed\"}"}"#,
        )
        .unwrap();
        let reports = vec![report];
        let graph = build_entity_graph(&session_payloads, &prior_config).unwrap();
        let (seeds, dropped) = derive_seeds(&reports, &[], &prior_config).unwrap();
        let ppr = run_ppr(&graph, &seeds, &prior_config).unwrap();
        let prior = assemble_prior(graph, seeds, &ppr, dropped, "terminal", &prior_config).unwrap();
        let bytes = prior.canonical_bytes().unwrap();
        let verified =
            VerifiedPrior::verify(&bytes, &session_payloads, &reports, &[], &prior_config)
                .expect("prior rebuild-verify");

        // 4) frozen 4C union
        let union = union_candidates(&scored, &verified, &sources, &config4).unwrap();
        let lexical_ids: std::collections::HashSet<String> = scored
            .iter()
            .map(|s| s.reference().event().to_string())
            .collect();
        let prior_ids: std::collections::HashSet<String> = union
            .prior()
            .iter()
            .map(|p| p.event().to_string())
            .collect();

        let events_out: Vec<serde_json::Value> = union
            .entries()
            .iter()
            .enumerate()
            .map(|(union_rank, e)| {
                // import-order index: the Nth imported message (0-based).
                // Event ids are deterministic over (identity, key, order),
                // so this index is stable across the union and packet runs.
                let import_index = sources
                    .iter()
                    .position(|s| s.event().to_string() == e.event())
                    .expect("union entry must reference an imported event");
                serde_json::json!({
                    "event_id": e.event().to_string(),
                    "import_index": import_index,
                    "union_rank": union_rank,
                    "in_lexical": lexical_ids.contains(e.event()),
                    "in_prior": prior_ids.contains(e.event()),
                })
            })
            .collect();
        sessions_out.push(serde_json::json!({
            "session_hmac": session_hmac(&key, sid),
            "event_count": events.len(),
            "lexical_count": scored.len(),
            "prior_count": union.prior().len(),
            "union_count": union.entries().len(),
            "events": events_out,
        }));
        eprintln!(
            "{sid}: events={} lexical={} prior={} union={}",
            events.len(),
            scored.len(),
            union.prior().len(),
            union.entries().len()
        );
    }

    let doc = serde_json::json!({
        "kind": "pilot-candidate-union",
        "arms": {"lexical_cap": 64, "prior_cap": 30},
        "disclosure": "prior arm has empty m4 seeds in this first cut (plan §8); union == lexical arm for these sessions",
        "sessions": sessions_out,
    });
    serde_json::to_writer_pretty(std::fs::File::create(&out_path).unwrap(), &doc).unwrap();
    println!(
        "manifest written: {out_path} ({} sessions)",
        sessions_out.len()
    );
}
