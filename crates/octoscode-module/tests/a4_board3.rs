//! A4 — the board-3 surfaces' production paths, at the wire.
//!
//! A scripted fake server (the f12/a3 pattern: one listener, a WebSocket for
//! the AppUI protocol, plus the two HTTP routes the web's file transfers use)
//! records every request's method AND params. Each test drives the SAME
//! functions the native UI calls — `board3::host::command` (the composer's
//! slash commands, `flow.rs::submit_draft`), `board3::host::perform` (every
//! mounted control's tap through the shared `taps` path), and
//! `board3::host::run` (the job the host spawns for an outcome) — then asserts
//! what reached the wire and what the dialog state now shows.
//!
//! Covered: runtime inventory (both modes + the live filter), the inspector
//! (`/threads`: typed `thread/graph/get` at current head + scopes), resume
//! (scoped `session/list`, exact-title gate, `session/open` with no replay
//! cursor), the switcher (fresh open), Fleet
//! (`profile/sub_providers/list`, prepare -> driver seat -> one dispatch,
//! steer), the strip's `session/status/read`, the reasoning effort on
//! `turn/start`, image uploads (`POST /api/upload` -> `turn/start` media),
//! and the transcript's system-notice / delivered-file rows.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::{Conversation, FlowEvent};
use octoscode_module::screens::board3::host::{self, Job, Outcome};
use octoscode_module::screens::board3::{fleetview, inventory, rows, strip};

const PROFILE: &str = "a4";

/// Every method the fake server advertises in its open reply.
const METHODS: &[&str] = &[
    "session/open",
    "session/list",
    "turn/start",
    "tool/status/list",
    "mcp/status/list",
    "thread/graph/get",
    "approval/scopes/list",
    "turn/state/get",
    "onboarding/workspace_list",
    "onboarding/workspace_create",
    "profile/sub_providers/list",
    "peer/prepare",
    "session/driver/get",
    "session/driver/acquire",
    "peer/dispatch",
    "peer/control",
    "session/status/read",
];

#[derive(Default)]
struct Seen {
    rpc: Vec<(String, Value)>,
    http: Vec<String>,
}

struct FakeServer {
    base_url: String,
    seen: Arc<Mutex<Seen>>,
}

fn result_for(method: &str, params: &Value) -> Value {
    let session = params["session_id"].as_str().unwrap_or("a4:main").to_owned();
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
                "supported_features": []
            }
        }}),
        "session/list" => json!({"sessions": [
            {"id": "a4:main", "message_count": 4, "updated_at": "2026-10-01T09:00:00Z"},
            {"id": "a4:alpha", "title": "Fix steer queue drop", "message_count": 3, "updated_at": "2026-10-01T10:00:00Z"},
            {"id": "bare-id", "title": "Unscoped row", "message_count": 1, "updated_at": "2026-09-30T10:00:00Z"}
        ]}),
        "tool/status/list" => json!({
            "session_id": session, "profile_id": PROFILE, "policy_id": "profile",
            "tools": [
                {"name": "apply_patch", "category": "edit", "status": "available", "policy": "allowed", "aliases": ["diff_edit"], "backend_tool": "native"},
                {"name": "bash", "category": "runtime", "status": "available", "policy": "approval_gated", "aliases": ["shell"], "backend_tool": "native"}
            ]
        }),
        "mcp/status/list" => json!({
            "session_id": session, "profile_id": PROFILE,
            "summary": {"connected": 1, "connecting": 0, "failed": 0, "disabled": 0},
            "servers": [{"id": "github", "transport": "stdio", "status": "connected", "tool_count": 9, "tools": ["search_issues"]}]
        }),
        // The recorded graph shape (r23-conversation line 12), with the
        // live server's control separator inside the stream key.
        "thread/graph/get" => json!({
            "cursor": {"seq": 278, "stream": "a4:main\u{0}~cwd-1"},
            "orphans": [],
            "session_id": session,
            "threads": [
                {"message_seqs": [0, 1], "root_seq": 0, "status": "unknown", "thread_id": "01920000-0000-7000-8000-00000000023b"},
                {"message_seqs": [2, 3], "root_seq": 2, "status": "unknown", "thread_id": "01920000-0000-7000-8000-00000000023c"}
            ]
        }),
        "approval/scopes/list" => json!({"scopes": []}),
        "onboarding/workspace_list" => match params["path"].as_str() {
            Some("/home/user/src/notes") => json!({
                "canonical_path": "/home/user/src/notes", "parent_path": "/home/user/src",
                "writable": true, "entries": [], "truncated": false, "hidden_skipped": 0
            }),
            _ => json!({
                "canonical_path": "/home/user/src", "parent_path": "/home/user", "writable": true,
                "entries": [{"name": "octos", "path": "/home/user/src/octos", "writable": true}],
                "truncated": false, "hidden_skipped": 2
            }),
        },
        "onboarding/workspace_create" => json!({"canonical_path": "/home/user/src/notes", "created": true}),
        "profile/sub_providers/list" => json!({"profile_id": PROFILE, "sub_providers": [
            {"key": "strong", "provider": "deepseek", "model": "deepseek-chat"}
        ]}),
        "peer/prepare" => json!({
            "slug": "review-diff", "topic": "peer-review-diff", "profile_id": PROFILE,
            "cwd": "/home/user/src/octos", "brief_path": "/home/user/.octos/peers/review-diff/brief.md"
        }),
        // A10: the web's external-driver shapes (`external-driver.ts`
        // `parseSessionDriverGetResult` / `parseDriverAcquireResult`,
        // `external-driver-peer-control.ts` receipts): an empty COMPLETE
        // inventory, a binding that names the acquiring driver, receipts that
        // echo the request's ids.
        "session/driver/get" => json!({
            "mode": "internal", "recovery": "none",
            "operations": {"items": [], "snapshot": "s1", "observed_revision": "0", "complete": true, "next_cursor": null}
        }),
        "session/driver/acquire" => json!({
            "control_token": "ctl-1", "recovery": "none",
            "binding": {"driver_id": params["driver_id"], "epoch": 3, "revision": 0, "lease_expires_at_ms": 1}
        }),
        "peer/dispatch" => json!({
            "operation_id": params["operation_id"], "state": "accepted", "model": "deepseek-chat",
            "model_lane": params["model"], "workspace_root": "/home/user/src/octos", "scoped_goal": null,
            "adopted_turn_id": "00000000-0000-4000-8000-0000000000d1",
            "adopted_session_id": format!("{session}#peer-review-diff"), "slug": "review-diff",
            "duplicate": false, "accepted_at_ms": 1, "payload_digest": "d"
        }),
        "peer/control" => json!({
            "operation_id": params["operation_id"], "state": "accepted",
            "target_operation_id": params["target_operation_id"], "expected_turn_id": params["expected_turn_id"],
            "target_session_id": format!("{session}#peer-review-diff"), "slug": "review-diff",
            "accepted_at_ms": 1, "payload_digest": "d", "duplicate": false
        }),
        "session/status/read" => json!({"session_id": session, "model": {"title": "DeepSeek V4 Flash", "model": "deepseek-v4-flash"}}),
        _ => json!({}),
    }
}

impl FakeServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let seen = Arc::new(Mutex::new(Seen::default()));
        let s2 = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let seen = s2.clone();
                tokio::spawn(async move { serve(stream, seen).await });
            }
        });
        Self { base_url, seen }
    }

    fn methods(&self) -> Vec<String> {
        self.seen.lock().unwrap().rpc.iter().map(|(m, _)| m.clone()).collect()
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().rpc.iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    fn http(&self) -> Vec<String> {
        self.seen.lock().unwrap().http.clone()
    }
}

/// One connection: the HTTP routes (`POST /api/upload`, `GET /api/files`)
/// answered by hand, anything else upgraded to the AppUI WebSocket.
async fn serve(mut stream: TcpStream, seen: Arc<Mutex<Seen>>) {
    let mut head = [0u8; 16];
    let n = stream.peek(&mut head).await.unwrap_or(0);
    let start = String::from_utf8_lossy(&head[..n]).to_string();
    if start.starts_with("POST /api/upload") || start.starts_with("GET /api/files") {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        // Read the head, then the body by Content-Length (reqwest sizes a
        // multipart form of known parts).
        let header_end = loop {
            let k = stream.read(&mut chunk).await.unwrap_or(0);
            if k == 0 {
                break None;
            }
            buf.extend_from_slice(&chunk[..k]);
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break Some(i + 4);
            }
        };
        let Some(header_end) = header_end else { return };
        let headers = String::from_utf8_lossy(&buf[..header_end]).to_string();
        let len = headers
            .lines()
            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
            .unwrap_or(0);
        while buf.len() < header_end + len {
            let k = stream.read(&mut chunk).await.unwrap_or(0);
            if k == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..k]);
        }
        let body = String::from_utf8_lossy(&buf[header_end..]).to_string();
        seen.lock().unwrap().http.push(format!("{}\n{}", headers, body));
        let (ctype, payload): (&str, Vec<u8>) = if start.starts_with("POST") {
            // A receipt in the server's handle grammar: `up/<b64url of
            // "<profile>/uploads/<name>">/<name>` (media.rs
            // `uploaded_handle_for_profile`).
            ("application/json", br#"["up/YTQvdXBsb2Fkcy9zaG90LnBuZw/shot.png"]"#.to_vec())
        } else {
            ("application/pdf", b"%PDF-1.7 delivered".to_vec())
        };
        let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", payload.len());
        let _ = stream.write_all(resp.as_bytes()).await;
        let _ = stream.write_all(&payload).await;
        return;
    }
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
    let (mut tx, mut rx_in) = ws.split();
    while let Some(Ok(msg)) = rx_in.next().await {
        let Message::Text(text) = msg else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        let method = v["method"].as_str().unwrap_or("").to_owned();
        if v.get("id").is_none() {
            continue; // a client notification
        }
        seen.lock().unwrap().rpc.push((method.clone(), v["params"].clone()));
        let frame = json!({"jsonrpc": "2.0", "id": v["id"].clone(), "result": result_for(&method, &v["params"])});
        let _ = tx.send(Message::Text(frame.to_string().into())).await;
    }
}

/// Connect + open, folding events until the open reply landed and the
/// connection reported Live.
async fn connected(server: &FakeServer) -> (Conversation, tokio::sync::mpsc::Receiver<octos_app_transport::TransportEvent>) {
    let (conv, mut events) =
        Conversation::connect(&server.base_url, "dummy", PROFILE, None, None).expect("connect");
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
    (conv, events)
}

/// Fold whatever the transport has queued (results the flow consumes).
async fn drain(conv: &Conversation, events: &mut tokio::sync::mpsc::Receiver<octos_app_transport::TransportEvent>) {
    while let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(150), events.recv()).await {
        let _ = conv.on_event(evt);
    }
}

/// The board-3 state, the media drafts and the env seams are process-global.
fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    host::reset();
    // The image drafts are kept per Session id, and every test opens "a4:main".
    if let Some(d) = octoscode_module::screens::media::drafts_for_session("a4:main") {
        for e in d.entries() {
            d.remove(&e.id);
        }
    }
    let dir = std::env::temp_dir().join(format!("a4-board3-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    std::env::set_var("OCTOSCODE_RECENTS_DIR", &dir);
    std::env::set_var("OCTOSCODE_SHOW_THINKING_FILE", dir.join("show-thinking.json"));
    g
}

/// Wait (bounded) until the server saw `n` requests of `method`: the
/// transport writes on its own task, so a fixed sleep races a loaded host.
async fn wait_for(server: &FakeServer, method: &str, n: usize) {
    for _ in 0..60 {
        if server.params_of(method).len() >= n {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn spawned(o: Option<Outcome>) -> Job {
    match o {
        Some(Outcome::Spawn(job)) => job,
        other => panic!("expected a spawned job, got {other:?}"),
    }
}

fn spawn_of(o: Outcome) -> Job {
    match o {
        Outcome::Spawn(job) => job,
        other => panic!("expected a spawned job, got {other:?}"),
    }
}

#[tokio::test]
async fn tools_reads_both_inventories_scoped_and_the_search_filters_without_a_reload() {
    let _g = lock();
    let server = FakeServer::start().await;
    let (conv, _ev) = connected(&server).await;
    let session = conv.session_id();

    let job = spawned(host::command("tools", "", &conv));
    assert_eq!(job, Job::InventoryLoad);
    host::run(job, &conv).await.expect("inventory load");

    let tools = server.params_of("tool/status/list");
    let mcp = server.params_of("mcp/status/list");
    assert_eq!(tools.len(), 1, "one tools read");
    assert_eq!(tools[0], json!({"session_id": session, "profile_id": PROFILE, "include_denied": true}));
    assert_eq!(mcp[0], json!({"session_id": session, "profile_id": PROFILE, "include_disabled": true}));
    {
        let st = host::state();
        let (policy, rows) = st.inv.tools.as_ref().expect("tools folded");
        assert_eq!(policy, "profile");
        assert_eq!(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["apply_patch", "bash"]);
        assert_eq!(st.inv.servers.as_ref().expect("servers folded").0[0].id, "github");
    }
    // Typing filters the mounted rows (visibility only, no new request).
    host::input_changed("inv.search", "shell");
    let vis = host::live_visibility(&conv.store);
    assert!(vis.contains(&("b3_inv_tool_0".to_owned(), false)));
    assert!(vis.contains(&("b3_inv_tool_1".to_owned(), true)), "the alias matched bash");
    assert!(vis.contains(&("b3_inv_servers_empty".to_owned(), true)));
    assert_eq!(server.params_of("tool/status/list").len(), 1, "filtering never re-reads");
    // The MCP tab only switches the order; Refresh re-reads both.
    assert_eq!(host::perform("b3.inv.tab.mcp", 0, &conv.store), Outcome::Done);
    assert_eq!(host::state().inv.tab, inventory::Tab::Mcp);
    assert_eq!(spawn_of(host::perform("b3.inv.refresh", 0, &conv.store)), Job::InventoryLoad);
}

#[tokio::test]
async fn threads_reads_the_graph_at_current_head_and_never_draws_a_control_char() {
    let _g = lock();
    let server = FakeServer::start().await;
    let (conv, _ev) = connected(&server).await;
    conv.store.set_capabilities(vec!["state.thread_graph.v1".into()]);

    let job = spawned(host::command("threads", "", &conv));
    host::run(job, &conv).await.expect("inspector load");
    let graph = server.params_of("thread/graph/get");
    assert_eq!(graph.len(), 1);
    assert!(graph[0].get("at").is_none(), "current head: no `at` on the wire ({})", graph[0]);
    assert_eq!(server.params_of("approval/scopes/list").len(), 1);
    let st = host::state();
    let g = st.insp.graph.as_ref().expect("graph folded");
    assert_eq!(g.threads.len(), 2);
    assert!(!g.cursor.chars().any(char::is_control), "sanitised cursor: {:?}", g.cursor);
    drop(st);
    // A malformed argument is refused locally — nothing reaches the wire.
    let before = server.methods().len();
    assert_eq!(host::command("turn", "not-a-uuid", &conv), Some(Outcome::Done));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(server.methods().len(), before, "a refused /turn sends nothing");
}

#[tokio::test]
async fn resume_lists_scoped_candidates_and_opens_only_the_exactly_confirmed_one_without_a_cursor() {
    let _g = lock();
    let server = FakeServer::start().await;
    let (conv, _ev) = connected(&server).await;

    let job = spawned(host::command("resume", "", &conv));
    host::run(job, &conv).await.expect("resume load");
    let scoped = server
        .params_of("session/list")
        .into_iter()
        .find(|p| p.get("profile_id").is_some())
        .expect("the scoped listing");
    assert_eq!(scoped, json!({"cwd": "/home/user/src/octos", "profile_id": PROFILE}));
    {
        let st = host::state();
        let ids: Vec<&str> = st.resume.candidates.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["a4:alpha", "bare-id"], "newest first; the current session is not a candidate");
        assert!(st.resume.candidates[1].blocked.is_some(), "an unscoped id is refused, never guessed");
    }
    // Blocked rows never arm; the confirm needs the exact title.
    host::perform("b3.resume.select", 1, &conv.store);
    assert_eq!(host::state().resume.selected, None);
    host::perform("b3.resume.select", 0, &conv.store);
    host::input_changed("resume.confirm", "fix steer queue drop");
    assert_eq!(host::perform("b3.resume.confirm", 0, &conv.store), Outcome::Done, "case differs: not armed");
    host::input_changed("resume.confirm", "Fix steer queue drop");
    let job = spawn_of(host::perform("b3.resume.confirm", 0, &conv.store));
    assert_eq!(job, Job::ResumeOpen("a4:alpha".into()));
    let opens_before = server.params_of("session/open").len();
    host::run(job, &conv).await.expect("resume open");
    wait_for(&server, "session/open", opens_before + 1).await;
    let opens = server.params_of("session/open");
    assert_eq!(opens.len(), opens_before + 1);
    let open = opens.last().unwrap();
    assert_eq!(open["session_id"], json!("a4:alpha"));
    assert_eq!(open["cwd"], json!("/home/user/src/octos"));
    assert!(open.get("after").is_none_or(Value::is_null), "no replay cursor: {open}");
    assert!(!server.methods().iter().any(|m| m == "turn/start"), "resuming never starts a turn");
}

#[tokio::test]
async fn the_switcher_opens_another_session_fresh_and_the_current_row_only_closes() {
    let _g = lock();
    let server = FakeServer::start().await;
    let (conv, _ev) = connected(&server).await;

    let job = spawned(host::command("sessions", "", &conv));
    host::run(job, &conv).await.expect("session list");
    let rows = octoscode_module::screens::board3::switcher::rows(&conv.store);
    let current = rows.iter().position(|r| r.current).expect("the current row");
    assert_eq!(host::perform("b3.switch.open", current, &conv.store), Outcome::Done);
    assert_eq!(host::open_dialog(), None, "selecting the current session just closes");
    let other = rows.iter().position(|r| r.id == "a4:alpha").expect("another row");
    let job = spawn_of(host::perform("b3.switch.open", other, &conv.store));
    let opens_before = server.params_of("session/open").len();
    host::run(job, &conv).await.expect("switch open");
    wait_for(&server, "session/open", opens_before + 1).await;
    let open = server.params_of("session/open").last().cloned().unwrap();
    assert_eq!(open["session_id"], json!("a4:alpha"));
    assert!(open.get("after").is_none_or(Value::is_null), "fresh: no replay cursor ({open})");
    assert_eq!(conv.session_id(), "a4:alpha");
}

#[tokio::test]
async fn fleet_start_seats_prepares_dispatches_once_and_steers_its_working_peer() {
    let _g = lock();
    let server = FakeServer::start().await;
    let (conv, mut ev) = connected(&server).await;
    conv.store.set_capabilities(vec!["external_driver_v1".into()]);
    octoscode_module::screens::fleet_driver::reset_seat();

    let job = spawn_of(host::perform("b3.open.fleet", 0, &conv.store));
    assert_eq!(job, Job::FleetLanes);
    host::run(job, &conv).await.expect("lanes + inventory");
    assert_eq!(server.params_of("profile/sub_providers/list")[0], json!({"profile_id": PROFILE}));
    assert_eq!(host::state().fleet.lanes[0].title(), "deepseek/deepseek-chat", "the form summarises the server's row");
    // A10: no implicit lane; a blank brief never starts.
    host::input_changed("fleet.brief", "Review the diff");
    assert_eq!(host::perform("b3.fleet.start", 0, &conv.store), Outcome::Done, "no lane chosen");
    host::perform("b3.fleet.lane", 0, &conv.store);
    host::input_changed("fleet.brief", "   ");
    assert_eq!(host::perform("b3.fleet.start", 0, &conv.store), Outcome::Done);
    host::input_changed("fleet.brief", "Review the diff");
    let job = spawn_of(host::perform("b3.fleet.start", 0, &conv.store));
    assert!(matches!(&job, Job::FleetStart { lane, brief, .. } if lane == "strong" && brief == "Review the diff"));
    host::run(job, &conv).await.expect("start");
    drain(&conv, &mut ev).await;
    let order: Vec<String> = server
        .methods()
        .into_iter()
        .filter(|m| m.starts_with("peer/") || m == "session/driver/acquire")
        .collect();
    // A10: Start IS the implicit acquisition — the seat comes FIRST
    // (fleet-start-sequencer.ts), then prepare, then EXACTLY ONE dispatch.
    assert_eq!(order, ["session/driver/acquire", "peer/prepare", "peer/dispatch"], "one dispatch after the seat");
    let dispatch = &server.params_of("peer/dispatch")[0];
    assert_eq!(dispatch["epoch"], json!(3));
    assert_eq!(dispatch["control_token"], json!("ctl-1"));
    assert_eq!(dispatch["model"], json!("strong"));
    assert_eq!(dispatch["dispatch"]["kind"], json!("new_brief"));
    let kickoff = dispatch["kickoff_input"][0]["text"].as_str().unwrap();
    assert!(kickoff.contains("Review the diff") && kickoff.contains("/home/user/.octos/peers/review-diff/brief.md"));
    // The adopted row reads Working; a steer goes to THAT operation and turn.
    let rows = fleetview::rows(&conv.store, octoscode_module::screens::peers::now_ms());
    assert_eq!(rows[0].status, fleetview::Status::Working);
    host::lower_open(&conv.store).expect("lowers");
    host::input_changed("fleet.steer#0", "Focus on tests");
    let job = spawn_of(host::perform("b3.fleet.steer", 0, &conv.store));
    host::run(job, &conv).await.expect("steer");
    let control = &server.params_of("peer/control")[0];
    assert_eq!(control["target_operation_id"], dispatch["operation_id"]);
    assert_eq!(control["expected_turn_id"], json!("00000000-0000-4000-8000-0000000000d1"));
    assert_eq!(control["command"]["input"][0]["text"], json!("Focus on tests"));
}

#[tokio::test]
async fn the_strip_reads_the_session_model_once_and_shows_it() {
    let _g = lock();
    let server = FakeServer::start().await;
    let (conv, _ev) = connected(&server).await;
    let session = conv.session_id();
    assert!(host::strip_status_needed(&session));
    assert!(!host::strip_status_needed(&session), "asked once per session");
    host::run(Job::StatusRead, &conv).await.expect("status read");
    assert_eq!(server.params_of("session/status/read")[0], json!({"session_id": session}));
    let (model, state, _perm) = strip::facts(&conv.store, &host::state().strip, None, None);
    assert_eq!(model, "DeepSeek V4 Flash");
    assert_eq!(state, "Ready");
}

#[tokio::test]
async fn the_chosen_thinking_effort_rides_the_next_turn_start() {
    let _g = lock();
    let server = FakeServer::start().await;
    let (conv, _ev) = connected(&server).await;
    assert_eq!(host::command("thinking", "", &conv), Some(Outcome::Done));
    assert_eq!(host::perform("b3.think.effort.high", 0, &conv.store), Outcome::Done);
    conv.ui().lock().unwrap().set_draft_inner("Which is larger?");
    let _ = conv.submit_draft().await;
    wait_for(&server, "turn/start", 1).await;
    let start = server.params_of("turn/start").last().cloned().expect("turn/start");
    assert_eq!(start["reasoning_effort"], json!("high"), "{start}");
    // Profile default omits the field.
    host::perform("b3.think.effort.default", 0, &conv.store);
    conv.ui().lock().unwrap().set_draft_inner("Again");
    let _ = conv.submit_draft().await;
    wait_for(&server, "turn/start", 2).await;
    let start = server.params_of("turn/start").last().cloned().unwrap();
    assert!(start.get("reasoning_effort").is_none_or(Value::is_null), "{start}");
}

#[tokio::test]
async fn images_upload_explicitly_then_ride_the_next_prompt() {
    let _g = lock();
    let server = FakeServer::start().await;
    let (conv, _ev) = connected(&server).await;
    let dir = std::env::temp_dir().join(format!("a4-img-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("shot.png");
    std::fs::write(&png, b"\x89PNG\r\n\x1a\n fake image bytes").unwrap();

    assert_eq!(host::command("images", "", &conv), Some(Outcome::Done));
    host::files_chosen(&[png.clone()]);
    assert!(server.http().is_empty(), "selecting a file does not upload it");
    let job = spawn_of(host::perform("b3.img.upload", 0, &conv.store));
    assert_eq!(job, Job::ImagesUpload);
    host::run(job, &conv).await.expect("upload");
    let http = server.http();
    assert_eq!(http.len(), 1, "one request per image");
    let req = http[0].to_ascii_lowercase();
    assert!(req.starts_with("post /api/upload"));
    assert!(req.contains("x-profile-id: a4") && req.contains("authorization: bearer dummy"));
    assert!(http[0].contains("filename=\"shot.png\""));
    conv.ui().lock().unwrap().set_draft_inner("What is in this image?");
    let _ = conv.submit_draft().await;
    wait_for(&server, "turn/start", 1).await;
    let start = server.params_of("turn/start").last().cloned().expect("turn/start");
    assert!(start.to_string().contains("up/YTQvdXBsb2Fkcy9zaG90LnBuZw/shot.png"), "the receipt rides the turn: {start}");
}

#[tokio::test]
async fn warnings_and_delivered_files_become_their_own_transcript_rows() {
    use octos_core::app_ui::AppUiBackendEvent as UiNotification;
    use octoscode_client::domains;
    use octoscode_client::registry::Registry;
    let _g = lock();
    let store = Arc::new(octoscode_store::Store::new());
    store.set_active(Some("dsflash:main".into()));
    let mut reg = Registry::new();
    domains::register_all(&mut reg, store.clone());
    let warning = UiNotification::from_method_and_params(
        "warning",
        json!({"session_id": "dsflash:main", "code": "server_restart", "message": "Server restarted; reconnecting the session."}),
    )
    .expect("warning decodes");
    assert!(reg.dispatch(&warning));
    // The recorded `assistant_persisted` envelope (live-turn-a6ea8505.jsonl),
    // carrying the delivered file in `meta.media` as the web model reads it.
    let persisted = UiNotification::from_method_and_params(
        "projection/envelope",
        json!({
            "cursor": {"seq": 25, "stream": "dsflash:main"},
            "payload": {"data": {
                "assistant_segment_id": "01a0e74a-1012-7335-b129-8004740ccc53:assistant:iteration:1",
                "meta": {"message_id": "dsflash:main:3:1790586786831488000", "persisted_at": "2026-09-28T09:13:06.831488Z",
                         "media": ["/home/user/src/octos/out/report.pdf"]},
                "text": "Five"
            }, "type": "assistant_persisted"},
            "seq": 3, "session_id": "dsflash:main",
            "thread_id": "01a0e74a-1012-7335-b129-8004740ccc53",
            "turn_id": "01a0e74a-1012-7335-b129-8004740ccc53"
        }),
    )
    .expect("envelope decodes");
    assert!(reg.dispatch(&persisted));
    let list = rows::timeline(&store, false);
    let notice = list.iter().find(|r| matches!(r, rows::TRow::Notice(_))).expect("a notice row");
    let file = list.iter().find(|r| matches!(r, rows::TRow::File(_))).expect("a file row");
    let n = rows::lower(notice, &store);
    assert!(n.contains("server_restart") && n.contains("Server restarted"), "{n}");
    let f = rows::lower(file, &store);
    assert!(f.contains("report.pdf") && f.contains("Download"), "{f}");
    assert!(!f.contains("/home/user/src/octos/out/report.pdf\"\n"), "never the raw path as body text");
    assert_eq!(rows::primary_action(file).as_deref().map(|a| a.starts_with("b3.file.download#")), Some(true));
}
