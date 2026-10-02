//! A8 — the session-config production paths, at the wire.
//!
//! A scripted fake server (the a4/f12 pattern: one listener, the AppUI
//! WebSocket) records every request's method AND params, and answers in the
//! recorded a6ea8505 shapes (`session/status/read` r3-session line 8,
//! `permission/profile/list|set` r2-profile lines 6/18, `profile/llm/list|
//! select` r2-profile lines 12/36). Each test drives the SAME functions the
//! native UI calls — `board3::host::perform` (every mounted control's tap
//! through the shared `taps` path, the strip's included) and
//! `board3::host::run` (the job the host spawns) — or `Conversation::new_chat`
//! (the one creation path every "New chat" control takes), then asserts what
//! reached the wire and what the pane now shows.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::{Conversation, FlowEvent};
use octoscode_module::screens::board3::host::{self, Dialog, Job, Outcome};
use octoscode_module::screens::board3::session_pane::{self, SaveState};
use octoscode_module::screens::driver_discovery::{self as dd, ErrorReason, Inventory};
use octoscode_module::screens::recents::MemoryStore;
use octoscode_module::screens::session_defaults as sd;

const PROFILE: &str = "a8";

const METHODS: &[&str] = &[
    "session/open",
    "session/list",
    "session/status/read",
    "turn/start",
    "permission/profile/list",
    "permission/profile/set",
    "profile/llm/list",
    "profile/llm/select",
    "session/driver/get",
    "session/driver/acquire",
    "session/driver/release",
    "thread/graph/get",
    "approval/scopes/list",
    "turn/state/get",
    "session/hydrate",
    "session/delete",
    "launch/resolve",
    "profile/local/create",
];

/// What the server answers (the knobs a test turns).
#[derive(Default)]
struct Script {
    held: bool,
    no_sandbox_feature: bool,
    /// `session/driver/get` fails with this `data.kind`.
    driver_refusal: Option<String>,
    /// The next N `permission/profile/set` calls fail.
    fail_sets: usize,
    /// `session/hydrate` answers an empty history.
    empty_history: bool,
    /// `session/list` rows (the resume catalog).
    list_rows: Vec<Value>,
    /// Sessions whose `session/hydrate` comes back empty.
    empty_for: Vec<String>,
    /// Hold `session/hydrate` replies back this long (ms).
    hydrate_delay_ms: u64,
    /// `launch/resolve`'s reply; `None` = the launch probe is not advertised.
    launch: Option<Value>,
    /// Hold `launch/resolve` replies back this long (ms).
    launch_delay_ms: u64,
    mode: String,
    network: String,
    approval: String,
    model: String,
}

struct FakeServer {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
    script: Arc<Mutex<Script>>,
}

impl FakeServer {
    async fn start(script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let script = Arc::new(Mutex::new(Script {
            mode: "workspace_write".into(),
            network: "allow".into(),
            approval: "on-request".into(),
            model: "deepseek-v4-flash".into(),
            ..script
        }));
        let (s2, sc2) = (seen.clone(), script.clone());
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let (seen, script) = (s2.clone(), sc2.clone());
                tokio::spawn(async move { serve(stream, seen, script).await });
            }
        });
        Self { base_url, seen, script }
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    fn methods(&self) -> Vec<String> {
        self.seen.lock().unwrap().iter().map(|(m, _)| m.clone()).collect()
    }
}

fn reply(method: &str, p: &Value, script: &Mutex<Script>) -> Result<Value, Value> {
    let mut s = script.lock().unwrap();
    let session = p["session_id"].as_str().unwrap_or("a8:main").to_owned();
    let mut features = vec![
        "external_driver_v1",
        "permission.profile.v1",
        "state.session_hydrate.v1",
        "state.thread_graph.v1",
        "state.turn_state_get.v1",
    ];
    if !s.no_sandbox_feature {
        features.push("session.sandbox.v1");
    }
    if s.launch.is_some() {
        features.push("session.workspace_cwd.v1");
    }
    Ok(match method {
        "session/open" => json!({"opened": {
            "session_id": session, "active_profile_id": PROFILE, "workspace_root": p["cwd"].as_str().unwrap_or("/home/user/octos"),
            "cursor": {"stream": session, "seq": 1},
            "capabilities": {
                "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                "capabilities_schema_version": 2,
                "supported_methods": METHODS,
                "supported_notifications": ["turn/started"],
                "supported_features": features
            }
        }}),
        "session/list" => json!({"sessions": s.list_rows}),
        "launch/resolve" => s.launch.clone().unwrap_or(json!({})),
        "profile/local/create" => json!({"profile_id": p["requested_id"]}),
        // r3-session line 29 (`{}`); a busy session refuses.
        "session/delete" => {
            if session.contains("locked") {
                return Err(json!({"code": -32000, "message": "session is busy"}));
            }
            s.list_rows.retain(|r| r["id"] != json!(session));
            json!({})
        }
        "session/status/read" => json!({
            "session_id": session, "profile_id": PROFILE,
            "model": {"model": s.model, "provider": "deepseek", "selected": true},
            "permission_profile": s.mode, "sandbox": "workspace-write",
            "runtime_policy_stamp": {"approval_policy": s.approval, "network": if s.network == "allow" { "allowed" } else { "blocked" }, "sandbox_mode": "workspace-write"}
        }),
        "permission/profile/list" => json!({
            "session_id": session, "current": {"mode": s.mode, "network": s.network},
            "profiles": [{"mode": "read_only", "network": "deny"}, {"mode": "workspace_write", "network": "deny"}, {"mode": "danger_full_access", "network": "allow"}]
        }),
        "permission/profile/set" => {
            if s.fail_sets > 0 {
                s.fail_sets -= 1;
                return Err(json!({"code": -32602, "message": "mode rejected"}));
            }
            let u = &p["update"];
            if let Some(m) = u["mode"].as_str() {
                s.mode = m.into();
            }
            if let Some(n) = u["network"].as_str() {
                s.network = n.into();
            }
            if let Some(a) = u["approval_policy"].as_str() {
                s.approval = a.into();
            }
            json!({"applied": true, "session_id": session, "current": {"mode": s.mode, "network": s.network}})
        }
        "profile/llm/list" => json!({"profile_id": PROFILE, "llm": {
            "primary": {"model_id": "deepseek-v4-flash", "family_id": "deepseek", "route_id": "deepseek",
                        "route": {"route_id": "deepseek", "label": "Official API"}, "selected": s.model == "deepseek-v4-flash", "available": true},
            "fallbacks": [{"model_id": "glm-5", "family_id": "zhipu", "route_id": "zhipu",
                        "route": {"route_id": "zhipu", "label": "BigModel API"}, "selected": s.model == "glm-5", "available": true}]
        }}),
        "profile/llm/select" => {
            let running = s.model.clone();
            s.model = p["model_id"].as_str().unwrap_or("").into();
            json!({"applied": true, "session_id": session, "runtime_disposition": "restart_required",
                   "restart_required": true, "runtime_policy_stamp": {"model": running}})
        }
        "session/driver/get" => {
            if let Some(kind) = &s.driver_refusal {
                return Err(json!({"code": -32000, "message": "RAW SERVER TEXT do-not-show", "data": {"kind": kind}}));
            }
            let ops = json!({"items": if s.held { json!([{
                "operation_id": "op-0001", "kind": "peer_dispatch", "lifecycle": "started", "created_at_ms": 1,
                "acceptance": {"model": "deepseek-v4-flash", "model_lane": "strong", "workspace_root": "/home/user/octos",
                    "adopted_turn_id": "01920000-0000-7000-8000-000000000301", "adopted_session_id": format!("{session}#peer-review-diff"),
                    "slug": "review-diff", "accepted_at_ms": 2, "payload_digest": "sha256:x"}
            }]) } else { json!([]) }, "snapshot": "snap-1", "observed_revision": "9", "complete": true, "next_cursor": null});
            if s.held {
                json!({"mode": "external", "recovery": "none",
                       "binding": {"driver_id": "octos-tui", "epoch": 4, "revision": 9, "lease_expires_at_ms": 1_790_000_000_000u64},
                       "operations": ops})
            } else {
                json!({"mode": "internal", "recovery": "none", "binding": null, "operations": ops})
            }
        }
        "session/driver/acquire" => json!({
            "control_token": "fixture-token",
            "binding": {"driver_id": p["driver_id"], "epoch": 5, "revision": 10, "lease_expires_at_ms": 1_790_000_060_000u64},
            "recovery": "none"
        }),
        "session/driver/release" => {
            s.held = false;
            json!({"mode": "internal", "recovery": "none"})
        }
        // The recorded r23-conversation graph shape.
        "thread/graph/get" => json!({
            "cursor": {"seq": 278, "stream": session}, "orphans": [], "session_id": session,
            "threads": [{"message_seqs": [0, 1], "root_seq": 0, "status": "unknown", "thread_id": "01920000-0000-7000-8000-00000000023b"}]
        }),
        "approval/scopes/list" => json!({"scopes": [
            {"session_id": session, "scope": "workspace", "scope_match": "/home/user/octos", "decision": "approved"}
        ]}),
        "turn/state/get" => json!({"session_id": session, "turn_id": p["turn_id"], "state": "unknown", "committed_seqs": []}),
        // The recorded r43a hydrate shape (messages with seq/role/content).
        "session/hydrate" => json!({"session_id": session, "cursor": {"stream": session, "seq": 3},
            "messages": if s.empty_history || s.empty_for.contains(&session) { json!([]) } else { json!([
                {"seq": 1, "role": "user", "content": "Fix the steer queue drop on reconnect", "persisted_at": "2026-10-01T09:00:00Z"},
                {"seq": 2, "role": "tool", "content": "cargo test: ok", "persisted_at": "2026-10-01T09:00:01Z"},
                {"seq": 3, "role": "assistant", "content": "The queue now re-drains after the socket is back.", "persisted_at": "2026-10-01T09:00:02Z"}
            ]) }}),
        _ => json!({}),
    })
}

async fn serve(stream: TcpStream, seen: Arc<Mutex<Vec<(String, Value)>>>, script: Arc<Mutex<Script>>) {
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
    let (mut tx, mut rx_in) = ws.split();
    while let Some(Ok(msg)) = rx_in.next().await {
        let Message::Text(text) = msg else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        if v.get("id").is_none() {
            continue;
        }
        let method = v["method"].as_str().unwrap_or("").to_owned();
        seen.lock().unwrap().push((method.clone(), v["params"].clone()));
        let frame = match reply(&method, &v["params"], &script) {
            Ok(r) => json!({"jsonrpc": "2.0", "id": v["id"].clone(), "result": r}),
            Err(e) => json!({"jsonrpc": "2.0", "id": v["id"].clone(), "error": e}),
        };
        let delay = match method.as_str() {
            "session/hydrate" => script.lock().unwrap().hydrate_delay_ms,
            "launch/resolve" => script.lock().unwrap().launch_delay_ms,
            _ => 0,
        };
        if delay > 0 {
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        let _ = tx.send(Message::Text(frame.to_string().into())).await;
    }
}

async fn connected(server: &FakeServer) -> (Conversation, tokio::sync::mpsc::Receiver<octos_app_transport::TransportEvent>) {
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
    (conv, events)
}

async fn drain(conv: &Conversation, events: &mut tokio::sync::mpsc::Receiver<octos_app_transport::TransportEvent>) {
    while let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(150), events.recv()).await {
        let _ = conv.on_event(evt);
    }
}

async fn wait_for(server: &FakeServer, method: &str, n: usize) {
    for _ in 0..60 {
        if server.params_of(method).len() >= n {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// The board-3 state, the defaults store and the env seams are process-global.
fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    host::reset();
    sd::set_storage(Arc::new(MemoryStore::new()));
    sd::note_apply_result(Ok(()));
    let dir = std::env::temp_dir().join(format!("a8-session-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    std::env::set_var("OCTOSCODE_RECENTS_DIR", &dir);
    std::env::set_var("OCTOSCODE_SHOW_THINKING_FILE", dir.join("show-thinking.json"));
    std::env::set_var("OCTOSCODE_PANE_ADVANCED_FILE", dir.join("pane-advanced.json"));
    std::env::set_var("OCTOSCODE_DRIVER_ID", ME);
    g
}

/// This app's driver id, pinned for the run (the product mints
/// `octoscode-native:<uuid>` once per install).
const ME: &str = "octoscode-native:00000000-0000-4000-8000-0000000000a8";

fn spawn_of(o: Outcome) -> Job {
    match o {
        Outcome::Spawn(job) => job,
        other => panic!("expected a spawned job, got {other:?}"),
    }
}

/// The strip's click (its wired `b3.strip.settings` tap) opens THIS pane and
/// the pane's opening reads the stamp, the presets, the models and the driver.
async fn open_pane(conv: &Conversation) {
    let job = spawn_of(host::perform("b3.strip.settings", 0, &conv.store));
    assert_eq!(job, Job::PaneLoad);
    assert_eq!(host::open_dialog(), Some(Dialog::SessionPane));
    host::run(job, conv).await.expect("pane load");
}

#[tokio::test]
async fn the_strip_opens_the_pane_which_reads_model_permissions_sandbox_and_controller() {
    let _g = lock();
    let server = FakeServer::start(Script { held: true, ..Default::default() }).await;
    let (conv, _ev) = connected(&server).await;
    let session = conv.session_id();
    open_pane(&conv).await;
    // The four reads, each addressed to THIS session.
    assert_eq!(server.params_of("session/status/read")[0]["session_id"], json!(session));
    assert_eq!(server.params_of("permission/profile/list")[0], json!({"session_id": session}));
    assert_eq!(server.params_of("profile/llm/list")[0]["session_id"], json!(session));
    assert_eq!(server.params_of("session/driver/get")[0], json!({"session_id": session, "operations": {"limit": 50}}));
    let st = host::state();
    let p = &st.pane;
    assert!(!p.loading);
    let status = p.status.clone().expect("status");
    assert_eq!(status.model.as_deref(), Some("deepseek-v4-flash"), "session runtime model");
    assert_eq!(status.approval_policy.as_deref(), Some("on-request"), "the stamp's approval policy");
    assert_eq!(p.models.iter().map(|m| (m.title.as_str(), m.selected)).collect::<Vec<_>>(), [("deepseek-v4-flash", true), ("glm-5", false)]);
    match &p.driver {
        Inventory::Complete { operations, disclosure, .. } => {
            assert_eq!(disclosure.mode, dd::Mode::External);
            assert_eq!(disclosure.binding.as_ref().unwrap().driver_id, "octos-tui");
            assert_eq!(operations[0].slug, "review-diff");
        }
        other => panic!("{other:?}"),
    }
    assert!(session_pane::foreign_held(p, &conv.store), "another app holds the seat");
    // The lowered pane names every section with the web's copy.
    drop(st);
    let dsl = host::lower_open(&conv.store).expect("open").dsl;
    for text in [
        "Session settings",
        "Another app is using this session",
        "Saved for this profile:",
        "Session runtime",
        "Write · Network allowed",
        "Approval policy:",
        "Sandbox:",
        "Set when the session opens — start a new session to change it",
        "Show thinking",
    ] {
        assert!(dsl.contains(text), "{text}");
    }
}

#[tokio::test]
async fn a_model_pick_sends_the_exact_select_and_shows_its_disposition() {
    let _g = lock();
    let server = FakeServer::start(Script::default()).await;
    let (conv, _ev) = connected(&server).await;
    open_pane(&conv).await;
    // Row 1 = glm-5 (the picker's tap `b3.sc.model#1`).
    let job = spawn_of(host::perform("b3.sc.model", 1, &conv.store));
    assert_eq!(job, Job::PaneModel(1));
    host::run(job, &conv).await.expect("select");
    assert_eq!(
        server.params_of("profile/llm/select")[0],
        json!({"family_id": "zhipu", "model_id": "glm-5", "profile_id": PROFILE, "route_id": "zhipu", "session_id": conv.session_id()}),
        "the route's wire id, never its label"
    );
    let st = host::state();
    assert_eq!(
        st.pane.model_notice.as_deref(),
        Some("Saved. The server keeps running deepseek-v4-flash until it restarts"),
        "restart_required names the model the session keeps serving"
    );
    assert!(st.pane.models[1].selected && !st.pane.models[0].selected);
}

#[tokio::test]
async fn permission_presets_and_the_approval_policy_save_then_read_back() {
    let _g = lock();
    let server = FakeServer::start(Script { fail_sets: 1, ..Default::default() }).await;
    let (conv, _ev) = connected(&server).await;
    open_pane(&conv).await;
    let opts = session_pane::perm_options(&conv.store);
    assert_eq!(opts[0].label(), "Write · Network allowed", "the current selection first");
    // A failing save: Failed + Retry re-sends the same update.
    let job = spawn_of(host::perform("b3.sc.perm", 1, &conv.store));
    assert!(host::run(job, &conv).await.is_err());
    assert_eq!(host::state().pane.perm_save, Some(SaveState::Failed("mode rejected".into())));
    let job = spawn_of(host::perform("b3.sc.perm.retry", 0, &conv.store));
    host::run(job, &conv).await.expect("retry");
    let sets = server.params_of("permission/profile/set");
    assert_eq!(sets.len(), 2);
    assert_eq!(sets[0], sets[1], "retry re-sends the same update");
    assert_eq!(sets[1]["update"], json!({"mode": "read_only", "network": "deny"}));
    assert_eq!(host::state().pane.perm_save, Some(SaveState::Saved));
    assert_eq!(session_pane::perm_selected(&conv.store).as_deref(), Some("read_only:deny"), "the reply's current is the readback");
    // The stamp is re-read after each save.
    assert_eq!(server.params_of("session/status/read").len(), 2);
    // Never ask.
    let job = spawn_of(host::perform("b3.sc.approval.never", 0, &conv.store));
    host::run(job, &conv).await.expect("approval");
    assert_eq!(server.params_of("permission/profile/set")[2]["update"], json!({"approval_policy": "never"}));
    assert_eq!(host::state().pane.status.as_ref().unwrap().approval_policy.as_deref(), Some("never"));
}

#[tokio::test]
async fn full_access_is_never_sent_without_the_acknowledgement() {
    let _g = lock();
    let server = FakeServer::start(Script::default()).await;
    let (conv, _ev) = connected(&server).await;
    open_pane(&conv).await;
    let full = session_pane::perm_options(&conv.store).iter().position(|o| o.dangerous()).expect("full access offered");
    assert_eq!(host::perform("b3.sc.perm", full, &conv.store), Outcome::Done, "asks first");
    assert_eq!(host::perform("b3.sc.risk.confirm", 0, &conv.store), Outcome::Done, "no ack: nothing");
    assert!(server.params_of("permission/profile/set").is_empty());
    host::perform("b3.sc.risk.ack", 0, &conv.store);
    let job = spawn_of(host::perform("b3.sc.risk.confirm", 0, &conv.store));
    host::run(job, &conv).await.expect("set");
    assert_eq!(server.params_of("permission/profile/set"), vec![json!({"session_id": conv.session_id(), "update": {"mode": "danger_full_access", "network": "allow"}})]);
}

/// `SessionConfigPane.tsx:340-368` + `App.tsx:3302-3306`: the Show thinking
/// row flips THIS Session's visibility and the persisted preference.
#[tokio::test]
async fn the_show_thinking_row_flips_the_sessions_visibility_and_the_saved_preference() {
    let _g = lock();
    let server = FakeServer::start(Script::default()).await;
    let (conv, _ev) = connected(&server).await;
    open_pane(&conv).await;
    let session = conv.session_id();
    let before = conv.store.domains.session.thinking(&session).show_reasoning;
    assert_eq!(host::perform("b3.sc.thinking", 0, &conv.store), Outcome::Done);
    assert_eq!(conv.store.domains.session.thinking(&session).show_reasoning, !before, "the Session's visibility");
    let file = std::env::var("OCTOSCODE_SHOW_THINKING_FILE").unwrap();
    let saved = std::fs::read_to_string(file).unwrap();
    assert_eq!(saved.trim(), if before { "false" } else { "true" }, "the saved preference");
    host::perform("b3.sc.thinking", 0, &conv.store);
    assert_eq!(conv.store.domains.session.thinking(&session).show_reasoning, before, "and back");
    assert!(host::lower_open(&conv.store).unwrap().dsl.contains("Show thinking"));
}

#[tokio::test]
async fn resume_chat_takes_the_seat_hands_it_back_and_sends_the_prompt_once() {
    let _g = lock();
    let server = FakeServer::start(Script { held: true, ..Default::default() }).await;
    let (conv, _ev) = connected(&server).await;
    open_pane(&conv).await;
    conv.set_draft("continue the review");
    let job = spawn_of(host::perform("b3.sc.resume_chat", 0, &conv.store));
    assert_eq!(job, Job::PaneResumeChat);
    assert_eq!(host::perform("b3.sc.resume_chat", 0, &conv.store), Outcome::Done, "single-flight while busy");
    host::run(job, &conv).await.expect("resume chat");
    let acquire = &server.params_of("session/driver/acquire")[0];
    assert_eq!(acquire["expected_revision"], json!(9), "CAS on the observed revision");
    assert_eq!(acquire["driver_id"], json!(ME));
    let release = &server.params_of("session/driver/release")[0];
    assert_eq!(release["next"], json!("internal"));
    assert_eq!(release["expected_revision"], json!(10), "the acquire's binding revision");
    assert_eq!(server.params_of("turn/start").len(), 1, "the prompt is sent ONCE");
    assert_eq!(server.params_of("turn/start")[0]["input"][0]["text"], json!("continue the review"));
    let order: Vec<String> = server
        .methods()
        .into_iter()
        .filter(|m| matches!(m.as_str(), "session/driver/acquire" | "session/driver/release" | "turn/start"))
        .collect();
    assert_eq!(order, ["session/driver/acquire", "session/driver/release", "turn/start"]);
    let st = host::state();
    assert!(!st.pane.resume_busy && st.pane.resume_notice.is_none());
    assert_eq!(st.pane.driver.disclosure().map(|d| d.mode), Some(dd::Mode::Internal), "re-read after the handover");
}

#[tokio::test]
async fn a_driver_refusal_shows_only_its_allowlisted_kind() {
    let _g = lock();
    let server = FakeServer::start(Script { driver_refusal: Some("driver_scope_mismatch".into()), ..Default::default() }).await;
    let (conv, _ev) = connected(&server).await;
    open_pane(&conv).await;
    assert_eq!(host::state().pane.driver, Inventory::Error(ErrorReason::Refused("driver_scope_mismatch")));
    host::perform("b3.sc.advanced", 0, &conv.store);
    let dsl = host::lower_open(&conv.store).unwrap().dsl;
    assert!(dsl.contains("This session can't be controlled from here"));
    assert!(!dsl.contains("RAW SERVER TEXT"), "the server's message is never rendered");
    // An unknown kind is the generic failure.
    server.script.lock().unwrap().driver_refusal = Some("something_else".into());
    host::run(Job::PaneLoad, &conv).await.unwrap();
    assert_eq!(host::state().pane.driver, Inventory::Error(ErrorReason::Unknown));
}

#[tokio::test]
async fn new_session_defaults_apply_at_creation_only() {
    let _g = lock();
    let server = FakeServer::start(Script::default()).await;
    let (conv, mut ev) = connected(&server).await;
    // Settings > Sandbox: the three switches persist the defaults.
    use octoscode_module::screens::settings;
    settings::reset_state();
    sd::set_storage(Arc::new(MemoryStore::new()));
    settings::apply_ui("sandbox_write.toggle"); // -> read_only
    settings::apply_ui("sandbox_network.toggle"); // -> allow
    settings::apply_ui("sandbox_read_outside.toggle"); // -> sandbox on
    let live = sd::current();
    assert!(live.stored);
    assert_eq!((live.value.permission_mode, live.value.network, live.value.sandbox.enabled), (sd::PermissionMode::ReadOnly, sd::NetworkPolicy::Allow, true));
    // A re-open of the existing session carries nothing.
    let existing = conv.session_id();
    conv.open_session(&existing, None).await.unwrap();
    drain(&conv, &mut ev).await;
    assert!(server.params_of("session/open").iter().all(|p| p.get("sandbox").is_none()), "a re-open never carries the sandbox");
    assert!(server.params_of("permission/profile/set").is_empty(), "a re-open never applies the permission default");
    // A NEW chat: the sandbox rides the open, ONE permission set follows for the created id.
    let created = conv.new_chat(None).await.unwrap();
    drain(&conv, &mut ev).await;
    wait_for(&server, "permission/profile/set", 1).await;
    let open = server.params_of("session/open").into_iter().find(|p| p["session_id"] == json!(created)).expect("created open");
    assert_eq!(open["sandbox"], json!({"enabled": true, "network_access": true}));
    assert_eq!(
        server.params_of("permission/profile/set"),
        vec![json!({"session_id": created, "update": {"mode": "read_only", "network": "allow"}})]
    );
    assert_eq!(conv.defaults_applied(), vec![created.clone()]);
    // Re-opening the created session later never re-applies them.
    conv.open_session(&created, None).await.unwrap();
    drain(&conv, &mut ev).await;
    assert_eq!(server.params_of("permission/profile/set").len(), 1);
    assert_eq!(sd::apply_error(), None);
}

#[tokio::test]
async fn an_unadvertised_sandbox_is_never_sent_and_a_failed_default_is_surfaced() {
    let _g = lock();
    let server = FakeServer::start(Script { no_sandbox_feature: true, fail_sets: 1, ..Default::default() }).await;
    let (conv, mut ev) = connected(&server).await;
    sd::update(|d| d.sandbox.enabled = true);
    let created = conv.new_chat(None).await.unwrap();
    drain(&conv, &mut ev).await;
    wait_for(&server, "permission/profile/set", 1).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let open = server.params_of("session/open").into_iter().find(|p| p["session_id"] == json!(created)).unwrap();
    assert!(open.get("sandbox").is_none(), "session.sandbox.v1 not advertised: fail closed");
    assert_eq!(sd::apply_error().as_deref(), Some(sd::APPLY_FAILED), "the failure is surfaced, not swallowed");
    // The pane shows it as its notice.
    open_pane(&conv).await;
    assert!(host::lower_open(&conv.store).unwrap().dsl.contains(sd::APPLY_FAILED));
}

#[tokio::test]
async fn the_inspection_dialog_titles_its_kind_scopes_its_session_and_refreshes() {
    let _g = lock();
    let server = FakeServer::start(Script::default()).await;
    let (conv, _ev) = connected(&server).await;
    let session = conv.session_id();
    // `/permissions` from the composer (the slash grammar's production entry).
    let job = spawn_of(host::command("permissions", "", &conv).expect("an inspection command"));
    assert_eq!(job, Job::InspectorLoad);
    assert_eq!(host::open_dialog(), Some(Dialog::Inspector));
    host::run(job, &conv).await.unwrap();
    let dsl = host::lower_open(&conv.store).unwrap().dsl;
    assert!(dsl.contains("text: \"Remembered approvals\""), "the per-kind title");
    assert!(dsl.contains(&format!("Session: {session}")), "the scope line names the Session");
    assert!(dsl.contains("Read-only server snapshot."), "the read-only note");
    assert_eq!(server.params_of("approval/scopes/list").len(), 1);
    // Refresh: Reading… while the read runs (no second tap), then a fresh read.
    let job = spawn_of(host::perform("b3.insp.refresh", 0, &conv.store));
    assert_eq!(job, Job::InspectorLoad);
    let reading = host::lower_open(&conv.store).unwrap().dsl;
    assert!(reading.contains("\"Reading…\"") && reading.contains("Reading the captured Session…"));
    assert_eq!(host::perform("b3.insp.refresh", 0, &conv.store), Outcome::Done, "no second read while one runs");
    host::run(job, &conv).await.unwrap();
    assert_eq!(server.params_of("approval/scopes/list").len(), 2, "refresh re-read the server snapshot");
    // The other two kinds keep their own titles; `/turn` adds the turn to the scope.
    host::run(spawn_of(host::command("threads", "", &conv).unwrap()), &conv).await.unwrap();
    assert!(host::lower_open(&conv.store).unwrap().dsl.contains("text: \"Thread graph\""));
    let turn = "01920000-0000-7000-8000-000000000301";
    host::run(spawn_of(host::command("turn", &format!("state {turn}"), &conv).unwrap()), &conv).await.unwrap();
    let dsl = host::lower_open(&conv.store).unwrap().dsl;
    assert!(dsl.contains("text: \"Turn state\""));
    assert!(dsl.contains("· Turn: 01920000"), "the turn joins the scope line");
    assert_eq!(server.params_of("turn/state/get")[0]["turn_id"], json!(turn));
}

#[tokio::test]
async fn the_conversation_link_copies_and_stays_visible_read_only() {
    let _g = lock();
    let server = FakeServer::start(Script::default()).await;
    let (conv, _ev) = connected(&server).await;
    host::run(spawn_of(host::command("threads", "", &conv).unwrap()), &conv).await.unwrap();
    let link = host::state().insp.link.clone();
    assert!(link.starts_with("octoscode://session?s="), "{link}");
    let dsl = host::lower_open(&conv.store).unwrap().dsl;
    // A13 (judge: an 8-line percent-encoded block): the link stays visible
    // on ONE line in its grey field, shortened in the middle — its scheme
    // and its end (the session id) readable — and Copy writes it whole.
    let field = dsl.find("b3_insp_link_value := Label").expect("the one-line link");
    let label = &dsl[field..field + dsl[field..].find("\n}").expect("the label closes")];
    assert!(label.contains("flow: Right\n"), "one line, no wrap: {label}");
    assert!(label.contains('…'), "shortened in the middle: {label}");
    assert!(label.contains("text: \"octoscode://session?s="), "the head stays: {label}");
    let tail: String = link.chars().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect();
    assert!(label.contains(&format!("{tail}\"")), "the tail stays: {label}");
    assert!(!dsl.contains(&format!("{link:?}")), "never the raw multi-line block");
    // Copy writes the clipboard and announces it.
    assert_eq!(host::perform("b3.insp.copy", 0, &conv.store), Outcome::Clipboard(link));
    assert!(host::lower_open(&conv.store).unwrap().dsl.contains("Conversation link copied."));
}

#[tokio::test]
async fn the_header_copy_reads_canonical_history_and_reports_each_phase() {
    use octoscode_module::screens::copy_button::{self, Phase};
    let _g = lock();
    copy_button::reset();
    let server = FakeServer::start(Script::default()).await;
    let (conv, _ev) = connected(&server).await;
    let session = conv.session_id();
    assert!(copy_button::offered(&conv.store, false), "a Session is open and session/hydrate is advertised");
    assert!(!copy_button::offered(&conv.store, true), "no room on the phone header");
    // The header pill's routed id is this module's.
    assert!(copy_button::owns(copy_button::ACTION));
    let req = copy_button::begin(&session).expect("idle -> copying");
    assert_eq!(copy_button::phase(&session).label(), "Copying…");
    assert!(copy_button::begin(&session).is_none(), "disabled while copying");
    assert_eq!(copy_button::run(req, &conv).await, Phase::Copied);
    assert_eq!(copy_button::phase(&session).label(), "Copied");
    assert_eq!(server.params_of("session/hydrate")[0], json!({"session_id": session}), "the canonical history, not the rendered timeline");
    let md = host::take_clipboard().expect("the markdown waits for the UI thread's clipboard write");
    assert!(md.starts_with("# octos"), "the workspace leaf heads it: {md}");
    assert!(md.contains("Fix the steer queue drop on reconnect") && md.contains("re-drains"));
    assert!(!md.contains("cargo test: ok"), "chat text only: tool output is dropped");
    // An empty conversation copies nothing.
    server.script.lock().unwrap().empty_history = true;
    let req = copy_button::begin(&session).unwrap();
    assert_eq!(copy_button::run(req, &conv).await, Phase::Empty);
    assert_eq!(copy_button::phase(&session).label(), "Nothing to copy");
    assert!(host::take_clipboard().is_none(), "nothing written");
}

fn catalog(rows: &[(&str, &str, u64)]) -> Vec<Value> {
    rows.iter()
        .map(|(id, title, n)| json!({"id": id, "title": title, "message_count": n, "updated_at": "2026-10-01T10:00:00Z"}))
        .collect()
}

#[tokio::test]
async fn resume_refuses_an_empty_history_and_puts_the_previous_session_back() {
    let _g = lock();
    let server = FakeServer::start(Script {
        list_rows: catalog(&[("a8:api:hollow", "Listed with messages", 4)]),
        empty_for: vec!["a8:api:hollow".into()],
        ..Default::default()
    })
    .await;
    let (conv, mut ev) = connected(&server).await;
    let before = conv.session_id();
    host::run(spawn_of(host::command("resume", "", &conv).unwrap()), &conv).await.unwrap();
    host::perform("b3.resume.select", 0, &conv.store);
    host::input_changed("resume.confirm", "Listed with messages");
    let job = spawn_of(host::perform("b3.resume.confirm", 0, &conv.store));
    assert!(host::run(job, &conv).await.is_err(), "an empty hydrate of advertised history fails");
    drain(&conv, &mut ev).await;
    assert_eq!(
        host::state().resume.error.as_deref(),
        Some("Historical identity was not resolved. The listed conversation was not resumed.")
    );
    assert_eq!(conv.session_id(), before, "the previous Session is put back");
    assert_eq!(conv.store.active_session().as_deref(), Some(before.as_str()), "the placeholder is not left selected");
    assert!(conv.store.sessions().iter().any(|s| s.id == "a8:api:hollow"), "the server's catalog row stays listed");
    assert!(
        server
            .params_of("session/hydrate")
            .iter()
            .any(|p| p["session_id"] == json!("a8:api:hollow") && p["include"] == json!(["messages"])),
        "the candidate's canonical history was read"
    );
    assert!(server.params_of("turn/start").is_empty(), "resuming never starts a turn");
}

#[tokio::test]
async fn resume_blocks_a_pooled_same_id_collision_and_unscoped_ids_before_open() {
    let _g = lock();
    let server = FakeServer::start(Script {
        list_rows: catalog(&[("a8:api:shared", "Retained elsewhere", 2), ("a8:bare", "Legacy id", 2), ("a8:api:ok", "Good one", 2)]),
        ..Default::default()
    })
    .await;
    let (conv, mut ev) = connected(&server).await;
    let main = conv.session_id();
    // This app already holds `a8:api:shared` under ANOTHER workspace…
    conv.open_session("a8:api:shared", Some("/home/user/other".into())).await.unwrap();
    drain(&conv, &mut ev).await;
    conv.open_session(&main, Some("/home/user/octos".into())).await.unwrap();
    drain(&conv, &mut ev).await;
    let opens = server.params_of("session/open").len();
    host::run(spawn_of(host::command("resume", "", &conv).unwrap()), &conv).await.unwrap();
    let st = host::state();
    let reason = |id: &str| st.resume.candidates.iter().find(|c| c.id == id).and_then(|c| c.blocked.clone());
    assert_eq!(
        reason("a8:api:shared").as_deref(),
        Some("This Session ID is already retained under another workspace or Profile. It cannot be rebound on the shared connection.")
    );
    assert!(reason("a8:bare").unwrap().starts_with("This catalog ID does not identify a full Session"), "no channel is guessed");
    assert_eq!(reason("a8:api:ok"), None);
    let shared = st.resume.candidates.iter().position(|c| c.id == "a8:api:shared").unwrap();
    drop(st);
    host::perform("b3.resume.select", shared, &conv.store);
    assert_eq!(host::state().resume.selected, None, "a blocked row never arms");
    assert_eq!(server.params_of("session/open").len(), opens, "nothing opened");
}

#[tokio::test]
async fn resume_rejects_a_double_submit_and_a_selection_switch_while_the_hydrate_is_pending() {
    let _g = lock();
    let server = FakeServer::start(Script {
        list_rows: catalog(&[("a8:api:first", "First", 2), ("a8:api:second", "Second", 2)]),
        hydrate_delay_ms: 400,
        ..Default::default()
    })
    .await;
    let (conv, _ev) = connected(&server).await;
    host::run(spawn_of(host::command("resume", "", &conv).unwrap()), &conv).await.unwrap();
    host::perform("b3.resume.select", 0, &conv.store);
    let first = host::state().resume.candidates[0].clone();
    host::input_changed("resume.confirm", &first.title);
    let job = spawn_of(host::perform("b3.resume.confirm", 0, &conv.store));
    let conv = Arc::new(conv);
    let c2 = conv.clone();
    let running = tokio::spawn(async move { host::run(job, &c2).await });
    tokio::time::sleep(Duration::from_millis(100)).await; // the hydrate is held back
    assert_eq!(host::perform("b3.resume.confirm", 0, &conv.store), Outcome::Done, "a second submit is refused");
    assert_eq!(host::state().resume.error.as_deref(), Some("A history opening is already pending."));
    host::perform("b3.resume.select", 1, &conv.store);
    assert_eq!(host::state().resume.selected, Some(0), "the selection does not switch while pending");
    running.await.unwrap().expect("the one opening completes");
    let opened: Vec<Value> = server.params_of("session/open").into_iter().filter(|p| p["session_id"] == json!(first.id)).collect();
    assert_eq!(opened.len(), 1, "exactly one opening");
    assert_eq!(conv.session_id(), first.id);
}

#[tokio::test]
async fn delete_removes_a_confirmed_row_and_keeps_a_refused_one_with_its_reason() {
    let _g = lock();
    let server = FakeServer::start(Script {
        list_rows: catalog(&[("a8:api:old", "Old chat", 2), ("a8:api:locked", "Busy chat", 2)]),
        ..Default::default()
    })
    .await;
    let (conv, _ev) = connected(&server).await;
    host::run(spawn_of(host::command("sessions", "", &conv).unwrap()), &conv).await.unwrap();
    let rows = octoscode_module::screens::board3::switcher::rows(&conv.store);
    let at = |id: &str| rows.iter().position(|r| r.id == id).unwrap();
    // The open Session is never offered.
    let me = rows.iter().position(|r| r.current).expect("the open Session is listed");
    host::perform("b3.switch.delete", me, &conv.store);
    assert_eq!(host::state().switch.confirm_delete, None);
    // Delete asks first, then sends ONE session/delete; the row goes on success.
    host::perform("b3.switch.delete", at("a8:api:old"), &conv.store);
    let dsl = host::lower_open(&conv.store).unwrap().dsl;
    assert!(dsl.contains("\"Delete?\""), "the confirmation is in place");
    let job = spawn_of(host::perform("b3.switch.delete.confirm", 0, &conv.store));
    host::run(job, &conv).await.unwrap();
    assert_eq!(server.params_of("session/delete"), vec![json!({"session_id": "a8:api:old"})]);
    assert!(!conv.store.sessions().iter().any(|s| s.id == "a8:api:old"), "removed on success");
    // A refused delete keeps the row and says why.
    let rows = octoscode_module::screens::board3::switcher::rows(&conv.store);
    let locked = rows.iter().position(|r| r.id == "a8:api:locked").unwrap();
    host::perform("b3.switch.delete", locked, &conv.store);
    let job = spawn_of(host::perform("b3.switch.delete.confirm", 0, &conv.store));
    assert!(host::run(job, &conv).await.is_err());
    assert!(conv.store.sessions().iter().any(|s| s.id == "a8:api:locked"), "the row stays");
    assert_eq!(host::state().switch.error.as_deref(), Some("Couldn't delete the session: session is busy"));
}

fn launch_lock() -> std::sync::MutexGuard<'static, ()> {
    let g = lock();
    octoscode_module::screens::launch::reset();
    octoscode_module::screens::drafts::reset();
    octoscode_module::screens::drafts::set_storage(Arc::new(MemoryStore::new()));
    g
}

#[tokio::test]
async fn a_cross_profile_launch_waits_for_the_choice_and_moves_the_draft_only_after_the_open_commits() {
    use octoscode_module::screens::{drafts, launch};
    let _g = launch_lock();
    let server = FakeServer::start(Script {
        launch: Some(json!({"decision": "cross_profile", "resolved_profile": "a8", "existing_profiles": ["glm-coder"]})),
        ..Default::default()
    })
    .await;
    let (conv, mut ev) = connected(&server).await;
    // What the person typed while the launch was pending stays with its Session.
    let a = drafts::active_key(&conv).unwrap();
    drafts::follow(Some(&a), "");
    conv.set_draft("plan the fix");
    drafts::follow(Some(&a), "plan the fix");
    let old_session = conv.session_id();
    let opens = server.params_of("session/open").len();
    assert_eq!(launch::create(&conv, "/home/user/octos".into()).await, launch::Launched::AwaitingChoice);
    assert_eq!(server.params_of("launch/resolve"), vec![json!({"cwd": "/home/user/octos", "profile_id": "a8"})]);
    assert_eq!(server.params_of("session/open").len(), opens, "nothing opens before the choice");
    assert_eq!(launch::snapshot().phase, launch::Phase::AwaitingChoice);
    assert_eq!(host::open_dialog(), Some(Dialog::Launch));
    let dsl = host::lower_open(&conv.store).unwrap().dsl;
    for t in ["Choose this workspace’s profile", "Start a8 here", "Start new session with glm-coder", "Server decision"] {
        assert!(dsl.contains(t), "{t}");
    }
    // The person chooses glm-coder (the panel's second button).
    let job = spawn_of(host::perform("b3.launch.choose", 1, &conv.store));
    assert_eq!(job, Job::LaunchChoose("glm-coder".into()));
    assert_eq!(drafts::get(&a).as_deref(), Some("plan the fix"), "not moved before the commit");
    host::run(job, &conv).await.unwrap();
    let open = server.params_of("session/open").last().unwrap().clone();
    assert!(open["session_id"].as_str().unwrap().starts_with("glm-coder:api:"), "{open}");
    assert_eq!((open["profile_id"].clone(), open["cwd"].clone()), (json!("glm-coder"), json!("/home/user/octos")));
    assert_eq!(host::open_dialog(), None, "the panel closes on success");
    assert_eq!(launch::snapshot().phase, launch::Phase::Idle);
    // Adopting the chosen profile never re-keys the Session still in the
    // composer (its key carries its OWN profile) ...
    assert_eq!(drafts::key_of(&conv, &old_session), a, "the old Session keeps its key");
    // ... and a frame taken before the open's reply lands keeps the text in
    // the composer (the frame loop follows the composer every frame).
    let mid = drafts::active_key(&conv);
    assert_eq!(drafts::follow(mid.as_deref(), "plan the fix"), None, "the composer keeps the text mid-transition");
    // The open committed (its reply named the workspace): the draft moves now.
    drain(&conv, &mut ev).await;
    let b = drafts::active_key(&conv).unwrap();
    assert_ne!(a, b);
    assert_eq!(drafts::follow(Some(&b), "plan the fix"), None, "the composer keeps the text");
    assert_eq!(drafts::get(&b).as_deref(), Some("plan the fix"), "moved to the committed Session");
    assert_eq!(drafts::get(&a), None, "and gone from the old one");
}

#[tokio::test]
async fn resume_and_activate_open_at_once_and_an_unadvertised_probe_is_skipped() {
    use octoscode_module::screens::launch;
    let _g = launch_lock();
    let server = FakeServer::start(Script { launch: Some(json!({"decision": "activate", "resolved_profile": "a8"})), ..Default::default() }).await;
    let (conv, _ev) = connected(&server).await;
    match launch::create(&conv, "/home/user/octos".into()).await {
        launch::Launched::Opened(id) => assert!(id.starts_with("a8:api:")),
        other => panic!("{other:?}"),
    }
    assert_eq!(server.params_of("launch/resolve").len(), 1);
    let quiet = FakeServer::start(Script::default()).await;
    let (conv2, _e2) = connected(&quiet).await;
    assert!(matches!(launch::create(&conv2, "/home/user/octos".into()).await, launch::Launched::Opened(_)));
    assert!(quiet.params_of("launch/resolve").is_empty(), "unadvertised: the launch opens directly");
}

#[tokio::test]
async fn a_newer_launch_retires_the_older_resolver() {
    use octoscode_module::screens::launch;
    let _g = launch_lock();
    let server = FakeServer::start(Script {
        launch: Some(json!({"decision": "activate", "resolved_profile": "a8"})),
        launch_delay_ms: 300,
        ..Default::default()
    })
    .await;
    let (conv, _ev) = connected(&server).await;
    let conv = Arc::new(conv);
    let c1 = conv.clone();
    let first = tokio::spawn(async move { launch::create(&c1, "/home/user/first".into()).await });
    tokio::time::sleep(Duration::from_millis(80)).await;
    let second = launch::create(&conv, "/home/user/second".into()).await;
    assert!(matches!(second, launch::Launched::Opened(_)));
    assert_eq!(first.await.unwrap(), launch::Launched::Stale, "the older resolver is rejected");
    let cwds: Vec<Value> = server.params_of("session/open").into_iter().map(|p| p["cwd"].clone()).collect();
    assert!(!cwds.contains(&json!("/home/user/first")), "the stale launch never opened: {cwds:?}");
    assert!(cwds.contains(&json!("/home/user/second")));
}

#[tokio::test]
async fn no_profile_offers_to_create_the_local_profile_then_opens() {
    use octoscode_module::screens::launch;
    let _g = launch_lock();
    let server = FakeServer::start(Script { launch: Some(json!({"decision": "no_profile"})), ..Default::default() }).await;
    let (conv, _ev) = connected(&server).await;
    assert_eq!(launch::create(&conv, "/home/user/octos".into()).await, launch::Launched::AwaitingChoice);
    assert!(host::lower_open(&conv.store).unwrap().dsl.contains("Create the local profile"));
    let job = spawn_of(host::perform("b3.launch.create_profile", 0, &conv.store));
    host::run(job, &conv).await.unwrap();
    assert_eq!(server.params_of("profile/local/create").len(), 1);
    let open = server.params_of("session/open").last().unwrap().clone();
    assert_eq!(open["cwd"], json!("/home/user/octos"));
    assert_eq!(host::open_dialog(), None);
}
