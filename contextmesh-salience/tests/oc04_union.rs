//! OC-04 Stage 4C: union tests (matrix rows U01–U08).
//!
//! Gate: tests/oc04_union.rs, `union_candidates` + `UnionOutcomeV1` (§7.1)
//! plus the additive root-carrier `select_scored`/`ScoredSelection` (§8).
//! Tests use the OC-03 artifact pipeline to build a real verified prior.

use contextmesh::crypto::SigningIdentity;
use contextmesh::model::ContextId;
use contextmesh::oc04_scored::ScoredSelection;
use contextmesh::receipt::TaskRecordV1;
use contextmesh::selection::{BaselineSelector, SourceEvent};
use contextmesh_salience::oc04_selection::Oc04ConfigV1;
use contextmesh_salience::oc04_selection::VerifiedPrior;
use contextmesh_salience::oc04_union::union_candidates;
use contextmesh_salience::prior::{
    PriorConfigV1, ReportContribution, SessionPayloads, assemble_prior, build_entity_graph,
    derive_seeds, run_ppr,
};
use serde_json::json;

/// Minimal computed report envelope (mirrors the OC-03 Stage 3F helper).
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

/// A deterministic verified source event. Its EventId is derived by the real
/// signed-event path; the fixture label is part of its kind to distinguish
/// otherwise equal payloads without affecting payload entity extraction.
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

fn scored_of(
    selector: &BaselineSelector,
    task: &TaskRecordV1,
    sources: &[SourceEvent],
) -> Vec<ScoredSelection> {
    selector.select_scored(task, sources).expect("scored")
}

// U01/U03/U04/U06/U07 exercise union_candidates directly with a hand-built
// verified prior via the OC-03 pipeline; U02/U05/U08 mix scored + prior.

#[test]
fn union_dedup_both_reason() {
    // U01: an event in BOTH arms dedups at rerank; here the union keeps
    // the event in both arm lists exactly once each (pre-union state).
    let prior = verified_fixture();
    let config = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let task = task("alpha");
    let sources = vec![source("evt-a", "alpha"), source("evt-c", "beta charlie")];
    let lexical = scored_of(&selector, &task, &sources);
    assert!(!lexical.is_empty(), "lexical arm must be non-empty here");
    let union = union_candidates(&lexical, &prior, &sources, &config).expect("union");
    // The same actual EventId is preserved at most once per arm. Rerank
    // (4D) assigns the `both` reason when it consumes these two arm lists.
    let event_a = sources[0].event().to_string();
    assert_eq!(
        union
            .lexical()
            .iter()
            .filter(|event| *event == &event_a)
            .count(),
        1,
        "duplicate EventId in lexical arm"
    );
    let both = union
        .entries()
        .iter()
        .filter(|candidate| candidate.event() == event_a)
        .collect::<Vec<_>>();
    assert_eq!(both.len(), 1, "EventId must be deduplicated in the union");
    assert_eq!(both[0].reason(), "both");
}

#[test]
fn tf_zero_enters_via_prior() {
    // U02: TF=0 event (no lexical match) with positive prior match is a
    // prior-arm candidate (reason `prior` is recorded at rerank, 4D).
    let prior = verified_fixture();
    let config = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    // Task terms that match NOTHING in the pool (lexical arm empty).
    let task = task("zzz qqq");
    let sources = vec![source("evt-a", "alpha"), source("evt-c", "beta charlie")];
    let lexical = scored_of(&selector, &task, &sources);
    assert!(lexical.is_empty(), "precondition: TF=0 for all sources");
    let union = union_candidates(&lexical, &prior, &sources, &config).expect("union");
    let entry = union
        .entries()
        .iter()
        .find(|candidate| candidate.event() == sources[0].event().to_string())
        .expect("prior arm must carry the TF=0 source");
    assert_eq!(entry.reason(), "prior");
    assert_eq!(entry.lexical_raw(), None);
}

#[test]
fn zero_prior_no_entry() {
    // U03: a zero-ppb vector entry contributes nothing (not a candidate,
    // not an orphan).
    let prior = verified_fixture();
    let config = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let task = task("zzz");
    let sources = vec![source("evt-zero", "unmatched")];
    let lexical = scored_of(&selector, &task, &sources);
    let union = union_candidates(&lexical, &prior, &sources, &config).expect("union");
    assert!(
        union.prior().is_empty(),
        "no positive match entered prior arm"
    );
}

#[test]
fn orphan_entity_counted() {
    // U04: positive vector entries matching no pool event are counted in
    // orphan_prior_entities (Ok, not Err — bound is X10's).
    let prior = verified_fixture();
    let config = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let task = task("zzz");
    // Pool contains only ONE of the two prior-backed events.
    let sources = vec![source("evt-a", "alpha")];
    let lexical = scored_of(&selector, &task, &sources);
    let union = union_candidates(&lexical, &prior, &sources, &config).expect("union ok");
    // evt-c (beta charlie) is prior-positive but absent from the pool,
    // so at least its entities are orphaned.
    assert!(
        union.orphan_prior_entities() > 0,
        "orphan counter must be > 0 when a prior-positive event is absent"
    );
}

#[test]
fn empty_prior_identity() {
    // U05: an empty prior yields a union whose lexical arm equals the
    // capped lexical arm byte-for-byte and whose prior arm is empty.
    let config = PriorConfigV1::default();
    let graph = build_entity_graph(&[], &config).expect("empty graph");
    let (seeds, dropped) = derive_seeds(&[], &[], &config).expect("empty seeds");
    let ppr = run_ppr(&graph, &seeds, &config).expect("empty ppr");
    let empty_prior =
        assemble_prior(graph, seeds, &ppr, dropped, "terminal", &config).expect("assemble");
    let bytes = empty_prior.canonical_bytes().expect("canonical");
    let verified = VerifiedPrior::verify(&bytes, &[], &[], &[], &config).expect("verified");
    assert!(verified.positive_seeds().is_empty());

    let oc04cfg = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let task = task("alpha");
    let sources = vec![source("evt-a", "alpha"), source("evt-b", "alpha alpha")];
    let lexical = scored_of(&selector, &task, &sources);
    let union = union_candidates(&lexical, &verified, &sources, &oc04cfg).expect("union");
    let expected: Vec<String> = lexical
        .iter()
        .map(|s| s.reference().event().to_string())
        .collect();
    assert_eq!(union.lexical(), expected);
    assert!(union.prior().is_empty());
    assert_eq!(union.orphan_prior_entities(), 0);
}

#[test]
fn prior_only_union() {
    // U06: with the lexical arm empty, the union is entirely prior-arm,
    // ordered by canonical EventId text ascending.
    let prior = verified_fixture();
    let config = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let task = task("zzz qqq");
    let sources = vec![source("evt-c", "beta charlie"), source("evt-a", "alpha")];
    let lexical = scored_of(&selector, &task, &sources);
    assert!(lexical.is_empty());
    let union = union_candidates(&lexical, &prior, &sources, &config).expect("union");
    assert!(!union.entries().is_empty());
    assert!(
        union.entries().iter().all(|candidate| {
            candidate.reason() == "prior" && candidate.lexical_raw().is_none()
        })
    );
    assert!(
        union.prior().windows(2).all(|pair| {
            pair[0].raw_ppb() > pair[1].raw_ppb()
                || (pair[0].raw_ppb() == pair[1].raw_ppb() && pair[0].event() <= pair[1].event())
        }),
        "prior arm must be ppb-desc with canonical EventId tie-break"
    );
}

#[test]
fn union_permutation_stable() {
    // U07: shuffling the source pool cannot change the union outcome
    // (canonical-order reconstruction; results compare equal).
    let prior = verified_fixture();
    let config = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let task = task("alpha beta");
    let pool_a = vec![
        source("evt-a", "alpha"),
        source("evt-b", "beta"),
        source("evt-c", "beta charlie"),
    ];
    let mut pool_b = vec![
        source("evt-c", "beta charlie"),
        source("evt-a", "alpha"),
        source("evt-b", "beta"),
    ];
    let lex_a = scored_of(&selector, &task, &pool_a);
    let lex_b = scored_of(&selector, &task, &pool_b);
    let union_a = union_candidates(&lex_a, &prior, &pool_a, &config).expect("union a");
    let union_b = union_candidates(&lex_b, &prior, &pool_b, &config).expect("union b");
    assert_eq!(union_a, union_b, "union not permutation-stable");
    pool_b.clear();
}

#[test]
fn per_arm_caps_enforced() {
    // U08: caps enforced pre-union — a 100-candidate lexical arm truncates
    // to 64; the prior arm truncates to 30.
    let prior = verified_fixture();
    let config = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let task = task("alpha");
    let sources: Vec<SourceEvent> = (0..100)
        .map(|i| source(&format!("evt-{i:03}"), "alpha"))
        .collect();
    let lexical = scored_of(&selector, &task, &sources);
    assert!(lexical.len() > 64, "fixture must exceed the lexical cap");
    let union = union_candidates(&lexical, &prior, &sources, &config).expect("union");
    assert_eq!(union.lexical().len(), 64, "lexical cap not enforced");
    assert_eq!(union.prior().len(), 30, "prior cap not enforced");
}
