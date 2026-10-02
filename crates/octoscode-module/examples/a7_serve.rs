//! A7 — a scripted AppUI fixture server for the A7 click walk
//! (`tools/walk/a7_composer_walk.py`): the composer's queue / steer /
//! collision / recovery paths, the answer's markdown, history fork + undo,
//! and a staged peer, each driven by a keyword in the prompt. No model runs.
//!
//! ```sh
//! cargo run -p octoscode-module --example a7_serve -- 8427
//! # the app, first-run against a dead port, then the walk types this
//! # server into the Connect card:
//! OCTOS_BASE_URL=http://127.0.0.1:8499 OCTOS_PROFILE_ID=a7 ... --module octoscode
//! ```
//!
//! Prompt keywords (case-insensitive, first match wins):
//! * `markdown` — streams an answer with a rust + toml fence, links (one
//!   unsafe), an image, inline + display math, over ~3 s.
//! * `slow`     — streams ~10 s; a `turn/steer` lands in the stream.
//! * `busy`     — refuses `turn/start` with the typed collision data and runs
//!   the occupying turn for ~5 s (another client).
//! * `lost`     — never acknowledges `turn/start`; `turn/state/get` then
//!   answers `completed` (`lost forever` answers `unknown`).
//! * `peer`     — stages a peer (`peer/staged`) whose Session is listed.
//! * anything else — a short answer.
//!
//! `POST /api/upload` (the image dialog's explicit upload) answers each file
//! after `A7_SERVE_UPLOAD_DELAY_MS` (default 4000) with a receipt in the
//! server's handle grammar, so a walk can press Upload twice, cancel or
//! remove while a transfer is in flight; every request is logged.
//!
//! `A7_SERVE_HELD=<driver id>` opens every Session in `external` driver mode,
//! parked by that driver (the board-12 held banner): `turn/start` is refused
//! `ExternalMasterHeld` until a `session/driver/acquire` (CAS on the revision)
//! and a `session/driver/release {next: "internal"}` with its proof
//! (`A7_SERVE_RELEASE_DELAY_MS` holds the release reply).
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

const PROFILE: &str = "a7";
const OCCUPIER: &str = "3f1a9c52-4d1b-4c2e-8f6a-0b7d21e9c4aa";

const MARKDOWN: &str = "## Steer queue fix\n\n\
The drop came from the reconnect path: it cleared the **pending buffer** before the flush. \
See [the protocol notes](https://docs.example.com/steer) — a [local link](javascript:alert(1)) stays plain text, \
and the chart ![queue depth chart](https://cdn.example.com/depth.png) is not loaded.\n\n\
```rust\nfn flush(queue: &mut Vec<String>) -> usize {\n    // drain in order, keep nothing behind\n    let n = queue.len();\n    queue.drain(..).for_each(send);\n    n\n}\n```\n\n\
With `n` steers queued, the flush costs $O(n)$ and the latency bound is\n\n$$\nT = \\sum_{i=1}^{n} t_i\n$$\n\n\
```toml\n[workspace]\nkind = \"session\"\n```\n\nThe run costs $12 and $5 more on CI.";

#[derive(Default)]
struct StreamCtl {
    steers: Mutex<Vec<String>>,
    interrupted: AtomicBool,
}

/// One Session's driver record: the disclosure plus the live proof.
#[derive(Clone)]
struct Driver {
    external: bool,
    driver: String,
    epoch: u64,
    revision: u64,
    lease: u64,
    token: Option<String>,
}

impl Driver {
    fn initial() -> Self {
        match std::env::var("A7_SERVE_HELD").ok().filter(|v| !v.trim().is_empty()) {
            Some(holder) => Self { external: true, driver: holder, epoch: 2, revision: 7, lease: 0, token: None },
            None => Self { external: false, driver: String::new(), epoch: 0, revision: 0, lease: 0, token: None },
        }
    }

    fn disclosure(&self) -> Value {
        if !self.external {
            return json!({"mode": "internal", "recovery": "none", "binding": null});
        }
        json!({"mode": "external", "recovery": "none", "binding": {
            "driver_id": self.driver, "epoch": self.epoch, "revision": self.revision,
            "lease_expires_at_ms": self.lease}})
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[derive(Default)]
struct World {
    sessions: Vec<(String, String)>,
    messages: HashMap<String, Vec<Value>>,
    turns: HashMap<String, Vec<(String, String)>>,
    live: HashMap<String, (String, Arc<StreamCtl>)>,
    drivers: HashMap<String, Driver>,
    seq: u64,
}

impl World {
    fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    fn add_message(&mut self, session: &str, role: &str, content: &str, turn: &str) {
        let seq = self.next_seq();
        let m = json!({
            "seq": seq, "role": role, "content": content, "turn_id": turn,
            "thread_id": turn, "persisted_at": "2026-10-01T10:00:00Z",
            "message_id": format!("m{seq}"),
        });
        self.messages.entry(session.to_owned()).or_default().push(m);
    }

    fn set_turn(&mut self, session: &str, turn: &str, state: &str) {
        let list = self.turns.entry(session.to_owned()).or_default();
        match list.iter_mut().find(|(t, _)| t == turn) {
            Some(e) => e.1 = state.to_owned(),
            None => list.push((turn.to_owned(), state.to_owned())),
        }
    }

    fn turn_state(&self, session: &str, turn: &str) -> Option<String> {
        self.turns.get(session)?.iter().find(|(t, _)| t == turn).map(|(_, s)| s.clone())
    }

    fn hydrate(&mut self, session: &str) -> Value {
        let seq = self.seq;
        let turns: Vec<Value> = self
            .turns
            .get(session)
            .map(|t| t.iter().map(|(id, st)| json!({"turn_id": id, "state": st})).collect())
            .unwrap_or_default();
        json!({
            "session_id": session,
            "cursor": {"stream": session, "seq": seq},
            "messages": self.messages.get(session).cloned().unwrap_or_default(),
            "turns": turns,
        })
    }

    fn list(&self) -> Value {
        let sessions: Vec<Value> = self
            .sessions
            .iter()
            .map(|(id, title)| {
                let n = self.messages.get(id).map(|m| m.len()).unwrap_or(0);
                let active = self.live.contains_key(id);
                json!({"id": id, "title": title, "message_count": n,
                       "updated_at": "2026-10-01T10:00:00Z", "active_turn": active})
            })
            .collect();
        json!({ "sessions": sessions })
    }

    fn ensure_session(&mut self, id: &str, title: &str) {
        if !self.sessions.iter().any(|(s, _)| s == id) {
            self.sessions.push((id.to_owned(), title.to_owned()));
        }
    }
}

type Tx = mpsc::UnboundedSender<String>;

fn notify(tx: &Tx, method: &str, params: Value) {
    let _ = tx.send(json!({"jsonrpc": "2.0", "method": method, "params": params}).to_string());
}

fn reply(tx: &Tx, id: &Value, result: Value) {
    let _ = tx.send(json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string());
}

fn reply_err(tx: &Tx, id: &Value, code: i64, message: &str, data: Option<Value>) {
    let mut error = json!({"code": code, "message": message});
    if let Some(d) = data {
        error["data"] = d;
    }
    let _ = tx.send(json!({"jsonrpc": "2.0", "id": id, "error": error}).to_string());
}

fn uuid(n: u64) -> String {
    format!("01920000-0000-7000-8000-{n:012x}")
}

/// Stream one turn: started, deltas (with any steers folded in), terminal.
async fn stream_turn(world: Arc<Mutex<World>>, tx: Tx, session: String, turn: String, chunks: Vec<String>, gap_ms: u64) {
    let ctl = Arc::new(StreamCtl::default());
    {
        let mut w = world.lock().unwrap();
        w.live.insert(session.clone(), (turn.clone(), ctl.clone()));
        w.set_turn(&session, &turn, "active");
    }
    notify(&tx, "turn/started", json!({"session_id": session, "turn_id": turn, "timestamp": "2026-10-01T10:00:00Z"}));
    let mut answer = String::new();
    for chunk in chunks {
        if ctl.interrupted.load(Ordering::Relaxed) {
            break;
        }
        let steered: Vec<String> = std::mem::take(&mut *ctl.steers.lock().unwrap());
        for s in steered {
            let text = format!("\n\n> Steered: {s}\n\n");
            answer.push_str(&text);
            notify(&tx, "message/delta", json!({"session_id": session, "turn_id": turn, "text": text}));
        }
        answer.push_str(&chunk);
        notify(&tx, "message/delta", json!({"session_id": session, "turn_id": turn, "text": chunk}));
        tokio::time::sleep(Duration::from_millis(gap_ms)).await;
    }
    let interrupted = ctl.interrupted.load(Ordering::Relaxed);
    {
        let mut w = world.lock().unwrap();
        w.live.remove(&session);
        w.add_message(&session, "assistant", &answer, &turn);
        w.set_turn(&session, &turn, if interrupted { "interrupted" } else { "completed" });
    }
    if interrupted {
        notify(&tx, "turn/error", json!({"session_id": session, "turn_id": turn, "code": "interrupted", "message": "Stopped by the user"}));
    } else {
        notify(&tx, "turn/completed", json!({"session_id": session, "turn_id": turn}));
    }
}

fn chunked(text: &str, parts: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let size = (chars.len() / parts.max(1)).max(1);
    chars.chunks(size).map(|c| c.iter().collect()).collect()
}

fn open_reply(session: &str) -> Value {
    json!({"opened": {
        "session_id": session,
        "active_profile_id": PROFILE,
        "workspace_root": "/home/user/src/octos",
        "cursor": {"stream": session, "seq": 1},
        "capabilities": {
            "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
            "capabilities_schema_version": 1,
            "supported_methods": [
                "session/open", "session/list", "session/hydrate", "turn/start", "turn/interrupt",
                "turn/steer", "turn/state/get", "session/fork", "session/rollback",
                "snapshot/list", "snapshot/restore",
                "session/driver/get", "session/driver/acquire", "session/driver/release"
            ],
            "supported_notifications": [
                "message/delta", "turn/started", "turn/completed", "turn/error", "turn/steer_dropped", "peer/staged"
            ],
            "supported_features": ["event.turn_steer_dropped.v1", "turn.state_get.v1"]
        }
    }})
}

fn snapshots(session: &str) -> Value {
    json!({
        "session_id": session, "enabled": true, "available": true,
        "snapshots": [
            {"id": "snap-2", "label": "Before the steer-queue refactor", "timestamp_unix": 1_790_000_000},
            {"id": "snap-1", "label": "Session start", "timestamp_unix": 1_789_990_000}
        ]
    })
}

async fn handle(world: Arc<Mutex<World>>, tx: Tx, v: Value, counter: Arc<AtomicU64>) {
    let method = v["method"].as_str().unwrap_or("").to_owned();
    let id = v["id"].clone();
    let p = &v["params"];
    let session = p["session_id"].as_str().unwrap_or("a7:main").to_owned();
    println!("[a7-serve] <- {method}");
    match method.as_str() {
        "session/open" => {
            world.lock().unwrap().ensure_session(&session, "Fix steer queue drop on reconnect");
            reply(&tx, &id, open_reply(&session));
        }
        "session/list" => {
            let l = world.lock().unwrap().list();
            reply(&tx, &id, l);
        }
        "session/hydrate" => {
            let h = world.lock().unwrap().hydrate(&session);
            reply(&tx, &id, h);
        }
        "session/driver/get" => {
            let d = world.lock().unwrap().drivers.entry(session.clone()).or_insert_with(Driver::initial).clone();
            reply(&tx, &id, d.disclosure());
        }
        "session/driver/acquire" => {
            let mut w = world.lock().unwrap();
            let d = w.drivers.entry(session.clone()).or_insert_with(Driver::initial);
            let who = p["driver_id"].as_str().unwrap_or_default().to_owned();
            if p["expected_revision"].as_u64() != Some(d.revision) {
                reply_err(&tx, &id, -32010, "driver_revision_conflict", None);
            } else if d.external && d.lease > now_ms() && d.driver != who {
                reply_err(&tx, &id, -32012, "driver_busy", None);
            } else {
                d.external = true;
                d.driver = who;
                d.epoch += 1;
                d.revision += 1;
                d.lease = now_ms() + p["lease_seconds"].as_u64().unwrap_or(60) * 1000;
                let token = format!("a7-proof-{}", d.epoch);
                d.token = Some(token.clone());
                let binding = json!({"driver_id": d.driver, "epoch": d.epoch, "revision": d.revision,
                    "lease_expires_at_ms": d.lease});
                reply(&tx, &id, json!({"control_token": token, "recovery": "none", "binding": binding}));
            }
        }
        "session/driver/release" => {
            // `A7_SERVE_RELEASE_DELAY_MS`: hold the handback so a walk can
            // see the composer's "Resuming chat…" / "Handing back control…".
            let delay = std::env::var("A7_SERVE_RELEASE_DELAY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(0u64);
            tokio::time::sleep(Duration::from_millis(delay)).await;
            let mut w = world.lock().unwrap();
            let d = w.drivers.entry(session.clone()).or_insert_with(Driver::initial);
            let proven = d.token.as_deref() == p["control_token"].as_str()
                && p["epoch"].as_u64() == Some(d.epoch)
                && p["expected_revision"].as_u64() == Some(d.revision)
                && p["driver_id"].as_str() == Some(d.driver.as_str());
            if !proven || p["next"] != "internal" {
                reply_err(&tx, &id, -32013, "driver_fence_stale", None);
            } else {
                d.external = false;
                d.revision += 1;
                d.lease = 0;
                d.token = None;
                reply(&tx, &id, json!({"mode": "internal", "recovery": "none"}));
            }
        }
        "turn/start" => {
            let turn = p["turn_id"].as_str().unwrap_or_default().to_owned();
            let text = p["input"][0]["text"].as_str().unwrap_or_default().to_owned();
            let lower = text.to_ascii_lowercase();
            let held = world.lock().unwrap().drivers.entry(session.clone()).or_insert_with(Driver::initial).external;
            if held {
                // The Core refuses chat while the Session is external.
                reply_err(&tx, &id, -32003, "turn admission refused for this session: ExternalMasterHeld", None);
                return;
            }
            if lower.contains("busy") {
                // Another client holds the slot: typed collision data, then its turn runs.
                reply_err(
                    &tx,
                    &id,
                    -32600,
                    "a turn is already running for this session",
                    Some(json!({"kind": "turn_in_progress", "turn_id": OCCUPIER})),
                );
                let already = world.lock().unwrap().live.contains_key(&session);
                if !already {
                    let (w, t, s) = (world.clone(), tx.clone(), session.clone());
                    let chunks = chunked("Another client is refactoring the steer queue in this session. ", 6);
                    tokio::spawn(async move { stream_turn(w, t, s, OCCUPIER.to_owned(), chunks, 800).await });
                }
                return;
            }
            world.lock().unwrap().add_message(&session, "user", &text, &turn);
            if lower.contains("lost") {
                // The ACK never arrives; the turn ran and finished server-side.
                let forever = lower.contains("forever");
                let mut w = world.lock().unwrap();
                w.set_turn(&session, &turn, if forever { "unknown" } else { "completed" });
                if !forever {
                    w.add_message(&session, "assistant", "The lost turn finished on the server.", &turn);
                }
                return;
            }
            reply(&tx, &id, json!({"accepted": true}));
            let (chunks, gap) = if lower.contains("markdown") {
                (chunked(MARKDOWN, 28), 110)
            } else if lower.contains("slow") {
                (chunked("Reading the steer queue, the reconnect path and the flush order before changing anything. Each step is checked against the recorded traffic, then the fix is applied once. ", 14), 700)
            } else if lower.contains("peer") {
                let slug = "review-diff";
                world.lock().unwrap().ensure_session(&format!("{PROFILE}:local:tui#peer-{slug}"), "Peer · review-diff");
                notify(&tx, "peer/staged", json!({
                    "session_id": session, "topic": format!("peer-{slug}"), "slug": slug,
                    "brief": "Review the steer-queue diff.", "brief_path": "/home/user/.octos/peers/review-diff/brief.md",
                    "cwd": "/home/user/src/octos", "profile_id": PROFILE
                }));
                (vec!["Staged a peer to review the diff: ".to_owned(), "`review-diff`.".to_owned()], 150)
            } else {
                (vec!["Done: ".to_owned(), text.clone()], 150)
            };
            let (w, t, s) = (world.clone(), tx.clone(), session.clone());
            tokio::spawn(async move { stream_turn(w, t, s, turn, chunks, gap).await });
        }
        "turn/steer" => {
            let expected = p["expected_turn_id"].as_str().unwrap_or_default().to_owned();
            let text = p["input"][0]["text"].as_str().unwrap_or_default().to_owned();
            let live = world.lock().unwrap().live.get(&session).cloned();
            match live {
                Some((turn, ctl)) if turn == expected => {
                    ctl.steers.lock().unwrap().push(text);
                    reply(&tx, &id, json!({"turn_id": turn, "steered": true}));
                }
                _ => {
                    // No active turn: Core starts the text as a new turn.
                    let turn = uuid(0xa700 + counter.fetch_add(1, Ordering::Relaxed));
                    world.lock().unwrap().add_message(&session, "user", &text, &turn);
                    reply(&tx, &id, json!({"turn_id": turn, "steered": false}));
                    let (w, t, s) = (world.clone(), tx.clone(), session.clone());
                    tokio::spawn(async move { stream_turn(w, t, s, turn, vec!["Started from the steer.".to_owned()], 150).await });
                }
            }
        }
        "turn/interrupt" => {
            let turn = p["turn_id"].as_str().unwrap_or_default().to_owned();
            if let Some((t, ctl)) = world.lock().unwrap().live.get(&session).cloned() {
                if t == turn {
                    ctl.interrupted.store(true, Ordering::Relaxed);
                }
            }
            reply(&tx, &id, json!({"interrupted": true}));
        }
        "turn/state/get" => {
            let turn = p["turn_id"].as_str().unwrap_or_default().to_owned();
            let state = world.lock().unwrap().turn_state(&session, &turn).unwrap_or_else(|| "unknown".to_owned());
            reply(&tx, &id, json!({"session_id": session, "turn_id": turn, "state": state}));
        }
        "session/fork" => {
            let name = p["new_chat_id"].as_str().unwrap_or("fork").to_owned();
            let child = format!("{PROFILE}:{name}");
            let mut w = world.lock().unwrap();
            let msgs = w.messages.get(&session).cloned().unwrap_or_default();
            let n = msgs.len() as u32;
            w.messages.insert(child.clone(), msgs);
            w.ensure_session(&child, &format!("Fork · {name}"));
            reply(&tx, &id, json!({"new_session_id": child, "parent_session_id": session, "copied_messages": n}));
        }
        "snapshot/list" => reply(&tx, &id, snapshots(&session)),
        "snapshot/restore" => {
            let restored = p["snapshot_id"].as_str().unwrap_or_default().to_owned();
            let mut s = snapshots(&session);
            s["restored"] = json!(restored);
            reply(&tx, &id, s);
        }
        "session/rollback" => {
            let n = p["num_turns"].as_u64().unwrap_or(0) as usize;
            let mut w = world.lock().unwrap();
            let msgs = w.messages.entry(session.clone()).or_default();
            let mut threads: Vec<String> = Vec::new();
            for m in msgs.iter() {
                if m["role"] == "user" {
                    let t = m["thread_id"].as_str().unwrap_or_default().to_owned();
                    if !threads.contains(&t) {
                        threads.push(t);
                    }
                }
            }
            let drop: Vec<String> = threads.iter().rev().take(n).cloned().collect();
            msgs.retain(|m| !drop.iter().any(|t| m["thread_id"].as_str() == Some(t)));
            let h = w.hydrate(&session);
            reply(&tx, &id, json!({"dropped_turns": n, "thread": h}));
        }
        "session/status/read" => reply(&tx, &id, json!({"session_id": session, "model": {"title": "DeepSeek V4 Flash", "model": "deepseek-v4-flash"}})),
        _ => {
            if !id.is_null() {
                reply(&tx, &id, json!({}));
            }
        }
    }
}

/// base64url without padding (the receipt grammar, media.rs
/// `uploaded_handle_for_profile`).
fn b64url(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        let k = chunk.len() + 1;
        for i in 0..k {
            out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

/// `POST /api/upload`: read the multipart body, wait, answer one receipt.
async fn upload(mut stream: TcpStream, counter: Arc<AtomicU64>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let header_end = loop {
        let k = stream.read(&mut chunk).await.unwrap_or(0);
        if k == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..k]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let len = head
        .lines()
        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
        .unwrap_or(0);
    while buf.len() < header_end + len {
        let k = stream.read(&mut chunk).await.unwrap_or(0);
        if k == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..k]);
    }
    let body = String::from_utf8_lossy(&buf[header_end..]).to_string();
    let name = body
        .split("filename=\"")
        .nth(1)
        .and_then(|r| r.split('"').next())
        .unwrap_or("image.png")
        .to_owned();
    let n = counter.fetch_add(1, Ordering::Relaxed);
    println!("[a7-serve] <- POST /api/upload #{n} {name} ({} bytes)", len);
    let delay = std::env::var("A7_SERVE_UPLOAD_DELAY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(4000u64);
    tokio::time::sleep(Duration::from_millis(delay)).await;
    let handle = format!("up/{}/{name}", b64url(format!("{PROFILE}/uploads/{name}").as_bytes()));
    let payload = json!([handle]).to_string();
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    let _ = stream.write_all(resp.as_bytes()).await;
    println!("[a7-serve] -> receipt #{n} {name}");
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args().nth(1).and_then(|p| p.parse().ok()).unwrap_or(8427);
    let listener = TcpListener::bind(("127.0.0.1", port)).await.expect("bind");
    println!("[a7-serve] listening on ws://127.0.0.1:{port}");
    let world = Arc::new(Mutex::new(World::default()));
    let counter = Arc::new(AtomicU64::new(1));
    let uploads = Arc::new(AtomicU64::new(1));
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue };
        let world = world.clone();
        let counter = counter.clone();
        let uploads = uploads.clone();
        tokio::spawn(async move {
            let mut head = [0u8; 16];
            let n = stream.peek(&mut head).await.unwrap_or(0);
            if head[..n].starts_with(b"POST /api/upload") {
                upload(stream, uploads).await;
                return;
            }
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            println!("[a7-serve] client connected");
            let (mut sink, mut source) = ws.split();
            let (tx, mut rx) = mpsc::unbounded_channel::<String>();
            tokio::spawn(async move {
                while let Some(frame) = rx.recv().await {
                    if sink.send(Message::Text(frame.into())).await.is_err() {
                        break;
                    }
                }
            });
            while let Some(Ok(msg)) = source.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                if v.get("id").is_none() {
                    continue;
                }
                let (w, t, c) = (world.clone(), tx.clone(), counter.clone());
                tokio::spawn(async move { handle(w, t, v, c).await });
            }
            println!("[a7-serve] client disconnected");
        });
    }
}
