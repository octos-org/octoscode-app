//! A15 — every Session open loads its history, on the production path: the
//! real WS transport, the real `Conversation` (link + flow), and a fake Core
//! that persists each Session the way octos does.
//!
//! Web reference: a candidate open is `openSession` then `hydrateSession`
//! (`features/session/candidate-session.ts:166-183`), whose result rebuilds
//! the transcript (`session-record-manager.ts:1038-1090`,
//! `timeline/model.ts:42-240` `timelineFromHydrate`). The live smoke found the
//! native app opening a restarted Session as an empty "New chat" (only a
//! reconnect re-hydrated).
//!
//! The fake Core keeps what octos keeps (recorded a6ea8505 shapes,
//! `r43a-recovery`): durable rows with `thread_id` = the turn UUID and no
//! `turn_id`; a tool-calling step persisted as an EMPTY assistant row with
//! its reasoning, then the `tool` row (its output), then the answer; the
//! session file's title is its first prompt; and, while the server runs, the
//! turn's tool envelopes retained for `replayed_tool_envelopes`.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octos_app_transport::{OutboundCommand, TransportEvent};
use octoscode_module::components::ItemKind;
use octoscode_module::flow::Conversation;
use octoscode_store::EntryKind;

const PROFILE: &str = "a15";
const CWD: &str = "/home/user/a15-ws";

#[derive(Default, Clone)]
struct Persisted {
    /// Durable rows (`HydratedMessage` shape), seq = index.
    rows: Vec<Value>,
    /// Retained tool envelopes (lost when the server restarts).
    tool_envs: Vec<Value>,
    /// Retained canonical records (`replayed_projection_envelopes`).
    projection_envs: Vec<Value>,
    /// Per-thread last projection seq.
    thread_seq: BTreeMap<String, u64>,
    title: Option<String>,
    last_prompt: Option<String>,
}

#[derive(Default)]
struct CoreState {
    sessions: BTreeMap<String, Persisted>,
    seen: Vec<(String, Value)>,
    cursor: u64,
    calls: u64,
}

/// A fake Core: per-Session durable history, a scripted tool-calling turn.
struct Core {
    base: String,
    st: Arc<Mutex<CoreState>>,
}

fn opened(session: &str, cwd: &str) -> Value {
    json!({"opened": {
        "session_id": session, "active_profile_id": PROFILE, "workspace_root": cwd,
        "cursor": {"stream": session, "seq": 1},
        "capabilities": {
            "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
            "capabilities_schema_version": 2,
            "supported_methods": ["session/open", "session/hydrate", "session/list", "turn/start", "turn/interrupt"],
            "supported_notifications": [],
            "supported_features": ["state.session_hydrate.v1", "projection.envelope.v2", "session.workspace_cwd.v1"]
        }
    }})
}

impl Core {
    async fn start() -> Arc<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let me = Arc::new(Self { base, st: Arc::new(Mutex::new(CoreState::default())) });
        let st = me.st.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let st = st.clone();
                tokio::spawn(async move {
                    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
                    let (tx, mut rx) = ws.split();
                    let tx = Arc::new(tokio::sync::Mutex::new(tx));
                    while let Some(Ok(msg)) = rx.next().await {
                        let Message::Text(text) = msg else { continue };
                        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                        if v.get("id").is_none() {
                            continue;
                        }
                        let method = v["method"].as_str().unwrap_or("").to_owned();
                        let p = v["params"].clone();
                        let session = p["session_id"].as_str().unwrap_or("").to_owned();
                        st.lock().unwrap().seen.push((method.clone(), p.clone()));
                        let result = match method.as_str() {
                            "session/open" => opened(&session, p["cwd"].as_str().unwrap_or(CWD)),
                            "turn/start" => {
                                let turn = p["turn_id"].as_str().unwrap_or_default().to_owned();
                                let prompt = p["input"][0]["text"].as_str().unwrap_or_default().to_owned();
                                let frames = Self::turn(&st, &session, &turn, &prompt);
                                let ok = json!({"jsonrpc": "2.0", "id": v["id"].clone(), "result": {"accepted": true}});
                                let tx2 = tx.clone();
                                tokio::spawn(async move {
                                    let _ = tx2.lock().await.send(Message::Text(ok.to_string().into())).await;
                                    for (m, params) in frames {
                                        tokio::time::sleep(Duration::from_millis(25)).await;
                                        let frame = json!({"jsonrpc": "2.0", "method": m, "params": params});
                                        let _ = tx2.lock().await.send(Message::Text(frame.to_string().into())).await;
                                    }
                                });
                                continue;
                            }
                            "session/hydrate" => {
                                let s = st.lock().unwrap();
                                let p = s.sessions.get(&session).cloned().unwrap_or_default();
                                let mut r = json!({
                                    "session_id": session, "cursor": {"stream": session, "seq": s.cursor},
                                    "messages": p.rows,
                                    "projection_thread_sequences": p.thread_seq,
                                });
                                if !p.tool_envs.is_empty() {
                                    r["replayed_tool_envelopes"] = json!(p.tool_envs);
                                }
                                if !p.projection_envs.is_empty() {
                                    r["replayed_projection_envelopes"] = json!(p.projection_envs);
                                }
                                r
                            }
                            "session/list" => {
                                let s = st.lock().unwrap();
                                let rows: Vec<Value> = s
                                    .sessions
                                    .iter()
                                    .filter(|(_, p)| !p.rows.is_empty())
                                    .map(|(id, p)| json!({
                                        "id": id, "title": p.title, "last_prompt": p.last_prompt,
                                        "message_count": p.rows.len(), "updated_at": "2026-10-02T09:00:00Z", "active_turn": false
                                    }))
                                    .collect();
                                // The scoped catalog attests its scope (`SessionListResult`).
                                match (p["cwd"].as_str(), p["profile_id"].as_str()) {
                                    (Some(cwd), Some(profile)) => {
                                        json!({"sessions": rows, "workspace_root": cwd, "profile_id": profile})
                                    }
                                    // The legacy listing does not see cwd-scoped stores.
                                    _ => json!({"sessions": []}),
                                }
                            }
                            _ => json!({}),
                        };
                        let frame = json!({"jsonrpc": "2.0", "id": v["id"].clone(), "result": result});
                        let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
                    }
                });
            }
        });
        me
    }

    /// One tool-calling turn: what streams live, and what Core persists.
    fn turn(st: &Arc<Mutex<CoreState>>, session: &str, turn: &str, prompt: &str) -> Vec<(&'static str, Value)> {
        let mut s = st.lock().unwrap();
        s.calls += 1;
        let call = format!("call_{:02}_a15", s.calls);
        let answer = format!("Answer to: {prompt}");
        let output = format!("1| fn main() {{ println!(\"{}\"); }}", s.calls);
        let base = s.cursor;
        s.cursor += 8;
        let env = |seq: u64, payload: Value| {
            json!({"session_id": session, "thread_id": turn, "turn_id": turn, "seq": seq,
                   "cursor": {"stream": session, "seq": base + seq}, "payload": payload})
        };
        let frames = vec![
            ("turn/started", json!({"session_id": session, "turn_id": turn, "timestamp": "2026-10-02T09:00:00Z"})),
            ("projection/envelope", env(1, json!({"type": "user_message", "data": {"text": prompt}}))),
            ("projection/envelope", env(2, json!({"type": "reasoning_delta", "data": {"text": "Reading main.rs first."}}))),
            ("projection/envelope", env(3, json!({"type": "tool_start", "data": {"tool_call_id": call, "name": "read_file", "arguments_preview": "path: \"main.rs\""}}))),
            ("projection/envelope", env(4, json!({"type": "tool_end", "data": {"tool_call_id": call, "status": "complete", "output_preview": output, "duration_ms": 3}}))),
            ("projection/envelope", env(5, json!({"type": "assistant_delta", "data": {"text": answer, "assistant_segment_id": format!("{turn}:assistant:iteration:2")}}))),
            ("projection/envelope", env(6, json!({"type": "assistant_persisted", "data": {"text": answer, "assistant_segment_id": format!("{turn}:assistant:iteration:2"), "meta": {"message_id": format!("{session}:{turn}:answer"), "persisted_at": "2026-10-02T09:00:01Z"}}}))),
            ("projection/envelope", env(7, json!({"type": "turn_terminal", "data": {"outcome": "completed"}}))),
        ];
        let p = s.sessions.entry(session.to_owned()).or_default();
        let n = p.rows.len() as u64;
        let row = |k: u64, role: &str, content: &str| {
            json!({"seq": n + k, "role": role, "content": content, "thread_id": turn,
                   "persisted_at": "2026-10-02T09:00:00Z", "message_id": format!("{session}:{}", n + k)})
        };
        p.rows.push(row(0, "user", prompt));
        let mut step = row(1, "assistant", "");
        step["reasoning_content"] = json!("Reading main.rs first.");
        p.rows.push(step);
        p.rows.push(row(2, "tool", &output));
        p.rows.push(row(3, "assistant", &answer));
        p.tool_envs.push(frames[3].1.clone());
        p.tool_envs.push(frames[4].1.clone());
        // Core retains every thread's terminal record (even a compacted one's).
        p.projection_envs.push(frames[7].1.clone());
        p.thread_seq.insert(turn.to_owned(), 7);
        p.title.get_or_insert_with(|| prompt.chars().take(50).collect());
        p.last_prompt = Some(prompt.to_owned());
        frames
    }

    /// A turn that was stopped before Core persisted anything for it: only
    /// its `turn_terminal` record is retained (the live smoke's lighthouse
    /// story, recorded against octos a6ea8505).
    fn retain_stopped_turn(&self, session: &str, turn: &str) {
        let mut s = self.st.lock().unwrap();
        s.cursor += 1;
        let cursor = s.cursor;
        let p = s.sessions.entry(session.to_owned()).or_default();
        p.projection_envs.push(json!({
            "session_id": session, "thread_id": turn, "turn_id": turn, "seq": 1,
            "cursor": {"stream": session, "seq": cursor},
            "payload": {"type": "turn_terminal", "data": {"outcome": "interrupted"}}
        }));
    }

    /// The server restarted: durable rows stay, the replay window is gone.
    fn forget_replay(&self) {
        for p in self.st.lock().unwrap().sessions.values_mut() {
            p.tool_envs.clear();
            p.projection_envs.clear();
            p.thread_seq.clear();
        }
    }

    fn hydrates_for(&self, session: &str) -> usize {
        self.st.lock().unwrap().seen.iter().filter(|(m, p)| m == "session/hydrate" && p["session_id"] == json!(session)).count()
    }

    fn opens_of(&self, session: &str) -> usize {
        self.st.lock().unwrap().seen.iter().filter(|(m, p)| m == "session/open" && p["session_id"] == json!(session)).count()
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.st.lock().unwrap().seen.iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }
}

type Events = tokio::sync::mpsc::Receiver<TransportEvent>;

async fn fold_until(conv: &Conversation, ev: &mut Events, secs: u64, done: impl Fn(&Conversation) -> bool) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    while tokio::time::Instant::now() < deadline {
        if done(conv) {
            return true;
        }
        if let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(50), ev.recv()).await {
            conv.on_event(evt);
        }
    }
    done(conv)
}

/// Fold whatever is still in flight (until the stream is quiet).
async fn settle(conv: &Conversation, ev: &mut Events) {
    while let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(300), ev.recv()).await {
        conv.on_event(evt);
    }
}

/// The app's startup path: connect, `open_workspace(cwd)` (lib.rs), drain.
async fn launch(core: &Core) -> (Arc<Conversation>, Events) {
    let (conv, mut ev) = Conversation::connect(&core.base, "dummy", PROFILE, Some(CWD.to_owned()), None).expect("connect");
    let conv = Arc::new(conv);
    conv.attach();
    let opener = {
        let c = conv.clone();
        tokio::spawn(async move { c.open_workspace(Some(CWD.to_owned())).await })
    };
    assert!(fold_until(&conv, &mut ev, 10, |c| c.store.is_live()).await, "live");
    opener.await.unwrap().expect("open");
    let s = conv.session_id();
    assert!(fold_until(&conv, &mut ev, 10, |_| core.hydrates_for(&s) >= 1).await, "the startup open hydrates");
    settle(&conv, &mut ev).await;
    (conv, ev)
}

/// A sidebar click / Resume / switch: `open_session`, its hydrate folded.
async fn open(conv: &Arc<Conversation>, ev: &mut Events, core: &Core, id: &str) {
    let before = core.hydrates_for(id);
    let h = {
        let (c, id) = (conv.clone(), id.to_owned());
        tokio::spawn(async move { c.open_session(&id, Some(CWD.to_owned())).await })
    };
    assert!(fold_until(conv, ev, 10, |_| core.hydrates_for(id) > before).await, "the open of {id} hydrates");
    h.await.unwrap().expect("open");
    settle(conv, ev).await;
}

async fn run_turn(conv: &Arc<Conversation>, ev: &mut Events, prompt: &str) -> String {
    let h = {
        let (c, p) = (conv.clone(), prompt.to_owned());
        tokio::spawn(async move { c.start_turn(p).await })
    };
    let turn = h.await.unwrap().expect("turn/start");
    assert!(
        fold_until(conv, ev, 10, |c| c.store.domains.turn.terminal(&turn).is_some()).await,
        "the turn streamed to its terminal"
    );
    settle(conv, ev).await;
    turn
}

/// What the transcript SHOWS for the active Session: the row model the
/// window draws (`screen::timeline_rows_folded`), as (kind, text) pairs.
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

fn count(rows: &[(ItemKind, String)], kind: ItemKind) -> usize {
    rows.iter().filter(|(k, _)| *k == kind).count()
}

fn texts(rows: &[(ItemKind, String)], kind: ItemKind) -> Vec<String> {
    rows.iter().filter(|(k, _)| *k == kind).map(|(_, t)| t.clone()).collect()
}

/// The turn's tool cards: (title, output) as the tool row draws them.
fn tool_cards(conv: &Conversation, session: &str) -> Vec<(String, String)> {
    let calls = conv.store.domains.tool.calls();
    conv.store
        .domains
        .session
        .timeline
        .of_kind(session, EntryKind::TOOL_CALL)
        .into_iter()
        .map(|e| {
            let id = e.data.get("tool_call_id").and_then(|v| v.as_str()).map(str::to_owned);
            let output = id
                .as_ref()
                .and_then(|id| calls.iter().find(|c| &c.tool_call_id == id))
                .and_then(|c| c.output_preview.clone())
                .or_else(|| e.data.get("output").and_then(|v| v.as_str()).map(str::to_owned))
                .unwrap_or_default();
            (e.text.clone(), output)
        })
        .collect()
}

fn quit(conv: &Conversation) {
    let _ = conv.command_sender().try_send(OutboundCommand::Disconnect);
}

/// The live smoke's failure, fixed: an app that restarts (same server, same
/// Session) shows the Session's prompts, tool rows and answers at once — the
/// startup open hydrates — with each turn drawn once.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restart_then_reopen_shows_the_history() {
    let core = Core::start().await;
    // The first run: two tool-calling turns.
    let (first, mut ev1) = launch(&core).await;
    let session = first.session_id();
    run_turn(&first, &mut ev1, "What does main.rs print?").await;
    run_turn(&first, &mut ev1, "And why?").await;
    let live = shown(&first);
    assert_eq!(texts(&live, ItemKind::UserBubble), vec!["What does main.rs print?", "And why?"]);
    quit(&first);

    // The restart: a NEW app instance on the same server opens the Session.
    let (second, _ev2) = launch(&core).await;
    assert_eq!(second.session_id(), session, "the same Session comes back");
    assert_eq!(core.opens_of(&session), 2);
    assert_eq!(core.hydrates_for(&session), 2, "one hydrate per open, the web's open -> hydrate");
    let rows = shown(&second);
    assert_eq!(texts(&rows, ItemKind::UserBubble), vec!["What does main.rs print?", "And why?"], "each prompt, once, in order");
    assert_eq!(
        texts(&rows, ItemKind::AssistantProse),
        vec!["Answer to: What does main.rs print?", "Answer to: And why?"],
        "each answer, once, under its own prompt"
    );
    assert_eq!(count(&rows, ItemKind::ToolCell), 2, "one tool row per turn");
    // The server kept its replay window: the tool rows are the named cards
    // (the web folds `replayed_tool_envelopes`), the persisted outputs are
    // not repeated as extra rows.
    let cards = tool_cards(&second, &session);
    assert_eq!(cards.len(), 2, "{cards:?}");
    assert!(cards.iter().all(|(title, out)| title == "read_file" && out.starts_with("1| fn main()")), "{cards:?}");
    quit(&second);

    // A server restart evicts the replay window: the persisted tool rows are
    // the web's "Tool output" rows, still one per call, with their output.
    core.forget_replay();
    let (third, _ev3) = launch(&core).await;
    let rows = shown(&third);
    assert_eq!(count(&rows, ItemKind::UserBubble), 2);
    assert_eq!(count(&rows, ItemKind::AssistantProse), 2);
    assert_eq!(count(&rows, ItemKind::ToolCell), 2);
    let cards = tool_cards(&third, &session);
    assert!(cards.iter().all(|(title, out)| title == "Tool output" && out.starts_with("1| fn main()")), "{cards:?}");
    assert!(
        third.store.domains.session.timeline.of_kind(&session, EntryKind::SYSTEM_NOTICE).is_empty(),
        "a tool output is a tool row, never a 'System' notice"
    );
    quit(&third);
}

/// Sidebar clicks back and forth between two Sessions: each open hydrates
/// once, and each history shows once — the turns this client streamed live
/// are recognised (Core's thread id = turn id), never appended again.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switching_sessions_back_and_forth_shows_each_history_once() {
    let core = Core::start().await;
    let (conv, mut ev) = launch(&core).await;
    let a = conv.session_id();
    run_turn(&conv, &mut ev, "alpha: what does main.rs print?").await;
    // New chat: a fresh Session, opened (and hydrated: empty) like any other.
    let b = {
        let c = conv.clone();
        let h = tokio::spawn(async move { c.new_chat(Some(CWD.to_owned())).await });
        assert!(fold_until(&conv, &mut ev, 10, |c| c.session_id() != a).await);
        let b = h.await.unwrap().expect("new chat");
        assert!(fold_until(&conv, &mut ev, 10, |_| core.hydrates_for(&b) >= 1).await, "New chat hydrates too");
        settle(&conv, &mut ev).await;
        b
    };
    run_turn(&conv, &mut ev, "beta: and why?").await;

    for round in 0..2 {
        open(&conv, &mut ev, &core, &a).await;
        let rows = shown(&conv);
        assert_eq!(texts(&rows, ItemKind::UserBubble), vec!["alpha: what does main.rs print?"], "round {round}: A's prompt once");
        assert_eq!(texts(&rows, ItemKind::AssistantProse), vec!["Answer to: alpha: what does main.rs print?"], "round {round}");
        assert_eq!(count(&rows, ItemKind::ToolCell), 1, "round {round}: A's tool row once");
        assert_eq!(tool_cards(&conv, &a).len(), 1, "round {round}: one card in A's transcript");

        open(&conv, &mut ev, &core, &b).await;
        let rows = shown(&conv);
        assert_eq!(texts(&rows, ItemKind::UserBubble), vec!["beta: and why?"], "round {round}: B's prompt once");
        assert_eq!(texts(&rows, ItemKind::AssistantProse), vec!["Answer to: beta: and why?"], "round {round}");
        assert_eq!(count(&rows, ItemKind::ToolCell), 1, "round {round}: B's tool row once");
        assert_eq!(tool_cards(&conv, &b).len(), 1, "round {round}: one card in B's transcript");
    }
    // Neither transcript gained a copy of its own rows.
    for s in [&a, &b] {
        let tl = &conv.store.domains.session.timeline;
        assert_eq!(tl.of_kind(s, EntryKind::USER_MESSAGE).len(), 1, "{s}");
        let answers: Vec<String> =
            tl.of_kind(s, EntryKind::ASSISTANT_TEXT).into_iter().map(|e| e.text).filter(|t| !t.is_empty()).collect();
        assert_eq!(answers.len(), 1, "{s}: {answers:?}");
    }
    // Every open asked for its history exactly once (the web: one hydrate per
    // candidate open), and no hydrate went out without an open.
    assert_eq!(core.hydrates_for(&a), core.opens_of(&a), "A: one hydrate per open");
    assert_eq!(core.hydrates_for(&b), core.opens_of(&b), "B: one hydrate per open");
    assert_eq!(core.opens_of(&a), 3);
    assert_eq!(core.opens_of(&b), 3);
    for p in core.params_of("session/hydrate") {
        assert_eq!(p["include"], json!(["messages"]), "the canonical messages read: {p}");
    }
    quit(&conv);
}

/// The sidebar row and the header name a Session the way the web does: the
/// server's catalog row (`title`, else `last_prompt`), listed per workspace
/// (`session/list {cwd, profile_id}`) and re-listed when the opened Session
/// changes and when a turn starts or ends (`App.tsx:753-760`). octos titles a
/// Session from its first prompt; the legacy unscoped listing never sees a
/// workspace's Sessions, so the row stayed "New chat" after several turns.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sessions_title_is_the_servers_catalog_row_after_its_first_turn() {
    use octoscode_module::screens::sidebar;
    let core = Core::start().await;
    let (conv, mut ev) = launch(&core).await;
    let session = conv.session_id();
    let label = |c: &Conversation| {
        c.store.sessions().into_iter().find(|s| s.id == session).and_then(|s| s.label_stem())
    };
    assert_eq!(label(&conv), None, "an empty Session has no title yet (\"New chat\")");
    run_turn(&conv, &mut ev, "What does main.rs print?").await;
    run_turn(&conv, &mut ev, "And why?").await;
    // The catalog re-list lands off the event path.
    assert!(
        fold_until(&conv, &mut ev, 10, |c| label(c).as_deref() == Some("What does main.rs print?")).await,
        "the row takes the server's title: {:?}",
        label(&conv)
    );
    // The web's catalog request: the opened workspace under the opened Profile.
    let scoped: Vec<Value> = core.params_of("session/list").into_iter().filter(|p| p.get("cwd").is_some()).collect();
    assert!(!scoped.is_empty(), "the catalog is read per workspace");
    assert!(scoped.iter().all(|p| *p == json!({"cwd": CWD, "profile_id": PROFILE})), "{scoped:?}");
    // What the sidebar draws.
    let p = sidebar::project_with(&conv.store, &sidebar::SidebarUi::default(), 0, &[]);
    let titles: Vec<String> = p
        .rows
        .iter()
        .filter_map(|r| match r {
            sidebar::Row::Session { title, .. } => Some(title.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(titles, vec!["What does main.rs print?".to_owned()], "one row, titled — never \"New chat\"");
    // A restarted app shows the title at once (the open re-lists).
    quit(&conv);
    let (again, mut ev2) = launch(&core).await;
    assert!(
        fold_until(&again, &mut ev2, 5, |c| c.store.sessions().iter().any(|s| s.id == session && s.label_stem().as_deref() == Some("What does main.rs print?"))).await,
        "the restarted app names the Session from the catalog"
    );
    quit(&again);
}

/// The live smoke's last turn was stopped mid-stream: Core persisted no row
/// for it, only its `turn_terminal` record. A restarted app shows that truth
/// — the turn's "Turn stopped" notice, as the live terminal drew it (the web
/// renders a stopped turn with no persisted message, `model.ts:240-270`) —
/// once, under the same `terminal:<turn>` id across re-opens.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stopped_turn_keeps_its_stopped_notice_once_after_a_restart() {
    let core = Core::start().await;
    let (first, mut ev1) = launch(&core).await;
    let session = first.session_id();
    run_turn(&first, &mut ev1, "What does main.rs print?").await;
    quit(&first);
    let stopped = "01920000-0000-7000-8000-0000000000aa";
    core.retain_stopped_turn(&session, stopped);

    let (second, mut ev2) = launch(&core).await;
    let notices = |c: &Conversation| -> Vec<(String, String)> {
        c.store
            .domains
            .session
            .timeline
            .of_kind(&session, EntryKind::SYSTEM_NOTICE)
            .into_iter()
            .filter(|e| e.turn_id.as_deref() == Some(stopped))
            .map(|e| octoscode_module::screens::board3::rows::notice_parts(&e))
            .collect()
    };
    assert_eq!(notices(&second), vec![("Turn stopped".to_owned(), String::new())], "the stopped turn is noted");
    // The completed turn is still drawn once, above it.
    let rows = shown(&second);
    assert_eq!(texts(&rows, ItemKind::UserBubble), vec!["What does main.rs print?"]);
    // A re-open (a sidebar click away and back) never notes it twice.
    let b = {
        let c = second.clone();
        let h = tokio::spawn(async move { c.new_chat(Some(CWD.to_owned())).await });
        assert!(fold_until(&second, &mut ev2, 10, |c| c.session_id() != session).await);
        h.await.unwrap().expect("new chat")
    };
    settle(&second, &mut ev2).await;
    assert_ne!(b, session);
    open(&second, &mut ev2, &core, &session).await;
    assert_eq!(notices(&second).len(), 1, "one notice per stopped turn");
    // The next prompt streams AFTER the stopped turn: the notice stays above it.
    let later = run_turn(&second, &mut ev2, "And now?").await;
    let order = |c: &Conversation| -> (usize, usize, usize) {
        use octoscode_module::screens::board3::rows::{timeline, TRow};
        let rows = timeline(&c.store, false);
        let bubble = |turn: &str| {
            rows.iter()
                .position(|r| matches!(r, TRow::Base(b) if b.kind == ItemKind::UserBubble && b.turn.as_deref() == Some(turn)))
                .expect("bubble")
        };
        let notice = rows
            .iter()
            .position(|r| match r {
                TRow::Notice(id) => c.store.domains.session.timeline.entries(&session).iter().any(|e| e.id == *id && e.turn_id.as_deref() == Some(stopped)),
                _ => false,
            })
            .expect("notice");
        let first_turn = c.store.domains.session.timeline.of_kind(&session, EntryKind::USER_MESSAGE)[0].turn_id.clone().unwrap();
        (bubble(&first_turn), notice, bubble(&later))
    };
    let (a, n, c) = order(&second);
    assert!(a < n && n < c, "live: first turn, stopped notice, later turn ({a}, {n}, {c})");
    quit(&second);
    // A cold restart folds every row first and the notice after them: it is
    // moved back before the next turn the transcript holds (stream order of
    // the retained terminals), never left below the later turn.
    let (third, _ev3) = launch(&core).await;
    let (a, n, c) = order(&third);
    assert!(a < n && n < c, "cold restart: first turn, stopped notice, later turn ({a}, {n}, {c})");
    assert_eq!(notices(&third).len(), 1);
    quit(&third);
}
