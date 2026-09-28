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

// ---------------------------------------------------- notification handlers
//
// One test per notification this lane handles: fixture JSON → the SAME
// decode path the transport uses (`UiNotification::from_method_and_params`)
// → registry dispatch → store state.

/// Decode a notification from its wire JSON, exactly as the transport does.
fn notification(method: &str, params: serde_json::Value) -> octos_core::app_ui::AppUiBackendEvent {
    use octos_core::app_ui::AppUiBackendEvent as N;
    N::from_method_and_params(method, params).expect("the fixture decodes")
}

fn wired() -> (octoscode_client::Registry, Arc<Store>) {
    let store = Arc::new(Store::new());
    let mut reg = octoscode_client::Registry::new();
    octoscode_client::domains::register_all(&mut reg, store.clone());
    (reg, store)
}

#[test]
fn session_event_notification_reaches_the_store() {
    let (mut reg, store) = wired();
    let n = notification(
        "session/event",
        serde_json::json!({
            "session_id": "c:c1", "kind": "turn.error",
            "payload": {"code": "boom"}
        }),
    );
    assert!(reg.dispatch(&n), "the session/event handler claims it");
    let got = store.domains.session.bridged_event("c:c1").expect("stored");
    assert_eq!(got.kind, "turn.error");
    assert_eq!(got.payload["code"], "boom");
    assert_eq!(store.seen_count("session/event"), 1);
}

#[test]
fn session_orchestration_notification_reaches_the_store() {
    let (mut reg, store) = wired();
    let n = notification(
        "session/orchestration",
        serde_json::json!({
            "session_id": "c:c1", "active": true,
            "running_agents": 3, "pending_continuations": 1, "phase": "agents"
        }),
    );
    assert!(reg.dispatch(&n));
    let got = store.domains.session.orchestration("c:c1").expect("stored");
    assert!(got.active);
    assert_eq!(got.running_agents, 3);
    assert_eq!(got.phase.as_deref(), Some("agents"));
}

#[test]
fn session_goal_updated_and_cleared_notifications_carry_the_generation_gate() {
    let (mut reg, store) = wired();
    let goal = serde_json::json!({
        "goal_id": "g1", "objective": "ship F3", "status": "active",
        "token_budget": 0, "tokens_used": 0, "time_used_seconds": 0,
        "created_at_ms": 0, "updated_at_ms": 0
    });
    // generation 5 update applies.
    assert!(reg.dispatch(&notification(
        "session/goal/updated",
        serde_json::json!({"session_id": "c:c1", "goal": goal, "generation": 5, "transition_actor": "model"}),
    )));
    assert_eq!(store.domains.session.goal("c:c1").unwrap()["goal_id"], "g1");

    // generation 4 update is stale -> dropped, goal unchanged.
    assert!(reg.dispatch(&notification(
        "session/goal/updated",
        serde_json::json!({"session_id": "c:c1", "goal": goal, "generation": 4, "transition_actor": "model"}),
    )));
    assert_eq!(store.domains.session.goal_generation("c:c1"), 5);

    // generation 6 clear applies -> goal gone.
    assert!(reg.dispatch(&notification(
        "session/goal/cleared",
        serde_json::json!({"session_id": "c:c1", "generation": 6, "transition_actor": "operator"}),
    )));
    assert!(store.domains.session.goal("c:c1").is_none());
    assert_eq!(store.domains.session.goal_generation("c:c1"), 6);
}

#[test]
fn replay_lossy_notification_reaches_the_store() {
    let (mut reg, store) = wired();
    let n = notification(
        "protocol/replay_lossy",
        serde_json::json!({
            "session_id": "c:c1", "dropped_count": 2,
            "last_durable_cursor": {"stream": "main", "seq": 9}
        }),
    );
    assert!(reg.dispatch(&n));
    let got = store.domains.config.replay_loss("c:c1").expect("stored");
    assert_eq!(got.dropped_count, 2);
    assert_eq!(got.last_durable_cursor.unwrap()["seq"], 9);
}

#[test]
fn warning_notification_reaches_the_store() {
    let (mut reg, store) = wired();
    let n = notification(
        "warning",
        serde_json::json!({
            "session_id": "c:c1", "code": "slow", "message": "the tool is slow"
        }),
    );
    assert!(reg.dispatch(&n));
    let got = store.domains.config.warning("c:c1").expect("stored");
    assert_eq!(got.code, "slow");
    assert_eq!(got.message, "the tool is slow");
}

#[test]
fn a_deliberately_unstored_notification_still_reaches_the_tolerated_arm() {
    // 8.8 condition 7: anything this lane does not store must still be logged
    // by name, never silently dropped. `agent/updated` belongs to another lane.
    let (mut reg, _store) = wired();
    let n = notification(
        "agent/updated",
        serde_json::json!({
            "session_id": "c:c1",
            "agent": {
                "agent_id": "a1", "session_id": "c:c1", "path": "root/a1",
                "role": "explorer", "nickname": "Scout", "backend_kind": "builtin",
                "status": "running", "profile_id": "coding",
                "created_at_ms": 0, "updated_at_ms": 0
            }
        }),
    );
    assert!(!reg.dispatch(&n), "no handler claims it -> tolerated-unknown");
    assert_eq!(reg.unknown_methods(), &["agent/updated".to_string()]);
}
