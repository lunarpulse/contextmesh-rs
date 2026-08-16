//! OA-05 CLI snapshot matrix: exact exits, commands, and JSON shapes.
//!
//! The fixture substitutes the flow's own live identifiers into expected
//! documents, so every byte is asserted deterministically. Each Command is a
//! separate process, so the whole matrix is restart evidence (05-C02).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "oa05-cli-{tag}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

struct Output {
    exit: Option<i32>,
    stdout: String,
}

fn run_cli(db: Option<&Path>, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_contextmesh"));
    command.args(args);
    // Options are position-independent without positionals; append --db last.
    if let Some(db) = db {
        command.arg("--db").arg(db);
    }
    let output = command.output().expect("run CLI");
    Output {
        exit: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
    }
}

fn parse(stdout: &str) -> serde_json::Value {
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim_end()).expect("stdout is one JSON document");
    // Exactly one document, one line.
    assert_eq!(
        stdout.trim_end_matches(['\r', '\n']).matches('\n').count(),
        0
    );
    value
}

fn substitute(template: serde_json::Value, map: &BTreeMap<&str, String>) -> serde_json::Value {
    let text = serde_json::to_string(&template).expect("template");
    let mut text = text;
    for (key, value) in map {
        text = text.replace(&format!("<{key}>"), value);
    }
    serde_json::from_str(&text).expect("substituted")
}

fn assert_case(
    fixture: &BTreeMap<String, serde_json::Value>,
    name: &str,
    output: &Output,
    map: &BTreeMap<&str, String>,
) -> serde_json::Value {
    let expected = fixture[name].clone();
    let exit = expected["exit"].as_i64().expect("exit") as i32;
    assert_eq!(output.exit, Some(exit), "{name} exit");
    let actual = parse(&output.stdout);
    let expected_doc = substitute(expected, map);
    assert_eq!(actual["command"], expected_doc["command"], "{name} command");
    assert_eq!(actual["ok"], expected_doc["ok"], "{name} ok");
    assert_eq!(
        actual["schema_version"], expected_doc["schema_version"],
        "{name} schema_version"
    );
    if expected_doc["ok"] == true {
        assert_eq!(actual["result"], expected_doc["result"], "{name} result");
    } else {
        assert_eq!(actual["error"], expected_doc["error"], "{name} error");
    }
    actual
}

fn fixtures() -> BTreeMap<String, serde_json::Value> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/oa05-cli-golden.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("json")
}

/// 05-C01/C02: every command's exit and exact JSON document across restarts.
#[test]
fn cli_snapshot_matrix_survives_restart() {
    let fixture = fixtures();
    let dir = temp_dir("matrix");
    let key = dir.join("key");
    let token = dir.join("token");
    let db = dir.join("a.db");
    let second = dir.join("b.db");
    let payload = dir.join("p.json");
    let input = dir.join("in.json");
    let bundle = dir.join("bundle.json");
    std::fs::write(&payload, r#"{"note":"cli"}"#).expect("payload");
    std::fs::write(&input, r#"{"q":1}"#).expect("input");
    let mut map: BTreeMap<&str, String> = BTreeMap::new();

    let out = run_cli(None, &["key", "generate", "--file", key.to_str().unwrap()]);
    let author = parse(&out.stdout)["result"]["author"]
        .as_str()
        .expect("author")
        .to_owned();
    map.insert("AUTH", author);
    assert_case(&fixture, "key_generate", &out, &map);

    let out = run_cli(
        None,
        &["token", "generate", "--file", token.to_str().unwrap()],
    );
    assert_case(&fixture, "token_generate", &out, &map);

    // Restart: a fresh process loads the same key and continues.
    let out = run_cli(
        Some(&db),
        &["context", "create", "--key-file", key.to_str().unwrap()],
    );
    let parsed = parse(&out.stdout);
    map.insert(
        "CTX",
        parsed["result"]["context"]
            .as_str()
            .expect("ctx")
            .to_owned(),
    );
    map.insert(
        "GEN",
        parsed["result"]["genesis"]
            .as_str()
            .expect("gen")
            .to_owned(),
    );
    assert_case(&fixture, "context_create", &out, &map);

    let out = run_cli(
        Some(&db),
        &[
            "append",
            "--key-file",
            key.to_str().unwrap(),
            "--context",
            map["CTX"].as_str(),
            "--branch",
            "main",
            "--expected-head",
            map["GEN"].as_str(),
            "--kind",
            "demo.note",
            "--payload-file",
            payload.to_str().unwrap(),
        ],
    );
    let parsed = parse(&out.stdout);
    map.insert(
        "EV",
        parsed["result"]["event"].as_str().expect("ev").to_owned(),
    );
    assert_case(&fixture, "append", &out, &map);
    let out = run_cli(
        Some(&db),
        &["show", "refs", "--context", map["CTX"].as_str()],
    );
    assert_case(&fixture, "show_refs", &out, &map);

    let out = run_cli(
        Some(&db),
        &[
            "show",
            "projection",
            "--context",
            map["CTX"].as_str(),
            "--head",
            map["EV"].as_str(),
        ],
    );
    assert_case(&fixture, "show_projection", &out, &map);

    let out = run_cli(Some(&db), &["show", "event", "--id", map["EV"].as_str()]);
    assert_case(&fixture, "show_event", &out, &map);

    let out = run_cli(
        Some(&db),
        &[
            "invoke",
            "--key-file",
            key.to_str().unwrap(),
            "--context",
            map["CTX"].as_str(),
            "--branch",
            "main",
            "--expected-head",
            map["EV"].as_str(),
            "--input-file",
            input.to_str().unwrap(),
            "--provider-command",
            env!("CARGO_BIN_EXE_demo_agent"),
        ],
    );
    let parsed = parse(&out.stdout);
    map.insert(
        "INV",
        parsed["result"]["invocation_id"]
            .as_str()
            .expect("inv")
            .to_owned(),
    );
    map.insert(
        "REQ",
        parsed["result"]["request"]
            .as_str()
            .expect("req")
            .to_owned(),
    );
    map.insert(
        "RES",
        parsed["result"]["result"].as_str().expect("res").to_owned(),
    );
    assert_case(&fixture, "invoke", &out, &map);

    let out = run_cli(
        Some(&db),
        &[
            "invocation",
            "pending",
            "--context",
            map["CTX"].as_str(),
            "--branch",
            "main",
        ],
    );
    assert_case(&fixture, "invocation_pending", &out, &map);
    let out = run_cli(
        Some(&db),
        &[
            "invocation",
            "detached",
            "--context",
            map["CTX"].as_str(),
            "--branch",
            "main",
        ],
    );
    assert_case(&fixture, "invocation_detached", &out, &map);

    let out = run_cli(Some(&db), &["verify"]);
    assert_case(&fixture, "verify", &out, &map);

    let out = run_cli(
        Some(&db),
        &[
            "bundle",
            "export",
            "--context",
            map["CTX"].as_str(),
            "--head",
            map["RES"].as_str(),
            "--out",
            bundle.to_str().unwrap(),
        ],
    );
    assert_case(&fixture, "bundle_export", &out, &map);

    let out = run_cli(
        Some(&second),
        &[
            "context",
            "join",
            "--context",
            map["CTX"].as_str(),
            "--expected-genesis",
            map["GEN"].as_str(),
            "--author",
            map["AUTH"].as_str(),
        ],
    );
    assert_eq!(out.exit, Some(0));

    let out = run_cli(
        Some(&second),
        &[
            "bundle",
            "import",
            "--peer",
            "alpha",
            "--file",
            bundle.to_str().unwrap(),
        ],
    );
    assert_case(&fixture, "bundle_import", &out, &map);

    let out = run_cli(None, &["key", "generate", "--file", key.to_str().unwrap()]);
    assert_case(&fixture, "key_duplicate", &out, &map);

    let out = run_cli(
        Some(&db),
        &["show", "event", "--id", &format!("evt1_{}", "A".repeat(43))],
    );
    assert_case(&fixture, "show_event_missing", &out, &map);

    let out = run_cli(Some(&db), &["show", "event", "--id", "not-an-id"]);
    assert_case(&fixture, "invalid_id", &out, &map);

    let out = run_cli(Some(&db), &["no-such-command"]);
    assert_case(&fixture, "usage_error", &out, &map);

    let out = run_cli(
        Some(&db),
        &[
            "sync",
            "--peer",
            "alpha",
            "--url",
            "http://127.0.0.1:9",
            "--token-file",
            token.to_str().unwrap(),
            "--context",
            map["CTX"].as_str(),
        ],
    );
    assert_case(&fixture, "sync_transport", &out, &map);
}
