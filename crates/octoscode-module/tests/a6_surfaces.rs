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
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use octoscode_client::domains;
use octoscode_client::registry::Registry;
use octoscode_module::conv_layout::Metrics;
use octoscode_module::flow::{Conversation, FlowEvent, FlowUi};
use octoscode_module::screens::board3::rows;
use octoscode_module::screens::surfaces::{self, takeover, trajectory, Job, KeyOutcome, Outcome, Takeover};
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
fn d_opens_the_diff_review_only_for_a_typed_diff_approval() {
    let _g = lock();
    let mut body = recorded("r5-turn-a6ea8505.jsonl", "approval/requested").remove(0);
    let session = body["session_id"].as_str().unwrap().to_owned();
    let (store, mut reg) = wired(&session);
    reg.dispatch(&note("approval/requested", body.clone()));
    let ui = ui();
    let m = Metrics::for_window(990.0, true);
    // A command approval carries no preview: D maps, but opens nothing, and
    // the card offers no Review diff.
    assert_eq!(surfaces::key(&store, "d", false, false, false, false, false), KeyOutcome::Action("cv.approval.diff".into(), 0));
    assert_eq!(surfaces::perform("cv.approval.diff", 0, &store, &ui), Outcome::Done);
    assert!(!surfaces::lower_takeover(&store, &m).unwrap().dsl.contains("Review diff"));
    reg.dispatch(&note("approval/decided", recorded("r5-turn-a6ea8505.jsonl", "approval/decided").remove(0)));
    // A typed DIFF approval: its preview id is the payload's
    // `typed_details.diff.preview_id` (interaction.ts:94-102).
    body["approval_id"] = json!("01a0e773-f844-7d50-b171-b38d159f00ab");
    body["approval_kind"] = json!("diff");
    body["typed_details"] = json!({"kind": "diff", "diff": {
        "preview_id": "01920000-0000-7000-8000-0000000000f1", "operation": "apply_patch", "file_count": 1
    }});
    reg.dispatch(&note("approval/requested", body));
    let card = surfaces::lower_takeover(&store, &m).unwrap().dsl;
    assert!(card.contains("Review diff") && card.contains("Y / S / N / D"), "{card}");
    assert_eq!(
        surfaces::perform("cv.approval.diff", 0, &store, &ui),
        Outcome::ReviewDiff("01920000-0000-7000-8000-0000000000f1".into())
    );
    // Modifier chords never act (ApprovalPanel.tsx:36-43).
    for (shift, ctrl, alt, logo) in [(true, false, false, false), (false, true, false, false), (false, false, true, false), (false, false, false, true)] {
        let k = surfaces::key(&store, "d", shift, ctrl, alt, logo, false);
        assert!(matches!(k, KeyOutcome::Pass), "{shift} {ctrl} {alt} {logo}: {k:?}");
    }
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

/// The recorded turn `turn` of r23 (every inbound frame naming it), in order.
fn r23_turn(turn: &str) -> Vec<(String, Value)> {
    fixture("r23-conversation-a6ea8505.jsonl")
        .into_iter()
        .filter(|f| f["dir"] == "in" && f["body"]["turn_id"] == turn)
        .map(|f| (f["method"].as_str().unwrap().to_owned(), f["body"].clone()))
        .collect()
}

/// Shape of the composed transcript rows (for readable assertions).
fn shape(store: &Arc<Store>) -> Vec<String> {
    rows::timeline(store, false)
        .iter()
        .map(|r| match r {
            rows::TRow::Base(b) => b.kind.id().to_owned(),
            rows::TRow::FoldBar => "fold".into(),
            rows::TRow::Thinking(_) => "thinking".into(),
            rows::TRow::Notice(_) => "notice".into(),
            rows::TRow::File(_) => "file".into(),
            rows::TRow::Receipt(_) => "receipt".into(),
        })
        .collect()
}

#[test]
fn an_auto_resolved_approval_inside_a_recorded_turn_renders_its_notice_row() {
    let _g = lock();
    let turn = "01920000-0000-7000-8000-000000000242";
    let (store, mut reg) = wired("dsflash:main");
    for (method, body) in r23_turn(turn) {
        if method == "approval/decided" {
            continue;
        }
        let (method, body) = if method == "approval/requested" {
            // The policy resolves it instead (octos-core
            // `ApprovalAutoResolvedEvent`).
            (
                "approval/auto_resolved".to_owned(),
                json!({"session_id": body["session_id"], "approval_id": body["approval_id"], "turn_id": body["turn_id"],
                       "tool_name": body["tool_name"], "scope": "session", "scope_match": "exact", "decision": "approve"}),
            )
        } else {
            (method, body)
        };
        if let Ok(n) = UiNotification::from_method_and_params(&method, body) {
            reg.dispatch(&n);
        }
    }
    let s = shape(&store);
    assert!(s.iter().any(|x| x == "notice"), "the auto-resolve notice is a row: {s:?}");
}

// ======================================================= the fake server
//
// The a4_board3 pattern plus a PUSH channel: the scripted server answers
// every request (recording its method + params) and sends notifications on
// the live socket, so recorded frames reach the app through the real
// transport decoder -> `Conversation::on_event` -> the client registry (and
// the surfaces' own `observe`), exactly the app's path.

const PROFILE: &str = "dsflash";
/// The recordings' session (the fake opens `<profile>:main`).
const SESSION: &str = "dsflash:main";

const METHODS: &[&str] = &[
    "session/open",
    "turn/start",
    "turn/interrupt",
    "approval/respond",
    "user_question/respond",
    "task/list",
    "task/output/read",
    "task/cancel",
    "task/artifact/list",
    "task/artifact/read",
    "session/status/read",
];
const FEATURES: &[&str] = &["approval.typed.v1", "user_question.v1", "plan.todos.v1", "harness.task_artifacts.v1"];

/// `task/output/read`'s two pages and `task/artifact/read`'s two pages.
const PAGE_1: &str = "Compiling octos-cli v0.24.1\nRunning unittests src/lib.rs\n";
const PAGE_2: &str = "test steer_queue::keeps_order ... ok\n";
const ART_1: &str = "# Test report\n";
const ART_2: &str = "12 passed, 0 failed\n";

#[derive(Default)]
struct Seen {
    rpc: Vec<(String, Value)>,
}

#[derive(Clone, Default)]
struct FakeServer {
    base_url: String,
    seen: Arc<Mutex<Seen>>,
    /// Methods answered with a JSON-RPC error instead of a result.
    fail: Arc<Mutex<Vec<String>>>,
    /// The live socket's push channel.
    push: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<String>>>>,
}

impl FakeServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let srv = FakeServer { base_url: format!("http://{}", listener.local_addr().expect("addr")), ..Default::default() };
        let s2 = srv.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let s = s2.clone();
                tokio::spawn(async move { serve(stream, s).await });
            }
        });
        srv
    }

    fn fail(&self, method: &str) {
        self.fail.lock().unwrap().push(method.to_owned());
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().rpc.iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    /// Send one notification frame on the live socket.
    fn push(&self, method: &str, params: Value) {
        let frame = json!({"jsonrpc": "2.0", "method": method, "params": params}).to_string();
        let tx = self.push.lock().unwrap().clone().expect("a live connection");
        tx.send(frame).expect("the socket is open");
    }
}

fn result_for(method: &str, params: &Value) -> Value {
    let session = params["session_id"].as_str().unwrap_or(SESSION).to_owned();
    let task_id = params["task_id"].as_str().unwrap_or_default().to_owned();
    match method {
        "session/open" => json!({"opened": {
            "session_id": session,
            "active_profile_id": PROFILE,
            "workspace_root": "/home/user/src/octos",
            "cursor": {"stream": session, "seq": 1},
            "capabilities": {
                "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                "capabilities_schema_version": 1,
                "supported_methods": METHODS,
                "supported_notifications": ["turn/started"],
                "supported_features": FEATURES
            }
        }}),
        "approval/respond" => json!({"accepted": true, "approval_id": params["approval_id"]}),
        "user_question/respond" => json!({"accepted": true, "question_id": params["question_id"]}),
        // The RECORDED replies (c24b's task/list, r3's status), re-pointed
        // at the asking session like the replay server does.
        "task/list" => {
            let mut v = recorded("c24b-subagent-a6ea8505.jsonl", "task/list").remove(0);
            v["session_id"] = json!(session);
            v
        }
        "session/status/read" => {
            let mut v = fixture("r3-session-a6ea8505.jsonl")
                .into_iter()
                .find(|f| f["method"] == "res:session/status/read")
                .map(|f| f["body"].clone())
                .expect("r3 records a status reply");
            v.as_object_mut().unwrap().remove("capabilities");
            v["session_id"] = json!(session);
            v
        }
        "task/output/read" => {
            let at = params.pointer("/cursor/offset").and_then(|o| o.as_u64());
            let total = (PAGE_1.len() + PAGE_2.len()) as u64;
            let (text, from, complete) = match at {
                None | Some(0) => (PAGE_1, 0, false),
                Some(o) => (PAGE_2, o, true),
            };
            json!({
                "session_id": session, "task_id": task_id, "source": "runtime_projection",
                "cursor": {"offset": from}, "next_cursor": {"offset": from + text.len() as u64}, "text": text,
                "bytes_read": text.len(), "total_bytes": total.max(from + text.len() as u64), "truncated": !complete,
                "complete": complete, "live_tail_supported": true, "is_snapshot_projection": false,
                "task_status": "running", "runtime_state": "executing_tool", "lifecycle_state": "running"
            })
        }
        "task/artifact/list" => json!({
            "session_id": session, "task_id": task_id,
            "artifacts": [
                {"id": "art-1", "title": "test-report.md", "kind": "report", "status": "ready", "path": "artifacts/test-report.md"},
                {"id": "art-2", "title": "coverage.json", "kind": "data", "status": "ready"}
            ]
        }),
        "task/artifact/read" => {
            let first = params.pointer("/cursor/offset").is_none();
            json!({
                "session_id": session, "task_id": task_id,
                "artifact": {"id": params["artifact_id"], "title": "test-report.md", "kind": "report", "status": "ready"},
                "content": if first { ART_1 } else { ART_2 },
                "has_more": first,
                "next_cursor": if first { json!({"offset": ART_1.len()}) } else { Value::Null }
            })
        }
        "task/cancel" => json!({"task_id": task_id, "status": "cancelled"}),
        _ => json!({}),
    }
}

/// One connection: `GET /api/files` answered by hand, anything else
/// upgraded to the AppUI WebSocket (requests answered, pushes forwarded).
async fn serve(stream: TcpStream, srv: FakeServer) {
    let mut head = [0u8; 16];
    let n = stream.peek(&mut head).await.unwrap_or(0);
    if String::from_utf8_lossy(&head[..n]).starts_with("GET /api/files") {
        let mut stream = stream;
        let mut buf = vec![0u8; 8192];
        let _ = stream.read(&mut buf).await;
        let body = b"%PDF-1.7 delivered";
        let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: application/pdf\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
        let _ = stream.write_all(resp.as_bytes()).await;
        let _ = stream.write_all(body).await;
        return;
    }
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
    let (mut tx, mut rx_in) = ws.split();
    let (push_tx, mut push_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    *srv.push.lock().unwrap() = Some(push_tx);
    loop {
        tokio::select! {
            msg = rx_in.next() => {
                let Some(Ok(msg)) = msg else { return };
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                if v.get("id").is_none() {
                    continue; // a client notification
                }
                let method = v["method"].as_str().unwrap_or("").to_owned();
                srv.seen.lock().unwrap().rpc.push((method.clone(), v["params"].clone()));
                let frame = if srv.fail.lock().unwrap().contains(&method) {
                    json!({"jsonrpc": "2.0", "id": v["id"].clone(),
                           "error": {"code": -32010, "message": "The question is no longer pending"}})
                } else {
                    json!({"jsonrpc": "2.0", "id": v["id"].clone(), "result": result_for(&method, &v["params"])})
                };
                if tx.send(Message::Text(frame.to_string().into())).await.is_err() {
                    return;
                }
            }
            out = push_rx.recv() => {
                let Some(out) = out else { return };
                if tx.send(Message::Text(out.into())).await.is_err() {
                    return;
                }
            }
        }
    }
}

type Events = tokio::sync::mpsc::Receiver<octos_app_transport::TransportEvent>;

/// Connect + open, folding events until the open reply landed and the
/// connection reported Live.
async fn connected(server: &FakeServer) -> (Conversation, Events) {
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", PROFILE, None, None).expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    let mut opened = false;
    for _ in 0..80 {
        if opened && conv.store.is_live() {
            break;
        }
        match tokio::time::timeout(Duration::from_millis(200), events.recv()).await {
            Ok(Some(evt)) => {
                if matches!(conv.on_event(evt), FlowEvent::WorkspaceOpened(_)) {
                    opened = true;
                }
            }
            _ => break,
        }
    }
    assert_eq!(conv.store.active_session().as_deref(), Some(SESSION));
    (conv, events)
}

/// Fold whatever the transport has queued.
async fn drain(conv: &Conversation, events: &mut Events) {
    while let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(200), events.recv()).await {
        let _ = conv.on_event(evt);
    }
}

fn spawn_of(o: Outcome) -> Job {
    match o {
        Outcome::Spawn(job) => job,
        other => panic!("expected a spawned job, got {other:?}"),
    }
}

fn desktop() -> Metrics {
    Metrics::for_window(990.0, true)
}

/// The recorded `turn_terminal` envelope of an r23 turn.
fn r23_terminal(turn: &str) -> Value {
    r23_turn(turn)
        .into_iter()
        .find(|(m, b)| m == "projection/envelope" && b["payload"]["type"] == "turn_terminal")
        .map(|(_, b)| b)
        .expect("the turn's recorded terminal")
}

// ---------------------------------------------------------- the question

#[tokio::test]
async fn a_recorded_question_takes_the_composer_over_and_enter_sends_the_web_answer_shape() {
    let _g = lock();
    let server = FakeServer::start().await;
    let (conv, mut events) = connected(&server).await;
    let body = recorded("r23-conversation-a6ea8505.jsonl", "user_question/requested").remove(0);
    let qid = body["question_id"].as_str().unwrap().to_owned();
    server.push("user_question/requested", body);
    drain(&conv, &mut events).await;
    let store = &conv.store;
    assert_eq!(surfaces::takeover(store), Some(Takeover::Question(qid.clone())));
    // The card (UserQuestionPanel.tsx:70-81): the title, the question's
    // header + rule hint, every option, the Other field, and a DISABLED
    // primary that says why, next to what it will do.
    let card = surfaces::lower_takeover(store, &desktop()).expect("the card lowers").dsl;
    for fact in [
        "Octos needs a decision",
        "Which color would you like to pick?",
        "COLOR CHOICE",
        "Choose one, or write your own",
        "Blue",
        "Green",
        "Red",
        "Purple",
        "Type another answer",
        "Choose an option to continue",
        takeover::CONSEQUENCE,
    ] {
        assert!(card.contains(fact), "{fact} on the card");
    }
    assert_eq!(surfaces::submit_question(store), Outcome::Done, "nothing chosen: the primary does nothing");
    assert_eq!(surfaces::live_reason(store).as_deref(), Some("Choose an option to continue"));
    // Choose "Green" through the shared tap path (`cv.q.opt#<q*100+o>`),
    // then type an Other answer (no remount: the draft only).
    let ui = ui();
    assert_eq!(surfaces::perform("cv.q.opt", 1, store, &ui), Outcome::Done);
    let mounted = surfaces::lower_takeover(store, &desktop()).unwrap().dsl;
    surfaces::input_changed("cv.q.other#0", "  teal, actually  ");
    assert_eq!(surfaces::lower_takeover(store, &desktop()).unwrap().dsl, mounted, "typing never remounts the card");
    assert_eq!(surfaces::live_reason(store), None, "complete: the primary is live");
    assert!(surfaces::live_visibility(store).contains(&("cv_q_submit_box".to_owned(), true)));
    // Enter ANYWHERE in the card submits — the text input included.
    assert_eq!(
        surfaces::key(store, "Enter", false, false, false, false, true),
        KeyOutcome::Action("cv.q.submit".into(), 0)
    );
    let job = spawn_of(surfaces::perform("cv.q.submit", 0, store, &ui));
    assert_eq!(surfaces::live_reason(store).as_deref(), Some("Sending your answer…"), "busy says why");
    // The host's sync while the card shows (no focus move yet).
    assert!(!surfaces::take_focus_restore(true));
    surfaces::run(job, &conv).await.expect("accepted");
    // `toWireAnswers` (answers.ts:44-55) over user_question/respond, with
    // the request's OWN ids.
    assert_eq!(
        server.params_of("user_question/respond"),
        vec![json!({"session_id": SESSION, "question_id": qid,
                    "answers": [{"selected_labels": ["Green"], "free_text": "teal, actually"}]})]
    );
    assert_eq!(surfaces::takeover(store), None, "answered: the composer is back");
    // The keyboard was inside the card, so focus returns to the composer —
    // exactly once (focus-restore.ts:20-46).
    assert!(surfaces::take_focus_restore(false));
    assert!(!surfaces::take_focus_restore(false));
}

#[tokio::test]
async fn a_failed_answer_keeps_the_selection_and_the_typed_text_and_says_why() {
    let _g = lock();
    let server = FakeServer::start().await;
    server.fail("user_question/respond");
    let (conv, mut events) = connected(&server).await;
    let body = recorded("r23-conversation-a6ea8505.jsonl", "user_question/requested").remove(0);
    server.push("user_question/requested", body);
    drain(&conv, &mut events).await;
    let store = &conv.store;
    let ui = ui();
    surfaces::lower_takeover(store, &desktop()).expect("mounted");
    surfaces::perform("cv.q.opt", 0, store, &ui);
    surfaces::lower_takeover(store, &desktop()).expect("remounted with the selection");
    surfaces::input_changed("cv.q.other#0", "navy");
    let job = spawn_of(surfaces::submit_question(store));
    let err = surfaces::run(job, &conv).await.expect_err("the server refused");
    assert!(err.contains("no longer pending"), "{err}");
    assert!(surfaces::takeover(store).is_some(), "the card stays up");
    let st = surfaces::state().question.clone();
    assert_eq!(st.answers[0].selected, vec!["Blue".to_owned()]);
    assert_eq!(st.answers[0].free_text, "navy");
    // The remount that shows the failure CARRIES the typed text.
    let card = surfaces::lower_takeover(store, &desktop()).unwrap().dsl;
    assert!(card.contains("The question is no longer pending"), "the failure is on the card");
    assert!(card.contains("text: \"navy\""), "the Other field keeps its text");
    assert_eq!(surfaces::live_reason(store), None, "and it can be sent again");
}

#[test]
fn multi_select_arrows_move_focus_and_space_toggles_each_label_independently() {
    let _g = lock();
    let mut body = recorded("r23-conversation-a6ea8505.jsonl", "user_question/requested").remove(0);
    body["questions"][0]["multi_select"] = json!(true);
    let (store, mut reg) = wired(SESSION);
    reg.dispatch(&note("user_question/requested", body.clone()));
    let ui = ui();
    let card = surfaces::lower_takeover(&store, &desktop()).unwrap().dsl;
    assert!(card.contains("Choose any that apply, or write your own"));
    let key = |k: &str| surfaces::key(&store, k, false, false, false, false, false);
    // ArrowDown moves the focus inside the group; a checkbox never selects
    // on the move (UserQuestionPanel.tsx:106-121).
    assert_eq!(key("ArrowDown"), KeyOutcome::Swallow);
    assert_eq!(surfaces::state().question.focus, (0, 1));
    assert!(surfaces::state().question.answers[0].selected.is_empty());
    let KeyOutcome::Action(a, i) = key("Space") else { panic!("Space toggles") };
    surfaces::perform(&a, i, &store, &ui);
    // ArrowUp wraps at both ends (question-card.ts:46-53).
    key("ArrowUp");
    key("ArrowUp");
    assert_eq!(surfaces::state().question.focus, (0, 3));
    let KeyOutcome::Action(a, i) = key("Space") else { panic!("Space toggles") };
    surfaces::perform(&a, i, &store, &ui);
    assert_eq!(surfaces::state().question.answers[0].selected, vec!["Green".to_owned(), "Purple".to_owned()]);
    surfaces::perform("cv.q.opt", 1, &store, &ui);
    assert_eq!(surfaces::state().question.answers[0].selected, vec!["Purple".to_owned()], "only that label toggles");
    // Only the four arrows move; chords and typing never act.
    assert_eq!(key("Tab"), KeyOutcome::Pass);
    assert_eq!(surfaces::key(&store, "ArrowDown", false, true, false, false, false), KeyOutcome::Pass);
    assert_eq!(surfaces::key(&store, "ArrowDown", false, false, false, false, true), KeyOutcome::Pass);
    // A radio group selects as it moves.
    body["questions"][0]["multi_select"] = json!(false);
    body["question_id"] = json!("01a0eb8f-7b23-7030-9f26-a284864217a1");
    reg.dispatch(&note("user_question/requested", body));
    surfaces::lower_takeover(&store, &desktop()).unwrap();
    assert_eq!(surfaces::key(&store, "ArrowDown", false, false, false, false, false), KeyOutcome::Action("cv.q.opt".into(), 1));
}

// -------------------------------------------------------------- the plan

#[tokio::test]
async fn the_recorded_plan_shows_is_replaced_wholesale_and_drops_with_its_turn() {
    let _g = lock();
    let server = FakeServer::start().await;
    let (conv, mut events) = connected(&server).await;
    let plan = recorded("r23-conversation-a6ea8505.jsonl", "plan/updated").remove(0);
    let turn = plan["turn_id"].as_str().unwrap().to_owned();
    let started = r23_turn(&turn).into_iter().find(|(m, _)| m == "turn/started").expect("turn/started").1;
    server.push("turn/started", started);
    server.push("plan/updated", plan.clone());
    drain(&conv, &mut events).await;
    let store = &conv.store;
    let card = surfaces::lower_plan(store, &desktop()).expect("the plan card shows");
    for fact in ["Plan", "0 of 3 done", "Add the --version flag", "Wire --help to document it", "Add a test for the flag", "Pending", "Updated "] {
        assert!(card.contains(fact), "{fact} on the card");
    }
    // plan/updated REPLACES the checklist wholesale (plan.ts:20).
    let mut next = plan.clone();
    next["plan"]["items"] = json!([
        {"id": "1", "status": "completed", "title": "Add the --version flag"},
        {"id": "2", "status": "in_progress", "title": "Wire --help to document it"}
    ]);
    server.push("plan/updated", next);
    drain(&conv, &mut events).await;
    let card = surfaces::lower_plan(store, &desktop()).expect("still shown");
    assert!(card.contains("1 of 2 done"), "counts follow the new list");
    assert!(card.contains("Plan · Wire --help to document it"), "headline: the in-progress item");
    assert!(!card.contains("Add a test for the flag"), "never a merge");
    // Another turn's terminal leaves it; the authoring turn's drops it.
    server.push("projection/envelope", r23_terminal("01920000-0000-7000-8000-000000000242"));
    drain(&conv, &mut events).await;
    assert!(surfaces::lower_plan(store, &desktop()).is_some(), "turn-scoped: another turn ended");
    server.push("projection/envelope", r23_terminal(&turn));
    drain(&conv, &mut events).await;
    assert!(surfaces::lower_plan(store, &desktop()).is_none(), "dropped on its own turn's terminal");
}

#[test]
fn the_plan_card_fails_closed_without_the_feature_and_hides_behind_a_takeover() {
    let _g = lock();
    let plan = recorded("r23-conversation-a6ea8505.jsonl", "plan/updated").remove(0);
    let (store, mut reg) = wired(SESSION);
    reg.dispatch(&note("plan/updated", plan));
    assert!(surfaces::lower_plan(&store, &desktop()).is_some());
    store.set_capabilities(vec!["user_question.v1".into()]);
    assert!(surfaces::lower_plan(&store, &desktop()).is_none(), "not advertised: no plan surface");
    store.set_capabilities(vec!["user_question.v1".into(), "plan.todos.v1".into()]);
    let q = recorded("r23-conversation-a6ea8505.jsonl", "user_question/requested").remove(0);
    reg.dispatch(&note("user_question/requested", q));
    assert!(surfaces::takeover(&store).is_some());
    assert!(surfaces::lower_plan(&store, &desktop()).is_none(), "a takeover owns the composer");
}

// --------------------------------------------------------- the trajectory

#[tokio::test]
async fn the_trajectory_lists_recorded_tasks_and_status_and_merges_live_updates() {
    let _g = lock();
    let server = FakeServer::start().await;
    let (conv, mut events) = connected(&server).await;
    let store = &conv.store;
    let ui = ui();
    assert!(surfaces::tabs_available(store));
    let job = spawn_of(surfaces::perform("cv.tab.trajectory", 0, store, &ui));
    assert_eq!(job, Job::Refresh);
    surfaces::run(job, &conv).await.expect("refresh");
    assert_eq!(server.params_of("task/list"), vec![json!({"session_id": SESSION})]);
    // A15 — the status read names the Profile (`dsflash:main` embeds none).
    assert_eq!(server.params_of("session/status/read"), vec![json!({"session_id": SESSION, "profile_id": PROFILE})]);
    let pane = surfaces::lower_trajectory(store, 990.0, false).expect("the pane").dsl;
    for fact in ["Trajectory", "Session status", "deepseek-v4-flash", "workspace_write", "Background tasks", "c24b-probe completed", "running", "Cancel"] {
        assert!(pane.contains(fact), "{fact} in the pane");
    }
    // r4's live task/updated frames (re-pointed at this session) merge in:
    // the new row on top, its title from the event, then its state change.
    let live: Vec<Value> = recorded("r4-task-a6ea8505.jsonl", "task/updated")
        .into_iter()
        .map(|mut b| {
            b["session_id"] = json!(SESSION);
            b
        })
        .collect();
    server.push("task/updated", live[0].clone());
    drain(&conv, &mut events).await;
    let rows = store.domains.task.session_rows(SESSION);
    assert_eq!(rows.len(), 3);
    assert_eq!(trajectory::task_title(&rows[0]), "M9 task output fixture", "newest first");
    assert_eq!(rows[0].state, "running");
    server.push("task/updated", live[1].clone());
    drain(&conv, &mut events).await;
    let rows = store.domains.task.session_rows(SESSION);
    assert_eq!(rows.len(), 3, "an update merges into its row");
    assert_eq!(rows[0].state, "completed");
    let pane = surfaces::lower_trajectory(store, 990.0, false).unwrap().dsl;
    assert!(pane.contains("M9 task output fixture"));
    // Cancel: optimistic `cancelling`, then the server's status.
    let running = rows.iter().position(|t| t.state == "running").expect("the c24b running task");
    let job = spawn_of(surfaces::perform("cv.task.cancel", running, store, &ui));
    surfaces::run(job, &conv).await.expect("cancelled");
    let id = rows[running].id.clone();
    assert_eq!(server.params_of("task/cancel"), vec![json!({"task_id": id, "session_id": SESSION})]);
    assert_eq!(store.domains.task.snapshot(&id).unwrap().state, "cancelled");
}

#[tokio::test]
async fn task_output_pages_by_byte_cursor_appends_live_deltas_and_fails_closed_on_a_gap() {
    let _g = lock();
    let server = FakeServer::start().await;
    let (conv, mut events) = connected(&server).await;
    let store = &conv.store;
    let ui = ui();
    surfaces::run(spawn_of(surfaces::perform("cv.tab.trajectory", 0, store, &ui)), &conv).await.unwrap();
    let rows = store.domains.task.session_rows(SESSION);
    let i = rows.iter().position(|t| t.state == "running").unwrap();
    let id = rows[i].id.clone();
    let job = spawn_of(surfaces::perform("cv.task.open", i, store, &ui));
    assert_eq!(job, Job::OpenTask(id.clone()));
    surfaces::run(job, &conv).await.expect("opened");
    assert_eq!(
        server.params_of("task/output/read"),
        vec![json!({"session_id": SESSION, "task_id": id, "limit_bytes": 131072})]
    );
    assert!(surfaces::detail_open());
    let dialog = surfaces::lower_detail(store).expect("the detail").dsl;
    assert!(dialog.contains("SUPERVISED TASK") && dialog.contains("Compiling octos-cli v0.24.1"), "{dialog}");
    // A live delta at the expected byte offset appends (r4's recorded
    // task/output/delta, re-pointed at this task).
    let mut delta = recorded("r4-task-a6ea8505.jsonl", "task/output/delta").remove(0);
    delta["session_id"] = json!(SESSION);
    delta["task_id"] = json!(id);
    delta["cursor"]["offset"] = json!(PAGE_1.len());
    let live_text = delta["text"].as_str().unwrap().to_owned();
    server.push("task/output/delta", delta.clone());
    drain(&conv, &mut events).await;
    let expected = format!("{PAGE_1}{live_text}");
    assert_eq!(surfaces::traj().lock().unwrap().detail.text, expected);
    // The same frame again is stale (a no-op); one starting PAST the
    // expected offset fails closed, the text untouched (model.ts:143-174).
    server.push("task/output/delta", delta.clone());
    let mut gap = delta.clone();
    gap["cursor"]["offset"] = json!(expected.len() + 64);
    server.push("task/output/delta", gap);
    drain(&conv, &mut events).await;
    {
        let t = surfaces::traj().lock().unwrap();
        assert_eq!(t.detail.text, expected, "never corrupted");
        assert_eq!(t.detail.error.as_deref(), Some(trajectory::GAP_ERROR));
    }
    // Load more resynchronizes from the byte cursor.
    surfaces::run(spawn_of(surfaces::perform("cv.detail.more", 0, store, &ui)), &conv).await.expect("more");
    let reads = server.params_of("task/output/read");
    assert_eq!(reads[1], json!({"session_id": SESSION, "task_id": id, "cursor": {"offset": expected.len()}, "limit_bytes": 131072}));
    let t = surfaces::traj().lock().unwrap().detail.clone();
    assert_eq!(t.text, format!("{expected}{PAGE_2}"));
    assert_eq!(t.error, None);
    assert!(t.output.unwrap().complete);
    // Escape / × closes the detail.
    surfaces::perform("cv.detail.close", 0, store, &ui);
    assert!(!surfaces::detail_open());
}

#[tokio::test]
async fn task_artifacts_list_then_read_and_page_their_content() {
    let _g = lock();
    let server = FakeServer::start().await;
    let (conv, _events) = connected(&server).await;
    let store = &conv.store;
    let ui = ui();
    surfaces::run(spawn_of(surfaces::perform("cv.tab.trajectory", 0, store, &ui)), &conv).await.unwrap();
    let id = store.domains.task.session_rows(SESSION)[0].id.clone();
    surfaces::run(spawn_of(surfaces::perform("cv.task.open", 0, store, &ui)), &conv).await.unwrap();
    assert_eq!(server.params_of("task/artifact/list"), vec![json!({"session_id": SESSION, "task_id": id})]);
    let dialog = surfaces::lower_detail(store).unwrap().dsl;
    assert!(dialog.contains("test-report.md") && dialog.contains("coverage.json"), "the list");
    surfaces::run(spawn_of(surfaces::perform("cv.art.read", 0, store, &ui)), &conv).await.expect("read");
    assert_eq!(
        server.params_of("task/artifact/read")[0],
        json!({"session_id": SESSION, "task_id": id, "artifact_id": "art-1", "limit_bytes": 262144})
    );
    surfaces::run(spawn_of(surfaces::perform("cv.art.more", 0, store, &ui)), &conv).await.expect("more");
    assert_eq!(
        server.params_of("task/artifact/read")[1],
        json!({"session_id": SESSION, "task_id": id, "artifact_id": "art-1", "cursor": {"offset": ART_1.len()}, "limit_bytes": 262144})
    );
    let sel = surfaces::traj().lock().unwrap().detail.selected.clone().expect("selected");
    assert_eq!(sel.content, format!("{ART_1}{ART_2}"), "pages of the same artifact append");
    assert!(!sel.has_more);
    let dialog = surfaces::lower_detail(store).unwrap().dsl;
    assert!(dialog.contains("12 passed, 0 failed"));
}

// --------------------------------------------------------------- timeline

fn warning(code: &str, message: &str) -> UiNotification {
    note("warning", json!({"session_id": SESSION, "code": code, "message": message}))
}

#[test]
fn turn_errors_and_warnings_are_deterministic_readable_notices() {
    let _g = lock();
    let (store, mut reg) = wired(SESSION);
    let turn = "01920000-0000-7000-8000-000000000244";
    // The recorded turn's start, then it fails: turn/error AND the
    // canonical turn_terminal(errored) settle ONE notice (`terminal:<turn>`).
    for (m, b) in r23_turn(turn) {
        if m == "turn/started" {
            reg.dispatch(&note(&m, b));
        }
    }
    let err = json!({"session_id": SESSION, "turn_id": turn, "code": "provider_unavailable", "message": ""});
    reg.dispatch(&note("turn/error", err.clone()));
    let mut terminal = r23_terminal(turn);
    terminal["payload"]["data"]["outcome"] = json!("errored");
    terminal["payload"]["data"]["error"] = json!({"code": "provider_unavailable", "message": ""});
    if let Ok(n) = UiNotification::from_method_and_params("projection/envelope", terminal.clone()) {
        reg.dispatch(&n);
    }
    reg.dispatch(&note("turn/error", err)); // a durable replay
    let notices: Vec<_> = rows::timeline(&store, false).into_iter().filter(|r| matches!(r, rows::TRow::Notice(_))).collect();
    assert_eq!(notices.len(), 1, "one row per turn terminal");
    let dsl = rows::lower(&notices[0], &store);
    assert!(dsl.contains("Turn failed") && dsl.contains("Server error (provider_unavailable)."), "readable: {dsl}");
    // Warnings: one row each, ids by ordinal (`warning:<n>`).
    assert!(reg.dispatch(&warning("server_restart", "Server restarted; reconnecting the session.")));
    assert!(reg.dispatch(&warning("rate_limited", "")));
    let entries = store.domains.session.timeline.entries(SESSION);
    let ids: Vec<String> = entries
        .iter()
        .filter_map(|e| e.data.get("notice_id").and_then(|v| v.as_str()).map(str::to_owned))
        .collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[0], format!("terminal:{turn}"));
    assert!(ids[1].starts_with("warning:") && ids[2].starts_with("warning:") && ids[1] != ids[2], "{ids:?}");
    let all: Vec<String> = rows::timeline(&store, false)
        .iter()
        .filter(|r| matches!(r, rows::TRow::Notice(_)))
        .map(|r| rows::lower(r, &store))
        .collect();
    assert!(all.iter().any(|d| d.contains("Server restarted; reconnecting the session.")));
    assert!(all.iter().any(|d| d.contains("The server reported a warning.")), "an empty message reads as a sentence");
}

#[test]
fn a_delivered_file_is_one_attachment_row_however_often_it_is_announced() {
    let _g = lock();
    let (store, mut reg) = wired(SESSION);
    let turn = "01920000-0000-7000-8000-00000000023c";
    let tool_end = r23_turn(turn)
        .into_iter()
        .find(|(m, b)| m == "projection/envelope" && b["payload"]["type"] == "tool_end")
        .expect("the recorded tool_end")
        .1;
    let mut last_seq = 0;
    for (m, b) in r23_turn(turn) {
        last_seq = last_seq.max(b["seq"].as_u64().unwrap_or(0));
        if let Ok(n) = UiNotification::from_method_and_params(&m, b) {
            reg.dispatch(&n);
        }
    }
    // The tool's delivered file, built from the turn's own tool_end frame
    // (the next per-thread seq: a stale seq is dropped before any handler).
    let path = "/home/user/src/octos/out/r23-report.pdf";
    let mut file = tool_end.clone();
    file["payload"] = json!({"type": "file_attached", "data": {
        "path": path, "mime": "application/pdf", "size_bytes": 2048,
        "attachment_owner": {"tool_call_id": tool_end["payload"]["data"]["tool_call_id"]}
    }});
    file["seq"] = json!(last_seq + 1);
    reg.dispatch(&note("projection/envelope", file.clone()));
    // The same file again: in the persisted answer's `meta.media` (the
    // web's attachment source, `AttachmentList.tsx`), then re-announced.
    let mut persisted = tool_end.clone();
    persisted["payload"] = json!({"type": "assistant_persisted", "data": {
        "assistant_segment_id": format!("{turn}:assistant:iteration:1"),
        "meta": {"message_id": "dsflash:main:3:1790586786831488000", "persisted_at": "2026-09-28T09:13:06.831488Z", "media": [path]},
        "text": "The report is attached."
    }});
    persisted["seq"] = json!(last_seq + 2);
    reg.dispatch(&note("projection/envelope", persisted));
    file["seq"] = json!(last_seq + 3);
    reg.dispatch(&note("projection/envelope", file));
    let files: Vec<_> = rows::timeline(&store, false).into_iter().filter(|r| matches!(r, rows::TRow::File(_))).collect();
    assert_eq!(files.len(), 1, "one row per (turn, file)");
    let dsl = rows::lower(&files[0], &store);
    assert!(dsl.contains("r23-report.pdf") && dsl.contains("Download") && dsl.contains("2 KiB"), "{dsl}");
    assert!(!dsl.contains("/home/user/src/octos/out/r23-report.pdf\""), "the server path is never body text");
}

#[test]
fn fold_all_covers_every_block_and_prune_forgets_blocks_that_left() {
    let _g = lock();
    let (store, mut reg) = wired(SESSION);
    store.domains.session.set_show_reasoning(SESSION, true);
    for turn in ["01920000-0000-7000-8000-00000000023c", "01920000-0000-7000-8000-00000000023e"] {
        for (m, b) in r23_turn(turn) {
            if let Ok(n) = UiNotification::from_method_and_params(&m, b) {
                reg.dispatch(&n);
            }
        }
    }
    let s = shape(&store);
    assert_eq!(s.first().map(String::as_str), Some("fold"), "the fold bar heads the transcript: {s:?}");
    let ui = ui();
    let (thinking, tools) = surfaces::folds::live_ids(&store, SESSION);
    assert_eq!((thinking.len(), tools.len()), (2, 2), "two reasoning blocks, two tool calls");
    // The fold bar's controls are on the row.
    let bar = rows::lower(&rows::TRow::FoldBar, &store);
    assert!(bar.contains("cv.fold.expand_all") && bar.contains("cv.fold.collapse_all"));
    surfaces::perform("cv.fold.expand_all", 0, &store, &ui);
    assert_eq!(store.domains.session.thinking(SESSION).expanded, thinking);
    assert_eq!(ui.lock().unwrap().expanded_keys(), tools);
    surfaces::perform("cv.fold.collapse_all", 0, &store, &ui);
    assert!(store.domains.session.thinking(SESSION).expanded.is_empty());
    assert!(ui.lock().unwrap().expanded_keys().is_empty());
    // Expand again, then the blocks leave (another session's transcript):
    // the memory of blocks no longer shown is pruned.
    surfaces::perform("cv.fold.expand_all", 0, &store, &ui);
    assert!(surfaces::folds_changed(&store));
    store.set_active(Some("dsflash:other".into()));
    assert!(surfaces::folds_changed(&store), "a session switch is a shape change");
    assert!(surfaces::folds::prune(&store, &ui));
    assert!(ui.lock().unwrap().expanded_keys().is_empty(), "the tool memory left with its rows");
    store.set_active(Some(SESSION.into()));
    assert_eq!(store.domains.session.thinking(SESSION).expanded, thinking, "per-session reasoning memory kept");
}

#[test]
fn recorded_reasoning_folds_into_one_thinking_row_with_its_duration_and_words() {
    let _g = lock();
    let (store, mut reg) = wired(SESSION);
    store.domains.session.set_show_reasoning(SESSION, true);
    let turn = "01920000-0000-7000-8000-00000000023b";
    let mut words = String::new();
    for (m, b) in r23_turn(turn) {
        if b["payload"]["type"] == "reasoning_delta" {
            words.push_str(b["payload"]["data"]["text"].as_str().unwrap_or(""));
        }
        if let Ok(n) = UiNotification::from_method_and_params(&m, b) {
            reg.dispatch(&n);
        }
    }
    let think: Vec<_> = rows::timeline(&store, false).into_iter().filter(|r| matches!(r, rows::TRow::Thinking(_))).collect();
    assert_eq!(think.len(), 1, "the turn's reasoning is ONE folded block");
    let dsl = rows::lower(&think[0], &store);
    let n = words.split_whitespace().count();
    assert!(dsl.contains(&format!(" s · {n} words")), "duration + words summary: {n} words in\n{dsl}");
}
