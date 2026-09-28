//! F5 — fake-transport round-trips for the turn/approval/review methods, and
//! registry-driven notification tests (fixture JSON → store state).
//!
//! Same fake-transport contract as `client_core.rs`: a channel pair that
//! answers each `OutboundCommand::Request`, so the production `Client::call`
//! path is exercised without a socket. The live socket path is
//! `tests/live_serve.rs` (`#[ignore]`).
use std::sync::{Arc, Mutex};

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::{methods, QuestionId, RpcError, TurnId};
use octos_core::SessionKey;
use octos_app_transport::OutboundCommand;
use octoscode_store::Store;
use tokio::sync::mpsc;

use octoscode_client::domains;
use octoscode_client::domains::approval::{ApprovalScopesList, UserQuestionRespond};
use octoscode_client::domains::review::ReviewStart;
use octoscode_client::domains::turn::{ThreadGraphGet, TurnStateGet, TurnSteer};
use octoscode_client::registry::Registry;
use octoscode_client::{Client, ClientError};

/// A fake transport: owns the receiver half of the command channel and
/// answers each request from a scripted function. The same command/reply
/// contract the real WS transport implements.
fn fake_transport<F>(responder: F) -> mpsc::Sender<OutboundCommand>
where
    F: Fn(&str, &serde_json::Value) -> Result<serde_json::Value, RpcError> + Send + 'static,
{
    let (tx, mut rx) = mpsc::channel::<OutboundCommand>(8);
    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            if let OutboundCommand::Request { method, params, reply } = cmd {
                let _ = reply.send(responder(&method, &params));
            }
        }
    });
    tx
}

/// Build a decoded notification from its wire JSON — the same path the
/// transport takes (`from_method_and_params`).
fn notification(method: &str, params: serde_json::Value) -> UiNotification {
    UiNotification::from_method_and_params(method, params)
        .expect("the fixture params decode for this method")
}

const SESSION: &str = "octoscode:main";
const TURN: &str = "00000000-0000-7000-8000-0000000000a1";
const APPROVAL: &str = "00000000-0000-7000-8000-0000000000b2";
const QUESTION: &str = "00000000-0000-7000-8000-0000000000c3";

// ------------------------------------------------------- request round-trips

#[tokio::test]
async fn turn_state_get_round_trips_with_the_web_params() {
    let turn = TurnId::new();
    let wire_turn = turn.0.to_string();
    let expect = wire_turn.clone();
    let tx = fake_transport(move |method, params| {
        assert_eq!(method, "turn/state/get");
        // The web sends `{session_id, turn_id}` (client.ts:491).
        assert_eq!(params["session_id"], SESSION);
        assert_eq!(params["turn_id"], expect);
        Ok(serde_json::json!({
            "session_id": SESSION,
            "turn_id": expect,
            "state": "active",
            "running": true,
        }))
    });
    let client = Client::new(tx);
    let wire_turn = turn.0;
    let params = octos_core::ui_protocol::TurnStateGetParams {
        session_id: SessionKey::new("octoscode", "main"),
        turn_id: turn,
    };
    let out = client.call::<TurnStateGet>(params).await.expect("round trip");
    assert_eq!(out.turn_id.0, wire_turn);
    assert_eq!(out.state.as_str(), "active");
    assert_eq!(out.running, Some(true));
}

#[tokio::test]
async fn turn_state_get_surfaces_an_rpc_error_with_the_method_name() {
    let tx = fake_transport(|_m, _p| {
        Err(RpcError { code: -32602, message: "unknown turn".into(), data: None })
    });
    let client = Client::new(tx);
    let params = octos_core::ui_protocol::TurnStateGetParams {
        session_id: SessionKey::new("octoscode", "main"),
        turn_id: TurnId::new(),
    };
    match client.call::<TurnStateGet>(params).await {
        Err(ClientError::Rpc { method, error }) => {
            assert_eq!(method, "turn/state/get");
            assert_eq!(error.code, -32602);
        }
        other => panic!("expected an RPC error, got {other:?}"),
    }
}

#[tokio::test]
async fn turn_steer_sends_the_web_input_shape_and_reads_the_receipt() {
    let expected = TURN.to_string();
    let expect = expected.clone();
    let tx = fake_transport(move |method, params| {
        assert_eq!(method, "turn/steer");
        // steer.ts:41-45 sends `{session_id, expected_turn_id, input:[{kind,text}]}`.
        assert_eq!(params["session_id"], SESSION);
        assert_eq!(params["expected_turn_id"], expect);
        assert_eq!(params["input"][0]["kind"], "text");
        assert_eq!(params["input"][0]["text"], "steer me");
        Ok(serde_json::json!({ "turn_id": expect, "steered": true }))
    });
    let client = Client::new(tx);
    let params = octoscode_client::domains::turn::TurnSteerParams {
        session_id: SESSION.to_string(),
        expected_turn_id: Some(expected),
        input: vec![octos_core::ui_protocol::InputItem::Text { text: "steer me".into() }],
    };
    let out = client.call::<TurnSteer>(params).await.expect("round trip");
    assert!(out.steered);
    assert_eq!(out.turn_id, TURN);
}

#[tokio::test]
async fn turn_steer_decodes_a_refused_steer_receipt() {
    let tx = fake_transport(|_m, _p| {
        // Server's live-turn mismatch path returns a fresh turn id, steered:false.
        Ok(serde_json::json!({
            "turn_id": "00000000-0000-7000-8000-0000000000d4",
            "steered": false
        }))
    });
    let client = Client::new(tx);
    let params = octoscode_client::domains::turn::TurnSteerParams {
        session_id: SESSION.to_string(),
        expected_turn_id: Some(TURN.to_string()),
        input: vec![octos_core::ui_protocol::InputItem::Text { text: "x".into() }],
    };
    let out = client.call::<TurnSteer>(params).await.expect("round trip");
    assert!(!out.steered, "a mismatched steer reports steered:false, not an error");
}

#[tokio::test]
async fn thread_graph_get_round_trips_with_the_web_params() {
    let tx = fake_transport(|method, params| {
        assert_eq!(method, "thread/graph/get");
        // inspection-binding sends `{session_id}` with `at` absent (current head).
        assert_eq!(params["session_id"], SESSION);
        assert!(params.get("at").is_none(), "no cursor for a current-head read");
        Ok(serde_json::json!({
            "session_id": SESSION,
            "cursor": { "stream": "s", "seq": 7 },
            "threads": [{
                "thread_id": "00000000-0000-7000-8000-0000000000e5",
                "root_seq": 1,
                "message_seqs": [1, 2, 3],
                "status": "completed"
            }],
            "orphans": []
        }))
    });
    let client = Client::new(tx);
    let params = octos_core::ui_protocol::ThreadGraphGetParams {
        session_id: SessionKey::new("octoscode", "main"),
        at: None,
    };
    let out = client.call::<ThreadGraphGet>(params).await.expect("round trip");
    assert_eq!(out.threads.len(), 1);
    assert_eq!(out.threads[0].message_seqs, vec![1, 2, 3]);
    assert_eq!(out.cursor.seq, 7);
    assert!(out.orphans.is_empty());
}

#[tokio::test]
async fn approval_scopes_list_round_trips_with_the_web_params() {
    let tx = fake_transport(|method, params| {
        assert_eq!(method, "approval/scopes/list");
        assert_eq!(params["session_id"], SESSION);
        Ok(serde_json::json!({
            "scopes": [{
                "session_id": SESSION,
                "scope": "shell",
                "scope_match": "prefix",
                "decision": "approve",
                "turn_id": TURN
            }]
        }))
    });
    let client = Client::new(tx);
    let params = octos_core::ui_protocol::ApprovalScopesListParams {
        session_id: SessionKey::new("octoscode", "main"),
    };
    let out = client.call::<ApprovalScopesList>(params).await.expect("round trip");
    assert_eq!(out.scopes.len(), 1);
    assert_eq!(out.scopes[0].scope, "shell");
    assert_eq!(out.scopes[0].decision.as_wire_str(), "approve");
}

#[tokio::test]
async fn user_question_respond_round_trips_with_the_web_params() {
    let expected = QUESTION.to_string();
    let expect = expected.clone();
    let tx = fake_transport(move |method, params| {
        assert_eq!(method, "user_question/respond");
        assert_eq!(params["session_id"], SESSION);
        assert_eq!(params["question_id"], expect);
        assert_eq!(params["answers"][0]["selected_labels"][0], "Yes");
        assert_eq!(params["answers"][0]["free_text"], "sure");
        Ok(serde_json::json!({
            "question_id": expect,
            "accepted": true,
            "runtime_resumed": true
        }))
    });
    let client = Client::new(tx);
    // Build the QuestionId by decoding the wire string the server would send
    // (`QuestionId(pub Uuid)` derives Deserialize), so the test never names
    // the `uuid` crate (not a dev-dependency of this crate).
    let question_id: QuestionId = serde_json::from_value(serde_json::json!(QUESTION))
        .expect("a uuid string decodes to a QuestionId");
    let params = octos_core::ui_protocol::UserQuestionRespondParams {
        session_id: SessionKey::new("octoscode", "main"),
        question_id,
        answers: vec![octos_core::ui_protocol::UserQuestionAnswer {
            selected_labels: vec!["Yes".into()],
            free_text: Some("sure".into()),
        }],
        client_note: None,
    };
    let out = client.call::<UserQuestionRespond>(params).await.expect("round trip");
    assert!(out.accepted);
    assert!(out.runtime_resumed);
}

#[tokio::test]
async fn review_start_sends_the_web_params_and_reads_the_receipt() {
    let expected = TURN.to_string();
    let expect = expected.clone();
    let tx = fake_transport(move |method, params| {
        assert_eq!(method, "review/start");
        // history.ts:259-264 sends `{session_id, turn_id, delivery:"inline", prompt?}`.
        assert_eq!(params["session_id"], SESSION);
        assert_eq!(params["turn_id"], expect);
        assert_eq!(params["delivery"], "inline");
        assert_eq!(params["prompt"], "focus on auth");
        Ok(serde_json::json!({
            "accepted": true,
            "session_id": SESSION,
            "turn_id": expect,
            "workflow": "code_review",
            "backend": "native",
            "agent_count": 3
        }))
    });
    let client = Client::new(tx);
    let params = octoscode_client::domains::review::ReviewStartParams {
        session_id: SESSION.to_string(),
        turn_id: expected,
        delivery: "inline".to_string(),
        prompt: Some("focus on auth".to_string()),
    };
    let out = client.call::<ReviewStart>(params).await.expect("round trip");
    assert!(out.accepted);
    assert_eq!(out.workflow, "code_review");
    assert_eq!(out.backend, "native");
    assert_eq!(out.agent_count, 3);
}

#[tokio::test]
async fn review_start_refusal_is_an_rpc_error_with_the_method_name() {
    let tx = fake_transport(|_m, _p| {
        Err(RpcError { code: -32000, message: "turn in progress".into(), data: None })
    });
    let client = Client::new(tx);
    let params = octoscode_client::domains::review::ReviewStartParams {
        session_id: SESSION.to_string(),
        turn_id: TURN.to_string(),
        delivery: "inline".to_string(),
        prompt: None,
    };
    match client.call::<ReviewStart>(params).await {
        Err(ClientError::Rpc { method, error }) => {
            assert_eq!(method, "review/start");
            assert_eq!(error.code, -32000);
        }
        other => panic!("expected an RPC error, got {other:?}"),
    }
}

// ------------------------------------------- notifications → store (fixtures)

/// A registry wired from every domain (the production `register_all` path),
/// over a fresh store.
fn wired() -> (Registry, Arc<Store>) {
    let store = Arc::new(Store::new());
    let mut reg = Registry::new();
    domains::register_all(&mut reg, store.clone());
    (reg, store)
}

#[test]
fn turn_steer_dropped_fixture_reaches_the_store() {
    let (mut reg, store) = wired();
    let n = notification(
        methods::TURN_STEER_DROPPED,
        serde_json::json!({
            "session_id": SESSION,
            "turn_id": TURN,
            "inputs": ["keep this", "and this"],
            "reason": "turn_ended"
        }),
    );
    assert!(reg.dispatch(&n), "the turn domain handles turn/steer_dropped");
    let dropped = store.domains.turn.dropped_steers();
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0].turn_id, TURN);
    assert_eq!(dropped[0].inputs, vec!["keep this", "and this"]);
    assert_eq!(dropped[0].reason, "turn_ended");
}

#[test]
fn approval_requested_fixture_creates_a_pending_row() {
    let (mut reg, store) = wired();
    let n = notification(
        methods::APPROVAL_REQUESTED,
        serde_json::json!({
            "session_id": SESSION,
            "approval_id": APPROVAL,
            "turn_id": TURN,
            "tool_name": "shell",
            "title": "Run command",
            "body": "rm -rf /tmp/x"
        }),
    );
    assert!(reg.dispatch(&n));
    let pending = store.domains.approval.pending();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, APPROVAL);
    assert_eq!(pending[0].target.as_deref(), Some("shell"));
    assert!(!pending[0].decided);
}

#[test]
fn approval_decided_fixture_settles_the_row() {
    let (mut reg, store) = wired();
    // Request first, then decide: the durable decision also replays alone.
    assert!(reg.dispatch(&notification(
        methods::APPROVAL_REQUESTED,
        serde_json::json!({
            "session_id": SESSION,
            "approval_id": APPROVAL,
            "turn_id": TURN,
            "tool_name": "shell",
            "title": "Run command",
            "body": "x"
        }),
    )));
    let n = notification(
        methods::APPROVAL_DECIDED,
        serde_json::json!({
            "session_id": SESSION,
            "approval_id": APPROVAL,
            "turn_id": TURN,
            "decision": "approve",
            "decided_at": "2026-09-28T00:00:00Z",
            "decided_by": "user",
            "auto_resolved": false
        }),
    );
    assert!(reg.dispatch(&n));
    let pending = store.domains.approval.pending();
    assert!(pending[0].decided, "a decided approval is no longer pending");
    assert!(!pending[0].auto_resolved, "the client decided it, not a policy");
}

#[test]
fn approval_auto_resolved_fixture_marks_the_row_auto_resolved() {
    let (mut reg, store) = wired();
    assert!(reg.dispatch(&notification(
        methods::APPROVAL_REQUESTED,
        serde_json::json!({
            "session_id": SESSION,
            "approval_id": APPROVAL,
            "turn_id": TURN,
            "tool_name": "shell",
            "title": "Run command",
            "body": "x"
        }),
    )));
    let n = notification(
        methods::APPROVAL_AUTO_RESOLVED,
        serde_json::json!({
            "session_id": SESSION,
            "approval_id": APPROVAL,
            "turn_id": TURN,
            "tool_name": "shell",
            "scope": "shell",
            "scope_match": "prefix",
            "decision": "approve"
        }),
    );
    assert!(reg.dispatch(&n));
    let pending = store.domains.approval.pending();
    assert!(pending[0].decided && pending[0].auto_resolved);
}

#[test]
fn approval_cancelled_fixture_cancels_the_row() {
    let (mut reg, store) = wired();
    assert!(reg.dispatch(&notification(
        methods::APPROVAL_REQUESTED,
        serde_json::json!({
            "session_id": SESSION,
            "approval_id": APPROVAL,
            "turn_id": TURN,
            "tool_name": "shell",
            "title": "Run command",
            "body": "x"
        }),
    )));
    let n = notification(
        methods::APPROVAL_CANCELLED,
        serde_json::json!({
            "session_id": SESSION,
            "approval_id": APPROVAL,
            "turn_id": TURN,
            "reason": "turn_interrupted"
        }),
    );
    assert!(reg.dispatch(&n));
    let pending = store.domains.approval.pending();
    assert!(pending[0].cancelled, "a cancelled approval is marked, not decided");
    assert!(!pending[0].decided);
}

#[test]
fn a_decision_for_an_unknown_id_is_ignored_not_fatal() {
    let (mut reg, store) = wired();
    // A durable decision can replay without the request on this connection.
    let n = notification(
        methods::APPROVAL_DECIDED,
        serde_json::json!({
            "session_id": SESSION,
            "approval_id": APPROVAL,
            "turn_id": TURN,
            "decision": "deny",
            "decided_at": "2026-09-28T00:00:00Z",
            "decided_by": "user"
        }),
    );
    assert!(reg.dispatch(&n), "handled, even though no row exists");
    assert!(store.domains.approval.pending().is_empty(), "nothing invented");
}

#[test]
fn f5_domains_register_their_methods_and_notifications() {
    let (reg, _store) = wired();
    // The 7 F5 methods' notification half + the approval lifecycle.
    assert!(reg.handles(methods::TURN_STEER_DROPPED));
    assert!(reg.handles(methods::APPROVAL_REQUESTED));
    assert!(reg.handles(methods::APPROVAL_DECIDED));
    assert!(reg.handles(methods::APPROVAL_CANCELLED));
    assert!(reg.handles(methods::APPROVAL_AUTO_RESOLVED));
}
