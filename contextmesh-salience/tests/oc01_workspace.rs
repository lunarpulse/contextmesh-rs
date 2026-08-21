use contextmesh::crypto::SigningIdentity;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const HISTORICAL_TIMEOUT: Duration = Duration::from_secs(1_800);
const OA07_COMMIT: &str = "9c275f0f83b320d697dc9ccccc2b51ee60a05114";
const OB13_COMMIT: &str = "1df53344afc29ac7730e373de1fb4a46def3a9f5";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static WORKTREE_LOCK: Mutex<()> = Mutex::new(());

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

const OA_SCRIPT_HASHES: &[(&str, &str)] = &[
    (
        "scripts/verify-oa00.sh",
        "81aa0da49934c55fecaa6adc99b8912fca04e6f460ebd3cebdf6befdb929c77d",
    ),
    (
        "scripts/verify-oa01.sh",
        "aa49a3cb745c7274ac8e6fbe60b71c65a03b5cec10b2c79d0d9998cd64d823f6",
    ),
    (
        "scripts/verify-oa02.sh",
        "a3b184408ba021c4a49f4cd64854823e94c648e2c2889a4ec283f17d7d4d1c34",
    ),
    (
        "scripts/verify-oa03.sh",
        "d010ed928a0f24f237393f859ea8831847383f141124d05bec9d2faeb79eb494",
    ),
    (
        "scripts/verify-oa04.sh",
        "f69326dfaf576dcbbf6f2996f046f4137bf90b6e8a68d78eae6928bcf10d8559",
    ),
    (
        "scripts/verify-oa04-dependencies.sh",
        "fc8fbcba46e114d66e28f53744067ae40f966343684f4687bb5d1d097cef0a33",
    ),
    (
        "scripts/verify-oa05.sh",
        "e93069379a04962d44671a2d925e9678fdd5c13892eaf739ca95b4bf7a684618",
    ),
    (
        "scripts/verify-oa06.sh",
        "4386997f008952cff8a3ce2b8da6ea511f6bd0ad0f770b163b71b30e63e3b998",
    ),
    (
        "scripts/verify-oa07.sh",
        "d3cda00f56dd6d8ddae97fa88291e2a55d94a57ca56bd6ff11c9b4d28580f87d",
    ),
];

const OB_SCRIPT_HASHES: &[(&str, &str)] = &[
    (
        "scripts/verify-ob01.sh",
        "c4b07275952c57d8eae6ead3e7f70b397e058262c564840f963291bc88611fac",
    ),
    (
        "scripts/verify-ob02.sh",
        "c2a141b87714ebb4f87c862f5418d138e18aa71c80ddecfcb162bfc93d36fbb3",
    ),
    (
        "scripts/verify-ob03.sh",
        "d5ade1db451c6fc2ce7e32705d75f4e58d97fb5f2bdbf7a55097ae0dace553e6",
    ),
    (
        "scripts/verify-ob04.sh",
        "9c95a0bbfc7ed8a9c17accd3822a7a9eb15d9789768ad146a5d7c75e0702af4d",
    ),
    (
        "scripts/verify-ob05.sh",
        "fabde8211e47f6a7fab2340030a2311183c0ace3aa1091275bd64ef7f83c15a8",
    ),
    (
        "scripts/verify-ob06.sh",
        "bfa448c1586775291cee65232b195ddcaef4e904bc1fd3ec390b696b1784e9df",
    ),
    (
        "scripts/verify-ob07.sh",
        "c3d47481ee394593ddffbd547da86dd53f47349a6a32c6225f96f149b02638e3",
    ),
    (
        "scripts/verify-ob08.sh",
        "6f236c980b72d8f772f092cbaa7970847c1c4dfc0d83775dc9964275f64834ba",
    ),
    (
        "scripts/verify-ob09.sh",
        "ae5d3e0d036259285b7363e55b782b64f471828dc56752746af9dc7e1c56a15a",
    ),
    (
        "scripts/verify-ob10.sh",
        "fa635b2594693481e9cdf24698b06fabde8b225058570a08736ece8ff1a6c2ee",
    ),
    (
        "scripts/verify-ob11.sh",
        "2b1bd82c78b3824087d15961042881186ceb259d2e77c2400f2ad27475b98db2",
    ),
    (
        "scripts/verify-ob12.sh",
        "fe40303455c6d4159b53ecf59dcd7fb34d70c138335a05917c162a0e74f718b9",
    ),
    (
        "scripts/verify-ob13.sh",
        "8de64abbc4e792112fa6fb1ab0931d5485f743aca43e12c78d5376539f5deae1",
    ),
];

fn sha256(path: &Path) -> String {
    let mut command = Command::new("sha256sum");
    command.arg(path);
    let output = run(command);
    String::from_utf8(output.stdout)
        .expect("sha256sum output is UTF-8")
        .split_whitespace()
        .next()
        .expect("sha256sum emits a digest")
        .to_owned()
}

fn assert_script_hashes(commit: &str, expected: &[(&str, &str)]) {
    let mut verify_commit = Command::new("git");
    verify_commit
        .current_dir(root())
        .args(["cat-file", "-e", &format!("{commit}^{{commit}}")]);
    run(verify_commit);

    let mut rev_parse = Command::new("git");
    rev_parse
        .current_dir(root())
        .args(["rev-parse", &format!("{commit}^{{commit}}")]);
    assert_eq!(
        String::from_utf8(run(rev_parse).stdout).unwrap().trim(),
        commit,
        "historical commit identity changed"
    );

    for (path, digest) in expected {
        assert_eq!(
            sha256(&root().join(path)),
            *digest,
            "legacy verifier changed"
        );
        let mut historical = Command::new("git");
        historical
            .current_dir(root())
            .args(["show", &format!("{commit}:{path}")]);
        let bytes = run(historical).stdout;
        let input = TemporaryInput::create("historical-script", &bytes);
        assert_eq!(
            sha256(&input.path),
            *digest,
            "historical verifier baseline changed"
        );
    }
}

struct HistoricalWorktree {
    repository: PathBuf,
    parent: PathBuf,
    path: PathBuf,
    added: bool,
}

impl HistoricalWorktree {
    fn add(commit: &str, tag: &str) -> Self {
        let nonce = SigningIdentity::generate()
            .expect("operating-system entropy")
            .author();
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let parent = std::env::temp_dir().join(format!(
            "oc01-worktree-{tag}-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&parent).expect("exclusive temporary worktree parent");
        let path = parent.join("checkout");
        let mut result = Self {
            repository: root(),
            parent,
            path,
            added: false,
        };
        let mut command = Command::new("git");
        command
            .current_dir(&result.repository)
            .args(["worktree", "add", "--detach"])
            .arg(&result.path)
            .arg(commit);
        run(command);
        result.added = true;
        result
    }
}

impl Drop for HistoricalWorktree {
    fn drop(&mut self) {
        if self.added {
            let _ = Command::new("git")
                .current_dir(&self.repository)
                .args(["worktree", "remove", "--force"])
                .arg(&self.path)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _ = Command::new("git")
            .current_dir(&self.repository)
            .args(["worktree", "prune"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = fs::remove_dir_all(&self.parent);
    }
}

fn historical_chain(commit: &str, verifier: &str, hashes: &[(&str, &str)], tag: &str) {
    if std::env::var_os("OC01_INNER_CURRENT_GATE").is_some() {
        return;
    }
    let _serial = WORKTREE_LOCK.lock().expect("historical worktree lock");
    assert_script_hashes(commit, hashes);
    let worktree = HistoricalWorktree::add(commit, tag);

    let mut status = Command::new("git");
    status
        .current_dir(&worktree.path)
        .args(["status", "--porcelain"]);
    assert!(
        run(status).stdout.is_empty(),
        "historical worktree is not clean"
    );

    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))
        .expect("Cargo home is discoverable");
    let proxy_directory = cargo_home.join("bin");
    assert!(
        proxy_directory.join("cargo").is_file(),
        "rustup Cargo proxy missing"
    );
    let mut search_path = vec![proxy_directory];
    if let Some(existing) = std::env::var_os("PATH") {
        search_path.extend(std::env::split_paths(&existing));
    }
    let search_path = std::env::join_paths(search_path).expect("toolchain PATH joins");

    let mut command = Command::new("bash");
    command
        .current_dir(&worktree.path)
        .env("PATH", search_path)
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_BUILD_JOBS", "1")
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_PROFILE_TEST_DEBUG", "0")
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("RUSTUP_TOOLCHAIN")
        .arg(verifier)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command.spawn().expect("historical verifier starts");
    let mut stdout = child.stdout.take().expect("historical stdout pipe");
    let mut stderr = child.stderr.take().expect("historical stderr pipe");
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .read_to_end(&mut bytes)
            .expect("historical stdout read");
        bytes
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr
            .read_to_end(&mut bytes)
            .expect("historical stderr read");
        bytes
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("historical verifier status") {
            break status;
        }
        if started.elapsed() >= HISTORICAL_TIMEOUT {
            let _ = Command::new("kill")
                .args(["-KILL", &format!("-{}", child.id())])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = child.wait();
            panic!("historical verifier timed out");
        }
        thread::sleep(Duration::from_millis(100));
    };
    let stdout = stdout_reader.join().expect("historical stdout reader");
    let stderr = stderr_reader.join().expect("historical stderr reader");
    if !status.success() {
        let summary = stdout
            .split(|byte| *byte == b'\n')
            .chain(stderr.split(|byte| *byte == b'\n'))
            .find_map(|line| {
                let text = std::str::from_utf8(line).ok()?;
                let allowed_prefix = text.starts_with("verify-oa07: FAIL ")
                    || text.starts_with("verify-ob13: FAIL ");
                (allowed_prefix
                    && text.len() <= 256
                    && text
                        .bytes()
                        .all(|byte| byte == b' ' || byte.is_ascii_graphic()))
                .then_some(text)
            })
            .unwrap_or("historical verifier failed");
        panic!("{summary}");
    }
}

#[test]
fn historical_oa07_chain_runs_unchanged_at_completion_commit() {
    historical_chain(
        OA07_COMMIT,
        "scripts/verify-oa07.sh",
        OA_SCRIPT_HASHES,
        "oa07",
    );
}

#[test]
fn historical_ob13_chain_runs_unchanged_at_completion_commit() {
    historical_chain(
        OB13_COMMIT,
        "scripts/verify-ob13.sh",
        OB_SCRIPT_HASHES,
        "ob13",
    );
}

#[test]
fn current_workspace_checks_are_package_scoped_and_legacy_scripts_immutable() {
    if std::env::var_os("OC01_INNER_CURRENT_GATE").is_some() {
        return;
    }
    assert_script_hashes(OA07_COMMIT, OA_SCRIPT_HASHES);
    assert_script_hashes(OB13_COMMIT, OB_SCRIPT_HASHES);

    let verifier = fs::read_to_string(root().join("scripts/verify-oc01.sh"))
        .expect("current OC verifier exists");
    for command in [
        "cargo build -p contextmesh --locked",
        "cargo build -p contextmesh-salience --locked",
        "cargo build --workspace --locked",
        "cargo test -p contextmesh-salience --locked",
        "cargo test -p contextmesh --locked",
        "cargo test --workspace --locked",
    ] {
        assert!(
            verifier.contains(command),
            "missing package-scoped current check"
        );
    }
    assert!(!verifier.contains("bash scripts/verify-oa"));
    assert!(!verifier.contains("bash scripts/verify-ob"));

    let mut command = Command::new("bash");
    command
        .current_dir(root())
        .args(["scripts/verify-oc01.sh", "--planned-surface-only"]);
    run(command);
}
