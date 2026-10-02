//! A6 — the conversation surfaces' production paths, over RECORDED and
//! fixture traffic.
//!
//! Notifications are the committed recordings' own frames
//! (`crates/octoscode-client/tests/fixtures/*.jsonl`), decoded by octos-core
//! (`AppUiBackendEvent::from_method_and_params`, the transport's decoder) and
//! dispatched through the client's production handler registry
//! (`domains::register_all`) into the store — then read back through the SAME
//! functions the mounted UI calls (`surfaces::takeover`, `lower_takeover`,
//! `perform`, `board3::rows::timeline` + `lower`). Requests go through a real
//! `Conversation` against a scripted fake server that records every method
//! and params (the a4_board3 pattern), so what reached the wire is asserted.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_client::domains;
use octoscode_client::registry::Registry;
use octoscode_module::flow::{Conversation, FlowEvent, FlowUi};
use octoscode_module::screens::board3::rows;
use octoscode_module::screens::surfaces::{self, Job, Outcome, Takeover};
use octoscode_store::Store;

// ------------------------------------------------------------ recordings

fn fixture(name: &str) -> Vec<Value> {
    let path = format!("{}/../octoscode-client/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("fixture line is JSON"))
        .collect()
}

/// The recorded inbound frames of `method` in `file`, in order.
fn recorded(file: &str, method: &str) -> Vec<Value> {
    fixture(file)
        .into_iter()
        .filter(|f| f["dir"] == "in" && f["method"] == method)
        .map(|f| f["body"].clone())
        .collect()
}

fn note(method: &str, params: Value) -> UiNotification {
    UiNotification::from_method_and_params(method, params).unwrap_or_else(|e| panic!("{method} decodes: {e:?}"))
}

/// A store with the client's production handlers, the session active and
/// the recorded capabilities (every recording advertises the same set).
fn wired(session: &str) -> (Arc<Store>, Registry) {
    let store = Arc::new(Store::new());
    store.set_active(Some(session.to_owned()));
    store.set_connection("Live".into(), true);
    store.domains.config.set_supported_methods(
        [
            "approval/respond", "user_question/respond", "task/list", "task/output/read", "task/cancel",
            "task/artifact/list", "task/artifact/read", "session/status/read",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
    );
    store.set_capabilities(
        ["approval.typed.v1", "user_question.v1", "plan.todos.v1", "harness.task_artifacts.v1"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    );
    let mut reg = Registry::new();
    domains::register_all(&mut reg, store.clone());
    (store, reg)
}

/// The surfaces' state is process-global; one test at a time.
fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    surfaces::reset();
    g
}

fn ui() -> Arc<Mutex<FlowUi>> {
    Arc::new(Mutex::new(FlowUi::default()))
}

// ---------------------------------------------------------- the approval

#[test]
fn a_recorded_typed_approval_takes_the_composer_over_with_every_fact() {
    let _g = lock();
    let body = recorded("r5-turn-a6ea8505.jsonl", "approval/requested").remove(0);
    let session = body["session_id"].as_str().unwrap().to_owned();
    let (store, mut reg) = wired(&session);
    assert!(reg.dispatch(&note("approval/requested", body.clone())));
    let id = body["approval_id"].as_str().unwrap();
    assert_eq!(surfaces::takeover(&store), Some(Takeover::Approval(id.to_owned())));
    let m = octoscode_module::conv_layout::Metrics::for_window(990.0, true);
    let card = surfaces::lower_takeover(&store, &m).expect("the card lowers").dsl;
    // title / body-or-command / risk / tool / kind (ApprovalPanel.tsx:58-87)
    for fact in ["M9 approval fixture", "low risk", "shell · command", "printf m9-approval-e2e", "Y / S / N"] {
        assert!(card.contains(fact), "{fact} on the card");
    }
    // The typed command: command_line first, else argv joined.
    let mut argv_only = body.clone();
    argv_only["typed_details"]["command"].as_object_mut().unwrap().remove("command_line");
    argv_only["approval_id"] = json!("01a0e773-f844-7d50-b171-b38d159f0001");
    argv_only["session_id"] = json!("other:session");
    reg.dispatch(&note("approval/requested", argv_only));
    let d = store.domains.approval.detail("01a0e773-f844-7d50-b171-b38d159f0001").unwrap();
    assert_eq!(d.command.as_deref(), Some("printf m9-approval-e2e"), "argv joined");
    // Session-scoped: the other session's approval never shows here.
    assert_eq!(surfaces::takeover(&store), Some(Takeover::Approval(id.to_owned())));
}

#[test]
fn buttons_and_keys_decide_with_the_web_scope_and_lifecycle_frames_clear_the_card() {
    let _g = lock();
    let body = recorded("r5-turn-a6ea8505.jsonl", "approval/requested").remove(0);
    let session = body["session_id"].as_str().unwrap().to_owned();
    let id = body["approval_id"].as_str().unwrap().to_owned();
    let (store, mut reg) = wired(&session);
    reg.dispatch(&note("approval/requested", body.clone()));
    let ui = ui();
    // Each control maps to the web's (decision, scope) (ApprovalPanel.tsx:100-125).
    for (action, decision, scope) in [
        ("cv.approval.once", "approve", "request"),
        ("cv.approval.session", "approve", "session"),
        ("cv.approval.deny", "deny", "request"),
    ] {
        surfaces::reset();
        match surfaces::perform(action, 0, &store, &ui) {
            Outcome::Spawn(Job::Approve { approval_id, decision: d, scope: s, session_id }) => {
                assert_eq!((approval_id.as_str(), d.as_str(), s.as_str(), session_id.as_str()), (id.as_str(), decision, scope, session.as_str()));
            }
            other => panic!("{action}: {other:?}"),
        }
    }
    // The same decisions from the keyboard (Y/S/N; chords never act).
    surfaces::reset();
    use surfaces::KeyOutcome;
    assert_eq!(surfaces::key(&store, "y", false, false, false, false, false), KeyOutcome::Action("cv.approval.once".into(), 0));
    assert_eq!(surfaces::key(&store, "s", false, false, false, false, false), KeyOutcome::Action("cv.approval.session".into(), 0));
    assert_eq!(surfaces::key(&store, "n", false, false, false, false, false), KeyOutcome::Action("cv.approval.deny".into(), 0));
    assert_eq!(surfaces::key(&store, "y", false, true, false, false, false), KeyOutcome::Pass);
    // The RECORDED approval/decided settles the card.
    let decided = recorded("r5-turn-a6ea8505.jsonl", "approval/decided").remove(0);
    assert_eq!(decided["approval_id"], body["approval_id"]);
    reg.dispatch(&note("approval/decided", decided));
    assert_eq!(surfaces::takeover(&store), None, "decided: the composer is back");
}

#[test]
fn cancel_and_auto_resolve_update_the_ui_and_auto_resolve_leaves_a_deterministic_notice() {
    let _g = lock();
    let body = recorded("r23-conversation-a6ea8505.jsonl", "approval/requested").remove(0);
    let session = body["session_id"].as_str().unwrap().to_owned();
    let (store, mut reg) = wired(&session);
    reg.dispatch(&note("approval/requested", body.clone()));
    assert!(surfaces::takeover(&store).is_some());
    // approval/cancelled (the server cancelled it, e.g. the turn ended).
    reg.dispatch(&note(
        "approval/cancelled",
        json!({"session_id": session, "approval_id": body["approval_id"], "turn_id": body["turn_id"], "reason": "turn_interrupted"}),
    ));
    assert_eq!(surfaces::takeover(&store), None, "cancelled: no card");
    // approval/auto_resolved: a policy decided — no card ever, one notice.
    let auto = json!({
        "session_id": session, "approval_id": "01a0eb93-10a9-7520-a1c0-028df331ecea",
        "turn_id": "01920000-0000-7000-8000-000000000242", "tool_name": "bash",
        "scope": "session", "scope_match": "exact", "decision": "approve"
    });
    assert!(reg.dispatch(&note("approval/auto_resolved", auto.clone())));
    reg.dispatch(&note("approval/auto_resolved", auto)); // the durable replay
    let notices: Vec<_> = rows::timeline(&store, false)
        .into_iter()
        .filter(|r| matches!(r, rows::TRow::Notice(_)))
        .collect();
    assert_eq!(notices.len(), 1, "a replay updates the same row (approval:<id>)");
    let dsl = rows::lower(&notices[0], &store);
    assert!(dsl.contains("Auto-approved") && dsl.contains("bash · matched the session scope"), "{dsl}");
}
