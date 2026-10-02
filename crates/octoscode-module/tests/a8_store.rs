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
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octos_app_transport::{ConnectionState, TransportEvent};
use octoscode_module::flow::{Conversation, FlowEvent};

const PROFILE: &str = "a8";

struct FakeServer {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
}

impl FakeServer {
    /// `hydrate_delay`: how long each `session/hydrate` reply is held back.
    async fn start(hydrate_delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let s2 = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let seen = s2.clone();
                tokio::spawn(async move {
                    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
                    let (tx, mut rx) = ws.split();
                    let tx = Arc::new(tokio::sync::Mutex::new(tx));
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
                                    "supported_methods": ["session/open", "session/hydrate", "session/list"],
                                    "supported_notifications": [],
                                    "supported_features": ["state.session_hydrate.v1"]
                                }
                            }}),
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
                        let frame = json!({"jsonrpc": "2.0", "id": v["id"].clone(), "result": result}).to_string();
                        let tx = tx.clone();
                        let delay = if method == "session/hydrate" { hydrate_delay } else { Duration::ZERO };
                        tokio::spawn(async move {
                            tokio::time::sleep(delay).await;
                            let _ = tx.lock().await.send(Message::Text(frame.into())).await;
                        });
                    }
                });
            }
        });
        Self { base_url, seen }
    }

    fn count(&self, method: &str) -> usize {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).count()
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
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
    // The socket drops and comes back (the transport's own state events).
    conv.on_event(TransportEvent::ConnectionState(ConnectionState::Reconnecting { attempt: 1 }));
    assert!(!conv.store.is_live());
    conv.on_event(TransportEvent::ConnectionState(ConnectionState::Live));
    assert_eq!(conv.generation(), g0 + 1, "a reconnect is a new authority generation");
    let seen = fold_until(&conv, &mut events, |c| c.store.domains.session.timeline.len(&session) >= 2).await;
    let opens = server.params_of("session/open");
    assert_eq!(opens.len(), 2, "the active Session is re-opened after the reconnect");
    assert_eq!(opens[1]["session_id"], json!(session));
    assert_eq!(server.params_of("session/hydrate")[0]["session_id"], json!(session), "then hydrated canonically");
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
    assert_eq!(server.count("session/hydrate"), 1, "a healthy session is not re-asked");
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
    assert_eq!(server.count("session/hydrate"), 2, "asked again under the current generation");
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
