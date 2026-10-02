//! #A2 — board 1's fixture server: ONE port answering both halves of what an
//! `octos serve` answers for board 1, so the real hidden app can walk every
//! board-1 flow by clicks with no model and no machine paths.
//!
//! - **HTTP** (pairing, before any socket): `GET /pair/info` and
//!   `POST /pair/claim` in the shapes a real octos a6ea8505 answered
//!   (`crates/octoscode-client/tests/fixtures/pairing-a6ea8505.jsonl`), and
//!   `POST /api/auth/solo` → 404 (no solo login here).
//! - **WebSocket** (the UI protocol, after pairing): `session/open` (echoes
//!   the requested cwd as `workspace_root`; advertises
//!   `onboarding.workspace_browse.v1` unless `--no-browse`), `session/list`,
//!   `profile/llm/list` (a DeepSeek primary with a stored key),
//!   `profile/llm/catalog` (the board's three DeepSeek models),
//!   `profile/llm/test` (the RECORDED a6ea8505 401 for any typed key not
//!   starting `sk-good`; r29a line 10), `profile/llm/upsert` (the recorded
//!   `applied: true`, r29a line 12), and `onboarding/workspace_list|create`
//!   over a small `/home/user` tree whose `/private` answers the typed
//!   `workspace_list_permission_denied` refusal.
//!
//! ```sh
//! cargo run -p octoscode-module --example board1_serve -- 8422 [--pair ok|used|expired|locked|unsupported|open] [--no-browse]
//! # `open`: /pair/info says pairing_required false (a server without a token; A11's tokenless offer)
//! # it prints the pairing link to paste:  PAIR-LINK http://app.invalid/?octos=http://127.0.0.1:8422&pair=3QK7ZP2M
//! ```
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

const CODE: &str = "3QK7ZP2M";
const TOKEN: &str = "board1-fixture-token";

#[derive(Clone)]
struct Cfg {
    port: u16,
    pair: String,
    browse: bool,
}

type Tree = Arc<Mutex<BTreeMap<String, Vec<String>>>>;

fn tree() -> Tree {
    let mut t = BTreeMap::new();
    let dirs: &[(&str, &[&str])] = &[
        ("/", &["home", "private", "srv"]),
        ("/home", &["user"]),
        ("/home/user", &["code", "Documents"]),
        ("/home/user/code", &["octos", "octoscode-app", "notes", "scratch"]),
        ("/home/user/code/octos", &["crates", "docs", "scripts"]),
        ("/home/user/code/octoscode-app", &["crates", "design", "docs"]),
        ("/home/user/code/notes", &[]),
        ("/home/user/code/scratch", &[]),
        ("/home/user/Documents", &[]),
        ("/srv", &["projects"]),
        ("/srv/projects", &[]),
    ];
    for (k, v) in dirs {
        t.insert((*k).to_owned(), v.iter().map(|s| (*s).to_owned()).collect());
    }
    Arc::new(Mutex::new(t))
}

fn recorded(line: usize) -> serde_json::Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../octoscode-client/tests/fixtures/r29a-onboarding-a6ea8505.jsonl"
    );
    let text = std::fs::read_to_string(path).expect("read the r29a fixture");
    let l = text.lines().nth(line - 1).expect("fixture line");
    serde_json::from_str::<serde_json::Value>(l).expect("json")["body"].clone()
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args.get(1).and_then(|p| p.parse().ok()).unwrap_or(8422);
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let cfg = Cfg {
        port,
        pair: flag("--pair").unwrap_or_else(|| "ok".to_owned()),
        browse: !args.iter().any(|a| a == "--no-browse"),
    };
    let listener = TcpListener::bind(("127.0.0.1", port)).await.expect("bind");
    println!("[board1-serve] listening on 127.0.0.1:{port} (pair={}, browse={})", cfg.pair, cfg.browse);
    println!("PAIR-LINK http://app.invalid/?octos=http://127.0.0.1:{port}&pair={CODE}");
    let claimed = Arc::new(Mutex::new(false));
    let fs = tree();
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue };
        let (cfg, claimed, fs) = (cfg.clone(), claimed.clone(), fs.clone());
        tokio::spawn(async move {
            let mut head = [0u8; 4096];
            let n = stream.peek(&mut head).await.unwrap_or(0);
            let text = String::from_utf8_lossy(&head[..n]).to_ascii_lowercase();
            if text.contains("upgrade: websocket") {
                ws(stream, cfg, fs).await;
            } else {
                http(stream, cfg, claimed).await;
            }
        });
    }
}

async fn http(mut stream: TcpStream, cfg: Cfg, claimed: Arc<Mutex<bool>>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let (head, mut body) = loop {
        let n = stream.read(&mut chunk).await.unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break (String::from_utf8_lossy(&buf[..i]).to_string(), buf[i + 4..].to_vec());
        }
    };
    let len: usize = head
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case("content-length").then(|| v.trim().parse().ok())?
        })
        .unwrap_or(0);
    while body.len() < len {
        let n = stream.read(&mut chunk).await.unwrap_or(0);
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    let mut first = head.lines().next().unwrap_or("").split_whitespace();
    let (method, path) = (first.next().unwrap_or("").to_owned(), first.next().unwrap_or("").to_owned());
    let origin = format!("http://127.0.0.1:{}", cfg.port);
    let kind = |k: &str| (400u16, Some(serde_json::json!({"error": {"kind": k}})));
    if path == "/pair/info" {
        // A11: every discovery probe is one line, so a walk can count them
        // ("one GET, once"; none for a refused origin).
        println!("[board1-serve] {method} /pair/info ({})", if cfg.pair == "unsupported" { 404 } else { 200 });
    }
    let (status, reply) = match (method.as_str(), path.as_str()) {
        (_, "/pair/info" | "/pair/claim") if cfg.pair == "unsupported" => (404, None),
        // `--pair open`: a server running without a bearer token answers
        // `pairing_required: false` (octos `pairing.rs` `pairing_required`).
        ("GET", "/pair/info") => (
            200,
            Some(serde_json::json!({"product": "octos", "version": "2.0.3-rc.13", "pairing_required": cfg.pair != "open", "server_origin": origin})),
        ),
        ("POST", "/pair/claim") => {
            let code = serde_json::from_slice::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v["code"].as_str().map(str::to_owned))
                .unwrap_or_default();
            println!("[board1-serve] POST /pair/claim ({} chars)", code.len());
            // Walk fixtures: the code picks the server's answer, so one run
            // can show every pairing state by clicks alone.
            //   USED0000 -> pair_code_unknown   EXPIRED0 -> pair_code_expired
            //   LOCKED00 -> pair_code_locked    NOPAIR00 -> 404 (no pairing)
            //   SLOWPAIR -> the good claim after 20 s (p4-02 / Cancel)
            if code == "SLOWPAIR" {
                tokio::time::sleep(std::time::Duration::from_secs(20)).await;
            }
            match (cfg.pair.as_str(), code.as_str()) {
                ("used", _) | (_, "USED0000") => kind("pair_code_unknown"),
                ("expired", _) | (_, "EXPIRED0") => kind("pair_code_expired"),
                ("locked", _) | (_, "LOCKED00") => kind("pair_code_locked"),
                (_, "NOPAIR00") => (404, None),
                _ if code.len() != 8 => kind("pair_code_invalid"),
                // The slow code never burns the printed one (a cancelled
                // exchange must leave the real link usable).
                (_, "SLOWPAIR") => (200, Some(serde_json::json!({"token": TOKEN, "server_origin": origin}))),
                _ if code == CODE && !std::mem::replace(&mut *claimed.lock().unwrap(), true) => {
                    (200, Some(serde_json::json!({"token": TOKEN, "server_origin": origin})))
                }
                _ => kind("pair_code_unknown"),
            }
        }
        _ => (404, None),
    };
    let text = reply.map(|v| v.to_string()).unwrap_or_default();
    let resp = format!(
        "HTTP/1.1 {status} {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
        if status == 200 { "OK" } else if status == 404 { "Not Found" } else { "Bad Request" },
        text.len()
    );
    let _ = stream.write_all(resp.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn listing(fs: &Tree, path: Option<&str>) -> Result<serde_json::Value, serde_json::Value> {
    let p = path.unwrap_or("/home/user/code").trim_end_matches('/');
    let p = if p.is_empty() { "/" } else { p };
    if p == "/private" || p.starts_with("/private/") {
        return Err(serde_json::json!({
            "code": -32602,
            "message": "workspace_list: permission denied at /private (EACCES)",
            "data": {"kind": "workspace_list_permission_denied"}
        }));
    }
    let t = fs.lock().unwrap();
    let Some(children) = t.get(p) else {
        return Err(serde_json::json!({
            "code": -32602, "message": "workspace_list: not found",
            "data": {"kind": "workspace_list_not_found"}
        }));
    };
    let parent = if p == "/" {
        serde_json::Value::Null
    } else {
        let cut = p.rfind('/').unwrap_or(0);
        serde_json::json!(if cut == 0 { "/".to_owned() } else { p[..cut].to_owned() })
    };
    let entries: Vec<serde_json::Value> = children
        .iter()
        .map(|c| {
            let child = if p == "/" { format!("/{c}") } else { format!("{p}/{c}") };
            serde_json::json!({"name": c, "path": child, "writable": child != "/private"})
        })
        .collect();
    Ok(serde_json::json!({
        "canonical_path": p,
        "parent_path": parent,
        "writable": p.starts_with("/home/user"),
        "entries": entries,
        "truncated": false,
        // The board's own fixture: the code folder hides three dot-folders.
        "hidden_skipped": if p == "/home/user/code" { 3 } else { 0 },
    }))
}

async fn ws(stream: TcpStream, cfg: Cfg, fs: Tree) {
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
    println!("[board1-serve] ui-protocol socket open");
    let (mut tx, mut rx) = ws.split();
    while let Some(Ok(msg)) = rx.next().await {
        let Message::Text(text) = msg else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        let method = v["method"].as_str().unwrap_or("").to_owned();
        let id = v["id"].clone();
        let params = v.get("params").cloned().unwrap_or(serde_json::Value::Null);
        println!("[board1-serve] <- {method}");
        let mut features = vec!["session.workspace_cwd.v1", "state.session_hydrate.v1", "permission.profile.v1"];
        if cfg.browse {
            features.push("onboarding.workspace_browse.v1");
        }
        let result: Result<serde_json::Value, serde_json::Value> = match method.as_str() {
            "session/open" => {
                let session = params["session_id"].as_str().unwrap_or("octoscode:main").to_owned();
                let root = params["cwd"].as_str().unwrap_or("/home/user/code").to_owned();
                Ok(serde_json::json!({"opened": {
                    "session_id": session,
                    "active_profile_id": "octoscode",
                    "workspace_root": root,
                    "cursor": {"stream": "board1", "seq": 1},
                    "capabilities": {
                        "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                        "capabilities_schema_version": 2,
                        "supported_methods": ["session/open", "session/list", "profile/llm/list", "profile/llm/catalog", "profile/llm/test", "profile/llm/upsert", "onboarding/workspace_list", "onboarding/workspace_create"],
                        "supported_notifications": ["turn/started", "turn/completed"],
                        "supported_features": features,
                    }
                }}))
            }
            "session/list" => Ok(serde_json::json!({"sessions": []})),
            "profile/llm/list" => Ok(serde_json::json!({
                "profile_id": "octoscode",
                "primary": {
                    "family_id": "deepseek", "model_id": "deepseek-v4-flash",
                    "route": {"route_id": "deepseek", "label": "Official API", "api_key_env": "DEEPSEEK_API_KEY", "api_type": "openai"},
                    "has_api_key": true, "selected": true, "available": true
                },
                "fallbacks": []
            })),
            "profile/llm/catalog" => Ok(serde_json::json!({"families": {"deepseek": {"env": "DEEPSEEK_API_KEY", "models": [
                {"id": "deepseek-v4-flash", "endpoints": [{"id": "deepseek", "label": "Official API"}]},
                {"id": "deepseek-v4", "endpoints": [{"id": "deepseek", "label": "Official API"}]},
                {"id": "deepseek-chat", "endpoints": [{"id": "deepseek", "label": "Official API"}]}
            ]}}})),
            "profile/llm/test" => {
                let key = params["api_key"].as_str().unwrap_or("");
                if key.is_empty() || key.starts_with("sk-good") {
                    Ok(serde_json::json!({"profile_id": "octoscode", "applied": true, "message": "ok"}))
                } else {
                    // The RECORDED a6ea8505 refusal (r29a line 10).
                    let mut b = recorded(10);
                    b["profile_id"] = serde_json::json!("octoscode");
                    Ok(b)
                }
            }
            "profile/llm/upsert" => {
                let mut b = recorded(12);
                b["profile_id"] = serde_json::json!("octoscode");
                Ok(b)
            }
            "onboarding/workspace_list" => listing(&fs, params["path"].as_str()),
            "onboarding/workspace_create" => {
                let parent = params["parent"].as_str().unwrap_or("").to_owned();
                let name = params["name"].as_str().unwrap_or("").to_owned();
                let mut t = fs.lock().unwrap();
                match t.get_mut(&parent) {
                    Some(children) => {
                        let created = !children.contains(&name);
                        if created {
                            children.push(name.clone());
                            children.sort_by_key(|c| c.to_lowercase());
                        }
                        let path = format!("{}/{name}", parent.trim_end_matches('/'));
                        t.entry(path.clone()).or_default();
                        Ok(serde_json::json!({"canonical_path": path, "created": created}))
                    }
                    None => Err(serde_json::json!({
                        "code": -32602, "message": "workspace_create: parent not found",
                        "data": {"kind": "workspace_create_parent_not_found"}
                    })),
                }
            }
            _ => Ok(serde_json::json!({})),
        };
        let frame = match result {
            Ok(r) => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(e) => serde_json::json!({"jsonrpc": "2.0", "id": id, "error": e}),
        };
        if tx.send(Message::Text(frame.to_string().into())).await.is_err() {
            break;
        }
    }
    println!("[board1-serve] socket closed");
}
