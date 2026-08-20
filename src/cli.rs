//! Stable automation CLI: parsing, dispatch, canonical JSON, and exit classes.

use std::ffi::OsString;
use std::io::{Read as _, Write as _};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde_json::{Value, json};

use crate::crypto::SigningIdentity;
use crate::error::{KeyFileError, ProviderError, StoreError, SyncError};
use crate::model::{AuthorId, ContextId, EventId};
use crate::provider::{CommandProvider, InvocationRequest, record_invocation};
use crate::store::{
    BundleLimits, ContextProvision, LocalRefName, ProjectionLimits, Store, VerificationLimits,
};
use crate::sync::{PullClient, PullClientConfig, PullLimits};

/// CLI JSON schema version.
pub const CLI_SCHEMA_VERSION: u8 = 1;
/// Maximum payload/input bytes accepted from files or stdin.
pub const CLI_PAYLOAD_LIMIT: usize = crate::provider::PROVIDER_INPUT_LIMIT;

#[derive(Debug)]
struct CliError {
    code: &'static str,
    exit: u8,
    details: Value,
}

impl CliError {
    fn new(code: &'static str, exit: u8, details: Value) -> Self {
        Self {
            code,
            exit,
            details,
        }
    }
}

fn usage(details: Value) -> CliError {
    CliError::new("usage", 2, details)
}
fn validation(details: Value) -> CliError {
    CliError::new("validation", 3, details)
}
fn conflict(details: Value) -> CliError {
    CliError::new("conflict", 4, details)
}
fn auth(details: Value) -> CliError {
    CliError::new("authentication", 5, details)
}
fn not_found(details: Value) -> CliError {
    CliError::new("not_found", 6, details)
}
fn provider_failure(details: Value) -> CliError {
    CliError::new("provider", 7, details)
}
fn transport(details: Value) -> CliError {
    CliError::new("transport", 8, details)
}
fn internal(details: Value) -> CliError {
    CliError::new("internal", 9, details)
}

fn store_error(error: StoreError) -> CliError {
    match error {
        StoreError::ContextUnknown => not_found(json!({"subject": "context"})),
        StoreError::RefMissing | StoreError::StaleHead { .. } => conflict(json!({
            "store_error": "conflict"
        })),
        StoreError::ParentMissing(_) | StoreError::ParentContextMismatch(_) => {
            validation(json!({"store_error": "parent"}))
        }
        StoreError::DatabaseUnavailable | StoreError::IndeterminateCommit => {
            internal(json!({"store_error": "database"}))
        }
        other => internal(json!({"store_error": format!("{other:?}")})),
    }
}

fn key_error(error: KeyFileError) -> CliError {
    let _ = error;
    auth(json!({"key_file": "unavailable"}))
}

fn provider_error(error: ProviderError) -> CliError {
    match error {
        ProviderError::PostExecutionConflict {
            result,
            current_head,
        } => provider_failure(json!({
            "current_head": current_head.map(|id| id.to_string()),
            "kind": "post_execution_conflict",
            "result": result.to_string(),
        })),
        ProviderError::Store(StoreError::StaleHead { .. })
        | ProviderError::Store(StoreError::RefMissing) => {
            conflict(json!({"store_error": "conflict"}))
        }
        ProviderError::Store(StoreError::ContextUnknown) => {
            not_found(json!({"subject": "context"}))
        }
        ProviderError::Validation | ProviderError::LimitExceeded => {
            validation(json!({"provider_error": "validation"}))
        }
        ProviderError::InvalidConfig => usage(json!({"provider_error": "config"})),
        other => provider_failure(json!({"provider_error": format!("{other:?}")})),
    }
}

fn sync_error(error: SyncError) -> CliError {
    let _ = error;
    transport(json!({"sync": "failed"}))
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Key custody operations.
    Key {
        #[command(subcommand)]
        sub: KeySub,
    },
    /// Bearer-token generation.
    Token {
        #[command(subcommand)]
        sub: TokenSub,
    },
    /// Context lifecycle operations.
    Context {
        #[command(subcommand)]
        sub: ContextSub,
    },
    /// Signs and CAS-appends one single-parent event.
    Append {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        key_file: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long)]
        branch: String,
        #[arg(long)]
        expected_head: String,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        payload_file: Option<PathBuf>,
        #[arg(long, default_value = "false")]
        payload_stdin: bool,
    },
    /// Branch operations.
    Branch {
        #[command(subcommand)]
        sub: BranchSub,
    },
    /// Signs an explicit multi-parent merge and CAS-moves the branch.
    Merge {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        key_file: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long)]
        branch: String,
        #[arg(long)]
        expected_head: String,
        #[arg(long = "parent")]
        parents: Vec<String>,
        #[arg(long)]
        payload_file: Option<PathBuf>,
        #[arg(long, default_value = "false")]
        payload_stdin: bool,
    },
    /// Read-only inspection commands.
    Show {
        #[command(subcommand)]
        sub: ShowSub,
    },
    /// Bundle transfer commands.
    Bundle {
        #[command(subcommand)]
        sub: BundleSub,
    },
    /// Lists pending requests or detached results.
    Invocation {
        #[command(subcommand)]
        query: InvocationQuery,
    },
    /// Verifies the complete store without repair.
    Verify {
        #[arg(long)]
        db: PathBuf,
    },
    /// Records one provider invocation end to end.
    Invoke {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        key_file: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long)]
        branch: String,
        #[arg(long)]
        expected_head: String,
        #[arg(long)]
        input_file: Option<PathBuf>,
        #[arg(long, default_value = "false")]
        input_stdin: bool,
        #[arg(long)]
        provider_command: PathBuf,
        #[arg(long = "provider-arg")]
        provider_args: Vec<OsString>,
    },
    /// Serves authenticated pull synchronization until SIGINT/SIGTERM.
    Serve {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        token_file: PathBuf,
        #[arg(long)]
        listen: String,
        #[arg(long)]
        ready_file: PathBuf,
        #[arg(long, default_value = "false")]
        acknowledge_non_loopback_plaintext: bool,
    },
    /// Pulls immutable history from one peer.
    Sync {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        peer: String,
        #[arg(long)]
        url: String,
        #[arg(long)]
        token_file: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long, default_value = "false")]
        acknowledge_non_loopback_plaintext: bool,
    },
    /// Option B receipt operations.
    ObReceipt {
        #[command(subcommand)]
        sub: ObReceiptSub,
    },
}

#[derive(Debug, Subcommand)]
enum ObReceiptSub {
    /// Issues a signed Option B agent-experience receipt. `--task` is the
    /// task verbatim; prefix `@` to read a bounded file, or `-` for stdin.
    Issue {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        key_file: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long = "event")]
        events: Vec<String>,
        #[arg(long)]
        task: String,
        #[arg(long)]
        recipient_head: String,
        #[arg(long)]
        selector: String,
        #[arg(long)]
        selector_version: String,
        #[arg(long)]
        config_hash: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Verifies a receipt's signature and its references against the DAG.
    Verify {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        file: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum KeySub {
    /// Generates an atomic 0600 Ed25519 seed file.
    Generate {
        #[arg(long)]
        file: PathBuf,
    },
    /// Explicitly repairs insecure key/token file permissions.
    RepairPermissions {
        #[arg(long)]
        file: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum TokenSub {
    /// Generates an atomic 0600 OA-04 token1_ bearer-token file.
    Generate {
        #[arg(long)]
        file: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum ContextSub {
    /// Creates, provisions, activates, and branches a new context.
    Create {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        key_file: PathBuf,
        #[arg(long, default_value = "main")]
        branch: String,
    },
    /// Provisions or idempotently rejoins a pending context.
    Join {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long)]
        expected_genesis: String,
        #[arg(long = "author")]
        authors: Vec<String>,
    },
    /// Appends one author to a context's allowlist.
    Authorize {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long)]
        author: String,
    },
}

#[derive(Debug, Subcommand)]
enum BranchSub {
    /// Creates an absent local branch at an existing event.
    Create {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        from_head: String,
    },
}

#[derive(Debug, Subcommand)]
enum ShowSub {
    /// Shows one stored event after strict verification.
    Event {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        id: String,
    },
    /// Shows the deterministic projection for explicit heads.
    Projection {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long = "head")]
        heads: Vec<String>,
    },
    /// Shows local or peer remote refs.
    Refs {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long)]
        peer: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum BundleSub {
    /// Exports a deterministic bounded Bundle v1.
    Export {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long = "head")]
        heads: Vec<String>,
        #[arg(long = "known-head")]
        known_heads: Vec<String>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Imports one bundle into a peer namespace.
    Import {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        peer: String,
        #[arg(long)]
        file: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum InvocationQuery {
    /// Requests on the branch with no linked result.
    Pending {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long)]
        branch: String,
    },
    /// Results unreachable from the branch head.
    Detached {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long)]
        branch: String,
    },
}

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

fn command_name(command: &Commands) -> &'static str {
    match command {
        Commands::Key { sub } => match sub {
            KeySub::Generate { .. } => "key generate",
            KeySub::RepairPermissions { .. } => "key repair-permissions",
        },
        Commands::Token {
            sub: TokenSub::Generate { .. },
        } => "token generate",
        Commands::Context { sub } => match sub {
            ContextSub::Create { .. } => "context create",
            ContextSub::Join { .. } => "context join",
            ContextSub::Authorize { .. } => "context authorize",
        },
        Commands::Append { .. } => "append",
        Commands::Branch {
            sub: BranchSub::Create { .. },
        } => "branch create",
        Commands::Merge { .. } => "merge",
        Commands::Show { sub } => match sub {
            ShowSub::Event { .. } => "show event",
            ShowSub::Projection { .. } => "show projection",
            ShowSub::Refs { .. } => "show refs",
        },
        Commands::Bundle { sub } => match sub {
            BundleSub::Export { .. } => "bundle export",
            BundleSub::Import { .. } => "bundle import",
        },
        Commands::Invocation { query } => match query {
            InvocationQuery::Pending { .. } => "invocation pending",
            InvocationQuery::Detached { .. } => "invocation detached",
        },
        Commands::Verify { .. } => "verify",
        Commands::Invoke { .. } => "invoke",
        Commands::Serve { .. } => "serve",
        Commands::Sync { .. } => "sync",
        Commands::ObReceipt { sub } => match sub {
            ObReceiptSub::Issue { .. } => "ob-receipt issue",
            ObReceiptSub::Verify { .. } => "ob-receipt verify",
        },
    }
}

fn parse_context(text: &str) -> Result<ContextId, CliError> {
    text.parse()
        .map_err(|_| validation(json!({"field": "context", "reason": "invalid canonical id"})))
}

fn parse_event(text: &str, field: &str) -> Result<EventId, CliError> {
    text.parse()
        .map_err(|_| validation(json!({"field": field, "reason": "invalid canonical id"})))
}

fn parse_author(text: &str) -> Result<AuthorId, CliError> {
    text.parse()
        .map_err(|_| validation(json!({"field": "author", "reason": "invalid canonical id"})))
}

fn parse_branch(text: &str) -> Result<LocalRefName, CliError> {
    text.parse()
        .map_err(|_| validation(json!({"field": "branch", "reason": "invalid canonical name"})))
}

fn read_bounded(source: &str, file: Option<&PathBuf>, stdin: bool) -> Result<Vec<u8>, CliError> {
    if stdin {
        let mut buffer = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buffer)
            .map_err(|_| usage(json!({"field": source, "reason": "stdin unreadable"})))?;
        if buffer.len() > CLI_PAYLOAD_LIMIT {
            return Err(validation(json!({"field": source, "reason": "over limit"})));
        }
        return Ok(buffer);
    }
    let path = file.ok_or_else(|| usage(json!({"field": source, "reason": "input required"})))?;
    let mut buffer = Vec::new();
    std::fs::File::open(path)
        .and_then(|mut handle| handle.read_to_end(&mut buffer))
        .map_err(|_| usage(json!({"field": source, "reason": "file unreadable"})))?;
    if buffer.len() > CLI_PAYLOAD_LIMIT {
        return Err(validation(json!({"field": source, "reason": "over limit"})));
    }
    Ok(buffer)
}

fn parse_payload(bytes: &[u8]) -> Result<Value, CliError> {
    serde_json::from_slice(bytes)
        .map_err(|_| validation(json!({"field": "payload", "reason": "invalid JSON"})))
}

async fn open_store(path: &std::path::Path) -> Result<Store, CliError> {
    Store::open(path).await.map_err(store_error)
}

fn load_identity(path: &std::path::Path) -> Result<SigningIdentity, CliError> {
    SigningIdentity::load_key_file(path).map_err(key_error)
}

async fn dispatch(command: Commands) -> Result<Value, CliError> {
    match command {
        Commands::Key {
            sub: KeySub::Generate { file },
        } => Ok(json!({
            "author": SigningIdentity::generate_key_file(&file).map_err(key_error)?.to_string()
        })),
        Commands::Token {
            sub: TokenSub::Generate { file },
        } => {
            SigningIdentity::generate_token_file(&file).map_err(key_error)?;
            Ok(json!({}))
        }
        Commands::Key {
            sub: KeySub::RepairPermissions { file },
        } => {
            SigningIdentity::repair_key_file_permissions(&file).map_err(key_error)?;
            Ok(json!({}))
        }
        Commands::Context {
            sub:
                ContextSub::Create {
                    db,
                    key_file,
                    branch,
                },
        } => {
            let store = open_store(&db).await?;
            let identity = load_identity(&key_file)?;
            let branch = parse_branch(&branch)?;
            let created = store
                .create_context(&identity, branch)
                .await
                .map_err(store_error)?;
            Ok(json!({
                "branch": created.branch.name.as_str(),
                "context": created.context.to_string(),
                "genesis": created.branch.head.to_string()
            }))
        }
        Commands::Context {
            sub:
                ContextSub::Join {
                    db,
                    context,
                    expected_genesis,
                    authors,
                },
        } => {
            let store = open_store(&db).await?;
            let mut sorted = authors.clone();
            sorted.sort();
            sorted.dedup();
            if sorted.len() != authors.len() {
                return Err(validation(
                    json!({"field": "author", "reason": "duplicate"}),
                ));
            }
            let mut parsed = Vec::with_capacity(sorted.len());
            for author in &sorted {
                parsed.push(parse_author(author)?);
            }
            store
                .join_context(ContextProvision {
                    context: parse_context(&context)?,
                    expected_genesis: parse_event(&expected_genesis, "expected-genesis")?,
                    authorized_authors: parsed,
                })
                .await
                .map_err(store_error)?;
            Ok(json!({}))
        }
        Commands::Context {
            sub:
                ContextSub::Authorize {
                    db,
                    context,
                    author,
                },
        } => {
            let store = open_store(&db).await?;
            store
                .authorize_author(parse_context(&context)?, parse_author(&author)?)
                .await
                .map_err(store_error)?;
            Ok(json!({}))
        }
        Commands::Append {
            db,
            key_file,
            context,
            branch,
            expected_head,
            kind,
            payload_file,
            payload_stdin,
        } => {
            let store = open_store(&db).await?;
            let identity = load_identity(&key_file)?;
            let payload = parse_payload(&read_bounded(
                "payload",
                payload_file.as_ref(),
                payload_stdin,
            )?)?;
            let event = store
                .append(
                    &identity,
                    parse_context(&context)?,
                    parse_branch(&branch)?,
                    parse_event(&expected_head, "expected-head")?,
                    kind,
                    payload,
                )
                .await
                .map_err(store_error)?;
            Ok(json!({"event": event.event_id().to_string()}))
        }
        Commands::Branch {
            sub:
                BranchSub::Create {
                    db,
                    context,
                    name,
                    from_head,
                },
        } => {
            let store = open_store(&db).await?;
            let branch = store
                .create_branch(
                    parse_context(&context)?,
                    parse_branch(&name)?,
                    parse_event(&from_head, "from-head")?,
                )
                .await
                .map_err(store_error)?;
            Ok(json!({"branch": branch.name.as_str()}))
        }
        Commands::Merge {
            db,
            key_file,
            context,
            branch,
            expected_head,
            parents,
            payload_file,
            payload_stdin,
        } => {
            let store = open_store(&db).await?;
            let identity = load_identity(&key_file)?;
            let payload = parse_payload(&read_bounded(
                "payload",
                payload_file.as_ref(),
                payload_stdin,
            )?)?;
            let mut parsed = Vec::with_capacity(parents.len());
            for parent in &parents {
                parsed.push(parse_event(parent, "parent")?);
            }
            let event = store
                .merge(
                    &identity,
                    parse_context(&context)?,
                    parse_branch(&branch)?,
                    parse_event(&expected_head, "expected-head")?,
                    parsed,
                    payload,
                )
                .await
                .map_err(store_error)?;
            Ok(json!({"event": event.event_id().to_string()}))
        }
        Commands::Show {
            sub: ShowSub::Event { db, id },
        } => {
            let store = open_store(&db).await?;
            let event = store
                .event(parse_event(&id, "id")?)
                .await
                .map_err(store_error)?
                .ok_or_else(|| not_found(json!({"subject": "event"})))?;
            Ok(json!({
                "kind": event.body().kind(),
                "payload": event.body().payload().clone()
            }))
        }
        Commands::Show {
            sub: ShowSub::Projection { db, context, heads },
        } => {
            let store = open_store(&db).await?;
            let mut parsed = Vec::with_capacity(heads.len());
            for head in &heads {
                parsed.push(parse_event(head, "head")?);
            }
            let projection = store
                .project(
                    parse_context(&context)?,
                    parsed,
                    ProjectionLimits::default(),
                )
                .await
                .map_err(store_error)?;
            Ok(json!({"events": projection.events.len()}))
        }
        Commands::Show {
            sub: ShowSub::Refs { db, context, peer },
        } => {
            let store = open_store(&db).await?;
            let context = parse_context(&context)?;
            if let Some(peer) = peer {
                let peer: crate::store::PeerName = peer
                    .parse()
                    .map_err(|_| validation(json!({"field": "peer", "reason": "invalid name"})))?;
                let refs = store
                    .list_remote_refs(Some(&peer), context)
                    .await
                    .map_err(store_error)?;
                return Ok(json!({"refs": refs.len()}));
            }
            let refs = store.list_local_refs(context).await.map_err(store_error)?;
            Ok(json!({"refs": refs.len()}))
        }
        Commands::Bundle {
            sub:
                BundleSub::Export {
                    db,
                    context,
                    heads,
                    known_heads,
                    out,
                },
        } => {
            let store = open_store(&db).await?;
            let context = parse_context(&context)?;
            let mut requested = Vec::with_capacity(heads.len());
            for head in &heads {
                requested.push(parse_event(head, "head")?);
            }
            let mut known = Vec::with_capacity(known_heads.len());
            for head in &known_heads {
                known.push(parse_event(head, "known-head")?);
            }
            let refs = store
                .sync_local_ref_snapshot(context)
                .await
                .map_err(store_error)?;
            let bundle = store
                .export_bundle(context, requested, known, refs, BundleLimits::default())
                .await
                .map_err(store_error)?;
            let wire = bundle
                .to_wire()
                .map_err(|_| internal(json!({"reason": "render"})))?;
            std::fs::write(&out, wire)
                .map_err(|_| usage(json!({"field": "out", "reason": "unwritable"})))?;
            Ok(json!({}))
        }
        Commands::Bundle {
            sub: BundleSub::Import { db, peer, file },
        } => {
            let store = open_store(&db).await?;
            let peer: crate::store::PeerName = peer
                .parse()
                .map_err(|_| validation(json!({"field": "peer", "reason": "invalid name"})))?;
            let wire = std::fs::read(&file)
                .map_err(|_| usage(json!({"field": "file", "reason": "unreadable"})))?;
            let report = store
                .import_bundle(peer, &wire, BundleLimits::default())
                .await
                .map_err(store_error)?;
            Ok(json!({
                "already_present": report.already_present,
                "inserted": report.inserted
            }))
        }
        Commands::Invocation { query } => {
            let (db, context, branch) = match &query {
                InvocationQuery::Pending {
                    db,
                    context,
                    branch,
                }
                | InvocationQuery::Detached {
                    db,
                    context,
                    branch,
                } => (db, context, branch),
            };
            let store = open_store(db).await?;
            let context = parse_context(context)?;
            let branch = parse_branch(branch)?;
            match query {
                InvocationQuery::Pending { .. } => {
                    let pending = store
                        .pending_invocations(context, branch)
                        .await
                        .map_err(store_error)?;
                    Ok(json!({"pending": pending.len()}))
                }
                InvocationQuery::Detached { .. } => {
                    let detached = store
                        .detached_results(context, branch)
                        .await
                        .map_err(store_error)?;
                    Ok(json!({"detached": detached.len()}))
                }
            }
        }
        Commands::Verify { db } => {
            let store = open_store(&db).await?;
            let report = store
                .verify_full(VerificationLimits::default())
                .await
                .map_err(store_error)?;
            if report.valid {
                Ok(json!({"valid": true}))
            } else {
                Err(internal(json!({"valid": false})))
            }
        }
        Commands::Invoke {
            db,
            key_file,
            context,
            branch,
            expected_head,
            input_file,
            input_stdin,
            provider_command,
            provider_args,
        } => {
            let store = open_store(&db).await?;
            let identity = load_identity(&key_file)?;
            let input = parse_payload(&read_bounded("input", input_file.as_ref(), input_stdin)?)?;
            let provider = CommandProvider::new(provider_command, provider_args);
            let report = record_invocation(
                &store,
                &identity,
                InvocationRequest {
                    context: parse_context(&context)?,
                    branch: parse_branch(&branch)?,
                    expected_head: parse_event(&expected_head, "expected-head")?,
                    input,
                    provider: &provider,
                },
            )
            .await
            .map_err(provider_error)?;
            Ok(json!({
                "invocation_id": report.invocation_id,
                "outcome": match report.outcome {
                    crate::provider::OutcomeKind::Response => "response",
                    crate::provider::OutcomeKind::RecordedError => "error",
                },
                "request": report.request_event_id.to_string(),
                "result": report.result_event_id.to_string()
            }))
        }
        Commands::Serve {
            db,
            token_file,
            listen,
            ready_file,
            acknowledge_non_loopback_plaintext,
        } => {
            let store = open_store(&db).await?;
            let bind = listen
                .parse()
                .map_err(|_| validation(json!({"field": "listen", "reason": "invalid address"})))?;
            let server = crate::http::SyncServer::bind(
                store,
                crate::http::SyncServerConfig {
                    bind,
                    acknowledge_non_loopback_plaintext,
                    token: crate::http::TokenSource::file(token_file),
                    limits: crate::http::TransportLimits::default(),
                },
            )
            .await
            .map_err(sync_error)?;
            if let Some(warning) = server.exposure_warning() {
                eprintln!("{warning}");
            }
            let address = server.local_addr();
            let temp = ready_file.with_extension("tmp");
            std::fs::write(&temp, address.to_string())
                .and_then(|_| std::fs::rename(&temp, &ready_file))
                .map_err(|_| usage(json!({"field": "ready-file", "reason": "unwritable"})))?;
            server
                .serve_until(shutdown_signal())
                .await
                .map_err(sync_error)?;
            Ok(json!({"address": address.to_string()}))
        }
        Commands::Sync {
            db,
            peer,
            url,
            token_file,
            context,
            acknowledge_non_loopback_plaintext,
        } => {
            let store = open_store(&db).await?;
            let peer: crate::store::PeerName = peer
                .parse()
                .map_err(|_| validation(json!({"field": "peer", "reason": "invalid name"})))?;
            let endpoint = crate::http::PeerEndpoint::new(&url, acknowledge_non_loopback_plaintext)
                .map_err(sync_error)?;
            let report = PullClient::new(
                store,
                PullClientConfig {
                    peer,
                    endpoint,
                    token: crate::http::TokenSource::file(token_file),
                    context: parse_context(&context)?,
                    limits: PullLimits::default(),
                    transport: crate::http::TransportLimits::default(),
                },
            )
            .map_err(sync_error)?
            .pull()
            .await
            .map_err(sync_error)?;
            Ok(json!({
                "inserted": report.inserted,
                "pages": report.pages,
                "remote_refs_updated": report.remote_refs_updated
            }))
        }
        Commands::ObReceipt {
            sub:
                ObReceiptSub::Issue {
                    db,
                    key_file,
                    context,
                    events,
                    task,
                    recipient_head,
                    selector,
                    selector_version,
                    config_hash,
                    out,
                },
        } => {
            let store = open_store(&db).await?;
            let identity = load_identity(&key_file)?;
            let context = parse_context(&context)?;
            let mut parsed_events = Vec::with_capacity(events.len());
            for event in &events {
                parsed_events.push(parse_event(event, "event")?);
            }
            parsed_events.sort();
            parsed_events.dedup();
            if parsed_events.len() != events.len() {
                return Err(validation(json!({"field": "event", "reason": "duplicate"})));
            }
            // `--task` is verbatim text, `@path` reads a bounded file, `-`
            // reads bounded stdin (documented in the subcommand help).
            let task_bytes = if let Some(path) = task.strip_prefix('@') {
                let mut buffer = Vec::new();
                std::fs::File::open(path)
                    .and_then(|mut handle| handle.read_to_end(&mut buffer))
                    .map_err(|_| usage(json!({"field": "task", "reason": "file unreadable"})))?;
                if buffer.len() > CLI_PAYLOAD_LIMIT {
                    return Err(validation(json!({"field": "task", "reason": "over limit"})));
                }
                buffer
            } else if task == "-" {
                let mut buffer = Vec::new();
                std::io::stdin()
                    .read_to_end(&mut buffer)
                    .map_err(|_| usage(json!({"field": "task", "reason": "stdin unreadable"})))?;
                if buffer.len() > CLI_PAYLOAD_LIMIT {
                    return Err(validation(json!({"field": "task", "reason": "over limit"})));
                }
                buffer
            } else {
                task.into_bytes()
            };
            let verbatim = String::from_utf8(task_bytes)
                .map_err(|_| validation(json!({"field": "task", "reason": "invalid UTF-8"})))?;
            let task = crate::receipt::TaskRecordV1::from_verbatim(verbatim, None)
                .map_err(|_| validation(json!({"field": "task", "reason": "invalid"})))?;
            let selector =
                crate::receipt::SelectorRecordV1::new(selector, selector_version, config_hash)
                    .map_err(|_| validation(json!({"field": "selector", "reason": "invalid"})))?;
            let body = crate::receipt::ReceiptBodyV1::new(
                context,
                parsed_events,
                task,
                crate::receipt::RecipientStateV1::new(parse_event(
                    &recipient_head,
                    "recipient-head",
                )?),
                selector,
                Vec::new(),
                Vec::new(),
                crate::receipt::utc_timestamp(),
                identity.author(),
            )
            .map_err(|_| validation(json!({"field": "receipt", "reason": "invalid"})))?;
            let receipt = crate::receipt::SignedReceiptV1::issue(&identity, body)
                .map_err(|_| validation(json!({"field": "receipt", "reason": "cannot sign"})))?;
            let report = receipt
                .verify_against_dag(&store)
                .await
                .map_err(store_error)?;
            if !report.valid {
                let findings: Vec<Value> = report
                    .findings
                    .iter()
                    .map(|finding| {
                        json!({"reason": finding.reason, "event": finding.event.to_string()})
                    })
                    .collect();
                return Err(conflict(
                    json!({"subject": "receipt", "findings": findings}),
                ));
            }
            let wire = receipt
                .to_wire()
                .map_err(|_| internal(json!({"reason": "render"})))?;
            std::fs::write(&out, wire)
                .map_err(|_| usage(json!({"field": "out", "reason": "unwritable"})))?;
            Ok(crate::receipt::receipt_json(&receipt))
        }
        Commands::ObReceipt {
            sub: ObReceiptSub::Verify { db, file },
        } => {
            let store = open_store(&db).await?;
            let wire = std::fs::read(&file)
                .map_err(|_| usage(json!({"field": "file", "reason": "unreadable"})))?;
            let receipt = crate::receipt::SignedReceiptV1::from_wire(&wire).map_err(|_| {
                validation(json!({"field": "receipt", "reason": "invalid or tampered"}))
            })?;
            let report = receipt
                .verify_against_dag(&store)
                .await
                .map_err(store_error)?;
            if report.valid {
                Ok(json!({
                    "receipt_id": receipt.receipt_id().to_string(),
                    "valid": true,
                    "checked_events": report.checked_events
                }))
            } else {
                let findings: Vec<Value> = report
                    .findings
                    .iter()
                    .map(|finding| {
                        json!({"reason": finding.reason, "event": finding.event.to_string()})
                    })
                    .collect();
                Err(internal(json!({"valid": false, "findings": findings})))
            }
        }
    }
}

async fn shutdown_signal() {
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("sigterm handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = sigterm.recv() => {},
    }
}

/// Runs the CLI, writing exactly one canonical JSON document to stdout.
pub async fn run(args: &[String]) -> std::process::ExitCode {
    let cli = match <Cli as clap::Parser>::try_parse_from(
        std::iter::once("contextmesh".to_owned()).chain(args.iter().cloned()),
    ) {
        Ok(cli) => cli,
        Err(_) => {
            emit_failure(
                "usage",
                &json!({"reason": "clap"}),
                std::process::ExitCode::from(2),
            );
            return std::process::ExitCode::from(2);
        }
    };
    let name = command_name(&cli.command);
    match dispatch(cli.command).await {
        Ok(result) => {
            emit_success(name, &result);
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            emit_failure(
                error.code,
                &error.details,
                std::process::ExitCode::from(error.exit),
            );
            std::process::ExitCode::from(error.exit)
        }
    }
}

fn render(value: &Value) -> Vec<u8> {
    crate::model::canonicalize(value).unwrap_or_else(|_| b"{}".to_vec())
}

fn emit_success(command: &str, result: &Value) {
    let document = json!({
        "command": command,
        "ok": true,
        "result": result,
        "schema_version": CLI_SCHEMA_VERSION
    });
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(&render(&document));
    let _ = stdout.write_all(b"\n");
    let _ = stdout.flush();
}

fn emit_failure(code: &str, details: &Value, exit: std::process::ExitCode) {
    let _ = exit;
    let document = json!({
        "command": "?",
        "error": {"code": code, "details": details},
        "ok": false,
        "schema_version": CLI_SCHEMA_VERSION
    });
    // The failure document is stable automation output on stdout; the exit
    // class distinguishes it. Only warnings belong on stderr.
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(&render(&document));
    let _ = stdout.write_all(b"\n");
    let _ = stdout.flush();
}
