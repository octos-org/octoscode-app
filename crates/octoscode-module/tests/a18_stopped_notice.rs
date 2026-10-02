//! A18 — one Stop draws ONE "Turn stopped" notice, on the production path:
//! the real WS transport, the real `Conversation` (the composer's submit, the
//! Stop's `turn/interrupt`, the per-open `session/hydrate`), and a fake Core
//! that keeps what octos keeps.
//!
//! The live smoke (docs/ux/a15-live/smoke/06-stopped.png) showed the stopped
//! turn's notice inside the turn AND a second identical one below it. The
//! smoke's own trace says where the second came from: the startup hydrate
//! returned NO durable rows (the serve's session store had been reset) while
//! the replay window still held an earlier run's turns — four completed ones
//! and a stopped one (`01a0fc1d-1349…`, "turn interrupted by client"). The
//! history fold restored that stop's notice although the transcript holds
//! nothing of its era; on that build it drew below every live turn, so the
//! one real Stop showed two notices.
//!
//! The fake Core here persists rows the octos way (r43a shapes, A15's model):
//! a completed turn's prompt, tool step and answer; NOTHING for a stopped turn
//! but its retained `turn_terminal` record; and `reset_store` empties the
//! durable rows while the replay window outlives them (the live serve).
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octos_app_transport::{OutboundCommand, TransportEvent};
use octoscode_module::flow::Conversation;
use octoscode_module::screens::board3::rows::{notice_parts, timeline, TRow};
use octoscode_store::EntryKind;

const PROFILE: &str = "a18";
const CWD: &str = "/home/user/a18-ws";

#[derive(Default, Clone)]
struct Persisted {
    rows: Vec<Value>,
    tool_envs: Vec<Value>,
    projection_envs: Vec<Value>,
    thread_seq: BTreeMap<String, u64>,
}

#[derive(Default)]
struct CoreState {
    sessions: BTreeMap<String, Persisted>,
    seen: Vec<(String, Value)>,
    cursor: u64,
    /// The stoppable turn in flight: (session, turn, next per-thread seq).
    streaming: Option<(String, String, u64)>,
}

struct Core {
    base: String,
    st: Arc<Mutex<CoreState>>,
}

type Sink = Arc<tokio::sync::Mutex<futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>, Message>>>;

async fn send_all(tx: Sink, frames: Vec<Value>) {
    for f in frames {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let _ = tx.lock().await.send(Message::Text(f.to_string().into())).await;
    }
}

fn notification(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
}

fn opened(session: &str) -> Value {
    json!({"opened": {
        "session_id": session, "active_profile_id": PROFILE, "workspace_root": CWD,
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
                    let tx: Sink = Arc::new(tokio::sync::Mutex::new(tx));
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
                        let reply = |result: Value| json!({"jsonrpc": "2.0", "id": v["id"].clone(), "result": result});
                        let (result, then) = match method.as_str() {
                            "session/open" => (opened(&session), Vec::new()),
                            "turn/start" => {
                                let turn = p["turn_id"].as_str().unwrap_or_default().to_owned();
                                let prompt = p["input"][0]["text"].as_str().unwrap_or_default().to_owned();
                                (json!({"accepted": true}), Self::turn(&st, &session, &turn, &prompt))
                            }
                            "turn/interrupt" => (json!({"interrupted": true}), Self::interrupt(&st)),
                            "session/hydrate" => {
                                let s = st.lock().unwrap();
                                let p = s.sessions.get(&session).cloned().unwrap_or_default();
                                let mut r = json!({
                                    "session_id": session, "cursor": {"stream": session, "seq": s.cursor},
                                    "messages": p.rows, "projection_thread_sequences": p.thread_seq,
                                });
                                if !p.tool_envs.is_empty() {
                                    r["replayed_tool_envelopes"] = json!(p.tool_envs);
                                }
                                if !p.projection_envs.is_empty() {
                                    r["replayed_projection_envelopes"] = json!(p.projection_envs);
                                }
                                (r, Vec::new())
                            }
                            "session/list" => (json!({"sessions": [], "workspace_root": CWD, "profile_id": PROFILE}), Vec::new()),
                            _ => (json!({}), Vec::new()),
                        };
                        let _ = tx.lock().await.send(Message::Text(reply(result).to_string().into())).await;
                        if !then.is_empty() {
                            tokio::spawn(send_all(tx.clone(), then));
                        }
                    }
                });
            }
        });
        me
    }

    fn env(session: &str, turn: &str, seq: u64, cursor: u64, payload: Value) -> Value {
        json!({"session_id": session, "thread_id": turn, "turn_id": turn, "seq": seq,
               "cursor": {"stream": session, "seq": cursor}, "payload": payload})
    }

    /// A prompt starting with "stop:" streams a story and waits for Stop;
    /// any other prompt is a tool-calling turn that completes (A15's shape).
    fn turn(st: &Arc<Mutex<CoreState>>, session: &str, turn: &str, prompt: &str) -> Vec<Value> {
        let mut s = st.lock().unwrap();
        let base = s.cursor;
        let started = notification("turn/started", json!({"session_id": session, "turn_id": turn, "timestamp": "2026-10-02T09:00:00Z"}));
        if prompt.starts_with("stop:") {
            s.cursor += 4;
            s.streaming = Some((session.to_owned(), turn.to_owned(), 5));
            let segment = format!("{turn}:assistant:iteration:1");
            return vec![
                started,
                notification("projection/envelope", Self::env(session, turn, 1, base + 1, json!({"type": "user_message", "data": {"text": prompt}}))),
                notification("projection/envelope", Self::env(session, turn, 2, base + 2, json!({"type": "assistant_delta", "data": {"text": "The lighthouse keeper", "assistant_segment_id": segment}}))),
                notification("projection/envelope", Self::env(session, turn, 3, base + 3, json!({"type": "assistant_delta", "data": {"text": " watched the storm", "assistant_segment_id": segment}}))),
                notification("projection/envelope", Self::env(session, turn, 4, base + 4, json!({"type": "assistant_delta", "data": {"text": " roll in.", "assistant_segment_id": segment}}))),
            ];
        }
        s.cursor += 7;
        let call = format!("call_{}", &turn[turn.len().saturating_sub(4)..]);
        let answer = format!("Answer to: {prompt}");
        let output = "1| fn main() { println!(\"5\"); }".to_owned();
        let segment = format!("{turn}:assistant:iteration:2");
        let envs = vec![
            Self::env(session, turn, 1, base + 1, json!({"type": "user_message", "data": {"text": prompt}})),
            Self::env(session, turn, 2, base + 2, json!({"type": "tool_start", "data": {"tool_call_id": call, "name": "read_file", "arguments_preview": "path: \"main.rs\""}})),
            Self::env(session, turn, 3, base + 3, json!({"type": "tool_end", "data": {"tool_call_id": call, "status": "complete", "output_preview": output, "duration_ms": 3}})),
            Self::env(session, turn, 4, base + 4, json!({"type": "assistant_delta", "data": {"text": answer, "assistant_segment_id": segment}})),
            Self::env(session, turn, 5, base + 5, json!({"type": "assistant_persisted", "data": {"text": answer, "assistant_segment_id": segment, "meta": {"message_id": format!("{session}:{turn}:answer"), "persisted_at": "2026-10-02T09:00:01Z"}}})),
            Self::env(session, turn, 6, base + 6, json!({"type": "turn_terminal", "data": {"outcome": "completed"}})),
        ];
        let p = s.sessions.entry(session.to_owned()).or_default();
        let n = p.rows.len() as u64;
        let row = |k: u64, role: &str, content: &str| {
            json!({"seq": n + k, "role": role, "content": content, "thread_id": turn,
                   "persisted_at": "2026-10-02T09:00:00Z", "message_id": format!("{session}:{}", n + k)})
        };
        p.rows.push(row(0, "user", prompt));
        p.rows.push(row(1, "assistant", ""));
        p.rows.push(row(2, "tool", &output));
        p.rows.push(row(3, "assistant", &answer));
        p.tool_envs.push(envs[1].clone());
        p.tool_envs.push(envs[2].clone());
        p.projection_envs.push(envs[5].clone());
        p.thread_seq.insert(turn.to_owned(), 6);
        std::iter::once(started).chain(envs.into_iter().map(|e| notification("projection/envelope", e))).collect()
    }

    /// `turn/interrupt`: the stopped turn's terminal — and all Core keeps of
    /// it (no row: octos persists nothing for an interrupted turn).
    fn interrupt(st: &Arc<Mutex<CoreState>>) -> Vec<Value> {
        let mut s = st.lock().unwrap();
        let Some((session, turn, seq)) = s.streaming.take() else { return Vec::new() };
        s.cursor += 1;
        let terminal = Self::env(&session, &turn, seq, s.cursor, json!({"type": "turn_terminal", "data": {
            "outcome": "interrupted", "error": {"code": "interrupted", "message": "turn interrupted by client"}}}));
        let p = s.sessions.entry(session.clone()).or_default();
        p.projection_envs.push(terminal.clone());
        p.thread_seq.insert(turn, seq);
        vec![notification("projection/envelope", terminal)]
    }

    /// The session store is reset (the live serve's copied data): the durable
    /// rows are gone, the replay window — Core's ledger — is not.
    fn reset_store(&self) {
        for p in self.st.lock().unwrap().sessions.values_mut() {
            p.rows.clear();
        }
    }

    fn hydrates_for(&self, session: &str) -> usize {
        self.st.lock().unwrap().seen.iter().filter(|(m, p)| m == "session/hydrate" && p["session_id"] == json!(session)).count()
    }

    fn interrupts(&self) -> usize {
        self.st.lock().unwrap().seen.iter().filter(|(m, _)| m == "turn/interrupt").count()
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

async fn settle(conv: &Conversation, ev: &mut Events) {
    while let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(300), ev.recv()).await {
        conv.on_event(evt);
    }
}

/// The app's startup path: connect, open the workspace, the open's hydrate.
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

/// The composer's send (Enter / the send button): `set_draft` + `submit_draft`.
async fn submit(conv: &Arc<Conversation>, ev: &mut Events, prompt: &str) -> String {
    conv.set_draft(prompt);
    let h = {
        let c = conv.clone();
        tokio::spawn(async move { c.submit_draft().await })
    };
    let mut turn = String::new();
    for _ in 0..200 {
        if h.is_finished() {
            turn = h.await.unwrap().expect("turn/start");
            break;
        }
        if let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(50), ev.recv()).await {
            conv.on_event(evt);
        }
    }
    assert!(!turn.is_empty(), "the composer started a turn for {prompt:?}");
    turn
}

async fn complete(conv: &Arc<Conversation>, ev: &mut Events, prompt: &str) -> String {
    let turn = submit(conv, ev, prompt).await;
    assert!(fold_until(conv, ev, 10, |c| c.store.domains.turn.terminal(&turn).is_some()).await, "{prompt:?} completes");
    settle(conv, ev).await;
    turn
}

/// A streaming turn, stopped by the user (the Stop button's `turn/interrupt`).
async fn stop_mid_stream(conv: &Arc<Conversation>, ev: &mut Events, core: &Core, prompt: &str) -> String {
    let turn = submit(conv, ev, prompt).await;
    let streaming = |c: &Conversation| {
        let s = c.session_id();
        c.store.domains.session.timeline.entries(&s).iter().any(|e| {
            e.kind == EntryKind::ASSISTANT_TEXT && e.turn_id.as_deref() == Some(turn.as_str()) && e.text.contains("storm")
        })
    };
    assert!(fold_until(conv, ev, 10, streaming).await, "the story streams");
    let before = core.interrupts();
    let h = {
        let (c, t) = (conv.clone(), turn.clone());
        tokio::spawn(async move { c.interrupt(&t).await })
    };
    assert!(fold_until(conv, ev, 10, |_| core.interrupts() > before).await, "Stop sends turn/interrupt");
    h.await.unwrap().expect("turn/interrupt");
    assert!(
        fold_until(conv, ev, 10, |c| c.store.domains.turn.terminal(&turn).as_deref() == Some("interrupted")).await,
        "the interrupted terminal settles the turn"
    );
    settle(conv, ev).await;
    turn
}

/// The composed transcript's notices: (row index, turn, title, body).
fn drawn_notices(conv: &Conversation) -> Vec<(usize, Option<String>, String, String)> {
    let session = conv.session_id();
    let entries = conv.store.domains.session.timeline.entries(&session);
    timeline(&conv.store, false)
        .iter()
        .enumerate()
        .filter_map(|(i, r)| match r {
            TRow::Notice(id) => entries.iter().find(|e| e.id == *id).map(|e| {
                let (title, body) = notice_parts(e);
                (i, e.turn_id.clone(), title, body)
            }),
            _ => None,
        })
        .collect()
}

/// Where a turn's rows sit in the composed transcript (first, last).
fn turn_span(conv: &Conversation, turn: &str) -> Option<(usize, usize)> {
    let rows = timeline(&conv.store, false);
    let at: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| matches!(r, TRow::Base(b) if b.turn.as_deref() == Some(turn)))
        .map(|(i, _)| i)
        .collect();
    Some((*at.first()?, *at.last()?))
}

fn quit(conv: &Conversation) {
    let _ = conv.command_sender().try_send(OutboundCommand::Disconnect);
}

/// The live smoke's sequence: an earlier run completed a turn and stopped
/// another; the session store was reset (the replay window kept both); this
/// run completes a turn and stops one. ONE notice — this run's — drawn in
/// its own turn; still one after a re-open and after a restart, in place.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_stop_draws_one_notice_even_when_the_replay_window_outlives_a_reset_store() {
    let core = Core::start().await;
    let (earlier, mut ev0) = launch(&core).await;
    let session = earlier.session_id();
    complete(&earlier, &mut ev0, "Create judge.txt").await;
    let old_stop = stop_mid_stream(&earlier, &mut ev0, &core, "stop: an earlier story").await;
    quit(&earlier);
    core.reset_store();

    // The startup hydrate: no rows, the earlier era's terminals retained.
    let (app, mut ev) = launch(&core).await;
    assert_eq!(app.session_id(), session);
    assert!(
        drawn_notices(&app).is_empty(),
        "a stop whose era the transcript lost is not noted: {:?}",
        drawn_notices(&app)
    );
    let done = complete(&app, &mut ev, "What does main.rs print?").await;
    let stopped = stop_mid_stream(&app, &mut ev, &core, "stop: a 400-word story about a lighthouse keeper").await;
    assert_ne!(stopped, old_stop);

    // One Stop, one notice: in the stopped turn, after its streamed prose.
    let notices = drawn_notices(&app);
    assert_eq!(notices.len(), 1, "one notice for one Stop: {notices:?}");
    let (at, turn, title, body) = notices[0].clone();
    assert_eq!(turn.as_deref(), Some(stopped.as_str()));
    assert_eq!((title.as_str(), body.as_str()), ("Turn stopped", "turn interrupted by client"));
    let (first, last) = turn_span(&app, &stopped).expect("the stopped turn's rows");
    assert!(first < at && at <= last + 1, "drawn inside the stopped turn (rows {first}..{last}, notice {at})");
    let (_, done_last) = turn_span(&app, &done).expect("the completed turn's rows");
    assert!(done_last < first, "the completed turn comes first");

    // A re-open (New chat, then the session again): its hydrate returns the
    // same terminal — still one notice.
    let other = {
        let c = app.clone();
        let h = tokio::spawn(async move { c.new_chat(Some(CWD.to_owned())).await });
        assert!(fold_until(&app, &mut ev, 10, |c| c.session_id() != session).await);
        h.await.unwrap().expect("new chat")
    };
    settle(&app, &mut ev).await;
    assert_ne!(other, session);
    let before = core.hydrates_for(&session);
    let h = {
        let (c, s) = (app.clone(), session.clone());
        tokio::spawn(async move { c.open_session(&s, Some(CWD.to_owned())).await })
    };
    assert!(fold_until(&app, &mut ev, 10, |_| core.hydrates_for(&session) > before).await, "the re-open hydrates");
    h.await.unwrap().expect("re-open");
    settle(&app, &mut ev).await;
    let notices = drawn_notices(&app);
    assert_eq!(notices.len(), 1, "a re-open never adds a notice: {notices:?}");
    assert_eq!(notices[0].1.as_deref(), Some(stopped.as_str()));
    quit(&app);

    // A15's restart: the stopped turn persisted nothing, its notice is still
    // shown — once, after the completed turn it followed.
    let (again, _ev2) = launch(&core).await;
    let notices = drawn_notices(&again);
    assert_eq!(notices.len(), 1, "after a restart, one notice: {notices:?}");
    let (at, turn, title, _) = notices[0].clone();
    assert_eq!(turn.as_deref(), Some(stopped.as_str()));
    assert_eq!(title, "Turn stopped");
    let (_, done_last) = turn_span(&again, &done).expect("the completed turn is back");
    assert!(done_last < at, "in place: after the turn it followed ({done_last} < {at})");
    quit(&again);
}

/// The live terminal and a hydrate of the same turn (a re-hydrate while the
/// app runs) name one `terminal:<turn>` row: still one notice.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_live_stop_and_its_hydrated_terminal_are_one_notice() {
    let core = Core::start().await;
    let (app, mut ev) = launch(&core).await;
    let session = app.session_id();
    complete(&app, &mut ev, "Warm up").await;
    let stopped = stop_mid_stream(&app, &mut ev, &core, "stop: a long story").await;
    assert_eq!(drawn_notices(&app).len(), 1);
    let before = core.hydrates_for(&session);
    assert!(app.request_hydrate(&session), "a hydrate is asked");
    assert!(fold_until(&app, &mut ev, 10, |_| core.hydrates_for(&session) > before).await);
    settle(&app, &mut ev).await;
    let notices = drawn_notices(&app);
    assert_eq!(notices.len(), 1, "the hydrated terminal is the live notice's row: {notices:?}");
    assert_eq!(notices[0].1.as_deref(), Some(stopped.as_str()));
    let stored = app
        .store
        .domains
        .session
        .timeline
        .of_kind(&session, EntryKind::SYSTEM_NOTICE)
        .into_iter()
        .filter(|e| e.turn_id.as_deref() == Some(stopped.as_str()))
        .count();
    assert_eq!(stored, 1, "one stored notice for the turn");
    // The stopped turn keeps its streamed prose above the notice.
    assert!(app
        .store
        .domains
        .session
        .timeline
        .entries(&session)
        .iter()
        .any(|e| e.kind == EntryKind::ASSISTANT_TEXT && e.turn_id.as_deref() == Some(stopped.as_str())));
    quit(&app);
}
