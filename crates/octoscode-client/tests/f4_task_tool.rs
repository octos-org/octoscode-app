//! [F4] `task` + `tool` client tests: fake-WS round-trips (one per method) and
//! a fixture-decode test per notification the domain handles.
//!
//! The fake transport is the same channel-pair contract `tests/client_core.rs`
//! uses: it speaks `OutboundCommand::Request` + a oneshot reply, so a route is
//! proved without a socket. The live socket path is `tests/live_f4.rs`.
use std::sync::{Arc, Mutex};

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::{methods, RpcError};
use octos_app_transport::OutboundCommand;
use octoscode_store::Store;
use tokio::sync::mpsc;

use octoscode_client::domains;
use octos_core::ui_protocol::{
    TaskArtifactListParams, TaskArtifactReadParams, TaskCancelParams, TaskListParams,
};
use octoscode_client::domains::task::{TaskArtifactList, TaskArtifactRead, TaskCancel, TaskList};
use octoscode_client::domains::tool::{McpStatusList, McpStatusListParams};
use octoscode_client::{Client, ClientError};

// ------------------------------------------------------------- fake transport

/// A transport that records every `(method, params)` it is handed and answers
/// with `responder`'s value.
fn fake_transport<F>(responder: F) -> (mpsc::Sender<OutboundCommand>, Arc<Mutex<Vec<serde_json::Value>>>)
where
    F: Fn(&str, &serde_json::Value) -> Result<serde_json::Value, RpcError> + Send + 'static,
{
    let (tx, mut rx) = mpsc::channel::<OutboundCommand>(8);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorder = seen.clone();
    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            if let OutboundCommand::Request { method, params, reply } = cmd {
                recorder
                    .lock()
                    .unwrap()
                    .push(serde_json::json!({"method": method, "params": params}));
                let _ = reply.send(responder(&method, &params));
            }
        }
    });
    (tx, seen)
}

fn notification(method: &str, params: serde_json::Value) -> UiNotification {
    UiNotification::from_method_and_params(method, params)
        .expect("the test's params decode for this method")
}

// ------------------------------------------------------ request round-trips

#[tokio::test]
async fn task_list_sends_session_id_and_decodes_typed_rows() {
    let (tx, seen) = fake_transport(|method, params| {
        assert_eq!(method, "task/list");
        // The web sends `{ session_id }` (supervision/use-supervision.ts:124).
        assert_eq!(params["session_id"], "s1");
        assert!(params.get("topic").is_none(), "topic is omitted when unset");
        Ok(serde_json::json!({
            "session_id": "s1",
            "tasks": [{
                "id": "00000000-0000-7000-8000-0000000000a1",
                "tool_name": "bash",
                "tool_call_id": "call-1",
                "state": "running",
                "status": "running",
                "lifecycle_state": "running",
                "runtime_state": "running",
                "summary": "build",
                "artifact_count": 2,
                "started_at": "2026-09-28T00:00:00Z",
                "updated_at": "2026-09-28T00:00:01Z"
            }]
        }))
    });
    let client = Client::new(tx);
    let params = TaskListParams {
        session_id: octos_core::SessionKey("s1".into()),
        topic: None,
    };
    let out = client.call::<TaskList>(params).await.expect("round trip");
    assert_eq!(out.tasks.len(), 1);
    assert_eq!(out.tasks[0].tool_name, "bash");
    assert_eq!(out.tasks[0].summary.as_deref(), Some("build"));
    assert_eq!(out.tasks[0].artifact_count, Some(2));
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn task_cancel_sends_task_and_session() {
    let (tx, _seen) = fake_transport(|method, params| {
        assert_eq!(method, "task/cancel");
        // The web sends `{ task_id, session_id }` (use-supervision.ts:329).
        assert_eq!(params["task_id"], "00000000-0000-7000-8000-0000000000a1");
        assert_eq!(params["session_id"], "s1");
        Ok(serde_json::json!({
            "task_id": "00000000-0000-7000-8000-0000000000a1",
            "status": "cancelled"
        }))
    });
    let client = Client::new(tx);
    let params = TaskCancelParams {
        task_id: octos_core::TaskId("00000000-0000-7000-8000-0000000000a1".parse().unwrap()),
        session_id: Some(octos_core::SessionKey("s1".into())),
        profile_id: None,
    };
    let out = client.call::<TaskCancel>(params).await.expect("round trip");
    assert_eq!(out.status, octos_core::ui_protocol::TaskRuntimeState::Cancelled);
}

#[tokio::test]
async fn task_artifact_list_sends_session_and_task() {
    let (tx, _seen) = fake_transport(|method, params| {
        assert_eq!(method, "task/artifact/list");
        // The web sends `{ session_id, task_id }` (use-supervision.ts:190).
        assert_eq!(params["session_id"], "s1");
        assert_eq!(params["task_id"], "00000000-0000-7000-8000-0000000000a1");
        Ok(serde_json::json!({
            "session_id": "s1",
            "task_id": "00000000-0000-7000-8000-0000000000a1",
            "artifacts": [{
                "id": "art-1", "title": "report.md", "kind": "file", "status": "ready"
            }]
        }))
    });
    let client = Client::new(tx);
    let params = TaskArtifactListParams {
        session_id: octos_core::SessionKey("s1".into()),
        task_id: octos_core::TaskId("00000000-0000-7000-8000-0000000000a1".parse().unwrap()),
        profile_id: None,
        agent_id: None,
    };
    let out = client.call::<TaskArtifactList>(params).await.expect("round trip");
    assert_eq!(out.artifacts.len(), 1);
    assert_eq!(out.artifacts[0].title, "report.md");
}

#[tokio::test]
async fn task_artifact_read_sends_the_window_the_web_sends() {
    let (tx, _seen) = fake_transport(|method, params| {
        assert_eq!(method, "task/artifact/read");
        // The web sends `{ session_id, task_id, artifact_id, limit_bytes: 262144 }`
        // (use-supervision.ts:385).
        assert_eq!(params["artifact_id"], "art-1");
        assert_eq!(params["limit_bytes"], 262_144);
        assert!(params.get("cursor").is_none(), "no cursor on the first page");
        Ok(serde_json::json!({
            "session_id": "s1",
            "task_id": "00000000-0000-7000-8000-0000000000a1",
            "artifact": {"id": "art-1", "title": "report.md", "kind": "file", "status": "ready"},
            "content": "hello",
            "next_cursor": {"offset": 5},
            "has_more": true
        }))
    });
    let client = Client::new(tx);
    let params = TaskArtifactReadParams {
        session_id: octos_core::SessionKey("s1".into()),
        task_id: octos_core::TaskId("00000000-0000-7000-8000-0000000000a1".parse().unwrap()),
        artifact_id: Some("art-1".into()),
        path: None,
        cursor: None,
        limit_bytes: Some(262_144),
        profile_id: None,
        agent_id: None,
    };
    let out = client.call::<TaskArtifactRead>(params).await.expect("round trip");
    assert_eq!(out.content.as_deref(), Some("hello"));
    assert_eq!(out.next_cursor.unwrap().offset, 5);
    assert!(out.has_more);
}

#[tokio::test]
async fn mcp_status_list_sends_profile_and_decode_the_extension_result() {
    let (tx, _seen) = fake_transport(|method, params| {
        // Extension method: the wire name is the one the server dispatches
        // (octos-cli `ui_protocol_transport.rs:273`).
        assert_eq!(method, "mcp/status/list");
        // The web sends `{ session_id, profile_id, include_disabled: true }`
        // (inventory.ts:184).
        assert_eq!(params["session_id"], "s1");
        assert_eq!(params["profile_id"], "octoscode");
        assert_eq!(params["include_disabled"], true);
        Ok(serde_json::json!({
            "session_id": "s1",
            "profile_id": "octoscode",
            "servers": [{
                "id": "filesystem", "display_name": "Filesystem", "transport": "stdio",
                "status": "connected", "tool_count": 3, "tools": ["read", "write"]
            }],
            "summary": {"connected": 1, "connecting": 0, "failed": 0, "disabled": 0}
        }))
    });
    let client = Client::new(tx);
    let out = client
        .call::<McpStatusList>(McpStatusListParams {
            session_id: "s1".into(),
            profile_id: "octoscode".into(),
            include_disabled: true,
        })
        .await
        .expect("round trip");
    assert_eq!(out.servers.len(), 1);
    assert_eq!(out.servers[0].status, "connected");
    assert_eq!(out.summary.connected, 1);
}

// --------------------------------------------------------- error surfaces

#[tokio::test]
async fn an_rpc_error_on_a_task_method_carries_the_method_name() {
    let (tx, _seen) = fake_transport(|_m, _p| {
        Err(RpcError { code: -32602, message: "no such task".into(), data: None })
    });
    let client = Client::new(tx);
    let params = TaskListParams {
        session_id: octos_core::SessionKey("s1".into()),
        topic: None,
    };
    match client.call::<TaskList>(params).await {
        Err(ClientError::Rpc { method, error }) => {
            assert_eq!(method, "task/list");
            assert_eq!(error.code, -32602);
        }
        other => panic!("expected an RPC error, got {other:?}"),
    }
}

// --------------------------------------------------- notification decoding

#[tokio::test]
async fn task_updated_decodes_and_updates_the_store_row() {
    let (tx, _seen) = fake_transport(|_m, _p| Ok(serde_json::json!({})));
    let _client = Client::new(tx);

    let store = Arc::new(Store::new());
    let mut reg = octoscode_client::Registry::new();
    domains::task::register(&mut reg, store.clone());

    let n = notification(
        methods::TASK_UPDATED,
        serde_json::json!({
            "session_id": "s1",
            "task_id": "00000000-0000-7000-8000-0000000000a1",
            "title": "bash",
            "state": "failed",
            "runtime_detail": "exit 1"
        }),
    );
    assert!(reg.dispatch(&n));
    let row = store
        .domains
        .task
        .snapshot("00000000-0000-7000-8000-0000000000a1")
        .expect("a row was written");
    assert_eq!(row.state, "failed");
    assert_eq!(row.error.as_deref(), Some("exit 1"));
    assert_eq!(row.tool_name, "bash", "the update's title seeds an empty row");
    assert_eq!(store.seen_count(methods::TASK_UPDATED), 1);
}

#[tokio::test]
async fn task_output_delta_decodes_and_accumulates() {
    let (tx, _seen) = fake_transport(|_m, _p| Ok(serde_json::json!({})));
    let _client = Client::new(tx);
    let store = Arc::new(Store::new());
    let mut reg = octoscode_client::Registry::new();
    domains::task::register(&mut reg, store.clone());

    let n = notification(
        methods::TASK_OUTPUT_DELTA,
        serde_json::json!({
            "session_id": "s1",
            "task_id": "00000000-0000-7000-8000-0000000000a1",
            "cursor": {"offset": 0},
            "text": "chunk"
        }),
    );
    assert!(reg.dispatch(&n));
    assert_eq!(
        store.domains.task.output("00000000-0000-7000-8000-0000000000a1"),
        "chunk"
    );
}

#[tokio::test]
async fn plan_updated_decodes_and_replaces_the_session_plan() {
    let (tx, _seen) = fake_transport(|_m, _p| Ok(serde_json::json!({})));
    let _client = Client::new(tx);
    let store = Arc::new(Store::new());
    let mut reg = octoscode_client::Registry::new();
    domains::task::register(&mut reg, store.clone());

    let n = notification(
        methods::PLAN_UPDATED,
        serde_json::json!({
            "session_id": "s1",
            "turn_id": "00000000-0000-7000-8000-0000000000b1",
            "plan": {
                "items": [
                    {"id": "i1", "title": "one", "status": "in_progress", "priority": "P1"},
                    {"id": "i2", "title": "two", "status": "pending"}
                ],
                "title": "Building",
                "updated_at_ms": 42
            }
        }),
    );
    assert!(reg.dispatch(&n));
    let plan = store.domains.task.plan("s1").expect("a plan was written");
    assert_eq!(plan.items.len(), 2);
    assert_eq!(plan.items[0].status, "in_progress");
    assert_eq!(plan.items[1].status, "pending");
    assert_eq!(plan.title.as_deref(), Some("Building"));
    assert_eq!(plan.updated_at_ms, 42);
    assert_eq!(
        plan.turn_id.as_deref(),
        Some("00000000-0000-7000-8000-0000000000b1")
    );
    assert_eq!(store.seen_count(methods::PLAN_UPDATED), 1);
}

#[test]
fn an_unhandled_notification_is_still_tolerated_by_name() {
    // The registry's tolerated-unknown arm (RULES #6 / 8.8 condition 7): a
    // notification outside this domain must not panic, and must be named.
    let store = Arc::new(Store::new());
    let mut reg = octoscode_client::Registry::new();
    domains::task::register(&mut reg, store);
    let n = notification(
        methods::WARNING,
        serde_json::json!({
            "session_id": "s1",
            "code": "degraded",
            "message": "something changed"
        }),
    );
    assert!(!reg.dispatch(&n));
    assert_eq!(reg.unknown_methods(), &[methods::WARNING.to_string()]);
}
