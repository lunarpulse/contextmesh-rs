//! OA-05 provider recording boundary: invocation contract, bounded
//! subprocess transport, and the linked recording sequence.
//!
//! Recording makes no semantic-selection, protocol-compliance, or exactly-once
//! claim. Provider effects are external and opaque.

use std::ffi::OsString;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio as StdStdio;
use std::time::Duration;

use std::borrow::Cow;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::crypto::SigningIdentity;
use crate::error::{ProviderError, ProviderResult};
use crate::model::{ContextId, EventId, SignedEventV1, canonicalize};
use crate::store::{LocalRefName, ProjectionLimits, RefExpectation, RefMutation, Store};

/// Maximum canonical provider input bytes (the OA-01 payload limit).
pub const PROVIDER_INPUT_LIMIT: usize = 1_048_576;
/// Maximum canonical provider response bytes.
pub const PROVIDER_RESPONSE_LIMIT: usize = 1_048_576;
/// Maximum JSONL line bytes including the newline.
pub const JSONL_LINE_LIMIT: usize = 2 * 1024 * 1024;
/// Maximum ancestry events in one invocation document.
pub const MAX_INVOCATION_ANCESTRY: usize = 1_024;
/// Maximum sanitized provider failure detail bytes.
pub const ERROR_DETAIL_LIMIT: usize = 1_024;
/// Whole subprocess execution timeout; the child is killed on expiry.
pub const PROVIDER_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);

/// One recorded invocation outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderOutcome {
    /// An opaque provider response.
    Response(Value),
    /// A failure; `code` is one of the frozen codes, `detail` is sanitized.
    Failure {
        /// Frozen non-secret failure code.
        code: &'static str,
        /// Sanitized bounded detail text.
        detail: String,
    },
}

/// Kind of the recorded result event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutcomeKind {
    /// An agent.response was recorded.
    Response,
    /// An agent.error was recorded.
    RecordedError,
}

/// Exact metadata supplied to a provider for one invocation.
pub struct InvocationContext {
    /// Context whose ancestry is selected.
    pub context: ContextId,
    /// Branch head selected by the caller.
    pub selected_head: EventId,
    /// Deterministic parent-first ancestry before the request.
    pub ancestry: Vec<SignedEventV1>,
    /// Committed agent.request event.
    pub request_event_id: EventId,
    /// Random invocation correlation ID (inv1_...).
    pub invocation_id: String,
    /// Opaque caller input.
    pub input: Value,
}

impl std::fmt::Debug for InvocationContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InvocationContext")
            .field("context", &self.context)
            .field("selected_head", &self.selected_head)
            .field("ancestry_events", &self.ancestry.len())
            .field("request_event_id", &self.request_event_id)
            .field("invocation_id", &self.invocation_id)
            .finish_non_exhaustive()
    }
}

/// Object-safe provider boundary returning a boxed Send future.
pub trait Provider: Send + Sync {
    /// Invokes the provider once with exact recorded metadata.
    fn invoke<'a>(
        &'a self,
        invocation: &'a InvocationContext,
    ) -> Pin<Box<dyn Future<Output = ProviderOutcome> + Send + 'a>>;
}

/// One requested invocation with an explicit provider.
pub struct InvocationRequest<'a> {
    /// Context owning the branch.
    pub context: ContextId,
    /// Local branch to record against.
    pub branch: LocalRefName,
    /// Expected current branch head.
    pub expected_head: EventId,
    /// Opaque input, size-checked before recording.
    pub input: Value,
    /// Provider invoked only after the request commits.
    pub provider: &'a dyn Provider,
}

/// Public non-secret report of one completed invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvocationReport {
    /// Committed agent.request event.
    pub request_event_id: EventId,
    /// Random invocation correlation ID.
    pub invocation_id: String,
    /// Recorded result kind.
    pub outcome: OutcomeKind,
    /// Committed agent.response or agent.error event.
    pub result_event_id: EventId,
}

/// Records one invocation: request commit, provider call, linked result.
///
/// Never holds a transaction or lock across the provider call. If the branch
/// moved before the result commit, the linked result is still admitted with no
/// ref mutation and `PostExecutionConflict` is returned.
pub async fn record_invocation(
    store: &Store,
    identity: &SigningIdentity,
    request: InvocationRequest<'_>,
) -> ProviderResult<InvocationReport> {
    let input_wire = canonicalize(&request.input).map_err(|_| ProviderError::Validation)?;
    if input_wire.len() > PROVIDER_INPUT_LIMIT {
        return Err(ProviderError::LimitExceeded);
    }
    let projection = store
        .project(
            request.context,
            vec![request.expected_head],
            ProjectionLimits::default(),
        )
        .await
        .map_err(ProviderError::Store)?;
    let ancestry = projection.events;
    let invocation_id = generate_invocation_id()?;
    let request_event = identity
        .create_event(
            request.context,
            vec![request.expected_head],
            "agent.request",
            json!({
                "input": request.input,
                "invocation_id": invocation_id,
                "selected_head": request.expected_head.to_string()
            }),
        )
        .map_err(|_| ProviderError::Validation)?;
    let request_event_id = request_event.event_id();
    store
        .admit(
            &request_event,
            RefMutation::CompareAndSwap {
                context: request.context,
                name: request.branch.clone(),
                expected: RefExpectation::Head(request.expected_head),
                new_head: request_event_id,
            },
        )
        .await
        .map_err(ProviderError::Store)?;
    let invocation = InvocationContext {
        context: request.context,
        selected_head: request.expected_head,
        ancestry,
        request_event_id,
        invocation_id: invocation_id.clone(),
        input: request.input.clone(),
    };
    let (kind, payload) = match request.provider.invoke(&invocation).await {
        ProviderOutcome::Response(response) => {
            let wire = canonicalize(&response).map_err(|_| ProviderError::ProviderMalformed)?;
            if wire.len() > PROVIDER_RESPONSE_LIMIT {
                return Err(ProviderError::LimitExceeded);
            }
            (
                OutcomeKind::Response,
                json!({"invocation_id": invocation_id, "response": response}),
            )
        }
        ProviderOutcome::Failure { code, detail } => (
            OutcomeKind::RecordedError,
            json!({
                "detail": sanitize_detail(&detail),
                "error_code": code,
                "invocation_id": invocation_id
            }),
        ),
    };
    let result_event = identity
        .create_event(
            request.context,
            vec![request_event_id],
            match kind {
                OutcomeKind::Response => "agent.response",
                OutcomeKind::RecordedError => "agent.error",
            },
            payload,
        )
        .map_err(|_| ProviderError::Internal)?;
    let result_event_id = result_event.event_id();
    match store
        .admit(
            &result_event,
            RefMutation::CompareAndSwap {
                context: request.context,
                name: request.branch.clone(),
                expected: RefExpectation::Head(request_event_id),
                new_head: result_event_id,
            },
        )
        .await
    {
        Ok(_) => Ok(InvocationReport {
            request_event_id,
            invocation_id,
            outcome: kind,
            result_event_id,
        }),
        Err(StoreError::StaleHead { current }) => {
            store
                .admit(&result_event, RefMutation::None)
                .await
                .map_err(ProviderError::Store)?;
            Err(ProviderError::PostExecutionConflict {
                result: result_event_id,
                current_head: current,
            })
        }
        Err(error) => Err(ProviderError::Store(error)),
    }
}

use crate::error::StoreError;

/// Runs one local command per invocation over bounded JSONL pipes.
pub struct CommandProvider {
    program: PathBuf,
    args: Vec<OsString>,
}

impl CommandProvider {
    /// Creates a subprocess provider for one command; never a shell.
    pub fn new(program: impl Into<PathBuf>, args: Vec<OsString>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }

    async fn run(&self, invocation: &InvocationContext) -> ProviderOutcome {
        match self.exchange(invocation).await {
            Ok(outcome) => outcome,
            Err(code_detail) => ProviderOutcome::Failure {
                code: code_detail.0,
                detail: code_detail.1,
            },
        }
    }

    async fn exchange(
        &self,
        invocation: &InvocationContext,
    ) -> Result<ProviderOutcome, (&'static str, String)> {
        let mut ancestry = Vec::with_capacity(invocation.ancestry.len());
        if invocation.ancestry.len() > MAX_INVOCATION_ANCESTRY {
            return Err((
                "limit_exceeded",
                "invocation ancestry exceeds its bound".into(),
            ));
        }
        for event in &invocation.ancestry {
            let wire = event.to_wire().map_err(|_| {
                (
                    "internal",
                    "stored ancestry could not be re-rendered".to_owned(),
                )
            })?;
            ancestry.push(
                serde_json::from_slice::<Value>(&wire)
                    .map_err(|_| ("internal", "ancestry re-parse failed".to_owned()))?,
            );
        }
        let document = json!({
            "ancestry": ancestry,
            "context": invocation.context.to_string(),
            "input": invocation.input,
            "invocation_id": invocation.invocation_id,
            "protocol_version": 1,
            "request_event_id": invocation.request_event_id.to_string(),
            "selected_head": invocation.selected_head.to_string()
        });
        let mut line = canonicalize(&document).map_err(|_| {
            (
                "internal",
                "invocation document rendering failed".to_owned(),
            )
        })?;
        if line.len() + 1 > JSONL_LINE_LIMIT {
            return Err((
                "limit_exceeded",
                "invocation document exceeds the JSONL line limit".into(),
            ));
        }
        line.push(b'\n');

        let mut child = tokio::process::Command::new(&self.program)
            .args(&self.args)
            .stdin(StdStdio::piped())
            .stdout(StdStdio::piped())
            .stderr(StdStdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| {
                (
                    "provider_transport",
                    "provider process could not start".into(),
                )
            })?;
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
        let mut stdin_handle = child.stdin.take().ok_or((
            "provider_transport",
            "provider stdin unavailable".to_owned(),
        ))?;
        let mut stdout_handle = child.stdout.take().ok_or((
            "provider_transport",
            "provider stdout unavailable".to_owned(),
        ))?;
        let outcome = tokio::time::timeout(PROVIDER_EXECUTION_TIMEOUT, async {
            stdin_handle.write_all(&line).await.map_err(|_| {
                (
                    "provider_transport",
                    "provider stdin write failed".to_owned(),
                )
            })?;
            stdin_handle.shutdown().await.map_err(|_| {
                (
                    "provider_transport",
                    "provider stdin close failed".to_owned(),
                )
            })?;
            drop(stdin_handle);
            let mut collected = Vec::new();
            let mut chunk = [0_u8; 8 * 1024];
            loop {
                let count = stdout_handle.read(&mut chunk).await.map_err(|_| {
                    (
                        "provider_transport",
                        "provider stdout read failed".to_owned(),
                    )
                })?;
                if count == 0 {
                    break;
                }
                collected.extend_from_slice(&chunk[..count]);
                if collected.len() > JSONL_LINE_LIMIT + 1 {
                    return Err((
                        "limit_exceeded",
                        "provider output exceeds the JSONL line limit".to_owned(),
                    ));
                }
                if collected.contains(&b'\n') {
                    break;
                }
            }
            drop(stdout_handle);
            let status = child.wait().await.map_err(|_| {
                (
                    "provider_transport",
                    "provider exit could not be observed".to_owned(),
                )
            })?;
            if !status.success() {
                return Err((
                    "provider_transport",
                    "provider exited without success".to_owned(),
                ));
            }
            Ok(collected)
        })
        .await;
        match outcome {
            Err(_) => {
                let _ = child.kill().await;
                Err(("provider_timeout", "provider execution timed out".into()))
            }
            Ok(result) => result,
        }
        .and_then(|collected| parse_provider_line(&collected, &invocation.invocation_id))
    }
}

impl Provider for CommandProvider {
    fn invoke<'a>(
        &'a self,
        invocation: &'a InvocationContext,
    ) -> Pin<Box<dyn Future<Output = ProviderOutcome> + Send + 'a>> {
        Box::pin(async move { self.run(invocation).await })
    }
}

fn parse_provider_line(
    collected: &[u8],
    expected_invocation: &str,
) -> Result<ProviderOutcome, (&'static str, String)> {
    let line = collected.split(|byte| *byte == b'\n').next().unwrap_or(&[]);
    if line.is_empty() {
        return Err((
            "provider_malformed",
            "provider emitted no response line".into(),
        ));
    }
    let value: Value = serde_json::from_slice(line).map_err(|_| {
        (
            "provider_malformed",
            "provider response is not JSON".to_owned(),
        )
    })?;
    let object = value
        .as_object()
        .ok_or_else(|| {
            (
                "provider_malformed",
                "provider response is not an object".to_owned(),
            )
        })?
        .clone();
    let malformed = || {
        (
            "provider_malformed",
            "provider response fields are invalid".to_owned(),
        )
    };
    if object.get("protocol_version").and_then(Value::as_u64) != Some(1) {
        return Err(malformed());
    }
    let supplied_id = object
        .get("invocation_id")
        .and_then(Value::as_str)
        .ok_or_else(malformed)?;
    if supplied_id != expected_invocation {
        return Err(malformed());
    }
    match object.get("ok").and_then(Value::as_bool) {
        Some(true)
            if object.len() == 4
                && matches!(
                    object.get("response"),
                    Some(Value::Object(_))
                        | Some(Value::Array(_))
                        | Some(Value::String(_))
                        | Some(Value::Number(_))
                        | Some(Value::Bool(_))
                        | Some(Value::Null)
                ) =>
        {
            let response = object.get("response").ok_or_else(malformed)?.clone();
            let wire = canonicalize(&response).map_err(|_| {
                (
                    "provider_malformed",
                    "provider response is not canonicalizable".to_owned(),
                )
            })?;
            if wire.len() > PROVIDER_RESPONSE_LIMIT {
                return Err((
                    "limit_exceeded",
                    "provider response exceeds its bound".into(),
                ));
            }
            Ok(ProviderOutcome::Response(response))
        }
        Some(false)
            if object.len() == 5
                && object.get("detail").and_then(Value::as_str).is_some()
                && object.get("error_code").and_then(Value::as_str).is_some() =>
        {
            let code = object
                .get("error_code")
                .and_then(Value::as_str)
                .ok_or_else(malformed)?;
            let detail = object
                .get("detail")
                .and_then(Value::as_str)
                .ok_or_else(malformed)?;
            Ok(ProviderOutcome::Failure {
                code: "provider_declared",
                detail: sanitize_detail(&format!("{code}: {detail}")),
            })
        }
        _ => Err(malformed()),
    }
}

/// Sanitizes failure detail: control characters replaced, bounded length.
pub fn sanitize_detail(text: &str) -> String {
    let mut out = String::with_capacity(text.len().min(ERROR_DETAIL_LIMIT));
    for character in text.chars() {
        let replacement = if character.is_ascii_control() {
            Cow::Borrowed("\u{FFFD}")
        } else {
            Cow::Owned(character.to_string())
        };
        if out.len() + replacement.len() > ERROR_DETAIL_LIMIT {
            break;
        }
        out.push_str(&replacement);
    }
    out
}

fn generate_invocation_id() -> ProviderResult<String> {
    let mut bytes = Zeroizing::new([0_u8; 16]);
    getrandom::fill(bytes.as_mut()).map_err(|_| ProviderError::Internal)?;
    Ok(format!("inv1_{}", URL_SAFE_NO_PAD.encode(bytes.as_ref())))
}
