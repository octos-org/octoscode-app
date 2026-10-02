//! Card #P4g1 — the `session:store-hydrate` rows through the PRODUCTION path
//! (`Conversation::connect` → transport → `Conversation::on_event` →
//! `dispatch` → store domains). The fake below only plays the SERVER side of
//! the wire (the f26 pattern); every client-side behaviour asserted is the
//! real production code.
//!
//! Rows covered:
//! - **204**: a resume open that returns a different `workspace_root` than
//!   the requested cwd is rejected (web `validateCandidateWorkspace`,
//!   `candidate-session.ts:230-243`); a fresh open without a cwd adopts the
//!   canonical root.
//! - **205**: durable projection frames (`projection/envelope`,
//!   `protocol/replay_lossy`) route by session scope — out-of-scope frames
//!   are `wrong_session` ignores on the web (`durable-session.ts:117-124`).
//! - **206**: a hydrate commit verifies the returned session id and adopts
//!   the cursor + continuation checkpoints (web `commitHydrate`,
//!   `durable-session.ts:82-107`); a hydrate naming another session fails
//!   closed.
//! - **213**: peer lifecycle events route only by the full originating
//!   SessionKey (`scope.ts:27-31`).
//! - **217**: the open reply's `UiProtocolCapabilities` is evaluated through
//!   the web's coding gate (`coding-capabilities.ts:38-66`).
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::{Conversation, FlowEvent};

/// A fake WS server (the f26 pattern): answers `session/open` with a default
/// `opened` (merged with `open_tweaks`), answers `session/hydrate` with
/// `hydrate_result`, replies `{}` to anything else, and streams `stream` as
/// RAW notification frames (`{"method", "params"}`) after the open reply —
/// the same raw grammar the real server uses and the f26 fixture replays.
struct RawServer {
    base_url: String,
    received: Arc<Mutex<Vec<String>>>,
}

impl RawServer {
    async fn start(
        stream: Vec<(String, serde_json::Value)>,
        open_tweaks: serde_json::Value,
        hydrate_result: serde_json::Value,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(Mutex::new(Vec::new()));
        let rx = received.clone();

        tokio::spawn(async move {
            let Ok((sock, _)) = listener.accept().await else {
                return;
            };
            let Ok(ws) = tokio_tungstenite::accept_async(sock).await else {
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
                        let mut opened = serde_json::json!({
                            "session_id": session,
                            "active_profile_id": "dsflash",
                            "cursor": {"stream": "dsflash:main", "seq": 1},
                            "capabilities": {
                                "version": {"protocol": "octos-ui/v1alpha1",
                                            "schema_version": 1, "jsonrpc": "2.0"},
                                "capabilities_schema_version": 1,
                                "supported_methods": ["session/open", "session/hydrate",
                                                      "turn/start", "turn/interrupt",
                                                      "session/list",
                                                      "session/status/read"],
                                "supported_notifications": ["projection/envelope",
                                                            "protocol/replay_lossy",
                                                            "peer/staged", "peer/closed"],
                                "supported_features": ["approval.typed.v1",
                                                       "state.session_hydrate.v1",
                                                       "projection.envelope.v2",
                                                       "auxiliary.rest_to_ws.v1"]
                            }
                        });
                        if let (Some(tweaks), Some(map)) =
                            (open_tweaks.as_object(), opened.as_object_mut())
                        {
                            for (k, val) in tweaks {
                                map.insert(k.clone(), val.clone());
                            }
                        }
                        let frame = serde_json::json!(
                            {"jsonrpc": "2.0", "id": id, "result": {"opened": opened}});
                        let _ =
                            tx.lock().await.send(Message::Text(frame.to_string().into())).await;
                        // Stream the raw notification frames after the open.
                        let tx2 = tx.clone();
                        let frames = stream.clone();
                        tokio::spawn(async move {
                            for (m, params) in frames {
                                let frame =
                                    serde_json::json!({"jsonrpc": "2.0", "method": m, "params": params});
                                let _ = tx2
                                    .lock()
                                    .await
                                    .send(Message::Text(frame.to_string().into()))
                                    .await;
                                tokio::time::sleep(Duration::from_millis(60)).await;
                            }
                        });
                    }
                    "session/hydrate" => {
                        let frame =
                            serde_json::json!({"jsonrpc": "2.0", "id": id, "result": hydrate_result});
                        let _ =
                            tx.lock().await.send(Message::Text(frame.to_string().into())).await;
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
}

/// One `projection/envelope` frame body (the flattened wire DTO:
/// routing keys + bare envelope fields).
fn envelope(session: &str, thread: &str, seq: u64, turn: &str, text: &str) -> serde_json::Value {
    serde_json::json!({
        "session_id": session,
        "thread_id": thread,
        "seq": seq,
        "turn_id": turn,
        "payload": {"type": "user_message", "data": {"text": text}}
    })
}

/// One `peer/staged` frame body (`PeerStagedEvent`, ui_protocol.rs:6482).
fn peer_staged(session: &str, slug: &str) -> serde_json::Value {
    serde_json::json!({
        "session_id": session,
        "topic": format!("peer-{slug}"),
        "slug": slug,
        "brief": "b",
        "brief_path": format!("/tmp/p/{slug}/brief.md"),
        "cwd": format!("/tmp/p/{slug}"),
        "profile_id": "dsflash"
    })
}

/// A `protocol/replay_lossy` frame body (`ReplayLossyEvent`).
fn replay_lossy(session: &str, dropped: u64) -> serde_json::Value {
    serde_json::json!({
        "session_id": session,
        "dropped_count": dropped,
        "last_durable_cursor": {"stream": session, "seq": 9}
    })
}

/// The good hydrate reply: the SAME session, a cursor, one checkpoint.
fn good_hydrate() -> serde_json::Value {
    serde_json::json!({
        "session_id": "dsflash:main",
        "cursor": {"stream": "dsflash:main", "seq": 42},
        "projection_thread_sequences": {"thread-x": 7}
    })
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
    for _ in 0..500 {
        match tokio::time::timeout(Duration::from_millis(80), events.recv()).await {
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

// ---- 204: reject a resume open that returns a different workspace ----------

#[tokio::test]
async fn a_resume_open_that_returns_a_different_workspace_is_rejected() {
    let server = RawServer::start(
        vec![],
        serde_json::json!({"workspace_root": "/srv/other"}),
        good_hydrate(),
    )
    .await;
    let (conv, mut events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");

    // A RESUME that names its workspace (the saved-link open: the web's
    // `requireExactWorkspace` path). The send is fire-and-forget; the REPLY
    // carries the wrong workspace root.
    let id = conv
        .open_workspace_as("dsflash:main", Some("/srv/mine".to_owned()))
        .await
        .expect("session/open sent");
    assert_eq!(id, "dsflash:main");
    let saw = drain_until(&conv, &mut events, |_| false).await;

    // The reject is recorded fail-closed: requested vs returned.
    let rejects = conv.store.domains.session.workspace_rejects();
    assert!(
        rejects.contains(&(
            "dsflash:main".to_owned(),
            "/srv/mine".to_owned(),
            "/srv/other".to_owned()
        )),
        "the workspace mismatch must be recorded: {rejects:?}"
    );
    // The rejected open is not adopted: no WorkspaceOpened event, no root.
    assert!(
        !saw.iter().any(|s| s.contains("WorkspaceOpened")),
        "a rejected open must not emit WorkspaceOpened: {saw:?}"
    );
    assert_eq!(
        conv.store.domains.session.workspace_root("dsflash:main"),
        None,
        "the wrong workspace_root must not be adopted"
    );
    // And the mismatch surfaces in the event stream (no silent drop).
    assert!(
        saw.iter()
            .any(|s| s.contains("session/open-workspace-mismatch")),
        "the mismatch event must surface: {saw:?}"
    );
}

#[tokio::test]
async fn a_fresh_open_without_a_cwd_adopts_the_canonicalized_workspace() {
    // No cwd requested (fresh launch): the web's `requireExactWorkspace =
    // false` default — the server's root is adopted (it may canonicalize).
    let server = RawServer::start(
        vec![],
        serde_json::json!({"workspace_root": "/srv/canonical"}),
        good_hydrate(),
    )
    .await;
    let (conv, mut events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    conv.open_workspace(None).await.expect("session/open sent");
    let saw = drain_until(&conv, &mut events, |_| false).await;
    assert!(saw.iter().any(|s| s.contains("WorkspaceOpened")), "{saw:?}");
    assert_eq!(
        conv.store.domains.session.workspace_root("dsflash:main").as_deref(),
        Some("/srv/canonical"),
        "a cwd-less open adopts the server's root"
    );
    assert!(conv.store.domains.session.workspace_rejects().is_empty());
    // 217: with the default (complete) advertisement the coding gate is open.
    assert!(
        conv.store.domains.config.coding_gate().is_empty(),
        "the full advertisement must pass the coding gate"
    );
    assert!(
        conv.store
            .domains
            .config
            .supported_methods()
            .contains(&"turn/start".to_owned()),
        "the open reply's methods are recorded"
    );
}

// ---- 217: the coding gate lists exactly what the advertisement missed ------

#[tokio::test]
async fn the_coding_gate_lists_the_missing_methods_and_features() {
    // Mirrors coding-capabilities.ts: session/open + session/hydrate +
    // turn/start and state.session_hydrate.v1 + projection.envelope.v2 are
    // the requirements. This advertisement misses hydrate + turn/start and
    // the projection feature.
    let caps = serde_json::json!({"capabilities": {
        "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
        "capabilities_schema_version": 1,
        "supported_methods": ["session/open", "session/list"],
        "supported_notifications": ["projection/envelope"],
        "supported_features": ["approval.typed.v1", "state.session_hydrate.v1"]
    }});
    let server = RawServer::start(vec![], caps, good_hydrate()).await;
    let (conv, mut events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    conv.open_workspace(None).await.expect("session/open sent");
    drain_until(&conv, &mut events, |_| false).await;

    assert_eq!(
        conv.store.domains.config.coding_gate(),
        vec![
            "session/hydrate".to_owned(),
            "turn/start".to_owned(),
            "projection.envelope.v2".to_owned(),
        ],
        "the gate lists exactly the unadvertised requirements (methods first, then features)"
    );
}

// ---- 206: hydrate commit verifies the id, adopts cursor + checkpoints ------

#[tokio::test]
async fn a_hydrate_commit_adopts_the_cursor_and_checkpoints_and_clears_lossy() {
    let server = RawServer::start(
        vec![
            ("protocol/replay_lossy".to_owned(), replay_lossy("dsflash:main", 3)),
        ],
        serde_json::json!({}),
        good_hydrate(),
    )
    .await;
    let (conv, mut events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    conv.open_workspace(None).await.expect("session/open");

    // Drain until the hydrate reply has been committed (lossy cleared, the
    // checkpoint folded, the cursor adopted).
    let saw = drain_until(&conv, &mut events, |_| {
        let rec = conv.store.domains.config.recovery("dsflash:main");
        !rec.resync_pending
            && rec.phase == octoscode_store::domains::config::LossyPhase::Healthy
            && conv.store.domains.turn.last_envelope_seq("thread-x") == Some(7)
            && conv.store.domains.turn.envelope_cursor()
                == Some(("dsflash:main".to_owned(), 42))
    })
    .await;
    let joined = saw.join(" | ");

    // The lossy event really produced a `session/hydrate` (the resync)…
    let received = server.received.lock().unwrap().clone();
    assert!(
        received.contains(&"session/hydrate".to_owned()),
        "the lossy event must produce a session/hydrate; sent {received:?}"
    );
    // …the continuation checkpoint landed…
    assert_eq!(
        conv.store.domains.turn.last_envelope_seq("thread-x"),
        Some(7),
        "projection_thread_sequences folded: {joined}"
    );
    // …and the hydrate's cursor was ADOPTED (the row-206 commit; max-wins
    // with the fold cursor, which never advanced past the open reply here).
    assert_eq!(
        conv.store.domains.turn.envelope_cursor(),
        Some(("dsflash:main".to_owned(), 42)),
        "commitHydrate adopts the returned cursor: {joined}"
    );
    let rec = conv.store.domains.config.recovery("dsflash:main");
    assert_eq!(rec.phase, octoscode_store::domains::config::LossyPhase::Healthy);
    assert!(!rec.resync_pending, "the resync was consumed");
    assert_eq!(
        conv.store.domains.config.replay_loss("dsflash:main").unwrap().dropped_count,
        3,
        "the lossy observation is still recorded"
    );
}

#[tokio::test]
async fn a_hydrate_that_names_a_different_session_fails_closed() {
    // The routing fault the web calls HydrateSessionMismatchError
    // ("inherently fatal", active-session-runtime.ts:371-380): the reply
    // carries another session's id. Native: fold NOTHING, adopt NO cursor,
    // keep the lossy phase + the owed resync, surface the mismatch.
    let mut bad = good_hydrate();
    bad["session_id"] = serde_json::json!("other:main");
    let server = RawServer::start(
        vec![
            ("protocol/replay_lossy".to_owned(), replay_lossy("dsflash:main", 3)),
        ],
        serde_json::json!({}),
        bad,
    )
    .await;
    let (conv, mut events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    conv.open_workspace(None).await.expect("session/open");

    let received = server.received.clone();
    let saw = drain_until(&conv, &mut events, |e| {
        matches!(e, FlowEvent::Other(s) if s == "session/hydrate-mismatch")
    })
    .await;
    assert!(
        saw.iter().any(|s| s.contains("session/hydrate-mismatch")),
        "the mismatch must surface: {saw:?}"
    );
    assert!(
        received.lock().unwrap().contains(&"session/hydrate".to_owned()),
        "the resync did go out"
    );
    // Fail closed: nothing committed.
    assert_eq!(
        conv.store.domains.turn.last_envelope_seq("thread-x"),
        None,
        "a mismatched hydrate folds no checkpoint"
    );
    assert_eq!(
        conv.store.domains.turn.envelope_cursor(),
        None,
        "a mismatched hydrate adopts no cursor"
    );
    let rec = conv.store.domains.config.recovery("dsflash:main");
    assert_eq!(
        rec.phase,
        octoscode_store::domains::config::LossyPhase::Lossy,
        "the lossy phase stays until a GOOD hydrate"
    );
    // The resync flag was consumed by the (failed) attempt — the web's
    // recover is one-shot (`active-session-runtime.ts:1239-1260`); an auto
    // retry would loop on a server that keeps answering the wrong id (the
    // "inherently fatal" note, :373-376). The session stays visibly Lossy.
    assert!(
        !rec.resync_pending,
        "the resync was issued once; the phase, not the flag, carries the failure"
    );
}

// ---- 205: durable projection frames route by session scope -----------------

#[tokio::test]
async fn durable_projection_frames_route_by_session_scope() {
    let server = RawServer::start(
        vec![
            // FOREIGN: another session's envelope + another session's lossy.
            (
                "projection/envelope".to_owned(),
                envelope("other:main", "th-foreign", 5, "tf", "FOREIGN"),
            ),
            (
                "protocol/replay_lossy".to_owned(),
                replay_lossy("other:main", 9),
            ),
            // IN-SCOPE: this runtime's own envelope must still fold.
            (
                "projection/envelope".to_owned(),
                envelope("dsflash:main", "th1", 5, "t1", "why blue?"),
            ),
        ],
        serde_json::json!({}),
        good_hydrate(),
    )
    .await;
    let (conv, mut events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    let saw = drain_until(&conv, &mut events, |_| false).await;

    // The foreign frames were dropped by name (no silent drop)…
    assert!(
        saw.iter()
            .any(|s| s.contains("wrong-session projection/envelope")),
        "the foreign envelope must surface as wrong-session: {saw:?}"
    );
    assert!(
        saw.iter()
            .any(|s| s.contains("wrong-session protocol/replay_lossy")),
        "the foreign lossy must surface as wrong-session: {saw:?}"
    );
    // …folded NOTHING for the foreign session…
    assert!(
        conv.store.domains.session.timeline.entries("other:main").is_empty(),
        "a foreign envelope must not fold"
    );
    assert!(
        conv.store.domains.config.replay_loss("other:main").is_none(),
        "a foreign lossy must not fold"
    );
    // …did not trigger a resync (a foreign loss is not ours)…
    // A15 — the open's own history read is the ONE hydrate (every open
    // hydrates, the web's open -> hydrate); a foreign loss adds none.
    let received = server.received.lock().unwrap().clone();
    assert_eq!(
        received.iter().filter(|m| *m == "session/hydrate").count(),
        1,
        "a foreign replay_lossy must not produce a resync; sent {received:?}"
    );
    // …and the in-scope envelope still folded normally.
    let users = conv
        .store
        .domains
        .session
        .timeline
        .of_kind("dsflash:main", octoscode_store::EntryKind::USER_MESSAGE);
    assert!(
        users.iter().any(|e| e.text == "why blue?"),
        "the in-scope envelope folds: {users:?}"
    );
    assert_eq!(conv.store.domains.turn.last_envelope_seq("th1"), Some(5));
}

// ---- 213: peer lifecycle events route by the full SessionKey ---------------

#[tokio::test]
async fn peer_lifecycle_events_route_only_by_the_full_originating_session_key() {
    let server = RawServer::start(
        vec![
            // FOREIGN stage: a different originating session — payload must
            // not matter (slug/topic are never the routing key).
            ("peer/staged".to_owned(), peer_staged("other:main", "edison")),
            // IN-SCOPE stage + close.
            ("peer/staged".to_owned(), peer_staged("dsflash:main", "ada")),
            (
                "peer/closed".to_owned(),
                serde_json::json!({
                    "session_id": "dsflash:main",
                    "topic": "peer-ada",
                    "slug": "ada",
                    "profile_id": "dsflash"
                }),
            ),
        ],
        serde_json::json!({}),
        good_hydrate(),
    )
    .await;
    let (conv, mut events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    let saw = drain_until(&conv, &mut events, |_| false).await;

    // The foreign stage never reached the roster (logged by name instead).
    assert!(
        saw.iter().any(|s| s.contains("wrong-session peer/staged")),
        "the foreign peer/staged must surface as wrong-session: {saw:?}"
    );
    assert!(
        conv.store.domains.peer.get("edison").is_none(),
        "a peer staged by ANOTHER session must not enter this runtime's roster"
    );
    // The in-scope pair works exactly as before the gate: stage then close.
    let ada = conv.store.domains.peer.get("ada").expect("the in-scope peer folds");
    assert!(
        ada.closed,
        "the in-scope peer/closed still tears the row down: {ada:?}"
    );
    assert_eq!(
        ada.origin_session_id.as_deref(),
        Some("dsflash:main"),
        "the row keeps its full originating SessionKey"
    );
}
