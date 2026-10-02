//! A20 — parity row 250 (session:links-resume): every approval or question is
//! associated with its EXACT origin Session/topic, on the production path.
//!
//! Web: `features/session/session-interaction-ledger.ts:83` (records keyed by
//! `[endpoint, sessionId] + kind + turnId + requestId`, `:115-125`; responses
//! go to the record's own owner after a generation preflight, `:400-481`;
//! hydrate restores only the asker's admitted entries, `:137-172`) and
//! `scope.ts:7-25` (a topicless Session is never a wildcard for its topics).
//! Web test: `session-interaction-ledger.test.ts` "rejects malformed,
//! foreign-topic and unnegotiated hydrate requests" (+ the switch/stale cases
//! beside it).
//!
//! The real `Conversation` (WS transport, flow, the client's production
//! handlers) against a scripted fake AppUI server that owns the parked
//! interactions the way Core does: `approval/requested` /
//! `user_question/requested` pushed on the socket, the canonical
//! `session/hydrate {include: ["pending_approvals"]}` answering what is still
//! parked per Session, and `approval/respond` / `user_question/respond`
//! accepted only for the Session that owns the request. Everything the app
//! sends is logged and asserted. The checks read the SAME functions the mounted
//! UI calls (`surfaces::takeover` / `perform` / `run`, the keyboard's
//! `keys::oldest_pending_id` + `keys::resolve`, the sidebar's
//! `session_status`). Only APIs that exist on main are used, so the file runs
//! against main as the failing-first proof.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use makepad_widgets::KeyCode;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octos_app_transport::TransportEvent;
use octoscode_module::flow::Conversation;
use octoscode_module::screens::keys::{self, KeyAction};
use octoscode_module::screens::sidebar::{self, Status};
use octoscode_module::screens::surfaces::{self, Job, Outcome, Takeover};

const PROFILE: &str = "a20";
const X: &str = "a20:api:xray";
const Y: &str = "a20:api:yankee";
const WS: &str = "/home/user/a20-ws";
const AP_X: &str = "01a0e773-f844-7d50-b171-b38d159f20a1";
const TURN_X: &str = "01920000-0000-7000-8000-0000000020b1";
const Q_X: &str = "01a0eb8f-7b23-7030-9f26-a284864220a2";
const Q_Y: &str = "01a0eb8f-7b23-7030-9f26-a284864220a3";
const TURN_Y: &str = "01920000-0000-7000-8000-0000000020b2";

// ------------------------------------------------------------ the server

/// What the fake server holds per Session, the way Core parks it.
#[derive(Default)]
struct Parked {
    approvals: HashMap<String, Vec<Value>>,
    questions: HashMap<String, Vec<Value>>,
    /// Extra raw entries the canonical hydrate of a Session also carries
    /// (malformed / foreign ones the client must drop).
    noise: HashMap<String, Vec<Value>>,
}

struct Server {
    port: u16,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
    parked: Arc<Mutex<Parked>>,
    /// `approval/respond` is advertised (the negotiation the hydrate needs).
    answerable: Arc<Mutex<bool>>,
    /// The parked-interaction hydrate fails (a restore that cannot run).
    fail_restore: Arc<Mutex<bool>>,
    push: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<String>>>>,
    accept: Mutex<Option<tokio::task::JoinHandle<()>>>,
    kick: tokio::sync::broadcast::Sender<()>,
}

impl Server {
    async fn start() -> Arc<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().unwrap().port();
        let (kick, _) = tokio::sync::broadcast::channel::<()>(8);
        let s = Arc::new(Self {
            port,
            seen: Arc::new(Mutex::new(Vec::new())),
            parked: Arc::new(Mutex::new(Parked::default())),
            answerable: Arc::new(Mutex::new(true)),
            fail_restore: Arc::new(Mutex::new(false)),
            push: Arc::new(Mutex::new(None)),
            accept: Mutex::new(None),
            kick,
        });
        s.serve(listener);
        s
    }

    fn base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn serve(self: &Arc<Self>, listener: TcpListener) {
        let me = self.clone();
        let h = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let me = me.clone();
                let mut kicked = me.kick.subscribe();
                tokio::spawn(async move {
                    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
                    let (mut tx, mut rx) = ws.split();
                    let (push_tx, mut push_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
                    *me.push.lock().unwrap() = Some(push_tx);
                    loop {
                        tokio::select! {
                            _ = kicked.recv() => {
                                let _ = tx.close().await;
                                return;
                            }
                            out = push_rx.recv() => {
                                let Some(out) = out else { return };
                                if tx.send(Message::Text(out.into())).await.is_err() {
                                    return;
                                }
                            }
                            msg = rx.next() => {
                                let Some(Ok(Message::Text(text))) = msg else { return };
                                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                                if v.get("id").is_none() {
                                    continue;
                                }
                                let method = v["method"].as_str().unwrap_or("").to_owned();
                                let p = v["params"].clone();
                                me.seen.lock().unwrap().push((method.clone(), p.clone()));
                                let frame = match me.answer(&method, &p) {
                                    Ok(r) => json!({"jsonrpc": "2.0", "id": v["id"].clone(), "result": r}),
                                    Err(e) => json!({"jsonrpc": "2.0", "id": v["id"].clone(), "error": e}),
                                };
                                if tx.send(Message::Text(frame.to_string().into())).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                });
            }
        });
        *self.accept.lock().unwrap() = Some(h);
    }

    fn answer(&self, method: &str, p: &Value) -> Result<Value, Value> {
        let session = p["session_id"].as_str().unwrap_or(X).to_owned();
        let mut methods = vec!["session/open", "session/hydrate", "session/list", "turn/start", "user_question/respond"];
        if *self.answerable.lock().unwrap() {
            methods.push("approval/respond");
        }
        match method {
            "session/open" => Ok(json!({"opened": {
                "session_id": session, "active_profile_id": PROFILE,
                "workspace_root": p["cwd"].as_str().unwrap_or(WS),
                "cursor": {"stream": session, "seq": 1},
                "capabilities": {
                    "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                    "capabilities_schema_version": 2,
                    "supported_methods": methods,
                    "supported_notifications": ["approval/requested", "user_question/requested"],
                    "supported_features": ["approval.typed.v1", "user_question.v1", "state.session_hydrate.v1"]
                }
            }})),
            "session/hydrate" => {
                let parked_read = p["include"].as_array().is_some_and(|a| a.iter().any(|x| x == "pending_approvals"));
                if !parked_read {
                    return Ok(json!({"session_id": session, "cursor": {"stream": session, "seq": 2}, "messages": []}));
                }
                if *self.fail_restore.lock().unwrap() {
                    return Err(json!({"code": -32603, "message": "hydrate unavailable"}));
                }
                let parked = self.parked.lock().unwrap();
                let mut approvals = parked.noise.get(&session).cloned().unwrap_or_default();
                approvals.extend(parked.approvals.get(&session).cloned().unwrap_or_default());
                Ok(json!({
                    "session_id": session, "cursor": {"stream": session, "seq": 2},
                    "pending_approvals": approvals,
                    "pending_questions": parked.questions.get(&session).cloned().unwrap_or_default()
                }))
            }
            "session/list" => Ok(json!({"sessions": [
                {"id": X, "title": "Session X", "message_count": 2, "updated_at": "2026-10-02T09:00:00Z"},
                {"id": Y, "title": "Session Y", "message_count": 2, "updated_at": "2026-10-02T08:00:00Z"}
            ]})),
            // Core answers a request only on the Session that owns it.
            "approval/respond" => {
                let id = p["approval_id"].as_str().unwrap_or_default().to_owned();
                let mut parked = self.parked.lock().unwrap();
                let list = parked.approvals.entry(session.clone()).or_default();
                match list.iter().position(|a| a["approval_id"] == json!(id)) {
                    Some(i) => {
                        list.remove(i);
                        Ok(json!({"approval_id": id, "accepted": true, "status": "accepted", "runtime_resumed": true}))
                    }
                    None => Err(json!({"code": -32602, "message": format!("approval {id} is not pending in {session}")})),
                }
            }
            "user_question/respond" => {
                let id = p["question_id"].as_str().unwrap_or_default().to_owned();
                let mut parked = self.parked.lock().unwrap();
                let list = parked.questions.entry(session.clone()).or_default();
                match list.iter().position(|q| q["question_id"] == json!(id)) {
                    Some(i) => {
                        list.remove(i);
                        Ok(json!({"question_id": id, "accepted": true, "runtime_resumed": true}))
                    }
                    None => Err(json!({"code": -32602, "message": format!("question {id} is not pending in {session}")})),
                }
            }
            _ => Ok(json!({})),
        }
    }

    fn send(&self, method: &str, params: Value) {
        let frame = json!({"jsonrpc": "2.0", "method": method, "params": params}).to_string();
        let tx = self.push.lock().unwrap().clone().expect("a live socket");
        tx.send(frame).expect("the socket is open");
    }

    /// Core parks an approval for `session`'s turn and announces it.
    fn raise_approval(&self, session: &str, approval_id: &str, turn: &str, command: &str) -> Value {
        let a = approval(session, approval_id, turn, command);
        self.parked.lock().unwrap().approvals.entry(session.to_owned()).or_default().push(a.clone());
        self.send("approval/requested", a.clone());
        a
    }

    fn raise_question(&self, session: &str, question_id: &str, turn: &str) {
        let q = question(session, question_id, turn);
        self.parked.lock().unwrap().questions.entry(session.to_owned()).or_default().push(q.clone());
        self.send("user_question/requested", q);
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    /// The server process dies (every socket drops) …
    async fn stop(&self) {
        if let Some(h) = self.accept.lock().unwrap().take() {
            h.abort();
        }
        let _ = self.kick.send(());
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    /// … and comes back on the SAME port.
    async fn restart(self: &Arc<Self>) {
        let mut tries = 0;
        let listener = loop {
            match TcpListener::bind(("127.0.0.1", self.port)).await {
                Ok(l) => break l,
                Err(e) if tries < 50 => {
                    tries += 1;
                    let _ = e;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => panic!("rebind {}: {e}", self.port),
            }
        };
        self.serve(listener);
    }
}

/// The recorded r5 typed approval's shape (`approval/requested`), for `session`.
fn approval(session: &str, approval_id: &str, turn: &str, command: &str) -> Value {
    json!({
        "approval_id": approval_id, "approval_kind": "command", "body": command, "risk": "high",
        "session_id": session, "title": "Approve command", "tool_name": "shell", "turn_id": turn,
        "typed_details": {"kind": "command", "command": {"argv": command.split(' ').collect::<Vec<_>>(),
                          "command_line": command, "tool_call_id": format!("call-{turn}")}}
    })
}

/// The recorded r23 question's shape (`user_question/requested`), for `session`.
fn question(session: &str, question_id: &str, turn: &str) -> Value {
    json!({
        "body": "1. Which color would you like to pick?", "question_id": question_id,
        "questions": [{"allow_free_text": true, "header": "Color choice", "multi_select": false,
                       "options": [{"description": "Calm", "label": "Blue"}, {"description": "Fresh", "label": "Green"}],
                       "question": "Which color would you like to pick?"}],
        "session_id": session, "title": "Which color would you like to pick?", "turn_id": turn
    })
}

// ------------------------------------------------------------ the client

type Events = tokio::sync::mpsc::Receiver<TransportEvent>;

/// The surfaces' state is process-global; one test at a time.
fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    surfaces::reset();
    g
}

/// Fold transport events until `done` holds (bounded).
async fn fold_until(conv: &Conversation, events: &mut Events, secs: u64, done: impl Fn(&Conversation) -> bool) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    while tokio::time::Instant::now() < deadline {
        if done(conv) {
            return true;
        }
        if let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(50), events.recv()).await {
            conv.on_event(evt);
        }
    }
    done(conv)
}

/// Fold whatever arrives for `ms`.
async fn settle(conv: &Conversation, events: &mut Events, ms: u64) {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(ms);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(50), events.recv()).await {
            conv.on_event(evt);
        }
    }
}

/// Open `session` (the sidebar row's production open) and fold until it is
/// the Session on screen and its parked-interaction read was answered.
async fn open(server: &Server, conv: &Arc<Conversation>, events: &mut Events, session: &str) {
    let reads = |s: &Server| {
        s.params_of("session/hydrate")
            .iter()
            .filter(|p| p["session_id"] == json!(session) && p["include"] == json!(["pending_approvals"]))
            .count()
    };
    let before = reads(server);
    let opener = {
        let c = conv.clone();
        let id = session.to_owned();
        tokio::spawn(async move { c.open_session(&id, Some(WS.to_owned())).await })
    };
    assert!(
        fold_until(conv, events, 10, |c| c.store.is_live() && c.store.active_session().as_deref() == Some(session)).await,
        "{session} is the Session on screen"
    );
    opener.await.unwrap().expect("session/open");
    assert!(fold_until(conv, events, 10, |_| reads(server) > before).await, "{session}'s parked interactions were read");
    settle(conv, events, 300).await;
}

async fn connected(server: &Arc<Server>) -> (Arc<Conversation>, Events) {
    let (conv, events) = Conversation::connect(&server.base(), "dummy", PROFILE, None, None).expect("connect");
    let conv = Arc::new(conv);
    conv.attach();
    (conv, events)
}

/// What a bare `y` resolves to in the Session on screen (the shell's
/// `Event::KeyDown` resolver over the keyboard's own gate, lib.rs).
fn bare_y(conv: &Conversation) -> KeyAction {
    let store = &conv.store;
    keys::resolve(
        KeyCode::KeyY,
        false,
        false,
        false,
        false,
        false,
        keys::oldest_pending_id(store).is_some(),
        keys::preview_id(store),
        false,
        true,
    )
}

fn spawn_of(o: Outcome) -> Job {
    match o {
        Outcome::Spawn(job) => job,
        other => panic!("expected a spawned job, got {other:?}"),
    }
}

// ------------------------------------------------------------- the tests

/// Session X raises an approval; Session Y is opened. Y never shows it, its
/// keyboard never answers it, its sidebar row never waits on it — X's does.
/// Back on X the approval is X's again (restored by X's canonical hydrate)
/// and answering it sends X's exact ids.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switching_sessions_never_shows_or_answers_another_sessions_approval() {
    let _g = lock();
    let server = Server::start().await;
    let (conv, mut events) = connected(&server).await;
    open(&server, &conv, &mut events, X).await;
    server.raise_approval(X, AP_X, TURN_X, "rm -rf scratch_dir");
    assert!(
        fold_until(&conv, &mut events, 5, |c| surfaces::takeover(&c.store) == Some(Takeover::Approval(AP_X.into()))).await,
        "X's approval takes X's composer over"
    );

    // ---- Session Y on screen.
    open(&server, &conv, &mut events, Y).await;
    let store = &conv.store;
    assert_eq!(surfaces::takeover(store), None, "Y never shows X's approval");
    assert_eq!(keys::oldest_pending_id(store), None, "Y's keyboard has nothing to answer");
    assert_eq!(bare_y(&conv), KeyAction::Ignore, "a bare `y` typed in Y is typing, not X's approval");
    let ui = conv.ui();
    assert_eq!(surfaces::perform("cv.approval.once", 0, store, &ui), Outcome::Done, "no decision from Y");
    assert_ne!(sidebar::session_status(store, Y, Some(Y)), Status::Waiting, "Y's row does not wait on X's approval");
    assert_eq!(sidebar::session_status(store, X, Some(Y)), Status::Waiting, "X's row shows X is waiting");
    assert!(server.params_of("approval/respond").is_empty(), "nothing was answered from Y");

    // ---- Back on X: X's approval again, answered with X's ids.
    open(&server, &conv, &mut events, X).await;
    let store = &conv.store;
    assert_eq!(surfaces::takeover(store), Some(Takeover::Approval(AP_X.into())), "X's approval is X's");
    let job = spawn_of(surfaces::perform("cv.approval.once", 0, store, &ui));
    surfaces::run(job, &conv).await.expect("accepted for X");
    assert_eq!(
        server.params_of("approval/respond"),
        vec![json!({"session_id": X, "approval_id": AP_X, "decision": "approve", "approval_scope": "request"})],
        "exactly one response, to its origin"
    );
    assert_eq!(surfaces::takeover(store), None, "decided: X's composer is back");
}

/// Each Session keeps its own question: Y's question never replaces X's,
/// and each is answered only on its own Session with its own ids.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_session_keeps_and_answers_only_its_own_question() {
    let _g = lock();
    let server = Server::start().await;
    let (conv, mut events) = connected(&server).await;
    open(&server, &conv, &mut events, X).await;
    server.raise_question(X, Q_X, TURN_X);
    assert!(fold_until(&conv, &mut events, 5, |c| surfaces::takeover(&c.store) == Some(Takeover::Question(Q_X.into()))).await);

    open(&server, &conv, &mut events, Y).await;
    assert_eq!(surfaces::takeover(&conv.store), None, "Y never shows X's question");
    server.raise_question(Y, Q_Y, TURN_Y);
    assert!(fold_until(&conv, &mut events, 5, |c| surfaces::takeover(&c.store) == Some(Takeover::Question(Q_Y.into()))).await);
    assert_eq!(sidebar::session_status(&conv.store, X, Some(Y)), Status::Waiting, "X still waits on ITS question");

    // Answer Y's on Y.
    let ui = conv.ui();
    surfaces::perform("cv.q.opt", 0, &conv.store, &ui);
    let job = spawn_of(surfaces::submit_question(&conv.store));
    surfaces::run(job, &conv).await.expect("Y's answer accepted");

    // X's question survived Y's: back on X it is X's, answered with X's ids.
    open(&server, &conv, &mut events, X).await;
    assert_eq!(surfaces::takeover(&conv.store), Some(Takeover::Question(Q_X.into())), "X's question is still X's");
    surfaces::perform("cv.q.opt", 1, &conv.store, &ui);
    let job = spawn_of(surfaces::submit_question(&conv.store));
    surfaces::run(job, &conv).await.expect("X's answer accepted");
    assert_eq!(
        server.params_of("user_question/respond"),
        vec![
            json!({"session_id": Y, "question_id": Q_Y, "answers": [{"selected_labels": ["Blue"]}]}),
            json!({"session_id": X, "question_id": Q_X, "answers": [{"selected_labels": ["Green"]}]}),
        ],
        "each answer went to the Session that asked it"
    );
}

/// A request for X's TOPIC child (`session_id` X + `topic`) is not X's: a
/// topicless Session is a scope, never a wildcard for its topics.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_foreign_topic_request_never_takes_its_base_session_over() {
    let _g = lock();
    let server = Server::start().await;
    let (conv, mut events) = connected(&server).await;
    open(&server, &conv, &mut events, X).await;
    let mut a = approval(X, AP_X, TURN_X, "git push --force");
    a["topic"] = json!("peer-review");
    server.send("approval/requested", a);
    let mut q = question(X, Q_X, TURN_X);
    q["topic"] = json!("peer-review");
    server.send("user_question/requested", q);
    settle(&conv, &mut events, 600).await;
    assert_eq!(surfaces::takeover(&conv.store), None, "X never shows its topic child's request");
    assert_eq!(keys::oldest_pending_id(&conv.store), None, "and its keyboard cannot answer it");
    assert_ne!(sidebar::session_status(&conv.store, X, Some(X)), Status::Waiting);
}

/// The web test the row names: "rejects malformed, foreign-topic and
/// unnegotiated hydrate requests" — over the canonical hydrate of X, each
/// parked entry is admitted on its own; the valid one is X's.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_restore_admits_only_the_askers_well_formed_negotiated_entries() {
    let _g = lock();
    let server = Server::start().await;
    {
        let mut parked = server.parked.lock().unwrap();
        parked.noise.insert(
            X.to_owned(),
            vec![
                json!({"approval_id": "incomplete", "turn_id": TURN_X}),
                approval("other:api:session", "01a0e773-f844-7d50-b171-b38d159f20f1", TURN_X, "printf foreign"),
                {
                    let mut t = approval(X, "01a0e773-f844-7d50-b171-b38d159f20f2", TURN_X, "printf topic");
                    t["topic"] = json!("peer-b");
                    t
                },
            ],
        );
        parked.approvals.insert(X.to_owned(), vec![approval(X, AP_X, TURN_X, "rm -rf scratch_dir")]);
    }
    let (conv, mut events) = connected(&server).await;
    open(&server, &conv, &mut events, X).await;
    assert!(
        fold_until(&conv, &mut events, 5, |c| surfaces::takeover(&c.store) == Some(Takeover::Approval(AP_X.into()))).await,
        "the well-formed parked approval of X is restored, attributed to X: {:?}",
        conv.store.domains.approval.pending()
    );
    for dropped in ["incomplete", "01a0e773-f844-7d50-b171-b38d159f20f1", "01a0e773-f844-7d50-b171-b38d159f20f2"] {
        assert!(conv.store.domains.approval.detail(dropped).is_none(), "{dropped} is never armed");
    }
    assert!(conv.store.domains.approval.detail(AP_X).is_some_and(|d| d.session_id == X));

    // Unnegotiated: a server that does not advertise approval/respond restores
    // nothing (on a fresh connection).
    let server = Server::start().await;
    *server.answerable.lock().unwrap() = false;
    server.parked.lock().unwrap().approvals.insert(X.to_owned(), vec![approval(X, AP_X, TURN_X, "rm -rf scratch_dir")]);
    let (conv, mut events) = connected(&server).await;
    let opener = {
        let c = conv.clone();
        tokio::spawn(async move { c.open_session(X, Some(WS.to_owned())).await })
    };
    assert!(fold_until(&conv, &mut events, 10, |c| c.store.active_session().as_deref() == Some(X)).await);
    opener.await.unwrap().expect("open");
    settle(&conv, &mut events, 600).await;
    assert_eq!(surfaces::takeover(&conv.store), None, "an unnegotiated parked approval is never a card");
}

/// The socket is replaced while X's approval waits and the canonical restore
/// cannot run: the record belongs to the retired transport generation, so
/// answering it is refused before any RPC, and the card says why.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_response_on_a_replaced_socket_fails_closed_until_restored() {
    let _g = lock();
    let server = Server::start().await;
    let (conv, mut events) = connected(&server).await;
    open(&server, &conv, &mut events, X).await;
    server.raise_approval(X, AP_X, TURN_X, "rm -rf scratch_dir");
    assert!(fold_until(&conv, &mut events, 5, |c| surfaces::takeover(&c.store).is_some()).await);

    // The server dies and comes back; its canonical restore is unavailable.
    *server.fail_restore.lock().unwrap() = true;
    server.stop().await;
    assert!(fold_until(&conv, &mut events, 10, |c| !c.store.is_live()).await, "the drop is seen");
    server.restart().await;
    let reopened = |s: &Server| s.params_of("session/open").len();
    let opens = reopened(&server);
    assert!(
        fold_until(&conv, &mut events, 20, |c| c.store.is_live() && reopened(&server) > opens).await,
        "re-dialed and re-opened"
    );
    settle(&conv, &mut events, 600).await;
    assert_eq!(surfaces::takeover(&conv.store), Some(Takeover::Approval(AP_X.into())), "the card still shows");

    let ui = conv.ui();
    let refused = match surfaces::perform("cv.approval.once", 0, &conv.store, &ui) {
        Outcome::Spawn(job) => surfaces::run(job, &conv).await.is_err(),
        _ => true,
    };
    assert!(refused, "a stale record is never answered");
    assert!(server.params_of("approval/respond").is_empty(), "nothing reached the wire");
    let error = surfaces::state().approval.error.clone();
    assert!(
        error.as_ref().is_some_and(|(id, e)| id == AP_X && e.contains("current Session generation")),
        "the card says why: {error:?}"
    );

    // Once the canonical restore runs (re-open X), it is X's and answerable.
    *server.fail_restore.lock().unwrap() = false;
    surfaces::reset();
    open(&server, &conv, &mut events, X).await;
    let job = spawn_of(surfaces::perform("cv.approval.once", 0, &conv.store, &ui));
    surfaces::run(job, &conv).await.expect("accepted after the restore");
    assert_eq!(server.params_of("approval/respond").len(), 1);
}
