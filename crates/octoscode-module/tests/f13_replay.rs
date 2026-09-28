//! Card #13 §5 — replay the **recorded real turn** through the live path.
//!
//! Unlike `f12_fake_server.rs` (hand-written frames), this test replays
//! `crates/octoscode-client/tests/fixtures/live-turn-a6ea8505.jsonl` — the
//! frames a real `octos serve` (a6ea8505, `dsflash`) actually sent for the
//! prompt `What does 2+3 equal? One word.` (the answer was "Five").
//!
//! The point of the card: #12's proof passed only because its fake server sent
//! bare `message/delta`. The real server sends `projection/envelope`, which no
//! domain handled, so every live update was dropped. This test drives the
//! **module's own `Conversation`** over a fake WS server that speaks the
//! recorded frames, and asserts the transcript, the tool rows and the terminal
//! all land — proving the envelope fold against real traffic.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::{Conversation, FlowEvent};

/// One recorded frame.
#[derive(Debug, Clone)]
struct Frame {
    dir: String,
    method: String,
    body: serde_json::Value,
}

/// Load the committed real fixture.
fn fixture() -> Vec<Frame> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../octoscode-client/tests/fixtures/live-turn-a6ea8505.jsonl"
    );
    let text = std::fs::read_to_string(path).expect("read the live-turn fixture");
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

/// The fixture's turn id (the `projection/envelope` `turn_id`).
fn fixture_turn_id(frames: &[Frame]) -> String {
    frames
        .iter()
        .find(|f| f.dir == "in" && f.method == "projection/envelope")
        .and_then(|f| f.body["turn_id"].as_str().map(str::to_owned))
        .expect("the fixture carries a projection/envelope turn id")
}

/// The fixture's own prompt + its streamed answer (`assistant_delta`/`persisted`).
fn fixture_prompt_and_answer(frames: &[Frame]) -> (String, String) {
    let mut prompt = String::new();
    let mut answer = String::new();
    for f in frames {
        if f.method != "projection/envelope" || f.dir != "in" {
            continue;
        }
        let p = &f.body["payload"];
        let ty = p["type"].as_str().unwrap_or("");
        let data = &p["data"];
        match ty {
            "user_message" => prompt = data["text"].as_str().unwrap_or("").to_owned(),
            "assistant_delta" | "assistant_persisted" => {
                answer = data["text"].as_str().unwrap_or("").to_owned();
            }
            _ => {}
        }
    }
    (prompt, answer)
}

/// A fake server that replays the fixture's notification frames.
struct ReplayServer {
    base_url: String,
    /// Every outbound method the client sent.
    received: Arc<Mutex<Vec<String>>>,
}

impl ReplayServer {
    async fn start(frames: Vec<Frame>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(Mutex::new(Vec::new()));
        let rx = received.clone();

        // The frames the server will stream back, in recording order: every
        // real *notification* (the handshake replies are answered normally).
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
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else {
                return;
            };
            let (tx, mut rx_in) = ws.split();
            let tx = Arc::new(tokio::sync::Mutex::new(tx));

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

                        // Then stream the RECORDED notification frames.
                        let tx2 = tx.clone();
                        let sf = stream_frames.clone();
                        tokio::spawn(async move {
                            for f in sf {
                                let frame = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "method": f.method,
                                    "params": f.body,
                                });
                                let _ = tx2
                                    .lock()
                                    .await
                                    .send(Message::Text(frame.to_string().into()))
                                    .await;
                                tokio::time::sleep(Duration::from_millis(10)).await;
                            }
                        });
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
        });

        Self {
            base_url: format!("http://{addr}"),
            received,
        }
    }
}

#[tokio::test]
async fn the_recorded_real_turn_folds_through_the_live_path() {
    let frames = fixture();
    assert!(!frames.is_empty(), "the fixture must not be empty");
    let (prompt, answer) = fixture_prompt_and_answer(&frames);
    let turn_id = fixture_turn_id(&frames);
    assert_eq!(prompt, "What does 2+3 equal? One word.");
    assert_eq!(answer, "Five", "the fixture holds the real answer");

    // The decisive count: the recorded turn carried real projection envelopes.
    let envelopes = frames
        .iter()
        .filter(|f| f.dir == "in" && f.method == "projection/envelope")
        .count();
    assert!(
        envelopes >= 3,
        "the real turn sent projection/envelope frames (got {envelopes})"
    );

    let server = ReplayServer::start(frames).await;
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
        .expect("connect");

    conv.open_workspace(None).await.expect("session/open");
    conv.start_turn(&prompt).await.expect("turn/start");

    let mut saw = Vec::new();
    for _ in 0..120 {
        match tokio::time::timeout(Duration::from_millis(120), events.recv()).await {
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

    // 1) The assistant answer landed as streamed text (the gate's whole point).
    assert!(
        joined.contains("Delta"),
        "the projection assistant_delta must fold to a Delta; got {joined}"
    );
    assert_eq!(
        conv.store.live_text("dsflash:main"),
        answer,
        "the transcript shows the real answer"
    );
    // 2) The user's own prompt is a user.message entry (card #13 §4).
    let entries = conv.store.domains.session.timeline.entries("dsflash:main");
    let user_entries: Vec<_> = entries
        .iter()
        .filter(|e| e.kind == octoscode_store::EntryKind::USER_MESSAGE)
        .collect();
    assert_eq!(user_entries.len(), 1, "one user.message entry");
    assert_eq!(user_entries[0].text, prompt);
    // 3) The terminal settled the turn (the button leaves "Queue").
    assert!(
        joined.contains("TurnEnded"),
        "the projection turn_terminal must end the turn; got {joined}"
    );
    assert_eq!(
        conv.store.domains.turn.terminal(&turn_id).as_deref(),
        Some("completed")
    );
    assert!(!conv.store.domains.turn.is_in_flight(&turn_id));
    // 4) The flow's answer facts the fallback view reads.
    {
        let ui = conv.ui();
        let ui = ui.lock().unwrap();
        assert!(!ui.turn_active(), "the turn is no longer live");
        assert!(ui.worked_for().starts_with("Worked for "), "{}", ui.worked_for());
        assert!(ui.draft().is_empty(), "the draft cleared on send");
    }
    // 5) The ordering state advanced (per-thread seq + canonical cursor).
    assert_eq!(
        conv.store.domains.turn.last_envelope_seq(&turn_id),
        Some(4)
    );
    assert!(conv.store.domains.turn.dropped_envelopes().is_empty());

    let received = server.received.lock().unwrap().clone();
    assert!(received.contains(&"session/open".to_owned()));
    assert!(received.contains(&"turn/start".to_owned()));
}

#[tokio::test]
async fn a_replayed_stale_seq_is_dropped() {
    // The ordering rule (`durable-session.ts:185`): a seq that does not
    // strictly increase is dropped, and the cursor keeps the max.
    let store = Arc::new(octoscode_store::Store::new());
    let t = &store.domains.turn;
    assert!(t.accept_envelope("thread", 1, Some(("s", 7))));
    assert!(t.accept_envelope("thread", 2, Some(("s", 8))));
    assert!(!t.accept_envelope("thread", 2, Some(("s", 9))), "duplicate seq dropped");
    assert!(!t.accept_envelope("thread", 1, Some(("s", 10))), "going backwards dropped");
    assert!(t.accept_envelope("thread", 3, Some(("s", 8))), "forward accepted");
    assert_eq!(t.last_envelope_seq("thread"), Some(3));
    // The cursor never goes backwards.
    assert_eq!(t.envelope_cursor(), Some(("s".to_owned(), 8)));
    assert_eq!(t.dropped_envelopes().len(), 2);
}
