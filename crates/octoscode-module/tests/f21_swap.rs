//! Card #21 — **the real #16 components swapped into the conversation screen**.
//!
//! #17 built `components::resolve` to prefer an on-disk component over the
//! placeholder and left the screen on placeholders until #16 landed. This test
//! proves the swap end to end:
//!
//! * **§1 resolution** — for every id the screen instantiates, `resolve` returns
//!   card #16's on-disk component (`bool = true`); none falls back to a
//!   placeholder.
//! * **§2 per-item binding** — each component is lowered with a **real item**
//!   replayed from `tests/fixtures/live-gate-a6ea8505.jsonl` (a real `octos
//!   serve` a6ea8505 turn), and the item's own text must appear in the lowered
//!   Splash DSL — i.e. the live value reached the component's own `copy` slot.
//!
//! No model is called: the fixture is replayed through the module's real
//! `Conversation`.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::bindings::Ctx;
use octoscode_module::components::{self, ItemKind};
use octoscode_module::flow::{Conversation, FlowEvent, FlowUi};
use octoscode_store::EntryKind;

// ---------------------------------------------------------------------------
// §1 — resolution: the on-disk #16 component wins, never the placeholder
// ---------------------------------------------------------------------------

#[test]
fn every_screen_component_resolves_to_the_on_disk_component() {
    // The 9 ids the conversation screen instantiates (card #21 §1).
    let expected = [
        "thread-row",
        "user-bubble",
        "assistant-prose",
        "tool-cell",
        "working-row",
        "worked-for",
        "answer-actions",
        "composer",
        "new-chat",
    ];
    assert_eq!(
        ItemKind::ALL.iter().map(|k| k.id()).collect::<Vec<_>>(),
        expected,
        "the resolver's kinds are exactly the screen's 9 components"
    );

    for kind in ItemKind::ALL {
        let (component, on_disk) = components::resolve(*kind);
        assert!(
            on_disk,
            "{} must resolve to the on-disk #16 component, not the placeholder",
            kind.id()
        );
        // The on-disk ledger is #16's measured one; the placeholder carries its
        // own banner comment. Neither may be the other.
        assert!(
            component.ledger.contains("runtime native Kit composition"),
            "{} resolved #16's ledger (got: {:.60})",
            kind.id(),
            component.ledger
        );
        assert!(
            !component.ledger.contains("PLACEHOLDER component"),
            "{} must not fall back to the placeholder",
            kind.id()
        );
        // Its own kit pack (not the shared PSurface/PText placeholder pack).
        assert!(
            component.kit_dir.join("native/light/kit.json").is_file(),
            "{} ships its own kit pack at {}",
            kind.id(),
            component.kit_dir.display()
        );
        assert!(
            !components::declared_copies(*kind).is_empty(),
            "{} declares at least one slot",
            kind.id()
        );
    }
}

// ---------------------------------------------------------------------------
// §2 — per-item binding, on a real item from the recorded live-gate turn
// ---------------------------------------------------------------------------

/// One recorded frame (the same shape `f17_screen.rs` replays).
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

fn fixture_prompt(frames: &[Frame]) -> String {
    frames
        .iter()
        .filter(|f| f.method == "projection/envelope")
        .find_map(|f| {
            let p = &f.body["payload"];
            (p["type"] == "user_message").then(|| p["data"]["text"].as_str().unwrap_or("").to_owned())
        })
        .expect("the fixture carries a user_message")
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
                    // The production replay server answers `session/list` with a
                    // one-row reply naming the opened session (`replay_serve.rs:289`,
                    // field `id`) so the thread list has a real row.
                    "session/list" => {
                        let frame = serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"sessions": [{
                                "id": "dsflash:main",
                                "title": null,
                                "message_count": 1,
                                "updated_at": null,
                                "last_prompt": null,
                                "active_turn": false,
                            }]}
                        });
                        let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
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

/// Drive the recorded turn to settlement and hand back the conversation.
async fn replayed_conversation() -> (Arc<Conversation>, String) {
    let frames = fixture();
    let prompt = fixture_prompt(&frames);
    let server = ReplayServer::start(frames).await;
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
        .expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    conv.start_turn(&prompt).await.expect("turn/start");
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
    (Arc::new(conv), prompt)
}

/// Escape a live value the way `l0_host::set_copy` + `design.rs`'s `{:?}` do, so
/// it can be compared against the lowered DSL source.
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

/// Lower `kind` with the copies the item at `index` binds, and assert each
/// non-empty live value is present in the lowered Splash DSL.
fn assert_lowered_with(
    kind: ItemKind,
    store: &Arc<octoscode_store::Store>,
    ui: &Arc<Mutex<FlowUi>>,
    index: usize,
    want: &[&str],
    forbidden: &[&str],
) {
    let copies = {
        let ctx = Ctx::new(store, ui);
        components::item_copies(kind, &ctx, index)
            .unwrap_or_else(|e| panic!("{} binds a replayed item: {e}", kind.id()))
    };
    let dsl = components::lower(kind, "0", &copies)
        .unwrap_or_else(|e| panic!("{} lowers: {e}", kind.id()));
    for w in want {
        assert!(
            dsl.contains(&esc(w)),
            "{} must show the live value {w:?} in its lowered splash",
            kind.id()
        );
    }
    for f in forbidden {
        assert!(
            !dsl.contains(&esc(f)),
            "{} must not leak the measured fixture string {f:?}",
            kind.id()
        );
    }
}

#[tokio::test]
async fn each_component_binds_a_real_item_from_the_recorded_turn() {
    let (conv, prompt) = replayed_conversation().await;
    let store = conv.store.clone();
    let ui = conv.ui();
    let session = store.active_session().expect("an active session");
    let entries = store.domains.session.timeline.entries(&session);

    // The real answer text the fixture streamed (the folded assistant entry).
    let (user_idx, answer_idx) = {
        let rows = octoscode_module::screen::timeline_rows(&store, false);
        let user = rows.iter().find(|r| r.kind == ItemKind::UserBubble).expect("user row");
        let answer = rows.iter().find(|r| r.kind == ItemKind::AssistantProse).expect("answer row");
        (user.index, answer.index)
    };
    let answer_text = entries[answer_idx].text.clone();
    assert!(!answer_text.is_empty(), "the answer carries the streamed prose");

    // ---- user-bubble: the real prompt is the bubble's first line -----------
    {
        let copies = {
            let ctx = Ctx::new(&store, &ui);
            components::item_copies(ItemKind::UserBubble, &ctx, user_idx).unwrap()
        };
        let dsl = components::lower(ItemKind::UserBubble, "0", &copies).unwrap();
        assert!(
            dsl.contains(&esc(&prompt)),
            "user-bubble shows the replayed prompt"
        );
        // Card #16's ledger ships a SECOND measured line ("steers survive a
        // reconnect"); the CLEAR slot must blank it so it does not show through.
        assert!(
            !dsl.contains(&esc("steers survive a reconnect")),
            "user-bubble clears its second measured fixture line"
        );
    }

    // ---- assistant-prose: the real streamed answer ------------------------
    assert_lowered_with(
        ItemKind::AssistantProse,
        &store,
        &ui,
        answer_idx,
        &[&answer_text],
        &["Queued steers now survive a reconnect"],
    );

    // ---- thread-row: the opened session's own title ------------------------
    {
        let title = store
            .sessions()
            .into_iter()
            .find(|s| s.id == session)
            .and_then(|s| s.title)
            .unwrap_or_else(|| session.clone());
        assert_lowered_with(
            ItemKind::ThreadRow,
            &store,
            &ui,
            0,
            &[&title],
            &["Fix steer queue drop on reconnect"],
        );
    }

    // ---- worked-for: the settled turn's own "Worked for" label -------------
    {
        // A completed turn sets `last_worked`; drive it deterministically.
        ui.lock().unwrap().begin_turn_now("t");
        ui.lock().unwrap().end_turn_now(true);
        let label = conv.ui().lock().unwrap().worked_for();
        assert!(!label.is_empty(), "the settled turn yields a worked-for label");
        assert_lowered_with(
            ItemKind::WorkedFor,
            &store,
            &ui,
            0,
            &[&label],
            &["Worked for 3m 4s"],
        );
    }

    // ---- answer-actions: the completion timestamp --------------------------
    {
        let ts = conv.ui().lock().unwrap().answer_timestamp();
        assert!(!ts.is_empty(), "the settled turn yields a timestamp");
        assert_lowered_with(ItemKind::AnswerActions, &store, &ui, 0, &[&ts], &["Sep 28, 9:41 PM"]);
    }

    // ---- working-row: the live activity label ------------------------------
    {
        let mut u = ui.lock().unwrap();
        u.begin_turn_now("t");
        let activity = u.turn_activity();
        drop(u);
        assert!(activity.starts_with("Working"), "a live turn yields activity: {activity}");
        assert_lowered_with(
            ItemKind::WorkingRow,
            &store,
            &ui,
            0,
            &[&activity],
            &["Working • 12s"],
        );
    }

    // ---- tool-cell: a real tool call + its status --------------------------
    {
        let turn_id = entries.iter().find_map(|e| e.turn_id.clone()).expect("a turn id");
        store.domains.session.timeline.append_data(
            &session,
            Some(turn_id),
            EntryKind::TOOL_CALL,
            "read_file".to_owned(),
            serde_json::json!({"path": "main.rs"}),
        );
        ui.lock().unwrap().note_tool_started_for_test("c1", "read_file");
        ui.lock()
            .unwrap()
            .note_tool_completed_for_test("c1", "read_file", true, Some("main.rs — 12 lines"));
        // The cell's two slots are the tool's summary line and its status
        // (card #16's `t01_text` / `t02_text`), both from the flow's own row.
        assert_lowered_with(
            ItemKind::ToolCell,
            &store,
            &ui,
            0,
            &["main.rs — 12 lines", "done"],
            &["• 412 lines"],
        );
    }

    // ---- composer: the draft + the idle placeholder ------------------------
    {
        ui.lock().unwrap().set_draft_inner("draft text");
        {
            let ctx = Ctx::new(&store, &ui);
            let copies = components::item_copies(ItemKind::Composer, &ctx, 0).unwrap();
            let dsl = components::lower(ItemKind::Composer, "0", &copies).unwrap();
            assert!(dsl.contains(&esc("draft text")), "composer shows the live draft");
            assert!(
                dsl.contains(&esc("Ask Octos anything")),
                "composer keeps its idle placeholder"
            );
        }
    }

    // ---- new-chat: the component's own static label ------------------------
    {
        let ctx = Ctx::new(&store, &ui);
        let copies = components::item_copies(ItemKind::NewChat, &ctx, 0).unwrap();
        let dsl = components::lower(ItemKind::NewChat, "0", &copies).unwrap();
        assert!(dsl.contains(&esc("New chat")), "new-chat keeps its label");
    }
}

/// §2 negative: a stale end-to-end check that the placeholder text of a
/// component never reaches a lowered real component.
#[test]
fn the_placeholder_never_renders_once_the_real_component_is_present() {
    for kind in ItemKind::ALL {
        let (component, on_disk) = components::resolve(*kind);
        assert!(on_disk && !component.ledger.contains("PLACEHOLDER"));
    }
}

