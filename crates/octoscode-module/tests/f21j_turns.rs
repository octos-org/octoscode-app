//! Card #21j — the worked-for label must come from **its own turn's** terminal.
//!
//! ## The defect
//!
//! `live-gate/evidence/g5-scrolled-top.png` (the timeline scrolled to the top)
//! showed **turn 1, which completed normally, labelled "Interrupted"** — turn 2's
//! outcome. Before turn 2 ran, the same row read `Worked for 1s ›`
//! (`g3-completed.png`). Root cause: `FlowUi` kept a *single* session-level
//! outcome + duration, `worked_for()` rendered that one value globally, and
//! `screen::timeline_rows` pushed a `worked-for` row for every settled turn — so
//! a later turn restamped every earlier row.
//!
//! ## This test
//!
//! Replays the committed **real** recording `r26-interrupted-a6ea8505.jsonl`
//! (turn 1 → `completed`, turn 2 → `interrupted`) through the module's own
//! `Conversation`, then reads the screen's row list and **lowers both worked-for
//! rows** through the component's own binding path. Turn 1 must say
//! `Worked for …`; turn 2 must say `Interrupted`. It fails on `main` (`c0fd521`),
//! where both rows render the single global (latest) outcome.
//!
//! The per-turn store record is checked too: each turn's own `TurnEnd.outcome`
//! is its own terminal, never the other's.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::bindings::Ctx;
use octoscode_module::components::{self, ItemKind};
use octoscode_module::flow::{Conversation, FlowEvent, FlowUi};
use octoscode_module::screen;

/// One recorded frame.
#[derive(Debug, Clone)]
struct Frame {
    dir: String,
    method: String,
    body: serde_json::Value,
}

fn load(path: &str) -> Vec<Frame> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
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

/// The committed interrupted-case recording: turn 1 completed, turn 2 interrupted.
fn interrupted_fixture() -> Vec<Frame> {
    load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../octoscode-client/tests/fixtures/r26-interrupted-a6ea8505.jsonl"
    ))
}

/// The `turn/start` a recorded turn was driven with: `(turn_id, prompt)`.
fn recorded_turns(frames: &[Frame]) -> Vec<(String, String)> {
    frames
        .iter()
        .filter(|f| f.dir == "out" && f.method == "turn/start")
        .filter_map(|f| {
            let id = f.body["turn_id"].as_str()?.to_owned();
            let text = f.body["input"][0]["text"].as_str()?.to_owned();
            Some((id, text))
        })
        .collect()
}

/// A fake WS server that replays the recording's notification frames.
struct ReplayServer {
    base_url: String,
}

impl ReplayServer {
    async fn start(frames: Vec<Frame>) -> Self {
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
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
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
                                    "jsonrpc": "2.0",
                                    "method": f.method,
                                    "params": f.body,
                                });
                                let _ = tx2
                                    .lock()
                                    .await
                                    .send(Message::Text(frame.to_string().into()))
                                    .await;
                                tokio::time::sleep(Duration::from_millis(2)).await;
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
        Self { base_url: format!("http://{addr}") }
    }
}

async fn drain_until<P>(
    conv: &Conversation,
    events: &mut tokio::sync::mpsc::Receiver<octos_app_transport::TransportEvent>,
    mut stop: P,
) where
    P: FnMut(&FlowEvent) -> bool,
{
    for _ in 0..400 {
        match tokio::time::timeout(Duration::from_millis(120), events.recv()).await {
            Ok(Some(evt)) => {
                let e = conv.on_event(evt);
                if stop(&e) {
                    break;
                }
            }
            _ => break,
        }
    }
}

/// Drive both recorded turns to their terminals.
async fn drive_both_turns() -> (Conversation, Vec<(String, String)>) {
    let frames = interrupted_fixture();
    let turns = recorded_turns(&frames);
    assert_eq!(turns.len(), 2, "the recording drove two turns");
    let server = ReplayServer::start(frames).await;
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
        .expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    drain_until(&conv, &mut events, |_| false).await;
    for (id, prompt) in &turns {
        conv.start_turn_with_id(prompt, id.clone())
            .await
            .expect("turn/start");
        drain_until(&conv, &mut events, |e| matches!(e, FlowEvent::TurnEnded { .. })).await;
    }
    (conv, turns)
}

/// The label a `worked-for` row renders, via the component's own binding path.
fn worked_for_label(
    store: &Arc<octoscode_store::Store>,
    ui: &Arc<Mutex<FlowUi>>,
    row: &screen::Row,
) -> String {
    let copies = {
        let ctx = Ctx::new(store, ui);
        components::item_copies(ItemKind::WorkedFor, &ctx, row.index, row.turn.as_deref())
            .expect("the worked-for row's binding resolves")
    };
    copies
        .iter()
        .find(|(copy, _)| copy == "worked_row_label_text")
        .map(|(_, v)| v.clone())
        .expect("the worked-for component binds `worked_row_label_text`")
}

/// **The card's required assertion.** Turn 1 (completed) renders `Worked for …`;
/// turn 2 (interrupted) renders `Interrupted`. On `main` both render the single
/// global outcome, so this fails there.
#[tokio::test]
async fn each_turn_renders_its_own_worked_for_label() {
    let (conv, turns) = drive_both_turns().await;
    let store = conv.store.clone();
    let ui = conv.ui();

    // The store's own per-turn record carries each turn's own terminal.
    for (id, _) in &turns {
        assert!(
            ui.lock().unwrap().turn_end(id).is_some(),
            "turn {id} recorded its own terminal"
        );
    }
    assert_eq!(
        ui.lock().unwrap().turn_end(&turns[0].0).map(|e| e.outcome.clone()),
        Some("completed".to_owned()),
        "turn 1's own outcome"
    );
    assert_eq!(
        ui.lock().unwrap().turn_end(&turns[1].0).map(|e| e.outcome.clone()),
        Some("interrupted".to_owned()),
        "turn 2's own outcome"
    );

    // The screen renders a worked-for row per settled turn, each carrying its
    // OWN turn id.
    let rows = screen::timeline_rows(&store, false);
    let worked: Vec<&screen::Row> = rows
        .iter()
        .filter(|r| r.kind == ItemKind::WorkedFor)
        .collect();
    assert_eq!(worked.len(), 2, "two settled turns → two worked-for rows");
    assert_eq!(
        worked[0].turn.as_deref(),
        Some(turns[0].0.as_str()),
        "row 1 belongs to turn 1"
    );
    assert_eq!(
        worked[1].turn.as_deref(),
        Some(turns[1].0.as_str()),
        "row 2 belongs to turn 2"
    );

    // The decisive labels.
    let l1 = worked_for_label(&store, &ui, worked[0]);
    let l2 = worked_for_label(&store, &ui, worked[1]);
    assert!(
        l1.starts_with("Worked for ") && l1.ends_with('›'),
        "turn 1 completed → 'Worked for … ›'; got {l1:?}"
    );
    assert_eq!(
        l2, "Interrupted",
        "turn 2 interrupted → 'Interrupted'; got {l2:?}"
    );
    assert_ne!(
        l1, "Interrupted",
        "a later turn's terminal must never relabel an earlier turn's row"
    );
}

/// A turn that neither settled nor produced a reply shows **no** settled tail.
///
/// The top-scrolled live capture (`g-21j-top.png`) showed a blank worked-for pill:
/// a prompt whose `turn/start` never came back (a replay-driven capture mints
/// such a group) had no terminal and no reply, yet still got a worked-for row
/// that then rendered empty. The settled tail is a turn's outcome disclosure, so
/// it renders only when the turn actually has one.
#[test]
fn a_turn_without_a_terminal_or_reply_has_no_worked_for_row() {
    let store = Arc::new(octoscode_store::Store::new());
    store.set_active(Some("dsflash:main".into()));
    let tl = &store.domains.session.timeline;
    // A prompt with NO reply and NO `turn_terminal` (the minted-artifact group).
    tl.upsert_user_message("dsflash:main", "t-unanswered", "hi", serde_json::json!({}));
    let rows = screen::timeline_rows(&store, false);
    assert!(
        !rows.iter().any(|r| r.kind == ItemKind::WorkedFor),
        "a turn with no terminal and no reply must not show a settled tail; got {:?}",
        rows.iter().map(|r| r.kind.id()).collect::<Vec<_>>()
    );

    // A turn that produced a reply DOES show it (the gate is not over-eager).
    tl.append_delta(
        "dsflash:main",
        Some("t-replied"),
        octoscode_store::EntryKind::ASSISTANT_TEXT,
        "because rayleigh",
    );
    store.domains.turn.set_terminal("t-replied", "completed");
    let rows = screen::timeline_rows(&store, false);
    assert!(
        rows.iter().any(|r| r.kind == ItemKind::WorkedFor),
        "a turn with a terminal/reply shows its settled tail"
    );
}
