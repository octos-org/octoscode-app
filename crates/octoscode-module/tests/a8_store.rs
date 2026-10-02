//! A8 — the session store's runtime rules on the production path: the
//! reconnect re-open + canonical hydrate under a new authority generation
//! (web `active-session-runtime.ts`: connect -> open -> hydrate -> ready), a
//! hydrate reply from a retired generation failing closed
//! (`active-session-runtime.test.ts` "fails recovery preparation before
//! committing the hydrate cursor"), and the runtime scope key
//! (`session-scope.ts`). A scripted fake AppUI server records every request
//! and can hold `session/hydrate` replies back, so a generation change can
//! land BEFORE the reply — the race the rule exists for.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::AsyncWriteExt;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octos_app_transport::TransportEvent;
use octoscode_module::flow::{Conversation, FlowEvent};

const PROFILE: &str = "a8";

struct FakeServer {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
    /// Closes every socket open at the time (the server side of an outage).
    kick: tokio::sync::broadcast::Sender<()>,
}

/// The recorded a6ea8505 live-turn frames (r22-live's `projection/envelope`
/// shape: `{session_id, thread_id, turn_id, seq, cursor, payload: {type,
/// data}}`) for one scripted turn: thinking, a shell tool, the answer.
fn turn_script(session: &str, turn: &str) -> Vec<(&'static str, Value)> {
    let env = |seq: u64, payload: Value| {
        json!({"session_id": session, "thread_id": turn, "turn_id": turn, "seq": seq,
               "cursor": {"stream": session, "seq": 100 + seq}, "payload": payload})
    };
    vec![
        ("turn/started", json!({"session_id": session, "turn_id": turn, "timestamp": "2026-10-01T10:00:00Z"})),
        ("projection/envelope", env(1, json!({"type": "reasoning_delta", "data": {"text": "Reading the steer queue first."}}))),
        ("projection/envelope", env(2, json!({"type": "tool_start", "data": {"tool_call_id": "call-a8-1", "name": "shell", "arguments_preview": "cargo test"}}))),
        ("projection/envelope", env(3, json!({"type": "tool_end", "data": {"tool_call_id": "call-a8-1", "status": "complete", "output_preview": "ok"}}))),
        ("projection/envelope", env(4, json!({"type": "assistant_delta", "data": {"text": "The queue re-drains now.", "assistant_segment_id": format!("{turn}:assistant:iteration:1")}}))),
        ("turn/completed", json!({"session_id": session, "turn_id": turn})),
    ]
}

impl FakeServer {
    /// `hydrate_delay`: how long each `session/hydrate` reply is held back.
    async fn start(hydrate_delay: Duration) -> Self {
        Self::start_with(hydrate_delay, false).await
    }

    /// `stream_turns`: `turn/start` is accepted and [`turn_script`] streams.
    async fn start_with(hydrate_delay: Duration, stream_turns: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let s2 = seen.clone();
        let (kick, _) = tokio::sync::broadcast::channel::<()>(4);
        let kick2 = kick.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let seen = s2.clone();
                let mut kicked = kick2.subscribe();
                tokio::spawn(async move {
                    // REST: the drafts' principal read (`/api/auth/me`).
                    let mut head = [0u8; 64];
                    let n = stream.peek(&mut head).await.unwrap_or(0);
                    if String::from_utf8_lossy(&head[..n]).starts_with("GET /api/auth/me") {
                        let mut stream = stream;
                        let body = r#"{"user":{"id":"a8-user"}}"#;
                        let resp = format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
                        let _ = stream.write_all(resp.as_bytes()).await;
                        let _ = stream.shutdown().await;
                        return;
                    }
                    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
                    let (tx, mut rx) = ws.split();
                    let tx = Arc::new(tokio::sync::Mutex::new(tx));
                    let closer = tx.clone();
                    tokio::spawn(async move {
                        if kicked.recv().await.is_ok() {
                            let _ = closer.lock().await.close().await;
                        }
                    });
                    while let Some(Ok(msg)) = rx.next().await {
                        let Message::Text(text) = msg else { continue };
                        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                        if v.get("id").is_none() {
                            continue;
                        }
                        let method = v["method"].as_str().unwrap_or("").to_owned();
                        let p = v["params"].clone();
                        seen.lock().unwrap().push((method.clone(), p.clone()));
                        let session = p["session_id"].as_str().unwrap_or("a8:main").to_owned();
                        let result = match method.as_str() {
                            "session/open" => json!({"opened": {
                                "session_id": session, "active_profile_id": PROFILE, "workspace_root": "/home/user/octos",
                                "cursor": {"stream": session, "seq": 1},
                                "capabilities": {
                                    "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                                    "capabilities_schema_version": 2,
                                    "supported_methods": ["session/open", "session/hydrate", "session/list", "approval/respond", "user_question/respond"],
                                    "supported_notifications": [],
                                    "supported_features": ["state.session_hydrate.v1"]
                                }
                            }}),
                            // The streaming server has nothing parked.
                            "session/hydrate" if p["include"] == json!(["pending_approvals"]) && stream_turns => json!({
                                "session_id": session, "cursor": {"stream": session, "seq": 12},
                                "pending_approvals": [], "pending_questions": []
                            }),
                            "session/hydrate" if p["include"] == json!(["pending_approvals"]) => json!({
                                "session_id": session, "cursor": {"stream": session, "seq": 12},
                                // The recorded r23 approval/question shapes; one foreign row.
                                "pending_approvals": [
                                    {"approval_id": "01a0eb92-9444-7101-aa6f-10065886f57d", "body": "Run command: sudo -n true", "risk": "unspecified",
                                     "session_id": session, "title": "Approve command", "tool_name": "bash", "turn_id": "01920000-0000-7000-8000-000000000241"},
                                    {"approval_id": "01a0eb92-9444-7101-aa6f-10065886f57e", "body": "Run command: rm", "risk": "unspecified",
                                     "session_id": "someone:else", "title": "Approve command", "tool_name": "bash", "turn_id": "01920000-0000-7000-8000-000000000242"}
                                ],
                                "pending_questions": [
                                    {"body": "Which color?", "question_id": "01a0eb8f-7b23-7030-9f26-a284864217a0",
                                     "questions": [{"allow_free_text": true, "header": "Color choice", "multi_select": false,
                                                    "options": [{"description": "Calm.", "label": "Blue"}], "question": "Which color?"}],
                                     "session_id": session, "title": "Which color would you like to pick?", "turn_id": "01920000-0000-7000-8000-000000000240"}
                                ]
                            }),
                            "session/hydrate" => json!({
                                "session_id": session, "cursor": {"stream": session, "seq": 12},
                                "messages": [
                                    {"seq": 10, "role": "user", "content": "why 5?", "turn_id": "0b3f6d5e-1111-4111-8111-111111111111", "persisted_at": "2026-09-30T00:00:00Z"},
                                    {"seq": 11, "role": "assistant", "content": "because five", "turn_id": "0b3f6d5e-1111-4111-8111-111111111111", "persisted_at": "2026-09-30T00:00:01Z"}
                                ]
                            }),
                            "session/list" => json!({"sessions": []}),
                            _ => json!({}),
                        };
                        if method == "turn/start" && stream_turns {
                            let turn = p["turn_id"].as_str().unwrap_or_default().to_owned();
                            let ok = json!({"jsonrpc": "2.0", "id": v["id"].clone(), "result": {"accepted": true}}).to_string();
                            let tx = tx.clone();
                            tokio::spawn(async move {
                                let _ = tx.lock().await.send(Message::Text(ok.into())).await;
                                for (m, params) in turn_script(&session, &turn) {
                                    tokio::time::sleep(Duration::from_millis(120)).await;
                                    let frame = json!({"jsonrpc": "2.0", "method": m, "params": params}).to_string();
                                    let _ = tx.lock().await.send(Message::Text(frame.into())).await;
                                }
                            });
                            continue;
                        }
                        let frame = if method == "turn/start" {
                            // A send the server refuses (after a while).
                            json!({"jsonrpc": "2.0", "id": v["id"].clone(), "error": {"code": -32603, "message": "No ProfileRuntime registered"}}).to_string()
                        } else {
                            json!({"jsonrpc": "2.0", "id": v["id"].clone(), "result": result}).to_string()
                        };
                        let tx = tx.clone();
                        let delay = match method.as_str() {
                            "session/hydrate" => hydrate_delay,
                            "turn/start" => Duration::from_millis(300),
                            _ => Duration::ZERO,
                        };
                        tokio::spawn(async move {
                            tokio::time::sleep(delay).await;
                            let _ = tx.lock().await.send(Message::Text(frame.into())).await;
                        });
                    }
                });
            }
        });
        Self { base_url, seen, kick }
    }

    /// Drop every open socket (the client sees its stream end).
    fn drop_sockets(&self) {
        let _ = self.kick.send(());
    }

    fn count(&self, method: &str) -> usize {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).count()
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    /// The transport's canonical message hydrates (not the parked-interaction reads).
    fn message_hydrates(&self) -> Vec<Value> {
        self.params_of("session/hydrate").into_iter().filter(|p| p["include"] == json!(["messages"])).collect()
    }
}

async fn connected(server: &FakeServer) -> (Conversation, tokio::sync::mpsc::Receiver<TransportEvent>) {
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

/// Fold the transport's events until `done` holds (bounded); returns what
/// the flow reported for each.
async fn fold_until(
    conv: &Conversation,
    events: &mut tokio::sync::mpsc::Receiver<TransportEvent>,
    done: impl Fn(&Conversation) -> bool,
) -> Vec<FlowEvent> {
    let mut out = Vec::new();
    for _ in 0..60 {
        if done(conv) {
            break;
        }
        if let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(100), events.recv()).await {
            out.push(conv.on_event(evt));
        }
    }
    out
}

#[tokio::test]
async fn a_reconnect_reopens_the_active_session_and_hydrates_under_a_new_generation() {
    let server = FakeServer::start(Duration::ZERO).await;
    let (conv, mut events) = connected(&server).await;
    let session = conv.session_id();
    let g0 = conv.generation();
    assert_eq!(server.count("session/open"), 1);
    // The server drops the socket. The REAL transport re-dials and parks the
    // new socket in Handshaking until a `session/open` answers: the flow's
    // re-open is what brings it back to Live.
    server.drop_sockets();
    let seen = fold_until(&conv, &mut events, |c| {
        c.store.is_live() && server.count("session/open") >= 2 && c.store.domains.session.timeline.len(&session) >= 2
    })
    .await;
    assert!(
        seen.iter().any(|e| matches!(e, FlowEvent::Connecting(s) if s.starts_with("Reconnecting"))),
        "the outage is seen: {seen:?}"
    );
    assert!(seen.iter().any(|e| *e == FlowEvent::Live), "Live again: {seen:?}");
    assert!(conv.store.is_live());
    assert_eq!(conv.generation(), g0 + 1, "a reconnect is a new authority generation");
    let opens = server.params_of("session/open");
    assert_eq!(opens.len(), 2, "the active Session is re-opened after the reconnect (once)");
    assert_eq!(opens[1]["session_id"], json!(session));
    assert_eq!(opens[1]["cwd"], json!("/home/user/octos"), "at its workspace");
    assert_eq!(server.message_hydrates()[0]["session_id"], json!(session), "then hydrated canonically");
    assert!(seen.iter().any(|e| *e == FlowEvent::Other("session/hydrate".into())), "{seen:?}");
    assert!(conv.store.domains.session.timeline.len(&session) >= 2, "the canonical history folded");
    // The first Live of a connection is NOT a reconnect.
    let (conv2, _ev2) = connected(&FakeServer::start(Duration::ZERO).await).await;
    assert_eq!(conv2.generation(), 1, "one open, no reconnect re-open");
}

#[tokio::test]
async fn a_hydrate_from_a_retired_generation_fails_closed() {
    let server = FakeServer::start(Duration::from_millis(400)).await;
    let (conv, mut events) = connected(&server).await;
    let session = conv.session_id();
    // A hydrate asked under generation g...
    assert!(conv.request_hydrate(&session));
    // ...then the authority moves (another open of the session) before it lands.
    conv.open_session(&session, None).await.unwrap();
    let seen = fold_until(&conv, &mut events, |_| false).await;
    assert!(
        seen.iter().any(|e| *e == FlowEvent::Other("session/hydrate-stale-authority".into())),
        "the stale reply is refused: {seen:?}"
    );
    assert_eq!(conv.store.domains.session.timeline.len(&session), 0, "nothing committed from the retired generation");
    assert_eq!(server.message_hydrates().len(), 1, "a healthy session is not re-asked");
}

#[tokio::test]
async fn a_stale_reply_for_an_owed_resync_is_asked_again_under_the_new_generation() {
    let server = FakeServer::start(Duration::from_millis(400)).await;
    let (conv, mut events) = connected(&server).await;
    let session = conv.session_id();
    // The session owes a resync (a lossy replay)...
    conv.store.domains.config.observe_replay_lossy(octoscode_store::domains::config::ReplayLoss {
        session_id: session.clone(),
        dropped_count: 3,
        last_durable_cursor: None,
    });
    assert!(conv.maybe_resync(), "the resync goes out under generation g");
    conv.open_session(&session, None).await.unwrap(); // g -> g + 1 before the reply
    let seen = fold_until(&conv, &mut events, |c| {
        c.store.domains.config.recovery(&c.session_id()).phase == octoscode_store::domains::config::LossyPhase::Healthy
    })
    .await;
    assert!(seen.iter().any(|e| *e == FlowEvent::Other("session/hydrate-stale-authority".into())));
    assert_eq!(server.message_hydrates().len(), 2, "asked again under the current generation");
    assert_eq!(
        conv.store.domains.config.recovery(&session).phase,
        octoscode_store::domains::config::LossyPhase::Healthy,
        "the fresh reply commits and clears the lossy phase"
    );
}

#[tokio::test]
async fn the_runtime_scope_key_is_the_webs_tuple_with_a_per_connection_epoch() {
    let server = FakeServer::start(Duration::ZERO).await;
    let (a, _ea) = connected(&server).await;
    let (b, _eb) = connected(&server).await;
    let (sa, sb) = (a.scope(), b.scope());
    assert_eq!((sa.profile_id.as_str(), sa.session_id.as_str()), (PROFILE, "a8:main"));
    assert_eq!(sa.workspace_root, "/home/user/octos", "the open reply's workspace root");
    assert_ne!(sa.authority_epoch, sb.authority_epoch, "each connection is a new authority");
    assert_ne!(a.scope_key(), b.scope_key(), "same ids, different authority: different scope");
    let v: Value = serde_json::from_str(&sa.key()).unwrap();
    assert_eq!(v.as_array().map(Vec::len), Some(5), "[endpoint, workspace, profile, session, epoch]");
    assert_eq!(v[4], json!(sa.authority_epoch));
    // A session switch is a new scope.
    let before = a.scope_key();
    a.open_session("a8:other", None).await.unwrap();
    assert_ne!(a.scope_key(), before);
}

/// The drafts' process-global state is shared by the tests that use it.
fn drafts_lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    octoscode_module::screens::drafts::reset();
    g
}

#[tokio::test]
async fn an_unsent_draft_survives_a_restart_per_principal_and_stays_with_its_session() {
    use octoscode_module::screens::drafts;
    let _g = drafts_lock();
    let store: Arc<dyn octoscode_module::screens::recents::Storage> = Arc::new(octoscode_module::screens::recents::MemoryStore::new());
    drafts::set_storage(store.clone());
    let server = FakeServer::start(Duration::ZERO).await;
    let (conv, mut events) = connected(&server).await;
    // The principal is read over REST (`/api/auth/me`), then the scope binds.
    assert_eq!(drafts::resolve_principal(&conv).await.as_deref(), Some("a8-user"));
    drafts::bind_connection(&conv).await;
    let a = drafts::active_key(&conv).expect("the open reply named the workspace");
    assert_eq!(a, r#"["/home/user/octos","a8","a8:main"]"#, "the web's workspaceSessionKey");
    assert_eq!(drafts::follow(Some(&a), ""), None);
    conv.set_draft("half a thought");
    drafts::follow(Some(&a), "half a thought");
    // Switch Sessions: A's text is filed, the new Session starts empty.
    conv.open_session("a8:api:other", None).await.unwrap();
    fold_until(&conv, &mut events, |c| drafts::active_key(c).is_some_and(|k| k != a)).await;
    let b = drafts::active_key(&conv).unwrap();
    assert_eq!(drafts::follow(Some(&b), "half a thought").as_deref(), Some(""), "the new Session has its own (empty) draft");
    assert_eq!(drafts::get(&a).as_deref(), Some("half a thought"), "A keeps its draft");
    // "Restart": a fresh process state and a fresh connection.
    drafts::reset();
    let (conv2, _e2) = connected(&server).await;
    let a2 = drafts::active_key(&conv2).unwrap();
    assert_eq!(a2, a);
    drafts::follow(Some(&a2), "");
    assert_eq!(drafts::bind_connection(&conv2).await.as_deref(), Some("half a thought"), "the unsent text is restored, not sent");
    assert_eq!(server.count("turn/start"), 0, "a restored draft is never dispatched");
}

/// `SessionStatusStrip.tsx:44-71` + `turn-activity.ts`: while OUR turn runs,
/// the strip's state word is its live step — Thinking… (reasoning), Running
/// shell… (a tool started), Thinking… (the tool ended), Writing… (answer
/// text) — and Ready once it completes. A prompt sent through the composer's
/// production path; the recorded envelope shapes streamed over the socket.
#[tokio::test]
async fn the_strip_word_follows_the_running_turn_step_by_step() {
    use octoscode_module::screens::board3::strip;
    let server = FakeServer::start_with(Duration::ZERO, true).await;
    let (conv, mut events) = connected(&server).await;
    let conv = Arc::new(conv);
    let word = |c: &Conversation| {
        let active = c.ui_ref().lock().unwrap().active_turn();
        strip::state_word(&c.store, active.as_deref(), None)
    };
    assert_eq!(word(&conv), "Ready");
    conv.set_draft("check the queue");
    let c2 = conv.clone();
    tokio::spawn(async move { c2.submit_draft().await });
    let mut seen: Vec<String> = vec![word(&conv)];
    for _ in 0..80 {
        match tokio::time::timeout(Duration::from_millis(200), events.recv()).await {
            Ok(Some(evt)) => {
                conv.on_event(evt);
            }
            _ => {}
        }
        let w = word(&conv);
        if seen.last() != Some(&w) {
            seen.push(w.clone());
        }
        if w == "Ready" && seen.len() > 1 {
            break;
        }
    }
    let steps: Vec<&str> = seen.iter().map(String::as_str).filter(|w| *w != "Responding").collect();
    assert_eq!(
        steps,
        ["Ready", "Thinking…", "Running shell…", "Thinking…", "Writing…", "Ready"],
        "the strip's word over the turn: {seen:?}"
    );
    assert_eq!(server.count("turn/start"), 1, "one send");
}

/// A slash command the app consumes (here `/sessions`, a board-3 surface) is
/// not an unsent draft: neither draft store may bring "/sessions" back when
/// the Session comes round again (A7's draft recovery saves every edit).
#[tokio::test]
async fn a_consumed_command_leaves_no_unsent_draft_behind() {
    use octoscode_module::screens::drafts;
    let _g = drafts_lock();
    drafts::set_storage(Arc::new(octoscode_module::screens::recents::MemoryStore::new()));
    let dir = std::env::temp_dir().join(format!("a8-store-cmd-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    std::env::set_var("OCTOSCODE_DRAFTS_FILE", dir.join("composer-drafts.json"));
    let server = FakeServer::start(Duration::ZERO).await;
    let (conv, _ev) = connected(&server).await;
    let session = conv.session_id();
    let key = drafts::key_of(&conv, &session);
    // The person types the command (each edit is saved by both stores)…
    conv.set_draft("/sessions");
    octoscode_module::drafts::save(&session, "/sessions");
    drafts::follow(Some(&key), "/sessions");
    // …and runs it.
    conv.submit_draft().await.expect("a local command");
    assert_eq!(conv.ui_ref().lock().unwrap().draft(), "", "the composer cleared");
    assert_eq!(octoscode_module::drafts::load(&session), None, "the saved text went with it");
    drafts::follow(Some(&key), "");
    assert_eq!(drafts::get(&key), None, "no per-Session draft either");
    assert_eq!(server.count("turn/start"), 0, "a command never reaches the model");
    std::env::remove_var("OCTOSCODE_DRAFTS_FILE");
}

#[tokio::test]
async fn a_failed_send_returns_the_prompt_to_its_own_session() {
    use octoscode_module::screens::drafts;
    let _g = drafts_lock();
    drafts::set_storage(Arc::new(octoscode_module::screens::recents::MemoryStore::new()));
    let server = FakeServer::start(Duration::ZERO).await;
    let (conv, _ev) = connected(&server).await;
    let a_key = drafts::key_of(&conv, "a8:main");
    // The send is in flight (the server answers after 300 ms)…
    let conv = Arc::new(conv);
    let c2 = conv.clone();
    let send = tokio::spawn(async move { c2.start_turn_with_id("prompt for A", "01920000-0000-7000-8000-0000000000a1".into()).await });
    tokio::time::sleep(Duration::from_millis(50)).await;
    // …when the person switches to another Session.
    conv.open_session("a8:api:other", None).await.unwrap();
    assert!(send.await.unwrap().is_err(), "the server refused the turn");
    assert_eq!(drafts::get(&a_key).as_deref(), Some("prompt for A"), "the prompt went back to A");
    assert_eq!(conv.ui_ref().lock().unwrap().draft(), "", "never into the other Session's composer");
}

#[tokio::test]
async fn an_open_restores_its_parked_approvals_and_questions_from_the_canonical_hydrate() {
    let server = FakeServer::start(Duration::ZERO).await;
    let (conv, _ev) = connected(&server).await;
    let session = conv.session_id();
    // The open asked for the pending sections of THIS Session…
    for _ in 0..40 {
        if !conv.store.domains.approval.pending().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let reads: Vec<Value> = server.params_of("session/hydrate").into_iter().filter(|p| p["include"] == json!(["pending_approvals"])).collect();
    assert_eq!(reads[0], json!({"session_id": session, "include": ["pending_approvals"]}));
    // …and the parked interactions are back (never another Session's).
    let pending = conv.store.domains.approval.pending();
    assert_eq!(pending.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["01a0eb92-9444-7101-aa6f-10065886f57d"]);
    assert_eq!(pending[0].target.as_deref(), Some("bash"));
    // The restored approval carries the takeover card's payload (A6), scoped
    // to its Session — it is asked again, not just counted.
    let (_, detail) = conv.store.domains.approval.showing(&session).expect("the card for this Session");
    assert_eq!((detail.title.as_str(), detail.body.as_str()), ("Approve command", "Run command: sudo -n true"));
    let q = conv.store.domains.approval.question().expect("the parked question");
    assert_eq!((q.question_id.as_str(), q.session_id.as_str()), ("01a0eb8f-7b23-7030-9f26-a284864217a0", session.as_str()));
}

#[tokio::test]
async fn a_parked_restore_from_a_retired_generation_is_dropped() {
    let server = FakeServer::start(Duration::from_millis(400)).await;
    let (conv, _ev) = connected(&server).await;
    // The first open's restore is in flight; another open retires its generation.
    conv.open_session(&conv.session_id(), None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    // Only the newest open's restore may land (it is held back too).
    let early = conv.store.domains.approval.pending().len();
    assert_eq!(early, 0, "nothing restored from the retired generation");
}
