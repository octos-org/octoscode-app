//! A29 — parity row 6: the `/btw` aside is owned by the Session that asked.
//!
//! Production path end to end: a scripted fake AppUI server (the a7 pattern:
//! one WebSocket per connection, every request recorded) and the app's own
//! calls — `Conversation::submit_draft` (the composer's Enter with a typed
//! `/btw <question>`), `Conversation::open_session` (the sidebar's switch,
//! `lib.rs` `actions::Effect::Open`), the transport's events through
//! `Conversation::on_event` (the lib.rs event loop), the screens table's
//! `aside.*` ids (`screens::sessions`) and the sidebar's projection
//! (`screens::sidebar::project_recorded`, what `chrome::draw_sidebar_list`
//! draws).
//!
//! The server HOLDS each `session/btw` reply until the test releases it, so
//! the ordering hazards are reproduced deterministically (LESSONS: replies and
//! notifications apply on different tasks — every reconcile is idempotent):
//!
//! * ask in X, switch to Y, then the reply: the answer lands in X only, Y's
//!   composer and panel are untouched, and X's sidebar row (not Y's) carries
//!   the aside marker (web e2e `native-workflows.spec.ts:240`);
//! * dismiss while answering, then the reply: it stays hidden
//!   (`lazy-btw-controller.ts:110`);
//! * the connection drops while answering: the aside shows the stale copy at
//!   once and no later event revives it (`lazy-btw-controller.ts:74-78`);
//! * an ordinary prompt admitted in the Session clears a SETTLED aside, never
//!   an answering one (`clearSettled`, `use-octos-session.ts:2625`);
//! * a second ask while one is answering is refused and the draft is kept
//!   (`busy`, `lazy-btw-controller.ts:97`);
//! * `/btw` with no question reports how to use it and sends nothing
//!   (`intent.ts:96-104`).
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, FlowEvent};
use octoscode_module::screens::{sessions, sidebar};

const PROFILE: &str = "a29";
const X: &str = "a29:x";
const Y: &str = "a29:y";
const X_TITLE: &str = "Fix steer queue drop on reconnect";
const Y_TITLE: &str = "Why is hydrate slow?";
const QUESTION: &str = "why does redeliver drain the whole queue first?";

/// One `session/btw` the server has not answered yet.
struct Held {
    id: Value,
    session: String,
    question: String,
    out: mpsc::UnboundedSender<String>,
}

#[derive(Clone)]
struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
    held: Arc<Mutex<Vec<Held>>>,
    /// The live connections' writers (the newest last); a [`CLOSE`] frame
    /// closes that socket.
    conns: Arc<Mutex<Vec<mpsc::UnboundedSender<String>>>>,
    /// Also list `session/btw` among the FEATURES (the f30d fake's shape).
    btw_feature: bool,
    /// Withdraw `session/btw` from the methods (a server without asides).
    no_btw: bool,
}

const CLOSE: &str = "\u{0}close";

impl Server {
    async fn start() -> Self {
        Self::start_with(false).await
    }

    async fn start_with(btw_feature: bool) -> Self {
        Self::start_opts(btw_feature, false).await
    }

    async fn start_opts(btw_feature: bool, no_btw: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let srv = Server {
            base_url: format!("http://{}", listener.local_addr().expect("addr")),
            seen: Arc::new(Mutex::new(Vec::new())),
            held: Arc::new(Mutex::new(Vec::new())),
            conns: Arc::new(Mutex::new(Vec::new())),
            btw_feature,
            no_btw,
        };
        let s = srv.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { continue };
                let (mut sink, mut source) = ws.split();
                let (tx, mut rx) = mpsc::unbounded_channel::<String>();
                s.conns.lock().unwrap().push(tx.clone());
                tokio::spawn(async move {
                    while let Some(frame) = rx.recv().await {
                        if frame == CLOSE {
                            let _ = sink.close().await;
                            break;
                        }
                        if sink.send(Message::Text(frame.into())).await.is_err() {
                            break;
                        }
                    }
                });
                let s = s.clone();
                tokio::spawn(async move {
                    while let Some(Ok(msg)) = source.next().await {
                        let Message::Text(text) = msg else { continue };
                        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                        if v.get("id").is_none() {
                            continue;
                        }
                        let method = v["method"].as_str().unwrap_or("").to_owned();
                        let p = v["params"].clone();
                        s.seen.lock().unwrap().push((method.clone(), p.clone()));
                        if method == "session/btw" {
                            s.held.lock().unwrap().push(Held {
                                id: v["id"].clone(),
                                session: p["session_id"].as_str().unwrap_or("").to_owned(),
                                question: p["question"].as_str().unwrap_or("").to_owned(),
                                out: tx.clone(),
                            });
                            continue;
                        }
                        let mut result = answer(&method, &p, s.btw_feature);
                        if s.no_btw {
                            if let Some(list) = result["opened"]["capabilities"]["supported_methods"].as_array_mut() {
                                list.retain(|m| m != "session/btw");
                            }
                        }
                        let frame = json!({"jsonrpc": "2.0", "id": v["id"], "result": result});
                        let _ = tx.send(frame.to_string());
                    }
                });
            }
        });
        srv
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    async fn wait_for(&self, method: &str, n: usize) -> Vec<Value> {
        for _ in 0..334 {
            let p = self.params_of(method);
            if p.len() >= n {
                return p;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        self.params_of(method)
    }

    /// Answer every held `session/btw` the way octos does
    /// (`SessionBtwResult {session_id, answer, model}`), on the socket it
    /// arrived on.
    fn release(&self) -> usize {
        let held: Vec<Held> = std::mem::take(&mut *self.held.lock().unwrap());
        for h in &held {
            let result = json!({
                "session_id": h.session,
                "answer": format!("Aside for {}: {}", h.session, h.question),
                "model": "deepseek-v4-flash"
            });
            let _ = h.out.send(json!({"jsonrpc": "2.0", "id": h.id, "result": result}).to_string());
        }
        held.len()
    }

    /// Close every live socket (the transport re-dials: a reconnect).
    fn drop_connections(&self) {
        for c in self.conns.lock().unwrap().drain(..) {
            let _ = c.send(CLOSE.to_owned());
        }
    }
}

fn answer(method: &str, p: &Value, btw_feature: bool) -> Value {
    let session = p["session_id"].as_str().unwrap_or(X).to_owned();
    let mut features = vec!["state.session_hydrate.v1"];
    if btw_feature {
        features.push("session/btw");
    }
    match method {
        "session/open" => json!({"opened": {
            "session_id": session,
            "active_profile_id": PROFILE,
            "workspace_root": "/home/user/src/octos",
            "cursor": {"stream": session, "seq": 1},
            "capabilities": {
                "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                "capabilities_schema_version": 1,
                // The recorded live servers advertise `session/btw` as a
                // METHOD (r1-autonomy / live-gate / r43a open replies) — the
                // web gates on `supported_methods` (`btw.ts:66`,
                // `interaction.ts:14-22`).
                "supported_methods": ["session/open", "session/list", "session/hydrate", "session/btw",
                    "turn/start", "turn/interrupt"],
                "supported_notifications": ["turn/started", "turn/completed", "message/delta"],
                "supported_features": features
            }
        }}),
        "session/hydrate" => json!({"session_id": session, "cursor": {"stream": session, "seq": 1}, "messages": []}),
        "session/list" => json!({"sessions": [
            {"id": X, "title": X_TITLE, "message_count": 4, "updated_at": "2026-10-02T09:00:00Z", "active_turn": false},
            {"id": Y, "title": Y_TITLE, "message_count": 2, "updated_at": "2026-10-02T08:00:00Z", "active_turn": false}
        ]}),
        "turn/start" => json!({"accepted": true}),
        _ => json!({}),
    }
}

/// Connect, open X, then run the production event loop (lib.rs `start`'s
/// `while let Some(evt) = evt_rx.recv()` → `on_event`) on the runtime.
async fn connected(server: &Server) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", PROFILE, None, None).expect("connect");
    let conv = Arc::new(conv);
    conv.attach();
    conv.open_session(X, None).await.expect("session/open X");
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
    let drv = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            let _ = drv.on_event(evt);
        }
    });
    conv.refresh_sessions().await.expect("session/list");
    conv
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

/// The screens' state is process-global and the env-backed draft file is
/// shared: one test at a time, the drafts file in a temp dir (brief §8 —
/// never the operator's ~/.octoscode).
fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    let dir = std::env::temp_dir().join(format!("a29-btw-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    std::env::set_var("OCTOSCODE_DRAFTS_FILE", dir.join("drafts.json"));
    sessions::reset_state();
    g
}

/// One binding of the active Session's aside (`aside.state` / `aside.answer`
/// / `aside.question` / `aside.error`), `null` when the id is unknown.
fn binding(conv: &Conversation, id: &str) -> Value {
    let ui = conv.ui();
    let ctx = Ctx::new(&conv.store, &ui);
    sessions::query(&ctx, id).unwrap_or(Value::Null)
}

fn act(conv: &Conversation, id: &str) {
    let ui = conv.ui();
    let ctx = Ctx::new(&conv.store, &ui);
    let _ = sessions::resolve(id, 0, &ctx);
}

/// The sidebar row the shell draws for `title` (Debug form: the test reads
/// its fields without naming them, so it compiles before and after A29).
fn row(conv: &Conversation, title: &str) -> String {
    let proj = sidebar::project_recorded(&conv.store, sidebar::now_ms(), &[]);
    proj.rows
        .iter()
        .map(|r| format!("{r:?}"))
        .find(|r| r.starts_with("Session") && r.contains(&format!("title: {title:?}")))
        .unwrap_or_default()
}

async fn ask(conv: &Arc<Conversation>, text: &str) {
    conv.set_draft(text);
    conv.submit_draft().await.expect("submit");
}

async fn switch(conv: &Arc<Conversation>, id: &str) {
    conv.open_session(id, None).await.expect("session/open");
    until(&format!("{id} active"), || conv.store.active_session().as_deref() == Some(id)).await;
}

// ----------------------------------------------------------------- ownership

#[tokio::test]
async fn an_answer_after_a_switch_lands_only_in_the_asking_session() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    assert_eq!(conv.store.active_session().as_deref(), Some(X));

    // The user's path: `/btw <question>` typed in X's composer, Enter.
    ask(&conv, &format!("/btw {QUESTION}")).await;
    let sent = server.wait_for("session/btw", 1).await;
    assert_eq!(sent.len(), 1, "the typed /btw reached the wire");
    assert_eq!(
        sent[0],
        json!({"session_id": X, "question": QUESTION}),
        "the request carries the ASKING Session's id and the trimmed question (btw.ts:77-81)"
    );
    assert_eq!(conv.ui().lock().unwrap().draft(), "", "an accepted aside consumes the draft");
    assert_eq!(binding(&conv, "aside.state"), json!("answering"));
    assert_eq!(binding(&conv, "aside.question"), json!(QUESTION));
    assert!(server.params_of("turn/start").is_empty(), "an aside is never a turn");

    // Switch to Y BEFORE the reply (the sidebar's own call).
    switch(&conv, Y).await;
    conv.set_draft("Unsent Y draft");
    assert_eq!(binding(&conv, "aside.state"), json!("hidden"), "Y holds no aside");
    let x_row = row(&conv, X_TITLE);
    assert!(x_row.contains("aside: Some(Answering)"), "X's row marks its answering aside: {x_row}");

    // Now the reply for X arrives.
    assert_eq!(server.release(), 1);
    until("X's row marks an answered aside", || row(&conv, X_TITLE).contains("aside: Some(Answered)")).await;
    // …and it landed ONLY in X: Y's panel, draft and row are untouched.
    assert_eq!(conv.store.active_session().as_deref(), Some(Y), "the answer never switches Sessions");
    assert_eq!(binding(&conv, "aside.state"), json!("hidden"), "the answer never shows in Y");
    assert_eq!(binding(&conv, "aside.answer"), json!(""));
    assert_eq!(conv.ui().lock().unwrap().draft(), "Unsent Y draft", "Y's draft is untouched");
    let y_row = row(&conv, Y_TITLE);
    assert!(y_row.contains("aside: None"), "Y's row carries no marker: {y_row}");

    // Back to X: its answer is there.
    switch(&conv, X).await;
    assert_eq!(binding(&conv, "aside.state"), json!("answered"));
    assert_eq!(binding(&conv, "aside.answer"), json!(format!("Aside for {X}: {QUESTION}")));
    let x_row = row(&conv, X_TITLE);
    assert!(x_row.contains("aside: None"), "the selected Session shows its panel, not a marker: {x_row}");
    // The aside never became a transcript row.
    let rows = conv.store.domains.session.timeline.entries(X);
    assert!(rows.iter().all(|e| !e.text.contains(QUESTION)), "the aside is not saved to the conversation");
    assert_eq!(server.params_of("session/btw").len(), 1);
}

/// The screens table's own route (`aside.ask` → `sessions::spawn` →
/// `apply`, lib.rs): the ask is admitted in X, the user switches to Y before
/// the spawned call runs, and the call must STILL carry X's id — the scope is
/// captured at admission (`lazy-btw-controller.ts:89-95`), never re-read at
/// send time. Served with `session/btw` also among the features, so the old
/// features-only gate admits the ask and the capture itself is what is tested.
#[tokio::test]
async fn the_ask_captures_its_session_before_a_switch() {
    let _g = lock();
    let server = Server::start_with(true).await;
    let conv = connected(&server).await;
    conv.set_draft(QUESTION);
    let effect = {
        let ui = conv.ui();
        let ctx = Ctx::new(&conv.store, &ui);
        sessions::resolve("aside.ask", 0, &ctx)
    };
    switch(&conv, Y).await;
    let c = conv.clone();
    tokio::spawn(async move {
        let _ = sessions::apply(effect, &c).await;
    });
    let sent = server.wait_for("session/btw", 1).await;
    assert_eq!(sent.len(), 1, "the admitted ask went out");
    assert_eq!(sent[0]["session_id"], json!(X), "the asking Session's id, captured at admission");
    server.release();
    until("X's row marks an answered aside", || row(&conv, X_TITLE).contains("aside: Some(Answered)")).await;
    assert_eq!(binding(&conv, "aside.state"), json!("hidden"), "never shown in Y");
    switch(&conv, X).await;
    assert_eq!(binding(&conv, "aside.answer"), json!(format!("Aside for {X}: {QUESTION}")));
}

// ------------------------------------------------------------- ordering hazards

#[tokio::test]
async fn a_late_answer_after_dismiss_stays_hidden() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    ask(&conv, &format!("/btw {QUESTION}")).await;
    assert_eq!(server.wait_for("session/btw", 1).await.len(), 1);
    assert_eq!(binding(&conv, "aside.state"), json!("answering"));

    // Close while answering (the panel's Close, `lazy-btw-controller.ts:110`).
    act(&conv, "aside.dismiss");
    assert_eq!(binding(&conv, "aside.state"), json!("hidden"));
    assert_eq!(server.release(), 1);
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(binding(&conv, "aside.state"), json!("hidden"), "a late answer stays hidden");
    assert_eq!(binding(&conv, "aside.answer"), json!(""));
    assert!(row(&conv, X_TITLE).contains("aside: None"));

    // A new ask, then the OLD request's reply can never land in it.
    ask(&conv, "/btw a second question").await;
    assert_eq!(server.wait_for("session/btw", 2).await.len(), 2);
    assert_eq!(binding(&conv, "aside.question"), json!("a second question"));
    assert_eq!(binding(&conv, "aside.state"), json!("answering"));
}

#[tokio::test]
async fn a_connection_change_while_answering_stales_the_aside() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    ask(&conv, &format!("/btw {QUESTION}")).await;
    assert_eq!(server.wait_for("session/btw", 1).await.len(), 1);

    // The socket drops while the aside answers: the stale copy shows at once
    // (`lazy-btw-controller.ts:74-78`), red lead + muted cause.
    server.drop_connections();
    until("the aside goes stale", || binding(&conv, "aside.state") == json!("stale")).await;
    assert_eq!(
        binding(&conv, "aside.error"),
        json!("The Session connection changed before the aside completed. Ask again when it is ready.")
    );
    // The transport comes back; nothing revives the stale aside.
    until("live again", || conv.store.is_live()).await;
    let _ = server.release();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(binding(&conv, "aside.state"), json!("stale"), "terminal: no late event revives it");
    assert!(row(&conv, X_TITLE).contains("aside: None"), "X is selected: the panel shows, not a marker");
}

#[tokio::test]
async fn an_admitted_prompt_clears_a_settled_aside_never_an_answering_one() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    ask(&conv, &format!("/btw {QUESTION}")).await;
    assert_eq!(server.wait_for("session/btw", 1).await.len(), 1);

    // A prompt admitted while the aside answers leaves it answering.
    ask(&conv, "Make redeliver survive a reconnect.").await;
    assert_eq!(server.wait_for("turn/start", 1).await.len(), 1, "the ordinary prompt went out");
    assert_eq!(binding(&conv, "aside.state"), json!("answering"), "clearSettled skips an answering aside");

    server.release();
    until("answered", || binding(&conv, "aside.state") == json!("answered")).await;
    // The next admitted prompt clears the settled aside.
    ask(&conv, "And add a test for it.").await;
    until("cleared", || binding(&conv, "aside.state") == json!("hidden")).await;
}

#[tokio::test]
async fn a_second_ask_while_answering_is_busy_and_keeps_the_draft() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    ask(&conv, &format!("/btw {QUESTION}")).await;
    assert_eq!(server.wait_for("session/btw", 1).await.len(), 1);
    ask(&conv, "/btw and another one?").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(server.params_of("session/btw").len(), 1, "busy: nothing sent");
    assert_eq!(conv.ui().lock().unwrap().draft(), "/btw and another one?", "the rejected command keeps its draft");
    assert_eq!(binding(&conv, "aside.question"), json!(QUESTION), "the answering aside is untouched");
}

#[tokio::test]
async fn btw_without_a_question_reports_how_to_use_it_and_sends_nothing() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    ask(&conv, "/btw   ").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(server.params_of("session/btw").is_empty(), "nothing was sent to the model");
    let receipts: Vec<String> = conv
        .store
        .domains
        .session
        .timeline
        .entries(X)
        .into_iter()
        .map(|e| e.text)
        .filter(|t| t.contains("Use /btw"))
        .collect();
    assert_eq!(receipts.len(), 1, "one receipt row");
    assert!(
        receipts[0].contains("for a temporary side answer. Nothing was sent to the model."),
        "the web's copy (intent.ts:103): {receipts:?}"
    );
    assert_eq!(binding(&conv, "aside.state"), json!("hidden"), "no panel for a hint");
}

#[tokio::test]
async fn an_empty_composer_submit_dismisses_the_aside_and_a_programmatic_send_does_not() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    ask(&conv, &format!("/btw {QUESTION}")).await;
    assert_eq!(server.wait_for("session/btw", 1).await.len(), 1);
    server.release();
    until("answered", || binding(&conv, "aside.state") == json!("answered")).await;

    // Resume chat's programmatic send (`submit_draft` with an empty draft,
    // board3/session_pane.rs) is not the composer's submit: the aside stays.
    conv.set_draft("");
    conv.submit_draft().await.expect("submit");
    assert_eq!(binding(&conv, "aside.state"), json!("answered"));
    // The composer's own empty submit dismisses it (`App.tsx:1170-1173`).
    conv.submit_composer().await.expect("submit");
    assert_eq!(binding(&conv, "aside.state"), json!("hidden"));
    assert!(server.params_of("turn/start").is_empty(), "an empty submit starts nothing");
}

#[tokio::test]
async fn an_unadvertised_btw_reports_unavailable_and_sends_nothing() {
    let _g = lock();
    let server = Server::start_opts(false, true).await;
    let conv = connected(&server).await;
    ask(&conv, "/btw is this offered?").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(server.params_of("session/btw").is_empty(), "fails closed: nothing left the process (btw.ts:66)");
    let receipt = conv
        .store
        .domains
        .session
        .timeline
        .entries(X)
        .into_iter()
        .map(|e| e.text)
        .find(|t| t.contains("/btw is unavailable"));
    assert!(receipt.is_some(), "the web's report title (local-report.ts:114)");
    assert_eq!(binding(&conv, "aside.state"), json!("hidden"), "no visible aside state");
    assert!(server.params_of("turn/start").is_empty(), "the command never reaches the model");
}
