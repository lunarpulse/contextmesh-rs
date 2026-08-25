//! OC-01 Stage 2E I/O tests (matrix rows OC01-X01..X05, X20).
//!
//! Export is create-new, canonical, synced, and store-independent; export
//! failure removes only its own partial new file; import is bounded,
//! regular-file-only, and never repairs input; verified import requires the
//! full DAG and current snapshot; every public failure path is panic-free
//! with no partial outputs; the operation wrapper preserves Artifact/Store/
//! Io causes while the twelve wire categories stay exact.
//!
//! Row OC01-X05's full hostile/injected matrix belongs to
//! `oc01_adversarial.rs` in Stage 2F; the I/O-suite share here is the
//! no-partial-artifact behavior exercised through the public I/O functions.

use std::path::PathBuf;

use contextmesh::crypto::SigningIdentity;
use contextmesh::store::{
    ContextProvision, LocalRefName, PeerName, RefExpectation, RefMutation, Store,
};
use contextmesh_salience::error::{OutcomeError, OutcomeOperationError};
use contextmesh_salience::io::{export_outcome, import_outcome, import_outcome_verified};
use contextmesh_salience::outcome::{OutcomeLedgerBodyV1, SignedOutcomeLedgerV1};
use contextmesh_salience::types::{
    AttemptErrorV1, AttemptStatus, AttemptV1, Blake3HashText, CostLedgerV1, CostValueV1,
    InputRefSnapshotV1, MechanismRecordV1, OutcomeLimits, OutcomeRecordV1, OutcomeValue, QualityV1,
    TaskBindingV1, TerminalV1, TimestampText,
};
use serde_json::json;

/// Published test-only issuer seed for the I/O matrix.
const IO_ISSUER_SEED: [u8; 32] = [0x58; 32];
/// Published test-only core-event author seed for the I/O DAG.
const IO_EVENT_AUTHOR_SEED: [u8; 32] = [0x69; 32];

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn serial() -> u64 {
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn limits() -> OutcomeLimits {
    OutcomeLimits::default()
}

fn identity() -> SigningIdentity {
    SigningIdentity::from_fixture_seed(IO_ISSUER_SEED)
}

fn hash_text_of(bytes: &[u8]) -> Blake3HashText {
    let digest = blake3::hash(bytes);
    let mut hex = String::new();
    for byte in digest.as_bytes() {
        hex.push_str(&format!("{byte:02x}"));
    }
    Blake3HashText::parse(&format!("blake3_{hex}")).expect("hash text is valid")
}

/// Fresh scratch path unique per call.
fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "oc01-io-{name}-{}-{}.json",
        std::process::id(),
        serial()
    ))
}

fn mechanism_named(identity_text: &str) -> MechanismRecordV1 {
    MechanismRecordV1::new(
        identity_text.to_owned(),
        "1.0.0".to_owned(),
        hash_text_of(b"oc01-io-mechanism"),
        &limits(),
    )
    .expect("io mechanism is valid")
}

fn cost_unavailable(reason: &str) -> CostValueV1 {
    CostValueV1::new(
        CostValueV1::Unavailable {
            reason: reason.to_owned(),
            provenance: mechanism_named("caller.example"),
        },
        &limits(),
    )
    .expect("io cost is valid")
}

fn cost_ledger_unavailable() -> CostLedgerV1 {
    CostLedgerV1::new(
        CostLedgerV1 {
            wall_clock_ms: cost_unavailable("wall clock not exposed"),
            tool_calls: cost_unavailable("call metering absent"),
            retries: cost_unavailable("retry metering absent"),
            input_tokens: cost_unavailable("input token metering absent"),
            output_tokens: cost_unavailable("output token metering absent"),
        },
        &limits(),
    )
    .expect("io cost ledger is valid")
}

/// A minimal admitted DAG (genesis plus one descendant) with exactly one
/// local and one remote ref, plus its database path for immutability checks.
struct IoDag {
    store: Store,
    context: contextmesh::model::ContextId,
    event: contextmesh::model::EventId,
    db_path: PathBuf,
}

async fn io_dag() -> IoDag {
    let db_path = std::env::temp_dir().join(format!(
        "oc01-io-dag-{}-{}.db",
        std::process::id(),
        serial()
    ));
    let _ = std::fs::remove_file(&db_path);
    let store = Store::open(&db_path).await.expect("io store opens");

    let author = SigningIdentity::from_fixture_seed(IO_EVENT_AUTHOR_SEED);
    let context = contextmesh::model::ContextId::from_bytes([0x35; 32]);
    let genesis = author
        .create_event(
            context,
            vec![],
            "context.genesis",
            json!({"fixture": "oc01-io"}),
        )
        .expect("io genesis constructs");
    store
        .provision_context(ContextProvision {
            context,
            expected_genesis: genesis.event_id(),
            authorized_authors: vec![author.author()],
        })
        .await
        .expect("io context provisions");
    store
        .admit(
            &genesis,
            RefMutation::CompareAndSwap {
                context,
                name: "main".parse::<LocalRefName>().expect("io local ref parses"),
                expected: RefExpectation::Absent,
                new_head: genesis.event_id(),
            },
        )
        .await
        .expect("io genesis admits");

    let event = author
        .create_event(
            context,
            vec![genesis.event_id()],
            "agent.request",
            json!({"fixture": "oc01-io", "ordinal": 1}),
        )
        .expect("io event constructs");
    store
        .admit(
            &event,
            RefMutation::CompareAndSwap {
                context,
                name: "main".parse::<LocalRefName>().expect("io local ref parses"),
                expected: RefExpectation::Head(genesis.event_id()),
                new_head: event.event_id(),
            },
        )
        .await
        .expect("io event admits");
    store
        .set_remote_ref(
            "peer.example".parse::<PeerName>().expect("io peer parses"),
            context,
            "main"
                .parse::<LocalRefName>()
                .expect("io remote ref parses"),
            event.event_id(),
        )
        .await
        .expect("io remote ref installs");

    IoDag {
        store,
        context,
        event: event.event_id(),
        db_path,
    }
}

/// Issues a small valid ledger against the I/O DAG.
async fn issue_ledger(dag: &IoDag) -> SignedOutcomeLedgerV1 {
    let snapshot = InputRefSnapshotV1::capture(&dag.store, dag.context, limits())
        .await
        .expect("io snapshot captures");
    let issuer = identity();
    let event = dag.event;
    let attempt = AttemptV1::new(
        AttemptV1 {
            attempt_id: "attempt1_000000".to_owned(),
            parent_attempt_id: None,
            status: AttemptStatus::Failed,
            operation_fingerprint: hash_text_of(b"oc01-io-attempt"),
            event_refs: vec![event],
            error: AttemptErrorV1::Unavailable {
                reason: "io detail not captured".to_owned(),
            },
            costs: cost_ledger_unavailable(),
            provenance: mechanism_named("caller.example"),
        },
        &limits(),
    )
    .expect("io attempt is valid");
    let body = OutcomeLedgerBodyV1::new(
        dag.context,
        snapshot,
        TaskBindingV1::new(hash_text_of(b"oc01-io-task"), None, None, &limits())
            .expect("io task binds"),
        TerminalV1::Event { event },
        OutcomeRecordV1::new(
            OutcomeValue::Succeeded,
            vec![event],
            mechanism_named("caller.example"),
            &limits(),
        )
        .expect("io outcome is valid"),
        QualityV1::new(
            QualityV1::Unavailable {
                reason: "io rubric absent".to_owned(),
                provenance: mechanism_named("caller.example"),
            },
            &limits(),
        )
        .expect("io quality is valid"),
        cost_ledger_unavailable(),
        vec![attempt],
        vec![],
        vec![],
        vec![],
        TimestampText::parse("2026-08-25T00:00:00Z").expect("io timestamp parses"),
        issuer.author(),
        limits(),
    )
    .expect("io body is valid");
    SignedOutcomeLedgerV1::issue(&issuer, &dag.store, body, limits())
        .await
        .expect("io ledger issues")
}

/// Snapshots the bytes of the store database and any sidecar files.
fn store_file_snapshot(db_path: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let prefix = db_path.to_string_lossy().to_string();
    let mut names: Vec<String> = std::fs::read_dir(db_path.parent().expect("db parent"))
        .expect("db dir lists")
        .filter_map(|entry| {
            let name = entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .to_string();
            name.starts_with(&prefix).then_some(name)
        })
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let bytes = std::fs::read(db_path.with_file_name(&name)).unwrap_or_default();
            (name, bytes)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// OC01-X01
// ---------------------------------------------------------------------------

/// OC01-X01: export re-verifies, emits exact JCS to a newly created regular
/// file, refuses an existing destination, syncs, and never writes Option A
/// storage.
#[tokio::test]
async fn export_is_create_new_canonical_synced_and_store_independent() {
    let dag = io_dag().await;
    let ledger = issue_ledger(&dag).await;
    let out = scratch("x01");

    // Option A database (and any sidecar) baseline before export.
    let db_before = store_file_snapshot(&dag.db_path);

    // Happy path: canonical bytes in a new regular file.
    export_outcome(&ledger, &out, limits()).expect("export succeeds");
    assert_eq!(
        std::fs::read(&out).expect("exported bytes read"),
        ledger.to_wire(limits()).expect("wire renders"),
        "exported bytes must equal to_wire exactly"
    );
    let meta = std::fs::symlink_metadata(&out).expect("exported metadata");
    assert!(meta.is_file(), "export creates a regular file");

    // Refuse: an existing destination rejects without truncation.
    let second = export_outcome(&ledger, &out, limits());
    assert!(
        matches!(
            &second,
            Err(OutcomeOperationError::Io(error))
                if error.kind() == std::io::ErrorKind::AlreadyExists
        ),
        "existing destination must reject with Io(AlreadyExists), got {second:?}"
    );
    assert_eq!(
        std::fs::read(&out).expect("original bytes intact"),
        ledger.to_wire(limits()).expect("wire renders"),
        "a refused second export must not truncate the original"
    );

    // The Option A database files are byte-identical after export: export
    // never writes Option A storage.
    let db_after = store_file_snapshot(&dag.db_path);
    assert_eq!(
        db_before, db_after,
        "export must not write or checkpoint Option A storage"
    );

    let _ = std::fs::remove_file(&out);
}

// ---------------------------------------------------------------------------
// OC01-X02
// ---------------------------------------------------------------------------

/// OC01-X02: export failure removes only its partial new file and returns no
/// success artifact. Hostile destinations (read-only parent directory,
/// directory path) fail with `Io` and leave no destination file, and
/// unrelated files are preserved. The write/sync failure cleanup branch of
/// the production export path is executed under injected faults by the unit
/// tests inside `src/io.rs`.
#[tokio::test]
async fn export_failure_removes_partial_new_file() {
    let dag = io_dag().await;
    let ledger = issue_ledger(&dag).await;

    // Hostile destination 1: a path inside a read-only directory. The OS
    // refuses the create-new open itself, so no destination can appear.
    let readonly_dir = scratch("x02-dir");
    let readonly_dir = readonly_dir.with_file_name(format!(
        "{}-d",
        readonly_dir.file_name().expect("name").to_string_lossy()
    ));
    std::fs::create_dir_all(&readonly_dir).expect("dir creates");
    let dest = readonly_dir.join("outcome.json");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&readonly_dir, std::fs::Permissions::from_mode(0o555))
            .expect("read-only mode sets");
    }

    // Hostile destination 2: a directory path (EISDIR at open time).
    let dir_dest = scratch("x02-isdir");
    std::fs::create_dir_all(&dir_dest).expect("dir creates");

    // An unrelated file that must survive every failure path.
    let unrelated = scratch("x02-unrelated");
    std::fs::write(&unrelated, b"unrelated bytes").expect("unrelated writes");

    let first = export_outcome(&ledger, &dest, limits());
    assert!(
        matches!(first, Err(OutcomeOperationError::Io(_))),
        "read-only parent must fail with Io, got {first:?}"
    );
    assert!(
        !dest.exists(),
        "a failed export must not leave its destination file"
    );

    let second = export_outcome(&ledger, &dir_dest, limits());
    assert!(
        matches!(second, Err(OutcomeOperationError::Io(_))),
        "directory destination must fail with Io, got {second:?}"
    );

    assert_eq!(
        std::fs::read(&unrelated).expect("unrelated intact"),
        b"unrelated bytes",
        "failures must not touch unrelated files"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&readonly_dir, std::fs::Permissions::from_mode(0o755))
            .expect("permissions restore");
    }
    let _ = std::fs::remove_file(&unrelated);
    let _ = std::fs::remove_dir_all(&readonly_dir);
    let _ = std::fs::remove_dir_all(&dir_dest);
}

// ---------------------------------------------------------------------------
// OC01-X03
// ---------------------------------------------------------------------------

/// OC01-X03: import accepts only regular non-symlink files, reads at most
/// max+1, and never repairs/sorts/rewrites input.
#[tokio::test]
async fn import_is_bounded_regular_file_only_and_never_repairs() {
    let dag = io_dag().await;
    let ledger = issue_ledger(&dag).await;
    let wire = ledger.to_wire(limits()).expect("wire renders");
    let good = scratch("x03-good");
    std::fs::write(&good, &wire).expect("good file writes");

    // A valid regular file imports to the identical ledger.
    let imported = import_outcome(&good, limits()).expect("import succeeds");
    assert_eq!(imported, ledger);
    assert_eq!(
        std::fs::read(&good).expect("input unchanged"),
        wire,
        "import must not rewrite its input"
    );

    // A symlink to the same regular file rejects.
    let link = scratch("x03-link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&good, &link).expect("symlink creates");
    let via_link = import_outcome(&link, limits());
    assert!(
        matches!(via_link, Err(OutcomeOperationError::Io(_))),
        "symlink import must reject with Io, got {via_link:?}"
    );

    // A directory rejects as non-regular.
    let dir = scratch("x03-dir");
    std::fs::create_dir_all(&dir).expect("dir creates");
    let via_dir = import_outcome(&dir, limits());
    assert!(
        matches!(via_dir, Err(OutcomeOperationError::Io(_))),
        "directory import must reject with Io, got {via_dir:?}"
    );

    // A missing path rejects with Io.
    let absent = scratch("x03-absent");
    let missing = import_outcome(&absent, limits());
    assert!(
        matches!(missing, Err(OutcomeOperationError::Io(_))),
        "missing path must reject with Io, got {missing:?}"
    );

    // A file one byte over the caller bound (max+1 read) rejects as
    // limit-exceeded.
    let mut oversized = wire.clone();
    oversized.push(b' ');
    let big = scratch("x03-big");
    std::fs::write(&big, &oversized).expect("oversized writes");
    let tiny = OutcomeLimits {
        max_wire_bytes: wire.len(),
        ..limits()
    };
    let excess = import_outcome(&big, tiny);
    assert!(
        matches!(
            excess,
            Err(OutcomeOperationError::Artifact(OutcomeError::LimitExceeded))
        ),
        "excess read must reject as LimitExceeded, got {excess:?}"
    );
    // The bound, not the file, caused that rejection: the identical valid
    // bytes import fine under the default bound, and the same valid file
    // under a one-byte-shorter bound rejects as limit-exceeded.
    import_outcome(&good, limits()).expect("valid file is within the default bound");
    let tighter = OutcomeLimits {
        max_wire_bytes: wire.len() - 1,
        ..limits()
    };
    let bounded = import_outcome(&good, tighter);
    assert!(
        matches!(
            bounded,
            Err(OutcomeOperationError::Artifact(OutcomeError::LimitExceeded))
        ),
        "a one-byte-shorter bound must reject the same valid file, got {bounded:?}"
    );

    // A semantically equivalent but noncanonical file rejects without being
    // repaired, sorted, or rewritten.
    let value: serde_json::Value = serde_json::from_slice(&wire).expect("wire parses");
    let pretty = serde_json::to_vec_pretty(&value).expect("pretty renders");
    let noncanon = scratch("x03-noncanon");
    std::fs::write(&noncanon, &pretty).expect("noncanonical writes");
    let rejected = import_outcome(&noncanon, limits());
    assert!(
        matches!(
            rejected,
            Err(OutcomeOperationError::Artifact(OutcomeError::Noncanonical))
        ),
        "noncanonical import must reject without repair, got {rejected:?}"
    );
    assert_eq!(
        std::fs::read(&noncanon).expect("noncanonical input unchanged"),
        pretty,
        "import must never rewrite its input"
    );

    let _ = std::fs::remove_file(&good);
    let _ = std::fs::remove_file(&link);
    let _ = std::fs::remove_file(&big);
    let _ = std::fs::remove_file(&noncanon);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// OC01-X04
// ---------------------------------------------------------------------------

/// OC01-X04: verified import additionally requires full DAG and current
/// snapshot before return.
#[tokio::test]
async fn verified_import_requires_dag_and_current_inputs() {
    let dag = io_dag().await;
    let ledger = issue_ledger(&dag).await;
    let path = scratch("x04");
    export_outcome(&ledger, &path, limits()).expect("export succeeds");

    // Happy path: the full DAG and current snapshot verify.
    let verified = import_outcome_verified(&path, &dag.store, limits())
        .await
        .expect("verified import succeeds");
    assert_eq!(verified, ledger);

    // A foreign empty store has none of the referenced events.
    let foreign_path = std::env::temp_dir().join(format!(
        "oc01-io-foreign-{}-{}.db",
        std::process::id(),
        serial()
    ));
    let _ = std::fs::remove_file(&foreign_path);
    let empty = Store::open(&foreign_path).await.expect("foreign opens");
    let missing = import_outcome_verified(&path, &empty, limits()).await;
    assert!(
        matches!(
            missing,
            Err(OutcomeOperationError::Artifact(OutcomeError::MissingEvent))
        ),
        "foreign store must yield MissingEvent, got {missing:?}"
    );
    let _ = std::fs::remove_file(&foreign_path);

    // Moving the local ref forward makes the embedded snapshot stale.
    let author = SigningIdentity::from_fixture_seed(IO_EVENT_AUTHOR_SEED);
    let next = author
        .create_event(
            dag.context,
            vec![dag.event],
            "agent.request",
            json!({"fixture": "oc01-io", "ordinal": 2}),
        )
        .expect("next event constructs");
    dag.store
        .admit(
            &next,
            RefMutation::CompareAndSwap {
                context: dag.context,
                name: "main".parse::<LocalRefName>().expect("ref parses"),
                expected: RefExpectation::Head(dag.event),
                new_head: next.event_id(),
            },
        )
        .await
        .expect("ref moves");
    let stale = import_outcome_verified(&path, &dag.store, limits()).await;
    assert!(
        matches!(
            stale,
            Err(OutcomeOperationError::Artifact(OutcomeError::StaleInput))
        ),
        "a moved ref must yield StaleInput, got {stale:?}"
    );

    let _ = std::fs::remove_file(&path);
}

// ---------------------------------------------------------------------------
// OC01-X20
// ---------------------------------------------------------------------------

/// OC01-X20: Artifact, Store, and file failures use
/// `OutcomeOperationError::{Artifact,Store,Io}` while the wire categories
/// remain exactly twelve. Mapping and the source chain are total, wrapper
/// Display/custom Debug output is generic and non-secret, and traversed
/// I/O source text is not surfaced by any wrapper output.
#[tokio::test]
async fn operation_error_wrapper_preserves_artifact_store_and_io_causes() {
    // The twelve wire categories map to Artifact with exact stable text.
    let categories = [
        OutcomeError::Malformed,
        OutcomeError::Noncanonical,
        OutcomeError::UnsupportedVersion,
        OutcomeError::LimitExceeded,
        OutcomeError::IdMismatch,
        OutcomeError::SignatureInvalid,
        OutcomeError::MissingEvent,
        OutcomeError::UnauthorizedEvent,
        OutcomeError::ContextMismatch,
        OutcomeError::StaleInput,
        OutcomeError::MechanismUnavailable,
        OutcomeError::IncompleteInput,
    ];
    let expected = [
        "malformed",
        "noncanonical",
        "unsupported-version",
        "limit-exceeded",
        "id-mismatch",
        "signature-invalid",
        "missing-event",
        "unauthorized-event",
        "context-mismatch",
        "stale-input",
        "mechanism-unavailable",
        "incomplete-input",
    ];
    assert_eq!(categories.len(), 12, "exactly twelve wire categories");
    for (error, text) in categories.iter().zip(expected) {
        let wrapped = OutcomeOperationError::from(*error);
        assert!(
            matches!(wrapped, OutcomeOperationError::Artifact(_)),
            "{text} must wrap as Artifact"
        );
        assert_eq!(wrapped.to_string(), "outcome artifact operation failed");
        assert_eq!(format!("{wrapped:?}"), "OutcomeOperationError::Artifact");
        let source = std::error::Error::source(&wrapped)
            .and_then(|source| source.downcast_ref::<OutcomeError>())
            .expect("artifact source retains the typed cause");
        assert_eq!(source.stable_category(), text);
    }

    // A real Store operational failure wraps as Store, generically.
    let bad = std::env::temp_dir().join(format!(
        "oc01-io-x20-corrupt-{}-{}.db",
        std::process::id(),
        serial()
    ));
    std::fs::write(&bad, b"not a sqlite database at all").expect("corrupt db writes");
    let store_error = match Store::open(&bad).await {
        Ok(_) => panic!("a corrupt database must fail to open"),
        Err(error) => error,
    };
    let store_wrapped = OutcomeOperationError::from(store_error);
    assert!(matches!(store_wrapped, OutcomeOperationError::Store(_)));
    assert_eq!(store_wrapped.to_string(), "outcome store operation failed");
    assert_eq!(format!("{store_wrapped:?}"), "OutcomeOperationError::Store");
    let _ = std::fs::remove_file(&bad);

    // Real I/O operations surface their causes through the wrapper: a
    // missing import path yields Io, and garbage yields Artifact(Malformed).
    let absent = scratch("x20-absent");
    let io_failure = import_outcome(&absent, limits()).expect_err("missing path fails");
    assert!(matches!(io_failure, OutcomeOperationError::Io(_)));
    let garbage = scratch("x20-garbage");
    std::fs::write(&garbage, b"not json at all").expect("garbage writes");
    let artifact_failure = import_outcome(&garbage, limits()).expect_err("garbage fails");
    assert!(
        matches!(
            artifact_failure,
            OutcomeOperationError::Artifact(OutcomeError::Malformed)
        ),
        "garbage import must be Artifact(Malformed), got {artifact_failure:?}"
    );

    // Arbitrary I/O source text is retained for programmatic inspection but
    // never surfaced by Display or Debug: the canary must not leak.
    let canary = "secret path /home/cosmo/private token sk-live";
    let io_canary = OutcomeOperationError::from(std::io::Error::other(canary));
    assert_eq!(io_canary.to_string(), "outcome file operation failed");
    assert_eq!(format!("{io_canary:?}"), "OutcomeOperationError::Io");
    let retained = std::error::Error::source(&io_canary)
        .and_then(|source| source.downcast_ref::<std::io::Error>())
        .expect("io source retains the typed cause");
    assert!(
        retained.to_string().contains("secret path"),
        "the typed cause remains inspectable programmatically"
    );
    assert!(
        !io_canary.to_string().contains("secret") && !format!("{io_canary:?}").contains("secret"),
        "wrapper output must never disclose the traversed I/O source text"
    );

    let _ = std::fs::remove_file(&garbage);
}
