//! A8 — the session/session-config fixture server: one port answering the UI
//! protocol the Session settings pane, the new-session defaults, the session
//! sidebar and the launch flow need, so the real hidden app can walk them by
//! clicks with no model and no machine paths. Shapes follow the recorded
//! a6ea8505 frames (`crates/octoscode-client/tests/fixtures/`):
//! `session/status/read` (r3-session line 8), `permission/profile/list|set`
//! (r2-profile lines 6/18), `profile/llm/list|select` (r2-profile lines
//! 12/36), `launch/resolve` (r3-session line 6), `session/delete` (r3 line 29);
//! the external-driver trio follows the web decoders
//! (`packages/client/src/external-driver*.ts`).
//!
//! Every request is appended to `--log <file>` as one JSON line
//! `{"method", "params"}` (control tokens redacted) so a walk can assert what
//! reached the wire.
//!
//! ```sh
//! cargo run -p octoscode-module --example a8_serve -- 8428 [--held] [--no-sandbox] \
//!     [--launch activate|resume|cross_profile|no_profile] [--log tmp/a8/serve.jsonl]
//! OCTOS_BASE_URL=http://127.0.0.1:8428 OCTOS_PROFILE_ID=a8 ... (the app)
//! ```
use std::io::Write as _;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

#[derive(Clone)]
struct Cfg {
    held: bool,
    sandbox: bool,
    launch: String,
    log: Option<String>,
}

/// The server's mutable state (shared by every socket).
struct World {
    /// mode, network
    permission: (String, String),
    approval: String,
    selected_model: String,
    /// The foreign seat is still held (until a Resume chat releases it).
    held: bool,
    sessions: Vec<(String, String)>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args.get(1).and_then(|p| p.parse().ok()).unwrap_or(8428);
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let cfg = Cfg {
        held: args.iter().any(|a| a == "--held"),
        sandbox: !args.iter().any(|a| a == "--no-sandbox"),
        launch: flag("--launch").unwrap_or_else(|| "activate".to_owned()),
        log: flag("--log"),
    };
    let world = Arc::new(Mutex::new(World {
        permission: ("workspace_write".into(), "allow".into()),
        approval: "on-request".into(),
        selected_model: "deepseek-v4-flash".into(),
        held: cfg.held,
        sessions: vec![
            ("a8:main".into(), "Fix steer queue drop on reconnect".into()),
            ("a8:alpha".into(), "Add session fork".into()),
            ("a8:beta".into(), "Review PR #2566".into()),
        ],
    }));
    let listener = TcpListener::bind(("127.0.0.1", port)).await.expect("bind");
    println!(
        "[a8-serve] listening on 127.0.0.1:{port} (held={}, sandbox={}, launch={})",
        cfg.held, cfg.sandbox, cfg.launch
    );
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue };
        let (cfg, world) = (cfg.clone(), world.clone());
        tokio::spawn(async move {
            let mut head = [0u8; 4096];
            let n = stream.peek(&mut head).await.unwrap_or(0);
            let text = String::from_utf8_lossy(&head[..n]).to_ascii_lowercase();
            if text.contains("upgrade: websocket") {
                ws(stream, cfg, world).await;
            } else {
                // No HTTP routes here (no solo login, no pairing): 404.
                let mut stream = stream;
                let _ = stream
                    .write_all(b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
                    .await;
            }
        });
    }
}

fn log(cfg: &Cfg, method: &str, params: &Value) {
    let Some(path) = &cfg.log else { return };
    let mut p = params.clone();
    if p.get("control_token").is_some() {
        p["control_token"] = json!("<redacted>");
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{}", json!({"method": method, "params": p}));
    }
}

const METHODS: &[&str] = &[
    "session/open",
    "session/list",
    "session/delete",
    "session/hydrate",
    "session/status/read",
    "turn/start",
    "turn/interrupt",
    "launch/resolve",
    "permission/profile/list",
    "permission/profile/set",
    "profile/llm/list",
    "profile/llm/select",
    "session/driver/get",
    "session/driver/acquire",
    "session/driver/release",
    "thread/graph/get",
    "approval/scopes/list",
];

fn driver_view(w: &World, session: &str, with_ops: bool) -> Value {
    let ops = |items: Vec<Value>| json!({"items": items, "snapshot": "snap-1", "observed_revision": "9", "complete": true, "next_cursor": null});
    if w.held {
        let row = |id: &str, slug: &str, life: &str| {
            json!({
                "operation_id": id, "kind": "peer_dispatch", "lifecycle": life, "created_at_ms": 1_790_000_000_000u64,
                "acceptance": {
                    "model": "deepseek-v4-flash", "model_lane": "strong", "workspace_root": "/home/user/octos",
                    "adopted_turn_id": "01920000-0000-7000-8000-000000000301",
                    "adopted_session_id": format!("{session}#peer-{slug}"), "slug": slug,
                    "accepted_at_ms": 1_790_000_000_500u64, "payload_digest": "sha256:fixture"
                }
            })
        };
        let mut v = json!({
            "mode": "external", "recovery": "none",
            "binding": {"driver_id": "octos-tui", "epoch": 4, "revision": 9, "lease_expires_at_ms": now_ms() + 300_000}
        });
        if with_ops {
            v["operations"] = ops(vec![row("op-0001", "review-diff", "started"), row("op-0002", "write-tests", "terminal")]);
        }
        v
    } else {
        let mut v = json!({"mode": "internal", "recovery": "none", "binding": null});
        if with_ops {
            v["operations"] = ops(vec![]);
        }
        v
    }
}

async fn ws(stream: TcpStream, cfg: Cfg, world: Arc<Mutex<World>>) {
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
    println!("[a8-serve] ui-protocol socket open");
    let (mut tx, mut rx) = ws.split();
    while let Some(Ok(msg)) = rx.next().await {
        let Message::Text(text) = msg else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        let method = v["method"].as_str().unwrap_or("").to_owned();
        let id = v["id"].clone();
        let params = v.get("params").cloned().unwrap_or(Value::Null);
        log(&cfg, &method, &params);
        println!("[a8-serve] <- {method}");
        let session = params["session_id"].as_str().unwrap_or("a8:main").to_owned();
        let mut features = vec![
            "session.workspace_cwd.v1",
            "state.session_hydrate.v1",
            "permission.profile.v1",
            "runtime.policy_stamp.v1",
            "external_driver_v1",
            "state.thread_graph.v1",
        ];
        if cfg.sandbox {
            features.push("session.sandbox.v1");
        }
        let result: Result<Value, Value> = {
            let mut w = world.lock().unwrap();
            match method.as_str() {
                "session/open" => {
                    let root = params["cwd"].as_str().unwrap_or("/home/user/octos").to_owned();
                    if !w.sessions.iter().any(|(s, _)| *s == session) {
                        w.sessions.insert(0, (session.clone(), String::new()));
                    }
                    Ok(json!({"opened": {
                        "session_id": session,
                        "active_profile_id": "a8",
                        "workspace_root": root,
                        "cursor": {"stream": session, "seq": 1},
                        "capabilities": {
                            "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                            "capabilities_schema_version": 2,
                            "supported_methods": METHODS,
                            "supported_notifications": ["turn/started", "turn/completed"],
                            "supported_features": features,
                        }
                    }}))
                }
                "session/list" => Ok(json!({"sessions": w.sessions.iter().map(|(s, t)| {
                    let mut row = json!({"id": s, "message_count": 2, "updated_at": "2026-10-01T09:00:00Z"});
                    if !t.is_empty() { row["title"] = json!(t); }
                    row
                }).collect::<Vec<_>>()})),
                "session/delete" => {
                    if session.contains("locked") {
                        Err(json!({"code": -32000, "message": "session is busy", "data": {"kind": "session_busy"}}))
                    } else {
                        w.sessions.retain(|(s, _)| *s != session);
                        Ok(json!({}))
                    }
                }
                "launch/resolve" => Ok(match cfg.launch.as_str() {
                    "no_profile" => json!({"decision": "no_profile"}),
                    "cross_profile" => json!({"decision": "cross_profile", "resolved_profile": "a8", "existing_profiles": ["glm-coder"]}),
                    d => json!({"decision": d, "resolved_profile": "a8"}),
                }),
                // The recorded r3-session line 8 shape.
                "session/status/read" => Ok(json!({
                    "session_id": session,
                    "profile_id": "a8",
                    "model": {"model": w.selected_model, "provider": "deepseek", "selected": true},
                    "permission_profile": w.permission.0,
                    "sandbox": "workspace-write",
                    "cursor": {"healthy": true, "replay_supported": true},
                    "health": {"status": "ok"},
                    "runtime_policy_stamp": {
                        "approval_policy": w.approval, "network": if w.permission.1 == "allow" { "allowed" } else { "blocked" },
                        "sandbox_mode": "workspace-write", "model": w.selected_model, "profile_id": "a8"
                    }
                })),
                // r2-profile line 6.
                "permission/profile/list" => Ok(json!({
                    "session_id": session,
                    "current": {"mode": w.permission.0, "network": w.permission.1},
                    "profiles": [
                        {"mode": "read_only", "network": "deny"},
                        {"mode": "workspace_write", "network": "deny"},
                        {"mode": "danger_full_access", "network": "allow"}
                    ]
                })),
                // r2-profile line 18.
                "permission/profile/set" => {
                    let u = &params["update"];
                    if let Some(m) = u["mode"].as_str() {
                        w.permission.0 = m.to_owned();
                    }
                    if let Some(n) = u["network"].as_str() {
                        w.permission.1 = n.to_owned();
                    }
                    if let Some(a) = u["approval_policy"].as_str() {
                        w.approval = if a == "ask" { "on-request".into() } else { a.to_owned() };
                    }
                    Ok(json!({"applied": true, "session_id": session, "current": {"mode": w.permission.0, "network": w.permission.1}}))
                }
                // r2-profile line 12 (primary + fallbacks).
                "profile/llm/list" => {
                    let entry = |model: &str, family: &str, route: &str, label: &str| json!({
                        "model_id": model, "family_id": family, "route_id": route,
                        "route": {"route_id": route, "label": label, "api_type": "openai"},
                        "has_api_key": true, "available": true, "selected": w.selected_model == model
                    });
                    let primary = if w.selected_model == "glm-5" {
                        entry("glm-5", "zhipu", "zhipu", "BigModel API")
                    } else {
                        entry("deepseek-v4-flash", "deepseek", "deepseek", "Official API")
                    };
                    let fallback = if w.selected_model == "glm-5" {
                        entry("deepseek-v4-flash", "deepseek", "deepseek", "Official API")
                    } else {
                        entry("glm-5", "zhipu", "zhipu", "BigModel API")
                    };
                    Ok(json!({"profile_id": "a8", "llm": {"primary": primary, "fallbacks": [fallback]}, "primary": primary, "fallbacks": [fallback]}))
                }
                // r2-profile line 36: restart_required keeps the boot model.
                "profile/llm/select" => {
                    let model = params["model_id"].as_str().unwrap_or("").to_owned();
                    let running = w.selected_model.clone();
                    if model == running {
                        Ok(json!({"applied": true, "session_id": session, "runtime_disposition": "unchanged"}))
                    } else {
                        w.selected_model = model.clone();
                        Ok(json!({
                            "applied": true, "session_id": session, "effective_from": "next_turn",
                            "restart_required": true, "runtime_disposition": "restart_required",
                            "runtime_policy_stamp": {"model": running, "approval_policy": w.approval}
                        }))
                    }
                }
                "session/driver/get" => Ok(driver_view(&w, &session, params.get("operations").is_some())),
                "session/driver/acquire" => {
                    let driver = params["driver_id"].as_str().unwrap_or("").to_owned();
                    Ok(json!({
                        "control_token": "fixture-control-token",
                        "binding": {"driver_id": driver, "epoch": 5, "revision": 10, "lease_expires_at_ms": now_ms() + 60_000},
                        "recovery": "none"
                    }))
                }
                "session/driver/release" => {
                    w.held = false;
                    Ok(json!({"mode": "internal", "recovery": "none"}))
                }
                "session/hydrate" => Ok(json!({
                    "session_id": session, "cursor": {"stream": session, "seq": 1}, "messages": []
                })),
                "thread/graph/get" => Ok(json!({"session_id": session, "cursor": {"stream": session, "seq": 1}, "threads": [], "orphans": []})),
                "approval/scopes/list" => Ok(json!({"scopes": []})),
                _ => Ok(json!({})),
            }
        };
        let frame = match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
        };
        if tx.send(Message::Text(frame.to_string().into())).await.is_err() {
            break;
        }
    }
    println!("[a8-serve] socket closed");
}
