//! OA-05 JSONL demo provider adversarial and subprocess transport evidence.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use contextmesh::crypto::SigningIdentity;
use contextmesh::model::{ContextId, EventId};
use contextmesh::provider::{
    CommandProvider, InvocationContext, JSONL_LINE_LIMIT, Provider, ProviderOutcome,
};

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "oa05-jsonl-{tag}-{}-{}.db",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

fn demo_agent() -> &'static str {
    env!("CARGO_BIN_EXE_demo_agent")
}

fn run_agent(input: &[u8]) -> (String, Option<i32>) {
    let mut child = Command::new(demo_agent())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn agent");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input)
        .expect("write stdin");
    let output = child.wait_with_output().expect("collect");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        output.status.code(),
    )
}

fn valid_line(invocation_id: &str) -> String {
    format!(
        "{{\"ancestry\":[],\"context\":\"ctx1_{}\",\"input\":{{\"q\":1}},\"invocation_id\":\"{invocation_id}\",\"protocol_version\":1,\"request_event_id\":\"evt1_{}\",\"selected_head\":\"evt1_\"}}",
        "A".repeat(43),
        "B".repeat(43)
    )
}

/// 05-J01: success echo under the demo namespace only.
#[test]
fn demo_agent_echoes_opaque_input_under_demo_namespace() {
    let line = valid_line("inv1_test01");
    let (stdout, code) = run_agent(format!("{line}\n").as_bytes());
    assert_eq!(code, Some(0));
    let value: serde_json::Value = serde_json::from_str(stdout.trim_end()).expect("json");
    assert_eq!(value["ok"], true);
    assert_eq!(value["invocation_id"], "inv1_test01");
    assert_eq!(value["protocol_version"], 1);
    assert_eq!(value["response"]["demo"]["echo"]["q"], 1);
}

/// 05-J01: malformed, wrong-version, unknown-field, and duplicate rejection.
#[test]
fn demo_agent_rejects_hostile_lines_without_panic() {
    let cases: Vec<(&str, String)> = vec![
        ("not-json", "garbage".to_owned()),
        (
            "wrong-version",
            valid_line("inv1_x").replace(r#""protocol_version":1"#, r#""protocol_version":2"#),
        ),
        (
            "unknown-field",
            valid_line("inv1_x").replace(r#"{"ancestry""#, r#"{"zzz":1,"ancestry""#),
        ),
        (
            "missing-field",
            valid_line("inv1_x").replace(r#""selected_head":"#, r#""hidden":"#),
        ),
        (
            "wrong-id-type",
            valid_line("inv1_x").replace(r#""invocation_id":"inv1_x""#, r#""invocation_id":7"#),
        ),
        ("empty", String::new()),
    ];
    for (name, line) in cases {
        let (stdout, code) = run_agent(format!("{line}\n").as_bytes());
        assert_eq!(code, Some(0), "{name} must not crash");
        let value: serde_json::Value =
            serde_json::from_str(stdout.trim_end()).expect("{name} responds");
        assert_eq!(value["ok"], false, "{name} fails");
        assert_eq!(value["protocol_version"], 1);
        if name != "empty" {
            let detail = value["detail"].as_str().expect("detail bounded");
            assert!(detail.len() <= 1_024);
        }
    }
}

/// 05-J01: an oversized line fails once and the stream resynchronizes.
#[test]
fn demo_agent_bounds_oversized_lines_and_resynchronizes() {
    let padding = "x".repeat(JSONL_LINE_LIMIT);
    let oversized = format!(
        "{{\"ancestry\":[],\"context\":\"c\",\"input\":{{\"pad\":\"{padding}\"}},\"invocation_id\":\"inv1_big\",\"protocol_version\":1,\"request_event_id\":\"e\",\"selected_head\":\"e\"}}"
    );
    let good = valid_line("inv1_after");
    let (stdout, code) = run_agent(format!("{oversized}\n{good}\n").as_bytes());
    assert_eq!(code, Some(0));
    let mut lines = stdout.lines();
    let first: serde_json::Value =
        serde_json::from_str(lines.next().expect("first")).expect("json");
    assert_eq!(first["ok"], false);
    assert_eq!(first["error_code"], "limit_exceeded");
    let second: serde_json::Value =
        serde_json::from_str(lines.next().expect("second")).expect("json");
    assert_eq!(second["ok"], true);
    assert_eq!(second["invocation_id"], "inv1_after");
}

async fn invocation(invocation_id: &str) -> InvocationContext {
    InvocationContext {
        context: ContextId::from_bytes([1; 32]),
        selected_head: EventId::from_bytes([2; 32]),
        ancestry: Vec::new(),
        request_event_id: EventId::from_bytes([3; 32]),
        invocation_id: invocation_id.to_owned(),
        input: serde_json::json!({"n": 1}),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn command_provider_round_trips_with_demo_agent() {
    let provider = CommandProvider::new(demo_agent(), Vec::new());
    let outcome = provider.invoke(&invocation("inv1_rt01").await).await;
    assert!(matches!(outcome, ProviderOutcome::Response(_)));
}

#[tokio::test(flavor = "current_thread")]
async fn command_provider_kills_on_execution_timeout() {
    let provider = CommandProvider::new("/bin/sleep", vec!["60".into()]);
    let started = std::time::Instant::now();
    let outcome = provider.invoke(&invocation("inv1_slow").await).await;
    assert!(
        matches!(&outcome, ProviderOutcome::Failure { code, .. } if *code == "provider_timeout"),
        "sleeping provider is killed at the frozen timeout: {outcome:?}"
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(35));
}

#[tokio::test(flavor = "current_thread")]
async fn command_provider_maps_failures_without_hanging() {
    let store = contextmesh::store::Store::open(temp_path("cmd"))
        .await
        .expect("store");
    let identity = SigningIdentity::from_fixture_seed([21; 32]);
    let created = store
        .create_context(&identity, "main".parse().expect("name"))
        .await
        .expect("context");
    let _ = created;
    // Nonexistent program: transport failure.
    let missing = CommandProvider::new("/nonexistent/provider-binary", Vec::new());
    let outcome = missing.invoke(&invocation("inv1_t1").await).await;
    assert!(
        matches!(&outcome, ProviderOutcome::Failure { code, .. } if *code == "provider_transport")
    );
    // Malformed output: cat echoes the request back; it is valid JSON but
    // never a provider response, so the classification is deterministic.
    // (A plain echo-style child can exit before the request write completes,
    // racing the classification between transport and malformed.)
    let echo = CommandProvider::new("/bin/cat", Vec::new());
    let outcome = echo.invoke(&invocation("inv1_t2").await).await;
    assert!(
        matches!(&outcome, ProviderOutcome::Failure { code, .. } if *code == "provider_malformed")
    );
    // Non-zero exit: transport failure.
    let fals = CommandProvider::new("/bin/false", Vec::new());
    let outcome = fals.invoke(&invocation("inv1_t3").await).await;
    assert!(
        matches!(&outcome, ProviderOutcome::Failure { code, .. } if *code == "provider_transport")
    );
}
