//! A20 — the interaction-origin + saved-link fixture server (parity rows 250
//! and 247): one port answering the UI protocol the a20 click walks need, with
//! no model and no machine paths. It owns parked interactions the way Core
//! does: a `turn/start` whose prompt asks to run `rm -rf …` raises a typed
//! `approval/requested` (the recorded r5 shape) and PAUSES its turn until
//! `approval/respond` answers it on the SAME Session; a prompt that says "ask
//! me" raises a `user_question/requested` (the recorded r23 shape) and pauses
//! until `user_question/respond`. A response on any other Session is refused
//! (`-32602`, "… is not pending in <session>"), as Core refuses it. The
//! canonical `session/hydrate {include: ["pending_approvals"]}` answers what
//! is still parked for the asking Session only.
//!
//! `session/list {cwd, profile_id}` attests a canonical `workspace_root`
//! (`--attest <cwd>=<root>` maps a "moved or symlinked" folder to its real
//! root; any other cwd attests itself) — the read the saved-link precondition
//! makes before any open.
//!
//! Every request is appended to `--log <file>` as one JSON line
//! `{"method", "params"}`, and printed as `<- <method> <session>`.
//!
//! ```sh
//! cargo run -p octoscode-module --example a20_serve -- 8485 --log tmp/a20/serve.jsonl \
//!     [--attest /home/user/a20-link-ws=/home/user/a20-real-ws]
//! ```
use std::io::Write as _;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

const PROFILE: &str = "a20";
const DEFAULT_WS: &str = "/home/user/a20-ws";

/// One parked interaction: its wire frame and what resumes its turn.
#[derive(Clone)]
struct Parked {
    kind: &'static str,
    session: String,
    turn: String,
    id: String,
    frame: Value,
}

#[derive(Default)]
struct World {
    sessions: Vec<(String, String)>,
    parked: Vec<Parked>,
    attest: Vec<(String, String)>,
    /// Every live socket's writer (notifications go to all of them).
    writers: Vec<tokio::sync::mpsc::UnboundedSender<String>>,
    next: u64,
    log: Option<String>,
}

type Shared = Arc<Mutex<World>>;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args.get(1).and_then(|p| p.parse().ok()).unwrap_or(8485);
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let attest = args
        .iter()
        .enumerate()
        .filter(|(_, a)| *a == "--attest")
        .filter_map(|(i, _)| args.get(i + 1))
        .filter_map(|m| m.split_once('=').map(|(a, b)| (a.to_owned(), b.to_owned())))
        .collect();
    let world = Arc::new(Mutex::new(World {
        sessions: vec![
            ("a20:api:xray".into(), "Session X — clean the scratch dir".into()),
            ("a20:api:yankee".into(), "Session Y — release notes".into()),
            ("a20:main".into(), "Main".into()),
        ],
        attest,
        log: flag("--log"),
        ..Default::default()
    }));
    let listener = TcpListener::bind(("127.0.0.1", port)).await.expect("bind");
    println!("[a20-serve] listening on 127.0.0.1:{port}");
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue };
        let world = world.clone();
        tokio::spawn(async move {
            let mut head = [0u8; 4096];
            let n = stream.peek(&mut head).await.unwrap_or(0);
            let text = String::from_utf8_lossy(&head[..n]).to_ascii_lowercase();
            if text.contains("upgrade: websocket") {
                ws(stream, world).await;
            } else {
                let mut stream = stream;
                let body = if text.starts_with("get /api/auth/me") { r#"{"user":{"id":"a20-user"}}"# } else { "" };
                let status = if body.is_empty() { "404 Not Found" } else { "200 OK" };
                let resp = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = tokio::io::AsyncWriteExt::write_all(&mut stream, resp.as_bytes()).await;
            }
        });
    }
}

fn log(w: &World, method: &str, params: &Value) {
    let Some(path) = &w.log else { return };
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{}", json!({"method": method, "params": params}));
    }
}

fn notify(world: &Shared, method: &str, params: Value) {
    let frame = json!({"jsonrpc": "2.0", "method": method, "params": params}).to_string();
    let mut w = world.lock().unwrap();
    w.writers.retain(|tx| tx.send(frame.clone()).is_ok());
}

fn uuid(n: u64, tag: u16) -> String {
    format!("01a0e773-f844-7d50-{tag:04x}-{n:012x}")
}

const METHODS: &[&str] = &[
    "session/open",
    "session/list",
    "session/hydrate",
    "session/status/read",
    "turn/start",
    "turn/interrupt",
    "approval/respond",
    "approval/scopes/list",
    "user_question/respond",
];

async fn ws(stream: TcpStream, world: Shared) {
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
    println!("[a20-serve] ui-protocol socket open");
    let (mut tx, mut rx) = ws.split();
    let (out, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    world.lock().unwrap().writers.push(out.clone());
    let writer = tokio::spawn(async move {
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
        let session = params["session_id"].as_str().unwrap_or("a20:main").to_owned();
        {
            let w = world.lock().unwrap();
            log(&w, &method, &params);
        }
        println!("[a20-serve] <- {method} {session}");
        let mut after: Option<Box<dyn FnOnce() + Send>> = None;
        let result: Result<Value, Value> = {
            let mut w = world.lock().unwrap();
            match method.as_str() {
                "session/open" => {
                    let root = params["cwd"].as_str().unwrap_or(DEFAULT_WS).to_owned();
                    if !w.sessions.iter().any(|(s, _)| *s == session) {
                        w.sessions.insert(0, (session.clone(), String::new()));
                    }
                    Ok(json!({"opened": {
                        "session_id": session, "active_profile_id": PROFILE, "workspace_root": root,
                        "cursor": {"stream": session, "seq": 1},
                        "capabilities": {
                            "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                            "capabilities_schema_version": 2,
                            "supported_methods": METHODS,
                            "supported_notifications": ["turn/started", "turn/completed", "approval/requested",
                                                        "approval/decided", "user_question/requested"],
                            "supported_features": ["session.workspace_cwd.v1", "state.session_hydrate.v1",
                                                   "user_question.v1", "approval.typed.v1"]
                        }
                    }}))
                }
                "session/list" => {
                    let rows: Vec<Value> = w
                        .sessions
                        .iter()
                        .map(|(s, t)| {
                            let mut row = json!({"id": s, "message_count": 2, "updated_at": "2026-10-02T09:00:00Z"});
                            if !t.is_empty() {
                                row["title"] = json!(t);
                            }
                            row
                        })
                        .collect();
                    let mut r = json!({"sessions": rows});
                    if let Some(cwd) = params["cwd"].as_str() {
                        let root = w.attest.iter().find(|(c, _)| c == cwd).map(|(_, r)| r.clone()).unwrap_or_else(|| cwd.to_owned());
                        r["workspace_root"] = json!(root);
                        r["profile_id"] = params["profile_id"].clone();
                    }
                    Ok(r)
                }
                "session/hydrate" if params["include"].as_array().is_some_and(|a| a.iter().any(|x| x == "pending_approvals")) => {
                    let mine = |k: &str| -> Vec<Value> {
                        w.parked.iter().filter(|p| p.session == session && p.kind == k).map(|p| p.frame.clone()).collect()
                    };
                    Ok(json!({
                        "session_id": session, "cursor": {"stream": session, "seq": 2},
                        "pending_approvals": mine("approval"), "pending_questions": mine("question")
                    }))
                }
                "session/hydrate" => Ok(json!({"session_id": session, "cursor": {"stream": session, "seq": 2}, "messages": []})),
                "session/status/read" => Ok(json!({"session_id": session, "profile_id": PROFILE,
                    "model": {"model": "deepseek-v4-flash", "provider": "deepseek", "selected": true},
                    "permission_profile": "workspace_write", "cursor": {"healthy": true, "replay_supported": true},
                    "health": {"status": "ok"}})),
                "approval/scopes/list" => Ok(json!({"scopes": []})),
                "turn/start" => {
                    let turn = params["turn_id"].as_str().unwrap_or_default().to_owned();
                    let prompt = params["input"][0]["text"].as_str().unwrap_or_default().to_owned();
                    w.next += 1;
                    let n = w.next;
                    let world2 = world.clone();
                    let s2 = session.clone();
                    after = Some(Box::new(move || {
                        tokio::spawn(start_turn(world2, s2, turn, prompt, n));
                    }));
                    Ok(json!({"accepted": true}))
                }
                "approval/respond" | "user_question/respond" => {
                    let (kind, key) = if method == "approval/respond" { ("approval", "approval_id") } else { ("question", "question_id") };
                    let rid = params[key].as_str().unwrap_or_default().to_owned();
                    match w.parked.iter().position(|p| p.kind == kind && p.id == rid && p.session == session) {
                        Some(i) => {
                            let p = w.parked.remove(i);
                            let world2 = world.clone();
                            let decision = params["decision"].as_str().unwrap_or("approve").to_owned();
                            let answer = params["answers"][0]["selected_labels"][0].as_str().unwrap_or("").to_owned();
                            after = Some(Box::new(move || {
                                tokio::spawn(finish_turn(world2, p, decision, answer));
                            }));
                            Ok(if kind == "approval" {
                                json!({"approval_id": rid, "accepted": true, "status": "accepted", "runtime_resumed": true})
                            } else {
                                json!({"question_id": rid, "accepted": true, "runtime_resumed": true})
                            })
                        }
                        None => Err(json!({"code": -32602, "message": format!("{kind} {rid} is not pending in {session}")})),
                    }
                }
                _ => Ok(json!({})),
            }
        };
        let frame = match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
        };
        if out.send(frame.to_string()).is_err() {
            break;
        }
        if let Some(f) = after {
            f();
        }
    }
    drop(out);
    let _ = writer.await;
    println!("[a20-serve] socket closed");
}

fn env(session: &str, turn: &str, seq: u64, payload: Value) -> Value {
    json!({"session_id": session, "thread_id": turn, "turn_id": turn, "seq": seq,
           "cursor": {"stream": session, "seq": 100 + seq}, "payload": payload})
}

/// The scripted turn: plain, or paused on an approval / a question.
async fn start_turn(world: Shared, session: String, turn: String, prompt: String, n: u64) {
    let step = std::time::Duration::from_millis(400);
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    notify(&world, "turn/started", json!({"session_id": session, "turn_id": turn, "timestamp": "2026-10-02T10:00:00Z"}));
    tokio::time::sleep(step).await;
    let lower = prompt.to_lowercase();
    if let Some(i) = lower.find("rm -rf") {
        let command: String = prompt[i..].split_whitespace().take(3).collect::<Vec<_>>().join(" ");
        let call = format!("call-a20-{n}");
        notify(&world, "projection/envelope", env(&session, &turn, 1, json!({"type": "tool_start", "data": {"tool_call_id": call, "name": "shell", "arguments_preview": command}})));
        tokio::time::sleep(step).await;
        let id = uuid(n, 0xa20a);
        let frame = json!({
            "approval_id": id, "approval_kind": "command", "body": command, "risk": "high",
            "session_id": session, "title": "Approve command", "tool_name": "shell", "turn_id": turn,
            "typed_details": {"kind": "command", "command": {"argv": command.split(' ').collect::<Vec<_>>(),
                              "command_line": command, "tool_call_id": call}}
        });
        world.lock().unwrap().parked.push(Parked { kind: "approval", session: session.clone(), turn: turn.clone(), id, frame: frame.clone() });
        notify(&world, "approval/requested", frame);
        return; // paused until approval/respond on THIS Session
    }
    if lower.contains("ask me") {
        let id = uuid(n, 0xa20b);
        let frame = json!({
            "body": "1. Which color would you like to pick? (options: Blue, Green; or reply with your own)",
            "question_id": id,
            "questions": [{"allow_free_text": true, "header": "Color choice", "multi_select": false,
                           "options": [{"description": "Calm, cool, classic.", "label": "Blue"},
                                       {"description": "Natural and balanced.", "label": "Green"}],
                           "question": "Which color would you like to pick?"}],
            "session_id": session, "title": "Which color would you like to pick?", "turn_id": turn
        });
        world.lock().unwrap().parked.push(Parked { kind: "question", session: session.clone(), turn: turn.clone(), id, frame: frame.clone() });
        notify(&world, "user_question/requested", frame);
        return; // paused until user_question/respond on THIS Session
    }
    notify(&world, "projection/envelope", env(&session, &turn, 1, json!({"type": "assistant_delta", "data": {"text": "Noted.", "assistant_segment_id": format!("{turn}:assistant:iteration:1")}})));
    tokio::time::sleep(step).await;
    notify(&world, "turn/completed", json!({"session_id": session, "turn_id": turn}));
}

/// The answered interaction resumes its own turn and finishes it.
async fn finish_turn(world: Shared, p: Parked, decision: String, answer: String) {
    let step = std::time::Duration::from_millis(300);
    tokio::time::sleep(step).await;
    let (session, turn) = (p.session.clone(), p.turn.clone());
    let text = if p.kind == "approval" {
        notify(&world, "approval/decided", json!({
            "session_id": session, "approval_id": p.id, "turn_id": turn, "decision": decision,
            "decided_at": "2026-10-02T10:00:05Z", "decided_by": "person"
        }));
        let call = p.frame["typed_details"]["command"]["tool_call_id"].clone();
        tokio::time::sleep(step).await;
        let status = if decision == "approve" { "complete" } else { "skipped" };
        notify(&world, "projection/envelope", env(&session, &turn, 2, json!({"type": "tool_end", "data": {"tool_call_id": call, "status": status, "output_preview": ""}})));
        if decision == "approve" { "Done: the scratch dir is gone.".to_owned() } else { "Understood — I left it alone.".to_owned() }
    } else {
        format!("You chose {answer}.")
    };
    tokio::time::sleep(step).await;
    notify(&world, "projection/envelope", env(&session, &turn, 3, json!({"type": "assistant_delta", "data": {"text": text, "assistant_segment_id": format!("{turn}:assistant:iteration:1")}})));
    tokio::time::sleep(step).await;
    notify(&world, "turn/completed", json!({"session_id": session, "turn_id": turn}));
}
