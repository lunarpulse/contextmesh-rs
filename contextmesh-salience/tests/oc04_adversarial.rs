//! OC-04 Stage 4F adversarial and boundary matrix: E07 + X01–X12b.
//! Tests use public production surfaces only; no test-only token constructor,
//! unsafe code, trybuild, or new dependency is permitted.

use contextmesh::closure::{ClosureLimits, CriticalPolicy};
use contextmesh::crypto::{SigningIdentity, verify_domain_message};
use contextmesh::delta::RecipientState;
use contextmesh::handoff::Handoff;
use contextmesh::model::{AuthorId, ContextId, EventId};
use contextmesh::oc04_scored::ScoredSelection;
use contextmesh::receipt::TaskRecordV1;
use contextmesh::repair::{RepairBounds, RepairHistory, TaskOutcome};
use contextmesh::selection::{BaselineSelector, Selector, SourceEvent};
use contextmesh::store::{ContextProvision, RefExpectation, RefMutation, Store};
use contextmesh_salience::oc04_exec::{
    ExecutionChainInputs, HandoffError4E, bind_execution, verify_execution,
};
use contextmesh_salience::oc04_rerank::rerank;
use contextmesh_salience::oc04_selection::{
    ENTRY_REASON_LEXICAL, ORPHAN_PRIOR_ENTITIES_MAX, Oc04ConfigV1, ScratchHistoryGuard,
    SelectionInfluenceEntryV1, SelectionInfluenceV1, SignedExecutionV1, VerifiedPrior,
    derive_execution_id, render_execution_body,
};
use contextmesh_salience::oc04_union::union_candidates;
use contextmesh_salience::prior::{
    PriorConfigV1, ReportContribution, SessionPayloads, assemble_prior, build_entity_graph,
    derive_seeds, run_ppr,
};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn temp_path(label: &str, ext: &str) -> PathBuf {
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "contextmesh-oc04f-{label}-{}-{serial}.{ext}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn identity(seed: u8) -> SigningIdentity {
    SigningIdentity::from_fixture_seed([seed; 32])
}

fn context(byte: u8) -> ContextId {
    ContextId::from_bytes([byte; 32])
}

fn config() -> Oc04ConfigV1 {
    Oc04ConfigV1::default()
}

fn report_json(report_id: &str, shares: &[(&str, u128)]) -> String {
    let m4: Vec<String> = shares
        .iter()
        .map(|(event, ppm)| {
            format!(
                "{{\"event\":\"{event}\",\"judge\":\"j.example\",\"judge_config_hash\":\"h\",\"judge_version\":\"v1\",\"samples\":64,\"share_ppm\":{ppm}}}"
            )
        })
        .collect();
    let tier = format!(
        "{{\"m3\":[],\"m4\":[{}],\"status\":\"computed\",\"uncertainty_markers\":[]}}",
        m4.join(",")
    );
    format!(
        "{{\"adapter_tier\":\"{}\",\"config_hash\":\"ocattrcfg1_x\",\"ledger_id\":\"ocout1_a\",\"prereg_reference\":\"be20d8fc\",\"report_id\":\"{report_id}\",\"task_fingerprint\":\"t\",\"input_snapshot_fingerprint\":\"i\",\"deterministic_tier\":\"d\",\"terminal_status\":\"terminal\",\"version\":1}}",
        tier.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

type PriorInputs = (
    Vec<SessionPayloads<'static>>,
    Vec<ReportContribution>,
    Vec<(&'static str, &'static str)>,
);

fn prior_inputs() -> PriorInputs {
    let sessions = vec![SessionPayloads::from_payloads(vec![
        r#"{"text":"alpha"}"#,
        r#"{"text":"beta charlie"}"#,
    ])];
    let report = ReportContribution::from_report_bytes(
        report_json("r1", &[("evt-a", 600_000), ("evt-c", 200_000)]).as_bytes(),
    )
    .expect("report parses");
    let events = vec![
        ("evt-a", r#"{"text":"alpha"}"#),
        ("evt-c", r#"{"text":"beta charlie"}"#),
    ];
    (sessions, vec![report], events)
}

fn prior_artifact() -> (Vec<u8>, PriorInputs) {
    let cfg = PriorConfigV1::default();
    let (sessions, reports, events) = prior_inputs();
    let graph = build_entity_graph(&sessions, &cfg).expect("graph");
    let (seeds, dropped) = derive_seeds(&reports, &events, &cfg).expect("seeds");
    let ppr = run_ppr(&graph, &seeds, &cfg).expect("ppr");
    let prior = assemble_prior(graph, seeds, &ppr, dropped, "terminal", &cfg).expect("prior");
    let bytes = prior.canonical_bytes().expect("canonical prior");
    (bytes, (sessions, reports, events))
}

fn verified_prior() -> VerifiedPrior {
    let (bytes, (sessions, reports, events)) = prior_artifact();
    VerifiedPrior::verify(
        &bytes,
        &sessions,
        &reports,
        &events,
        &PriorConfigV1::default(),
    )
    .expect("verified prior")
}

fn source(label: &str, text: &str) -> SourceEvent {
    let event = identity(17)
        .create_event(
            context(21),
            Vec::new(),
            format!("note-{label}"),
            json!({"text": text}),
        )
        .expect("event");
    SourceEvent::from_signed(&event).expect("source")
}

fn task(text: &str) -> TaskRecordV1 {
    TaskRecordV1::from_verbatim(text.to_owned(), None).expect("task")
}

fn scored(
    selector: &BaselineSelector,
    task: &TaskRecordV1,
    sources: &[SourceEvent],
) -> Vec<ScoredSelection> {
    selector.select_scored(task, sources).expect("scored")
}

const LIMITS: ClosureLimits = ClosureLimits {
    max_events: 100_000,
    max_exported_bytes: 64 * 1024 * 1024,
};

fn policy() -> CriticalPolicy {
    CriticalPolicy::new(vec!["context.critical".to_owned()]).unwrap()
}

fn budget() -> contextmesh::selection::SelectionBudget {
    contextmesh::selection::SelectionBudget {
        max_selected_events: 100_000,
        max_exported_bytes: 64 * 1024 * 1024,
    }
}

fn bounds() -> RepairBounds {
    RepairBounds {
        max_iterations: 8,
        max_re_included_events: 8,
        max_delta_bytes: 64 * 1024 * 1024,
    }
}

async fn provision(store: &Store, event: &contextmesh::model::SignedEventV1, author: AuthorId) {
    store
        .provision_context(ContextProvision {
            context: event.body().context(),
            expected_genesis: event.event_id(),
            authorized_authors: vec![author],
        })
        .await
        .unwrap();
}

async fn chain_store(depth: usize) -> (Store, SigningIdentity, ContextId, Vec<EventId>) {
    let db = temp_path("chain", "db");
    let store = Store::open(&db).await.unwrap();
    let signer = identity(7);
    let ctx = context(8);
    let genesis = signer
        .create_event(ctx, vec![], "context.genesis", json!({"note": "root"}))
        .unwrap();
    provision(&store, &genesis, signer.author()).await;
    store.admit(&genesis, RefMutation::None).await.unwrap();
    let mut ids = vec![genesis.event_id()];
    let mut head = genesis.event_id();
    for step in 1..=depth {
        let event = signer
            .create_event(
                ctx,
                vec![head],
                "agent.request",
                json!({"note": format!("step {step}"), "value": step}),
            )
            .unwrap();
        let expected = if step == 1 {
            RefExpectation::Absent
        } else {
            RefExpectation::Head(head)
        };
        store
            .admit(
                &event,
                RefMutation::CompareAndSwap {
                    context: ctx,
                    name: "main".parse().unwrap(),
                    expected,
                    new_head: event.event_id(),
                },
            )
            .await
            .unwrap();
        head = event.event_id();
        ids.push(head);
    }
    (store, signer, ctx, ids)
}

struct Rig {
    store: Store,
    signer: SigningIdentity,
    context: ContextId,
    ids: Vec<EventId>,
    policy: CriticalPolicy,
    limits: ClosureLimits,
    budget: contextmesh::selection::SelectionBudget,
    recipient: RecipientState,
    bounds: RepairBounds,
}

async fn rig(depth: usize) -> Rig {
    let (store, signer, context, ids) = chain_store(depth).await;
    Rig {
        recipient: RecipientState::cold_start(context),
        policy: policy(),
        limits: LIMITS,
        budget: budget(),
        bounds: bounds(),
        store,
        signer,
        context,
        ids,
    }
}

type Driver = fn(&Handoff) -> std::future::Ready<TaskOutcome>;
type DriverFactory = fn() -> Driver;

fn success_driver(_: &Handoff) -> std::future::Ready<TaskOutcome> {
    std::future::ready(TaskOutcome::Success)
}

fn success_factory() -> Driver {
    success_driver
}

fn chain_inputs<'a>(
    r: &'a Rig,
    scratch: &'a Path,
    history: &'a mut RepairHistory,
) -> ExecutionChainInputs<'a, DriverFactory, Driver, std::future::Ready<TaskOutcome>> {
    ExecutionChainInputs {
        context: &r.context,
        store: &r.store,
        b3_candidates: &r.ids,
        b3_policy: &r.policy,
        b3_limits: &r.limits,
        budget: &r.budget,
        recipient: &r.recipient,
        repair_bounds: &r.bounds,
        repair_driver_factory: success_factory,
        repair_history: history,
        scratch_history_path: scratch,
        critical_ids: &r.ids,
    }
}

fn influence(ids: &[EventId]) -> SelectionInfluenceV1 {
    let mut entries = ids
        .iter()
        .map(|id| {
            SelectionInfluenceEntryV1::new(id.to_string(), ENTRY_REASON_LEXICAL, 1, 0).unwrap()
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        b.score_ppm()
            .cmp(&a.score_ppm())
            .then_with(|| a.event_id_text().cmp(b.event_id_text()))
    });
    SelectionInfluenceV1::assemble(
        &config(),
        "ocprior1_0000000000000000000000000000000000000000000000000000000000000000",
        "task-fingerprint-fixtures",
        entries,
    )
    .unwrap()
}

async fn bound_execution(r: &Rig, label: &str) -> (SignedExecutionV1, PathBuf) {
    let history_path = temp_path(&format!("{label}-history"), "jsonl");
    let mut history = RepairHistory::open(&history_path).unwrap();
    let scratch = temp_path(&format!("{label}-scratch"), "jsonl");
    let mut chain = chain_inputs(r, &scratch, &mut history);
    let (env, _) = bind_execution(&influence(&r.ids), &mut chain, &r.signer, &config())
        .await
        .expect("bind");
    (env, history_path)
}

#[test]
fn verified_prior_compile_gate() {
    // Resolve artifacts relative to this test executable so custom
    // CARGO_TARGET_DIR/profile layouts are honored. Try every matching rlib
    // rather than guessing by mtime; success means the snippet reached the
    // intended private-field gate in a linkable crate instance.
    let deps = std::env::current_exe()
        .expect("current test executable")
        .parent()
        .expect("test executable deps directory")
        .to_path_buf();
    let mut candidates = std::fs::read_dir(&deps)
        .expect("target deps")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("libcontextmesh_salience-") && name.ends_with(".rlib")
                })
        })
        .collect::<Vec<_>>();
    candidates.sort();
    assert!(!candidates.is_empty(), "contextmesh_salience rlib missing");
    let snippet =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/compile/oc04_token_privacy.rs");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let mut diagnostics = Vec::new();
    let mut privacy_gate_seen = false;
    for (index, rlib) in candidates.iter().enumerate() {
        let out = temp_path(&format!("privacy-{index}"), "rlib");
        let output = std::process::Command::new(&rustc)
            .arg("--edition=2021")
            .arg("--crate-type=lib")
            .arg(&snippet)
            .arg("--extern")
            .arg(format!("contextmesh_salience={}", rlib.display()))
            .arg("-L")
            .arg(format!("dependency={}", deps.display()))
            .arg("-o")
            .arg(&out)
            .output()
            .expect("rustc harness");
        let _ = std::fs::remove_file(out);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() && stderr.contains("E0616") && stderr.contains("field `prior`")
        {
            privacy_gate_seen = true;
            break;
        }
        diagnostics.push(format!("{}:\n{stderr}", rlib.display()));
    }
    assert!(
        privacy_gate_seen,
        "expected private-field E0616 from a linked crate artifact; attempts:\n{}",
        diagnostics.join("\n")
    );
}

#[test]
fn unverified_prior_rejected() {
    let (mut bytes, (sessions, reports, events)) = prior_artifact();
    let needle = b"\"iterations\":";
    let at = bytes
        .windows(needle.len())
        .position(|w| w == needle)
        .unwrap()
        + needle.len();
    assert!(bytes[at].is_ascii_digit());
    bytes[at] = if bytes[at] == b'9' {
        b'8'
    } else {
        bytes[at] + 1
    };
    assert!(
        VerifiedPrior::verify(
            &bytes,
            &sessions,
            &reports,
            &events,
            &PriorConfigV1::default()
        )
        .is_err(),
        "X01: tampered prior bytes must fail rebuild verification"
    );
}

#[tokio::test]
async fn forged_influence_rejected() {
    let r = rig(2).await;
    // Strong self-consistent forgery: preserve the entry COUNT while
    // replacing one actual candidate with a different valid EventId, then
    // let assemble re-derive a matching influence_id. A count-only check
    // would accept this shape; exact set equality must reject it.
    let alien = identity(99)
        .create_event(
            context(99),
            Vec::new(),
            "agent.request",
            json!({"note": "alien"}),
        )
        .unwrap()
        .event_id();
    assert!(!r.ids.contains(&alien));
    let mut forged_ids = r.ids.clone();
    *forged_ids.last_mut().unwrap() = alien;
    let forged = influence(&forged_ids);
    assert_eq!(forged.entries().len(), r.ids.len());
    assert!(forged.influence_id().starts_with("oc04inf1_"));
    let history_path = temp_path("x02-history", "jsonl");
    let mut history = RepairHistory::open(&history_path).unwrap();
    let scratch = temp_path("x02-scratch", "jsonl");
    let mut chain = chain_inputs(&r, &scratch, &mut history);
    assert!(
        bind_execution(&forged, &mut chain, &r.signer, &config())
            .await
            .is_err(),
        "X02: internally self-consistent influence must diverge from actual candidate set"
    );
}

#[tokio::test]
async fn forged_execution_rejected() {
    let r = rig(1).await;
    let (env, _) = bound_execution(&r, "x03").await;
    env.verify()
        .expect("precondition: original envelope verifies");
    let original_bytes = render_execution_body(env.body());
    let mut tampered = env.body().clone();
    tampered.closed_count += 1;
    tampered.execution_id = derive_execution_id(&tampered);
    let bytes = render_execution_body(&tampered);
    assert_ne!(
        bytes, original_bytes,
        "X03 precondition: body bytes changed"
    );
    let mut author = [0_u8; 32];
    author.copy_from_slice(env.signer());
    let result = verify_domain_message(
        AuthorId::from_bytes(author),
        contextmesh_salience::oc04_selection::OC04_EXEC_SIGNATURE_DOMAIN,
        &bytes,
        env.signature(),
    );
    assert!(
        result.is_err(),
        "X03: original signature must reject tampered body"
    );
}

#[test]
fn baseline_invariance() {
    let selector = BaselineSelector::new();
    let sources = vec![source("a", "alpha alpha"), source("b", "alpha beta")];
    let task = task("alpha");
    let before = selector.select(&task, &sources).unwrap();
    let scored = selector.select_scored(&task, &sources).unwrap();
    let after = selector.select(&task, &sources).unwrap();
    assert_eq!(before, after, "X04: additive OC-04 API changed B2 output");
    assert_eq!(
        before.iter().map(|r| r.event()).collect::<Vec<_>>(),
        scored
            .iter()
            .map(|r| r.reference().event())
            .collect::<Vec<_>>()
    );
}

#[test]
fn thorn_absent() {
    let sources = [
        include_str!("../src/oc04_selection.rs"),
        include_str!("../src/oc04_union.rs"),
        include_str!("../src/oc04_rerank.rs"),
        include_str!("../src/oc04_exec.rs"),
    ];
    for source in sources {
        let code_only = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code_only
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .any(|t| t == "Thorn" || t == "thorn"),
            "X05: Thorn identifier found in OC-04 production code"
        );
    }
}

fn sha256(bytes: &[u8]) -> String {
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sha256sum");
    child.stdin.as_mut().unwrap().write_all(bytes).unwrap();
    String::from_utf8(child.wait_with_output().unwrap().stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

#[test]
fn no_new_deps() {
    assert_eq!(
        sha256(include_bytes!("../../Cargo.toml")),
        "7c2075b807d9e5b7471e73aca95fa2984f9059da613d0eabae8c9bc5bb470124"
    );
    assert_eq!(
        sha256(include_bytes!("../../Cargo.lock")),
        "653accffb3d64e3a2810d4974112637fb98e7efa7eb1ab0a3ce99c543ea1ddf0"
    );
    assert_eq!(
        sha256(include_bytes!("../Cargo.toml")),
        "e6aa9120a7115a08978dae517641fd6f80869ee2d393ae20ddf8db6f6261c3f4"
    );
}

#[test]
fn duplicate_folded_once() {
    let prior = verified_prior();
    let selector = BaselineSelector::new();
    let sources = vec![source("evt-a", "alpha"), source("evt-c", "beta charlie")];
    let lexical = scored(&selector, &task("alpha"), &sources);
    let union = union_candidates(&lexical, &prior, &sources, &config()).unwrap();
    let influence = rerank(&union, &prior, "task-fingerprint", &config()).unwrap();
    let id = sources[0].event().to_string();
    let matches = influence
        .entries()
        .iter()
        .filter(|entry| entry.event_id_text() == id)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "X07: duplicate EventId must fold once");
    assert_eq!(matches[0].entry_reason(), "both");
}

#[tokio::test]
async fn stale_state_no_artifact() {
    let r = rig(2).await;
    let (env, history_path) = bound_execution(&r, "x08").await;
    let drifted = RecipientState::at_head(&r.store, r.context, r.ids[0], &r.limits)
        .await
        .unwrap();
    let before = std::fs::read(&history_path).unwrap();
    let mut history = RepairHistory::open(&history_path).unwrap();
    let scratch = temp_path("x08-scratch", "jsonl");
    {
        let mut chain = ExecutionChainInputs {
            recipient: &drifted,
            ..chain_inputs(&r, &scratch, &mut history)
        };
        assert_eq!(
            verify_execution(&env, &mut chain, &config()).await,
            Err(HandoffError4E::Malformed),
            "X08: recipient head drift must reject at replay-body divergence"
        );
    }
    drop(history);
    assert_eq!(
        std::fs::read(&history_path).unwrap(),
        before,
        "X08: stale rejection mutated production history"
    );
    assert!(!scratch.exists(), "X08: stale rejection leaked scratch");
}

#[test]
fn noncanonical_prior_payload_rejected() {
    let (bytes, (mut sessions, reports, events)) = prior_artifact();
    sessions[0] = SessionPayloads::from_payloads(vec![
        "{ \"text\" : \"alpha\" }",
        r#"{"text":"beta charlie"}"#,
    ]);
    assert!(
        VerifiedPrior::verify(
            &bytes,
            &sessions,
            &reports,
            &events,
            &PriorConfigV1::default()
        )
        .is_err(),
        "X09: semantically equivalent but non-canonical payload must reject"
    );
}

#[test]
fn orphan_bound_fail_closed() {
    // The verified OC-03 graph caps entities at exactly 1,024, equal to the
    // OC-04 orphan bound. Therefore a 1,025-entry VerifiedPrior cannot be
    // constructed through the only public constructor. Literal 1024 pins
    // BOTH constants against joint drift (e.g. both moving to 2048); the
    // pairwise check alone would pass such a joint move. §17 records the
    // >1,024 branch as impossible through the verified surface.
    assert_eq!(contextmesh_salience::prior::caps::MAX_ENTITIES, 1024);
    assert_eq!(
        usize::try_from(ORPHAN_PRIOR_ENTITIES_MAX).unwrap(),
        contextmesh_salience::prior::caps::MAX_ENTITIES
    );
    // Honest evidence scope: the ordinary fixture proves the reachable
    // count is bounded by the cap, not that the cap value itself is
    // exercised — a 1,024-entity verified fixture is not constructible in
    // this suite within the frozen per-event/per-session bounds.
    let prior = verified_prior();
    assert!(
        prior.positive_seeds().len() <= contextmesh_salience::prior::caps::MAX_ENTITIES,
        "X10: reachable verified positive entities must not exceed the orphan bound"
    );
}

#[tokio::test]
async fn verifier_replay_positive() {
    let r = rig(2).await;
    let (env, history_path) = bound_execution(&r, "x11").await;
    let before = std::fs::read(&history_path).unwrap();
    let mut history = RepairHistory::open(&history_path).unwrap();
    let scratch = temp_path("x11-scratch", "jsonl");
    {
        let mut chain = chain_inputs(&r, &scratch, &mut history);
        verify_execution(&env, &mut chain, &config()).await.unwrap();
    }
    drop(history);
    let after = std::fs::read(&history_path).unwrap();
    assert_eq!(before, after, "X11: replay mutated production history");
    assert!(
        !scratch.exists(),
        "X11: RAII guard did not remove scratch history"
    );
}

#[tokio::test]
async fn verifier_replay_wrong_hash() {
    let r = rig(2).await;
    let (env, history_path) = bound_execution(&r, "x11b").await;
    let mut body = env.body().clone();
    let original = body.handoff_hash.clone();
    body.handoff_hash = "0".repeat(original.len());
    if body.handoff_hash == original {
        body.handoff_hash = "1".repeat(original.len());
    }
    body.execution_id = derive_execution_id(&body);
    let forged = SignedExecutionV1::issue(body, &r.signer).expect("fresh valid signature");
    let before = std::fs::read(&history_path).unwrap();
    let mut history = RepairHistory::open(&history_path).unwrap();
    let scratch = temp_path("x11b-scratch", "jsonl");
    {
        let mut chain = chain_inputs(&r, &scratch, &mut history);
        assert!(
            verify_execution(&forged, &mut chain, &config())
                .await
                .is_err()
        );
    }
    drop(history);
    assert_eq!(
        std::fs::read(&history_path).unwrap(),
        before,
        "X11b: forged-envelope rejection mutated production history"
    );
    assert!(!scratch.exists(), "X11b: error path leaked scratch");
}

#[test]
fn scratch_guard_same_path_rejected() {
    let path = temp_path("x12-production", "jsonl");
    std::fs::write(&path, b"production-sentinel").unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(ScratchHistoryGuard::reserve(&path, &path).is_err());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "X12: same-path rejection changed production bytes"
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn scratch_guard_existing_file_rejected() {
    let scratch = temp_path("x12b-existing", "jsonl");
    let production = temp_path("x12b-production", "jsonl");
    std::fs::write(&scratch, b"sentinel").unwrap();
    assert!(ScratchHistoryGuard::reserve(&scratch, &production).is_err());
    assert_eq!(std::fs::read(&scratch).unwrap(), b"sentinel");
    std::fs::remove_file(scratch).unwrap();
}
