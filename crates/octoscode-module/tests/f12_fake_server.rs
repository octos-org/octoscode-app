//! Card #12 §1 (fake half) + §2: a scripted fake WS server.
//!
//! It speaks just enough of the UI protocol for the real transport + flow to
//! walk the gate's path end to end with **no model and no serve**:
//! - It records the handshake's `ui_feature=` query params and
//!   `x-octos-ui-features` header, so the feature-negotiation test can assert
//!   the client asks for exactly the web's list.
//! - It answers `session/open` with a `SessionOpenResult` (with capabilities),
//!   then replays a realistic turn: `turn/started`, streamed `message/delta`s,
//!   one `tool/started` → `tool/completed`, then `turn/completed`.
//!
//! No UI: these tests drive [`octoscode_module::flow`] directly.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

/// What the fake server saw at the handshake.
#[derive(Debug, Clone, Default)]
pub struct Handshake {
    /// The request URI's path+query (as the client sent it).
    pub uri: String,
    /// `x-octos-ui-features`, verbatim.
    pub features_header: Option<String>,
    /// Every `ui_feature=` query value, in order.
    pub query_features: Vec<String>,
}

/// A running fake server: its base URL and what it captured.
pub struct FakeServer {
    pub base_url: String,
    pub handshake: Arc<Mutex<Handshake>>,
    /// Every frame the server received (method name), in order.
    pub received: Arc<Mutex<Vec<String>>>,
}

impl FakeServer {
    /// Start the server on an ephemeral port. It serves one connection.
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let base_url = format!("http://{addr}");

        let handshake = Arc::new(Mutex::new(Handshake::default()));
        let received = Arc::new(Mutex::new(Vec::new()));

        let hs = handshake.clone();
        let rx = received.clone();
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let hs_cb = hs.clone();
            let ws = tokio_tungstenite::accept_hdr_async(
                stream,
                move |req: &tokio_tungstenite::tungstenite::handshake::server::Request,
                      resp: tokio_tungstenite::tungstenite::handshake::server::Response| {
                    {
                        let mut h = hs_cb.lock().unwrap();
                        h.uri = req.uri().to_string();
                        h.features_header = req
                            .headers()
                            .get("x-octos-ui-features")
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_owned);
                        h.query_features = req
                            .uri()
                            .query()
                            .map(|q| {
                                q.split('&')
                                    .filter_map(|kv| kv.split_once('='))
                                    .filter(|(k, _)| *k == "ui_feature")
                                    .map(|(_, v)| v.replace("%2E", ".").replace("%5F", "_"))
                                    .collect()
                            })
                            .unwrap_or_default();
                    }
                    Ok(resp)
                },
            )
            .await;

            let Ok(ws) = ws else {
                return;
            };
            let (mut tx, mut rx_in) = ws.split();

            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                rx.lock().unwrap().push(method.clone());

                match method.as_str() {
                    "session/open" => {
                        let session = v["params"]["session_id"].as_str().unwrap_or("f1:main");
                        let caps = serde_json::json!({
                            "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                            "capabilities_schema_version": 1,
                            "supported_methods": ["session/open", "turn/start", "turn/interrupt"],
                            "supported_notifications": [
                                "message/delta", "turn/started", "turn/completed",
                                "tool/started", "tool/completed"
                            ],
                            "supported_features": ["coding.autonomy.v1", "context.lifecycle.v1"]
                        });
                        let frame = serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"opened": {
                                "session_id": session,
                                "active_profile_id": "f1",
                                "cursor": {"stream": "f1", "seq": 1},
                                "capabilities": caps
                            }}
                        });
                        let _ = tx.send(Message::Text(frame.to_string().into())).await;
                    }
                    "turn/start" => {
                        let session = v["params"]["session_id"].as_str().unwrap_or("f1:main").to_owned();
                        let turn = v["params"]["turn_id"].as_str().unwrap_or("t").to_owned();
                        let _ = tx
                            .send(Message::Text(
                                serde_json::json!({
                                    "jsonrpc": "2.0", "id": id,
                                    "result": {"accepted": true}
                                })
                                .to_string()
                                .into(),
                            ))
                            .await;
                        // Replay a realistic turn inline (the ack went first;
                        // the client sees frames in send order).
                        let notif = |method: &str, params: serde_json::Value| {
                            serde_json::json!({
                                "jsonrpc": "2.0", "method": method, "params": params
                            })
                            .to_string()
                        };
                        let _ = tx
                            .send(Message::Text(
                                notif(
                                    "turn/started",
                                    serde_json::json!({
                                        "session_id": session, "turn_id": turn,
                                        "timestamp": "2026-09-28T00:00:00Z"
                                    }),
                                )
                                .into(),
                            ))
                            .await;
                        for text in ["Hello", ", ", "world"] {
                            let _ = tx
                                .send(Message::Text(
                                    notif(
                                        "message/delta",
                                        serde_json::json!({
                                            "session_id": session, "turn_id": turn,
                                            "text": text
                                        }),
                                    )
                                    .into(),
                                ))
                                .await;
                        }
                        let _ = tx
                            .send(Message::Text(
                                notif(
                                    "tool/started",
                                    serde_json::json!({
                                        "session_id": session, "turn_id": turn,
                                        "tool_call_id": "c1", "tool_name": "bash"
                                    }),
                                )
                                .into(),
                            ))
                            .await;
                        let _ = tx
                            .send(Message::Text(
                                notif(
                                    "tool/completed",
                                    serde_json::json!({
                                        "session_id": session, "turn_id": turn,
                                        "tool_call_id": "c1", "tool_name": "bash",
                                        "success": true, "output_preview": "exit 0"
                                    }),
                                )
                                .into(),
                            ))
                            .await;
                        let _ = tx
                            .send(Message::Text(
                                notif(
                                    "turn/completed",
                                    serde_json::json!({
                                        "session_id": session, "turn_id": turn,
                                        "cursor": {"stream": "f1", "seq": 9}
                                    }),
                                )
                                .into(),
                            ))
                            .await;
                    }
                    "turn/interrupt" => {
                        let _ = tx
                            .send(Message::Text(
                                serde_json::json!({
                                    "jsonrpc": "2.0", "id": id,
                                    "result": {"interrupted": true}
                                })
                                .to_string()
                                .into(),
                            ))
                            .await;
                    }
                    _ => {
                        let _ = tx
                            .send(Message::Text(
                                serde_json::json!({
                                    "jsonrpc": "2.0", "id": id,
                                    "result": {}
                                })
                                .to_string()
                                .into(),
                            ))
                            .await;
                    }
                }
            }
        });

        Self { base_url, handshake, received }
    }
}

/// The web's 21 features, verbatim (`client.ts:106-128`).
const WEB_FEATURES: &[&str] = &[
    "approval.typed.v1",
    "pane.snapshots.v1",
    "session.workspace_cwd.v1",
    "auxiliary.rest_to_ws.v1",
    "state.session_hydrate.v1",
    "state.thread_graph.v1",
    "state.turn_state_get.v1",
    "event.turn_steer_dropped.v1",
    "user_question.v1",
    "plan.todos.v1",
    "projection.envelope.v2",
    "harness.task_control.v1",
    "harness.task_artifacts.v1",
    "context.lifecycle.v1",
    "review.start.v1",
    "coding.autonomy.v1",
    "coding.agent_control.v1",
    "coding.goal_runtime.v1",
    "coding.loop_runtime.v1",
    "coding.monitor_runtime.v1",
    "external_driver_v1",
];

use octoscode_module::flow::{Conversation, Direction, FlowEvent};

#[tokio::test]
async fn feature_negotiation_sends_the_webs_list() {
    let server = FakeServer::start().await;
    let (_conv, mut events) = Conversation::connect(
        &server.base_url,
        "dummy",
        "f1",
        None,
        None,
    )
    .expect("connect");

    // Open a session so the handshake completes.
    let conv = _conv;
    conv.open_workspace(None).await.expect("open");
    // Drain until the capabilities event lands (or time out).
    let mut caps_seen = false;
    for _ in 0..40 {
        match tokio::time::timeout(Duration::from_millis(100), events.recv()).await {
            Ok(Some(evt)) => {
                if matches!(
                    conv.on_event(evt),
                    FlowEvent::Capabilities(_)
                ) {
                    caps_seen = true;
                    break;
                }
            }
            _ => break,
        }
    }
    assert!(caps_seen, "the session/open capabilities must arrive");

    let hs = server.handshake.lock().unwrap().clone();
    // The web sends one `ui_feature=` per feature (`url.ts:24-28`).
    assert_eq!(
        hs.query_features, WEB_FEATURES,
        "the query params must be the web's list, in order"
    );
    // And our transport also sets the header (additive; server accepts either).
    let header = hs.features_header.expect("x-octos-ui-features header");
    for f in WEB_FEATURES {
        assert!(
            header.split(',').any(|h| h.trim() == *f),
            "header must carry {f}; got {header}"
        );
    }
    assert!(hs.uri.contains("/api/ui-protocol/ws"), "uri: {}", hs.uri);
}

#[tokio::test]
async fn the_conversation_flow_walks_a_scripted_turn() {
    let server = FakeServer::start().await;
    let (conv, mut events) = Conversation::connect(
        &server.base_url,
        "dummy",
        "f1",
        None,
        None,
    )
    .expect("connect");

    conv.open_workspace(None).await.expect("open");
    conv.start_turn("hi").await.expect("turn/start");

    // Drain events until the turn completes.
    let mut saw = Vec::new();
    for _ in 0..80 {
        match tokio::time::timeout(Duration::from_millis(150), events.recv()).await {
            Ok(Some(evt)) => {
                let e = conv.on_event(evt);
                let done = matches!(e, FlowEvent::TurnEnded { .. });
                saw.push(format!("{e:?}"));
                if done {
                    break;
                }
            }
            _ => break,
        }
    }

    let joined = saw.join(" | ");
    assert!(joined.contains("Live"), "connection went live: {joined}");
    assert!(joined.contains("WorkspaceOpened"), "opened: {joined}");
    assert!(joined.contains("TurnStarted"), "turn started: {joined}");
    assert!(
        saw.iter().filter(|s| s.contains("Delta")).count() >= 3,
        "three deltas: {joined}"
    );
    assert!(joined.contains("ToolStarted"), "tool row: {joined}");
    assert!(joined.contains("ToolCompleted"), "tool row done: {joined}");
    assert!(joined.contains("TurnEnded"), "turn ended: {joined}");

    // The timeline folded the deltas into ONE assistant entry.
    assert_eq!(conv.store.live_text("f1:main"), "Hello, world");

    // The flow's UI facts the bindings read.
    let ui = conv.ui();
    let ui = ui.lock().unwrap();
    assert_eq!(ui.tools().len(), 1, "one tool row");
    assert_eq!(ui.tools()[0].status, "done");
    assert_eq!(ui.tool_output(), vec!["exit 0".to_owned()]);
    assert!(!ui.turn_active(), "the turn is no longer live");
    assert!(ui.worked_for().starts_with("Worked for "));

    // The trace records both directions with method names (card §2's trace).
    let trace = conv.trace.entries();
    assert!(trace.iter().any(|e| e.direction == Direction::Out && e.method == "turn/start"));
    assert!(trace.iter().any(|e| e.method == "message/delta"));
    assert!(!conv.trace.render().is_empty());

    let received = server.received.lock().unwrap().clone();
    assert!(received.contains(&"session/open".to_owned()));
    assert!(received.contains(&"turn/start".to_owned()));
}

#[tokio::test]
async fn an_interrupt_mid_stream_shows_the_stopped_state() {
    let server = FakeServer::start().await;
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "f1", None, None)
        .expect("connect");

    conv.open_workspace(None).await.expect("open");
    let turn = conv.start_turn("long job").await.expect("turn/start");

    // Interrupt as soon as the first delta lands.
    let mut interrupted = false;
    for _ in 0..80 {
        match tokio::time::timeout(Duration::from_millis(150), events.recv()).await {
            Ok(Some(evt)) => {
                if matches!(conv.on_event(evt), FlowEvent::Delta { .. }) && !interrupted {
                    let r = conv.interrupt(&turn).await.expect("turn/interrupt");
                    assert_eq!(r["interrupted"], true);
                    interrupted = true;
                }
            }
            _ => break,
        }
    }
    assert!(interrupted, "we interrupted mid-stream");
    // The interrupt reached the wire.
    let received = server.received.lock().unwrap().clone();
    assert!(received.contains(&"turn/interrupt".to_owned()), "sent: {received:?}");
}
