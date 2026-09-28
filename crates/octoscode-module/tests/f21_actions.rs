//! Card #21 §3 — the per-item **actions on the replay server**.
//!
//! The router itself is unit-tested in `src/actions.rs`. This test proves the
//! actions reach the wire: over a replay server that speaks the recorded
//! `live-gate-a6ea8505.jsonl`, a routed action drives the module's real
//! `Conversation`, and the server must observe the protocol method the action
//! names (`thread.open` → `session/open`, `composer.submit` → `turn/start`,
//! `turn.steer` → `turn/steer`, `turn.interrupt` → `turn/interrupt`).
//!
//! No model is called: the fixture is replayed.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::actions::{self, Effect};
use octoscode_module::bindings::Ctx;
use octoscode_module::flow::Conversation;

#[derive(Debug, Clone)]
struct Frame {
    dir: String,
    method: String,
    body: serde_json::Value,
}

fn fixture() -> Vec<Frame> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../octoscode-client/tests/fixtures/live-gate-a6ea8505.jsonl"
    );
    let text = std::fs::read_to_string(path).expect("read the live-gate fixture");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v.get("body").cloned().unwrap_or(serde_json::Value::Null),
            }
        })
        .collect()
}

struct ReplayServer {
    base_url: String,
    received: Arc<Mutex<Vec<String>>>,
}

impl ReplayServer {
    async fn start(frames: Vec<Frame>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(Mutex::new(Vec::new()));
        let rx = received.clone();
        let stream_frames: Vec<Frame> = frames
            .iter()
            .filter(|f| {
                f.dir == "in"
                    && matches!(
                        f.method.as_str(),
                        "turn/started"
                            | "context/normalization_reported"
                            | "progress/updated"
                            | "projection/envelope"
                    )
            })
            .cloned()
            .collect();

        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (tx, mut rx_in) = ws.split();
            let tx = Arc::new(tokio::sync::Mutex::new(tx));
            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                rx.lock().unwrap().push(method.clone());
                let reply = match method.as_str() {
                    "session/open" => {
                        let session = v["params"]["session_id"]
                            .as_str()
                            .unwrap_or("dsflash:main")
                            .to_owned();
                        serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"opened": {
                                "session_id": session,
                                "active_profile_id": "dsflash",
                                "cursor": {"stream": session, "seq": 1},
                                "capabilities": {
                                    "version": {"protocol": "octos-ui/v1alpha1",
                                                "schema_version": 1, "jsonrpc": "2.0"},
                                    "capabilities_schema_version": 1,
                                    "supported_methods": ["session/open", "turn/start",
                                                          "turn/interrupt", "turn/steer",
                                                          "session/list"],
                                    "supported_notifications": ["projection/envelope",
                                                                "message/delta", "turn/started"],
                                    "supported_features": ["projection.envelope.v2",
                                                           "event.turn_steer_dropped.v1"]
                                }
                            }}
                        })
                    }
                    "session/list" => serde_json::json!({
                        "jsonrpc": "2.0", "id": id,
                        "result": {"sessions": [{
                            "id": "dsflash:main", "title": null, "message_count": 1,
                            "updated_at": null, "last_prompt": null, "active_turn": false,
                        }]}
                    }),
                    "turn/start" => {
                        let frame = serde_json::json!({
                            "jsonrpc": "2.0", "id": id, "result": {"accepted": true}
                        });
                        let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
                        let tx2 = tx.clone();
                        let sf = stream_frames.clone();
                        tokio::spawn(async move {
                            for f in sf {
                                let frame = serde_json::json!({
                                    "jsonrpc": "2.0", "method": f.method, "params": f.body,
                                });
                                let _ = tx2.lock().await.send(Message::Text(frame.to_string().into())).await;
                                tokio::time::sleep(Duration::from_millis(8)).await;
                            }
                        });
                        continue;
                    }
                    "turn/steer" => serde_json::json!({
                        "jsonrpc": "2.0", "id": id,
                        "result": {"turn_id": "t", "steered": true}
                    }),
                    "turn/interrupt" => serde_json::json!({
                        "jsonrpc": "2.0", "id": id,
                        "result": {"interrupted": true}
                    }),
                    _ => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {}}),
                };
                let _ = tx.lock().await.send(Message::Text(reply.to_string().into())).await;
            }
        });

        Self { base_url: format!("http://{addr}"), received }
    }

    fn saw(&self, method: &str) -> bool {
        self.received.lock().unwrap().iter().any(|m| m == method)
    }
}

/// Perform a routed action's effect against the conversation (the same mapping
/// `perform_action` in `lib.rs` uses).
async fn perform(conv: &Conversation, effect: &Effect) {
    match effect {
        Effect::Refresh => {
            let _ = conv.refresh_sessions().await;
        }
        Effect::NewChat => {
            let _ = conv.new_chat(None).await;
        }
        Effect::Submit => {
            let _ = conv.submit_draft().await;
        }
        Effect::Steer(text) => {
            let _ = conv.steer(text).await;
        }
        Effect::Interrupt(turn) => {
            let _ = conv.interrupt(turn).await;
        }
        Effect::Open(session) => {
            let _ = conv.open_session(session, None).await;
        }
        Effect::ToggleTool(_) | Effect::CopyAnswer | Effect::Unhandled(_) => {}
    }
}

#[tokio::test]
async fn routed_item_actions_reach_the_protocol_on_the_replay_server() {
    let server = ReplayServer::start(fixture()).await;
    let (conv, _events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    // Let the handshake settle so the session list is populated.
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(server.saw("session/open"), "the open reached the server");

    // `session.refresh` → the server sees a second `session/list`.
    {
        let ui = conv.ui();
        let ctx = Ctx::new(&conv.store, &ui);
        let effect = actions::resolve("session.refresh", 0, &ctx);
        assert_eq!(effect, Effect::Refresh);
        perform(&conv, &effect).await;
    }
    assert!(server.saw("session/list"), "session.refresh re-listed");

    // `thread.open` (row 0) → `session/open` naming that row's own id.
    {
        let ui = conv.ui();
        let ctx = Ctx::new(&conv.store, &ui);
        let effect = actions::resolve("thread.open", 0, &ctx);
        assert_eq!(effect, Effect::Open("dsflash:main".into()));
        perform(&conv, &effect).await;
    }
    assert!(server.saw("session/open"), "thread.open opened the row's session");

    // `composer.submit` → `turn/start` carrying the draft.
    {
        conv.ui().lock().unwrap().set_draft_inner("hello from the composer");
        let ui = conv.ui();
        let ctx = Ctx::new(&conv.store, &ui);
        let effect = actions::resolve("composer.submit", 0, &ctx);
        assert_eq!(effect, Effect::Submit);
        perform(&conv, &effect).await;
    }
    assert!(server.saw("turn/start"), "composer.submit started a turn");

    // `turn.steer` → the AppUI extension `turn/steer`.
    {
        conv.ui().lock().unwrap().begin_turn_now("t-steer");
        conv.ui().lock().unwrap().set_draft_inner("steer this");
        let ui = conv.ui();
        let ctx = Ctx::new(&conv.store, &ui);
        let effect = actions::resolve("turn.steer", 0, &ctx);
        assert_eq!(effect, Effect::Steer("steer this".into()));
        perform(&conv, &effect).await;
    }
    assert!(server.saw("turn/steer"), "turn.steer reached the server");

    // `turn.interrupt` → `turn/interrupt` for the live turn.
    {
        let ui = conv.ui();
        let ctx = Ctx::new(&conv.store, &ui);
        let effect = actions::resolve("turn.interrupt", 0, &ctx);
        assert_eq!(effect, Effect::Interrupt("t-steer".into()));
        perform(&conv, &effect).await;
    }
    assert!(server.saw("turn/interrupt"), "turn.interrupt reached the server");

    // `tool.toggle` is UI-local: it changes the flow's disclosure and sends nothing.
    {
        conv.ui().lock().unwrap().note_tool_started_for_test("c1", "read_file");
        let before = server.received.lock().unwrap().len();
        let ui = conv.ui();
        let ctx = Ctx::new(&conv.store, &ui);
        let effect = actions::resolve("tool.toggle", 0, &ctx);
        assert_eq!(effect, Effect::ToggleTool("c1".into()));
        if let Effect::ToggleTool(key) = effect {
            conv.ui().lock().unwrap().toggle_expanded(&key);
        }
        assert!(conv.ui().lock().unwrap().is_expanded("c1"), "the tool is disclosed");
        assert_eq!(
            server.received.lock().unwrap().len(),
            before,
            "tool.toggle is UI-local (no protocol method)"
        );
    }
}
