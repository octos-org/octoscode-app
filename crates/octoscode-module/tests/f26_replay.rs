//! Card #26 — optimistic user row on send + acting on the `replay_lossy` resync.
//!
//! §1: the prompt must show the moment it is sent, and must still show for an
//! **interrupted** turn (whose server `user_message` envelope never arrives).
//! §2: a `protocol/replay_lossy` must actually produce a `session/hydrate`,
//! whose result is folded and clears the lossy phase.
//!
//! The §1 tests replay the committed **real** live-gate recording
//! (`crates/octoscode-client/tests/fixtures/live-gate-a6ea8505.jsonl`), whose
//! turn 2 was interrupted and carries no `user_message` payload.
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

/// The committed live-gate recording (2 turns; turn 2 interrupted, no
/// `user_message`).
fn live_gate() -> Vec<Frame> {
    load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../octoscode-client/tests/fixtures/live-gate-a6ea8505.jsonl"
    ))
}

/// The **interrupted-case** recording (board #26 addendum): the live gate's own
/// `live-gate/evidence/trace.jsonl`, scrubbed to `<WORKSPACE>` and committed.
/// Turn 2 was interrupted (`out turn/interrupt`) and carries no `user_message`.
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

/// Which turns the recording carries a `user_message` payload for.
fn recorded_user_turns(frames: &[Frame]) -> std::collections::BTreeSet<String> {
    frames
        .iter()
        .filter(|f| f.dir == "in" && f.method == "projection/envelope")
        .filter(|f| f.body["payload"]["type"] == "user_message")
        .filter_map(|f| f.body["turn_id"].as_str().map(str::to_owned))
        .collect()
}

// ------------------------- §3 canonical hydrate rebuilds the transcript (#P4b2)

#[tokio::test]
async fn a_hydrate_rebuilds_the_transcript_through_the_production_path() {
    // The full production chain: a real transport `SessionHydrated` (raised
    // by the lossy resync, exactly like §2) whose result carries the
    // authoritative `messages` — the flow must rebuild the transcript in seq
    // order (web `restoreCanonicalHydrate`, canonical-hydrate.ts:31) WITHOUT
    // deleting or overwriting the live optimistic row ("ambiguous identity
    // never deletes a transcript row").
    let lossy = Frame {
        dir: "in".to_owned(),
        method: "protocol/replay_lossy".to_owned(),
        body: serde_json::json!({
            "session_id": "dsflash:main",
            "dropped_count": 3,
            "last_durable_cursor": {"stream": "main", "seq": 9}
        }),
    };
    let messages = serde_json::json!([
        {"seq": 10, "role": "user", "content": "why 5?",
         "turn_id": "0b3f6d5e-1111-4111-8111-111111111111",
         "persisted_at": "2026-09-30T00:00:00Z"},
        {"seq": 11, "role": "assistant", "content": "because five",
         "turn_id": "0b3f6d5e-1111-4111-8111-111111111111",
         "reasoning_content": "counting to five",
         "persisted_at": "2026-09-30T00:00:01Z"}
    ]);
    let server = ReplayServer::start_with_hydrate(vec![lossy], messages).await;
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
        .expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    // A LIVE optimistic row (card #26 §1) the snapshot does NOT contain.
    conv.start_turn_with_id("live prompt".to_owned(), "t-live".to_owned())
        .await
        .expect("turn/start");

    // Drain until the rebuild has landed: the live row + the three hydrated
    // entries (user, its reasoning, the answer).
    let saw = drain_until(&conv, &mut events, |_| {
        conv.store.domains.session.timeline.len("dsflash:main") >= 4
    })
    .await;
    let joined = saw.join(" | ");

    // The production trigger really fired.
    let received = server.received.lock().unwrap().clone();
    assert!(
        received.contains(&"session/hydrate".to_owned()),
        "the lossy resync must produce session/hydrate; sent {received:?}"
    );

    let es = conv.store.domains.session.timeline.entries("dsflash:main");
    assert_eq!(es.len(), 4, "live row + 3 hydrated entries; got {joined}");

    // The LIVE row survives, untouched (never deleted, never overwritten).
    let live = es.iter().find(|e| e.text == "live prompt").expect("live row survives");
    assert_eq!(live.kind, octoscode_store::EntryKind::USER_MESSAGE);
    assert!(!live.finalized, "the live row is not a durable receipt");
    assert!(live.data.get("hydrate_id").is_none(), "the live row was not rewritten");

    // The hydrated rows: seq order (10 before 11), the captured reasoning as
    // its own entry right before the answer, all finalized with identities.
    let hyd: Vec<&octoscode_store::TimelineEntry> =
        es.iter().filter(|e| e.data.get("hydrate_id").is_some()).collect();
    assert_eq!(hyd.len(), 3, "every hydrated row carries its seq identity");
    let texts: Vec<&str> = hyd.iter().map(|e| e.text.as_str()).collect();
    assert_eq!(
        texts,
        vec!["why 5?", "counting to five", "because five"],
        "seq order; reasoning immediately before its answer"
    );
    let kinds: Vec<octoscode_store::EntryKind> =
        hyd.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        vec![
            octoscode_store::EntryKind::USER_MESSAGE,
            octoscode_store::EntryKind::REASONING,
            octoscode_store::EntryKind::ASSISTANT_TEXT,
        ]
    );
    assert!(
        hyd.iter().all(|e| e.finalized),
        "durable receipts absorb no further deltas (#21i semantics)"
    );
    assert!(
        hyd.iter().all(|e| e.data["hydrate_id"].as_str().unwrap_or("").starts_with("hydrate:")),
        "idempotency identities present (content `hydrate:seq:N`, its reasoning \
         `hydrate:reasoning:seq:N`): {:?}",
        hyd.iter().map(|e| e.data["hydrate_id"].clone()).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------- fake server

/// A fake WS server that replays frames and answers the lifecycle RPCs.
struct ReplayServer {
    base_url: String,
    received: Arc<Mutex<Vec<String>>>,
}

impl ReplayServer {
    async fn start(frames: Vec<Frame>) -> Self {
        // The real *notification* frames to stream back after `session/open`.
        let stream: Vec<Frame> = frames
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
        Self::start_with_stream(stream, None).await
    }

    async fn start_with_stream(
        stream_frames: Vec<Frame>,
        // #P4b2: `messages` merged into the `session/hydrate` result (None
        // keeps the minimal checkpoint-only snapshot).
        hydrate_messages: Option<serde_json::Value>,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(Mutex::new(Vec::new()));
        let rx = received.clone();

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
                                                          "turn/interrupt", "session/list",
                                                          "session/hydrate"],
                                    "supported_notifications": ["projection/envelope",
                                                                "message/delta", "turn/started",
                                                                "protocol/replay_lossy"],
                                    "supported_features": ["projection.envelope.v2",
                                                           "state.session_hydrate.v1"]
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
                                tokio::time::sleep(Duration::from_millis(10)).await;
                            }
                        });
                    }
                    "session/hydrate" => {
                        // A minimal authoritative snapshot: seed one thread's
                        // continuation checkpoint so the fold is observable.
                        let session = v["params"]["session_id"]
                            .as_str()
                            .unwrap_or("dsflash:main")
                            .to_owned();
                        let mut frame = serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {
                                "session_id": session,
                                "cursor": {"stream": "dsflash:main", "seq": 42},
                                "projection_thread_sequences": {"thread-x": 7}
                            }
                        });
                        if let Some(msgs) = &hydrate_messages {
                            frame["result"]["messages"] = msgs.clone();
                        }
                        let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
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

        Self {
            base_url: format!("http://{addr}"),
            received,
        }
    }

    /// #P4b2: like `start_with_stream`, but the `session/hydrate` result also
    /// carries the authoritative `messages` array.
    async fn start_with_hydrate(stream_frames: Vec<Frame>, messages: serde_json::Value) -> Self {
        Self::start_with_stream(stream_frames, Some(messages)).await
    }
}

/// Drain events into the flow until `stop` is true or the budget runs out.
async fn drain_until<P>(
    conv: &Conversation,
    events: &mut tokio::sync::mpsc::Receiver<octos_app_transport::TransportEvent>,
    mut stop: P,
) -> Vec<String>
where
    P: FnMut(&FlowEvent) -> bool,
{
    let mut saw = Vec::new();
    for _ in 0..400 {
        match tokio::time::timeout(Duration::from_millis(120), events.recv()).await {
            Ok(Some(evt)) => {
                let e = conv.on_event(evt);
                let done = stop(&e);
                saw.push(format!("{e:?}"));
                if done {
                    break;
                }
            }
            _ => break,
        }
    }
    saw
}

// ------------------------------------------------------------ §1 the bug (a)

#[tokio::test]
async fn an_interrupted_turn_still_shows_its_user_row() {
    let frames = live_gate();
    let turns = recorded_turns(&frames);
    assert!(turns.len() >= 2, "the recording drove at least two turns");
    let user_turns = recorded_user_turns(&frames);
    // The premise: the server sent a `user_message` for turn 1 but NOT turn 2.
    assert!(user_turns.contains(&turns[0].0), "turn 1 has a server user_message");
    assert!(
        !user_turns.contains(&turns[1].0),
        "turn 2 (interrupted) has NO server user_message — the live bug"
    );

    let server = ReplayServer::start(frames).await;
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
        .expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    drain_until(&conv, &mut events, |_| false).await;

    // Drive BOTH turns with their recorded ids (the web's own mint).
    for (id, prompt) in &turns[..2] {
        conv.start_turn_with_id(prompt, id.clone())
            .await
            .expect("turn/start");
        drain_until(&conv, &mut events, |e| matches!(e, FlowEvent::TurnEnded { .. })).await;
    }

    let tl = &conv.store.domains.session.timeline;
    let users = tl.of_kind("dsflash:main", octoscode_store::EntryKind::USER_MESSAGE);
    let prompts: Vec<&str> = users.iter().map(|e| e.text.as_str()).collect();
    // The decisive assertion: turn 2's prompt is present even though its
    // server `user_message` never arrived.
    assert!(
        prompts.contains(&turns[1].1.as_str()),
        "the interrupted turn's prompt must show; got {prompts:?}"
    );
    assert_eq!(users.len(), 2, "exactly one user row per turn: {prompts:?}");
}

/// Board #26 addendum (§8.14): the same claim on the **interrupted-case**
/// recording the live gate itself produced, scrubbed into
/// `r26-interrupted-a6ea8505.jsonl`. Turn 2 carries `out turn/interrupt` and NO
/// `user_message`, so a naive client shows only turn 1's bubble; with the
/// optimistic row turn 2's prompt must still show.
#[tokio::test]
async fn the_interrupted_case_recording_shows_turn_2s_prompt() {
    let frames = interrupted_fixture();
    let turns = recorded_turns(&frames);
    assert_eq!(turns.len(), 2, "the recording drove two turns");
    let user_turns = recorded_user_turns(&frames);
    assert!(user_turns.contains(&turns[0].0), "turn 1 has a server user_message");
    assert!(
        !user_turns.contains(&turns[1].0),
        "turn 2 was interrupted: no server user_message"
    );

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

    let tl = &conv.store.domains.session.timeline;
    let users = tl.of_kind("dsflash:main", octoscode_store::EntryKind::USER_MESSAGE);
    let prompts: Vec<&str> = users.iter().map(|e| e.text.as_str()).collect();
    assert!(
        prompts.contains(&turns[1].1.as_str()),
        "the interrupted turn's prompt must show from the recording; got {prompts:?}"
    );
    assert_eq!(users.len(), 2, "exactly one user row per turn: {prompts:?}");
}

// -------------------------------------------------------------- §1 dedup (b)

#[test]
fn optimistic_and_server_user_rows_dedup_to_one() {
    let store = octoscode_store::Store::new();
    let tl = &store.domains.session.timeline;
    // The optimistic insert (what `start_turn_with_id` does now)…
    tl.upsert_user_message("s", "t1", "why is the sky blue?", serde_json::json!({"optimistic": true}));
    // …then the canonical server copy for the SAME turn id.
    tl.upsert_user_message("s", "t1", "why is the sky blue?", serde_json::json!({"files": []}));
    let users = tl.of_kind("s", octoscode_store::EntryKind::USER_MESSAGE);
    assert_eq!(users.len(), 1, "one user row per turn id, never two: {users:?}");
    assert_eq!(users[0].text, "why is the sky blue?");
}

// ------------------------------------------------------------ §1 ordering (c)

#[test]
fn the_user_row_precedes_its_turns_reasoning_and_answer() {
    let store = octoscode_store::Store::new();
    let tl = &store.domains.session.timeline;
    // A reply lands FIRST (the real server sends `user_message` last, behind the
    // deltas)…
    tl.append_delta("s", Some("t1"), octoscode_store::EntryKind::REASONING, "hmm");
    tl.append_delta("s", Some("t1"), octoscode_store::EntryKind::ASSISTANT_TEXT, "because rayleigh");
    // …then the user row is inserted (optimistic send or the late server copy).
    tl.upsert_user_message("s", "t1", "why blue?", serde_json::json!({}));

    let entries = tl.entries("s");
    let user_at = entries
        .iter()
        .position(|e| e.kind == octoscode_store::EntryKind::USER_MESSAGE)
        .expect("a user row");
    let first_reply = entries
        .iter()
        .position(|e| {
            e.kind == octoscode_store::EntryKind::REASONING
                || e.kind == octoscode_store::EntryKind::ASSISTANT_TEXT
        })
        .expect("a reply row");
    assert!(
        user_at < first_reply,
        "the user row must precede its turn's reasoning/answer: {:?}",
        entries.iter().map(|e| e.kind.tag()).collect::<Vec<_>>()
    );
}

// ------------------------------------------- §1 no turn from an empty draft

#[tokio::test]
async fn an_empty_draft_starts_no_turn() {
    // Found by the live proof: the composer's send control doubles as STOP while
    // a turn is live, so a stop-glyph click that lands just after the turn
    // settled routed to `composer.submit` with the already-cleared draft and
    // minted an empty optimistic row. The web refuses the same at its submit
    // entry (`use-turn-controller.ts:631` `!text.trim()`).
    let server = ReplayServer::start_with_stream(vec![], None).await;
    let (conv, _events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
        .expect("connect");
    conv.open_workspace(None).await.expect("session/open");

    // The draft is empty (never typed, or already cleared by a prior send).
    assert!(conv.ui().lock().unwrap().draft().is_empty());
    let id = conv.submit_draft().await.expect("submit is not an error");
    assert!(id.is_empty(), "an empty draft starts no turn, got {id:?}");

    // No `turn/start` reached the wire, and no user row was minted.
    let received = server.received.lock().unwrap().clone();
    assert!(
        !received.contains(&"turn/start".to_owned()),
        "an empty draft must send no turn/start; sent {received:?}"
    );
    let users = conv
        .store
        .domains
        .session
        .timeline
        .of_kind("dsflash:main", octoscode_store::EntryKind::USER_MESSAGE);
    assert!(users.is_empty(), "no phantom empty user row: {users:?}");
}

// -------------------------------------------------- §2 act on the lossy resync

#[tokio::test]
async fn a_replay_lossy_triggers_a_hydrate_that_clears_the_lossy_phase() {
    // A fixture that injects `protocol/replay_lossy` (the card's §2 test).
    let lossy = Frame {
        dir: "in".to_owned(),
        method: "protocol/replay_lossy".to_owned(),
        body: serde_json::json!({
            "session_id": "dsflash:main",
            "dropped_count": 3,
            "last_durable_cursor": {"stream": "main", "seq": 9}
        }),
    };
    let server = ReplayServer::start_with_stream(vec![lossy], None).await;
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
        .expect("connect");
    conv.open_workspace(None).await.expect("session/open");

    // Drain until the hydrate reply has been folded (the lossy phase clears).
    let saw = drain_until(&conv, &mut events, |_| {
        conv.store.domains.config.recovery("dsflash:main").resync_pending == false
            && conv.store.domains.config.recovery("dsflash:main").phase
                == octoscode_store::domains::config::LossyPhase::Healthy
            && conv.store.domains.turn.last_envelope_seq("thread-x") == Some(7)
    })
    .await;
    let joined = saw.join(" | ");

    // The client actually issued `session/hydrate` (the resync the #22 hook
    // raised)…
    let received = server.received.lock().unwrap().clone();
    assert!(
        received.contains(&"session/hydrate".to_owned()),
        "the lossy event must produce a session/hydrate; sent {received:?}"
    );
    // …its result was folded (the continuation checkpoint landed)…
    assert_eq!(
        conv.store.domains.turn.last_envelope_seq("thread-x"),
        Some(7),
        "the hydrate's projection_thread_sequences folded: {joined}"
    );
    // …and the session returned to healthy.
    let rec = conv.store.domains.config.recovery("dsflash:main");
    assert_eq!(
        rec.phase,
        octoscode_store::domains::config::LossyPhase::Healthy,
        "mark_recovered cleared the lossy phase: {joined}"
    );
    assert!(!rec.resync_pending, "the resync was consumed");
    // The lossy observation is still recorded (the #22 projection).
    assert_eq!(conv.store.domains.config.replay_loss("dsflash:main").unwrap().dropped_count, 3);
}
