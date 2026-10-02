//! A19 — the connect-time launch, the web's way, at the wire.
//!
//! Every test drives the functions the native start and Connect paths call
//! (lib.rs `start` / `connect_now`): `screens::launch::plan` (which profile id
//! a new connection carries), `Conversation::connect` with that id,
//! `Conversation::remember_opens`, and `screens::launch::startup` (what the
//! connection opens first) — then asserts what reached the wire, what the
//! window shows and what the app remembers.
//!
//! The fake server answers in the recorded a6ea8505 shapes
//! (`crates/octoscode-client/tests/fixtures/r29a-onboarding-a6ea8505.jsonl`:
//! the capability object of its line 2, the catalog of line 6, line 4's
//! refusal of a `session/open` for a profile that does not exist) and decides
//! `launch/resolve` with Core's own table (`octos-cli/src/runtime/launch.rs`
//! `resolve_launch_decision`: the requested profile, else the folder's sticky
//! one, else the default — the first known profile — else `no_profile`). It
//! also serves the two REST reads of the one-time migration
//! (`POST /api/auth/solo`, `GET /api/admin/profiles`). Every request is
//! recorded, so "nothing was created" is checked on the wire.
//!
//! Web references (src-web/apps/web/src): a fresh connection carries no
//! profile id (`features/connection/connection-bootstrap.ts:17-23`) and
//! `launch/resolve` sends one only when the connection has one
//! (`features/session/use-octos-session.ts:3033-3036`); `no_profile` prepares
//! the onboarding panel (`:3048-3058`, `features/workspace/LaunchDecisionPanel.tsx:33-48`);
//! a remembered Session is restored directly (`use-octos-session.ts:2956-3008`);
//! the web's only `profile/local/create` is the onboarding submission
//! (`features/onboarding/onboarding-submission.ts:59`).
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::Conversation;
use octoscode_module::screens::board3::host::{self, Dialog};
use octoscode_module::screens::launch::{self, Launched, Start, Started};
use octoscode_module::screens::onboarding::{self as onb, Phase};
use octoscode_module::screens::recents::MemoryStore;
use octoscode_module::screens::remembered;

const FOLDER: &str = "/srv/work/no-profile";
const OTHER: &str = "/srv/work/other";
/// The server's own working directory (`onboarding/workspace_list` with no path).
const SERVER_CWD: &str = "/srv/work/ws";

fn fixture() -> Vec<Value> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../octoscode-client/tests/fixtures/r29a-onboarding-a6ea8505.jsonl");
    std::fs::read_to_string(path)
        .expect("read the r29a fixture")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("fixture line is JSON"))
        .collect()
}

fn recorded(method: &str) -> Value {
    fixture()
        .into_iter()
        .find(|f| f["dir"] == "in" && f["method"] == method && f.get("body").is_some_and(|b| !b.is_null()))
        .unwrap_or_else(|| panic!("r29a records {method}"))["body"]
        .clone()
}

/// r29a line 4: the recorded refusal of a `session/open` for an unknown profile.
fn recorded_open_refusal(profile: &str) -> (i64, String) {
    let line = fixture()
        .into_iter()
        .find(|f| f["dir"] == "in" && f["method"] == "session/open" && f.get("error").is_some())
        .expect("r29a records the refused open");
    let code = line["error"]["code"].as_i64().unwrap();
    let message = line["error"]["message"].as_str().unwrap().replace("'octoscode'", &format!("'{profile}'"));
    (code, message)
}

#[derive(Default)]
struct World {
    /// Core's known profiles (enabled, top-level).
    profiles: Vec<String>,
    /// The profiles whose config names a model (`config.llm.primary`).
    runnable: Vec<String>,
    /// `POST /api/auth/solo`'s local user, when the server has one.
    solo_user: Option<String>,
    /// The profiles with a session store in the launched folder (the first
    /// one is the folder's sticky profile).
    folder_profiles: Vec<String>,
    seen: Vec<(String, Value)>,
    http: Vec<String>,
}

struct FakeServer {
    base_url: String,
    world: Arc<Mutex<World>>,
}

impl FakeServer {
    async fn start(world: World) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let world = Arc::new(Mutex::new(world));
        let w2 = world.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                tokio::spawn(route(stream, w2.clone()));
            }
        });
        Self { base_url, world }
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.world.lock().unwrap().seen.iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    fn methods(&self) -> Vec<String> {
        self.world.lock().unwrap().seen.iter().map(|(m, _)| m.clone()).collect()
    }

    fn http(&self) -> Vec<String> {
        self.world.lock().unwrap().http.clone()
    }
}

/// One accepted socket: the AppUI WebSocket upgrade, or a plain REST read.
async fn route(stream: TcpStream, world: Arc<Mutex<World>>) {
    let mut head = vec![0u8; 8192];
    let mut n = 0;
    for _ in 0..200 {
        n = stream.peek(&mut head).await.unwrap_or(0);
        if head[..n].windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let text = String::from_utf8_lossy(&head[..n]).to_ascii_lowercase();
    if text.contains("upgrade: websocket") {
        serve_ws(stream, world).await;
    } else {
        serve_http(stream, world).await;
    }
}

async fn serve_http(mut stream: TcpStream, world: Arc<Mutex<World>>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let Ok(k) = stream.read(&mut chunk).await else { return };
        if k == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..k]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let len = head
        .lines()
        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
        .unwrap_or(0);
    while buf.len() < header_end + len {
        let Ok(k) = stream.read(&mut chunk).await else { return };
        if k == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..k]);
    }
    let line = head.lines().next().unwrap_or("").to_owned();
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or("").to_owned(), parts.next().unwrap_or("").to_owned());
    let (status, body) = {
        let mut w = world.lock().unwrap();
        w.http.push(format!("{method} {path}"));
        match (method.as_str(), path.as_str()) {
            ("POST", "/api/auth/solo") => match &w.solo_user {
                Some(id) => ("200 OK", json!({"token": "solo-session-dummy", "user": {"id": id}})),
                None => ("404 Not Found", json!({"error": "no local solo user"})),
            },
            ("GET", "/api/admin/profiles") => {
                let rows: Vec<Value> = w
                    .profiles
                    .iter()
                    .map(|p| {
                        let config = if w.runnable.contains(p) {
                            json!({"llm": {"primary": {"provider": "deepseek", "model": "deepseek-v4-flash"}}})
                        } else {
                            json!({})
                        };
                        json!({"id": p, "config": config})
                    })
                    .collect();
                ("200 OK", json!(rows))
            }
            _ => ("404 Not Found", json!({})),
        }
    };
    let body = body.to_string();
    let out = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(out.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// Core's launch table (`runtime/launch.rs` `resolve_launch_decision`).
fn core_decision(w: &World, p: &Value) -> Value {
    let requested = p.get("profile_id").and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
    let mut known = w.profiles.clone();
    known.sort();
    let resolved = requested.or_else(|| w.folder_profiles.first().cloned()).or_else(|| known.first().cloned());
    match resolved {
        None => json!({"decision": "no_profile", "existing_profiles": []}),
        Some(id) if w.folder_profiles.contains(&id) => json!({"decision": "resume", "resolved_profile": id, "existing_profiles": []}),
        Some(id) if w.folder_profiles.is_empty() => json!({"decision": "activate", "resolved_profile": id, "existing_profiles": []}),
        Some(id) => json!({"decision": "cross_profile", "resolved_profile": id, "existing_profiles": w.folder_profiles}),
    }
}

fn reply(w: &mut World, method: &str, p: &Value) -> Result<Value, (i64, String)> {
    let caps = recorded("config/capabilities/list")["capabilities"].clone();
    match method {
        "config/capabilities/list" => Ok(recorded("config/capabilities/list")),
        "onboarding/workspace_list" => Ok(json!({
            "canonical_path": SERVER_CWD, "parent_path": "/srv/work", "writable": true,
            "entries": [], "truncated": false, "hidden_skipped": 0
        })),
        "launch/resolve" => Ok(core_decision(w, p)),
        "profile/llm/catalog" => Ok(recorded("profile/llm/catalog")),
        "session/open" => {
            let session = p["session_id"].as_str().unwrap_or("").to_owned();
            let profile = p["profile_id"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| session.split(':').next().unwrap_or("").to_owned());
            if !w.profiles.contains(&profile) {
                return Err(recorded_open_refusal(&profile));
            }
            Ok(json!({"opened": {
                "session_id": session,
                "active_profile_id": profile,
                "workspace_root": p["cwd"].as_str().unwrap_or(SERVER_CWD),
                "cursor": {"stream": session, "seq": 1},
                "capabilities": {
                    "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                    "capabilities_schema_version": 2,
                    "supported_methods": caps["supported_methods"],
                    "supported_notifications": ["turn/started"],
                    "supported_features": caps["supported_features"]
                }
            }}))
        }
        "session/list" => Ok(json!({"sessions": []})),
        "profile/local/create" => Ok(recorded("profile/local/create")),
        _ => Ok(json!({})),
    }
}

async fn serve_ws(stream: TcpStream, world: Arc<Mutex<World>>) {
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
    let (mut tx, mut rx) = ws.split();
    while let Some(Ok(msg)) = rx.next().await {
        let Message::Text(text) = msg else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        if v.get("id").is_none() {
            continue;
        }
        let method = v["method"].as_str().unwrap_or("").to_owned();
        let params = v["params"].clone();
        let out = {
            let mut w = world.lock().unwrap();
            w.seen.push((method.clone(), params.clone()));
            match reply(&mut w, &method, &params) {
                Ok(result) => json!({"jsonrpc": "2.0", "id": v["id"], "result": result}),
                Err((code, message)) => json!({"jsonrpc": "2.0", "id": v["id"], "error": {"code": code, "message": message}}),
            }
        };
        if tx.send(Message::Text(out.to_string().into())).await.is_err() {
            break;
        }
    }
}

/// The production connect for `start` (lib.rs `connect_now`): the plan's
/// profile id on the connection, opens remembered unless the dev/test
/// override, the event drain running.
fn connect(server: &FakeServer, start: &Start) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", &start.profile(), None, None).expect("connect");
    let conv = Arc::new(conv);
    conv.remember_opens(start.remembers());
    let drain = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            let _ = drain.on_event(evt);
        }
    });
    conv
}

/// The board-3 / launch / onboarding state, the storage seams and the
/// remembered-connection file are process-global: one test at a time, each
/// from a clean slate, every file inside a per-test temp dir.
fn lock(name: &str) -> (std::sync::MutexGuard<'static, ()>, std::path::PathBuf) {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    host::reset();
    launch::reset();
    octoscode_module::screens::drafts::reset();
    octoscode_module::screens::drafts::set_storage(Arc::new(MemoryStore::new()));
    octoscode_module::screens::session_defaults::set_storage(Arc::new(MemoryStore::new()));
    let dir = std::env::temp_dir().join(format!("a19-launch-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("OCTOSCODE_CONNECTION_FILE", dir.join("connection-v1.json"));
    std::env::set_var("OCTOSCODE_CREDENTIALS_DIR", dir.join("cred"));
    std::env::set_var("OCTOSCODE_RECENTS_DIR", dir.join("recents"));
    std::env::set_var("OCTOSCODE_SHOW_THINKING_FILE", dir.join("show-thinking.json"));
    std::env::remove_var("OCTOS_PROFILE_ID");
    std::env::remove_var("OCTOS_CREATE_PROFILE");
    remembered::set_legacy_for_test(None);
    (g, dir)
}

async fn until(what: &str, mut pred: impl FnMut() -> bool) {
    for _ in 0..150 {
        if pred() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    panic!("timed out waiting for {what}");
}

fn assert_nothing_created(server: &FakeServer) {
    assert!(
        server.params_of("profile/local/create").is_empty(),
        "no profile is created outside the onboarding submission (web: onboarding-submission.ts:59); wire: {:?}",
        server.methods()
    );
}

// ------------------------------------------------------------------ fresh

/// The card's first proof: a fresh connection with nothing remembered sends
/// NO profile id, a server with no profile answers `no_profile`, and the
/// onboarding panel shows with the provider catalog loaded — nothing created,
/// nothing opened, nothing remembered.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_fresh_connection_sends_no_profile_id_and_no_profile_shows_the_onboarding_panel() {
    let (_g, dir) = lock("fresh");
    let server = FakeServer::start(World::default()).await;
    let start = launch::plan(&server.base_url);
    assert_eq!(start, Start::Fresh);
    assert_eq!(start.profile(), "", "a fresh connection carries no profile id (connection-bootstrap.ts:21)");
    let conv = connect(&server, &start);

    let r = launch::startup(&conv, start, Some(FOLDER.into())).await;
    assert_eq!(r, Started::Launched(Launched::AwaitingChoice));

    // The wire: the capability read (the web's authenticate), then the probe
    // with the folder ONLY — no profile_id key at all.
    assert_eq!(server.params_of("config/capabilities/list").len(), 1);
    assert_eq!(server.params_of("launch/resolve"), vec![json!({"cwd": FOLDER})]);
    // The panel: Core's no_profile is the onboarding (LaunchDecisionPanel.tsx:33-48).
    assert_eq!(host::open_dialog(), Some(Dialog::Launch));
    assert!(launch::is_no_profile(&launch::snapshot()));
    until("the provider catalog", || onb::snapshot().phase == Phase::Ready).await;
    assert!(onb::snapshot().catalog.is_some(), "the catalog loaded");
    assert_eq!(server.params_of("profile/llm/catalog").len(), 1, "prepare reads the catalog once");
    let lowered = host::lower_open(&conv.store).expect("the panel lowers");
    assert!(lowered.dsl.contains(onb::TITLE), "the onboarding panel, not the decision list");
    assert!(launch::holds_first_run(&conv.store), "the panel owns the first-run frame (no Connect form behind it)");

    // Nothing created, opened, discovered or remembered.
    assert_nothing_created(&server);
    assert!(server.params_of("session/open").is_empty(), "no Session before the person decides");
    assert!(server.http().is_empty(), "no solo login, no profile list: {:?}", server.http());
    assert_eq!(remembered::load(&server.base_url), None);
    // No Session yet: a New chat and a submit are refused, nothing is sent.
    assert!(conv.new_chat(Some(FOLDER.into())).await.is_err());
    conv.set_draft("hello");
    assert_eq!(conv.submit_draft().await.unwrap(), "", "held: no Session yet");
    assert_eq!(conv.ui().lock().unwrap().draft(), "hello", "the text stays");
    assert!(server.params_of("turn/start").is_empty() && server.params_of("session/open").is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

/// An EXISTING setup with nothing remembered (another device, or a fresh app
/// state): no folder configured, so the launch folder is the server's own
/// working directory; still no profile id, and Core's `activate` opens the
/// Session at once under the profile it names — straight to a session. The
/// committed open is remembered, and a later launch in another folder then
/// carries the active profile (the web's committed config,
/// `active-session-runtime.ts:1504-1514` `committedSessionConfig`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_existing_setup_goes_straight_to_a_session_and_the_open_is_remembered() {
    let (_g, dir) = lock("existing");
    let world = World {
        profiles: vec!["octoscode-6218".into(), "dsflash".into()],
        runnable: vec!["dsflash".into()],
        ..Default::default()
    };
    let server = FakeServer::start(world).await;
    let start = launch::plan(&server.base_url);
    assert_eq!(start, Start::Fresh);
    let conv = connect(&server, &start);

    let r = launch::startup(&conv, start, None).await;
    let Started::Launched(Launched::Opened(id)) = r else { panic!("straight to a session, got {r:?}") };
    assert_eq!(server.params_of("onboarding/workspace_list"), vec![json!({"path": null})]);
    assert_eq!(server.params_of("launch/resolve"), vec![json!({"cwd": SERVER_CWD})], "no profile id");
    let opens = server.params_of("session/open");
    assert_eq!(opens.len(), 1);
    assert_eq!(opens[0]["profile_id"], json!("dsflash"), "Core's resolved profile");
    assert_eq!(opens[0]["cwd"], json!(SERVER_CWD));
    assert_eq!(opens[0]["session_id"], json!(id));
    assert!(id.starts_with("dsflash:"), "a fresh Session bound to the profile: {id}");
    until("the Session is live", || conv.store.is_live()).await;
    assert_eq!(conv.store.active_session().as_deref(), Some(id.as_str()));
    until("the open is remembered", || remembered::load(&server.base_url).is_some()).await;
    assert_eq!(
        remembered::load(&server.base_url),
        Some(remembered::Remembered { profile_id: "dsflash".into(), session_id: id.clone(), cwd: SERVER_CWD.into() })
    );
    assert!(!launch::holds_first_run(&conv.store));
    // The next launch carries the active profile now.
    assert!(matches!(launch::create(&conv, OTHER.into()).await, Launched::Opened(_)));
    assert_eq!(server.params_of("launch/resolve")[1], json!({"cwd": OTHER, "profile_id": "dsflash"}));
    assert_nothing_created(&server);
    assert!(server.http().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------- restore

/// The card's second proof: a remembered profile is SENT — on the connection
/// and on the open — and the app goes straight to its Session: no probe, no
/// capability detour, nothing created (the web's restore,
/// `use-octos-session.ts:2985-2999`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_remembered_profile_is_sent_and_goes_straight_to_its_session() {
    let (_g, dir) = lock("restore");
    let server = FakeServer::start(World {
        profiles: vec!["dsflash".into(), "octoscode-6218".into()],
        folder_profiles: vec!["octoscode-6218".into()],
        ..Default::default()
    })
    .await;
    // A previous launch's committed open (the production writer).
    remembered::note_opened(&server.base_url, "dsflash", "dsflash:api:web-1", Some(SERVER_CWD));
    let start = launch::plan(&server.base_url);
    let Start::Restore(r) = &start else { panic!("a remembered open is restored, got {start:?}") };
    assert_eq!(r.profile_id, "dsflash");
    assert_eq!(start.profile(), "dsflash", "the connection carries the remembered profile");
    let conv = connect(&server, &start);
    assert_eq!(conv.profile(), "dsflash");

    let started = launch::startup(&conv, start, None).await;
    assert_eq!(started, Started::Restored("dsflash:api:web-1".into()));
    let opens = server.params_of("session/open");
    assert_eq!(opens.len(), 1);
    assert_eq!(opens[0]["session_id"], json!("dsflash:api:web-1"));
    assert_eq!(opens[0]["profile_id"], json!("dsflash"));
    assert_eq!(opens[0]["cwd"], json!(SERVER_CWD));
    assert!(server.params_of("launch/resolve").is_empty(), "a restore asks no launch decision");
    assert!(!server.methods().iter().any(|m| m == "config/capabilities/list"));
    until("the Session is live", || conv.store.is_live()).await;
    assert_eq!(conv.store.active_session().as_deref(), Some("dsflash:api:web-1"));
    assert_eq!(host::open_dialog(), None, "no panel");
    assert_nothing_created(&server);
    assert!(server.http().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

/// A remembered Session the server refuses (its profile is gone) is
/// forgotten and the launch starts fresh with NO profile id (the web's
/// `restoreRejected` clears profile, Session and workspace, `App.tsx:952-974`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_restore_is_forgotten_and_the_launch_starts_fresh() {
    let (_g, dir) = lock("refused");
    let server = FakeServer::start(World::default()).await;
    remembered::note_opened(&server.base_url, "gone", "gone:main", Some(SERVER_CWD));
    let start = launch::plan(&server.base_url);
    assert!(matches!(start, Start::Restore(_)));
    let conv = connect(&server, &start);

    let r = launch::startup(&conv, start, Some(FOLDER.into())).await;
    assert_eq!(r, Started::Launched(Launched::AwaitingChoice), "Core's no_profile after the refusal");
    assert_eq!(server.params_of("session/open").len(), 1, "the one refused restore");
    assert_eq!(server.params_of("launch/resolve"), vec![json!({"cwd": FOLDER})], "the refused profile is not carried on");
    assert!(launch::is_no_profile(&launch::snapshot()));
    assert_eq!(remembered::load(&server.base_url), None, "the refused target is forgotten");
    assert_eq!(launch::plan(&server.base_url), Start::Fresh);
    assert_nothing_created(&server);
    let _ = std::fs::remove_dir_all(dir);
}

/// Another identity on the same server (another token) starts with nothing
/// remembered (`ConnectionGate.tsx:266-307`); the same identity keeps it; and
/// Settings > Forget server clears it (`ConnectionGate.tsx:319-356`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_identity_or_forget_drops_the_remembered_session() {
    let (_g, dir) = lock("identity");
    let server = "http://127.0.0.1:59999";
    octoscode_module::credentials::remember_token(server, "tok-a").unwrap();
    remembered::note_opened(server, "dsflash", "dsflash:main", Some(SERVER_CWD));
    assert!(!remembered::on_connect_identity(server, "tok-a"), "the same token keeps it");
    assert!(remembered::load(server).is_some());
    assert!(remembered::on_connect_identity(server, "tok-b"), "another token drops it");
    assert_eq!(remembered::load(server), None);
    remembered::note_opened(server, "dsflash", "dsflash:main", Some(SERVER_CWD));
    octoscode_module::screens::a9_settings::forget_saved(server);
    assert_eq!(remembered::load(server), None, "Forget server clears it");
    let _ = std::fs::remove_dir_all(dir);
}

// ------------------------------------------------------- dev/test override

/// OCTOS_PROFILE_ID is a dev/test override only (the web has no such
/// setting): the profile is used as given, `<profile>:main` opens at the
/// startup workspace, no probe — and nothing is created or remembered.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_dev_override_opens_its_main_session_and_creates_nothing() {
    let (_g, dir) = lock("explicit");
    let server = FakeServer::start(World { profiles: vec!["dsflash".into()], ..Default::default() }).await;
    std::env::set_var("OCTOS_PROFILE_ID", "dsflash");
    let start = launch::plan(&server.base_url);
    std::env::remove_var("OCTOS_PROFILE_ID");
    assert_eq!(start, Start::Explicit("dsflash".into()));
    assert!(!start.remembers());
    let conv = connect(&server, &start);
    let r = launch::startup(&conv, start, Some(SERVER_CWD.into())).await;
    assert_eq!(r, Started::Explicit("dsflash:main".into()));
    let opens = server.params_of("session/open");
    assert_eq!(opens.len(), 1);
    assert_eq!((opens[0]["session_id"].clone(), opens[0]["profile_id"].clone()), (json!("dsflash:main"), json!("dsflash")));
    until("the Session is live", || conv.store.is_live()).await;
    assert!(server.params_of("launch/resolve").is_empty());
    assert_nothing_created(&server);
    assert_eq!(remembered::load(&server.base_url), None, "a harness's profile is never remembered");
    let _ = std::fs::remove_dir_all(dir);
}

// --------------------------------------------------------------- migration

/// The one-time migration: a device that used this server before the
/// upgrade (A1's last-server names it, nothing remembered yet) lands where
/// the previous build landed — the solo login's first runnable profile
/// (`discover_solo_profile`, commit 04c49631) and its `<profile>:main` —
/// creating nothing; the open is remembered, so the NEXT launch is a plain
/// restore, and the migration never runs twice.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_one_time_migration_reopens_the_previous_builds_landing() {
    let (_g, dir) = lock("migrate");
    let server = FakeServer::start(World {
        profiles: vec!["octoscode-6218".into(), "dsflash".into(), "octoscode-desktop".into()],
        runnable: vec!["dsflash".into()],
        solo_user: Some("octoscode-6218".into()),
        folder_profiles: vec!["dsflash".into()],
        ..Default::default()
    })
    .await;
    remembered::set_legacy_for_test(octoscode_module::credentials::origin(&server.base_url).as_deref());
    let start = launch::plan(&server.base_url);
    assert_eq!(start, Start::Migrate);
    assert!(!remembered::legacy_candidate(&server.base_url), "the migration runs once per origin");
    let conv = connect(&server, &start);

    let r = launch::startup(&conv, start, None).await;
    assert_eq!(r, Started::Migrated("dsflash:main".into()));
    assert_eq!(server.http(), vec!["POST /api/auth/solo".to_owned(), "GET /api/admin/profiles".to_owned()]);
    let opens = server.params_of("session/open");
    assert_eq!(opens.len(), 1);
    assert_eq!((opens[0]["session_id"].clone(), opens[0]["profile_id"].clone()), (json!("dsflash:main"), json!("dsflash")));
    assert!(opens[0]["cwd"].is_null(), "the previous build's open: no folder");
    assert!(server.params_of("launch/resolve").is_empty());
    assert_nothing_created(&server);
    until("the open is remembered", || remembered::load(&server.base_url).is_some()).await;
    assert_eq!(
        remembered::load(&server.base_url),
        Some(remembered::Remembered { profile_id: "dsflash".into(), session_id: "dsflash:main".into(), cwd: SERVER_CWD.into() })
    );
    // The next launch restores it — same profile, same Session.
    let Start::Restore(next) = launch::plan(&server.base_url) else { panic!("the relaunch restores") };
    assert_eq!((next.profile_id.as_str(), next.session_id.as_str()), ("dsflash", "dsflash:main"));
    let _ = std::fs::remove_dir_all(dir);
}

/// The migration on a server with no profile (a fresh data dir): nothing to
/// migrate, so it is the fresh launch — no profile id, Core's `no_profile`,
/// the onboarding panel; nothing created.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_migration_on_a_fresh_server_is_the_fresh_launch() {
    let (_g, dir) = lock("migrate-fresh");
    let server = FakeServer::start(World::default()).await;
    remembered::set_legacy_for_test(octoscode_module::credentials::origin(&server.base_url).as_deref());
    let start = launch::plan(&server.base_url);
    assert_eq!(start, Start::Migrate);
    let conv = connect(&server, &start);
    let r = launch::startup(&conv, start, Some(FOLDER.into())).await;
    assert_eq!(r, Started::Launched(Launched::AwaitingChoice));
    assert_eq!(server.http(), vec!["POST /api/auth/solo".to_owned()], "no solo user: nothing to rank");
    assert_eq!(server.params_of("launch/resolve"), vec![json!({"cwd": FOLDER})]);
    assert!(launch::is_no_profile(&launch::snapshot()));
    assert!(server.params_of("session/open").is_empty());
    assert_nothing_created(&server);
    let _ = std::fs::remove_dir_all(dir);
}
