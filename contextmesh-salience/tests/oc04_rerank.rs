//! OC-04 Stage 4D: normalization + rerank tests (matrix rows R01–R08 and
//! R03b).
//!
//! Gate: tests/oc04_rerank.rs, `rerank` (§7.2) over the 4C union outcome.
//! Tests use the OC-03 artifact pipeline to build a real verified prior
//! and the real signed-event path for sources (no test-only constructors).
//!
//! Membership-truth separation (founder-approved 4D change control):
//! `entry_reason` records union membership; min-max normalization
//! legitimately collapses an arm's minimum member to 0 ppm. The collapse
//! cases asserted here as first-class behavior: both-member arm-minimum
//! collapses (`minimum_collapse_membership_truth` — lexical minimum kept
//! with reason `both`; `distinct_tf_nonmember_zero_and_collapse` — prior
//! minimum kept with reason `both`). The SINGLE-ARM zero collapses
//! (`lexical` with 0 lexical_ppm, `prior` with 0 prior_ppm) require an
//! arm-minimum event that belongs to only ONE arm — the rerank-level
//! schema acceptance of those shapes is covered by the constructor tests
//! in `tests/oc04_schema.rs` (S01 renders + one-way rule negative tests);
//! a pipeline-level single-arm-collapse vector is reserved for 4E's
//! §7.5 chain where a signed execution over such an entry is verified.

use contextmesh::crypto::SigningIdentity;
use contextmesh::model::ContextId;
use contextmesh::oc04_scored::ScoredSelection;
use contextmesh::receipt::TaskRecordV1;
use contextmesh::selection::{BaselineSelector, SourceEvent};
use contextmesh_salience::oc04_rerank::rerank;
use contextmesh_salience::oc04_selection::{
    ENTRY_REASON_BOTH, ENTRY_REASON_PRIOR, Oc04ConfigV1, VerifiedPrior,
};
use contextmesh_salience::oc04_union::union_candidates;
use contextmesh_salience::prior::{
    PriorConfigV1, ReportContribution, SessionPayloads, assemble_prior, build_entity_graph,
    derive_seeds, run_ppr,
};
use serde_json::json;

/// Minimal computed report envelope (mirrors the OC-03 Stage 3F helper and
/// the 4C test fixture).
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

/// Standard fixture inputs: one session, one computed report.
fn fixture_inputs() -> (
    Vec<SessionPayloads<'static>>,
    Vec<ReportContribution>,
    Vec<(&'static str, &'static str)>,
) {
    let sessions = vec![SessionPayloads::from_payloads(vec![
        r#"{"text":"alpha"}"#,
        r#"{"text":"beta charlie"}"#,
    ])];
    let report = ReportContribution::from_report_bytes(
        report_json("r1", "computed", &[("evt-a", 600_000), ("evt-c", 200_000)]).as_bytes(),
    )
    .expect("report parses");
    let events: Vec<(&str, &str)> = vec![
        ("evt-a", r#"{"text":"alpha"}"#),
        ("evt-c", r#"{"text":"beta charlie"}"#),
    ];
    (sessions, vec![report], events)
}

/// Full deterministic OC-03 pipeline → verified prior token.
fn verified_fixture() -> VerifiedPrior {
    let config = PriorConfigV1::default();
    let (sessions, reports, events) = fixture_inputs();
    let graph = build_entity_graph(&sessions, &config).expect("graph");
    let (seeds, dropped) = derive_seeds(&reports, &events, &config).expect("seeds");
    assert_eq!(dropped, 0);
    let ppr = run_ppr(&graph, &seeds, &config).expect("ppr");
    let prior = assemble_prior(graph, seeds, &ppr, dropped, "terminal", &config).expect("assemble");
    let bytes = prior.canonical_bytes().expect("canonical");
    VerifiedPrior::verify(&bytes, &sessions, &reports, &events, &config).expect("verified")
}

/// A deterministic verified source event (real signed-event path).
fn source(label: &str, text: &str) -> SourceEvent {
    let event = SigningIdentity::from_fixture_seed([17; 32])
        .create_event(
            ContextId::from_bytes([21; 32]),
            Vec::new(),
            format!("note-{label}"),
            json!({"text": text}),
        )
        .expect("signed fixture event");
    SourceEvent::from_signed(&event).expect("verified source")
}

fn task(verbatim: &str) -> TaskRecordV1 {
    TaskRecordV1::from_verbatim(verbatim.to_owned(), None).expect("task")
}

/// Runs the full 4C→4D pipeline on the standard two-source fixture with the
/// given task text and returns the reranked influence record.
fn rerank_fixture(
    task_text: &str,
) -> (
    contextmesh_salience::oc04_selection::SelectionInfluenceV1,
    contextmesh_salience::oc04_union::UnionOutcomeV1,
) {
    let prior = verified_fixture();
    let config = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let task = task(task_text);
    let sources = vec![source("evt-a", "alpha"), source("evt-c", "beta charlie")];
    let lexical: Vec<ScoredSelection> = selector.select_scored(&task, &sources).expect("scored");
    let union = union_candidates(&lexical, &prior, &sources, &config).expect("union");
    let influence = rerank(&union, &prior, "task-fp", &config).expect("rerank");
    (influence, union)
}

#[test]
fn normalization_exact() {
    // R01: per-arm min-max to [0, 1e6] ppm exact, hand-computed u128
    // values, clip bounds — on the NON-degenerate path (distinct raw TFs,
    // task "alpha alpha beta" gives TFs 2 and 1) so the
    // `(raw − min) × 1e6 / (max − min)` division branch is exercised.
    // Expected: max member → 1e6; min member → 0 (collapse).
    let (influence, union) = rerank_fixture("alpha alpha beta");
    let config = Oc04ConfigV1::default();
    let mut raws: Vec<(String, u128)> = union
        .entries()
        .iter()
        .filter_map(|c| c.lexical_raw().map(|raw| (c.event().to_owned(), raw)))
        .collect();
    raws.sort_by(|a, b| a.0.cmp(&b.0));
    let min = raws.iter().map(|(_, raw)| raw).min().expect("non-empty");
    let max = raws.iter().map(|(_, raw)| raw).max().expect("non-empty");
    assert_ne!(
        min, max,
        "fixture must give distinct TFs (non-degenerate arm)"
    );
    // span = max − min = 1 on this fixture (2 − 1), so the division branch
    // truncates nothing here; the exact expected values are hand-computed.
    let span = max - min;
    assert_eq!(span, 1, "fixture span must be 1 (TFs 2 vs 1)");
    for entry in influence.entries() {
        let raw = raws
            .iter()
            .find(|(event, _)| *event == entry.event_id_text())
            .map(|(_, raw)| *raw)
            .expect("entry is a lexical member here");
        let expected = if raw == *max {
            // (max − min) × 1e6 / span = 1e6 exactly.
            config.clip_above_ppm
        } else {
            // Hand-computed: (raw − min) × 1e6 / 1 = 0 for raw = min.
            assert_eq!(raw, *min);
            0
        };
        assert_eq!(
            entry.lexical_ppm(),
            expected,
            "min-max exact for {}",
            entry.event_id_text()
        );
        assert!(entry.lexical_ppm() <= config.clip_above_ppm);
        assert!(entry.prior_ppm() <= config.clip_above_ppm);
    }
}

#[test]
fn tie_break_canonical_text() {
    // R02: rank = score desc, then canonical EventId TEXT ascending.
    // The EventId canonical text is `evt1_` + base64url (model.rs
    // fixed_text_type Display); base64url sorts differently from the raw
    // bytes (bytes ≥ 0x80 map below '-'/'_' etc.), so a pair whose
    // text-order differs from raw-byte order is REQUIRED by the matrix.
    //
    // Deterministic construction: fixture labels are search-ordered by the
    // known seeds of this repo's fixture identity ([17; 32]) — scan labels
    // until a pair of real signed events is found whose TEXT order is the
    // REVERSE of their RAW-BYTE order (EventId::to_bytes comparison).
    let mut ids: Vec<(String, [u8; 32])> = (0..24u8)
        .map(|i| {
            let event = SigningIdentity::from_fixture_seed([17; 32])
                .create_event(
                    ContextId::from_bytes([21; 32]),
                    Vec::new(),
                    format!("note-r02-{i}"),
                    json!({"text": "alpha"}),
                )
                .expect("signed fixture event");
            let source = SourceEvent::from_signed(&event).expect("verified");
            (source.event().to_string(), source.event().to_bytes())
        })
        .collect();
    ids.sort_by(|a, b| a.0.cmp(&b.0));
    // The texts are now ascending; find an adjacent pair whose raw bytes
    // compare in the OPPOSITE direction. base64url-vs-byte divergence is
    // guaranteed for byte values ≥ 0x80 ('_'=0x5F sorts below many high
    // bytes), so such a pair exists within a small scan.
    let pair = ids
        .windows(2)
        .find(|w| w[0].1 > w[1].1)
        .expect("fixture scan must find a text/byte divergent adjacent pair");
    let (text_lo, bytes_lo) = (&pair[0].0, &pair[0].1);
    let (text_hi, bytes_hi) = (&pair[1].0, &pair[1].1);
    assert!(
        text_lo < text_hi && bytes_lo > bytes_hi,
        "divergence precondition: text asc but raw bytes desc"
    );

    // Build a rerank with an exact score tie between exactly these two
    // events. Both match entity "alpha" (task "alpha", payloads "alpha");
    // equal TF + equal prior ppb ⇒ equal score ⇒ tie broken by text.
    let prior = verified_fixture();
    let config = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let task = task("alpha");
    // NOTE: signed events derive their IDs from the note kind + payload;
    // the r02-scan events share the fixture seed so their payload entity
    // extraction still yields "alpha". Use them directly as sources.
    let sources: Vec<SourceEvent> = (0..24u8)
        .map(|i| {
            let event = SigningIdentity::from_fixture_seed([17; 32])
                .create_event(
                    ContextId::from_bytes([21; 32]),
                    Vec::new(),
                    format!("note-r02-{i}"),
                    json!({"text": "alpha"}),
                )
                .expect("signed fixture event");
            SourceEvent::from_signed(&event).expect("verified")
        })
        .collect();
    // Keep only the two divergence-pair events: rebuild the exact events
    // whose ids matched (scan again by text).
    let selected: Vec<SourceEvent> = sources
        .into_iter()
        .filter(|s| {
            let text = s.event().to_string();
            text == *text_lo || text == *text_hi
        })
        .collect();
    assert_eq!(selected.len(), 2, "both divergence-pair events present");
    let lexical = selector.select_scored(&task, &selected).expect("scored");
    assert_eq!(lexical.len(), 2, "both tie members match the task");
    let union = union_candidates(&lexical, &prior, &selected, &config).expect("union");
    let influence = rerank(&union, &prior, "task-fp", &config).expect("rerank");
    let entries = influence.entries();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].score_ppm(), entries[1].score_ppm(), "exact tie");
    // THE R02 assertion: tie resolves to canonical TEXT ascending — a
    // raw-byte-ordered implementation would emit the reverse here.
    assert_eq!(entries[0].event_id_text(), *text_lo);
    assert_eq!(entries[1].event_id_text(), *text_hi);
}

#[test]
fn influence_order_matches() {
    // R03: influence entry order = rerank order (score desc, canonical
    // EventId text asc) — zip-compare the whole sequence.
    let (influence, _) = rerank_fixture("alpha beta");
    let entries = influence.entries();
    assert!(!entries.is_empty());
    for pair in entries.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let key_a = (std::cmp::Reverse(a.score_ppm()), a.event_id_text());
        let key_b = (std::cmp::Reverse(b.score_ppm()), b.event_id_text());
        assert!(
            key_a <= key_b,
            "entries must be in rerank order (score desc, text asc)"
        );
    }
    // The record validates against assemble's own ordering gate: re-run
    // assemble on the same entries must succeed (order already valid).
    let config = Oc04ConfigV1::default();
    let rebuilt = contextmesh_salience::oc04_selection::SelectionInfluenceV1::assemble(
        &config,
        influence.prior_id(),
        influence.task_fingerprint(),
        entries.to_vec(),
    );
    assert!(
        rebuilt.is_ok(),
        "recorded order must satisfy the rerank gate"
    );
}

#[test]
fn influence_covers_union() {
    // R03b: influence entries cover every union member (len == union size,
    // same EventId set).
    let (influence, union) = rerank_fixture("alpha beta");
    assert_eq!(influence.entries().len(), union.entries().len());
    let mut union_events: Vec<&str> = union.entries().iter().map(|c| c.event()).collect();
    union_events.sort_unstable();
    let mut influence_events: Vec<&str> = influence
        .entries()
        .iter()
        .map(|e| e.event_id_text())
        .collect();
    influence_events.sort_unstable();
    assert_eq!(union_events, influence_events);
}

#[test]
fn formula_exact() {
    // R04: score_ppm = lexical_ppm + prior_ppm exactly (u128 identity,
    // per entry).
    let (influence, _) = rerank_fixture("alpha beta");
    for entry in influence.entries() {
        assert_eq!(
            entry.score_ppm(),
            u128::from(entry.lexical_ppm()) + u128::from(entry.prior_ppm())
        );
    }
}

#[test]
fn degenerate_arm_rule() {
    // R05: degenerate arm (min = max) — every member maps to 1e6 if
    // raw > 0, else 0. Single-candidate arm case: the lexical arm has
    // exactly one distinct raw value (single matched member).
    //
    // Fixture: task matching exactly one source word → lexical arm with
    // one member; the prior arm may be larger, so the LEXICAL arm is the
    // degenerate one (min = max = its single raw TF).
    let (influence, union) = rerank_fixture("alpha");
    let lexical_entries: Vec<_> = union
        .entries()
        .iter()
        .filter(|c| c.lexical_raw().is_some())
        .collect();
    assert_eq!(lexical_entries.len(), 1, "single-member lexical arm");
    let raw = lexical_entries[0].lexical_raw().expect("member");
    assert!(raw > 0);
    let entry = influence
        .entries()
        .iter()
        .find(|e| e.event_id_text() == lexical_entries[0].event())
        .expect("entry present");
    assert_eq!(entry.lexical_ppm(), 1_000_000, "degenerate raw>0 arm → 1e6");
    // The entry's reason reflects membership, unaffected by normalization.
    // This member is ALSO prior-positive (evt-a), so its membership reason
    // is `both` — the collapse to the degenerate-arm 1e6 changes nothing.
    assert_eq!(entry.entry_reason(), ENTRY_REASON_BOTH);
    assert!(entry.prior_ppm() > 0, "prior membership retained");
}

#[test]
fn rerank_determinism() {
    // R06: two runs byte-identical.
    let (first, _) = rerank_fixture("alpha beta");
    let (second, _) = rerank_fixture("alpha beta");
    let a = first.canonical_bytes().expect("canonical");
    let b = second.canonical_bytes().expect("canonical");
    assert_eq!(a, b, "two rerank runs must be byte-identical");
}

#[test]
fn prior_arm_ordering() {
    // R07: prior-arm ranking = ppb desc, then canonical EventId asc on
    // ties — exercised with TWO prior members sharing the SAME raw ppb so
    // the tie-break is the DECIDING key (round-2 blocker: the previous
    // fixture folded to different ppbs 544161234 vs 227881413 and the tie
    // branch never ran).
    //
    // Equal-ppb construction: both sources carry payload "alpha" — the
    // SAME single entity key — so both max-fold to the identical
    // 'alpha' entity ppb. Unconditional precondition asserts guarantee the
    // tie is actually reached.
    let prior = verified_fixture();
    let config = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let task = task("zzz");
    let sources = vec![source("evt-a", "alpha"), source("evt-b", "alpha")];
    let lexical = selector.select_scored(&task, &sources).expect("scored");
    let union = union_candidates(&lexical, &prior, &sources, &config).expect("union");
    assert_eq!(union.prior().len(), 2, "both prior-positive events in arm");
    let (a, b) = (&union.prior()[0], &union.prior()[1]);
    // UNCONDITIONAL tie precondition: both fold to the same entity ppb.
    assert_eq!(
        a.raw_ppb(),
        b.raw_ppb(),
        "fixture must produce an exact ppb tie (same single entity)"
    );
    // Tie reached: canonical EventId text ascending is the deciding key.
    assert!(
        a.event() < b.event(),
        "tie must resolve to canonical EventId text ascending"
    );
}

#[test]
fn prior_arm_ordering_distinct_ppb() {
    // R07 non-tie branch: distinct ppbs rank strictly descending (the
    // original two-entity fixture — 'alpha' 544161234 vs 'beta charlie'
    // 227881413).
    let prior = verified_fixture();
    let config = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let task = task("zzz");
    let sources = vec![source("evt-a", "alpha"), source("evt-c", "beta charlie")];
    let lexical = selector.select_scored(&task, &sources).expect("scored");
    let union = union_candidates(&lexical, &prior, &sources, &config).expect("union");
    assert_eq!(union.prior().len(), 2);
    let (a, b) = (&union.prior()[0], &union.prior()[1]);
    assert_ne!(a.raw_ppb(), b.raw_ppb(), "fixture must have distinct ppbs");
    assert!(
        a.raw_ppb() > b.raw_ppb(),
        "prior arm must be strictly ppb descending on distinct values"
    );
}

#[test]
fn multi_entity_max_fold() {
    // R08: an event matching 2 entities folds to the MAX ppb, not the sum.
    // Decisive evidence: compute the two candidate interpretations and
    // require the exact max — a sum-fold implementation makes this test
    // FAIL because raw would equal ppb_x + ppb_y (> max for positive
    // ppbs), not ppb_max.
    let prior = verified_fixture();
    let config = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let task = task("zzz");
    // 'beta charlie' matches entities 'beta' and 'charlie' (two-entity
    // event); 'alpha' matches only 'alpha'.
    let sources = vec![source("evt-c", "beta charlie")];
    let lexical = selector.select_scored(&task, &sources).expect("scored");
    let union = union_candidates(&lexical, &prior, &sources, &config).expect("union");
    assert_eq!(
        union.prior().len(),
        1,
        "the two-entity event is the only prior candidate"
    );
    let raw = union.prior()[0].raw_ppb();

    // Independent derivation of the matched entities' ppbs from the
    // verified prior's positive seeds. NOTE: entity keys on this fixture
    // are payload fragments (e.g. `{"text":"beta` / `charlie"}` — the
    // derive_entity_keys tokenizer splits the JSON text), so matching is
    // by substring; the two seeds carrying 227881413 correspond to the
    // 'beta'/'charlie' fragments.
    let matched: Vec<u128> = prior
        .positive_seeds()
        .iter()
        .filter(|seed| seed.entity().contains("beta") || seed.entity().contains("charlie"))
        .map(|seed| seed.ppb())
        .collect();
    assert_eq!(matched.len(), 2, "two distinct matched entities");
    let max_matched = matched.iter().copied().max().expect("non-empty");
    let sum_matched: u128 = matched.iter().sum();
    assert_ne!(max_matched, sum_matched, "max must differ from sum here");

    // THE R08 assertion: exact max fold.
    assert_eq!(
        raw, max_matched,
        "event ppb must equal the MAX matched-entity ppb (a sum fold would give {sum_matched})"
    );
    // And the reranked record's prior ppm for this sole prior member is
    // the degenerate-arm 1e6 (single-member prior arm, raw > 0).
    let influence = rerank(&union, &prior, "task-fp", &config).expect("rerank");
    let entry = influence
        .entries()
        .iter()
        .find(|e| e.event_id_text() == union.prior()[0].event())
        .expect("entry present");
    assert_eq!(entry.prior_ppm(), 1_000_000);
    assert_eq!(entry.entry_reason(), ENTRY_REASON_PRIOR);
    assert_eq!(
        entry.lexical_ppm(),
        0,
        "TF=0 prior entry has lexical_ppm = 0"
    );
}

#[test]
fn minimum_collapse_membership_truth() {
    // R01/R05 collapse cases (membership-truth separation): the arm's
    // minimum member normalizes to 0 ppm while KEEPING its membership
    // reason. Fixture "alpha alpha beta" gives DISTINCT lexical TFs (2, 1)
    // → non-degenerate lexical arm → the minimum member MUST carry
    // lexical_ppm = 0 with its `both` membership intact. Precondition
    // asserts (no silent skips).
    let (influence, union) = rerank_fixture("alpha alpha beta");
    let mut lexical_members: Vec<_> = union
        .entries()
        .iter()
        .filter(|c| c.lexical_raw().is_some())
        .collect();
    lexical_members.sort_by_key(|c| c.lexical_raw());
    assert!(
        lexical_members.len() >= 2,
        "fixture must provide >= 2 lexical members"
    );
    let min_member = lexical_members[0];
    assert_ne!(
        lexical_members[0].lexical_raw(),
        lexical_members[1].lexical_raw(),
        "fixture must provide distinct raw TFs (non-degenerate arm)"
    );
    let entry = influence
        .entries()
        .iter()
        .find(|e| e.event_id_text() == min_member.event())
        .expect("entry present");
    assert_eq!(
        entry.lexical_ppm(),
        0,
        "min-max collapses the arm minimum to 0"
    );
    assert_eq!(
        entry.entry_reason(),
        min_member.reason(),
        "collapse must not alter the membership reason"
    );
    assert_eq!(entry.entry_reason(), ENTRY_REASON_BOTH);
    // The maximum member normalizes to the clip bound.
    let max_member = lexical_members.last().expect("non-empty");
    let max_entry = influence
        .entries()
        .iter()
        .find(|e| e.event_id_text() == max_member.event())
        .expect("entry present");
    assert_eq!(max_entry.lexical_ppm(), 1_000_000);
}

#[test]
fn qb1_distinct_tf_regression() {
    // QB1 regression (re-review Blocker 1): a valid union whose lexical
    // arm is in score-desc order that DIFFERS from canonical-text order
    // must rerank without Malformed. Fixture: TF 2 vs 1 puts the two
    // events in the opposite order from their canonical text sort.
    let prior = verified_fixture();
    let config = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let task = task("alpha alpha beta");
    let sources = vec![source("evt-a", "alpha"), source("evt-c", "beta charlie")];
    let scored = selector.select_scored(&task, &sources).expect("scored");
    assert_eq!(scored.len(), 2);
    assert_ne!(
        scored[0].lexical_raw(),
        scored[1].lexical_raw(),
        "distinct TFs"
    );
    // score order differs from canonical text order:
    assert!(
        scored[0].reference().event().to_string() > scored[1].reference().event().to_string(),
        "fixture: score order must differ from text order"
    );
    let union = union_candidates(&scored, &prior, &sources, &config).expect("union");
    let influence = rerank(&union, &prior, "task-fp", &config)
        .expect("rerank must not reject valid score-desc arm");
    assert_eq!(influence.entries().len(), union.entries().len());
}

#[test]
fn distinct_tf_nonmember_zero_and_collapse() {
    // Prior-only minimum collapse + non-member-zero rendering, over the
    // distinct-TF fixture: the prior arm's minimum member (evt-c's ppb vs
    // evt-a's) renders prior_ppm = 0 while KEEPING reason `both` (it is
    // still a lexical member); any lexical-only member would render
    // prior_ppm = 0 per §6.
    let (influence, union) = rerank_fixture("alpha alpha beta");
    let mut prior_members: Vec<_> = union
        .entries()
        .iter()
        .filter(|c| c.prior_raw().is_some())
        .collect();
    prior_members.sort_by_key(|c| c.prior_raw());
    assert!(prior_members.len() >= 2, "two prior members expected");
    let (min, max) = (prior_members[0], prior_members.last().expect("non-empty"));
    assert_ne!(min.prior_raw(), max.prior_raw(), "distinct prior ppbs");
    let min_entry = influence
        .entries()
        .iter()
        .find(|e| e.event_id_text() == min.event())
        .expect("entry present");
    let max_entry = influence
        .entries()
        .iter()
        .find(|e| e.event_id_text() == max.event())
        .expect("entry present");
    assert_eq!(min_entry.prior_ppm(), 0, "prior-arm minimum collapses to 0");
    assert_eq!(min_entry.entry_reason(), ENTRY_REASON_BOTH);
    assert_eq!(max_entry.prior_ppm(), 1_000_000);
    assert_eq!(
        u128::from(min_entry.lexical_ppm()) + u128::from(min_entry.prior_ppm()),
        min_entry.score_ppm()
    );
    assert_eq!(
        u128::from(max_entry.lexical_ppm()) + u128::from(max_entry.prior_ppm()),
        max_entry.score_ppm()
    );
}
