//! Card #15 §5 — a **replay server**: the recorded live-gate trace, served on a
//! port so the real hidden app can mount its cards against it.
//!
//! Unlike `fake_serve.rs` (hand-written scripted frames), this serves
//! `crates/octoscode-client/tests/fixtures/live-gate-a6ea8505.jsonl` — the 266
//! real frames the outer loop's gate captured from `octos serve` a6ea8505 +
//! `dsflash`. **No model is called** (card #15: "Don't run a real model turn").
//!
//! ```sh
//! # lane p0-build owns 8390-8399 (card #15); the replay server takes 8393.
//! cargo run -p octoscode-module --example replay_serve -- 8393
//! # then the app points at it:
//! OCTOS_BASE_URL=http://127.0.0.1:8393 OCTOS_PROFILE_ID=dsflash \
//!   MAKEPAD_HIDE_WINDOWS=1 <octosense> --module octoscode --remote 8394
//! ```
//!
//! Handshake: `session/open` is answered from the recording (its own
//! `session/open` result carries the negotiated capabilities + workspace root);
//! `session/list` gets a one-row reply naming the opened session (what a real
//! server returns for the workspace it just opened); every other request gets
//! `{}`, and `turn/start` replays the recorded turn's own frames.
use std::collections::BTreeMap;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

/// One recorded frame.
#[derive(Clone)]
struct Frame {
    dir: String,
    method: String,
    body: Value,
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
            let v: Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v.get("body").cloned().unwrap_or(Value::Null),
            }
        })
        .collect()
}

/// The recorded `session/open` result — the frame that carries capabilities,
/// the workspace root and the cursor.
fn recorded_open_result(frames: &[Frame]) -> Option<Value> {
    frames
        .iter()
        .filter(|f| f.dir == "in" && f.method == "session/open")
        .map(|f| &f.body)
        // The richer frame (the one with `active_profile_id`) is the opened result.
        .find(|b| b.get("active_profile_id").is_some())
        .cloned()
}

/// The recorded notification frames for one turn, in order.
fn turn_frames(frames: &[Frame], turn_id: &str) -> Vec<Frame> {
    frames
        .iter()
        .filter(|f| f.dir == "in" && f.method != "session/open" && f.method != "capabilities")
        .filter(|f| !f.method.starts_with("state:"))
        .filter(|f| f.body.get("turn_id").and_then(|t| t.as_str()) == Some(turn_id))
        .map(|f| Frame {
            dir: f.dir.clone(),
            method: f.method.clone(),
            body: f.body.clone(),
        })
        .collect()
}

/// The turn whose frames we replay first (the gate's first turn, 156 frames).
fn first_turn(frames: &[Frame]) -> Option<String> {
    frames
        .iter()
        .filter(|f| f.dir == "in" && f.method == "projection/envelope")
        .filter_map(|f| f.body.get("turn_id").and_then(|t| t.as_str()))
        .map(str::to_owned)
        .next()
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args.get(1).and_then(|p| p.parse().ok()).unwrap_or(8393);
    let delay_ms: u64 = args
        .iter()
        .position(|a| a == "--delay-ms")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);

    let frames = fixture();
    // turn_id -> its frames. A `turn/start` from the client is matched to the
    // next unplayed recorded turn (the app mints its own turn ids, so we cannot
    // match on the id).
    let mut by_turn: BTreeMap<String, Vec<Frame>> = BTreeMap::new();
    for f in frames.iter().filter(|f| f.dir == "in") {
        if let Some(t) = f.body.get("turn_id").and_then(|t| t.as_str()) {
            by_turn.entry(t.to_owned()).or_default().push(Frame {
                dir: f.dir.clone(),
                method: f.method.clone(),
                body: f.body.clone(),
            });
        }
    }
    let open_result = recorded_open_result(&frames).expect("the fixture has a session/open result");
    let first = first_turn(&frames).expect("the fixture has a turn");
    let recorded_turns: Vec<String> = by_turn
        .iter()
        .filter(|(_, v)| v.iter().any(|f| f.method == "projection/envelope"))
        .map(|(k, _)| k.clone())
        .collect();
    println!(
        "[replay-serve] {} frames; {} recorded turns {:?}; first = {first}",
        frames.len(),
        recorded_turns.len(),
        recorded_turns
    );

    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).await.expect("bind");
    println!("[replay-serve] listening on ws://{addr}/api/ui-protocol/ws (delay {delay_ms}ms)");

    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        println!("[replay-serve] client connected");
        let by_turn = by_turn.clone();
        let open_result = open_result.clone();
        let recorded_turns = recorded_turns.clone();
        let first = first.clone();
        tokio::spawn(async move {
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else {
                return;
            };
            let (tx, mut rx) = ws.split();
            let tx = std::sync::Arc::new(tokio::sync::Mutex::new(tx));
            let mut played = 0usize;

            while let Some(Ok(msg)) = rx.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                println!("[replay-serve] <- {method} (id={id})");

                match method.as_str() {
                    "session/open" => {
                        // Honour the app's requested session id, but keep the
                        // recorded capabilities + workspace root.
                        let requested = v["params"]["session_id"]
                            .as_str()
                            .unwrap_or("dsflash:main")
                            .to_owned();
                        // The RPC result is `SessionOpenResult` = `{opened: SessionOpened}`
                        // (`ui_protocol.rs:4572-4574`); the recorded frame's body
                        // IS the `SessionOpened`, so it must be wrapped.
                        let mut opened = open_result.clone();
                        if let Some(obj) = opened.as_object_mut() {
                            obj.insert("session_id".to_owned(), Value::String(requested));
                        }
                        let frame = serde_json::json!({
                            "jsonrpc": "2.0", "id": id, "result": {"opened": opened}
                        });
                        let _ = tx
                            .lock()
                            .await
                            .send(Message::Text(frame.to_string().into()))
                            .await;
                    }
                    "session/list" => {
                        // A real server lists the workspace's sessions — the one
                        // it just opened included (this is what the thread-list
                        // card renders).
                        let session = v["params"]["session_id"]
                            .as_str()
                            .unwrap_or("dsflash:main")
                            .to_owned();
                        let frame = serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"sessions": [{
                                "id": session,
                                "title": "Why does main.rs print 5?",
                                "message_count": 1,
                                "active_turn": false
                            }]}
                        });
                        let _ = tx
                            .lock()
                            .await
                            .send(Message::Text(frame.to_string().into()))
                            .await;
                    }
                    "turn/start" => {
                        let _ = tx
                            .lock()
                            .await
                            .send(Message::Text(
                                serde_json::json!({
                                    "jsonrpc": "2.0", "id": id, "result": {"accepted": true}
                                })
                                .to_string()
                                .into(),
                            ))
                            .await;
                        // Replay the recorded turn: the first `turn/start` gets
                        // the first recorded turn, the second gets the second
                        // (whose terminal is `interrupted`).
                        let turn = recorded_turns
                            .get(played)
                            .cloned()
                            .unwrap_or_else(|| first.clone());
                        played += 1;
                        let frames = by_turn.get(&turn).cloned().unwrap_or_default();
                        println!(
                            "[replay-serve] replaying {turn} ({} recorded frames)",
                            frames.len()
                        );
                        let tx2 = tx.clone();
                        tokio::spawn(async move {
                            for f in frames {
                                let frame = serde_json::json!({
                                    "jsonrpc": "2.0", "method": f.method, "params": f.body
                                });
                                let _ = tx2
                                    .lock()
                                    .await
                                    .send(Message::Text(frame.to_string().into()))
                                    .await;
                                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                            }
                        });
                    }
                    _ => {
                        let _ = tx
                            .lock()
                            .await
                            .send(Message::Text(
                                serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {}})
                                    .to_string()
                                    .into(),
                            ))
                            .await;
                    }
                }
            }
        });
    }
}
