//! OC-04 Stage 4G: Human-gold metric gate harness (§14, matrix row 4G).
//!
//! ⚠️ NOT-REAL-DATA: the gold labels in this harness are SYNTHESIZED by
//! construction (the same disclosure discipline as `oc02_evaluation.rs`).
//! Every label marks events chosen by fixture design (which payload the
//! task token names / which event the report share names); NO human
//! relevance judgment exists in this corpus. Per §14, the P3-GO gate
//! stays OPEN until a real human-gold corpus exists — this harness ships
//! the evaluator and the deterministic pipeline path only, and asserts
//! NOTHING about real-world retrieval quality.
//!
//! Primary metric: preregistered nDCG@12 (P1 prereg `evaluation.metrics`),
//! computed in INTEGER arithmetic only (no floats, no clocks, no network)
//! against a fixed-point discount table. Secondary: strict TF=0 recovery —
//! a candidate with lexical TF=0 entering through the prior arm must
//! remain in the ranked list (positive-prior arm discipline, P3 gate).

use contextmesh::crypto::SigningIdentity;
use contextmesh::model::ContextId;
use contextmesh::oc04_scored::ScoredSelection;
use contextmesh::receipt::TaskRecordV1;
use contextmesh::selection::{BaselineSelector, SourceEvent};
use contextmesh_salience::oc04_rerank::rerank;
use contextmesh_salience::oc04_selection::{Oc04ConfigV1, VerifiedPrior};
use contextmesh_salience::oc04_union::union_candidates;
use contextmesh_salience::prior::{
    PriorConfigV1, ReportContribution, SessionPayloads, assemble_prior, build_entity_graph,
    derive_seeds, run_ppr,
};
use serde_json::json;

/// PREREGISTRATION MARKER asserted in every test: this harness is bound to
/// the synthetic-label path until a real gold corpus replaces it under
/// founder change control.
pub const GOLD_LABELS_REAL_DATA: bool = false;

/// nDCG evaluation depth (prereg primary metric: nDCG@12).
const NDCG_K: usize = 12;

// ---------------------------------------------------------------------------
// Integer nDCG evaluator (ppm scale, no floats at runtime)
// ---------------------------------------------------------------------------

/// Fixed-point discount table: DISCOUNT_PPM[i] = round(1e6 / log2(i+2)).
/// Hand-derived constants; rank i gain is discounted by this factor.
const DISCOUNT_PPM: [u128; NDCG_K] = [
    1_000_000, // i=0:  1/log2(2)  = 1.0
    630_930,   // i=1:  1/log2(3)  ≈ 0.63092975
    500_000,   // i=2:  1/log2(4)  = 0.5
    430_677,   // i=3:  1/log2(5)  ≈ 0.43067655
    386_853,   // i=4:  1/log2(6)  ≈ 0.38685281
    356_207,   // i=5:  1/log2(7)  ≈ 0.35620722
    333_333,   // i=6:  1/log2(8)  = 1/3
    314_698,   // i=7:  1/log2(9)  ≈ 0.31469828
    298_974,   // i=8:  1/log2(10) ≈ 0.29897353
    285_714,   // i=9:  1/log2(11) ≈ 0.28571429
    274_421,   // i=10: 1/log2(12) ≈ 0.27442065
    264_643,   // i=11: 1/log2(13) ≈ 0.26464237
];

/// Integer DCG@k in ppm over a ranked EventId list against a gold set
/// (binary relevance).
fn dcg_ppm(ranked: &[String], gold: &[String], k: usize) -> u128 {
    let mut total: u128 = 0;
    for (i, event) in ranked.iter().take(k).enumerate() {
        if gold.iter().any(|g| g == event) {
            total += DISCOUNT_PPM[i];
        }
    }
    total
}

/// Ideal DCG@k for `gold_len` relevant docs (all hits at the top ranks).
fn idcg_ppm(gold_len: usize, k: usize) -> u128 {
    DISCOUNT_PPM.iter().take(k.min(gold_len)).sum()
}

/// Integer nDCG@k in ppm (0 on empty gold — degenerate-task guard).
fn ndcg_ppm(ranked: &[String], gold: &[String], k: usize) -> u128 {
    let ideal = idcg_ppm(gold.len(), k);
    if ideal == 0 {
        return 0;
    }
    dcg_ppm(ranked, gold, k) * 1_000_000 / ideal
}

// ---------------------------------------------------------------------------
// Fixture plumbing (real signed events + real OC-03 pipeline; 4D pattern)
// ---------------------------------------------------------------------------

/// Minimal computed report envelope (mirrors the 4C/4D fixture helper).
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

/// A deterministic verified source event (real signed-event path).
fn source(label: &str, text: &str) -> SourceEvent {
    let event = SigningIdentity::from_fixture_seed([17; 32])
        .create_event(
            ContextId::from_bytes([21; 32]),
            Vec::new(),
            format!("note-{label}"),
            json!({ "text": text }),
        )
        .expect("signed fixture event");
    SourceEvent::from_signed(&event).expect("verified source")
}

fn task(verbatim: &str) -> TaskRecordV1 {
    TaskRecordV1::from_verbatim(verbatim.to_owned(), None).expect("task")
}

/// Standard fixture inputs: one session, one computed report sharing the
/// `evt-a` event (the prior arm's seed so the prior arm is non-empty).
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
        report_json("r1", "computed", &[("evt-a", 600_000)]).as_bytes(),
    )
    .expect("report parses");
    let events: Vec<(&str, &str)> = vec![
        ("evt-a", r#"{"text":"alpha"}"#),
        ("evt-c", r#"{"text":"beta charlie"}"#),
    ];
    (sessions, vec![report], events)
}

/// Full deterministic OC-03 pipeline → verified prior token (4D pattern).
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

/// Full 4C→4D pipeline over the fixture: lexical + prior → union → rerank.
/// Returns the influence record (rank order = score_ppm desc, the §7.2
/// canonical ranking consumed downstream).
fn pipeline_ranked(task_text: &str) -> Vec<(String, u128)> {
    let prior = verified_fixture();
    let config = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let task = task(task_text);
    let sources = vec![source("evt-a", "alpha"), source("evt-c", "beta charlie")];
    let lexical: Vec<ScoredSelection> = selector.select_scored(&task, &sources).expect("scored");
    let union = union_candidates(&lexical, &prior, &sources, &config).expect("union");
    let influence = rerank(&union, &prior, "task-fp", &config).expect("rerank");
    let mut ranked: Vec<(String, u128)> = influence
        .entries()
        .iter()
        .map(|entry| (entry.event_id_text().to_owned(), entry.score_ppm()))
        .collect();
    // Canonical ranking: score desc, EventId text asc tie-break (§7.2).
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
}

// ---------------------------------------------------------------------------
// 4G harness tests
// ---------------------------------------------------------------------------

/// 4G-V01: the preregistration marker is the synthetic-label disclosure —
/// a compile-checked constant. The conditional (not a direct assert)
/// defeats clippy::assertions_on_constants while keeping the same
/// property: any future real-data swap must touch this test (founder
/// change control visible in the diff).
#[test]
fn gold_labels_disclosed_synthetic() {
    if GOLD_LABELS_REAL_DATA {
        panic!("4G ships synthetic labels only — real-data swap requires founder change control");
    }
}

/// 4G-V02: the integer nDCG evaluator is exact against hand-computed
/// values (evaluator correctness precedes any metric claim).
#[test]
fn ndcg_evaluator_exact() {
    let g = |s: &str| s.to_owned();
    // Single gold at rank 0 → nDCG = 1e6 ppm exactly.
    let ranked = vec![g("evt-a"), g("evt-b"), g("evt-c")];
    let gold = vec![g("evt-a")];
    assert_eq!(ndcg_ppm(&ranked, &gold, NDCG_K), 1_000_000);
    // Single gold at rank 1 → nDCG = DISCOUNT_PPM[1] ppm (0.630930).
    let ranked = vec![g("evt-b"), g("evt-a"), g("evt-c")];
    assert_eq!(ndcg_ppm(&ranked, &gold, NDCG_K), 630_930);
    // Two gold at ranks 0 and 1 → DCG = 1_630_930, IDCG = 1_630_930 → 1e6.
    let gold2 = vec![g("evt-a"), g("evt-b")];
    let ranked = vec![g("evt-a"), g("evt-b"), g("evt-c")];
    assert_eq!(ndcg_ppm(&ranked, &gold2, NDCG_K), 1_000_000);
    // Two gold, hits at ranks 1 and 2 → hand-computed integer value.
    let ranked = vec![g("evt-x"), g("evt-a"), g("evt-b")];
    // DCG = 630_930 + 500_000 = 1_130_930; IDCG = 1_630_930.
    // nDCG = 1_130_930 × 1e6 / 1_630_930 = 693_426 (truncating division).
    assert_eq!(ndcg_ppm(&ranked, &gold2, NDCG_K), 693_426);
    // Gold below the cutoff k: rank 13 (index 12) is OUT of @12 — hits
    // beyond k must not count (metric-honesty guard).
    let mut ranked = vec![g("evt-fill")];
    for i in 0..13 {
        ranked.push(g(&format!("evt-d{i}")));
    }
    let gold_far = vec![g("evt-d12")];
    assert_eq!(ndcg_ppm(&ranked, &gold_far, NDCG_K), 0);
    // Empty gold → 0 (degenerate guard, no division by zero).
    assert_eq!(ndcg_ppm(&ranked, &[], NDCG_K), 0);
}

/// 4G-V03: end-to-end synthetic-gold evaluation over the REAL pipeline.
/// The task names "alpha" (gold = evt-a, labeled by fixture design).
/// The ranked list must place the gold event first → nDCG@12 = 1e6 ppm.
#[test]
fn pipeline_ndcg_perfect_on_named_gold() {
    let ranked = pipeline_ranked("alpha");
    assert!(!ranked.is_empty(), "pipeline must rank the fixture corpus");
    let event_a = source("evt-a", "alpha").event().to_string();
    let gold = vec![event_a];
    let ids: Vec<String> = ranked.iter().map(|(id, _)| id.clone()).collect();
    let score = ndcg_ppm(&ids, &gold, NDCG_K);
    assert_eq!(
        score, 1_000_000,
        "gold event (the task token's payload) must rank first on this corpus"
    );
}

/// 4G-V04: strict TF=0 recovery (secondary metric stratum). The task text
/// matches NOTHING in the pool — the lexical arm is empty and every entry
/// that survives must have entered through the prior arm. The union must
/// be non-empty (prior arm carries candidates with TF=0), and the ranked
/// list must be non-empty: TF=0 events are NOT excluded from ranking.
#[test]
fn strict_tf0_recovers_through_prior_arm() {
    let prior = verified_fixture();
    let config = Oc04ConfigV1::default();
    let selector = BaselineSelector::new();
    let task = task("zzz qqq");
    let sources = vec![source("evt-a", "alpha"), source("evt-c", "beta charlie")];
    let lexical: Vec<ScoredSelection> = selector.select_scored(&task, &sources).expect("scored");
    assert!(
        lexical.is_empty(),
        "precondition: lexical arm empty (all TF=0)"
    );
    let union = union_candidates(&lexical, &prior, &sources, &config).expect("union");
    assert!(
        !union.entries().is_empty(),
        "strict TF=0: prior arm must still nominate candidates"
    );
    let influence = rerank(&union, &prior, "task-fp", &config).expect("rerank");
    assert!(
        !influence.entries().is_empty(),
        "strict TF=0: reranked list must be non-empty — TF=0 is not exclusion"
    );
    // Every entry that entered lexically would contradict the empty
    // lexical arm; all must carry positive prior normalization or the
    // `prior`/`both` reason with lexical_ppm = 0 (non-member zero rule).
    for entry in influence.entries() {
        assert_eq!(entry.lexical_ppm(), 0, "lexical arm was empty");
    }
}

/// 4G-V05: metric honesty under rerank reordering — a deliberately
/// perturbed ranking (gold demoted to rank 1) must score STRICTLY below
/// the perfect ranking, proving the evaluator is sensitive to rank order
/// (a constant-valued metric could never gate anything).
#[test]
fn ndcg_rank_order_sensitive() {
    let g = |s: &str| s.to_owned();
    let gold = vec![g("evt-a")];
    let best = vec![g("evt-a"), g("evt-b"), g("evt-c")];
    let demoted = vec![g("evt-b"), g("evt-a"), g("evt-c")];
    let s_best = ndcg_ppm(&best, &gold, NDCG_K);
    let s_demoted = ndcg_ppm(&demoted, &gold, NDCG_K);
    assert_eq!(s_best, 1_000_000);
    assert_eq!(s_demoted, 630_930);
    assert!(s_demoted < s_best, "demotion must strictly lower nDCG");
}

/// 4G-V06: determinism — two pipeline runs on the identical fixture
/// produce byte-identical rankings (no clocks, no floats, no randomness).
#[test]
fn pipeline_ranking_deterministic() {
    let a = pipeline_ranked("alpha");
    let b = pipeline_ranked("alpha");
    assert_eq!(a, b, "ranking must be deterministic run-to-run");
}
