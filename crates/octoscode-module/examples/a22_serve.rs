//! A22 — the fixture server for the four Session rows' click walk
//! (`tools/walk/a22_sessions_walk.py`): one port answering the UI protocol
//! the way octos a6ea8505 does (recorded shapes: `r43a-recovery` for the
//! hydrate, `r23-conversation` for the question, `r22-live` for the turn
//! envelopes), no model, no machine paths.
//!
//! * **row 228** — `session/list {cwd, profile_id}` ATTESTS its scope
//!   (`workspace_root` + `profile_id`) and lists the full Sessions of `a22`
//!   plus three rows the app must never project: another profile's, a bare
//!   id, a legacy `a22:legacy`. The legacy `{}` listing names everything and
//!   attests nothing.
//! * **row 203** — the first history read of "Review the hydrate path"
//!   (`a22:api:history`) is answered 1.5 s late, and BEFORE it the socket
//!   carries another client's live turn in that Session (and a replayed frame
//!   of a turn the history already holds); the rest of that turn follows the
//!   history.
//! * **row 216** — `a22:main` opens with `reasoning_effort: "high"`; a prompt
//!   containing `[refuse]` is refused (`-32000`), `[busy]` runs ~18 s.
//! * **row 236** — prompts drive background work: `[slow]` a ~20 s turn,
//!   `[fail]` a turn that errors after 3 s, `[ask]` a user question after 4 s.
//! * **A25** (background attention) — `[stop]` a turn stopped elsewhere
//!   after 3 s (`interrupted`), `[limit]` a rate-limited turn after 3 s.
//!
//! Every request is appended to `--log <file>` as one JSON line
//! `{"method", "params"}`, and every notification the server pushes as
//! `{"push", "session"}`, so a walk can assert the wire.
//!
//! ```sh
//! cargo run -p octoscode-module --example a22_serve -- 8496 --log tmp/a22/serve.jsonl
//! OCTOS_BASE_URL=http://127.0.0.1:8496 OCTOS_PROFILE_ID=a22 OCTOS_WORKSPACE_CWD=/home/user/a22-ws ... (the app)
//! ```
use std::collections::HashMap;
use std::io::Write as _;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::tungstenite::Message;

const PROFILE: &str = "a22";
const HISTORY: &str = "a22:api:history";
/// T1/T2: the history Session's persisted turns; T3: another client's live turn.
const T1: &str = "01920000-0000-7000-8000-000000022a01";
const T2: &str = "01920000-0000-7000-8000-000000022a02";
const T3: &str = "01920000-0000-7000-8000-000000022a03";

/// (id, title, minutes ago): the attested catalog's rows of `a22`, plus the
/// three the app must never project.
const CATALOG: &[(&str, &str, u64)] = &[
    ("a22:main", "Startup chat", 1),
    (HISTORY, "Review the hydrate path", 5),
    ("a22:api:build", "Build the release", 20),
    ("a22:api:tests", "Run the test suite", 40),
    ("a22:api:ask", "Pick a branch", 90),
    ("other:api:foreign", "Foreign profile row", 2),
    ("bare", "Bare id row", 3),
    ("a22:legacy", "Legacy row", 4),
];

#[derive(Default)]
struct World {
    /// Persisted rows per Session (`HydratedMessage` shape).
    rows: HashMap<String, Vec<Value>>,
    /// Per-thread last projection seq.
    threads: HashMap<String, std::collections::BTreeMap<String, u64>>,
    /// Per-Session durable cursor.
    cursor: HashMap<String, u64>,
    /// Sessions whose first history read was answered (the race is played once).
    raced: std::collections::HashSet<String>,
}

impl World {
    fn next_cursor(&mut self, session: &str) -> u64 {
        let c = self.cursor.entry(session.to_owned()).or_insert(10);
        *c += 1;
        *c
    }

    fn persist(&mut self, session: &str, turn: &str, role: &str, content: &str) {
        let rows = self.rows.entry(session.to_owned()).or_default();
        let seq = rows.len() as u64 + 1;
        rows.push(json!({"seq": seq, "role": role, "content": content, "thread_id": turn,
                         "persisted_at": "2026-10-02T09:00:00Z", "message_id": format!("{session}:{seq}")}));
    }
}

#[derive(Clone)]
struct Cfg {
    log: Option<String>,
}

fn now_iso(minutes_ago: u64) -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
        .saturating_sub(minutes_ago * 60_000);
    let secs = ms / 1000;
    let days = secs / 86_400;
    let (y, m, d) = civil(days as i64);
    let tod = secs % 86_400;
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", tod / 3600, (tod % 3600) / 60, tod % 60)
}

/// Howard Hinnant's civil_from_days.
fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn log(cfg: &Cfg, line: Value) {
    // One writer at a time: the turn tasks and the socket log concurrently.
    static LOG: Mutex<()> = Mutex::new(());
    let Some(path) = &cfg.log else { return };
    let _g = LOG.lock().unwrap_or_else(|p| p.into_inner());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args.get(1).and_then(|p| p.parse().ok()).unwrap_or(8496);
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let cfg = Cfg { log: flag("--log") };
    let mut w = World::default();
    // The history Session's two persisted turns (and the startup chat's one).
    w.persist(HISTORY, T1, "user", "First prompt: where does the hydrate start?");
    w.persist(HISTORY, T1, "assistant", "It starts in the open reply arm.");
    w.persist(HISTORY, T2, "user", "Second prompt: and the cursor?");
    w.persist(HISTORY, T2, "assistant", "The hydrate cursor is adopted max-wins.");
    w.threads.entry(HISTORY.into()).or_default().extend([(T1.to_owned(), 4u64), (T2.to_owned(), 4u64)]);
    w.cursor.insert(HISTORY.into(), 14);
    let world = Arc::new(Mutex::new(w));
    let listener = TcpListener::bind(("127.0.0.1", port)).await.expect("bind");
    println!("[a22-serve] listening on 127.0.0.1:{port}");
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue };
        let (cfg, world) = (cfg.clone(), world.clone());
        tokio::spawn(async move {
            let mut head = [0u8; 4096];
            let n = stream.peek(&mut head).await.unwrap_or(0);
            let text = String::from_utf8_lossy(&head[..n]).to_ascii_lowercase();
            if text.contains("upgrade: websocket") {
                ws(stream, cfg, world).await;
            } else if text.starts_with("get /api/auth/me") {
                // The drafts' principal (`resolveDraftPrincipal`).
                let mut stream = stream;
                let body = r#"{"user":{"id":"a22-user"}}"#;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            } else {
                let mut stream = stream;
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").await;
            }
        });
    }
}

fn opened(session: &str, cwd: &str) -> Value {
    let mut o = json!({
        "session_id": session, "active_profile_id": PROFILE, "workspace_root": cwd,
        "cursor": {"stream": session, "seq": 1},
        "capabilities": {
            "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
            "capabilities_schema_version": 2,
            "supported_methods": ["session/open", "session/hydrate", "session/list", "turn/start", "turn/interrupt", "user_question/respond"],
            "supported_notifications": ["turn/started", "turn/completed", "projection/envelope", "user_question/requested"],
            "supported_features": ["state.session_hydrate.v1", "projection.envelope.v2", "session.workspace_cwd.v1", "user_question.v1"]
        }
    });
    if session == "a22:main" {
        o["reasoning_effort"] = json!("high");
    }
    json!({ "opened": o })
}

fn note(method: &str, params: Value) -> String {
    json!({"jsonrpc": "2.0", "method": method, "params": params}).to_string()
}

fn send(out: &UnboundedSender<String>, cfg: &Cfg, session: &str, method: &str, params: Value) -> bool {
    log(cfg, json!({"push": method, "session": session}));
    out.send(note(method, params)).is_ok()
}

fn env(world: &Arc<Mutex<World>>, session: &str, turn: &str, seq: u64, payload: Value) -> Value {
    let cursor = world.lock().unwrap().next_cursor(session);
    json!({"session_id": session, "thread_id": turn, "turn_id": turn, "seq": seq,
           "cursor": {"stream": session, "seq": cursor}, "payload": payload})
}

/// One turn the app started, by its prompt's marker.
async fn run_turn(out: UnboundedSender<String>, cfg: Cfg, world: Arc<Mutex<World>>, session: String, turn: String, prompt: String) {
    let step = |ms: u64| tokio::time::sleep(Duration::from_millis(ms));
    let seg = format!("{turn}:assistant:iteration:1");
    step(250).await;
    send(&out, &cfg, &session, "turn/started", json!({"session_id": session, "turn_id": turn, "timestamp": "2026-10-02T09:00:00Z"}));
    let e = env(&world, &session, &turn, 1, json!({"type": "user_message", "data": {"text": prompt}}));
    send(&out, &cfg, &session, "projection/envelope", e);
    world.lock().unwrap().persist(&session, &turn, "user", &prompt);
    let (answer, outcome, ticks, ask) = if prompt.contains("[slow]") {
        ("Built and verified.", "completed", 20u64, false)
    } else if prompt.contains("[busy]") {
        ("Finished the long task.", "completed", 18, false)
    } else if prompt.contains("[fail]") {
        ("Running the suite…", "errored", 3, false)
    } else if prompt.contains("[ask]") {
        ("I need one answer first.", "", 4, true)
    } else if prompt.contains("[stop]") {
        // A25: a turn stopped elsewhere (another client's Stop) after 3 s.
        ("Stopping here.", "interrupted", 3, false)
    } else if prompt.contains("[limit]") {
        // A25: a turn the provider rate-limits after 3 s.
        ("Waiting for quota.", "rate_limited", 3, false)
    } else {
        ("Done.", "completed", 1, false)
    };
    for k in 0..ticks {
        step(1000).await;
        let text = if k + 1 == ticks { answer.to_owned() } else { format!("Step {} of {}. ", k + 1, ticks - 1) };
        let e = env(&world, &session, &turn, 2 + k, json!({"type": "assistant_delta", "data": {"text": text, "assistant_segment_id": seg}}));
        if !send(&out, &cfg, &session, "projection/envelope", e) {
            return;
        }
    }
    if ask {
        send(&out, &cfg, &session, "user_question/requested", json!({
            "session_id": session, "turn_id": turn, "question_id": "01a0eb8f-7b23-7030-9f26-a28486422a01",
            "title": "Which branch should I use?", "body": "1. Which branch should I use? (options: main, release)",
            "questions": [{"allow_free_text": true, "header": "Branch", "multi_select": false, "question": "Which branch should I use?",
                           "options": [{"label": "main", "description": "The default branch."},
                                       {"label": "release", "description": "The release branch."}]}]
        }));
        return; // the turn waits for the answer
    }
    let seq = 2 + ticks;
    if outcome == "completed" {
        let e = env(&world, &session, &turn, seq, json!({"type": "assistant_persisted", "data": {"text": answer, "assistant_segment_id": seg,
            "meta": {"message_id": format!("{session}:{turn}:answer"), "persisted_at": "2026-10-02T09:00:01Z"}}}));
        send(&out, &cfg, &session, "projection/envelope", e);
        world.lock().unwrap().persist(&session, &turn, "assistant", answer);
    }
    let e = env(&world, &session, &turn, seq + 1, json!({"type": "turn_terminal", "data": {"outcome": outcome,
        "error": if outcome == "errored" { json!({"code": "tool_failed", "message": "2 tests failed"}) } else { Value::Null }}}));
    send(&out, &cfg, &session, "projection/envelope", e);
    world.lock().unwrap().threads.entry(session.clone()).or_default().insert(turn.clone(), seq + 1);
}

/// Row 203: the history Session's first read — another client's live turn
/// (and a replayed frame of T2) reach the socket BEFORE the history; the
/// rest of the live turn follows it.
async fn race_history(out: UnboundedSender<String>, cfg: Cfg, world: Arc<Mutex<World>>, id: Value) {
    let seg = format!("{T3}:assistant:iteration:1");
    // A replayed frame of T2, which the history already holds (cursor 13).
    send(&out, &cfg, HISTORY, "projection/envelope", json!({"session_id": HISTORY, "thread_id": T2, "turn_id": T2, "seq": 4,
        "cursor": {"stream": HISTORY, "seq": 13},
        "payload": {"type": "assistant_persisted", "data": {"text": "The hydrate cursor is adopted max-wins.",
            "assistant_segment_id": format!("{T2}:assistant:iteration:1"), "meta": {"message_id": format!("{HISTORY}:4"), "persisted_at": "2026-10-02T09:00:00Z"}}}}));
    send(&out, &cfg, HISTORY, "turn/started", json!({"session_id": HISTORY, "turn_id": T3, "timestamp": "2026-10-02T09:00:00Z"}));
    let e = json!({"session_id": HISTORY, "thread_id": T3, "turn_id": T3, "seq": 1, "cursor": {"stream": HISTORY, "seq": 15},
        "payload": {"type": "user_message", "data": {"text": "Third prompt: live from another client"}}});
    send(&out, &cfg, HISTORY, "projection/envelope", e);
    let e = json!({"session_id": HISTORY, "thread_id": T3, "turn_id": T3, "seq": 2, "cursor": {"stream": HISTORY, "seq": 16},
        "payload": {"type": "assistant_delta", "data": {"text": "The live answer ", "assistant_segment_id": seg}}});
    send(&out, &cfg, HISTORY, "projection/envelope", e);
    // The history, 1.5 s late (cursor 14: it holds T1 and T2, not T3).
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let result = {
        let w = world.lock().unwrap();
        json!({"session_id": HISTORY, "cursor": {"stream": HISTORY, "seq": 14},
               "messages": w.rows.get(HISTORY).cloned().unwrap_or_default(),
               "projection_thread_sequences": {T1: 4, T2: 4}})
    };
    log(&cfg, json!({"reply": "session/hydrate", "session": HISTORY, "raced": true}));
    let _ = out.send(json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string());
    tokio::time::sleep(Duration::from_millis(700)).await;
    let e = json!({"session_id": HISTORY, "thread_id": T3, "turn_id": T3, "seq": 3, "cursor": {"stream": HISTORY, "seq": 17},
        "payload": {"type": "assistant_delta", "data": {"text": "arrived after the history.", "assistant_segment_id": seg}}});
    send(&out, &cfg, HISTORY, "projection/envelope", e);
    let answer = "The live answer arrived after the history.";
    let e = json!({"session_id": HISTORY, "thread_id": T3, "turn_id": T3, "seq": 4, "cursor": {"stream": HISTORY, "seq": 18},
        "payload": {"type": "assistant_persisted", "data": {"text": answer, "assistant_segment_id": seg,
            "meta": {"message_id": format!("{HISTORY}:6"), "persisted_at": "2026-10-02T09:00:02Z"}}}});
    send(&out, &cfg, HISTORY, "projection/envelope", e);
    let e = json!({"session_id": HISTORY, "thread_id": T3, "turn_id": T3, "seq": 5, "cursor": {"stream": HISTORY, "seq": 19},
        "payload": {"type": "turn_terminal", "data": {"outcome": "completed"}}});
    send(&out, &cfg, HISTORY, "projection/envelope", e);
    let mut w = world.lock().unwrap();
    w.persist(HISTORY, T3, "user", "Third prompt: live from another client");
    w.persist(HISTORY, T3, "assistant", answer);
    w.threads.entry(HISTORY.into()).or_default().insert(T3.to_owned(), 5);
    w.cursor.insert(HISTORY.into(), 19);
}

async fn ws(stream: TcpStream, cfg: Cfg, world: Arc<Mutex<World>>) {
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
    println!("[a22-serve] ui-protocol socket open");
    let (mut tx, mut rx) = ws.split();
    let (out, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        while let Some(frame) = out_rx.recv().await {
            if tx.send(Message::Text(frame.into())).await.is_err() {
                break;
            }
        }
    });
    while let Some(Ok(msg)) = rx.next().await {
        let Message::Text(text) = msg else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        let method = v["method"].as_str().unwrap_or("").to_owned();
        let id = v["id"].clone();
        let params = v.get("params").cloned().unwrap_or(Value::Null);
        log(&cfg, json!({"method": method, "params": params}));
        println!("[a22-serve] <- {method}");
        let session = params["session_id"].as_str().unwrap_or("a22:main").to_owned();
        let reply = |result: Value| json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string();
        match method.as_str() {
            "session/open" => {
                let cwd = params["cwd"].as_str().unwrap_or("/home/user/a22-ws").to_owned();
                let _ = out.send(reply(opened(&session, &cwd)));
            }
            "session/hydrate" => {
                let first = world.lock().unwrap().raced.insert(session.clone());
                if session == HISTORY && first {
                    tokio::spawn(race_history(out.clone(), cfg.clone(), world.clone(), id.clone()));
                    continue;
                }
                let result = {
                    let w = world.lock().unwrap();
                    let c = w.cursor.get(&session).copied().unwrap_or(1);
                    json!({"session_id": session, "cursor": {"stream": session, "seq": c},
                           "messages": w.rows.get(&session).cloned().unwrap_or_default(),
                           "projection_thread_sequences": w.threads.get(&session).cloned().unwrap_or_default()})
                };
                let _ = out.send(reply(result));
            }
            "session/list" => {
                let rows: Vec<Value> = CATALOG
                    .iter()
                    .map(|(sid, title, ago)| json!({"id": sid, "title": title, "message_count": 4, "updated_at": now_iso(*ago), "active_turn": false}))
                    .collect();
                let result = match (params["cwd"].as_str(), params["profile_id"].as_str()) {
                    (Some(cwd), Some(profile)) if profile == PROFILE => json!({"sessions": rows, "workspace_root": cwd, "profile_id": profile}),
                    // The legacy global listing: every row, nothing attested.
                    _ => json!({"sessions": rows}),
                };
                let _ = out.send(reply(result));
            }
            "turn/start" => {
                let prompt = params["input"][0]["text"].as_str().unwrap_or("").to_owned();
                if prompt.contains("[refuse]") {
                    let _ = out.send(json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32000, "message": "The server refused this turn."}}).to_string());
                    continue;
                }
                let turn = params["turn_id"].as_str().unwrap_or("").to_owned();
                let _ = out.send(reply(json!({"accepted": true})));
                tokio::spawn(run_turn(out.clone(), cfg.clone(), world.clone(), session, turn, prompt));
            }
            _ => {
                let _ = out.send(reply(json!({})));
            }
        }
    }
}
