//! P4f1 — the production-path replay tests for the history mutations, mirroring
//! fp4a3's replay server: a fake WS server serves the RECORDED real-gate frames
//! (r3-session for `snapshot/list`+`snapshot/restore`+`session/rollback`+
//! `session/fork`, r43a-recovery for the canonical `session/hydrate`), the
//! screen's functions run through the production `Conversation` client, and the
//! server's received-method log proves the wire traffic.
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use octoscode_module::flow::Conversation;
use octoscode_module::screens::history::{
    self, ConversationCheckpoint, HistoryMode,
};
use octoscode_store::Store;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

struct Frame {
    dir: String,
    method: String,
    body: Value,
}

fn load(path: &str) -> Vec<Frame> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
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

/// An r3 IN reply: the recording labels a successful reply `res:<method>`
/// (a failure is `error:<method>` / `err:<method>`). The r43a recording uses
/// bare method names, so it goes through `recorded`.
fn recorded_res(path: &str, method: &str) -> Value {
    load(path)
        .iter()
        .find(|f| f.dir == "in" && f.method == format!("res:{method}"))
        .map(|f| f.body.clone())
        .unwrap_or_else(|| panic!("res:{method} is in the recording {path}"))
}

fn recorded(path: &str, method: &str) -> Value {
    load(path)
        .iter()
        .find(|f| f.dir == "in" && f.method == method)
        .map(|f| f.body.clone())
        .unwrap_or_else(|| panic!("{method} is in the recording {path}"))
}

/// fp4a3's replay server: canned replies per method, `{}` for the rest, and a
/// log of every method the (production) client sent. `errs` serves a JSON-RPC
/// ERROR instead, so the error paths can be replayed too (the r3 recording has
/// a real `snapshot/restore` error frame).
struct ReplayServer {
    base_url: String,
    received: Arc<Mutex<Vec<(String, Value)>>>,
}

impl ReplayServer {
    async fn start(canned: Vec<(String, Value)>, errs: Vec<(String, Value)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(Mutex::new(Vec::new()));
        let rx = received.clone();
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (tx, mut rx_in) = ws.split();
            let tx = Arc::new(tokio::sync::Mutex::new(tx));
            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                let params = v.get("params").cloned().unwrap_or(Value::Null);
                rx.lock().unwrap().push((method.clone(), params));
                let frame = if let Some((_, e)) = errs.iter().find(|(m, _)| *m == method) {
                    json!({"jsonrpc": "2.0", "id": id, "error": e})
                } else {
                    let body = canned
                        .iter()
                        .find(|(m, _)| *m == method)
                        .map(|(_, b)| b.clone())
                        .unwrap_or(json!({}));
                    json!({"jsonrpc": "2.0", "id": id, "result": body})
                };
                let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
            }
        });
        Self { base_url: format!("http://{addr}"), received }
    }

    fn saw(&self, method: &str) -> bool {
        self.received.lock().unwrap().iter().any(|(m, _)| m == method)
    }

    /// The params the production client actually sent for `method`.
    fn params_of(&self, method: &str) -> Value {
        self.received
            .lock()
            .unwrap()
            .iter()
            .find(|(m, _)| m == method)
            .map(|(_, p)| p.clone())
            .unwrap_or(Value::Null)
    }

    fn count(&self, method: &str) -> usize {
        self.received.lock().unwrap().iter().filter(|(m, _)| m == method).count()
    }
}

const R3: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../octoscode-client/tests/fixtures/r3-session-a6ea8505.jsonl"
);
const R43A: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../octoscode-client/tests/fixtures/r43a-recovery-a6ea8505.jsonl"
);

/// The recorded r3 session id the history frames were captured against.
const R3_SESSION: &str = "dsflash:api:main";

/// Advertise the methods the given modes need (the gate reads the store's
/// `supported_methods`, `history-binding.ts:53-67`).
fn advertised(store: &Store, modes: &[&str]) {
    let mut all = vec!["session/hydrate".to_owned()];
    for m in modes {
        match *m {
            "undo" => {
                all.push("snapshot/list".into());
                all.push("snapshot/restore".into());
            }
            "rewind" => all.push("session/rollback".into()),
            "fork" => {
                all.push("session/fork".into());
                all.push("session/open".into());
            }
            other => panic!("unknown mode {other}"),
        }
    }
    store.domains.config.set_supported_methods(all);
}

async fn connect(server: &ReplayServer) -> (Conversation, std::sync::Arc<Store>) {
    let (conv, mut _events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    let store = conv.store.clone();
    (conv, store)
}

/// A checkpoint built from the recorded hydrate, for the rewind selection.
fn checkpoint_from(thread: &Value) -> Vec<ConversationCheckpoint> {
    let messages = thread["messages"].as_array().cloned().unwrap_or_default();
    history::conversation_checkpoints(&messages)
}

// ---------------------------------------------------------------- rewind

/// Rewind: the recorded `session/rollback` + the canonical `session/hydrate`
/// reply, driven through the production client. The picker is built from the
/// RECORDED history, the identity is recomputed against a FRESH read of the
/// same canonical history, and the rollback carries the recomputed `num_turns`.
#[tokio::test]
async fn rewind_recomputes_identity_against_the_recorded_hydrate_then_rolls_back() {
    let hydrate = recorded(R43A, "session/hydrate");
    let session = hydrate["session_id"].as_str().expect("recorded session id").to_owned();
    // The recorded `session/rollback` frame was captured against the r3 session;
    // only its reply identity is rebound to the session under test, so the
    // recorded request/reply shape is otherwise replayed verbatim.
    let mut rollback = recorded_res(R3, "session/rollback");
    rollback["thread"]["session_id"] = json!(session);

    let server = ReplayServer::start(
        vec![
            ("session/hydrate".to_owned(), hydrate.clone()),
            ("session/rollback".to_owned(), rollback.clone()),
        ],
        vec![],
    )
    .await;
    let (conv, store) = connect(&server).await;
    advertised(&store, &["rewind"]);

    // The picker: checkpoints from the recorded canonical history.
    let picker = history::load_history(conv.client(), &store, &session, HistoryMode::Rewind)
        .await
        .expect("picker loads from the canonical hydrate");
    assert!(!picker.is_empty(), "the recorded history has user-rooted turns");
    assert!(server.saw("session/hydrate"), "the picker read canonical history");
    // The hydrate carried the canonical include list the web sends.
    let sent = server.params_of("session/hydrate");
    assert_eq!(sent["session_id"], session);
    let include = sent["include"].as_array().expect("include list").clone();
    for want in ["messages", "turns", "pending_approvals"] {
        assert!(
            include.iter().any(|v| v == want),
            "session/hydrate must include {want}, got {include:?}"
        );
    }
    // Newest first, and distinct thread_id groups (the recorded fixture has
    // several).
    assert!(
        picker.windows(2).all(|w| w[0].checkpoint > w[1].checkpoint),
        "the picker lists newest first: {:?}",
        picker.iter().map(|c| c.checkpoint).collect::<Vec<_>>()
    );
    let user_threads = {
        let mut ids: Vec<&str> = hydrate["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["role"] == "user")
            .filter_map(|m| m["thread_id"].as_str())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids.len()
    };
    assert_eq!(picker.len(), user_threads, "one checkpoint per user-rooted thread");

    // Pick the OLDEST checkpoint -> the maximum num_turns.
    let oldest = picker.last().expect("oldest checkpoint").clone();
    let out = history::rewind_conversation(conv.client(), &store, &session, &oldest)
        .await
        .expect("rewind through the production client");
    assert!(server.saw("session/rollback"), "the wire carried the rollback");

    // The rollback carried the RECOMPUTED num_turns (users.len() - index), not
    // the checkpoint's displayed ordinal.
    let sent = server.params_of("session/rollback");
    assert_eq!(sent["session_id"], session);
    assert_eq!(
        sent["num_turns"].as_u64(),
        Some(user_threads as u64),
        "num_turns is the drop count from the end, not the ordinal"
    );

    // It ends in the canonical rehydration of the OWNING record, and the
    // per-mode notice states exactly what it did NOT change.
    // Three canonical reads: the picker, the fresh pre-rollback read that
    // recomputes the identity, and the rehydration of the owning record.
    assert_eq!(server.count("session/hydrate"), 3, "picker + fresh + rehydration");
    assert_eq!(out.notice, HistoryMode::Rewind.completed_notice());
    assert_eq!(out.notice, "Conversation rewound. Workspace files were not restored.");
    assert!(out.prefill.is_some(), "rewind hands the prefill back for the empty draft");
    assert!(out.forked_session_id.is_none());
    assert_eq!(out.thread["session_id"], session, "the rehydration is the bound session");
}

/// A stale target is refused against fresh history and the rollback NEVER
/// reaches the wire (the web's `resolveCheckpoint` → `null` arm).
#[tokio::test]
async fn a_stale_checkpoint_refuses_before_the_rollback_touches_the_wire() {
    let hydrate = recorded(R43A, "session/hydrate");
    let session = hydrate["session_id"].as_str().expect("recorded session id").to_owned();

    let server = ReplayServer::start(
        vec![
            ("session/hydrate".to_owned(), hydrate.clone()),
            ("session/rollback".to_owned(), recorded_res(R3, "session/rollback")),
        ],
        vec![],
    )
    .await;
    let (conv, store) = connect(&server).await;
    advertised(&store, &["rewind"]);

    // A selection whose key/prefill are NOT in the recorded history — e.g. the
    // dialog kept a selection across a history replacement.
    let stale = ConversationCheckpoint {
        key: json!(["gone-thread", "gone-message", 999, ""]).to_string(),
        checkpoint: 1,
        preview: "gone".into(),
        prefill: "gone".into(),
        media_count: 0,
        user_message_count: 1,
    };
    let err = history::rewind_conversation(conv.client(), &store, &session, &stale)
        .await
        .expect_err("a stale target is refused");
    assert_eq!(err, "History changed. Reload the checkpoint picker.");
    assert!(server.saw("session/hydrate"), "fresh history was read");
    assert!(!server.saw("session/rollback"), "the mutation never left the client");
}

/// The active-Session refusal: the fresh history has an `active` turn, so the
/// rollback is refused (history-coordinator.ts:178).
#[tokio::test]
async fn a_session_that_became_active_refuses_the_rewind() {
    let mut hydrate = recorded(R43A, "session/hydrate");
    let session = hydrate["session_id"].as_str().expect("recorded session id").to_owned();
    // The typed result parses `turn_id` as a UUID, so reuse a recorded one
    // rather than inventing a malformed id.
    hydrate["turns"] = json!([{
        "state": "active",
        "turn_id": "01a0f814-5d33-70d5-8121-1da0934df3c5",
    }]);

    let server = ReplayServer::start(
        vec![
            ("session/hydrate".to_owned(), hydrate.clone()),
            ("session/rollback".to_owned(), recorded_res(R3, "session/rollback")),
        ],
        vec![],
    )
    .await;
    let (conv, store) = connect(&server).await;
    advertised(&store, &["rewind"]);
    let picker = checkpoint_from(&hydrate);
    let err = history::rewind_conversation(conv.client(), &store, &session, &picker[0])
        .await
        .expect_err("an active Session refuses the rewind");
    assert_eq!(err, "The Session became active. Wait before rewinding.");
    assert!(!server.saw("session/rollback"), "no rollback on the wire");
}

// ------------------------------------------------------------------ fork

/// Fork: the recorded `session/fork` reply, driven through the production
/// client. The response must name THIS parent and a DIFFERENT child, and the
/// exact child id is returned for the background open.
#[tokio::test]
async fn fork_sends_the_recorded_shape_and_returns_the_exact_child() {
    let hydrate = recorded(R43A, "session/hydrate");
    let session = hydrate["session_id"].as_str().expect("recorded session id").to_owned();
    let fork = recorded_res(R3, "session/fork");
    // The recorded fork's parent is the r3 session; use it as our bound session
    // so the identity check is the recorded one.
    let parent = fork["parent_session_id"].as_str().expect("recorded parent").to_owned();
    let child = fork["new_session_id"].as_str().expect("recorded child").to_owned();

    let server = ReplayServer::start(
        vec![("session/fork".to_owned(), fork.clone())],
        vec![],
    )
    .await;
    let (conv, _store) = connect(&server).await;

    // The recorded OUT frame's new_chat_id ("r3child") is the request the
    // production path must reproduce.
    let out_frame = load(R3)
        .into_iter()
        .find(|f| f.dir == "out" && f.method == "session/fork")
        .map(|f| f.body)
        .expect("the recording has a session/fork OUT frame");
    let recorded_chat = out_frame["new_chat_id"].as_str().expect("recorded new_chat_id").to_owned();

    let out = history::fork_conversation(conv.client(), &parent, &recorded_chat, None)
        .await
        .expect("fork through the production client");
    assert!(server.saw("session/fork"), "the wire carried the fork");
    assert_eq!(out.forked_session_id, child, "the exact child, opened in the background");
    assert_eq!(out.notice, HistoryMode::Fork.completed_notice());
    assert_eq!(
        out.notice,
        "Conversation fork opened in the background. Your selection was not changed."
    );
    // The request reproduced the recorded shape, and `copy_messages` is ABSENT
    // when not requested (the web omits it, `history.ts:245-249`).
    let sent = server.params_of("session/fork");
    assert_eq!(sent["session_id"], parent);
    assert_eq!(sent["new_chat_id"], recorded_chat);
    assert!(sent.get("copy_messages").is_none(), "copy_messages is omitted, not null");

    // A fork response naming ANOTHER parent is refused.
    let mut foreign = fork.clone();
    foreign["parent_session_id"] = json!("other:parent");
    let server = ReplayServer::start(vec![("session/fork".to_owned(), foreign)], vec![]).await;
    let (conv, _store) = connect(&server).await;
    let err = history::fork_conversation(conv.client(), &session, &recorded_chat, None)
        .await
        .expect_err("a foreign parent is refused");
    assert_eq!(err, "The fork response belongs to another Session.");

    // An unusable name never reaches the wire.
    let server = ReplayServer::start(vec![("session/fork".to_owned(), fork)], vec![]).await;
    let (conv, _store) = connect(&server).await;
    let err = history::fork_conversation(conv.client(), &parent, "a/b", None)
        .await
        .expect_err("an invalid name is refused");
    assert_eq!(err, "Choose a valid conversation name.");
    assert!(!server.saw("session/fork"), "the invalid name never left the client");
}

// ------------------------------------------------------------------ undo

/// Undo: the recorded `snapshot/list` + the recorded `snapshot/restore`
/// success path, then the canonical rehydration. The restore response must
/// name EXACTLY the requested snapshot.
#[tokio::test]
async fn undo_lists_then_restores_then_rehydrates_the_owning_record() {
    // This mode's recorded frames are the r3 ones, so the session under test IS
    // the recorded r3 session: the list/restore replies are then replayed
    // verbatim. Only the r43a hydrate's identity is rebound to it.
    let session = R3_SESSION.to_owned();
    let mut hydrate = recorded(R43A, "session/hydrate");
    hydrate["session_id"] = json!(session);

    // The recorded list says snapshots are DISABLED and empty; give the picker
    // one real target so the restore arm is reachable, keeping the recorded
    // fields the store folds.
    let mut list = recorded_res(R3, "snapshot/list");
    list["enabled"] = json!(true);
    list["available"] = json!(true);
    list["snapshots"] = json!([{"id": "r3-snap-1", "label": "before edits", "timestamp_unix": 1}]);
    let restore = json!({
        "session_id": session,
        "restored": "r3-snap-1",
        "snapshots": [{"id": "r3-snap-2", "label": "after restore", "timestamp_unix": 2}],
    });

    let server = ReplayServer::start(
        vec![
            ("snapshot/list".to_owned(), list.clone()),
            ("snapshot/restore".to_owned(), restore.clone()),
            ("session/hydrate".to_owned(), hydrate.clone()),
        ],
        vec![],
    )
    .await;
    let (conv, store) = connect(&server).await;
    advertised(&store, &["undo"]);

    // Load arm: the list folds into the store's snapshot projection.
    history::load_history(conv.client(), &store, &session, HistoryMode::Undo)
        .await
        .expect("the picker loads");
    assert!(server.saw("snapshot/list"));
    assert_eq!(store.domains.config.snapshots().snapshots.len(), 1);

    let out = history::undo_workspace_changes(conv.client(), &store, &session, "r3-snap-1")
        .await
        .expect("undo through the production client");
    // The wire order is the web's: list -> restore -> canonical rehydration.
    assert!(server.saw("snapshot/restore"), "the wire carried the restore");
    assert_eq!(server.count("snapshot/list"), 2, "freshness re-check before the restore");
    assert_eq!(server.count("session/hydrate"), 1, "the owning record is rehydrated");
    let sent = server.params_of("snapshot/restore");
    assert_eq!(sent["session_id"], session);
    assert_eq!(sent["snapshot_id"], "r3-snap-1");
    // The restore's returned list replaced the folded projection.
    assert_eq!(store.domains.config.snapshots().snapshots[0].id, "r3-snap-2");
    assert_eq!(out.notice, HistoryMode::Undo.completed_notice());
    assert_eq!(
        out.notice,
        "Workspace snapshot restored. Conversation history was not changed."
    );
    assert!(out.prefill.is_none(), "file undo never prefills the composer");
    assert!(out.forked_session_id.is_none());

    // The freshness re-check refuses a target the FRESH list no longer offers,
    // BEFORE the restore reaches the wire.
    let server = ReplayServer::start(
        vec![
            ("snapshot/list".to_owned(), list.clone()),
            ("snapshot/restore".to_owned(), restore.clone()),
        ],
        vec![],
    )
    .await;
    let (conv, store) = connect(&server).await;
    advertised(&store, &["undo"]);
    let err = history::undo_workspace_changes(conv.client(), &store, &session, "r3-vanished")
        .await
        .expect_err("a vanished target is refused");
    assert_eq!(err, "The selected snapshot is no longer available. Reload history.");
    assert!(!server.saw("snapshot/restore"), "the stale target never reached the wire");

    // The RECORDED error frame: restore of a snapshot that does not exist
    // (-32602 "no snapshots have been taken for this workspace") surfaces as an
    // error, never as a completed undo.
    let recorded_error = load(R3)
        .into_iter()
        .find(|f| f.dir == "in" && f.method == "error:snapshot/restore")
        .map(|f| f.body)
        .expect("the recording has a snapshot/restore error frame");
    let server = ReplayServer::start(
        vec![(
            "snapshot/list".to_owned(),
            json!({"session_id": session, "enabled": true, "available": true,
                   "snapshots": [{"id": "r3-does-not-exist", "label": "", "timestamp_unix": 0}]}),
        )],
        vec![("snapshot/restore".to_owned(), recorded_error.clone())],
    )
    .await;
    let (conv, store) = connect(&server).await;
    advertised(&store, &["undo"]);
    let err = history::undo_workspace_changes(conv.client(), &store, &session, "r3-does-not-exist")
        .await
        .expect_err("the recorded restore error surfaces");
    assert!(err.contains("snapshot/restore"), "the transport error is surfaced: {err}");
    assert!(
        err.contains("no snapshots have been taken"),
        "the server's own reason survives: {err}"
    );
}

// ------------------------------------------------------- the blocked gate

/// The blocked-reason gate: an unadvertised method refuses every mode BEFORE
/// the wire, and the gate reads the store's advertised methods.
#[tokio::test]
async fn the_capability_gate_refuses_every_mode_before_the_wire() {
    let hydrate = recorded(R43A, "session/hydrate");
    let session = hydrate["session_id"].as_str().expect("recorded session id").to_owned();
    let server = ReplayServer::start(
        vec![
            ("session/hydrate".to_owned(), hydrate.clone()),
            ("session/rollback".to_owned(), recorded_res(R3, "session/rollback")),
            ("snapshot/list".to_owned(), recorded_res(R3, "snapshot/list")),
            ("session/fork".to_owned(), recorded_res(R3, "session/fork")),
        ],
        vec![],
    )
    .await;
    let (conv, store) = connect(&server).await;

    // Nothing advertised -> every mode is blocked with the web's verbatim copy.
    for mode in [HistoryMode::Undo, HistoryMode::Rewind, HistoryMode::Fork] {
        let err = history::blocked_reason(&store, &session, mode)
            .expect("an unadvertised server blocks history");
        assert_eq!(err, "This server does not advertise the required history methods.");
    }

    // Advertised, but the server has a live turn -> the settle copy.
    advertised(&store, &["rewind"]);
    assert!(history::blocked_reason(&store, &session, HistoryMode::Rewind).is_none());
    store.domains.turn.started("turn-live");
    let err = history::blocked_reason(&store, &session, HistoryMode::Rewind)
        .expect("a live turn blocks the mutation");
    assert_eq!(
        err,
        "Wait for affected turns, queued prompts, and questions to settle before changing history."
    );
    store.domains.turn.ended("turn-live");
    assert!(
        history::blocked_reason(&store, &session, HistoryMode::Rewind).is_none(),
        "once the turn settles the gate clears"
    );

    // A standing question blocks the bound record.
    advertised(&store, &["fork"]);
    store.domains.approval.set_question(octoscode_store::domains::approval::PendingQuestion {
        question_id: "q1".into(),
        session_id: session.clone(),
        turn_id: "t1".into(),
        title: "Allow?".into(),
        body: "Allow?".into(),
        questions: json!([]),
    });
    assert!(history::blocked_reason(&store, &session, HistoryMode::Fork).is_some());
    store.domains.approval.clear_question();
    assert!(history::blocked_reason(&store, &session, HistoryMode::Fork).is_none());

    // The gate refuses the ACTION surface too, so the production path cannot
    // mutate while work is unsettled. With a live turn the fork never leaves.
    advertised(&store, &["fork"]);
    store.domains.turn.started("turn-live");
    let err = history::perform(&conv, "history.fork_conversation", &store, Some("r3child"))
        .await
        .expect_err("the production action honours the gate");
    assert_eq!(
        err,
        "Wait for affected turns, queued prompts, and questions to settle before changing history."
    );
    assert!(!server.saw("session/fork"), "the blocked action never touched the wire");
}
