//! A9 — Activity's production path, at the wire.
//!
//! A scripted fake AppUI server (the a4/f12 pattern) records every request
//! and answers `task/list` with the RECORDED replies of
//! `crates/octoscode-client/tests/fixtures/c24b-subagent-a6ea8505.jsonl`
//! (its running and its completed task snapshot), re-pointed at the asked
//! session. Each test drives the same functions the native UI calls —
//! `activity::perform` (the palette row's `activity.open`, the dialog's taps)
//! and `activity::read_once` / `refresh_loop` (the job the host spawns) —
//! then asserts what reached the wire and what the dialog shows.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::{Conversation, FlowEvent};
use octoscode_module::screens::activity::{self, Outcome};

const PROFILE: &str = "a9";

/// The recorded task/list replies (c24b): the first carries the running
/// snapshot, a later one the completed one.
fn recorded_tasks() -> (Vec<Value>, Vec<Value>) {
    let path = format!(
        "{}/../octoscode-client/tests/fixtures/c24b-subagent-a6ea8505.jsonl",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).expect("fixture");
    let replies: Vec<Vec<Value>> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["method"] == "task/list" && v["dir"] == "in")
        .map(|v| v["body"]["tasks"].as_array().cloned().unwrap_or_default())
        .filter(|t| !t.is_empty())
        .collect();
    // The recording's reply carries both snapshots (the running sub-agent
    // and an earlier completed one): each session gets one of them.
    let all: Vec<Value> = replies.into_iter().flatten().collect();
    let running: Vec<Value> = all.iter().filter(|x| x["state"] == "running").cloned().collect();
    let done: Vec<Value> = all.iter().filter(|x| x["state"] == "completed").cloned().collect();
    assert!(!running.is_empty() && !done.is_empty(), "the recording has both states");
    (running, done)
}

#[derive(Default)]
struct Seen {
    rpc: Vec<(String, Value)>,
    outstanding: usize,
    peak: usize,
}

struct FakeServer {
    base_url: String,
    seen: Arc<Mutex<Seen>>,
}

/// How the scripted server behaves for one test.
#[derive(Clone)]
struct Script {
    sessions: Vec<(String, String)>,
    advertise_task_list: bool,
    /// A delay on every task/list reply (to observe the batch concurrency).
    task_delay: Duration,
}

fn open_reply(session: &str, advertise: bool) -> Value {
    let mut methods = vec!["session/open", "session/list", "task/output/read"];
    if advertise {
        methods.push("task/list");
    }
    json!({"opened": {
        "session_id": session,
        "active_profile_id": PROFILE,
        "workspace_root": "/home/user/src/octos",
        "cursor": {"stream": session, "seq": 1},
        "capabilities": {
            "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
            "capabilities_schema_version": 1,
            "supported_methods": methods,
            "supported_notifications": ["turn/started"],
            "supported_features": []
        }
    }})
}

fn task_reply(session: &str) -> Result<Value, Value> {
    let (running, done) = recorded_tasks();
    let suffix = session.rsplit(':').next().unwrap_or("");
    let tasks = match suffix {
        "main" => running,
        "fork" => done,
        "bump" => {
            // Derived from the recorded running entry: the same snapshot in
            // the terminal `failed` state (no failed task was recorded).
            let mut t = running[0].clone();
            t["state"] = json!("failed");
            t["status"] = json!("failed");
            t["error"] = json!("cargo build: 2 errors");
            t["summary"] = json!("Bump octos-core to a6ea8505");
            vec![t]
        }
        // A reply that names another session: fail closed.
        "wrong" => return Ok(json!({"session_id": format!("{PROFILE}:private"), "tasks": running})),
        "bad" => return Err(json!({"code": -32603, "message": "task snapshot unavailable"})),
        _ => vec![],
    };
    Ok(json!({"session_id": session, "tasks": tasks}))
}

impl FakeServer {
    async fn start(script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let seen = Arc::new(Mutex::new(Seen::default()));
        let s2 = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let seen = s2.clone();
                let script = script.clone();
                tokio::spawn(async move { serve(stream, seen, script).await });
            }
        });
        Self { base_url, seen }
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().rpc.iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    fn asked_sessions(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .params_of("task/list")
            .iter()
            .map(|p| p["session_id"].as_str().unwrap_or("").to_owned())
            .collect();
        v.sort();
        v
    }
}

async fn serve(stream: TcpStream, seen: Arc<Mutex<Seen>>, script: Script) {
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
    let (tx, mut rx_in) = ws.split();
    let tx = Arc::new(tokio::sync::Mutex::new(tx));
    while let Some(Ok(msg)) = rx_in.next().await {
        let Message::Text(text) = msg else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        if v.get("id").is_none() {
            continue;
        }
        let method = v["method"].as_str().unwrap_or("").to_owned();
        let params = v["params"].clone();
        seen.lock().unwrap().rpc.push((method.clone(), params.clone()));
        let id = v["id"].clone();
        let session = params["session_id"].as_str().unwrap_or("a9:main").to_owned();
        let reply: Result<Value, Value> = match method.as_str() {
            "session/open" => Ok(open_reply(&session, script.advertise_task_list)),
            "session/list" => Ok(json!({"sessions": script.sessions.iter().map(|(id, title)| json!({
                "id": id, "title": title, "message_count": 2, "updated_at": "2026-10-01T09:00:00Z"
            })).collect::<Vec<_>>()})),
            "task/list" => task_reply(&session),
            _ => Ok(json!({})),
        };
        let frame = match reply {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
        };
        if method == "task/list" && !script.task_delay.is_zero() {
            {
                let mut s = seen.lock().unwrap();
                s.outstanding += 1;
                s.peak = s.peak.max(s.outstanding);
            }
            let tx = tx.clone();
            let seen = seen.clone();
            let delay = script.task_delay;
            tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                seen.lock().unwrap().outstanding -= 1;
                let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
            });
        } else {
            let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
        }
    }
}

async fn connected(server: &FakeServer) -> (Arc<Conversation>, tokio::sync::mpsc::Receiver<octos_app_transport::TransportEvent>) {
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
    (Arc::new(conv), events)
}

/// The activity state is process-global.
fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    activity::reset();
    g
}

fn board_sessions() -> Vec<(String, String)> {
    vec![
        ("a9:main".into(), "Fix steer queue drop on reconnect".into()),
        ("a9:fork".into(), "Add session fork".into()),
        ("a9:bump".into(), "Bump octos-core to a6ea8505".into()),
        ("a9:wrong".into(), "Review PR #2566".into()),
        ("a9:bad".into(), "Why is hydrate slow?".into()),
        // Another Profile's session is never a target.
        ("other:x".into(), "Foreign".into()),
    ]
}

#[tokio::test]
async fn activity_reads_every_confirmed_session_and_fails_closed_per_session() {
    let _g = lock();
    let server = FakeServer::start(Script { sessions: board_sessions(), advertise_task_list: true, task_delay: Duration::ZERO }).await;
    let (conv, _ev) = connected(&server).await;
    let opens_before = server.params_of("session/open").len();

    // The palette row's effect: `activity.open` -> a read job.
    let gen = match activity::perform(activity::ACTION_OPEN, 0, &conv.store) {
        Outcome::Read(g) => g,
        other => panic!("expected a read, got {other:?}"),
    };
    assert!(activity::read_once(&conv, gen).await, "published");

    // One task/list per confirmed session of THIS Profile; nothing opened.
    assert_eq!(server.asked_sessions(), ["a9:bad", "a9:bump", "a9:fork", "a9:main", "a9:wrong"]);
    assert_eq!(server.params_of("session/open").len(), opens_before, "the scan never opens a session");
    for p in server.params_of("task/list") {
        assert_eq!(p.as_object().unwrap().len(), 1, "only {{session_id}}: {p}");
    }
    {
        let st = activity::state();
        assert_eq!(st.unavailable.iter().map(String::as_str).collect::<Vec<_>>(), ["a9:wrong", "a9:bad"]);
        assert_eq!(st.error.as_deref(), Some("2 Session task snapshots unavailable"));
        assert!(!st.tasks.contains_key("a9:wrong"), "a wrong-session snapshot is never shown");
        assert_eq!(st.labels.get("a9:fork").map(String::as_str), Some("Add session fork"));
    }
    // The dialog: running first, then failed, then done; the counts.
    let low = activity::lower(&conv.store).expect("open");
    let shown: Vec<(String, String)> = activity::state()
        .shown
        .iter()
        .map(|r| (r.session_id.clone(), r.state.id().to_owned()))
        .collect();
    assert_eq!(
        shown,
        [
            ("a9:main".to_owned(), "running".to_owned()),
            ("a9:bump".to_owned(), "failed".to_owned()),
            ("a9:fork".to_owned(), "done".to_owned()),
        ]
    );
    for label in ["All 3", "Running 1", "Failed 1", "Done 1", "2 Session task snapshots unavailable"] {
        assert!(low.dsl.contains(label), "{label} in the dialog");
    }
    // The recorded completed snapshot keeps its recorded summary.
    assert!(low.dsl.contains("c24b-probe completed"));
    activity::close();
}

#[tokio::test]
async fn reads_are_batched_four_at_a_time_without_a_session_wall() {
    let _g = lock();
    let sessions: Vec<(String, String)> = (0..15).map(|i| (format!("a9:s{i}"), format!("Session {i}"))).collect();
    let server = FakeServer::start(Script { sessions, advertise_task_list: true, task_delay: Duration::from_millis(60) }).await;
    let (conv, _ev) = connected(&server).await;
    let gen = match activity::perform(activity::ACTION_OPEN, 0, &conv.store) {
        Outcome::Read(g) => g,
        other => panic!("expected a read, got {other:?}"),
    };
    assert!(activity::read_once(&conv, gen).await);
    // The 15 listed sessions plus the opened one (`a9:main`, confirmed by
    // its own session/open) — no eight-session wall.
    assert_eq!(server.asked_sessions().len(), 16, "no eight-session wall");
    let peak = server.seen.lock().unwrap().peak;
    assert!(peak <= 4, "at most four reads in flight (peak {peak})");
    assert!(peak >= 2, "the batch is concurrent (peak {peak})");
    activity::close();
}

#[tokio::test]
async fn the_refresh_runs_only_while_open_and_a_closed_dialog_never_publishes() {
    let _g = lock();
    let server = FakeServer::start(Script { sessions: board_sessions(), advertise_task_list: true, task_delay: Duration::ZERO }).await;
    let (conv, _ev) = connected(&server).await;
    let gen = match activity::perform(activity::ACTION_OPEN, 0, &conv.store) {
        Outcome::Read(g) => g,
        other => panic!("expected a read, got {other:?}"),
    };
    let loop_task = tokio::spawn(activity::refresh_loop(conv.clone(), gen, Duration::from_millis(150)));
    tokio::time::sleep(Duration::from_millis(520)).await;
    let reads_open = activity::state().reads;
    assert!(reads_open >= 3, "it re-read while open ({reads_open})");
    // The close tap ends the loop; nothing more reaches the wire.
    assert_eq!(activity::perform(activity::ACTION_CLOSE, 0, &conv.store), Outcome::Done);
    tokio::time::timeout(Duration::from_secs(2), loop_task).await.expect("the loop ends").unwrap();
    let asked = server.params_of("task/list").len();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(server.params_of("task/list").len(), asked, "no read after close");
}

#[tokio::test]
async fn an_unadvertised_task_list_is_never_sent() {
    let _g = lock();
    let server = FakeServer::start(Script { sessions: board_sessions(), advertise_task_list: false, task_delay: Duration::ZERO }).await;
    let (conv, _ev) = connected(&server).await;
    assert_eq!(activity::perform(activity::ACTION_OPEN, 0, &conv.store), Outcome::Done);
    let low = activity::lower(&conv.store).expect("open");
    assert!(low.dsl.contains("This server does not advertise task snapshots."));
    assert!(low.dsl.contains("Read-only · snapshots unavailable"));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(server.params_of("task/list").is_empty());
    activity::close();
}

#[tokio::test]
async fn a_row_opens_its_owning_session_through_the_sidebar_path() {
    let _g = lock();
    let server = FakeServer::start(Script { sessions: board_sessions(), advertise_task_list: true, task_delay: Duration::ZERO }).await;
    let (conv, _ev) = connected(&server).await;
    let gen = match activity::perform(activity::ACTION_OPEN, 0, &conv.store) {
        Outcome::Read(g) => g,
        other => panic!("expected a read, got {other:?}"),
    };
    activity::read_once(&conv, gen).await;
    activity::lower(&conv.store);
    // Row 1 is a9:bump's failed task: Open session -> its store index (the
    // `thread.open` the sidebar's rows use).
    let index = conv.store.sessions().iter().position(|s| s.id == "a9:bump").unwrap();
    assert_eq!(
        activity::perform(activity::ACTION_ROW, 1, &conv.store),
        Outcome::OpenSession { index, session: "a9:bump".into() }
    );
    assert!(!activity::is_open(), "the dialog closes on its action");
    // The current session's row inspects instead (task/output/read advertised).
    let gen = match activity::perform(activity::ACTION_OPEN, 0, &conv.store) {
        Outcome::Read(g) => g,
        other => panic!("expected a read, got {other:?}"),
    };
    activity::read_once(&conv, gen).await;
    activity::lower(&conv.store);
    assert_eq!(
        activity::perform(activity::ACTION_ROW, 0, &conv.store),
        Outcome::Action("dialog.open.tasks".into())
    );
}
