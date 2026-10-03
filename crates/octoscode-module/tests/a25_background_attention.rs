//! A25 — a BACKGROUND Session's attention, on the production path: the real
//! WS transport, the real `Conversation` (link + flow + the lib.rs event
//! loop's `on_event`), A22's background records (row 236), and the
//! attention model the host runs on every sync (`attention::sync`, what
//! lib.rs's `attention::observe` runs) over a recording OS.
//!
//! Web:
//! - e2e/attention.spec.ts:231-269 "a background Session completing while
//!   another is selected signals and acknowledges on return": the page stays
//!   VISIBLE, one notification is created, the row reads "Completed in
//!   background", selecting it closes the notification;
//! - use-octos-session.ts:2714-2763 `publishBackgroundTurns`: every
//!   background record feeds its OWN `foregroundAttentionTurns(scope, active,
//!   current interaction, timeline)` (model.ts:101-131), a turn recovery could
//!   not prove read as failed (:2726-2747);
//! - model.ts:37-93 `observe`: reading = visible && selected (:64), so a
//!   Session that is not selected is never being read; a transition out of
//!   running/waiting signals (:66-76); a stopped or rate-limited turn's
//!   terminal is `info` (timeline/model.ts:779-786) and `findLast` passes over
//!   it (model.ts:108-114), so it never signals;
//! - use-attention.ts:86 the notice is withdrawn when nothing is unread.
//!
//! The fake Core is A22's (tests/a22_sessions.rs), cut down to what these
//! tests need.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::attention::{self, Authorization, Controller, NotifyOs, OsEvent, TurnState};
use octoscode_module::flow::Conversation;
use octoscode_module::screens::sidebar;

const PROFILE: &str = "a25";
const CWD: &str = "/home/user/a25-ws";
const MAIN: &str = "a25:main";
const BUILD: &str = "a25:api:build";
const TESTS: &str = "a25:api:tests";
const STOPPED: &str = "a25:api:stopped";
const LIMITED: &str = "a25:api:limited";
const ASK: &str = "a25:api:ask";

// ------------------------------------------------------------- the fake Core

type Script = Arc<dyn Fn(&str, &Value) -> Value + Send + Sync>;

struct Core {
    base: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
    push: Arc<Mutex<Option<mpsc::UnboundedSender<String>>>>,
}

fn note(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
}

impl Core {
    async fn start(script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let push: Arc<Mutex<Option<mpsc::UnboundedSender<String>>>> = Arc::new(Mutex::new(None));
        let (seen2, push2) = (seen.clone(), push.clone());
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { continue };
                let (mut sink, mut source) = ws.split();
                let (tx, mut rx) = mpsc::unbounded_channel::<String>();
                *push2.lock().unwrap() = Some(tx.clone());
                tokio::spawn(async move {
                    while let Some(frame) = rx.recv().await {
                        if sink.send(Message::Text(frame.into())).await.is_err() {
                            break;
                        }
                    }
                });
                let (seen, script) = (seen2.clone(), script.clone());
                tokio::spawn(async move {
                    while let Some(Ok(msg)) = source.next().await {
                        let Message::Text(text) = msg else { continue };
                        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                        if v.get("id").is_none() {
                            continue;
                        }
                        let method = v["method"].as_str().unwrap_or("").to_owned();
                        seen.lock().unwrap().push((method.clone(), v["params"].clone()));
                        let result = script(&method, &v["params"]);
                        let _ = tx.send(json!({"jsonrpc": "2.0", "id": v["id"], "result": result}).to_string());
                    }
                });
            }
        });
        Self { base, seen, push }
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    fn notify(&self, method: &str, params: Value) {
        if let Some(tx) = self.push.lock().unwrap().as_ref() {
            let _ = tx.send(note(method, params).to_string());
        }
    }
}

fn opened(session: &str, cwd: &str) -> Value {
    json!({ "opened": {
        "session_id": session, "active_profile_id": PROFILE, "workspace_root": cwd,
        "cursor": {"stream": session, "seq": 1},
        "capabilities": {
            "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
            "capabilities_schema_version": 2,
            "supported_methods": ["session/open", "session/hydrate", "session/list", "turn/start", "turn/interrupt",
                                  "user_question/respond"],
            "supported_notifications": ["turn/started", "projection/envelope", "user_question/requested"],
            "supported_features": ["state.session_hydrate.v1", "projection.envelope.v2", "session.workspace_cwd.v1", "user_question.v1"]
        }
    }})
}

/// The attested catalog: every Session of this profile, with its title.
fn catalog() -> Value {
    let row = |id: &str, title: &str| json!({"id": id, "message_count": 2, "title": title, "updated_at": "2026-10-02T09:00:00Z"});
    json!({"sessions": [
        row(MAIN, "Startup chat"),
        row(BUILD, "Build the release"),
        row(TESTS, "Run the test suite"),
        row(STOPPED, "Try the refactor"),
        row(LIMITED, "Summarize the logs"),
        row(ASK, "Pick a branch"),
    ], "workspace_root": CWD, "profile_id": PROFILE})
}

fn env(session: &str, turn: &str, seq: u64, cursor: u64, payload: Value) -> Value {
    json!({"session_id": session, "thread_id": turn, "turn_id": turn, "seq": seq,
           "cursor": {"stream": session, "seq": cursor}, "payload": payload})
}

fn empty_history(session: &str) -> Value {
    json!({"session_id": session, "cursor": {"stream": session, "seq": 1}, "messages": [],
           "projection_thread_sequences": {}})
}

async fn core() -> Core {
    Core::start(Arc::new(|method, p| {
        let session = p["session_id"].as_str().unwrap_or("").to_owned();
        match method {
            "session/open" => opened(&session, p["cwd"].as_str().unwrap_or(CWD)),
            "session/hydrate" => empty_history(&session),
            "session/list" if p.get("cwd").is_some() => catalog(),
            "session/list" => json!({"sessions": []}),
            "turn/start" => json!({"accepted": true}),
            _ => json!({}),
        }
    }))
    .await
}

// --------------------------------------------------------------- the driver

async fn launch(core: &Core) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&core.base, "dummy", PROFILE, Some(CWD.to_owned()), None).expect("connect");
    let conv = Arc::new(conv);
    conv.attach();
    let drv = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            let _ = drv.on_event(evt);
        }
    });
    conv.open_workspace(Some(CWD.to_owned())).await.expect("session/open");
    let s = conv.session_id();
    until("the startup Session's history settles", || {
        conv.store.is_live() && conv.history(&s) == octoscode_module::flow::History::Ready
    })
    .await;
    until("the attested catalog names the Sessions", || conv.store.sessions().iter().any(|s| s.id == BUILD)).await;
    conv
}

/// A sidebar click: `open_session` with the Session's folder (the lib.rs
/// `thread.open` effect), its history settled.
async fn open(conv: &Arc<Conversation>, id: &str) {
    conv.open_session(id, Some(CWD.to_owned())).await.expect("session/open");
    until(&format!("{id}'s history settles"), || {
        conv.store.active_session().as_deref() == Some(id) && conv.history(id) == octoscode_module::flow::History::Ready
    })
    .await;
}

/// Start a turn in `id` (a prompt submitted while it is selected) and let
/// the server report it started; returns the turn id.
async fn run_turn_in(core: &Core, conv: &Arc<Conversation>, id: &str, prompt: &str) -> String {
    open(conv, id).await;
    let before = core.params_of("turn/start").len();
    conv.set_draft(prompt);
    conv.submit_draft().await.expect("turn/start");
    until("the turn was sent", || core.params_of("turn/start").len() == before + 1).await;
    let turn = core.params_of("turn/start")[before]["turn_id"].as_str().unwrap().to_owned();
    core.notify("turn/started", json!({"session_id": id, "turn_id": turn, "timestamp": "2026-10-02T09:00:00Z"}));
    until("the turn is live", || conv.store.domains.composer.snapshot(id).active.is_some_and(|t| t.turn_id == turn)).await;
    turn
}

fn finish(core: &Core, session: &str, turn: &str, cursor: u64, outcome: &str) {
    core.notify("projection/envelope", env(session, turn, 9, cursor, json!({"type": "turn_terminal", "data": {"outcome": outcome}})));
}

async fn until(what: &str, f: impl Fn() -> bool) {
    for _ in 0..500 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for: {what}");
}

async fn quiet() {
    tokio::time::sleep(Duration::from_millis(400)).await;
}

fn status_of(conv: &Conversation, id: &str) -> Option<sidebar::Status> {
    let active = conv.store.active_session();
    conv.store.sessions().iter().any(|s| s.id == id).then(|| sidebar::row_status(&conv.store, id, active.as_deref()))
}

/// Process-wide test state (the opt-in file, drafts, the start timeout).
fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    let dir = std::env::temp_dir().join(format!("a25-background-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    std::env::set_var("OCTOSCODE_DRAFTS_FILE", dir.join("drafts.json"));
    std::env::set_var("OCTOSCODE_SHOW_THINKING_FILE", dir.join("show-thinking.json"));
    std::env::set_var("OCTOSCODE_TURN_START_TIMEOUT_MS", "4000");
    g
}

// ------------------------------------------------------- the recording OS

#[derive(Debug, Default)]
struct Os {
    posts: Vec<(String, String, String)>,
    closes: Vec<String>,
    requests: usize,
}

impl NotifyOs for Os {
    fn query(&mut self) {}
    fn request(&mut self) {
        self.requests += 1;
    }
    fn post(&mut self, id: &str, title: &str, body: &str) {
        self.posts.push((id.into(), title.into(), body.into()));
    }
    fn close(&mut self, id: &str) {
        self.closes.push(id.into());
    }
    fn read_preference(&self) -> bool {
        true
    }
    fn save_preference(&mut self, _: bool) {}
}

/// The person opted in and the OS allows notices; `focused`: the window
/// has focus (the person is in the app, reading the selected Session).
fn attention_model(os: &mut Os, focused: bool) -> Controller {
    let mut c = Controller::default();
    c.start(os, focused);
    c.desktop.authorization(os, Authorization::Granted, false, None);
    assert!(c.desktop.settings().enabled);
    c
}

/// One host sync: what lib.rs's `attention::observe` runs.
fn sync(c: &mut Controller, os: &mut Os, conv: &Conversation) -> Vec<attention::TurnSnapshot> {
    let live = conv.store.active_session().and_then(|s| conv.ui().lock().unwrap().live_turn_in(&s));
    attention::sync(c, os, &conv.store, live)
}

// ===================================================================== tests

/// attention.spec.ts:231-269 natively: a background Session's turn completes
/// while the person reads ANOTHER Session (the window focused, i.e.
/// visible): its row reads "Completed in background", ONE notice names it
/// (later syncs never repeat it); selecting it acknowledges it and the
/// notice is withdrawn.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_background_session_finishing_while_another_is_selected_notifies_once_and_selecting_it_acknowledges() {
    let _g = lock();
    let core = core().await;
    let conv = launch(&core).await;
    assert_eq!(conv.session_id(), MAIN);
    let mut os = Os::default();
    let mut c = attention_model(&mut os, true);
    sync(&mut c, &mut os, &conv);

    // The build runs, the person goes back to the startup chat.
    let turn = run_turn_in(&core, &conv, BUILD, "build the release").await;
    sync(&mut c, &mut os, &conv);
    open(&conv, MAIN).await;
    sync(&mut c, &mut os, &conv);
    assert_eq!(status_of(&conv, BUILD), Some(sidebar::Status::Running), "the build works in the background");
    assert!(os.posts.is_empty(), "nothing finished yet: {:?}", os.posts);

    // It finishes while the startup chat is on screen.
    finish(&core, BUILD, &turn, 3, "completed");
    until("the build's terminal reached the app", || {
        conv.store.domains.turn.latest_real_terminal(BUILD).is_some_and(|(t, _)| t == turn)
    })
    .await;
    let notices = sync(&mut c, &mut os, &conv);
    assert_eq!(status_of(&conv, BUILD), Some(sidebar::Status::Done));
    assert_eq!(sidebar::status_label(sidebar::Status::Done, true), "Completed in background");
    assert_eq!(notices.len(), 1, "one signal: {notices:?}");
    assert_eq!((notices[0].scope.session_id.as_str(), notices[0].state), (BUILD, TurnState::Completed));
    assert_eq!(
        os.posts,
        vec![(
            attention::notice_id(BUILD),
            "Build the release".to_owned(),
            attention::notice_body(TurnState::Completed).to_owned()
        )],
        "one notice naming THAT Session, though the window is focused (the web: reading = visible && selected)"
    );
    assert_eq!(c.tracker.count(), 1);
    // Later syncs never repeat it.
    quiet().await;
    sync(&mut c, &mut os, &conv);
    sync(&mut c, &mut os, &conv);
    assert_eq!(os.posts.len(), 1);

    // Selecting it acknowledges it and withdraws the notice.
    open(&conv, BUILD).await;
    sync(&mut c, &mut os, &conv);
    assert_eq!(c.tracker.count(), 0, "reading it acknowledged it");
    assert_eq!(os.closes, vec![attention::notice_id(BUILD)], "the notice was withdrawn");
    assert!(c.desktop.notice().is_none());
}

/// The click on a background Session's notice names that Session (the host
/// opens it, lib.rs `open_attention_session` -> `thread.open`), and opening
/// it acknowledges.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_click_on_a_background_notice_opens_that_session_and_acknowledges() {
    let _g = lock();
    let core = core().await;
    let conv = launch(&core).await;
    let mut os = Os::default();
    // The window is NOT focused (the person is in another app).
    let mut c = attention_model(&mut os, false);
    sync(&mut c, &mut os, &conv);
    let turn = run_turn_in(&core, &conv, TESTS, "run the test suite").await;
    sync(&mut c, &mut os, &conv);
    open(&conv, MAIN).await;
    sync(&mut c, &mut os, &conv);
    // It fails in the background: "needs attention".
    finish(&core, TESTS, &turn, 3, "errored");
    until("the failure reached the app", || conv.store.domains.turn.latest_real_terminal(TESTS).is_some()).await;
    sync(&mut c, &mut os, &conv);
    assert_eq!(
        os.posts,
        vec![(attention::notice_id(TESTS), "Run the test suite".to_owned(), attention::notice_body(TurnState::Failed).to_owned())]
    );
    assert_eq!(status_of(&conv, TESTS), Some(sidebar::Status::Failed));
    // The click: the platform focused the app; the Session it names opens.
    let named = c.os_event(&mut os, OsEvent::Clicked { id: attention::notice_id(TESTS) });
    assert_eq!(named.as_deref(), Some(TESTS));
    assert_eq!(os.closes, vec![attention::notice_id(TESTS)], "the clicked notice is withdrawn");
    open(&conv, TESTS).await;
    c.set_focused(&mut os, true);
    sync(&mut c, &mut os, &conv);
    assert_eq!(c.tracker.count(), 0);
    assert_eq!(os.posts.len(), 1, "no new notice");
}

/// A stopped or rate-limited background turn never notifies: its terminal
/// is `info` and the web's `findLast` passes over it (model.ts:108-114).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stopped_or_rate_limited_background_turn_never_notifies() {
    let _g = lock();
    let core = core().await;
    let conv = launch(&core).await;
    let mut os = Os::default();
    let mut c = attention_model(&mut os, false);
    sync(&mut c, &mut os, &conv);
    let stopped = run_turn_in(&core, &conv, STOPPED, "try the refactor").await;
    sync(&mut c, &mut os, &conv);
    let limited = run_turn_in(&core, &conv, LIMITED, "summarize the logs").await;
    sync(&mut c, &mut os, &conv);
    open(&conv, MAIN).await;
    sync(&mut c, &mut os, &conv);
    finish(&core, STOPPED, &stopped, 3, "interrupted");
    finish(&core, LIMITED, &limited, 3, "rate_limited");
    until("both terminals reached the app", || {
        conv.store.domains.turn.terminal(&stopped).as_deref() == Some("interrupted")
            && conv.store.domains.turn.terminal(&limited).as_deref() == Some("rate_limited")
    })
    .await;
    quiet().await;
    sync(&mut c, &mut os, &conv);
    sync(&mut c, &mut os, &conv);
    assert!(os.posts.is_empty(), "a stopped or rate-limited turn never notifies: {:?}", os.posts);
    assert_eq!(c.tracker.count(), 0);
}

/// A background Session that starts waiting on the person notifies "needs
/// your input" naming it (a running -> waiting transition, model.ts:66-76).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_background_session_that_waits_for_an_answer_notifies_needs_input() {
    let _g = lock();
    let core = core().await;
    let conv = launch(&core).await;
    let mut os = Os::default();
    let mut c = attention_model(&mut os, false);
    sync(&mut c, &mut os, &conv);
    let turn = run_turn_in(&core, &conv, ASK, "pick a branch").await;
    sync(&mut c, &mut os, &conv);
    open(&conv, MAIN).await;
    sync(&mut c, &mut os, &conv);
    core.notify("user_question/requested", json!({"session_id": ASK, "turn_id": turn,
        "question_id": "01a0eb8f-7b23-7030-9f26-a28486425a01", "title": "Which branch?", "body": "1. Which branch?",
        "questions": [{"allow_free_text": true, "header": "Branch", "multi_select": false,
                       "options": [{"label": "main", "description": "the default"}], "question": "Which branch?"}]}));
    until("the background Session waits", || status_of(&conv, ASK) == Some(sidebar::Status::Waiting)).await;
    sync(&mut c, &mut os, &conv);
    assert_eq!(
        os.posts,
        vec![(attention::notice_id(ASK), "Pick a branch".to_owned(), attention::notice_body(TurnState::Waiting).to_owned())]
    );
}
