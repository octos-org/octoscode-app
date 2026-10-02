//! A7 — the composer's turn controller at the wire (production path).
//!
//! A scripted fake server (the f12/a4 pattern: one WebSocket, every request's
//! method + params recorded, notifications pushed by the test) answers the
//! SAME calls the app makes: `Conversation::submit_draft` (the composer's
//! Enter / send), `steer_queued_head` (the queued chip's "Steer now"),
//! `remove_queued` (its ✕), `check_turn_state` / `continue_without_turn`
//! (the recovery notice), `interrupt`, and the transport's own events through
//! `Conversation::on_event` (the lib.rs event loop) — and asserts what reached
//! the wire and what the store's composer domain now holds.
//!
//! Covered (the web's tests they port): FIFO queueing and removal
//! (`turn-queue.test.ts:47/:81`, `use-turn-controller.test.ts:38/:88`), the
//! typed collision (`turn-collision.test.ts`, `chat-handover.spec.ts:258`),
//! steering into the captured owner and the in-order restore of a refused
//! steer (`turn-steering.test.ts:141/:160`), an unknown-outcome start held for
//! recovery and resolved by `turn/state/get` or released by "Continue"
//! (`use-turn-controller.test.ts:226/:399/:426`), and durable ownership
//! (a hydrated / foreign active turn is observed, never owned; a reconnect
//! hands the lease off — `:911/:925/:1038`).
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::{Conversation, FlowEvent};
use octoscode_store::domains::composer::{Origin, Ownership, RecoveryPhase};

const PROFILE: &str = "a7";
const OCCUPIER: &str = "3f1a9c52-4d1b-4c2e-8f6a-0b7d21e9c4aa";

enum Reply {
    Ok(Value),
    Err(i64, &'static str, Option<Value>),
    /// Never answer (a lost ACK).
    Silent,
}

type Script = Arc<dyn Fn(&str, &Value) -> Reply + Send + Sync>;

struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
    push: Arc<Mutex<Option<mpsc::UnboundedSender<String>>>>,
}

impl Server {
    async fn start(script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
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
                        let frame = match script(&method, &v["params"]) {
                            Reply::Ok(result) => json!({"jsonrpc": "2.0", "id": v["id"], "result": result}),
                            Reply::Err(code, message, data) => {
                                let mut e = json!({"code": code, "message": message});
                                if let Some(d) = data {
                                    e["data"] = d;
                                }
                                json!({"jsonrpc": "2.0", "id": v["id"], "error": e})
                            }
                            Reply::Silent => continue,
                        };
                        let _ = tx.send(frame.to_string());
                    }
                });
            }
        });
        Self { base_url, seen, push }
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    fn notify(&self, method: &str, params: Value) {
        let frame = json!({"jsonrpc": "2.0", "method": method, "params": params}).to_string();
        if let Some(tx) = self.push.lock().unwrap().as_ref() {
            let _ = tx.send(frame);
        }
    }

    async fn wait_for(&self, method: &str, n: usize) -> Vec<Value> {
        for _ in 0..100 {
            let p = self.params_of(method);
            if p.len() >= n {
                return p;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        self.params_of(method)
    }
}

fn open_reply(params: &Value) -> Value {
    let session = params["session_id"].as_str().unwrap_or("a7:main");
    json!({"opened": {
        "session_id": session,
        "active_profile_id": PROFILE,
        "workspace_root": "/home/user/src/octos",
        "cursor": {"stream": session, "seq": 1},
        "capabilities": {
            "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
            "capabilities_schema_version": 1,
            "supported_methods": ["session/open", "session/list", "session/hydrate", "turn/start",
                "turn/interrupt", "turn/steer", "turn/state/get"],
            "supported_notifications": ["turn/started", "turn/completed", "message/delta", "turn/steer_dropped"],
            "supported_features": ["event.turn_steer_dropped.v1"]
        }
    }})
}

/// The default answers: open, list, everything else `{}`.
fn base(method: &str, params: &Value) -> Option<Reply> {
    match method {
        "session/open" => Some(Reply::Ok(open_reply(params))),
        "session/list" => Some(Reply::Ok(json!({"sessions": []}))),
        _ => None,
    }
}

/// Connect + open, then run the production event loop (lib.rs `start`'s
/// `while let Some(evt) = evt_rx.recv()` → `on_event`) on the runtime.
async fn connected(server: &Server) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", PROFILE, None, None).expect("connect");
    let conv = Arc::new(conv);
    conv.attach();
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
    let drv = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            let _ = drv.on_event(evt);
        }
    });
    conv
}

async fn until(what: &str, f: impl Fn() -> bool) {
    for _ in 0..150 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for: {what}");
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    // A lost ACK resolves in 1.5 s here (the app's default is the web's 30 s).
    std::env::set_var("OCTOSCODE_TURN_START_TIMEOUT_MS", "1500");
    let dir = std::env::temp_dir().join(format!("a7-composer-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    std::env::set_var("OCTOSCODE_DRAFTS_FILE", dir.join("drafts.json"));
    g
}

fn submit(conv: &Arc<Conversation>, text: &str) -> tokio::task::JoinHandle<String> {
    conv.set_draft(text);
    let c = conv.clone();
    tokio::spawn(async move { c.submit_draft().await.unwrap_or_default() })
}

fn notices(conv: &Conversation) -> Vec<(String, String)> {
    let session = conv.session_id();
    conv.store
        .domains
        .session
        .timeline
        .entries(&session)
        .into_iter()
        .filter(|e| e.kind == octoscode_store::EntryKind::SYSTEM_NOTICE)
        .map(|e| (e.data["title"].as_str().unwrap_or("").to_owned(), e.text.clone()))
        .collect()
}

fn texts(starts: &[Value]) -> Vec<String> {
    starts.iter().map(|p| p["input"][0]["text"].as_str().unwrap_or("").to_owned()).collect()
}

// ------------------------------------------------------------------ queue

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_prompt_while_a_turn_runs_queues_fifo_and_drains_on_the_terminal() {
    let _g = lock();
    let server = Server::start(Arc::new(|m: &str, p: &Value| {
        base(m, p).unwrap_or(match m {
            "turn/start" => Reply::Ok(json!({"accepted": true})),
            _ => Reply::Ok(json!({})),
        })
    }))
    .await;
    let conv = connected(&server).await;
    let session = conv.session_id();

    let first = submit(&conv, "one").await.unwrap();
    assert!(!first.is_empty(), "the first prompt starts now");
    assert_eq!(texts(&server.wait_for("turn/start", 1).await), ["one"]);
    assert_eq!(conv.store.domains.composer.ownership(&session), Ownership::LocalOwner);
    assert_eq!(conv.ui().lock().unwrap().draft(), "", "the composer clears on admission");

    // While it runs, later prompts queue: no second turn/start races it.
    submit(&conv, "two").await.unwrap();
    submit(&conv, "three").await.unwrap();
    let snap = conv.store.domains.composer.snapshot(&session);
    assert_eq!(snap.pending.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(), ["two", "three"]);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(server.params_of("turn/start").len(), 1, "queued prompts wait");

    // The queued chip's ✕: a pending prompt is removed without touching the
    // server (no interrupt, no frame).
    let two = snap.pending[0].turn_id.clone();
    assert!(conv.remove_queued(&two));
    assert!(server.params_of("turn/interrupt").is_empty());

    // The active turn's terminal starts the next head, exactly once.
    server.notify("turn/completed", json!({"session_id": session, "turn_id": first}));
    let starts = server.wait_for("turn/start", 2).await;
    assert_eq!(texts(&starts), ["one", "three"], "FIFO, the removed prompt never sent");
    let third = starts[1]["turn_id"].as_str().unwrap().to_owned();
    until("the next head is active", || {
        conv.store.domains.composer.snapshot(&session).active.map(|a| a.turn_id) == Some(third.clone())
    })
    .await;
    server.notify("turn/completed", json!({"session_id": session, "turn_id": third}));
    until("the queue drains", || conv.store.domains.composer.snapshot(&session).active.is_none()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(server.params_of("turn/start").len(), 2);
}

// -------------------------------------------------------------- collision

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_typed_collision_keeps_the_text_and_waits_for_the_other_clients_turn() {
    let _g = lock();
    let server = Server::start(Arc::new(|m: &str, p: &Value| {
        base(m, p).unwrap_or(match m {
            "turn/start" if p["input"][0]["text"] == "mine" => Reply::Err(
                -32600,
                "a turn is already running for this session",
                Some(json!({"kind": "turn_in_progress", "turn_id": OCCUPIER})),
            ),
            "turn/start" => Reply::Ok(json!({"accepted": true})),
            _ => Reply::Ok(json!({})),
        })
    }))
    .await;
    let conv = connected(&server).await;
    let session = conv.session_id();

    submit(&conv, "mine").await.unwrap();
    server.wait_for("turn/start", 1).await;
    until("the busy notice", || notices(&conv).iter().any(|(t, _)| t == "Session busy")).await;
    let (_, body) = notices(&conv).into_iter().find(|(t, _)| t == "Session busy").unwrap();
    assert!(body.starts_with("Another client was working in this session"), "{body}");
    assert!(!notices(&conv).iter().any(|(t, _)| t == "Turn rejected"), "never reported as a rejection");
    assert_eq!(conv.ui().lock().unwrap().draft(), "mine", "the text is not thrown away");
    let active = conv.store.domains.composer.snapshot(&session).active.unwrap();
    assert_eq!((active.turn_id.as_str(), active.origin), (OCCUPIER, Origin::Adopted));
    assert_eq!(conv.store.domains.composer.ownership(&session), Ownership::Observed);

    // A prompt sent now waits behind the occupier, then goes once it ends.
    submit(&conv, "after").await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(server.params_of("turn/start").len(), 1);
    server.notify("turn/completed", json!({"session_id": session, "turn_id": OCCUPIER}));
    let starts = server.wait_for("turn/start", 2).await;
    assert_eq!(texts(&starts), ["mine", "after"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_start_without_typed_data_is_a_rejection_that_returns_the_text() {
    let _g = lock();
    let server = Server::start(Arc::new(|m: &str, p: &Value| {
        base(m, p).unwrap_or(match m {
            "turn/start" => Reply::Err(-32602, "cwd is not accessible", None),
            _ => Reply::Ok(json!({})),
        })
    }))
    .await;
    let conv = connected(&server).await;
    let session = conv.session_id();
    let _ = submit(&conv, "denied").await;
    until("the rejection notice", || notices(&conv).iter().any(|(t, b)| t == "Turn rejected" && b == "cwd is not accessible")).await;
    assert_eq!(conv.ui().lock().unwrap().draft(), "denied");
    assert!(conv.store.domains.composer.snapshot(&session).active.is_none(), "nothing holds the queue");
    assert!(!conv.ui().lock().unwrap().turn_active(), "no live turn for a start that never ran");
}

// --------------------------------------------------------------- steering

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn steer_now_steers_into_the_captured_turn_and_a_refused_steer_is_restored_in_order() {
    let _g = lock();
    let refuse = Arc::new(Mutex::new(false));
    let r2 = refuse.clone();
    let server = Server::start(Arc::new(move |m: &str, p: &Value| {
        base(m, p).unwrap_or(match m {
            "turn/start" => Reply::Ok(json!({"accepted": true})),
            "turn/steer" if *r2.lock().unwrap() => Reply::Err(-32600, "steer refused", None),
            "turn/steer" => Reply::Ok(json!({"turn_id": p["expected_turn_id"], "steered": true})),
            _ => Reply::Ok(json!({})),
        })
    }))
    .await;
    let conv = connected(&server).await;
    let session = conv.session_id();
    let owner = submit(&conv, "work").await.unwrap();
    server.wait_for("turn/start", 1).await;
    submit(&conv, "also add a test").await.unwrap();
    submit(&conv, "and docs").await.unwrap();
    assert!(conv.can_steer(), "turn/steer + event.turn_steer_dropped.v1 advertised");
    assert!(conv.store.domains.composer.can_steer_now(&session));

    // "Steer now": the head goes into the CAPTURED turn — no second start.
    assert!(conv.steer_queued_head().await);
    let steer = server.wait_for("turn/steer", 1).await;
    assert_eq!(steer[0]["expected_turn_id"], json!(owner));
    assert_eq!(steer[0]["input"][0]["text"], json!("also add a test"));
    assert!(notices(&conv).iter().any(|(t, b)| t == "Steering accepted" && b == "also add a test"));
    assert_eq!(server.params_of("turn/start").len(), 1, "steering never races a second turn/start");
    let pending: Vec<String> = conv.store.domains.composer.snapshot(&session).pending.into_iter().map(|p| p.text).collect();
    assert_eq!(pending, ["and docs"]);

    // A refused steer comes back AHEAD of later prompts.
    submit(&conv, "later").await.unwrap();
    *refuse.lock().unwrap() = true;
    assert!(conv.steer_queued_head().await);
    server.wait_for("turn/steer", 2).await;
    until("the steer is restored", || notices(&conv).iter().any(|(t, _)| t == "Steering queued")).await;
    let pending: Vec<String> = conv.store.domains.composer.snapshot(&session).pending.into_iter().map(|p| p.text).collect();
    assert_eq!(pending, ["and docs", "later"], "restored in order");

    // /steer on: a mid-turn submission steers instead of queueing (once the
    // queue is empty), and the opt-in says what it does.
    for t in conv.store.domains.composer.snapshot(&session).pending {
        conv.remove_queued(&t.turn_id);
    }
    *refuse.lock().unwrap() = false;
    conv.set_draft("/steer on");
    assert_eq!(conv.submit_draft().await.unwrap(), "", "a local command, never a turn");
    assert!(conv.store.domains.composer.steering_enabled(&session));
    let receipt = conv.store.domains.session.timeline.entries(&session).into_iter().rev()
        .find(|e| e.kind == octoscode_module::screens::palette::REPORT_KIND).map(|e| e.text).unwrap_or_default();
    assert_eq!(receipt, "Steering enabled for this Session. Eligible mid-turn text is sent to the active turn; other inputs remain queued.");
    submit(&conv, "steer me directly").await.unwrap();
    let steer = server.wait_for("turn/steer", 3).await;
    assert_eq!(steer[2]["input"][0]["text"], json!("steer me directly"));
    assert_eq!(server.params_of("turn/start").len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn returned_steering_goes_back_to_the_queue_on_steer_dropped() {
    let _g = lock();
    let server = Server::start(Arc::new(|m: &str, p: &Value| {
        base(m, p).unwrap_or(match m {
            "turn/start" => Reply::Ok(json!({"accepted": true})),
            "turn/steer" => Reply::Ok(json!({"turn_id": p["expected_turn_id"], "steered": true})),
            _ => Reply::Ok(json!({})),
        })
    }))
    .await;
    let conv = connected(&server).await;
    let session = conv.session_id();
    let owner = submit(&conv, "work").await.unwrap();
    server.wait_for("turn/start", 1).await;
    submit(&conv, "late steer").await.unwrap();
    assert!(conv.steer_queued_head().await);
    // Core could not drain it before the turn ended: it comes back first.
    server.notify(
        "turn/steer_dropped",
        json!({"session_id": session, "turn_id": owner, "inputs": ["late steer"], "reason": "turn_ended"}),
    );
    until("the returned steer", || notices(&conv).iter().any(|(t, _)| t == "Steering returned to queue")).await;
    server.notify("turn/completed", json!({"session_id": session, "turn_id": owner}));
    let starts = server.wait_for("turn/start", 2).await;
    assert_eq!(texts(&starts), ["work", "late steer"], "the returned steer becomes the next turn");
}

// --------------------------------------------------------------- recovery

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unacknowledged_start_is_held_then_recovered_by_turn_state_get() {
    let _g = lock();
    let server = Server::start(Arc::new(|m: &str, p: &Value| {
        base(m, p).unwrap_or(match m {
            "turn/start" if p["input"][0]["text"] == "lost" => Reply::Silent,
            "turn/start" => Reply::Ok(json!({"accepted": true})),
            "turn/state/get" => Reply::Ok(json!({"session_id": p["session_id"], "turn_id": p["turn_id"], "state": "completed"})),
            _ => Reply::Ok(json!({})),
        })
    }))
    .await;
    let conv = connected(&server).await;
    let session = conv.session_id();
    let lost = submit(&conv, "lost").await.unwrap();
    assert!(!lost.is_empty());
    until("the timeout notice", || notices(&conv).iter().any(|(t, _)| t == "Turn start timed out")).await;
    assert_eq!(conv.store.domains.composer.recovery(&session).unwrap().phase, RecoveryPhase::Unknown);

    // Sending is paused: nothing new is admitted, the text stays.
    conv.set_draft("next");
    conv.submit_draft().await.unwrap();
    assert_eq!(conv.ui().lock().unwrap().draft(), "next", "held, not sent and not cleared");
    assert_eq!(server.params_of("turn/start").len(), 1);

    // "Check status": the lifecycle lookup proves completion; never resent.
    conv.check_turn_state().await;
    let checks = server.params_of("turn/state/get");
    assert_eq!(checks[0], json!({"session_id": session, "turn_id": lost}));
    assert_eq!(conv.store.domains.composer.recovery(&session), None);
    assert!(conv.store.domains.composer.snapshot(&session).active.is_none());
    assert_eq!(server.params_of("turn/start").len(), 1, "the held turn is never resent");
    conv.submit_draft().await.unwrap();
    assert_eq!(texts(&server.wait_for("turn/start", 2).await), ["lost", "next"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn continue_without_it_releases_an_unknown_turn_and_sends_the_queue() {
    let _g = lock();
    let server = Server::start(Arc::new(|m: &str, p: &Value| {
        base(m, p).unwrap_or(match m {
            "turn/start" if p["input"][0]["text"] == "lost" => Reply::Silent,
            "turn/start" => Reply::Ok(json!({"accepted": true})),
            "turn/state/get" => Reply::Ok(json!({"session_id": p["session_id"], "turn_id": p["turn_id"], "state": "unknown"})),
            _ => Reply::Ok(json!({})),
        })
    }))
    .await;
    let conv = connected(&server).await;
    let session = conv.session_id();
    let first = submit(&conv, "lost");
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Queued BEFORE the timeout resolves: it must wait.
    submit(&conv, "queued").await.unwrap();
    first.await.unwrap();
    conv.check_turn_state().await;
    assert_eq!(conv.store.domains.composer.recovery(&session).unwrap().phase, RecoveryPhase::Unknown);
    conv.continue_without_turn();
    assert!(notices(&conv).iter().any(|(t, b)| t == "Response outcome unknown"
        && b == "Continued without confirming this response. No stop request or resubmission was sent."));
    let starts = server.wait_for("turn/start", 2).await;
    assert_eq!(texts(&starts), ["lost", "queued"], "queued messages send next");
    assert!(server.params_of("turn/interrupt").is_empty(), "nothing was stopped");
}

// ------------------------------------------------------ durable ownership

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ownership_is_kept_by_a_hydrate_and_handed_off_by_a_reconnect() {
    let _g = lock();
    let turn = Arc::new(Mutex::new(String::new()));
    let t2 = turn.clone();
    let server = Server::start(Arc::new(move |m: &str, p: &Value| {
        base(m, p).unwrap_or(match m {
            "turn/start" => {
                *t2.lock().unwrap() = p["turn_id"].as_str().unwrap_or("").to_owned();
                Reply::Ok(json!({"accepted": true}))
            }
            "session/hydrate" => Reply::Ok(json!({
                "session_id": p["session_id"], "cursor": {"stream": "s", "seq": 9},
                "messages": [], "turns": [{"turn_id": *t2.lock().unwrap(), "state": "active"}]
            })),
            _ => Reply::Ok(json!({})),
        })
    }))
    .await;
    let conv = connected(&server).await;
    let session = conv.session_id();
    submit(&conv, "long job").await.unwrap();
    server.wait_for("turn/start", 1).await;
    assert_eq!(conv.store.domains.composer.ownership(&session), Ownership::LocalOwner);
    // A same-transport hydrate keeps the accepted owner.
    conv.reconcile_from_hydrate(true).await;
    assert_eq!(conv.store.domains.composer.ownership(&session), Ownership::LocalOwner);
    // The transport drops and comes back: the lease is handed off, and the
    // canonical hydrate (WITH turns) shows the turn active → observed.
    use octos_app_transport::{ConnectionState, TransportEvent};
    conv.on_event(TransportEvent::ConnectionState(ConnectionState::Reconnecting { attempt: 1 }));
    conv.on_event(TransportEvent::ConnectionState(ConnectionState::Live));
    let hydrates = server.wait_for("session/hydrate", 2).await;
    assert_eq!(hydrates[1]["include"], json!(["messages", "turns"]));
    until("observed after reconnect", || conv.store.domains.composer.ownership(&session) == Ownership::Observed).await;
    // A prompt now queues behind the observed turn; its terminal releases it.
    submit(&conv, "next").await.unwrap();
    let active = turn.lock().unwrap().clone();
    server.notify("turn/completed", json!({"session_id": session, "turn_id": active}));
    assert_eq!(texts(&server.wait_for("turn/start", 2).await), ["long job", "next"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn another_clients_turn_started_is_observed_and_the_composer_queues_behind_it() {
    let _g = lock();
    let server = Server::start(Arc::new(|m: &str, p: &Value| {
        base(m, p).unwrap_or(match m {
            "turn/start" => Reply::Ok(json!({"accepted": true})),
            _ => Reply::Ok(json!({})),
        })
    }))
    .await;
    let conv = connected(&server).await;
    let session = conv.session_id();
    server.notify(
        "turn/started",
        json!({"session_id": session, "turn_id": OCCUPIER, "timestamp": "2026-10-01T10:00:00Z"}),
    );
    until("adopted", || conv.store.domains.composer.ownership(&session) == Ownership::Observed).await;
    submit(&conv, "mine").await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(server.params_of("turn/start").is_empty(), "never raced over another client's turn");
    server.notify("turn/completed", json!({"session_id": session, "turn_id": OCCUPIER}));
    assert_eq!(texts(&server.wait_for("turn/start", 1).await), ["mine"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_before_the_ack_sends_no_interrupt_and_an_interrupted_prompt_returns_on_its_terminal() {
    let _g = lock();
    let server = Server::start(Arc::new(|m: &str, p: &Value| {
        base(m, p).unwrap_or(match m {
            "turn/start" if p["input"][0]["text"] == "slow ack" => Reply::Silent,
            "turn/start" => Reply::Ok(json!({"accepted": true})),
            "turn/interrupt" => Reply::Ok(json!({"interrupted": true})),
            _ => Reply::Ok(json!({})),
        })
    }))
    .await;
    let conv = connected(&server).await;
    let session = conv.session_id();
    let pending = submit(&conv, "slow ack");
    tokio::time::sleep(Duration::from_millis(150)).await;
    let starting = conv.store.domains.composer.snapshot(&session).active.unwrap().turn_id;
    conv.interrupt(&starting).await.unwrap();
    assert!(server.params_of("turn/interrupt").is_empty(), "Core has not accepted it yet");
    assert!(notices(&conv).iter().any(|(t, _)| t == "Turn is still starting"));
    pending.await.unwrap();
    conv.continue_without_turn();

    let owner = submit(&conv, "stop me").await.unwrap();
    conv.interrupt(&owner).await.unwrap();
    assert_eq!(server.wait_for("turn/interrupt", 1).await.len(), 1);
    conv.interrupt(&owner).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(server.params_of("turn/interrupt").len(), 1, "Stop is de-duplicated");
    server.notify("turn/error", json!({"session_id": session, "turn_id": owner, "code": "interrupted", "message": "Stopped"}));
    until("the prompt returns", || conv.ui().lock().unwrap().draft() == "stop me").await;
}
