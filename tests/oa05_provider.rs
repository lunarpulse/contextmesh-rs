//! OA-05 provider recording, linked results, conflicts, and recovery queries.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use contextmesh::crypto::SigningIdentity;
use contextmesh::error::ProviderError;
use contextmesh::model::{ContextId, EventId};
use contextmesh::provider::{
    InvocationContext, InvocationRequest, OutcomeKind, Provider, ProviderOutcome,
    record_invocation, sanitize_detail,
};

#[derive(Clone, Debug)]
struct CapturedRecord {
    request_event_id: EventId,
    invocation_id: String,
    ancestry_heads: Vec<EventId>,
}
use contextmesh::store::{LocalRefName, ProjectionLimits, Store};
use serde_json::{Value, json};

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "oa05-prov-{tag}-{}-{}.db",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

#[derive(Clone, Copy)]
enum Behavior {
    Echo,
    Fail,
    Race,
}

struct Double {
    store: Store,
    identity: SigningIdentity,
    context: ContextId,
    branch: LocalRefName,
    behavior: Behavior,
    captured: Arc<Mutex<Option<CapturedRecord>>>,
}

impl Provider for Double {
    fn invoke<'a>(
        &'a self,
        invocation: &'a InvocationContext,
    ) -> Pin<Box<dyn Future<Output = ProviderOutcome> + Send + 'a>> {
        Box::pin(async move {
            // 05-P01: the request is committed and visible before the call.
            let committed = self
                .store
                .event(invocation.request_event_id)
                .await
                .expect("event lookup")
                .expect("request committed before provider call");
            assert_eq!(committed.body().kind(), "agent.request");
            // 05-P05: the request is pending during the crash window.
            let pending = self
                .store
                .pending_invocations(self.context, self.branch.clone())
                .await
                .expect("pending");
            assert!(
                pending
                    .iter()
                    .any(|event| event.event_id() == invocation.request_event_id)
            );
            // 05-P03: exact deterministic pre-request ancestry.
            let expected = self
                .store
                .project(
                    self.context,
                    vec![invocation.selected_head],
                    ProjectionLimits::default(),
                )
                .await
                .expect("project");
            let expected_ids: Vec<EventId> = expected.events.iter().map(|e| e.event_id()).collect();
            let supplied_ids: Vec<EventId> =
                invocation.ancestry.iter().map(|e| e.event_id()).collect();
            assert_eq!(expected_ids, supplied_ids);
            if matches!(self.behavior, Behavior::Race) {
                self.store
                    .append(
                        &self.identity,
                        self.context,
                        self.branch.clone(),
                        invocation.request_event_id,
                        "demo.race",
                        json!({}),
                    )
                    .await
                    .expect("racing writer moves the branch");
            }
            *self.captured.lock().expect("capture") = Some(CapturedRecord {
                request_event_id: invocation.request_event_id,
                invocation_id: invocation.invocation_id.clone(),
                ancestry_heads: invocation
                    .ancestry
                    .iter()
                    .map(|event| event.event_id())
                    .collect(),
            });
            match self.behavior {
                Behavior::Echo => ProviderOutcome::Response(json!({"echo": invocation.input})),
                Behavior::Fail => ProviderOutcome::Failure {
                    code: "provider_declared",
                    detail: "declared\u{0001}failure".to_owned(),
                },
                Behavior::Race => ProviderOutcome::Response(json!({"raced": true})),
            }
        })
    }
}

async fn setup(tag: &str) -> (Store, SigningIdentity, ContextId, EventId) {
    let store = Store::open(temp_path(tag)).await.expect("store");
    let identity = SigningIdentity::from_fixture_seed([11; 32]);
    let created = store
        .create_context(&identity, "main".parse().expect("name"))
        .await
        .expect("context");
    (store, identity, created.context, created.branch.head)
}

/// 05-P01/P02/P03 success path: linked agent.response with exact payload.
#[tokio::test]
async fn request_precedes_call_and_links_response() {
    let (store, identity, context, head) = setup("success").await;
    let captured = Arc::new(Mutex::new(None));
    let provider = Double {
        store: store.clone(),
        identity: SigningIdentity::from_fixture_seed([11; 32]),
        context,
        branch: "main".parse().expect("branch"),
        behavior: Behavior::Echo,
        captured: captured.clone(),
    };
    let report = record_invocation(
        &store,
        &identity,
        InvocationRequest {
            context,
            branch: "main".parse().expect("branch"),
            expected_head: head,
            input: json!({"note": "hello"}),
            provider: &provider,
        },
    )
    .await
    .expect("recorded");
    assert_eq!(report.outcome, OutcomeKind::Response);
    let record = captured.lock().expect("capture").clone().expect("captured");
    assert_eq!(record.request_event_id, report.request_event_id);
    assert_eq!(record.invocation_id, report.invocation_id);
    assert!(report.invocation_id.starts_with("inv1_") && report.invocation_id.len() == 27);
    assert_eq!(record.ancestry_heads, vec![head]);
    let response = store
        .event(report.result_event_id)
        .await
        .expect("lookup")
        .expect("result");
    assert_eq!(response.body().kind(), "agent.response");
    assert_eq!(response.body().parents(), &[report.request_event_id]);
    assert_eq!(
        response.body().payload(),
        &json!({"invocation_id": report.invocation_id, "response": {"echo": {"note": "hello"}}})
    );
    let branch_head = store
        .local_ref(context, &"main".parse().expect("branch"))
        .await
        .expect("ref");
    assert_eq!(branch_head, Some(report.result_event_id));
    assert!(
        store
            .pending_invocations(context, "main".parse().expect("branch"))
            .await
            .expect("pending")
            .is_empty()
    );
}

/// 05-P02 failure path: linked sanitized agent.error.
#[tokio::test]
async fn declared_failure_links_sanitized_error() {
    let (store, identity, context, head) = setup("failure").await;
    let provider = Double {
        store: store.clone(),
        identity: SigningIdentity::from_fixture_seed([11; 32]),
        context,
        branch: "main".parse().expect("branch"),
        behavior: Behavior::Fail,
        captured: Arc::new(Mutex::new(None)),
    };
    let report = record_invocation(
        &store,
        &identity,
        InvocationRequest {
            context,
            branch: "main".parse().expect("branch"),
            expected_head: head,
            input: Value::Null,
            provider: &provider,
        },
    )
    .await
    .expect("recorded failure outcome");
    assert_eq!(report.outcome, OutcomeKind::RecordedError);
    let error = store
        .event(report.result_event_id)
        .await
        .expect("lookup")
        .expect("result");
    assert_eq!(error.body().kind(), "agent.error");
    assert_eq!(error.body().parents(), &[report.request_event_id]);
    let payload = error.body().payload();
    assert_eq!(payload["error_code"], "provider_declared");
    assert_eq!(payload["invocation_id"], json!(report.invocation_id));
    let detail = payload["detail"].as_str().expect("detail");
    assert!(detail.contains("\u{fffd}"));
    assert!(!detail.contains('\u{0001}'));
}

/// 05-P04: a racing writer retains the detached result and reports conflict.
#[tokio::test]
async fn post_execution_conflict_retains_detached_result() {
    let (store, identity, context, head) = setup("conflict").await;
    let provider = Double {
        store: store.clone(),
        identity: SigningIdentity::from_fixture_seed([11; 32]),
        context,
        branch: "main".parse().expect("branch"),
        behavior: Behavior::Race,
        captured: Arc::new(Mutex::new(None)),
    };
    let error = record_invocation(
        &store,
        &identity,
        InvocationRequest {
            context,
            branch: "main".parse().expect("branch"),
            expected_head: head,
            input: Value::Null,
            provider: &provider,
        },
    )
    .await
    .expect_err("branch moved");
    let ProviderError::PostExecutionConflict {
        result,
        current_head,
    } = error
    else {
        panic!("expected PostExecutionConflict, got {error:?}");
    };
    assert!(current_head.is_some());
    let retained = store
        .event(result)
        .await
        .expect("lookup")
        .expect("detached result retained");
    assert_eq!(retained.body().kind(), "agent.response");
    assert_ne!(
        store
            .local_ref(context, &"main".parse().expect("branch"))
            .await
            .expect("ref"),
        Some(result)
    );
    let detached = store
        .detached_results(context, "main".parse().expect("branch"))
        .await
        .expect("detached");
    assert!(detached.iter().any(|event| event.event_id() == result));
    assert!(
        store
            .pending_invocations(context, "main".parse().expect("branch"))
            .await
            .expect("pending")
            .is_empty()
    );
}

/// 05-P05: a request with no result stays pending and is recoverable.
#[tokio::test]
async fn pending_request_is_recoverable() {
    let (store, identity, context, head) = setup("pending").await;
    let request = identity
        .create_event(
            context,
            vec![head],
            "agent.request",
            json!({"input": Value::Null, "invocation_id": "inv1_pending", "selected_head": head.to_string()}),
        )
        .expect("request");
    store
        .admit(
            &request,
            contextmesh::store::RefMutation::CompareAndSwap {
                context,
                name: "main".parse().expect("branch"),
                expected: contextmesh::store::RefExpectation::Head(head),
                new_head: request.event_id(),
            },
        )
        .await
        .expect("commit");
    let pending = store
        .pending_invocations(context, "main".parse().expect("branch"))
        .await
        .expect("pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].event_id(), request.event_id());
    assert!(
        store
            .detached_results(context, "main".parse().expect("branch"))
            .await
            .expect("detached")
            .is_empty()
    );
}

/// Stale expected head on the request CAS fails before any provider call.
#[tokio::test]
async fn stale_request_head_conflicts_without_invoking() {
    let (store, identity, context, head) = setup("stale").await;
    let other = SigningIdentity::from_fixture_seed([13; 32]);
    let moved = store
        .append(
            &other,
            context,
            "main".parse().expect("branch"),
            head,
            "demo.move",
            json!({}),
        )
        .await;
    if moved.is_err() {
        store
            .authorize_author(context, other.author())
            .await
            .expect("authorize");
    }
    let moved = store
        .append(
            &other,
            context,
            "main".parse().expect("branch"),
            head,
            "demo.move",
            json!({}),
        )
        .await
        .expect("move");
    let provider = Double {
        store: store.clone(),
        identity: SigningIdentity::from_fixture_seed([11; 32]),
        context,
        branch: "main".parse().expect("branch"),
        behavior: Behavior::Echo,
        captured: Arc::new(Mutex::new(None)),
    };
    let error = record_invocation(
        &store,
        &identity,
        InvocationRequest {
            context,
            branch: "main".parse().expect("branch"),
            expected_head: head,
            input: Value::Null,
            provider: &provider,
        },
    )
    .await
    .expect_err("stale");
    assert!(matches!(error, ProviderError::Store(_)));
    assert!(provider.captured.lock().expect("capture").is_none());
    assert_eq!(
        store
            .local_ref(context, &"main".parse().expect("branch"))
            .await
            .expect("ref"),
        Some(moved.event_id())
    );
}

#[test]
fn sanitizer_replaces_controls_and_bounds_length() {
    assert_eq!(sanitize_detail("a\u{0000}b"), "a\u{fffd}b");
    assert_eq!(sanitize_detail("x\u{007f}y"), "x\u{fffd}y");
    let long = "z".repeat(2_048);
    assert_eq!(sanitize_detail(&long).len(), 1_024);
}
