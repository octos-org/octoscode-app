//! A7 — the history dialog's undo and fork modes at the wire (production
//! path): the composer's `/undo` and `/fork` (`board3::host::command`, which
//! `flow.rs::submit_draft` calls), the dialog's taps (`board3::host::perform`
//! — the same shared tap path the mounted controls publish) and the jobs the
//! host spawns (`board3::host::run`), against a scripted fake server.
//!
//! Ports: `history-coordinator.test.ts:435` "opens the server's exact fork
//! identity in the background without a kickoff" (no `session/open`, no
//! `turn/start`, the selection unchanged), the fork-name validation
//! (`packages/client/src/history.ts:54-61`), and the undo order of
//! `HistoryDialog` + `history-coordinator.ts:161-186` (fresh list, restore,
//! the owner rehydrated).
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::{Conversation, FlowEvent};
use octoscode_module::screens::board3::host::{self, Job, Outcome};

const PROFILE: &str = "a7";

struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
}

fn result_for(method: &str, p: &Value) -> Value {
    let session = p["session_id"].as_str().unwrap_or("a7:main");
    match method {
        "session/open" => json!({"opened": {
            "session_id": session, "active_profile_id": PROFILE,
            "workspace_root": "/home/user/src/octos",
            "cursor": {"stream": session, "seq": 1},
            "capabilities": {
                "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                "capabilities_schema_version": 1,
                "supported_methods": ["session/open", "session/list", "session/hydrate", "session/fork",
                    "session/rollback", "snapshot/list", "snapshot/restore", "turn/start"],
                "supported_notifications": [], "supported_features": []
            }
        }}),
        "session/list" => json!({"sessions": [
            {"id": "a7:main", "title": "Fix steer queue drop", "message_count": 2, "updated_at": "2026-10-01T10:00:00Z"},
            {"id": "a7:steer-v2", "title": "Fork · steer-v2", "message_count": 2, "updated_at": "2026-10-01T10:01:00Z"}
        ]}),
        "session/hydrate" => json!({
            "session_id": session, "cursor": {"stream": session, "seq": 4},
            "messages": [
                {"seq": 1, "role": "user", "content": "Fix the steer queue", "thread_id": "t1", "turn_id": "01920000-0000-7000-8000-000000000001", "persisted_at": "2026-10-01T10:00:00Z"},
                {"seq": 2, "role": "assistant", "content": "Done.", "thread_id": "t1", "turn_id": "01920000-0000-7000-8000-000000000001", "persisted_at": "2026-10-01T10:00:01Z"}
            ],
            "turns": []
        }),
        "session/fork" => json!({
            "new_session_id": format!("{PROFILE}:{}", p["new_chat_id"].as_str().unwrap_or("x")),
            "parent_session_id": session, "copied_messages": 2
        }),
        "snapshot/list" => json!({"session_id": session, "enabled": true, "available": true, "snapshots": [
            {"id": "snap-2", "label": "Before refactor", "timestamp_unix": 1_790_000_000},
            {"id": "snap-1", "label": "Session start", "timestamp_unix": 1_789_990_000}
        ]}),
        "snapshot/restore" => json!({"session_id": session, "restored": p["snapshot_id"], "snapshots": [
            {"id": "snap-2", "label": "Before refactor", "timestamp_unix": 1_790_000_000},
            {"id": "snap-1", "label": "Session start", "timestamp_unix": 1_789_990_000}
        ]}),
        _ => json!({}),
    }
}

impl Server {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let s2 = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let seen = s2.clone();
                tokio::spawn(async move {
                    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
                    let (mut tx, mut rx) = ws.split();
                    while let Some(Ok(msg)) = rx.next().await {
                        let Message::Text(text) = msg else { continue };
                        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                        if v.get("id").is_none() {
                            continue;
                        }
                        let method = v["method"].as_str().unwrap_or("").to_owned();
                        seen.lock().unwrap().push((method.clone(), v["params"].clone()));
                        let frame = json!({"jsonrpc": "2.0", "id": v["id"], "result": result_for(&method, &v["params"])});
                        let _ = tx.send(Message::Text(frame.to_string().into())).await;
                    }
                });
            }
        });
        Self { base_url, seen }
    }

    fn methods(&self) -> Vec<String> {
        self.seen.lock().unwrap().iter().map(|(m, _)| m.clone()).collect()
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }
}

async fn connected(server: &Server) -> Conversation {
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", PROFILE, None, None).expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    let mut opened = false;
    for _ in 0..80 {
        if opened && conv.store.is_live() {
            break;
        }
        match tokio::time::timeout(Duration::from_millis(200), events.recv()).await {
            Ok(Some(evt)) => {
                if matches!(conv.on_event(evt), FlowEvent::WorkspaceOpened(_)) {
                    opened = true;
                }
            }
            _ => break,
        }
    }
    conv
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    host::reset();
    g
}

fn spawned(o: Option<Outcome>) -> Job {
    match o {
        Some(Outcome::Spawn(job)) => job,
        other => panic!("expected a job, got {other:?}"),
    }
}

#[tokio::test]
async fn fork_names_the_branch_then_opens_the_exact_child_in_the_background() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    let parent = conv.session_id();

    // `/fork` from the composer opens the dialog in its fork mode.
    let job = spawned(host::command("fork", "", &conv));
    host::run(job, &conv).await.expect("fork mode loads");
    {
        let st = host::state();
        assert_eq!(st.ck.mode, octoscode_module::screens::history::HistoryMode::Fork);
        assert!(!st.ck.loading);
        assert_eq!(st.ck.blocked, None, "session/fork + session/open + session/hydrate advertised");
    }
    // An invalid name never leaves the client.
    host::input_changed("ck.fork", "a:b");
    assert_eq!(host::perform("b3.ck.fork", 0, &conv.store), Outcome::Done);
    assert!(server.params_of("session/fork").is_empty());

    host::input_changed("ck.fork", "steer-v2");
    let job = match host::perform("b3.ck.fork", 0, &conv.store) {
        Outcome::Spawn(job) => job,
        other => panic!("{other:?}"),
    };
    assert_eq!(job, Job::Fork("steer-v2".into()));
    host::run(job, &conv).await.expect("fork");

    let forks = server.params_of("session/fork");
    assert_eq!(forks, vec![json!({"session_id": parent, "new_chat_id": "steer-v2"})]);
    let st = host::state();
    assert_eq!(st.ck.forked.as_deref(), Some("a7:steer-v2"), "the server's exact child");
    assert_eq!(
        st.ck.notice.as_deref(),
        Some("Conversation fork opened in the background. Your selection was not changed.")
    );
    assert!(st.ck.completed);
    drop(st);
    // Background: the child is known (and listed) but never selected, and no
    // kickoff turn or foreground open was sent for it.
    assert_eq!(conv.store.active_session().as_deref(), Some(parent.as_str()), "selection unchanged");
    assert!(conv.store.sessions().iter().any(|s| s.id == "a7:steer-v2"), "the fork is in the session list");
    let after: Vec<String> = server.methods().into_iter().skip_while(|m| m != "session/fork").collect();
    assert_eq!(after, ["session/fork", "session/hydrate", "session/list"], "rehydrate the owner, list the child");
    assert!(server.params_of("turn/start").is_empty(), "no kickoff");
}

#[tokio::test]
async fn undo_lists_snapshots_confirms_then_restores_and_rehydrates() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    let session = conv.session_id();

    let job = spawned(host::command("undo", "", &conv));
    host::run(job, &conv).await.expect("snapshot list");
    {
        let st = host::state();
        assert_eq!(st.ck.mode, octoscode_module::screens::history::HistoryMode::Undo);
        let rows = &st.ck.snapshots.as_ref().expect("listed").snapshots;
        assert_eq!(rows.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["snap-2", "snap-1"]);
    }
    assert_eq!(host::perform("b3.ck.snap", 0, &conv.store), Outcome::Done);
    assert_eq!(host::state().ck.snap_confirm, Some(0), "a selection asks for confirmation first");
    assert!(server.params_of("snapshot/restore").is_empty());
    let job = match host::perform("b3.ck.snap_confirm", 0, &conv.store) {
        Outcome::Spawn(job) => job,
        other => panic!("{other:?}"),
    };
    host::run(job, &conv).await.expect("restore");
    // A15 — the open itself hydrates first (every open loads its history,
    // the web's open -> hydrate); the undo's own order starts at its list.
    let after: Vec<String> =
        server.methods().into_iter().skip_while(|m| m != "snapshot/list").filter(|m| m != "session/list").collect();
    assert_eq!(
        after,
        ["snapshot/list", "snapshot/list", "snapshot/restore", "session/hydrate"],
        "fresh list, restore the exact target, rehydrate the owner"
    );
    assert_eq!(
        server.params_of("snapshot/restore"),
        vec![json!({"session_id": session, "snapshot_id": "snap-2"})]
    );
    let st = host::state();
    assert_eq!(
        st.ck.notice.as_deref(),
        Some("Workspace snapshot restored. Conversation history was not changed.")
    );
    assert!(st.ck.completed);
}
