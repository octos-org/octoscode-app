//! A12 — the outage on the production path: a live connection whose server
//! dies (its port closes) and comes back on the SAME port. The real WS
//! transport, the real `Conversation` (link + flow), a scripted fake AppUI
//! server that can be stopped and restarted.
//!
//! Web reference: `active-session-runtime.ts` keeps an opened Session through
//! a drop (`#scheduleReconnect` -> `#reconnect` -> `openSession` -> `#hydrate`,
//! `:951-1069`) and shows "Reconnecting to Octos" (`App.tsx:2724-2754`); only
//! a voluntary `disconnect()` (`:769-784`) ends it.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octos_app_transport::{ConnectionState, OutboundCommand, TransportEvent};
use octoscode_module::flow::{link, Conversation};

const PROFILE: &str = "a12";

/// A fake AppUI server on a fixed port that can die and come back.
struct Server {
    port: u16,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
    /// Connections accepted (sockets), across restarts.
    sockets: Arc<Mutex<usize>>,
    accept: Mutex<Option<tokio::task::JoinHandle<()>>>,
    kick: tokio::sync::broadcast::Sender<()>,
    /// `session/open` replies to swallow (then the socket is closed): the
    /// "dropped before its first answer" case.
    swallow_opens: Arc<Mutex<usize>>,
    /// Turns streamed so far: (turn id, prompt) — the hydrate returns them.
    turns: Arc<Mutex<Vec<(String, String)>>>,
    /// `session/open` requests to REFUSE with a JSON-RPC error.
    refuse_opens: Arc<Mutex<usize>>,
}

impl Server {
    async fn start() -> Arc<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().unwrap().port();
        let (kick, _) = tokio::sync::broadcast::channel::<()>(8);
        let s = Arc::new(Self {
            port,
            seen: Arc::new(Mutex::new(Vec::new())),
            sockets: Arc::new(Mutex::new(0)),
            accept: Mutex::new(None),
            kick,
            swallow_opens: Arc::new(Mutex::new(0)),
            turns: Arc::new(Mutex::new(Vec::new())),
            refuse_opens: Arc::new(Mutex::new(0)),
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
                *me.sockets.lock().unwrap() += 1;
                let seen = me.seen.clone();
                let swallow = me.swallow_opens.clone();
                let turns = me.turns.clone();
                let refuse = me.refuse_opens.clone();
                let mut kicked = me.kick.subscribe();
                tokio::spawn(async move {
                    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
                    let (tx, mut rx) = ws.split();
                    let tx = Arc::new(tokio::sync::Mutex::new(tx));
                    loop {
                        let msg = tokio::select! {
                            m = rx.next() => m,
                            _ = kicked.recv() => {
                                let _ = tx.lock().await.close().await;
                                return;
                            }
                        };
                        let Some(Ok(Message::Text(text))) = msg else { return };
                        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                        if v.get("id").is_none() {
                            continue;
                        }
                        let method = v["method"].as_str().unwrap_or("").to_owned();
                        let p = v["params"].clone();
                        seen.lock().unwrap().push((method.clone(), p.clone()));
                        let session = p["session_id"].as_str().unwrap_or("a12:main").to_owned();
                        if method == "session/open" {
                            let swallowed = {
                                let mut n = swallow.lock().unwrap();
                                let s = *n > 0;
                                if s {
                                    *n -= 1;
                                }
                                s
                            };
                            if swallowed {
                                // Never answered: the socket goes away first.
                                let _ = tx.lock().await.close().await;
                                return;
                            }
                            let refused = {
                                let mut n = refuse.lock().unwrap();
                                let r = *n > 0;
                                if r {
                                    *n -= 1;
                                }
                                r
                            };
                            if refused {
                                let frame = json!({"jsonrpc": "2.0", "id": v["id"].clone(),
                                    "error": {"code": -32603, "message": "session is being restored, try again"}}).to_string();
                                let _ = tx.lock().await.send(Message::Text(frame.into())).await;
                                continue;
                            }
                        }
                        if method == "slow/never" {
                            continue; // a request that is never answered
                        }
                        if method == "turn/start" {
                            // Accept, then stream one scripted turn (the
                            // recorded a6ea8505 envelope shapes); the turn is
                            // remembered so a later hydrate returns it the way
                            // Core does: thread_id = the turn UUID, no turn_id.
                            let turn = p["turn_id"].as_str().unwrap_or_default().to_owned();
                            let prompt = p["input"][0]["text"].as_str().unwrap_or_default().to_owned();
                            turns.lock().unwrap().push((turn.clone(), prompt));
                            let ok = json!({"jsonrpc": "2.0", "id": v["id"].clone(), "result": {"accepted": true}}).to_string();
                            let _ = tx.lock().await.send(Message::Text(ok.into())).await;
                            let env = |seq: u64, payload: Value| {
                                json!({"session_id": session, "thread_id": turn, "turn_id": turn, "seq": seq,
                                       "cursor": {"stream": session, "seq": 100 + seq}, "payload": payload})
                            };
                            for (m, params) in [
                                ("turn/started", json!({"session_id": session, "turn_id": turn, "timestamp": "2026-10-02T00:00:00Z"})),
                                ("projection/envelope", env(1, json!({"type": "assistant_delta", "data": {"text": "2 + 3 = 5.", "assistant_segment_id": format!("{turn}:assistant:iteration:1")}}))),
                                ("turn/completed", json!({"session_id": session, "turn_id": turn})),
                            ] {
                                tokio::time::sleep(Duration::from_millis(60)).await;
                                let frame = json!({"jsonrpc": "2.0", "method": m, "params": params}).to_string();
                                let _ = tx.lock().await.send(Message::Text(frame.into())).await;
                            }
                            continue;
                        }
                        let result = match method.as_str() {
                            "session/open" => json!({"opened": {
                                "session_id": session, "active_profile_id": PROFILE, "workspace_root": "/home/user/octos",
                                "cursor": {"stream": session, "seq": 1},
                                "capabilities": {
                                    "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                                    "capabilities_schema_version": 2,
                                    "supported_methods": ["session/open", "session/hydrate", "session/list"],
                                    "supported_notifications": [],
                                    "supported_features": ["state.session_hydrate.v1"]
                                }
                            }}),
                            "session/hydrate" => {
                                let streamed = turns.lock().unwrap().clone();
                                let messages: Vec<Value> = if streamed.is_empty() {
                                    vec![
                                        json!({"seq": 10, "role": "user", "content": "why 5?", "turn_id": "0b3f6d5e-1111-4111-8111-111111111111", "persisted_at": "2026-09-30T00:00:00Z"}),
                                        json!({"seq": 11, "role": "assistant", "content": "because five", "turn_id": "0b3f6d5e-1111-4111-8111-111111111111", "persisted_at": "2026-09-30T00:00:01Z"}),
                                    ]
                                } else {
                                    // Core's shape: thread_id = the turn UUID, no turn_id.
                                    streamed
                                        .iter()
                                        .enumerate()
                                        .flat_map(|(i, (turn, prompt))| {
                                            vec![
                                                json!({"seq": 2 * i as u64, "role": "user", "content": prompt, "thread_id": turn, "persisted_at": "2026-10-02T00:00:00Z"}),
                                                json!({"seq": 2 * i as u64 + 1, "role": "assistant", "content": "2 + 3 = 5.", "thread_id": turn, "persisted_at": "2026-10-02T00:00:01Z"}),
                                            ]
                                        })
                                        .collect()
                                };
                                json!({
                                    "session_id": session, "cursor": {"stream": session, "seq": 12},
                                    "messages": messages,
                                    "pending_approvals": [], "pending_questions": []
                                })
                            }
                            "session/list" => json!({"sessions": []}),
                            _ => json!({}),
                        };
                        let frame = json!({"jsonrpc": "2.0", "id": v["id"].clone(), "result": result}).to_string();
                        let _ = tx.lock().await.send(Message::Text(frame.into())).await;
                    }
                });
            }
        });
        *self.accept.lock().unwrap() = Some(h);
    }

    /// The server process dies: the port closes and every socket drops.
    async fn stop(&self) {
        if let Some(h) = self.accept.lock().unwrap().take() {
            h.abort();
        }
        let _ = self.kick.send(());
        // Let the aborted accept loop drop its listener (the port closes).
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    /// The server comes back on the SAME port.
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

    fn count(&self, method: &str) -> usize {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).count()
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }
}

/// Fold transport events until `done` holds (bounded by `secs`).
async fn fold_until(
    conv: &Conversation,
    events: &mut tokio::sync::mpsc::Receiver<TransportEvent>,
    secs: u64,
    done: impl Fn(&Conversation) -> bool,
) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    while tokio::time::Instant::now() < deadline {
        if done(conv) {
            return true;
        }
        if let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(100), events.recv()).await {
            conv.on_event(evt);
        }
    }
    done(conv)
}

async fn connected(server: &Server) -> (Arc<Conversation>, tokio::sync::mpsc::Receiver<TransportEvent>) {
    let (conv, mut events) = Conversation::connect(&server.base(), "dummy", PROFILE, None, None).expect("connect");
    let conv = Arc::new(conv);
    conv.attach();
    // The drain runs while the open waits for its session/list reply (the
    // Connect path's order).
    let opener = {
        let c = conv.clone();
        tokio::spawn(async move { c.open_workspace(None).await })
    };
    assert!(fold_until(&conv, &mut events, 10, |c| c.store.is_live()).await, "live");
    opener.await.unwrap().expect("open");
    (conv, events)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_live_connection_that_drops_keeps_the_shell_and_reopens_on_the_same_server() {
    octoscode_module::screens::reconnect::clear_held();
    let server = Server::start().await;
    let (conv, mut events) = connected(&server).await;
    let session = conv.session_id();
    assert_eq!(server.count("session/open"), 1);
    assert!(conv.store.keeps_shell());

    // The server dies.
    server.stop().await;
    assert!(
        fold_until(&conv, &mut events, 10, |c| c.store.outage().is_some_and(|o| o.attempt >= 1)).await,
        "the outage is retained: {:?}",
        conv.store.connection()
    );
    assert!(!conv.store.is_live());
    assert!(conv.store.keeps_shell(), "the shell stays (no Connect card)");
    let o = conv.store.outage().unwrap();
    assert_eq!(o.endpoint, server.base(), "the banner names the server in use, never a default");
    assert!(conv.in_outage());

    // A request during the outage fails AT ONCE (nothing parks in a dead
    // socket's queue), with the named refusal.
    let t0 = std::time::Instant::now();
    let r = conv.client().request("session/list", json!({})).await;
    assert!(t0.elapsed() < Duration::from_secs(2), "refused at once");
    match r {
        Err(octoscode_client::ClientError::Rpc { error, .. }) => {
            assert_eq!(error.code, link::NOT_CONNECTED_CODE);
            assert_eq!(error.message, link::NOT_CONNECTED);
        }
        other => panic!("expected the not-connected refusal, got {other:?}"),
    }

    // The composer refuses honestly: the draft stays, the banner says why,
    // nothing reaches the wire.
    conv.set_draft("unsent words");
    let id = conv.submit_draft().await.expect("submit");
    assert!(id.is_empty(), "no turn started");
    assert_eq!(conv.ui_ref().lock().unwrap().draft(), "unsent words", "the draft is kept");
    assert_eq!(
        octoscode_module::screens::reconnect::held().as_deref(),
        Some("Not sent — your text stays in the composer.")
    );
    assert_eq!(server.count("turn/start"), 0);

    // The server comes back on the same port: the transport re-dials, the
    // flow re-opens the SAME Session and hydrates it; Live again.
    server.restart().await;
    assert!(
        fold_until(&conv, &mut events, 40, |c| c.store.is_live() && server.count("session/open") >= 2).await,
        "reconnected: {:?}",
        conv.store.connection()
    );
    let opens = server.params_of("session/open");
    assert_eq!(opens.last().unwrap()["session_id"], json!(session), "the same Session");
    assert!(fold_until(&conv, &mut events, 5, |_| server
        .params_of("session/hydrate")
        .iter()
        .any(|p| p["session_id"] == json!(session) && p["include"] == json!(["messages"])))
    .await);
    assert!(conv.store.outage().is_none(), "the banner goes");
    assert!(!conv.in_outage());
    assert_eq!(conv.ui_ref().lock().unwrap().draft(), "unsent words", "the draft survived the outage");
    // The wire works again.
    assert!(conv.client().request("session/list", json!({})).await.is_ok());
}

/// The live proof's duplicated turn: a turn streamed live, the server dies
/// and returns, the re-hydrate brings that turn back the way Core sends it
/// (`thread_id` = the turn UUID, no `turn_id`): the transcript shows it ONCE,
/// and the next prompt streams on the recovered connection.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_streamed_turn_is_not_duplicated_by_the_reconnect_rehydrate_and_the_next_prompt_streams() {
    use octoscode_store::EntryKind;
    let server = Server::start().await;
    let (conv, mut events) = connected(&server).await;
    let session = conv.session_id();
    let sent = {
        let c = conv.clone();
        tokio::spawn(async move { c.start_turn("what is 2 + 3?").await })
    };
    assert!(
        fold_until(&conv, &mut events, 10, |c| c
            .store
            .domains
            .session
            .timeline
            .of_kind(&c.session_id(), EntryKind::ASSISTANT_TEXT)
            .iter()
            .any(|e| e.text == "2 + 3 = 5."))
        .await,
        "the turn streamed live"
    );
    sent.await.unwrap().expect("turn/start accepted");
    let count = |c: &Conversation, k: EntryKind| c.store.domains.session.timeline.of_kind(&session, k).len();
    let (users, answers) = (count(&conv, EntryKind::USER_MESSAGE), count(&conv, EntryKind::ASSISTANT_TEXT));
    assert_eq!((users, answers), (1, 1));

    server.stop().await;
    assert!(fold_until(&conv, &mut events, 10, |c| c.store.outage().is_some()).await);
    server.restart().await;
    let hydrates_before = server.count("session/hydrate");
    assert!(
        fold_until(&conv, &mut events, 40, |c| c.store.is_live() && server.count("session/hydrate") > hydrates_before).await,
        "recovered and re-hydrated"
    );
    // Let the hydrate reply fold.
    fold_until(&conv, &mut events, 2, |_| false).await;
    assert_eq!(count(&conv, EntryKind::USER_MESSAGE), 1, "the prompt shows once after the re-hydrate");
    assert_eq!(count(&conv, EntryKind::ASSISTANT_TEXT), 1, "the answer shows once after the re-hydrate");

    // The next prompt streams on the recovered connection.
    let next = {
        let c = conv.clone();
        tokio::spawn(async move { c.start_turn("and again?").await })
    };
    assert!(
        fold_until(&conv, &mut events, 10, |c| c.store.domains.session.timeline.of_kind(&session, EntryKind::ASSISTANT_TEXT).len() == 2)
            .await,
        "the next turn streamed after the reconnect"
    );
    next.await.unwrap().expect("the next turn/start accepted");
}

/// The re-open on the re-dialed socket is REFUSED: the outage says "Session
/// recovery required" with the reason (the web's third recovery phase), the
/// Session is not switched away while down, and Retry now re-dials and
/// re-opens until it is back.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_reopen_requires_recovery_and_retry_now_brings_the_session_back() {
    let server = Server::start().await;
    let (conv, mut events) = connected(&server).await;
    let session = conv.session_id();
    server.stop().await;
    assert!(fold_until(&conv, &mut events, 10, |c| c.store.outage().is_some()).await);
    // While down, no Session change: a switch is refused and the active
    // Session stays (the reconnect would otherwise re-open the other one).
    let r = conv.open_session("a12:other", None).await;
    assert!(r.is_err(), "no open while reconnecting: {r:?}");
    assert_eq!(conv.session_id(), session);
    assert_eq!(conv.store.active_session().as_deref(), Some(session.as_str()));
    *server.refuse_opens.lock().unwrap() = 1;
    server.restart().await;
    assert!(
        fold_until(&conv, &mut events, 40, |c| c.store.outage().is_some_and(|o| o.error.is_some())).await,
        "the refused re-open is surfaced: {:?}",
        conv.store.outage()
    );
    let o = conv.store.outage().unwrap();
    assert_eq!(o.error.as_deref(), Some("session is being restored, try again"));
    assert!(!conv.store.is_live() && conv.store.keeps_shell(), "still the shell, not live");
    // Retry now: a fresh transport re-dials, the Session re-opens.
    assert!(conv.retry_now());
    assert!(
        fold_until(&conv, &mut events, 40, |c| c.store.is_live()).await,
        "recovered after Retry now: {:?}",
        conv.store.connection()
    );
    assert!(conv.store.outage().is_none());
    assert_eq!(server.params_of("session/open").last().unwrap()["session_id"], json!(session));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_socket_that_drops_before_its_first_open_answers_reopens_on_the_redial() {
    let server = Server::start().await;
    *server.swallow_opens.lock().unwrap() = 1;
    let (conv, mut events) = Conversation::connect(&server.base(), "dummy", PROFILE, None, None).expect("connect");
    let conv = Arc::new(conv);
    let opener = {
        let c = conv.clone();
        tokio::spawn(async move { c.open_workspace(None).await })
    };
    // The first open is never answered (its socket closes). Before A12 the
    // re-dialed socket parked in Handshaking forever (the re-open was keyed
    // on an earlier Live).
    assert!(
        fold_until(&conv, &mut events, 30, |c| c.store.is_live()).await,
        "the re-dialed socket re-opens the Session and goes live: {:?}",
        conv.store.connection()
    );
    assert!(server.count("session/open") >= 2);
    assert!(*server.sockets.lock().unwrap() >= 2);
    let _ = tokio::time::timeout(Duration::from_secs(5), opener).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_request_whose_socket_drops_fails_instead_of_hanging() {
    let server = Server::start().await;
    let (conv, mut events) = connected(&server).await;
    let client = conv.client().clone();
    let pending = tokio::spawn(async move { client.request("slow/never", json!({})).await });
    // Let it reach the wire.
    assert!(fold_until(&conv, &mut events, 5, |_| server.count("slow/never") == 1).await);
    server.stop().await;
    assert!(fold_until(&conv, &mut events, 10, |c| c.in_outage()).await);
    let r = tokio::time::timeout(Duration::from_secs(5), pending).await.expect("not hanging").unwrap();
    match r {
        // A transport failure (not a server refusal): the turn controller
        // treats a start lost this way as UNCONFIRMED, never as rejected.
        Err(octoscode_client::ClientError::Transport { .. }) => {}
        other => panic!("expected a transport failure, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retry_now_and_a_give_up_replace_the_transport_under_the_same_conversation() {
    let server = Server::start().await;
    let (conv, mut events) = connected(&server).await;
    let session = conv.session_id();
    conv.store.domains.session.timeline.append(&session, None, octoscode_store::EntryKind::USER_MESSAGE, "kept".into());
    server.stop().await;
    assert!(fold_until(&conv, &mut events, 10, |c| c.store.outage().is_some()).await);
    let before = conv.store.outage().unwrap().attempt;
    // Retry now: a fresh transport at once (the old one stops speaking).
    assert!(conv.retry_now(), "retry while the outage is retained");
    // The transport gave up (its 5-minute budget): the flow starts a fresh
    // one instead of dropping to the Connect card (the web retries an
    // opened Session forever).
    conv.on_event(TransportEvent::ConnectionState(ConnectionState::Failed));
    assert!(conv.store.keeps_shell(), "still the shell after a give-up");
    assert!(conv.store.outage().is_some());
    server.restart().await;
    assert!(
        fold_until(&conv, &mut events, 40, |c| c.store.is_live()).await,
        "the replaced transport reconnects: {:?}",
        conv.store.connection()
    );
    assert!(server.params_of("session/open").last().unwrap()["session_id"] == json!(session));
    assert!(
        conv.store.domains.session.timeline.entries(&session).iter().any(|e| e.text == "kept"),
        "the same store: the transcript survived both replacements"
    );
    let _ = before;
    // Retry is refused when there is nothing to retry.
    assert!(!conv.retry_now());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_voluntary_disconnect_during_an_outage_is_final() {
    let server = Server::start().await;
    let (conv, mut events) = connected(&server).await;
    server.stop().await;
    assert!(fold_until(&conv, &mut events, 10, |c| c.store.outage().is_some()).await);
    // A9's Disconnect (Settings > Connection, or the banner's): the command
    // through the stable sender, the store Offline.
    let _ = conv.command_sender().send(OutboundCommand::Disconnect).await;
    conv.store.set_connection("Offline".into(), false);
    assert!(!conv.store.keeps_shell(), "the Connect card returns");
    assert!(conv.store.outage().is_none());
    let opens = server.count("session/open");
    server.restart().await;
    // Nothing reconnects, nothing re-opens, the store stays Offline.
    fold_until(&conv, &mut events, 4, |_| false).await;
    assert_eq!(server.count("session/open"), opens, "no re-open after a voluntary leave");
    assert_eq!(conv.store.connection(), "Offline");
    assert!(!conv.retry_now());
    // The conversation's event stream ENDS (its drain task finishes, as it
    // did when its one transport exited) — nothing parks forever.
    let ended = tokio::time::timeout(Duration::from_secs(5), async {
        while events.recv().await.is_some() {}
    })
    .await;
    assert!(ended.is_ok(), "the event stream closed after the leave");
}
