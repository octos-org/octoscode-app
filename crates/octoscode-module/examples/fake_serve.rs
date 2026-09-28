//! Card #12 §5 — a standalone **fake WS server** for the headless gate proof.
//!
//! It replays a realistic turn (deltas, one tool call, completion) with **no
//! model**, so the module's fallback view can be driven and captured (`/snap`,
//! `/g`) before the live `dsflash` gate runs.
//!
//! ```sh
//! # lane p0-build owns 8390-8399 (card #12); the fake server takes 8391.
//! cargo run -p octoscode-module --example fake_serve -- 8391
//! # then, in another shell, the app points at it:
//! OCTOS_BASE_URL=http://127.0.0.1:8391 ... --module octoscode
//! ```
//!
//! Protocol behaviour is the same as `tests/f12_fake_server.rs` (which asserts
//! the flow against it in-process); this binary just exposes it on a port.
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args
        .get(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(8391);
    // `--delay-ms N` spaces the deltas out (the gate's mid-stream interrupt
    // proof needs a stream long enough to click Stop inside).
    let delay_ms: u64 = args
        .iter()
        .position(|a| a == "--delay-ms")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).await.expect("bind");
    println!("[fake-serve] listening on ws://{addr}/api/ui-protocol/ws (delta delay {delay_ms}ms)");

    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        println!("[fake-serve] client connected");
        tokio::spawn(async move {
            // Accept, logging the handshake's requested features.
            let ws = tokio_tungstenite::accept_hdr_async(
                stream,
                |req: &tokio_tungstenite::tungstenite::handshake::server::Request,
                 resp: tokio_tungstenite::tungstenite::handshake::server::Response| {
                    let feats = req
                        .headers()
                        .get("x-octos-ui-features")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    let query_feats = req
                        .uri()
                        .query()
                        .map(|q| q.split('&').filter(|kv| kv.starts_with("ui_feature=")).count())
                        .unwrap_or(0);
                    println!(
                        "[fake-serve] handshake: {} query ui_feature= params; header={}",
                        query_feats,
                        &feats[..feats.len().min(120)]
                    );
                    Ok(resp)
                },
            )
            .await;
            let Ok(ws) = ws else { return };
            let (tx, mut rx_in) = ws.split();
            // The sink + a per-turn interrupt flag are shared with the replay
            // task so `turn/interrupt` can land MID-STREAM (a blocking replay
            // in this read loop would always finish first).
            let tx = std::sync::Arc::new(tokio::sync::Mutex::new(tx));
            let interrupted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                let method = v["method"].as_str().unwrap_or("");
                let id = v["id"].as_str().unwrap_or("");
                println!("[fake-serve] <- {method} (id={id})");

                match method {
                    "session/open" => {
                        let session = v["params"]["session_id"].as_str().unwrap_or("octoscode:main");
                        let frame = serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"opened": {
                                "session_id": session,
                                "active_profile_id": "octoscode",
                                "cursor": {"stream": "fake", "seq": 1},
                                "capabilities": {
                                    "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                                    "capabilities_schema_version": 1,
                                    "supported_methods": [
                                        "session/open", "session/list", "turn/start",
                                        "turn/interrupt", "config/capabilities/list"
                                    ],
                                    "supported_notifications": [
                                        "message/delta", "turn/started", "turn/completed",
                                        "turn/error", "tool/started", "tool/progress", "tool/completed"
                                    ],
                                    "supported_features": [
                                        "coding.autonomy.v1", "context.lifecycle.v1"
                                    ]
                                }
                            }}
                        });
                        let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
                    }
                    "session/list" => {
                        let frame = serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"sessions": [{
                                "id": "octoscode:main",
                                "title": "Fix steer queue drop on reconnect",
                                "message_count": 3,
                                "active_turn": true
                            }]}
                        });
                        let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
                    }
                    "turn/start" => {
                        let session = v["params"]["session_id"].as_str().unwrap_or("octoscode:main").to_owned();
                        let turn = v["params"]["turn_id"].as_str().unwrap_or("t").to_owned();
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
                        let tx2 = tx.clone();
                        let flag = interrupted.clone();
                        flag.store(false, std::sync::atomic::Ordering::Relaxed);
                        let session2 = session.clone();
                        let turn2 = turn.clone();
                        tokio::spawn(async move {
                        let notif = |m: &str, p: serde_json::Value| {
                            serde_json::json!({"jsonrpc": "2.0", "method": m, "params": p}).to_string()
                        };
                        let _ = tx2.lock().await.send(Message::Text(
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
                        for text in [
                            "I checked the steer queue: ",
                            "the drop happened because the reconnect ",
                            "cleared the pending buffer before flush.",
                        ] {
                            if flag.load(std::sync::atomic::Ordering::Relaxed) {
                                println!("[fake-serve] replay stopped (interrupted mid-stream)");
                                return;
                            }
                            let _ = tx2
                                .lock()
                                .await
                                .send(Message::Text(
                                    notif(
                                        "message/delta",
                                        serde_json::json!({
                                            "session_id": session2, "turn_id": turn2, "text": text
                                        }),
                                    )
                                    .into(),
                                ))
                                .await;
                            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                        }
                        if flag.load(std::sync::atomic::Ordering::Relaxed) {
                            return;
                        }
                        let _ = tx2
                            .lock()
                            .await
                            .send(Message::Text(
                                notif(
                                    "tool/started",
                                    serde_json::json!({
                                        "session_id": session2, "turn_id": turn2,
                                        "tool_call_id": "c1", "tool_name": "bash"
                                    }),
                                )
                                .into(),
                            ))
                            .await;
                        let _ = tx2
                            .lock()
                            .await
                            .send(Message::Text(
                                notif(
                                    "tool/completed",
                                    serde_json::json!({
                                        "session_id": session2, "turn_id": turn2,
                                        "tool_call_id": "c1", "tool_name": "bash",
                                        "success": true, "output_preview": "cargo test -p octoscode-store"
                                    }),
                                )
                                .into(),
                            ))
                            .await;
                        let _ = tx2
                            .lock()
                            .await
                            .send(Message::Text(
                                notif(
                                    "turn/completed",
                                    serde_json::json!({
                                        "session_id": session2, "turn_id": turn2,
                                        "cursor": {"stream": "fake", "seq": 9}
                                    }),
                                )
                                .into(),
                            ))
                            .await;
                        });
                    }
                    "turn/interrupt" => {
                        // Land the interrupt MID-STREAM: the replay task checks
                        // this flag between deltas and stops.
                        interrupted.store(true, std::sync::atomic::Ordering::Relaxed);
                        let _ = tx
                            .lock()
                            .await
                            .send(Message::Text(
                                serde_json::json!({
                                    "jsonrpc": "2.0", "id": id, "result": {"interrupted": true}
                                })
                                .to_string()
                                .into(),
                            ))
                            .await;
                    }
                    _ => {
                        let _ = tx
                            .lock()
                            .await
                            .send(Message::Text(
                                serde_json::json!({
                                    "jsonrpc": "2.0", "id": id, "result": {}
                                })
                                .to_string()
                                .into(),
                            ))
                            .await;
                    }
                }
            }
            println!("[fake-serve] client disconnected");
        });
    }
}
