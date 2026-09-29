//! Card #21c — the **L1/L2 live defects**, replayed through the module's own
//! `Conversation` over the real 2-turn recording.
//!
//! ## L1 — the stop control must send `turn/interrupt`
//!
//! The live dsflash run (main `be8188a`) showed a stop click mid-turn produce no
//! outbound `turn/interrupt` (`live-gate/evidence/trace.jsonl`, run 3: turn 2
//! ran to `stream_end` at 16.6 s). Root cause: `FlowUi::end_turn` was **not
//! gated by turn id**, so any terminal — including one for a *different* turn —
//! cleared the running turn's `active_turn`, making `turn.interrupt` resolve to
//! `Effect::Unhandled`. The gate now only ends the turn whose id it names.
//!
//! ## L2 — the second turn must appear
//!
//! Turn 2's frames DID arrive (`trace.jsonl`: 1007 `projection/envelope` frames
//! on turn 2), yet the screen showed only turn 1. This test replays two real
//! consecutive turns and asserts the store folds both and the row model yields
//! two bubbles + two answers. The widget-layer half (the timeline must tail the
//! newest row) is proved by the headless capture, and is fixed by `auto_tail`.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::actions::{self, Effect};
use octoscode_module::bindings::Ctx;
use octoscode_module::components::ItemKind;
use octoscode_module::flow::{Conversation, FlowEvent, FlowUi};
use octoscode_module::screen::timeline_rows;

#[derive(Debug, Clone)]
struct Frame {
    dir: String,
    method: String,
    body: serde_json::Value,
}

fn fixture() -> Vec<Frame> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../octoscode-client/tests/fixtures/live-two-turn-a6ea8505.jsonl"
    );
    let text = std::fs::read_to_string(path).expect("read the two-turn fixture");
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

/// A replay server that streams the recording's notifications and records every
/// outbound request with its params (so a test can assert the `turn_id`).
/// The first recorded turn id (from the recording's own frames).
///
/// Card #26 §1: the web mints the turn id client-side and sends it
/// (`client.ts:488`), and the server's `user_message` carries that same id — so
/// an optimistic row keyed by it dedups into the server copy. This fixture was
/// recorded without `out` frames, so a replay must name the turn explicitly;
/// otherwise a freshly minted id can never match the recorded copy and the row
/// would double.
fn first_recorded_turn_id(frames: &[Frame]) -> String {
    frames
        .iter()
        .filter(|f| f.dir == "in")
        .find_map(|f| {
            f.body["turn_id"]
                .as_str()
                .or_else(|| f.body["payload"]["turn_id"].as_str())
                .map(str::to_owned)
        })
        .expect("the recording names a turn")
}

struct ReplayServer {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
}

impl ReplayServer {
    async fn start(frames: Vec<Frame>, deliver_on_open: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let rec = seen.clone();
        let stream_frames: Vec<Frame> = frames
            .iter()
            .filter(|f| {
                f.dir == "in"
                    && matches!(
                        f.method.as_str(),
                        "turn/started" | "context/normalization_reported"
                            | "progress/updated" | "projection/envelope"
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
                let params = v.get("params").cloned().unwrap_or(serde_json::Value::Null);
                rec.lock().unwrap().push((method.clone(), params.clone()));

                match method.as_str() {
                    "session/open" => {
                        let session = params["session_id"]
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
                                                          "turn/interrupt", "turn/steer",
                                                          "session/list"],
                                    "supported_notifications": ["projection/envelope",
                                                                "message/delta", "turn/started"],
                                    "supported_features": ["projection.envelope.v2",
                                                           "event.turn_steer_dropped.v1"]
                                }
                            }}
                        });
                        let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
                        if deliver_on_open {
                            let tx2 = tx.clone();
                            let sf = stream_frames.clone();
                            tokio::spawn(async move {
                                for f in sf {
                                    let frame = serde_json::json!({
                                        "jsonrpc": "2.0", "method": f.method, "params": f.body,
                                    });
                                    let _ = tx2.lock().await
                                        .send(Message::Text(frame.to_string().into())).await;
                                    tokio::time::sleep(Duration::from_millis(1)).await;
                                }
                            });
                        }
                    }
                    "turn/start" => {
                        let _ = tx.lock().await.send(Message::Text(
                            serde_json::json!({"jsonrpc":"2.0","id":id,"result":{"accepted":true}})
                                .to_string().into())).await;
                    }
                    _ => {
                        let _ = tx.lock().await.send(Message::Text(
                            serde_json::json!({"jsonrpc":"2.0","id":id,"result":{}})
                                .to_string().into())).await;
                    }
                }
            }
        });

        Self { base_url: format!("http://{addr}"), seen }
    }

    fn param_of(&self, method: &str) -> Option<serde_json::Value> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|(m, _)| m == method)
            .map(|(_, p)| p.clone())
    }
}

// ---------------------------------------------------------------------------
// L1 — the stop control
// ---------------------------------------------------------------------------

/// The gate itself: a terminal for a turn that is NOT live must leave the live
/// turn in place (with the old unconditional `end_turn` this failed, and the
/// live run's stop click then resolved to `Unhandled`).
#[test]
fn a_terminal_for_another_turn_does_not_clear_the_running_turn() {
    let ui = Arc::new(Mutex::new(FlowUi::default()));
    ui.lock().unwrap().begin_turn_now("turn-A");
    assert!(ui.lock().unwrap().turn_active());

    // A stale terminal arrives for a DIFFERENT turn.
    ui.lock().unwrap().end_turn_for_test("turn-B", true);
    assert_eq!(
        ui.lock().unwrap().active_turn().as_deref(),
        Some("turn-A"),
        "a terminal for turn-B must not clear the live turn-A"
    );

    // …and the stop click still names the running turn.
    let store = Arc::new(octoscode_store::Store::new());
    let ctx = Ctx::new(&store, &ui);
    assert_eq!(
        actions::resolve("turn.interrupt", 0, &ctx),
        Effect::Interrupt("turn-A".into()),
        "the stop control resolves to the RUNNING turn's id"
    );

    // The live turn's own terminal ends it.
    ui.lock().unwrap().end_turn_for_test("turn-A", true);
    assert!(!ui.lock().unwrap().turn_active(), "turn-A's own terminal ends it");
}

/// The wire half: over the real recording, drive to a running turn, route the
/// stop control, and the server must observe `turn/interrupt` naming the
/// running `turn_id` (the board's required replay test).
#[tokio::test]
async fn the_stop_control_sends_turn_interrupt_with_the_running_turn_id() {
    let server = ReplayServer::start(fixture(), true).await;
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
        .expect("connect");

    conv.open_workspace(None).await.expect("session/open");
    conv.start_turn("first prompt").await.expect("turn/start");

    // Drain until `turn/started` made a turn live (the real server's own id).
    let mut running = None;
    for _ in 0..400 {
        match tokio::time::timeout(Duration::from_millis(80), events.recv()).await {
            Ok(Some(evt)) => {
                let e = conv.on_event(evt);
                if let FlowEvent::TurnStarted(id) = &e {
                    running = Some(id.clone());
                    break;
                }
            }
            _ => break,
        }
    }
    let running = running.expect("the recording's turn/started made a turn live");
    assert_eq!(
        conv.ui().lock().unwrap().active_turn().as_deref(),
        Some(running.as_str()),
        "the live turn is the one the server named"
    );

    // Route the stop control (exactly what `lib.rs` does for `ids!(stop)`).
    let ui = conv.ui();
    let ctx = Ctx::new(&conv.store, &ui);
    let effect = actions::resolve("turn.interrupt", 0, &ctx);
    assert_eq!(effect, Effect::Interrupt(running.clone()));
    if let Effect::Interrupt(turn) = effect {
        conv.interrupt(&turn).await.expect("turn/interrupt");
    }

    let params = server
        .param_of("turn/interrupt")
        .expect("the stop control reached the wire");
    assert_eq!(
        params["turn_id"].as_str(),
        Some(running.as_str()),
        "turn/interrupt names the RUNNING turn (got {params})"
    );
    assert_eq!(params["session_id"].as_str(), Some("dsflash:main"));
}

// ---------------------------------------------------------------------------
// L2 — both turns render
// ---------------------------------------------------------------------------

#[tokio::test]
async fn two_replayed_real_turns_render_as_two_bubbles_and_two_answers() {
    let frames = fixture();
    // Card #26 §1: this recording has no `out` frames, so name the first turn
    // explicitly — the optimistic row then dedups into that turn's replayed
    // `user_message` (two recorded turns → exactly two user rows).
    let turn_id = first_recorded_turn_id(&frames);
    let server = ReplayServer::start(frames, true).await;
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
        .expect("connect");

    conv.open_workspace(None).await.expect("session/open");
    conv.start_turn_with_id("first prompt", turn_id)
        .await
        .expect("turn/start");

    let mut ended = 0;
    for _ in 0..6000 {
        match tokio::time::timeout(Duration::from_millis(40), events.recv()).await {
            Ok(Some(evt)) => {
                if matches!(conv.on_event(evt), FlowEvent::TurnEnded { .. }) {
                    ended += 1;
                }
            }
            _ => break,
        }
    }
    assert_eq!(ended, 2, "both recorded turns reached a terminal");

    let entries = conv.store.domains.session.timeline.entries("dsflash:main");
    let users = entries.iter().filter(|e| e.kind == octoscode_store::EntryKind::USER_MESSAGE).count();
    let answers = entries.iter().filter(|e| e.kind == octoscode_store::EntryKind::ASSISTANT_TEXT).count();
    assert_eq!(users, 2, "the store folded BOTH user messages");
    assert_eq!(answers, 2, "the store folded BOTH answers");

    let rows = timeline_rows(&conv.store, false);
    let kinds: Vec<&str> = rows.iter().map(|r| r.kind.id()).collect();
    let bubbles = rows.iter().filter(|r| r.kind == ItemKind::UserBubble).count();
    let prose = rows.iter().filter(|r| r.kind == ItemKind::AssistantProse).count();
    assert_eq!(bubbles, 2, "two bubbles in the row model; got {kinds:?}");
    assert_eq!(prose, 2, "two answers in the row model; got {kinds:?}");
    // …in display order: each turn's bubble precedes its answer.
    let first_bubble = rows.iter().position(|r| r.kind == ItemKind::UserBubble).unwrap();
    let second_bubble = rows.iter().skip(first_bubble + 1).position(|r| r.kind == ItemKind::UserBubble).unwrap() + first_bubble + 1;
    let first_answer = rows.iter().position(|r| r.kind == ItemKind::AssistantProse).unwrap();
    assert!(first_bubble < first_answer, "turn 1 bubble precedes its answer");
    assert!(first_answer < second_bubble, "turn 2 bubble comes after turn 1's answer");
}
