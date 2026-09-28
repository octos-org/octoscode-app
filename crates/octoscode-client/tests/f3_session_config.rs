//! Card #F3 client tests — one fake-WS round-trip per owned method.
//!
//! The fake transport is the same channel-pair contract the real WS transport
//! implements (`crates/octoscode-client/tests/client_core.rs`), so every call
//! here rides the production `Client::call` path. Each test asserts the exact
//! **request frame** the web sends (params + defaults) and decodes the typed
//! result; one error case proves an RPC error keeps the method name.
use std::sync::{Arc, Mutex};

use octos_app_transport::OutboundCommand;
use octos_core::ui_protocol::RpcError;
use octoscode_store::Store;
use tokio::sync::mpsc;

use octoscode_client::domains::{config, session};
use octoscode_client::{Client, ClientError};

/// A fake transport that records every `(method, params)` and answers from
/// `responder`. Returns the sender + the shared request log.
fn fake_transport<F>(
    responder: F,
) -> (
    mpsc::Sender<OutboundCommand>,
    Arc<Mutex<Vec<(String, serde_json::Value)>>>,
)
where
    F: Fn(&str, &serde_json::Value) -> Result<serde_json::Value, RpcError> + Send + 'static,
{
    let (tx, mut rx) = mpsc::channel::<OutboundCommand>(8);
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = log.clone();
    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            if let OutboundCommand::Request { method, params, reply } = cmd {
                sink.lock().unwrap().push((method.clone(), params.clone()));
                let _ = reply.send(responder(&method, &params));
            }
        }
    });
    (tx, log)
}

fn one(log: &Arc<Mutex<Vec<(String, serde_json::Value)>>>) -> (String, serde_json::Value) {
    log.lock().unwrap().first().cloned().expect("one request")
}

// --------------------------------------------------------------- session/*

#[tokio::test]
async fn session_btw_sends_session_id_and_question() {
    let (tx, log) = fake_transport(|m, _p| {
        assert_eq!(m, "session/btw");
        Ok(serde_json::json!({
            "session_id": "c:c1", "answer": "working on F3", "model": "deepseek-chat"
        }))
    });
    let client = Client::new(tx);
    let out = client
        .call::<session::SessionBtw>(octos_core::ui_protocol::SessionBtwParams {
            session_id: octos_core::SessionKey("c:c1".into()),
            topic: None,
            question: "what are you doing?".into(),
        })
        .await
        .expect("round trip");
    assert_eq!(out.session_id.0, "c:c1");
    assert_eq!(out.answer, "working on F3");
    assert_eq!(out.model.as_deref(), Some("deepseek-chat"));

    // The web sends `{session_id, question}` and omits the optional `topic`.
    let (m, p) = one(&log);
    assert_eq!(m, "session/btw");
    assert_eq!(p["session_id"], "c:c1");
    assert_eq!(p["question"], "what are you doing?");
    assert!(p.get("topic").is_none(), "topic omitted like the web (btw.ts:77)");
}

#[tokio::test]
async fn session_delete_sends_session_id_and_decodes_empty_result() {
    let (tx, log) = fake_transport(|m, _p| {
        assert_eq!(m, "session/delete");
        Ok(serde_json::json!({}))
    });
    let client = Client::new(tx);
    client
        .call::<session::SessionDelete>(octos_core::ui_protocol::SessionDeleteParams {
            session_id: "c:c1".into(),
        })
        .await
        .expect("round trip");
    let (m, p) = one(&log);
    assert_eq!(m, "session/delete");
    assert_eq!(p, serde_json::json!({"session_id": "c:c1"}));
}

#[tokio::test]
async fn session_files_list_sends_session_id() {
    let (tx, log) = fake_transport(|_m, _p| {
        Ok(serde_json::json!({"files": [{"handle": "h1", "path": "a.txt"}]}))
    });
    let client = Client::new(tx);
    let out = client
        .call::<session::SessionFilesList>(octos_core::ui_protocol::SessionFilesListParams {
            session_id: "c:c1".into(),
        })
        .await
        .expect("round trip");
    assert_eq!(out.files[0]["handle"], "h1");
    let (m, p) = one(&log);
    assert_eq!(m, "session/files.list");
    assert_eq!(p, serde_json::json!({"session_id": "c:c1"}));
}

#[tokio::test]
async fn session_fork_sends_chat_id_and_optional_copy_count() {
    let (tx, log) = fake_transport(|_m, _p| {
        Ok(serde_json::json!({
            "new_session_id": "c:child", "parent_session_id": "c:c1", "copied_messages": 4
        }))
    });
    let client = Client::new(tx);
    let out = client
        .call::<session::SessionFork>(octos_core::ui_protocol::SessionForkParams {
            session_id: octos_core::SessionKey("c:c1".into()),
            new_chat_id: "child".into(),
            copy_messages: Some(4),
        })
        .await
        .expect("round trip");
    assert_eq!(out.new_session_id.0, "c:child");
    assert_eq!(out.copied_messages, 4);

    let (m, p) = one(&log);
    assert_eq!(m, "session/fork");
    assert_eq!(p["session_id"], "c:c1");
    assert_eq!(p["new_chat_id"], "child");
    assert_eq!(p["copy_messages"], 4);
}

#[tokio::test]
async fn session_fork_omits_copy_messages_when_absent() {
    let (tx, log) = fake_transport(|_m, _p| {
        Ok(serde_json::json!({
            "new_session_id": "c:child", "parent_session_id": "c:c1", "copied_messages": 0
        }))
    });
    let client = Client::new(tx);
    client
        .call::<session::SessionFork>(octos_core::ui_protocol::SessionForkParams {
            session_id: octos_core::SessionKey("c:c1".into()),
            new_chat_id: "child".into(),
            copy_messages: None,
        })
        .await
        .expect("round trip");
    // `copy_messages` is `skip_serializing_if = "Option::is_none"` in the pin.
    assert!(one(&log).1.get("copy_messages").is_none());
}

#[tokio::test]
async fn session_rollback_sends_num_turns() {
    let (tx, log) = fake_transport(|_m, _p| {
        Ok(serde_json::json!({
            "dropped_turns": 2,
            "thread": {"session_id": "c:c1", "cursor": {"stream": "main", "seq": 7}}
        }))
    });
    let client = Client::new(tx);
    let out = client
        .call::<session::SessionRollback>(octos_core::ui_protocol::SessionRollbackParams {
            session_id: octos_core::SessionKey("c:c1".into()),
            num_turns: 2,
        })
        .await
        .expect("round trip");
    assert_eq!(out.dropped_turns, 2);
    let (m, p) = one(&log);
    assert_eq!(m, "session/rollback");
    assert_eq!(p, serde_json::json!({"session_id": "c:c1", "num_turns": 2}));
}

#[tokio::test]
async fn session_status_read_sends_session_id_and_checks_identity() {
    let (tx, log) = fake_transport(|_m, _p| {
        Ok(serde_json::json!({
            "session_id": "c:c1",
            "profile_id": "coding",
            "runtime_policy_stamp": {"model": "deepseek-chat", "provider": "deepseek"},
            "health": {"status": "ok"}
        }))
    });
    let client = Client::new(tx);
    let out = client
        .call::<session::SessionStatusRead>(session::SessionStatusReadParams {
            session_id: "c:c1".into(),
        })
        .await
        .expect("round trip");
    assert_eq!(out.session_id, "c:c1");
    assert_eq!(out.profile_id.as_deref(), Some("coding"));
    let (m, p) = one(&log);
    assert_eq!(m, "session/status/read");
    assert_eq!(p, serde_json::json!({"session_id": "c:c1"}));
}

#[tokio::test]
async fn session_compact_sends_session_id_and_decodes_outcome() {
    let (tx, log) = fake_transport(|_m, _p| {
        Ok(serde_json::json!({
            "session_id": "c:c1", "compacted": true, "status": "ok",
            "token_estimate_before": 90000, "token_estimate_after": 40000
        }))
    });
    let client = Client::new(tx);
    let out = client
        .call::<session::SessionCompact>(session::SessionCompactParams {
            session_id: "c:c1".into(),
        })
        .await
        .expect("round trip");
    assert!(out.compacted);
    assert_eq!(out.token_estimate_after, Some(40000));
    let (m, p) = one(&log);
    assert_eq!(m, "session/compact");
    assert_eq!(p, serde_json::json!({"session_id": "c:c1"}));
}

#[tokio::test]
async fn session_compact_mode_set_sends_mode() {
    let (tx, log) = fake_transport(|_m, _p| {
        Ok(serde_json::json!({"session_id": "c:c1", "mode": "heuristic"}))
    });
    let client = Client::new(tx);
    let out = client
        .call::<session::SessionCompactModeSet>(session::SessionCompactModeSetParams {
            session_id: "c:c1".into(),
            mode: "heuristic".into(),
        })
        .await
        .expect("round trip");
    assert_eq!(out.mode, "heuristic");
    let (m, p) = one(&log);
    assert_eq!(m, "session/compact/mode/set");
    assert_eq!(p, serde_json::json!({"session_id": "c:c1", "mode": "heuristic"}));
}

// ---------------------------------------------------------------- config/*

#[tokio::test]
async fn launch_resolve_sends_cwd_and_optional_profile() {
    let (tx, log) = fake_transport(|_m, _p| {
        Ok(serde_json::json!({
            "decision": "cross_profile",
            "resolved_profile": "coding",
            "existing_profiles": ["other"]
        }))
    });
    let client = Client::new(tx);
    let out = client
        .call::<config::LaunchResolve>(octos_core::ui_protocol::LaunchResolveParams {
            cwd: "/tmp/proj".into(),
            profile_id: None,
        })
        .await
        .expect("round trip");
    use octos_core::ui_protocol::LaunchDecisionKind;
    assert_eq!(out.decision, LaunchDecisionKind::CrossProfile);
    assert_eq!(out.resolved_profile.as_deref(), Some("coding"));
    let (m, p) = one(&log);
    assert_eq!(m, "launch/resolve");
    assert_eq!(p["cwd"], "/tmp/proj");
    assert!(p.get("profile_id").is_none(), "profile_id omitted when absent");
}

#[tokio::test]
async fn snapshot_list_sends_session_id() {
    let (tx, log) = fake_transport(|_m, _p| {
        Ok(serde_json::json!({
            "session_id": "c:c1", "enabled": true, "available": true,
            "snapshots": [{"id": "snap-1", "label": "before", "timestamp_unix": 1700000000}]
        }))
    });
    let client = Client::new(tx);
    let out = client
        .call::<config::SnapshotListMethod>(config::SnapshotListParams {
            session_id: "c:c1".into(),
        })
        .await
        .expect("round trip");
    assert!(out.available);
    assert_eq!(out.snapshots[0].id, "snap-1");
    let (m, p) = one(&log);
    assert_eq!(m, "snapshot/list");
    assert_eq!(p, serde_json::json!({"session_id": "c:c1"}));
}

#[tokio::test]
async fn snapshot_restore_sends_snapshot_id() {
    let (tx, log) = fake_transport(|_m, _p| {
        Ok(serde_json::json!({
            "session_id": "c:c1", "restored": "snap-1",
            "snapshots": [{"id": "snap-1", "label": "before", "timestamp_unix": 1700000000}]
        }))
    });
    let client = Client::new(tx);
    let out = client
        .call::<config::SnapshotRestoreMethod>(config::SnapshotRestoreParams {
            session_id: "c:c1".into(),
            snapshot_id: "snap-1".into(),
        })
        .await
        .expect("round trip");
    assert_eq!(out.restored, "snap-1");
    let (m, p) = one(&log);
    assert_eq!(m, "snapshot/restore");
    assert_eq!(p, serde_json::json!({"session_id": "c:c1", "snapshot_id": "snap-1"}));
}

#[tokio::test]
async fn server_shutdown_sends_empty_params_and_decodes_stopping() {
    let (tx, log) = fake_transport(|_m, _p| Ok(serde_json::json!({"stopping": true})));
    let client = Client::new(tx);
    let out = client
        .call::<config::ServerShutdown>(config::ServerShutdownParams {})
        .await
        .expect("round trip");
    assert!(out.stopping);
    let (m, p) = one(&log);
    assert_eq!(m, "server/shutdown");
    assert_eq!(p, serde_json::json!({}), "empty params object");
}

// ------------------------------------------------------------ error + wiring

#[tokio::test]
async fn an_rpc_error_carries_the_owned_method_name() {
    let (tx, _log) = fake_transport(|_m, _p| {
        Err(RpcError {
            code: -32602,
            message: "session_id is required".into(),
            data: None,
        })
    });
    let client = Client::new(tx);
    match client
        .call::<session::SessionCompact>(session::SessionCompactParams {
            session_id: "c:c1".into(),
        })
        .await
    {
        Err(ClientError::Rpc { method, error }) => {
            assert_eq!(method, "session/compact");
            assert_eq!(error.code, -32602);
        }
        other => panic!("expected an RPC error, got {other:?}"),
    }
}

/// The two domains register their handlers through `register_all`, and the
/// store they write through is the shared one.
#[test]
fn register_all_wires_the_f3_handlers() {
    let store = Arc::new(Store::new());
    let mut reg = octoscode_client::Registry::new();
    octoscode_client::domains::register_all(&mut reg, store);
    for m in [
        octos_core::ui_protocol::methods::SESSION_EVENT,
        octos_core::ui_protocol::methods::SESSION_ORCHESTRATION,
        octos_core::ui_protocol::methods::SESSION_GOAL_UPDATED,
        octos_core::ui_protocol::methods::SESSION_GOAL_CLEARED,
        octos_core::ui_protocol::methods::REPLAY_LOSSY,
        octos_core::ui_protocol::methods::WARNING,
    ] {
        assert!(reg.handles(m), "expected a handler for {m}");
    }
}
