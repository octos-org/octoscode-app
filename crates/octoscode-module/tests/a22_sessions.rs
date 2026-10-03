//! A22 — four Session rows on the production path: the real WS transport,
//! the real `Conversation` (link + flow + the lib.rs event loop's
//! `on_event`), and a scripted fake Core that answers the way octos
//! a6ea8505 does (recorded shapes: `r43a-recovery`, `r23-conversation`).
//!
//! * **Row 203** — a candidate Session is opened and hydrated before it is
//!   shown, and its live events are BUFFERED until the hydrate commits, then
//!   released in order: no loss, no duplicate (web
//!   `features/session/candidate-session.ts:73-220`, the production
//!   pooled-transport variant `active-session-runtime.ts:74-207`, the drain
//!   `:647-681`, the buffered staleness rule `durable-session.ts:160-186`;
//!   test `candidate-session.test.ts` "buffers live events until optional
//!   hydrate preparation finishes").
//! * **Row 216** — per-record composer draft: effort / showReasoning /
//!   images, and ordered restores of FULL returned turns (web
//!   `session-composer-drafts.ts:25-206`; test
//!   `session-composer-drafts.test.ts` "keeps full returned drafts in order
//!   without replacing new images or another Session").
//! * **Row 228** — only full Sessions of the requested profile are projected
//!   from an ATTESTED catalog, merged with the Sessions this app opened (web
//!   `workspace-session-catalog.ts:66-92` + `:197-209`,
//!   `SessionSidebar.tsx:116-140`; test `workspace-session-catalog.test.ts`
//!   "keeps only full sessions of the requested profile and maps server
//!   metadata").
//! * **Row 236** — a background Session's visible state (web
//!   `background-session-status.ts:9-28`, `use-octos-session.ts:2714-2763`,
//!   `SessionSidebar.tsx:99-104`/`:163-176`/`:251-264`; test
//!   `background-session-status.test.ts` "uses the latest real terminal,
//!   regardless of later generic activity").
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::components::ItemKind;
use octoscode_module::flow::Conversation;
use octoscode_module::screens::sidebar;

const PROFILE: &str = "a22";
const CWD: &str = "/home/user/a22-ws";

// ------------------------------------------------------------- the fake Core

enum Reply {
    Ok(Value),
    Err(i64, String),
    /// Notification frames, then the result, then more frames — all on the
    /// one socket, in this order (the candidate race: live events of a
    /// Session reach the client between its open and its history read).
    Around { before: Vec<Value>, result: Value, after: Vec<Value> },
    /// Frames now, the result `delay_ms` later (a history read that settles
    /// after the person moved on).
    Late { before: Vec<Value>, delay_ms: u64, result: Value },
    /// Frames now, the result once the test opens `gate`.
    Held { before: Vec<Value>, gate: Arc<tokio::sync::Notify>, result: Value },
}

type Script = Arc<dyn Fn(&str, &Value) -> Reply + Send + Sync>;

struct Core {
    base: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
    push: Arc<Mutex<Option<mpsc::UnboundedSender<String>>>>,
}

fn note(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
}

impl Core {
    async fn start(script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let push: Arc<Mutex<Option<mpsc::UnboundedSender<String>>>> = Arc::new(Mutex::new(None));
        let (seen2, push2) = (seen.clone(), push.clone());
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { continue };
                let (mut sink, mut source) = ws.split();
                let (tx, mut rx) = mpsc::unbounded_channel::<String>();
                *push2.lock().unwrap() = Some(tx.clone());
                tokio::spawn(async move {
                    while let Some(frame) = rx.recv().await {
                        if sink.send(Message::Text(frame.into())).await.is_err() {
                            break;
                        }
                    }
                });
                let (seen, script) = (seen2.clone(), script.clone());
                tokio::spawn(async move {
                    while let Some(Ok(msg)) = source.next().await {
                        let Message::Text(text) = msg else { continue };
                        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                        if v.get("id").is_none() {
                            continue;
                        }
                        let method = v["method"].as_str().unwrap_or("").to_owned();
                        seen.lock().unwrap().push((method.clone(), v["params"].clone()));
                        let ok = |result: Value| json!({"jsonrpc": "2.0", "id": v["id"], "result": result});
                        match script(&method, &v["params"]) {
                            Reply::Ok(result) => {
                                let _ = tx.send(ok(result).to_string());
                            }
                            Reply::Err(code, message) => {
                                let e = json!({"jsonrpc": "2.0", "id": v["id"], "error": {"code": code, "message": message}});
                                let _ = tx.send(e.to_string());
                            }
                            Reply::Around { before, result, after } => {
                                for f in before {
                                    let _ = tx.send(f.to_string());
                                }
                                let _ = tx.send(ok(result).to_string());
                                for f in after {
                                    let _ = tx.send(f.to_string());
                                }
                            }
                            Reply::Late { before, delay_ms, result } => {
                                for f in before {
                                    let _ = tx.send(f.to_string());
                                }
                                let (tx, frame) = (tx.clone(), ok(result).to_string());
                                tokio::spawn(async move {
                                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                                    let _ = tx.send(frame);
                                });
                            }
                            Reply::Held { before, gate, result } => {
                                for f in before {
                                    let _ = tx.send(f.to_string());
                                }
                                let (tx, frame) = (tx.clone(), ok(result).to_string());
                                tokio::spawn(async move {
                                    gate.notified().await;
                                    let _ = tx.send(frame);
                                });
                            }
                        }
                    }
                });
            }
        });
        Self { base, seen, push }
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    fn notify(&self, method: &str, params: Value) {
        if let Some(tx) = self.push.lock().unwrap().as_ref() {
            let _ = tx.send(note(method, params).to_string());
        }
    }
}

/// The transport's own history read (messages); A8/A20's parked-interaction
/// read names `include: ["pending_approvals"]` and gets a plain reply.
fn messages_read(p: &Value) -> bool {
    !p["include"].as_array().is_some_and(|a| a.iter().any(|x| x == "pending_approvals"))
}

fn opened(session: &str, cwd: &str, effort: Option<&str>) -> Value {
    let mut o = json!({
        "session_id": session, "active_profile_id": PROFILE, "workspace_root": cwd,
        "cursor": {"stream": session, "seq": 1},
        "capabilities": {
            "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
            "capabilities_schema_version": 2,
            // A20: a question gets a card only when its answer is negotiated
            // (`user_question/respond` + `user_question.v1`).
            "supported_methods": ["session/open", "session/hydrate", "session/list", "turn/start", "turn/interrupt",
                                  "user_question/respond"],
            "supported_notifications": ["turn/started", "projection/envelope", "user_question/requested"],
            "supported_features": ["state.session_hydrate.v1", "projection.envelope.v2", "session.workspace_cwd.v1", "user_question.v1"]
        }
    });
    if let Some(e) = effort {
        o["reasoning_effort"] = json!(e);
    }
    json!({ "opened": o })
}

/// One `projection/envelope` (the recorded a6ea8505 shape).
fn env(session: &str, turn: &str, seq: u64, cursor: u64, payload: Value) -> Value {
    note(
        "projection/envelope",
        json!({"session_id": session, "thread_id": turn, "turn_id": turn, "seq": seq,
               "cursor": {"stream": session, "seq": cursor}, "payload": payload}),
    )
}

fn started(session: &str, turn: &str) -> Value {
    note("turn/started", json!({"session_id": session, "turn_id": turn, "timestamp": "2026-10-02T09:00:00Z"}))
}

fn terminal(session: &str, turn: &str, seq: u64, cursor: u64, outcome: &str) -> Value {
    env(session, turn, seq, cursor, json!({"type": "turn_terminal", "data": {"outcome": outcome}}))
}

/// A persisted transcript row (`HydratedMessage`: `thread_id` = the turn).
fn row(seq: u64, role: &str, content: &str, turn: &str) -> Value {
    json!({"seq": seq, "role": role, "content": content, "thread_id": turn,
           "persisted_at": "2026-10-02T09:00:00Z", "message_id": format!("m{seq}")})
}

fn hydrated(session: &str, cursor: u64, rows: Vec<Value>, threads: &[(&str, u64)]) -> Value {
    let seqs: BTreeMap<String, u64> = threads.iter().map(|(t, s)| ((*t).to_owned(), *s)).collect();
    json!({"session_id": session, "cursor": {"stream": session, "seq": cursor},
           "messages": rows, "projection_thread_sequences": seqs})
}

fn empty_history(session: &str) -> Value {
    hydrated(session, 1, vec![], &[])
}

// --------------------------------------------------------------- the driver

/// The app's startup path: connect, the lib.rs drain loop on the runtime
/// (`while let Some(evt) = evt_rx.recv()` -> `on_event`), then the startup
/// `open_workspace(cwd)`; returns once the Session's history settled.
async fn launch(core: &Core) -> Arc<Conversation> {
    launch_logged(core).await.0
}

/// [`launch`], keeping the flow's verdict for every event (`on_event`'s
/// `FlowEvent`, as text) so a test can wait on one.
async fn launch_logged(core: &Core) -> (Arc<Conversation>, Arc<Mutex<Vec<String>>>) {
    launch_paced(core, Arc::default()).await
}

/// [`launch_logged`] whose drain loop sleeps `pace` µs after each event: a
/// consumer as slow as a busy UI thread, so the transport's event channel
/// stays full while the server keeps sending.
async fn launch_paced(
    core: &Core,
    pace: Arc<std::sync::atomic::AtomicU64>,
) -> (Arc<Conversation>, Arc<Mutex<Vec<String>>>) {
    let (conv, mut events) = Conversation::connect(&core.base, "dummy", PROFILE, Some(CWD.to_owned()), None).expect("connect");
    let conv = Arc::new(conv);
    conv.attach();
    let drv = conv.clone();
    let log: Arc<Mutex<Vec<String>>> = Arc::default();
    let sink = log.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            let out = drv.on_event(evt);
            sink.lock().unwrap().push(format!("{out:?}"));
            let us = pace.load(std::sync::atomic::Ordering::Relaxed);
            if us > 0 {
                tokio::time::sleep(Duration::from_micros(us)).await;
            }
        }
    });
    conv.open_workspace(Some(CWD.to_owned())).await.expect("session/open");
    let s = conv.session_id();
    until("the startup Session's history settles", || {
        conv.store.is_live() && conv.history(&s) == octoscode_module::flow::History::Ready
    })
    .await;
    (conv, log)
}

/// A sidebar click / switch: `open_session` with the Session's folder (the
/// lib.rs `thread.open` effect), its history settled.
async fn open(conv: &Arc<Conversation>, id: &str) {
    conv.open_session(id, Some(CWD.to_owned())).await.expect("session/open");
    until(&format!("{id}'s history settles"), || {
        conv.store.active_session().as_deref() == Some(id) && conv.history(id) == octoscode_module::flow::History::Ready
    })
    .await;
}

async fn until(what: &str, f: impl Fn() -> bool) {
    for _ in 0..500 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for: {what}");
}

/// Let in-flight frames land (nothing is expected to change).
async fn quiet() {
    tokio::time::sleep(Duration::from_millis(400)).await;
}

/// The transcript the window draws for the ACTIVE Session
/// (`screen::timeline_rows_folded`): (kind, text) in order.
fn shown(conv: &Conversation) -> Vec<(ItemKind, String)> {
    let session = conv.store.active_session().unwrap_or_default();
    let entries = conv.store.domains.session.timeline.entries(&session);
    octoscode_module::screen::timeline_rows_folded(&conv.store, false, &[])
        .into_iter()
        .map(|r| {
            let text = match r.kind {
                ItemKind::UserBubble | ItemKind::AssistantProse => entries.get(r.index).map(|e| e.text.clone()).unwrap_or_default(),
                _ => String::new(),
            };
            (r.kind, text)
        })
        .collect()
}

fn texts(rows: &[(ItemKind, String)], kind: ItemKind) -> Vec<String> {
    rows.iter().filter(|(k, _)| *k == kind).map(|(_, t)| t.clone()).collect()
}

/// The sidebar's session rows as the chrome draws them: (session id, title,
/// status). The production projection over the store.
fn sidebar_rows(conv: &Conversation) -> Vec<(String, String, sidebar::Status)> {
    let ui = sidebar::SidebarUi { mode: sidebar::Mode::Flat, ..Default::default() };
    let sessions = conv.store.sessions();
    sidebar::project_with(&conv.store, &ui, sidebar::now_ms(), &[])
        .rows
        .into_iter()
        .filter_map(|r| match r {
            sidebar::Row::Session { store_index, title, status, .. } => {
                Some((sessions.get(store_index).map(|s| s.id.clone()).unwrap_or_default(), title, status))
            }
            _ => None,
        })
        .collect()
}

fn status_of(conv: &Conversation, id: &str) -> Option<sidebar::Status> {
    sidebar_rows(conv).into_iter().find(|(s, _, _)| s == id).map(|(_, _, st)| st)
}

fn quit(conv: &Conversation) {
    let _ = conv.command_sender().try_send(octos_app_transport::OutboundCommand::Disconnect);
}

/// Process-wide test state (drafts file, the composer's start timeout).
fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    let dir = std::env::temp_dir().join(format!("a22-sessions-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    std::env::set_var("OCTOSCODE_DRAFTS_FILE", dir.join("drafts.json"));
    std::env::set_var("OCTOSCODE_SHOW_THINKING_FILE", dir.join("show-thinking.json"));
    std::env::set_var("OCTOSCODE_TURN_START_TIMEOUT_MS", "4000");
    g
}

// ================================================================ row 228

/// `workspace-session-catalog.test.ts` "keeps only full sessions of the
/// requested profile and maps server metadata": the attested catalog keeps
/// only `<profile>:<channel>:<chat>` rows of THIS profile (another
/// profile's row and a bare id are dropped, never guessed), maps the
/// server's title / last prompt / time, and the sidebar merges it with the
/// Sessions this app opened (`SessionSidebar.tsx:116-140`: the opened
/// Session stays even though the catalog never lists a `<profile>:main`).
/// An unattested listing projects nothing (`:197-209`, "a client must not
/// place rows under a workspace unless this attests the scope",
/// octos-core `SessionListResult`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn row_228_only_full_sessions_of_the_requested_profile_are_projected() {
    let _g = lock();
    let rows = |with_new: bool| {
        let mut v = vec![
            json!({"id": "a22:api:web-old", "message_count": 730, "title": "学习一下如何做editable pptx",
                   "updated_at": "2026-09-22T01:37:00.222Z", "last_prompt": "记住现在的skills", "active_turn": false}),
            // Another profile's row must never show under this profile.
            json!({"id": "other:api:web-x", "message_count": 1, "title": "Foreign profile row"}),
            // A bare id cannot be routed; dropped rather than guessed.
            json!({"id": "bare", "message_count": 1, "title": "Bare id row"}),
            // No channel: not a full Session of this profile.
            json!({"id": "a22:legacy", "message_count": 4, "title": "Legacy row"}),
            // The Session this app opened (its native `<profile>:main`).
            json!({"id": "a22:main", "message_count": 2, "title": "Main chat", "updated_at": "2026-09-30T08:00:00Z"}),
        ];
        if with_new {
            v.insert(1, json!({"id": "a22:api:web-new", "message_count": 9}));
        }
        v
    };
    // The scoped catalog answers by phase (the startup may read it more than
    // once): attested (all rows); attested for ANOTHER profile; attested
    // again without web-new.
    let phase: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let listings: Arc<Vec<Value>> = Arc::new(vec![
        json!({"sessions": rows(true), "workspace_root": CWD, "profile_id": PROFILE}),
        json!({"sessions": rows(true), "workspace_root": CWD, "profile_id": "other"}),
        json!({"sessions": rows(false), "workspace_root": CWD, "profile_id": PROFILE}),
    ]);
    let (p2, l2) = (phase.clone(), listings.clone());
    let core = Core::start(Arc::new(move |method, p| match method {
        "session/open" => Reply::Ok(opened(p["session_id"].as_str().unwrap_or(""), p["cwd"].as_str().unwrap_or(CWD), None)),
        "session/hydrate" => Reply::Ok(empty_history(p["session_id"].as_str().unwrap_or(""))),
        "session/list" => match (p["cwd"].as_str(), p["profile_id"].as_str()) {
            (Some(_), Some(_)) => Reply::Ok(l2[*p2.lock().unwrap()].clone()),
            // The legacy global listing: every row, nothing attested.
            _ => Reply::Ok(json!({"sessions": rows(true)})),
        },
        _ => Reply::Ok(json!({})),
    }))
    .await;
    let conv = launch(&core).await;
    let main = conv.session_id();
    assert_eq!(main, "a22:main");
    until("the scoped catalog was read", || core.params_of("session/list").iter().any(|p| p.get("cwd").is_some())).await;
    quiet().await;
    let mut ids: Vec<String> = conv.store.sessions().into_iter().map(|s| s.id).collect();
    ids.sort();
    assert_eq!(
        ids,
        vec!["a22:api:web-new".to_owned(), "a22:api:web-old".to_owned(), "a22:main".to_owned()],
        "only full Sessions of a22 from the attested catalog, plus the Session this app opened"
    );
    // What the sidebar draws: the server's title, else "New chat"; newest
    // first among the catalog rows.
    let drawn = sidebar_rows(&conv);
    let titled: Vec<(String, String)> = drawn.iter().map(|(id, t, _)| (id.clone(), t.clone())).collect();
    assert!(titled.contains(&("a22:api:web-old".to_owned(), "学习一下如何做editable pptx".to_owned())), "{titled:?}");
    assert!(titled.contains(&("a22:api:web-new".to_owned(), "New chat".to_owned())), "{titled:?}");
    // The stated native adaptation (screens/catalog.rs): the catalog row of
    // the Session this app opened names it, though `a22:main` is not in the
    // full grammar; an unopened non-full id (`a22:legacy`) stays out.
    assert!(titled.contains(&("a22:main".to_owned(), "Main chat".to_owned())), "{titled:?}");
    for foreign in ["other:api:web-x", "bare", "a22:legacy"] {
        assert!(!titled.iter().any(|(id, _)| id == foreign), "{foreign} must never be projected: {titled:?}");
    }
    // A listing attested for ANOTHER profile is unscoped for this one:
    // nothing of it is projected; the opened Session stays.
    *phase.lock().unwrap() = 1;
    conv.refresh_sessions().await.expect("second listing");
    let ids: Vec<String> = conv.store.sessions().into_iter().map(|s| s.id).collect();
    assert_eq!(ids, vec!["a22:main".to_owned()], "an unattested listing projects nothing; the opened Session stays");
    // Attested again, without web-new: web-new is gone (never opened here).
    *phase.lock().unwrap() = 2;
    conv.refresh_sessions().await.expect("third listing");
    let mut ids: Vec<String> = conv.store.sessions().into_iter().map(|s| s.id).collect();
    ids.sort();
    assert_eq!(ids, vec!["a22:api:web-old".to_owned(), "a22:main".to_owned()]);
    quit(&conv);
}

// ================================================================ row 203

const T1: &str = "01920000-0000-7000-8000-0000000002a1";
const T2: &str = "01920000-0000-7000-8000-0000000002a2";
const T3: &str = "01920000-0000-7000-8000-0000000002a3";

/// `candidate-session.test.ts` "buffers live events until optional hydrate
/// preparation finishes", on the native open: Session B holds two persisted
/// turns and another client is running a third. Opening B, the live frames
/// of that third turn — and a replayed frame of the second, which B's
/// history already holds — reach the client BEFORE B's history read is
/// answered. They are staged, then released after the history commits: the
/// transcript reads turn 1, turn 2, turn 3 in order (the live turn below the
/// history, not above it), the replayed frame is not applied twice, and no
/// live frame is lost.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn row_203_a_candidates_live_events_wait_for_its_history_then_release_in_order() {
    let _g = lock();
    let b = "a22:api:beta";
    let core = Core::start(Arc::new(move |method, p| {
        let session = p["session_id"].as_str().unwrap_or("").to_owned();
        match method {
            "session/open" => Reply::Ok(opened(&session, p["cwd"].as_str().unwrap_or(CWD), None)),
            "session/hydrate" if session == b && messages_read(p) => Reply::Around {
                before: vec![
                    // A replayed frame of T2 — B's history already holds it.
                    env(b, T2, 4, 13, json!({"type": "assistant_persisted", "data": {
                        "text": "Answer two", "assistant_segment_id": format!("{T2}:assistant:iteration:1"),
                        "meta": {"message_id": "m4", "persisted_at": "2026-10-02T09:00:01Z"}}})),
                    // Another client's live turn T3.
                    started(b, T3),
                    env(b, T3, 1, 15, json!({"type": "user_message", "data": {"text": "Third prompt"}})),
                    env(b, T3, 2, 16, json!({"type": "assistant_delta", "data": {"text": "Partial ", "assistant_segment_id": format!("{T3}:assistant:iteration:1")}})),
                ],
                result: hydrated(
                    b,
                    14,
                    vec![
                        row(1, "user", "First prompt", T1),
                        row(2, "assistant", "Answer one", T1),
                        row(3, "user", "Second prompt", T2),
                        row(4, "assistant", "Answer two", T2),
                    ],
                    &[(T1, 4), (T2, 4)],
                ),
                after: vec![
                    env(b, T3, 3, 17, json!({"type": "assistant_delta", "data": {"text": "answer.", "assistant_segment_id": format!("{T3}:assistant:iteration:1")}})),
                    terminal(b, T3, 4, 18, "completed"),
                ],
            },
            "session/hydrate" => Reply::Ok(empty_history(&session)),
            "session/list" => Reply::Ok(json!({"sessions": []})),
            _ => Reply::Ok(json!({})),
        }
    }))
    .await;
    let conv = launch(&core).await;
    open(&conv, b).await;
    until("T3's terminal is folded", || conv.store.domains.turn.terminal(T3).as_deref() == Some("completed")).await;
    quiet().await;
    let rows = shown(&conv);
    assert_eq!(
        texts(&rows, ItemKind::UserBubble),
        vec!["First prompt", "Second prompt", "Third prompt"],
        "the history first, then the live turn — each prompt once"
    );
    assert_eq!(
        texts(&rows, ItemKind::AssistantProse),
        vec!["Answer one", "Answer two", "Partial answer."],
        "each answer once, the live one whole (no frame lost)"
    );
    // The replayed T2 answer was not drawn a second time.
    let answers_t2 = conv
        .store
        .domains
        .session
        .timeline
        .entries(b)
        .into_iter()
        .filter(|e| e.turn_id.as_deref() == Some(T2) && e.kind == octoscode_store::EntryKind::ASSISTANT_TEXT)
        .count();
    assert_eq!(answers_t2, 1, "the replayed frame the history holds is not applied twice");
    quit(&conv);
}

/// `candidate-session.test.ts` "fails closed on the 4097th notification
/// before commit": a candidate whose buffer passes the bound is disposed —
/// none of its buffered events is applied, the window says why ("Session
/// recovery required"), and its history read, answered afterwards, does not
/// commit it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn row_203_a_candidate_fails_closed_on_the_4097th_buffered_event() {
    use octoscode_module::flow::{History, CANDIDATE_LIMIT};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let _g = lock();
    let b = "a22:api:flood";
    let reads = Arc::new(AtomicUsize::new(0));
    let r2 = reads.clone();
    let gate = Arc::new(tokio::sync::Notify::new());
    let g2 = gate.clone();
    let core = Core::start(Arc::new(move |method, p| {
        let session = p["session_id"].as_str().unwrap_or("").to_owned();
        match method {
            "session/open" => Reply::Ok(opened(&session, p["cwd"].as_str().unwrap_or(CWD), None)),
            // The first history read of B is surrounded by a flood of its
            // live events; a later one (the re-open) is answered plainly.
            // (its reply waits until the app has taken the whole flood in:
            // the transport hands a reply over with a non-blocking send and
            // drops one that finds its 64-slot event channel full — not this
            // test's subject)
            "session/hydrate" if session == b && messages_read(p) && r2.fetch_add(1, Ordering::SeqCst) == 0 => Reply::Held {
                before: (1..=CANDIDATE_LIMIT as u64 + 1)
                    .map(|i| env(b, T3, i, 14 + i, json!({"type": "assistant_delta", "data": {"text": "x", "assistant_segment_id": "s"}})))
                    .collect(),
                gate: g2.clone(),
                result: hydrated(b, 14, vec![row(1, "user", "First prompt", T1), row(2, "assistant", "Answer one", T1)], &[(T1, 2)]),
            },
            "session/hydrate" if session == b && messages_read(p) => Reply::Ok(hydrated(
                b,
                4114,
                vec![row(1, "user", "First prompt", T1), row(2, "assistant", "Answer one", T1)],
                &[(T1, 2)],
            )),
            "session/hydrate" => Reply::Ok(empty_history(&session)),
            "session/list" => Reply::Ok(json!({"sessions": []})),
            _ => Reply::Ok(json!({})),
        }
    }))
    .await;
    let (conv, verdicts) = launch_logged(&core).await;
    conv.open_session(b, Some(CWD.to_owned())).await.expect("session/open");
    until("the candidate failed closed", || {
        matches!(conv.history(b), History::Failed(ref r) if r == "The candidate session emitted too many events while opening.")
    })
    .await;
    // The history read is answered after the flood: it never commits.
    gate.notify_one();
    until("B's history reply was judged", || {
        verdicts.lock().unwrap().iter().any(|v| v.contains("session/hydrate-candidate-failed"))
    })
    .await;
    quiet().await;
    assert!(matches!(conv.history(b), History::Failed(_)), "still failed: {:?}", conv.history(b));
    // B's live events keep coming after the failure: the failed candidate
    // stays closed — none is drawn under its failure.
    // (they continue B's stream: the flood ended at cursor 4111)
    for i in 1..=3u64 {
        let f = env(b, T3, CANDIDATE_LIMIT as u64 + 1 + i, 4111 + i,
            json!({"type": "assistant_delta", "data": {"text": "late ", "assistant_segment_id": "s"}}));
        core.notify(f["method"].as_str().unwrap(), f["params"].clone());
    }
    quiet().await;
    let reached = |conv: &Conversation| -> Vec<(String, String)> {
        conv.store
            .domains
            .session
            .timeline
            .entries(b)
            .into_iter()
            .take(6)
            .map(|e| (format!("{:?}", e.kind), e.text.chars().take(40).collect()))
            .collect()
    };
    assert!(
        reached(&conv).is_empty(),
        "none of the buffered events, the uncommitted history or the later live events reached the transcript: {:?}",
        reached(&conv)
    );
    // Opened again (the failure notice's own advice: "Reopen it from the
    // sidebar"): a new open, a new candidate, its history commits, and B's
    // live events are B's again.
    conv.open_session(b, Some(CWD.to_owned())).await.expect("session/open again");
    until("B is ready after the re-open", || conv.history(b) == History::Ready).await;
    // (the next event after the re-open's history: cursor 4115 follows its 4114)
    let f = env(b, T2, 1, 4115, json!({"type": "assistant_delta", "data": {"text": "after the re-open", "assistant_segment_id": "s4"}}));
    core.notify(f["method"].as_str().unwrap(), f["params"].clone());
    until("B's live event is drawn again", || reached(&conv).iter().any(|(_, t)| t.contains("after the re-open"))).await;
    let texts: Vec<String> = reached(&conv).into_iter().map(|(_, t)| t).collect();
    assert!(texts.iter().any(|t| t == "First prompt") && !texts.iter().any(|t| t.starts_with('x') || t.starts_with("late")), "{texts:?}");
    quit(&conv);
}

/// `candidate-session.test.ts` "keeps a newer candidate owned when an older
/// stage settles after cancel": the person opens B, and opens C before B's
/// history answered. C's open replaces B's candidate — B's buffered live
/// events are dropped with it — C is prepared and shown in order, and B's
/// history, answered late under a retired generation, never commits B (it
/// is no record of this connection: its later events are foreign).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn row_203_a_newer_open_replaces_an_unprepared_candidate() {
    let _g = lock();
    let (b, c) = ("a22:api:slow", "a22:api:next");
    let core = Core::start(Arc::new(move |method, p| {
        let session = p["session_id"].as_str().unwrap_or("").to_owned();
        match method {
            "session/open" => Reply::Ok(opened(&session, p["cwd"].as_str().unwrap_or(CWD), None)),
            "session/hydrate" if session == b && messages_read(p) => Reply::Late {
                before: vec![
                    started(b, T3),
                    env(b, T3, 1, 15, json!({"type": "user_message", "data": {"text": "B's live prompt"}})),
                ],
                delay_ms: 900,
                result: hydrated(b, 14, vec![row(1, "user", "B history", T1)], &[(T1, 1)]),
            },
            "session/hydrate" if session == c && messages_read(p) => Reply::Around {
                before: vec![
                    started(c, T3.replace("a3", "c3").as_str()),
                    env(c, &T3.replace("a3", "c3"), 1, 9, json!({"type": "user_message", "data": {"text": "C live"}})),
                ],
                result: hydrated(c, 8, vec![row(1, "user", "C history", T2), row(2, "assistant", "C answer", T2)], &[(T2, 2)]),
                after: vec![],
            },
            "session/hydrate" => Reply::Ok(empty_history(&session)),
            "session/list" => Reply::Ok(json!({"sessions": []})),
            _ => Reply::Ok(json!({})),
        }
    }))
    .await;
    let conv = launch(&core).await;
    conv.open_session(b, Some(CWD.to_owned())).await.expect("open B");
    until("B's history read went out", || core.params_of("session/hydrate").iter().any(|p| p["session_id"] == json!(b))).await;
    until("B's live events are buffered", || conv.candidate_session().as_deref() == Some(b)).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    // The person moves on before B's history answered.
    open(&conv, c).await;
    let rows = shown(&conv);
    assert_eq!(texts(&rows, ItemKind::UserBubble), vec!["C history", "C live"], "C: history first, then its live turn");
    // B's late history (a retired generation) settles after: never committed.
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    assert!(conv.store.domains.session.timeline.entries(b).is_empty(), "nothing of B reached its transcript");
    assert!(!conv.store.domains.session.is_record(b), "B never became a record");
    assert!(conv.store.domains.session.is_record(c));
    assert_eq!(conv.store.active_session().as_deref(), Some(c));
    // A later event of B is foreign to this connection now.
    core.notify("projection/envelope", json!({"session_id": b, "thread_id": T3, "turn_id": T3, "seq": 2,
        "cursor": {"stream": b, "seq": 16}, "payload": {"type": "assistant_delta", "data": {"text": "late", "assistant_segment_id": "s"}}}));
    quiet().await;
    assert!(conv.store.domains.session.timeline.entries(b).is_empty());
    quit(&conv);
}

// ================================================================ row 216

/// `session-composer-drafts.ts:40` + "restores server thinking choice once":
/// the Session's thinking effort starts from its open reply ONCE; a re-open
/// (switching away and back) never overwrites what the person chose since.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn row_216_the_effort_is_seeded_once_per_record() {
    let _g = lock();
    let b = "a22:api:other";
    let core = Core::start(Arc::new(move |method, p| {
        let session = p["session_id"].as_str().unwrap_or("").to_owned();
        match method {
            "session/open" => {
                let effort = (session != b).then_some("high");
                Reply::Ok(opened(&session, p["cwd"].as_str().unwrap_or(CWD), effort))
            }
            "session/hydrate" => Reply::Ok(empty_history(&session)),
            "session/list" => Reply::Ok(json!({"sessions": []})),
            _ => Reply::Ok(json!({})),
        }
    }))
    .await;
    let conv = launch(&core).await;
    let a = conv.session_id();
    let effort = |s: &str| conv.store.domains.session.thinking(s).effort;
    assert_eq!(effort(&a), "high", "the open reply seeds the record's effort");
    // The person picks Low (the Thinking dialog's segment).
    octoscode_module::screens::board3::thinking::apply_arg(&conv.store, &a, "low").expect("low");
    assert_eq!(effort(&a), "low");
    open(&conv, b).await;
    assert_eq!(effort(b), "", "a Session whose reply names none keeps the Profile default");
    open(&conv, &a).await;
    assert_eq!(effort(&a), "low", "the re-open's reply does not overwrite the person's choice");
    quit(&conv);
}

/// `session-composer-drafts.test.ts` "keeps full returned drafts in order
/// without replacing new images or another Session", on the native send
/// path: two prompts queued behind a running turn capture the effort (and
/// the first one the uploaded image) AT ADMISSION; the server refuses both
/// starts, and each comes back WHOLE — its text, its effort, its image — to
/// its own Session, in order; never over images the person selected since,
/// and never into another Session. Sent again, the image rides without a
/// second upload.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn row_216_returned_prompts_come_back_whole_and_in_order() {
    use octoscode_module::screens::composer_drafts;
    use octoscode_module::screens::media::{self, LocalFile, TurnMedia};
    let _g = lock();
    let starts = Arc::new(Mutex::new(0usize));
    let s2 = starts.clone();
    let core = Core::start(Arc::new(move |method, p| {
        let session = p["session_id"].as_str().unwrap_or("").to_owned();
        match method {
            "session/open" => Reply::Ok(opened(&session, p["cwd"].as_str().unwrap_or(CWD), Some("high"))),
            "session/hydrate" => Reply::Ok(empty_history(&session)),
            "session/list" => Reply::Ok(json!({"sessions": []})),
            "turn/start" => {
                let mut n = s2.lock().unwrap();
                *n += 1;
                if *n == 2 || *n == 3 {
                    Reply::Err(-32000, "the server refused this turn".to_owned())
                } else {
                    Reply::Ok(json!({"accepted": true}))
                }
            }
            _ => Reply::Ok(json!({})),
        }
    }))
    .await;
    let conv = launch(&core).await;
    let a = conv.session_id();
    let effort = |c: &Conversation| c.store.domains.session.thinking(&a).effort;
    let image = TurnMedia {
        path: format!("up/{}/original.png", b64("a22/original")),
        mime: "image/png".into(),
        size_bytes: 11,
    };
    // A running turn, so later prompts queue (FIFO).
    conv.set_draft("busy");
    conv.submit_draft().await.expect("busy");
    let busy = core.params_of("turn/start")[0]["turn_id"].as_str().unwrap().to_owned();
    // The first queued prompt carries the uploaded image and the effort High.
    let drafts = media::drafts_for_conv(&conv);
    assert!(drafts.restore_uploaded(vec![image.clone()]).expect("uploaded"), "the image is in A's draft");
    conv.set_draft("first returned draft");
    conv.submit_draft().await.expect("first");
    assert!(drafts.is_empty(), "admission consumed the image");
    // The second is queued with Low, no image.
    octoscode_module::screens::board3::thinking::apply_arg(&conv.store, &a, "low").expect("low");
    conv.set_draft("second returned draft");
    conv.submit_draft().await.expect("second");
    assert_eq!(conv.store.domains.composer.snapshot(&a).pending.len(), 2);
    // The person selects a NEW image meanwhile.
    drafts
        .select_files(vec![LocalFile { name: "new.png".into(), bytes: 3, mime: "image/png".into(), content: Arc::new(b"new".to_vec()) }])
        .expect("select");
    // The running turn ends: each queued head is started and REFUSED.
    core.notify("projection/envelope", json!({"session_id": a, "thread_id": busy, "turn_id": busy, "seq": 1,
        "cursor": {"stream": a, "seq": 2}, "payload": {"type": "turn_terminal", "data": {"outcome": "completed"}}}));
    until("both queued starts were refused", || core.params_of("turn/start").len() == 3).await;
    quiet().await;
    let sent = core.params_of("turn/start");
    assert_eq!(sent[1]["reasoning_effort"], json!("high"), "captured at admission: {}", sent[1]);
    assert_eq!(sent[1]["media"][0]["path"], json!(image.path), "the image rode the first prompt");
    assert_eq!(sent[2]["reasoning_effort"], json!("low"));
    assert!(sent[2].get("media").is_none(), "admission consumed A's media: the next prompt cannot borrow it");
    // Never over new images: both wait on A's record, in order.
    assert_eq!(conv.ui().lock().unwrap().draft(), "", "nothing replaced the new image");
    assert_eq!(effort(&conv), "low");
    assert_eq!(drafts.entries()[0].name, "new.png");
    assert_eq!(composer_drafts::pending(&conv, &a), 2);
    assert_eq!(composer_drafts::consume_restore(&conv, &a), None, "new images block the restore");
    // Another Session never takes A's.
    assert_eq!(composer_drafts::consume_restore(&conv, "a22:api:other"), None);
    assert_eq!(composer_drafts::peek_restore(&conv, "a22:api:other"), None);
    // The person removes the new image; the empty composer takes the FIRST
    // returned prompt back whole (what lib.rs `sync_composer_extras` calls).
    drafts.remove(&drafts.entries()[0].id);
    assert_eq!(composer_drafts::consume_restore(&conv, &a).as_deref(), Some("first returned draft"));
    assert_eq!(effort(&conv), "high", "the returned turn's own effort");
    let back = drafts.entries();
    assert_eq!(back.len(), 1, "{back:?}");
    assert_eq!((back[0].name.as_str(), back[0].status), ("original.png", media::DraftStatus::Ready), "uploaded, not selected");
    assert_eq!(composer_drafts::consume_restore(&conv, &a), None, "the restored image blocks the next restore");
    // Sent again: the SAME uploaded handle rides with High (an upload would
    // mint a new handle; the draft re-adopted this one).
    conv.set_draft("first returned draft");
    conv.submit_draft().await.expect("resend");
    until("the resend went out", || core.params_of("turn/start").len() == 4).await;
    let again = core.params_of("turn/start")[3].clone();
    assert_eq!(again["media"], json!([{"path": image.path, "mime": "image/png", "size_bytes": 11}]), "{again}");
    assert_eq!(again["reasoning_effort"], json!("high"));
    // The composer is empty again: the second comes back with its own Low.
    assert_eq!(composer_drafts::consume_restore(&conv, &a).as_deref(), Some("second returned draft"));
    assert_eq!(effort(&conv), "low");
    assert_eq!(composer_drafts::consume_restore(&conv, &a), None);
    // Reasoning visibility is per record and never captured by a turn.
    octoscode_module::screens::board3::thinking::perform(&conv.store, &a, "b3.think.show");
    assert!(!conv.store.domains.session.thinking(&a).show_reasoning);
    assert!(conv.store.domains.session.thinking("a22:api:other").show_reasoning, "another record keeps its own");
    assert!(again.get("show_reasoning").is_none());
    // A retired record drops its restores and ignores a late one.
    composer_drafts::restore_interrupt_prompt(&conv, &a, "interrupted prompt");
    composer_drafts::retire(&conv, &a);
    composer_drafts::restore_interrupt_prompt(&conv, &a, "late");
    assert_eq!(composer_drafts::peek_restore(&conv, &a), None);
    assert!(drafts.disposed(), "the record's image draft is released");
    quit(&conv);
}

/// The web's `btoa(...).replace(/=/g, "")` of an upload handle's profile part.
fn b64(s: &str) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = s.as_bytes();
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..=chunk.len() {
            out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

// ================================================================ row 236

const TC1: &str = "01920000-0000-7000-8000-0000000002c1";
const TC2: &str = "01920000-0000-7000-8000-0000000002c2";

/// `background-session-status.test.ts`: a Session this app opened keeps its
/// own visible state while another one is selected — its work (the record's
/// own queue) reads "running", its turn's terminal lands in the background
/// ("completed" / "failed"), and the LATEST real terminal decides,
/// regardless of later generic activity. Its queued prompt starts there (a
/// terminal advances a background FIFO, `use-octos-session.ts:2711-2713`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn row_236_a_background_session_keeps_its_own_visible_state() {
    let _g = lock();
    let b = "a22:api:bg-build";
    let c = "a22:api:bg-tests";
    let core = Core::start(Arc::new(move |method, p| {
        let session = p["session_id"].as_str().unwrap_or("").to_owned();
        match method {
            "session/open" => Reply::Ok(opened(&session, p["cwd"].as_str().unwrap_or(CWD), None)),
            "session/hydrate" => Reply::Ok(empty_history(&session)),
            "session/list" => Reply::Ok(json!({"sessions": []})),
            "turn/start" => Reply::Ok(json!({"accepted": true})),
            _ => Reply::Ok(json!({})),
        }
    }))
    .await;
    let conv = launch(&core).await;
    let a = conv.session_id();
    // B: a prompt runs, a second one waits behind it.
    open(&conv, b).await;
    conv.set_draft("build the release");
    conv.submit_draft().await.expect("start in B");
    let tb = core.params_of("turn/start")[0]["turn_id"].as_str().unwrap().to_owned();
    core.notify("turn/started", json!({"session_id": b, "turn_id": tb, "timestamp": "2026-10-02T09:00:00Z"}));
    conv.set_draft("then package it");
    conv.submit_draft().await.expect("queued in B");
    until("B's second prompt is queued", || conv.store.domains.composer.snapshot(b).pending.len() == 1).await;
    // C: a turn that fails, then a later one that completes, then generic
    // activity on the OLD turn.
    open(&conv, c).await;
    // D: a Session that will wait for an answer in the background.
    let d = "a22:api:bg-ask";
    open(&conv, d).await;
    // The person switches back to A: B, C and D are background Sessions now.
    open(&conv, &a).await;
    assert_eq!(status_of(&conv, b), Some(sidebar::Status::Running), "B's own queue is still working");
    assert_eq!(status_of(&conv, &a), Some(sidebar::Status::Idle), "the selected row reads its OWN work, not B's live turn");
    // The web's BackgroundSessionSnapshot for B.
    let snap = |id: &str| sidebar::background_sessions(&conv.store).into_iter().find(|s| s.session_id == id).expect("a background record");
    let sb = snap(b);
    assert_eq!((sb.state, sb.active_turn_id.as_deref(), sb.queued_count, sb.waiting), (sidebar::Status::Running, Some(tb.as_str()), 1, false));
    assert!(!sidebar::background_sessions(&conv.store).iter().any(|s| s.session_id == a), "the selected Session is not background");
    assert_eq!(sidebar::status_label(sb.state, true), "Working in background");
    // D's question arrives while it is in the background: D waits, A's
    // foreground does not.
    core.notify("user_question/requested", json!({"session_id": d, "turn_id": "01920000-0000-7000-8000-0000000002d1",
        "question_id": "01a0eb8f-7b23-7030-9f26-a284864217a1", "title": "Which branch?", "body": "1. Which branch?",
        "questions": [{"allow_free_text": true, "header": "Branch", "multi_select": false,
                       "options": [{"label": "main", "description": "the default"}], "question": "Which branch?"}]}));
    until("D waits", || status_of(&conv, d) == Some(sidebar::Status::Waiting)).await;
    let sd = snap(d);
    assert!(sd.waiting && sd.unread, "{sd:?}");
    assert_eq!(sidebar::status_label(sd.state, true), "Waiting for input");
    assert!(!conv.ui().lock().unwrap().question_pending(), "A's foreground is not waiting for D's question");
    // C's turns land while it is in the background.
    for f in [
        started(c, TC1),
        terminal(c, TC1, 1, 3, "errored"),
        started(c, TC2),
        terminal(c, TC2, 1, 5, "completed"),
    ] {
        core.notify(f["method"].as_str().unwrap(), f["params"].clone());
    }
    // Generic activity for the OLD (failed) turn after the newer terminal.
    core.notify("projection/envelope", json!({"session_id": c, "thread_id": TC1, "turn_id": TC1, "seq": 2,
        "cursor": {"stream": c, "seq": 6}, "payload": {"type": "reasoning_delta", "data": {"text": "late note"}}}));
    until("C's terminals landed", || conv.store.domains.turn.terminal(TC2).as_deref() == Some("completed")).await;
    quiet().await;
    assert_eq!(status_of(&conv, c), Some(sidebar::Status::Done), "the latest real terminal decides");
    assert!(snap(c).unread, "C's activity is unread");
    assert_eq!(sidebar::status_label(sidebar::Status::Done, true), "Completed in background");
    // B's running turn completes in the background: its queued prompt
    // starts THERE (the second turn/start names B), so B keeps working.
    core.notify("projection/envelope", json!({"session_id": b, "thread_id": tb, "turn_id": tb, "seq": 1,
        "cursor": {"stream": b, "seq": 2}, "payload": {"type": "turn_terminal", "data": {"outcome": "completed"}}}));
    until("B's queued prompt was started in the background", || core.params_of("turn/start").len() == 2).await;
    let second = core.params_of("turn/start")[1].clone();
    assert_eq!(second["session_id"], json!(b), "{second}");
    assert_eq!(second["input"][0]["text"], json!("then package it"));
    assert_eq!(status_of(&conv, b), Some(sidebar::Status::Running));
    let tb2 = second["turn_id"].as_str().unwrap().to_owned();
    core.notify("projection/envelope", json!({"session_id": b, "thread_id": tb2, "turn_id": tb2, "seq": 1,
        "cursor": {"stream": b, "seq": 3}, "payload": {"type": "turn_terminal", "data": {"outcome": "completed"}}}));
    until("B completed in the background", || status_of(&conv, b) == Some(sidebar::Status::Done)).await;
    // The foreground never took B's or C's work for its own.
    assert_eq!(conv.store.active_session().as_deref(), Some(a.as_str()));
    assert!(!conv.ui().lock().unwrap().turn_active(), "A's composer is not live for B's turn");
    // B's turns were B's: its transcript holds both prompts; A's none.
    let prompts = |id: &str| -> Vec<String> {
        conv.store.domains.session.timeline.of_kind(id, octoscode_store::EntryKind::USER_MESSAGE).into_iter().map(|e| e.text).collect()
    };
    assert_eq!(prompts(b), vec!["build the release", "then package it"]);
    assert!(prompts(&a).is_empty());
    // Selecting C reads it.
    open(&conv, c).await;
    assert!(!conv.store.domains.session.unread(c), "selected: read");
    assert_eq!(status_of(&conv, c), Some(sidebar::Status::Done), "the foreground's own terminal");
    quit(&conv);
}

/// The transport never drops a server reply (OctoSense fork patch 0002): a
/// history reply that reaches the client right behind a flood of live events
/// — the transport's event channel full, its consumer as slow as a busy UI —
/// arrives, every one of them, in the order the server answered, and the
/// per-Session history-read queue stays in step: B's read commits, B's
/// re-open's read commits (never judged by a lost read's generation), C's
/// read commits, and no read is left waiting. Before 0002 the transport's
/// `try_emit` dropped the reply that found the channel full: B never left
/// "Loading conversation…" and its queue stayed one read behind.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_history_reply_arrives_in_order_behind_a_flood_of_live_events() {
    use octoscode_module::flow::History;
    use std::sync::atomic::{AtomicU64, Ordering};
    let _g = lock();
    let (b, c) = ("a22:api:flood-b", "a22:api:flood-c");
    let elsewhere = "a22:api:elsewhere";
    // Each history read of B or C is answered right behind 400 live events
    // of a Session this client does not show (another client's busy turn).
    let cursor = Arc::new(AtomicU64::new(100));
    let c2 = cursor.clone();
    let core = Core::start(Arc::new(move |method, p| {
        let session = p["session_id"].as_str().unwrap_or("").to_owned();
        match method {
            "session/open" => Reply::Ok(opened(&session, p["cwd"].as_str().unwrap_or(CWD), None)),
            "session/hydrate" if (session == b || session == c) && messages_read(p) => {
                let from = c2.fetch_add(400, Ordering::SeqCst);
                Reply::Around {
                    before: (1..=400u64)
                        .map(|i| env(elsewhere, T3, from + i, from + i, json!({"type": "assistant_delta",
                            "data": {"text": "x", "assistant_segment_id": "s"}})))
                        .collect(),
                    result: hydrated(&session, 1, vec![row(1, "user", &format!("history of {session}"), T1)], &[(T1, 1)]),
                    after: vec![],
                }
            }
            "session/hydrate" => Reply::Ok(empty_history(&session)),
            "session/list" => Reply::Ok(json!({"sessions": [], "workspace_root": CWD, "profile_id": PROFILE})),
            _ => Reply::Ok(json!({})),
        }
    }))
    .await;
    let pace = Arc::new(AtomicU64::new(0));
    let (conv, verdicts) = launch_paced(&core, pace.clone()).await;
    let a = conv.session_id();
    // From here the consumer is slow: 2 ms per event.
    pace.store(2_000, Ordering::Relaxed);
    let hydrate_verdicts = || -> Vec<String> {
        verdicts.lock().unwrap().iter().filter(|v| v.contains("session/hydrate")).cloned().collect()
    };
    let reads = || -> Vec<String> {
        core.params_of("session/hydrate")
            .into_iter()
            .filter(|p| messages_read(p))
            .map(|p| p["session_id"].as_str().unwrap_or("").to_owned())
            .collect()
    };
    let settle = |what: &str, id: &str| {
        let what = what.to_owned();
        let id = id.to_owned();
        let conv = conv.clone();
        let hydrate_verdicts = hydrate_verdicts.clone();
        let reads = reads.clone();
        async move {
            for _ in 0..750 {
                if conv.store.active_session().as_deref() == Some(id.as_str()) && conv.history(&id) == History::Ready {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            panic!(
                "{what}: {id} never left {:?} — the server answered {} history reads ({:?}), the client took {} ({:?}); \
                 {id}'s history-read queue: {} read(s) still waiting",
                conv.history(&id),
                reads().len(),
                reads(),
                hydrate_verdicts().len(),
                hydrate_verdicts(),
                conv.history_reads_in_flight(&id)
            );
        }
    };
    conv.open_session(b, Some(CWD.to_owned())).await.expect("open B");
    settle("B's history behind a flood", b).await;
    conv.open_session(b, Some(CWD.to_owned())).await.expect("re-open B");
    settle("B's re-read behind a flood", b).await;
    conv.open_session(c, Some(CWD.to_owned())).await.expect("open C");
    settle("C's history behind a flood", c).await;
    pace.store(0, Ordering::Relaxed);
    quiet().await;
    // Every history reply arrived, committed, in the server's order.
    assert_eq!(reads(), vec![a.clone(), b.to_owned(), b.to_owned(), c.to_owned()], "the reads the server answered");
    assert_eq!(
        hydrate_verdicts(),
        vec!["Other(\"session/hydrate\")".to_owned(); 4],
        "every reply committed — none lost, none judged stale"
    );
    // The per-Session history-read queue is in step: nothing left waiting.
    for id in [a.as_str(), b, c] {
        assert_eq!(conv.history_reads_in_flight(id), 0, "{id}'s history-read queue");
    }
    quit(&conv);
}
