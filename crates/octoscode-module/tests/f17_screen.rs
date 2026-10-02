//! Card #17 §Tests — the conversation screen replayed on the RECORDED live-gate
//! traffic, and the list's display order asserted on it.
//!
//! The screen's two `PortalList`s instantiate `screen::timeline_rows` in order.
//! This test drives the module's own `Conversation` over a fake WS server that
//! speaks `crates/octoscode-client/tests/fixtures/live-gate-a6ea8505.jsonl` —
//! the 266 real frames a `octos serve` a6ea8505 turn actually sent — then
//! asserts the row list is exactly the screen's order: **user first, reasoning
//! folded away, the answer, the tools, then the settled `worked-for` tail**.
//!
//! No model is called: the fixture is replayed.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::{Conversation, FlowEvent};
use octoscode_module::screen;
use octoscode_store::EntryKind;

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

/// The fixture's prompt (`user_message` text).
fn fixture_prompt(frames: &[Frame]) -> String {
    frames
        .iter()
        .filter(|f| f.method == "projection/envelope")
        .find_map(|f| {
            let p = &f.body["payload"];
            (p["type"] == "user_message")
                .then(|| p["data"]["text"].as_str().unwrap_or("").to_owned())
        })
        .expect("the fixture carries a user_message")
}

/// The recorded turn id the first `turn/start` was driven with (card #26 §1:
/// drive with the server's own id, the way the web mints it — `client.ts:488` —
/// so the optimistic user row dedups into the replayed `user_message` copy).
fn fixture_turn_id(frames: &[Frame]) -> String {
    frames
        .iter()
        .filter(|f| f.dir == "out" && f.method == "turn/start")
        .find_map(|f| f.body["turn_id"].as_str().map(str::to_owned))
        .expect("the fixture carries an out turn/start id")
}

struct ReplayServer {
    base_url: String,
}

impl ReplayServer {
    async fn start(frames: Vec<Frame>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
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
                match method.as_str() {
                    "session/open" => {
                        let session = v["params"]["session_id"]
                            .as_str()
                            .unwrap_or("dsflash:main")
                            .to_owned();
                        let frame = serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"opened": {
                                "session_id": session,
                                "active_profile_id": "dsflash",
                                "cursor": {"stream": "dsflash:main", "seq": 1},
                                "capabilities": {
                                    "version": {"protocol": "octos-ui/v1alpha1",
                                                "schema_version": 1, "jsonrpc": "2.0"},
                                    "capabilities_schema_version": 1,
                                    "supported_methods": ["session/open", "turn/start",
                                                          "turn/interrupt", "session/list"],
                                    "supported_notifications": ["projection/envelope",
                                                                "message/delta", "turn/started"],
                                    "supported_features": ["projection.envelope.v2"]
                                }
                            }}
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
                                tokio::time::sleep(Duration::from_millis(10)).await;
                            }
                        });
                    }
                    "turn/start" => {
                        let _ = tx.lock().await.send(Message::Text(
                            serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {"accepted": true}})
                                .to_string().into())).await;
                    }
                    _ => {
                        let _ = tx.lock().await.send(Message::Text(
                            serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {}})
                                .to_string().into())).await;
                    }
                }
            }
        });

        Self { base_url: format!("http://{addr}") }
    }
}

#[tokio::test]
async fn the_replayed_turn_lists_in_screen_order() {
    let frames = fixture();
    let prompt = fixture_prompt(&frames);
    let turn_id = fixture_turn_id(&frames);
    let server = ReplayServer::start(frames).await;
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
        .expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    // Card #26 §1: drive with the RECORDED turn id so the optimistic user row
    // dedups into the replayed `user_message` copy (exactly one user bubble).
    conv.start_turn_with_id(&prompt, turn_id.clone())
        .await
        .expect("turn/start");

    // Drain until the turn settles (the fixture's `turn_terminal`).
    let mut ended = false;
    for _ in 0..300 {
        match tokio::time::timeout(Duration::from_millis(120), events.recv()).await {
            Ok(Some(evt)) => {
                if matches!(conv.on_event(evt), FlowEvent::TurnEnded { .. }) {
                    ended = true;
                    break;
                }
            }
            _ => break,
        }
    }
    assert!(ended, "the replayed turn must settle");

    let session = conv.store.active_session().expect("an active session");
    let turn_id = conv
        .store
        .domains
        .session
        .timeline
        .entries(&session)
        .into_iter()
        .find_map(|e| e.turn_id)
        .expect("the turn carries an id");

    // This fixture's turn calls no tools; add one so the tool POSITION is
    // asserted too (A1: the work, then its result — the worked-for header
    // heads the turn's tool group, the answer follows the calls).
    conv.store.domains.session.timeline.append_data(
        &session,
        Some(turn_id.clone()),
        EntryKind::TOOL_CALL,
        "read_file".to_owned(),
        serde_json::json!({"path": "main.rs"}),
    );

    let rows = screen::timeline_rows(&conv.store, false);
    let kinds: Vec<&str> = rows.iter().map(|r| r.kind.id()).collect();

    // The whole order, on REAL traffic.
    assert_eq!(
        kinds,
        vec!["user-bubble", "worked-for", "tool-cell", "assistant-prose", "answer-actions"],
        "user first, reasoning folded, header, tools, answer, actions — got {kinds:?}"
    );

    // Content: the user row projects the REAL prompt; the answer row the REAL
    // assistant text (both from the fixture's own envelope frames).
    let entries = conv.store.domains.session.timeline.entries(&session);
    assert_eq!(entries[rows[0].index].text, prompt, "the user bubble is the real prompt");
    let prose = rows
        .iter()
        .find(|r| r.kind.id() == "assistant-prose")
        .expect("an answer row");
    assert!(
        !entries[prose.index].text.is_empty(),
        "the assistant prose row carries the streamed answer"
    );
    // Reasoning is FOLDED: it is in the store (the fixture streams it) but no
    // row instantiates it.
    assert!(
        entries.iter().any(|e| e.kind == EntryKind::REASONING),
        "the fixture streams reasoning (so its absence from `rows` is folding, not a miss)"
    );
    assert!(!kinds.contains(&"reasoning"), "no reasoning row is instantiated");
}
