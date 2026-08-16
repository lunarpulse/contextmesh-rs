//! OA-06 demo matrix: reproducibility, lifecycle, cleanup, and secrecy.
//!
//! Every test drives `scripts/demo.sh` as an external process against an
//! explicit runtime root outside the repository, matching the approved
//! traceability rows 06-D01..06-D06 and the plan's lifecycle/security list.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn temp_base(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "oa06-demo-{tag}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

struct DemoRun {
    exit: Option<i32>,
    stdout: String,
    stderr: String,
    root: PathBuf,
    elapsed: Duration,
}

/// Serializes demo runs so their stage-1 `cargo build --locked` invocations
/// never contend on the target-directory lock; the wall-clock bounds below
/// then measure the demo itself, not build queuing.
static DEMO_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Terminates a child the way the demo's own cleanup would: TERM first, a
/// bounded grace so the EXIT trap can clean up its daemons, then KILL.
fn terminate_gracefully(child: &mut std::process::Child) {
    let pid = child.id() as i32;
    let _ = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status();
    let started = Instant::now();
    while child
        .try_wait()
        .map(|status| status.is_none())
        .unwrap_or(false)
    {
        if started.elapsed() > Duration::from_secs(15) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Runs the demo with an explicit runtime root and a hard wall-clock bound.
fn run_demo(tag: &str, envs: &[(&str, &str)], bound: Duration) -> DemoRun {
    let _guard = DEMO_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let base = temp_base(tag);
    std::fs::create_dir_all(&base).expect("base dir");
    let root = base.join("root");
    let script = repo_root().join("scripts/demo.sh");
    let mut command = Command::new("bash");
    command
        .arg(&script)
        .env("OA06_DEMO_RUNTIME_ROOT", &root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in envs {
        command.env(key, value);
    }
    let started = Instant::now();
    let mut child = command.spawn().expect("spawn demo");
    while child.try_wait().expect("poll demo").is_none() {
        if started.elapsed() >= bound {
            terminate_gracefully(&mut child);
            panic!("demo exceeded its wall-clock bound");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let output = child.wait_with_output().expect("collect demo output");
    // Success deletes the runtime root itself; drop the now-empty base too.
    // Failed runs keep their base: their preserved runtime is evidence.
    if output.status.code() == Some(0) && !root.exists() {
        std::fs::remove_dir(&base).ok();
    }
    DemoRun {
        exit: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        root,
        elapsed: started.elapsed(),
    }
}

fn assert_pass(run: &DemoRun) {
    assert_eq!(run.exit, Some(0), "demo failed:\n{}", run.stderr);
    assert!(
        run.stdout.contains("demo: PASS"),
        "missing PASS line:\n{}",
        run.stdout
    );
    for stage in 1..=17 {
        assert!(
            run.stdout.contains(&format!("stage {stage:02}")),
            "missing stage {stage:02}:\n{}",
            run.stdout
        );
    }
}

fn pass_context(run: &DemoRun) -> String {
    let line = run
        .stdout
        .lines()
        .find(|line| line.starts_with("demo: PASS"))
        .expect("PASS line");
    let start = line.find("context=").expect("context field") + "context=".len();
    let end = line[start..]
        .find(' ')
        .map(|offset| start + offset)
        .unwrap_or(line.len());
    line[start..end].to_owned()
}

/// Arguments of every process on this host, for secret and leak scanning.
fn process_args() -> String {
    let output = Command::new("ps")
        .args(["-eo", "args="])
        .output()
        .expect("ps");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// No daemon of this run's runtime root remains alive.
fn assert_no_daemons(root: &Path) {
    let needle = root.to_string_lossy().to_string();
    for line in process_args().lines() {
        if line.contains(&needle) && line.contains("contextmesh serve") {
            panic!("leftover daemon process: {line}");
        }
    }
}

/// Collects all file bytes under a directory (max 64 MiB per file).
fn collect_files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in std::fs::read_dir(dir).expect("read dir") {
            let entry = entry.expect("entry");
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
                continue;
            }
            let mut bytes = Vec::new();
            std::fs::File::open(&path)
                .expect("open")
                .take(64 * 1024 * 1024)
                .read_to_end(&mut bytes)
                .expect("read");
            out.push((path, bytes));
        }
    }
    let mut files = Vec::new();
    walk(root, &mut files);
    files
}

fn secret_materials(root: &Path) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        (
            "node-a.key",
            std::fs::read(root.join("node-a.key")).expect("key a"),
        ),
        (
            "node-b.key",
            std::fs::read(root.join("node-b.key")).expect("key b"),
        ),
        (
            "node-a.token",
            std::fs::read(root.join("node-a.token")).expect("token a"),
        ),
        (
            "node-b.token",
            std::fs::read(root.join("node-b.token")).expect("token b"),
        ),
    ]
}

fn assert_transcript_secret_free(run: &DemoRun) {
    assert!(
        !run.stdout.contains("token1_"),
        "token prefix leaked into stdout:\n{}",
        run.stdout
    );
    assert!(
        !run.stderr.contains("token1_"),
        "token prefix leaked into stderr:\n{}",
        run.stderr
    );
}

/// 06-D01..06-D06 plus plan section 31: the demo passes with the public,
/// count-only summary and all seventeen stages.
#[test]
fn demo_passes_with_public_summary() {
    let run = run_demo("summary", &[], Duration::from_secs(240));
    assert_pass(&run);
    assert!(run.stdout.contains("authors=2 events=6 stages=17"));
    assert_transcript_secret_free(&run);
    assert_no_daemons(&run.root);
}

/// Plan section 31: the demo runs twice reproducibly with fresh OS-random
/// secrets (distinct context identities), never fixtures.
#[test]
fn demo_runs_twice_with_fresh_secrets() {
    let first = run_demo("twice-1", &[], Duration::from_secs(240));
    let second = run_demo("twice-2", &[], Duration::from_secs(240));
    assert_pass(&first);
    assert_pass(&second);
    assert_ne!(
        pass_context(&first),
        pass_context(&second),
        "two runs reused one context identity"
    );
}

/// Plan section 31 "occupy port": fixed ports are structurally impossible
/// (daemons bind 127.0.0.1:0), so the equivalent adversarial condition is
/// two concurrent independent instances competing for ephemeral ports. While
/// both run, live daemon arguments are sampled and contain no token bytes
/// (tokens cross argv only as file paths, by construction).
#[test]
fn concurrent_demos_use_independent_ports() {
    let _guard = DEMO_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    // Pre-build once so neither child queues on the cargo target lock.
    let build = Command::new("cargo")
        .args(["build", "--workspace", "--locked"])
        .current_dir(repo_root())
        .status()
        .expect("pre-build");
    assert!(build.success(), "pre-build failed");

    let spawn = |tag: &str| {
        let script = repo_root().join("scripts/demo.sh");
        let base = temp_base(tag);
        let root = base.join("root");
        let mut command = Command::new("bash");
        command
            .arg(&script)
            .env("OA06_DEMO_RUNTIME_ROOT", &root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        (command.spawn().expect("spawn"), base)
    };
    let bound = Duration::from_secs(240);
    let started = Instant::now();
    let (mut left, left_base) = spawn("concurrent-left");
    let (mut right, right_base) = spawn("concurrent-right");
    loop {
        if left.try_wait().expect("poll left").is_some()
            && right.try_wait().expect("poll right").is_some()
        {
            break;
        }
        assert!(
            !process_args().contains("token1_"),
            "token prefix visible in live process arguments"
        );
        if started.elapsed() >= bound {
            terminate_gracefully(&mut left);
            terminate_gracefully(&mut right);
            panic!("concurrent demos exceeded their wall-clock bound");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let left = left.wait_with_output().expect("collect left");
    let right = right.wait_with_output().expect("collect right");
    std::fs::remove_dir_all(left_base).ok();
    std::fs::remove_dir_all(right_base).ok();
    assert_eq!(left.status.code(), Some(0), "left demo failed");
    assert_eq!(right.status.code(), Some(0), "right demo failed");
    let left_stdout = String::from_utf8_lossy(&left.stdout);
    let right_stdout = String::from_utf8_lossy(&right.stdout);
    assert!(left_stdout.contains("demo: PASS"), "left demo missing PASS");
    assert!(
        right_stdout.contains("demo: PASS"),
        "right demo missing PASS"
    );
}

/// Delayed daemon readiness within the timeout still passes: the harness
/// polls, it never sleeps blindly.
#[test]
fn readiness_delay_still_passes() {
    let run = run_demo(
        "delay-ok",
        &[("OA06_DEMO_TEST_SERVE_DELAY_SECS", "2")],
        Duration::from_secs(240),
    );
    assert_pass(&run);
}

/// Readiness beyond the hard bound fails promptly, preserves the runtime,
/// and leaves no processes behind.
#[test]
fn readiness_timeout_is_bounded() {
    let run = run_demo(
        "delay-timeout",
        &[
            ("OA06_DEMO_TEST_SERVE_DELAY_SECS", "60"),
            ("OA06_DEMO_READY_TIMEOUT_SECS", "2"),
        ],
        Duration::from_secs(120),
    );
    assert_ne!(run.exit, Some(0), "unready daemon unexpectedly passed");
    assert!(
        run.stderr.contains("readiness timeout"),
        "unexpected failure:\n{}",
        run.stderr
    );
    assert!(
        run.elapsed < Duration::from_secs(60),
        "failure was not bounded: {:?}",
        run.elapsed
    );
    assert!(
        run.root.join("node-a.log").is_file(),
        "failure did not preserve logs at {}",
        run.root.display()
    );
    assert_no_daemons(&run.root);
    std::fs::remove_dir_all(run.root.parent().expect("parent")).ok();
}

/// A crashed daemon fails the demo, the surviving daemon is cleaned up, the
/// runtime is preserved, and no recorded PID survives.
#[test]
fn crash_after_ready_fails_and_cleans() {
    let run = run_demo(
        "crash",
        &[("OA06_DEMO_TEST_CRASH_AFTER_READY", "node-a")],
        Duration::from_secs(240),
    );
    assert_ne!(run.exit, Some(0), "crash injection unexpectedly passed");
    assert!(
        run.stderr.contains("injected crash"),
        "unexpected failure:\n{}",
        run.stderr
    );
    assert!(run.root.join("node-a.log").is_file());
    assert!(run.root.join("node-b.log").is_file());
    assert!(run.stderr.contains("runtime preserved"));
    assert_no_daemons(&run.root);
    std::fs::remove_dir_all(run.root.parent().expect("parent")).ok();
}

/// Success deletes the runtime root; OA06_DEMO_KEEP=1 retains it while the
/// transcript stays secret-free.
#[test]
fn success_deletes_runtime_and_debug_keeps_it() {
    let plain = run_demo("cleanup-plain", &[], Duration::from_secs(240));
    assert_pass(&plain);
    assert!(
        !plain.root.exists(),
        "runtime root survived a successful run"
    );

    let kept = run_demo(
        "cleanup-keep",
        &[("OA06_DEMO_KEEP", "1")],
        Duration::from_secs(240),
    );
    assert_pass(&kept);
    assert!(kept.stdout.contains("runtime kept at"));
    assert!(kept.root.join("node-a.db").is_file());
    assert!(kept.root.join("node-a.key").is_file());
    assert_transcript_secret_free(&kept);
    assert_no_daemons(&kept.root);
    std::fs::remove_dir_all(kept.root.parent().expect("runtime parent")).ok();
}

/// Token and seed material never appears in the transcript, in any runtime
/// file except the secret files themselves, or in any process arguments.
#[test]
fn transcripts_logs_and_process_args_have_no_secrets() {
    let run = run_demo(
        "secrets",
        &[("OA06_DEMO_KEEP", "1")],
        Duration::from_secs(240),
    );
    assert_pass(&run);
    let secrets = secret_materials(&run.root);
    let transcript = run.stdout.as_bytes();
    let stderr = run.stderr.as_bytes();
    for (name, material) in &secrets {
        assert!(
            !contains_subslice(transcript, material),
            "{name} material leaked into stdout"
        );
        assert!(
            !contains_subslice(stderr, material),
            "{name} material leaked into stderr"
        );
    }
    let args = process_args();
    for (name, material) in &secrets {
        let text = String::from_utf8_lossy(material);
        assert!(
            !args.contains(text.as_ref()),
            "{name} material leaked into process arguments"
        );
    }
    let secret_names: Vec<std::ffi::OsString> = secrets
        .iter()
        .map(|(name, _)| std::ffi::OsString::from(*name))
        .collect();
    for (path, bytes) in collect_files(&run.root) {
        if secret_names.contains(&path.file_name().expect("name").to_os_string()) {
            continue;
        }
        for (name, material) in &secrets {
            assert!(
                !contains_subslice(&bytes, material),
                "{name} material leaked into {}",
                path.display()
            );
        }
    }
    std::fs::remove_dir_all(run.root.parent().expect("parent")).ok();
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len().max(1))
        .any(|window| window == needle)
}

/// Repository hygiene around demo runs: the `.gitignore` rules keep local
/// database runtime files out of version control (the demo itself runs in
/// an external private root), and a demo run leaves the worktree's porcelain
/// state byte-identical, proving it writes nothing into the repository. The
/// fully clean-tree guarantee for the released state is additionally
/// asserted by scripts/verify-oa06.sh, which runs after the OA-06 commit.
#[test]
fn runtime_artifacts_are_ignored_and_repo_stays_clean() {
    for name in ["node-a.db", "node-a.db-wal", "node-a.db-shm"] {
        let status = Command::new("git")
            .args(["check-ignore", "-q", name])
            .current_dir(repo_root())
            .status()
            .expect("git check-ignore");
        assert!(status.success(), "{name} is not ignored");
    }

    let before = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root())
        .output()
        .expect("git status")
        .stdout;

    let run = run_demo("clean-tree", &[], Duration::from_secs(240));
    assert_pass(&run);

    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root())
        .output()
        .expect("git status");
    assert_eq!(
        output.stdout, before,
        "the demo changed the repository worktree"
    );
    // The fully clean-tree guarantee for the released state is additionally
    // asserted by scripts/verify-oa06.sh, which runs after the OA-06 commit.
}

/// Every command family the demo documents appears in the transcript, and
/// the frozen OA-05 CLI snapshot fixture still enumerates every documented
/// command family, so every documented command stays under exact test.
#[test]
fn documented_commands_are_covered() {
    let run = run_demo("commands", &[], Duration::from_secs(240));
    assert_pass(&run);
    for token in [
        "key generate",
        "token generate",
        "key repair-permissions",
        "context create",
        "context authorize",
        "context join",
        "bundle export",
        "bundle import",
        "branch create",
        "invocation pending",
        "invocation detached",
        "show event",
        "show projection",
        "show refs",
        "invoke",
        "merge",
        "serve",
        "sync",
        "verify",
    ] {
        assert!(
            run.stdout.contains(token),
            "command family {token} missing from the transcript"
        );
    }

    let fixture = std::fs::read_to_string(
        repo_root()
            .join("tests")
            .join("fixtures")
            .join("oa05-cli-golden.json"),
    )
    .expect("fixture");
    let document: serde_json::Value = serde_json::from_str(&fixture).expect("fixture json");
    let keys: Vec<&str> = document
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    for required in [
        "key_generate",
        "token_generate",
        "context_create",
        "append",
        "show_refs",
        "show_projection",
        "show_event",
        "invoke",
        "invocation_pending",
        "invocation_detached",
        "verify",
        "bundle_export",
        "bundle_import",
        "sync_transport",
    ] {
        assert!(
            keys.contains(&required),
            "CLI snapshot fixture lost command family {required}"
        );
    }
}

/// Plan section 31: independent fresh-checkout execution. Ignored in normal
/// suite runs because it cold-builds the workspace in the clone; executed
/// explicitly by scripts/verify-oa06.sh.
#[test]
#[ignore = "cold-builds a fresh checkout; run via verify-oa06.sh"]
fn fresh_checkout_demo_passes() {
    // The clone copies committed state, so the tree must be clean for the
    // fresh checkout to contain the real demo.
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root())
        .output()
        .expect("git status");
    assert_eq!(
        String::from_utf8_lossy(&status.stdout),
        "",
        "worktree must be clean before cloning"
    );

    let base = temp_base("fresh-checkout");
    std::fs::create_dir_all(&base).expect("base");
    let checkout = base.join("checkout");
    let clone = Command::new("git")
        .args(["clone", "--quiet"])
        .arg(repo_root())
        .arg(&checkout)
        .status()
        .expect("git clone");
    assert!(clone.success(), "clone failed");

    let script = checkout.join("scripts").join("demo.sh");
    let root = base.join("runtime");
    let started = Instant::now();
    let mut child = Command::new("bash")
        .arg(&script)
        .env("OA06_DEMO_RUNTIME_ROOT", &root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn fresh demo");
    loop {
        if child.try_wait().expect("poll").is_some() {
            break;
        }
        if started.elapsed() >= Duration::from_secs(1800) {
            terminate_gracefully(&mut child);
            panic!("fresh-checkout demo exceeded 30 minutes");
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let output = child.wait_with_output().expect("collect");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "fresh-checkout demo failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("demo: PASS"), "no PASS in {stdout}");
    assert!(!root.exists(), "fresh runtime root survived success");
    std::fs::remove_dir_all(&base).ok();
}
