//! Card #15 §5 / card #19 — a **replay server**: committed real fixtures served
//! on a port so the real hidden app can mount its cards against recorded
//! traffic. **No model is called.**
//!
//! Card #19 extends it with **named scenarios**: `--scenario <name>` chooses
//! which committed recording in `crates/octoscode-client/tests/fixtures/` is
//! served, so the walk-runner can drive one capability at a time:
//!
//! ```sh
//! cargo run -p octoscode-module --example replay_serve -- 8380 --scenario approval
//! # then point the app at it (its own port block, 8370-8379):
//! OCTOS_BASE_URL=http://127.0.0.1:8380 OCTOS_PROFILE_ID=dsflash \
//!   MAKEPAD_HIDE_WINDOWS=1 MAKEPAD_WM_TEST_APP=octoscode \
//!   <octosense> --module octoscode --remote 8370
//! ```
//!
//! Scenarios (fixture → what it exercises):
//!
//! | name | fixture | exercises |
//! |---|---|---|
//! | `conversation` (default) | `live-gate-a6ea8505` | one full streamed turn |
//! | `interrupt` | `live-gate-a6ea8505` | the 2nd turn (terminal `interrupted`) |
//! | `live-turn` | `live-turn-a6ea8505` | the minimal captured turn |
//! | `approval` | `r5-turn-a6ea8505` | `approval/requested` + decisions |
//! | `autonomy` | `r1-autonomy-a6ea8505` | monitors, loops, session goals |
//! | `task` | `r4-task-a6ea8505` | `task/updated`, task artifacts |
//! | `peer` | `r6-peer-a6ea8505` | `peer/gather`, `peer/prepare` |
//! | `session` | `r3-session-a6ea8505` | the context-compaction lifecycle |
//!
//! ## Session-id rewriting (why the recording is portable)
//!
//! A recording carries the session id of the profile it was captured under
//! (`dsflash:main`, `octoscode49213:main`, …). The app opens `{OCTOS_PROFILE_ID}:main`.
//! If the two differ, every replayed frame lands in a *different* session than
//! the app has active and **nothing renders**. So the server rewrites the
//! recorded session id to the one the app actually requested in `session/open`
//! — across every served frame, including `cursor.stream`. That is what makes a
//! committed fixture portable between profiles.
//!
//! Handshake: `session/open` is answered from the recording (its own result
//! carries the negotiated capabilities + workspace root); `session/list` gets a
//! one-row reply naming the opened session; the recording's standalone
//! notifications (anything not tied to a turn — approvals, monitors, …) are
//! delivered right after open; `turn/start` replays the next unplayed recorded
//! turn; every other request gets `{}`.
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

/// The scenario → fixture mapping. `(label, file)` under `tests/fixtures/`.
fn scenario_fixture(name: &str) -> (&'static str, &'static str) {
    match name {
        "interrupt" | "conversation" | "live-gate" | "" => {
            ("conversation", "live-gate-a6ea8505.jsonl")
        }
        "live-turn" => ("live-turn", "live-turn-a6ea8505.jsonl"),
        // Card #21c L2: two REAL consecutive turns from one live session, so a
        // `turn/start` twice reproduces the live "second turn never appears".
        "two-turn" => ("two-turn", "live-two-turn-a6ea8505.jsonl"),
        "approval" => ("approval", "r5-turn-a6ea8505.jsonl"),
        // Card #21c item 8: the same recording carries a real TOOL_CALL
        // (`tool_start`/`tool_progress`/`tool_end`), so a `tool-cell` row renders.
        "tool" | "tool-cell" => ("tool", "r5-turn-a6ea8505.jsonl"),
        "autonomy" => ("autonomy", "r1-autonomy-a6ea8505.jsonl"),
        "task" => ("task", "r4-task-a6ea8505.jsonl"),
        "peer" => ("peer", "r6-peer-a6ea8505.jsonl"),
        "session" => ("session", "r3-session-a6ea8505.jsonl"),
        // #32b3: one synthetic turn whose fenced code block carries a 227-column
        // line — the long-code-line render capture (the web wraps: pre-wrap).
        "longcodeline" => ("longcodeline", "longcodeline-a6ea8505.jsonl"),
        other => {
            eprintln!("[replay-serve] unknown scenario '{other}' — using `conversation`");
            ("conversation", "live-gate-a6ea8505.jsonl")
        }
    }
}

fn fixture(file: &str) -> Vec<Frame> {
    let path = format!(
        "{}/../octoscode-client/tests/fixtures/{file}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read the fixture {path}: {e}"));
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
        .find(|b| b.get("active_profile_id").is_some())
        .cloned()
}

/// The session id the recording was captured under (the open result's id, else
/// the most common `session_id` in the frames).
fn recorded_session(frames: &[Frame]) -> String {
    if let Some(id) = recorded_open_result(frames)
        .and_then(|b| b.get("session_id").and_then(|s| s.as_str()).map(str::to_owned))
    {
        return id;
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for f in frames {
        if let Some(s) = f.body.get("session_id").and_then(|s| s.as_str()) {
            *counts.entry(s.to_owned()).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(s, _)| s)
        .unwrap_or_else(|| "dsflash:main".to_owned())
}

/// Recursively rewrite the recorded session id → the app's requested id, so the
/// recording lands in the session the app actually has active.
fn rewrite_session(value: &mut Value, from: &str, to: &str) {
    match value {
        Value::String(s) => {
            if s.contains(from) {
                *s = s.replace(from, to);
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_session(item, from, to);
            }
        }
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                rewrite_session(v, from, to);
            }
        }
        _ => {}
    }
}

/// The recording's standalone notifications: decoded notifications NOT tied to a
/// turn (so they carry no `turn_id`), delivered right after open. `projection/envelope`
/// frames are turn-streamed instead; `session/open`/`capabilities`/`state:` are
/// handshake, not notifications.
fn standalone_notifications(frames: &[Frame]) -> Vec<Frame> {
    frames
        .iter()
        .filter(|f| f.dir == "in")
        .filter(|f| {
            !f.method.starts_with("state:")
                && f.method != "session/open"
                && f.method != "capabilities"
                && f.method != "projection/envelope"
        })
        .filter(|f| f.body.get("turn_id").is_none())
        .cloned()
        .collect()
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
    let scenario = args
        .iter()
        .position(|a| a == "--scenario")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_default();
    let (label, file) = scenario_fixture(&scenario);

    let frames = fixture(file);
    // turn_id -> its frames. The app mints its own turn ids, so a `turn/start`
    // is matched to the next unplayed recorded turn.
    let mut by_turn: BTreeMap<String, Vec<Frame>> = BTreeMap::new();
    for f in frames.iter().filter(|f| f.dir == "in") {
        if let Some(t) = f.body.get("turn_id").and_then(|t| t.as_str()) {
            by_turn.entry(t.to_owned()).or_default().push(f.clone());
        }
    }
    let open_result = recorded_open_result(&frames).expect("the fixture has a session/open result");
    let recorded = recorded_session(&frames);
    let recorded_turns: Vec<String> = by_turn
        .iter()
        .filter(|(_, v)| v.iter().any(|f| f.method == "projection/envelope"))
        .map(|(k, _)| k.clone())
        .collect();
    let standalone = standalone_notifications(&frames);
    println!(
        "[replay-serve] scenario={label} fixture={file}: {} frames; recorded session `{recorded}`; \
         {} recorded turns {:?}; {} standalone notifications",
        frames.len(),
        recorded_turns.len(),
        recorded_turns,
        standalone.len()
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
        let recorded = recorded.clone();
        let recorded_turns = recorded_turns.clone();
        let standalone = standalone.clone();
        tokio::spawn(async move {
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else {
                return;
            };
            let (tx, mut rx) = ws.split();
            let tx = std::sync::Arc::new(tokio::sync::Mutex::new(tx));
            let mut played = 0usize;
            // The session id the app opens; every served frame is rewritten to it.
            let mut active_session = recorded.clone();

            while let Some(Ok(msg)) = rx.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                println!("[replay-serve] <- {method} (id={id})");

                // A helper to send one JSON-RPC reply/notification.
                async fn send(tx: &std::sync::Arc<tokio::sync::Mutex<futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>, Message>>>, v: Value) {
                    let _ = tx.lock().await.send(Message::Text(v.to_string().into())).await;
                }

                match method.as_str() {
                    "session/open" => {
                        let requested = v["params"]["session_id"]
                            .as_str()
                            .unwrap_or(&recorded)
                            .to_owned();
                        active_session = requested.clone();
                        let mut opened = open_result.clone();
                        rewrite_session(&mut opened, &recorded, &requested);
                        if let Some(obj) = opened.as_object_mut() {
                            obj.insert("session_id".to_owned(), Value::String(requested));
                        }
                        send(&tx, serde_json::json!({
                            "jsonrpc": "2.0", "id": id, "result": {"opened": opened}
                        })).await;
                        // Deliver the recording's standalone notifications (approvals,
                        // monitors, task updates, …) so their cards can mount without
                        // needing a turn.
                        let tx2 = tx.clone();
                        let notifs = standalone.clone();
                        let from = recorded.clone();
                        let to = active_session.clone();
                        tokio::spawn(async move {
                            for mut f in notifs {
                                rewrite_session(&mut f.body, &from, &to);
                                let frame = serde_json::json!({
                                    "jsonrpc": "2.0", "method": f.method, "params": f.body
                                });
                                let _ = tx2.lock().await.send(Message::Text(frame.to_string().into())).await;
                                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                            }
                        });
                    }
                    "session/list" => {
                        let session = v["params"]["session_id"]
                            .as_str()
                            .unwrap_or(&active_session)
                            .to_owned();
                        send(&tx, serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"sessions": [{
                                "id": session,
                                "title": "Why does main.rs print 5?",
                                "message_count": 1,
                                "active_turn": false
                            }]}
                        })).await;
                    }
                    // #P4a1 — echo the requested mode as the read-back.
                    "permission/profile/set" => {
                        send(&tx, serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"current": {"mode": v["params"]["update"]["mode"]}}
                        })).await;
                    }
                    "turn/start" => {
                        send(&tx, serde_json::json!({
                            "jsonrpc": "2.0", "id": id, "result": {"accepted": true}
                        })).await;
                        let turn = recorded_turns
                            .get(played)
                            .cloned()
                            .unwrap_or_else(|| recorded_turns.first().cloned().unwrap_or_default());
                        played += 1;
                        let frames = by_turn.get(&turn).cloned().unwrap_or_default();
                        println!(
                            "[replay-serve] replaying {turn} ({} recorded frames)",
                            frames.len()
                        );
                        let tx2 = tx.clone();
                        let from = recorded.clone();
                        let to = active_session.clone();
                        tokio::spawn(async move {
                            for mut f in frames {
                                rewrite_session(&mut f.body, &from, &to);
                                let frame = serde_json::json!({
                                    "jsonrpc": "2.0", "method": f.method, "params": f.body
                                });
                                let _ = tx2.lock().await.send(Message::Text(frame.to_string().into())).await;
                                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                            }
                        });
                    }
                    _ => {
                        send(&tx, serde_json::json!({
                            "jsonrpc": "2.0", "id": id, "result": {}
                        })).await;
                    }
                }
            }
        });
    }
}
