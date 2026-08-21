use contextmesh::crypto::SigningIdentity;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("salience crate has repository parent")
        .to_path_buf()
}

fn cargo() -> OsString {
    for variable in ["OC01_CARGO", "CARGO"] {
        if let Some(candidate) = std::env::var_os(variable) {
            return candidate;
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&paths) {
            let candidate = directory.join("cargo");
            if candidate.is_file() {
                return candidate.into_os_string();
            }
        }
    }
    if let Some(home) = std::env::var_os("CARGO_HOME") {
        let candidate = PathBuf::from(home).join("bin/cargo");
        if candidate.is_file() {
            return candidate.into_os_string();
        }
    }
    let candidate =
        PathBuf::from(std::env::var_os("HOME").expect("HOME is set")).join(".cargo/bin/cargo");
    assert!(candidate.is_file(), "cargo executable not found");
    candidate.into_os_string()
}

fn run_bounded(mut command: Command) -> Output {
    command.env("CARGO_NET_OFFLINE", "true");
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().expect("command starts");
    let mut stdout = child.stdout.take().expect("stdout pipe");
    let mut stderr = child.stderr.take().expect("stderr pipe");
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).expect("stdout read");
        bytes
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).expect("stderr read");
        bytes
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status") {
            break status;
        }
        if started.elapsed() >= COMMAND_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("command timed out");
        }
        thread::sleep(Duration::from_millis(10));
    };
    Output {
        status,
        stdout: stdout_reader.join().expect("stdout reader"),
        stderr: stderr_reader.join().expect("stderr reader"),
    }
}

fn run(command: Command) -> Output {
    let output = run_bounded(command);
    assert!(output.status.success(), "command failed");
    output
}

fn metadata() -> Value {
    let mut command = Command::new(cargo());
    command
        .current_dir(root())
        .args(["metadata", "--locked", "--format-version", "1"]);
    serde_json::from_slice(&run(command).stdout).expect("cargo metadata is JSON")
}

fn helper_output() -> Output {
    let mut command = Command::new("python3");
    command
        .current_dir(root())
        .arg("scripts/check-core-dependencies.py");
    run(command)
}

fn helper_json() -> Value {
    serde_json::from_slice(&helper_output().stdout).expect("helper emits JSON")
}

fn package_ids_by_name(metadata: &Value) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    for name in ["contextmesh", "contextmesh-salience"] {
        let matches: Vec<_> = metadata["packages"]
            .as_array()
            .expect("packages array")
            .iter()
            .filter(|package| package["name"] == name)
            .collect();
        assert_eq!(matches.len(), 1, "workspace package name is not unique");
        result.insert(
            name.to_owned(),
            matches[0]["id"].as_str().expect("package id").to_owned(),
        );
    }
    result
}

fn reachable(metadata: &Value, start: &str) -> BTreeSet<String> {
    let nodes: BTreeMap<&str, &Value> = metadata["resolve"]["nodes"]
        .as_array()
        .expect("resolve nodes")
        .iter()
        .map(|node| (node["id"].as_str().expect("node id"), node))
        .collect();
    let mut found = BTreeSet::new();
    let mut pending = vec![start.to_owned()];
    while let Some(id) = pending.pop() {
        if !found.insert(id.clone()) {
            continue;
        }
        let node = nodes.get(id.as_str()).expect("reachable node exists");
        for dependency in node["deps"].as_array().expect("node deps") {
            pending.push(
                dependency["pkg"]
                    .as_str()
                    .expect("dependency package id")
                    .to_owned(),
            );
        }
    }
    found
}

#[test]
fn workspace_shape_is_exact() {
    let metadata = metadata();
    assert_eq!(metadata["workspace_root"].as_str(), root().to_str());
    let ids = package_ids_by_name(&metadata);
    let members: BTreeSet<_> = metadata["workspace_members"]
        .as_array()
        .expect("workspace members")
        .iter()
        .map(|member| member.as_str().expect("member id"))
        .collect();
    assert_eq!(members.len(), 2);
    assert_eq!(
        members,
        BTreeSet::from([
            ids["contextmesh"].as_str(),
            ids["contextmesh-salience"].as_str()
        ])
    );

    let core = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "contextmesh")
        .unwrap();
    assert_eq!(
        Path::new(core["manifest_path"].as_str().unwrap()),
        root().join("Cargo.toml")
    );
}

#[test]
fn salience_manifest_and_direct_pins_are_exact() {
    let metadata = metadata();
    let package = metadata["packages"]
        .as_array()
        .expect("packages array")
        .iter()
        .find(|package| package["name"] == "contextmesh-salience")
        .expect("salience package");
    assert_eq!(package["version"], "0.1.0");
    assert_eq!(package["edition"], "2024");
    assert_eq!(package["rust_version"], "1.97");
    assert_eq!(package["publish"], serde_json::json!([]));
    assert_eq!(helper_json()["salience_direct_dependencies_exact"], true);
}

#[test]
fn dependency_direction_is_strictly_one_way() {
    let metadata = metadata();
    let ids = package_ids_by_name(&metadata);
    let core = &ids["contextmesh"];
    let salience = &ids["contextmesh-salience"];
    assert!(reachable(&metadata, salience).contains(core));
    assert!(!reachable(&metadata, core).contains(salience));
}

#[test]
fn salience_adds_no_registry_or_forbidden_capability() {
    let report = helper_json();
    assert_eq!(report["new_registry_identities"], 0);
    assert_eq!(report["forbidden_capabilities"], Value::Array(vec![]));
    assert_eq!(report["salience_direct_dependencies_exact"], true);
}

#[test]
fn workspace_and_lock_counts_match_migration_contract() {
    let report = helper_json();
    assert_eq!(report["workspace_members"], 2);
    assert_eq!(report["workspace_local_packages"], 2);
    assert_eq!(report["core_reachable_packages"], 320);
    assert_eq!(report["core_reachable_external_packages"], 319);
    assert_eq!(report["lock_packages"], 321);
}

#[test]
fn core_registry_closure_is_byte_for_byte_unchanged() {
    let report = helper_json();
    assert_eq!(report["core_registry_closure_unchanged"], true);
    assert_eq!(report["core_direct_dependencies_unchanged"], true);
    assert_eq!(
        report["core_registry_closure_sha256"],
        "ae86da65ff5138bb51836d303ec9370ad9da8c8f112ad84ad59b5e362136113d"
    );
}

fn synthetic_metadata(packages: Value, nodes: Value) -> Value {
    serde_json::json!({
        "packages": packages,
        "workspace_members": [],
        "resolve": {"root": null, "nodes": nodes}
    })
}

struct TemporaryInput {
    path: PathBuf,
}

impl TemporaryInput {
    fn create(tag: &str, bytes: &[u8]) -> Self {
        for _ in 0..32 {
            let nonce = SigningIdentity::generate()
                .expect("operating-system entropy")
                .author();
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "oc01-{tag}-{}-{nonce}-{sequence}.json",
                std::process::id()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    file.write_all(bytes).expect("temporary input write");
                    return Self { path };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => panic!("temporary input creation failed"),
            }
        }
        panic!("temporary input collision bound exceeded");
    }
}

impl Drop for TemporaryInput {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn named_reachability(metadata: &Value, tag: &str) -> Output {
    let input = TemporaryInput::create(tag, &serde_json::to_vec(metadata).unwrap());
    let mut command = Command::new("python3");
    command
        .current_dir(root())
        .env("CARGO_NET_OFFLINE", "true")
        .arg("scripts/check-core-dependencies.py")
        .arg("--named-reachability")
        .arg(&input.path);
    run_bounded(command)
}

#[test]
fn dependency_helper_uses_named_package_reachability() {
    let valid = synthetic_metadata(
        serde_json::json!([{"id": "core-id", "name": "contextmesh", "version": "0.1.0", "source": null}]),
        serde_json::json!([{"id": "core-id", "deps": []}]),
    );
    let output = named_reachability(&valid, "valid");
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["core_reachable"],
        1
    );

    let missing = synthetic_metadata(serde_json::json!([]), serde_json::json!([]));
    let output = named_reachability(&missing, "missing");
    assert!(!output.status.success());
    assert_eq!(output.stderr, b"dependency audit failed\n");

    let duplicate = synthetic_metadata(
        serde_json::json!([
            {"id": "core-a", "name": "contextmesh", "version": "0.1.0", "source": null},
            {"id": "core-b", "name": "contextmesh", "version": "0.1.0", "source": null}
        ]),
        serde_json::json!([{"id": "core-a", "deps": []}, {"id": "core-b", "deps": []}]),
    );
    let output = named_reachability(&duplicate, "duplicate");
    assert!(!output.status.success());
    assert_eq!(output.stderr, b"dependency audit failed\n");
}

#[test]
fn dependency_helper_output_and_feature_tree_are_stable() {
    let first = helper_output();
    let second = helper_output();
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(first.stderr, second.stderr);

    let mut command = Command::new(cargo());
    command
        .current_dir(root())
        .args(["tree", "-p", "contextmesh", "--locked", "-e", "features"]);
    let output = String::from_utf8(run(command).stdout).unwrap();
    let mut lines = output.split_inclusive('\n');
    let first = lines.next().expect("feature tree first line");
    assert!(first.starts_with("contextmesh v0.1.0 (") && first.trim_end().ends_with(')'));
    let actual = format!(
        "contextmesh v0.1.0 (<WORKSPACE>)\n{}",
        lines.collect::<String>()
    );
    assert_eq!(
        actual.into_bytes(),
        fs::read(root().join("cargo-tree-oa05-features.txt")).unwrap()
    );
}
