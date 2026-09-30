//! #29b — Stage C wiring for board 2.4/2.5/2.6, replayed over recorded real
//! traffic shapes (`crates/octoscode-client/tests/fixtures/r2-profile-a6ea8505.jsonl`
//! — the a6ea8505/dsflash profile run that carried `onboarding/workspace_list`,
//! `onboarding/workspace_create`, `permission/profile/set+list` and
//! `profile/llm/select`; LESSONS "tests replay recorded real traffic").
//!
//! §1 `ws.refresh` reads the three tables and the bindings project them.
//! §2 `ws.open` drives `session/open` with the picked row's path (parity 162).
//! §3 `set.permission.set` sends the picked mode and reads the reply's
//!     `current` back (parity 102, permissions-section.test.tsx:51).
//! §4 `set.model.select` maps the recorded `restart_required` disposition to
//!     the web's exact message (SessionConfigPane.tsx:73-76).
//! §5 the browse affordance fails closed without the advertised feature
//!     (parity 166) and folder names are pre-validated before any request
//!     (parity 165, workspace-browse.ts:160-175).
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, FlowUi};
use octoscode_module::screens::workspace as ws;
use octoscode_store::Store;

/// A recorded `dir`/`method`/`body` line (same shape as the f26 loader).
#[derive(Debug, Clone)]
struct Frame {
    dir: String,
    method: String,
    body: serde_json::Value,
}

/// The committed profile recording, scrubbed to `<WORKSPACE>`/`<TMP>`.
fn r2_frames() -> Vec<Frame> {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../octoscode-client/tests/fixtures/r2-profile-a6ea8505.jsonl"
    ))
    .expect("read r2-profile fixture");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v.get("body").cloned().unwrap_or(serde_json::Value::Null),
            }
        })
        .collect()
}

/// The first recorded request/response pair for `method`.
fn r2_pair(method: &str) -> (serde_json::Value, serde_json::Value) {
    let frames = r2_frames();
    let req = frames
        .iter()
        .find(|f| f.dir == "out" && f.method == method)
        .map(|f| f.body.clone())
        .unwrap_or_else(|| panic!("fixture has an outbound {method}"));
    let res = frames
        .iter()
        .find(|f| f.dir == "in" && f.method == method)
        .map(|f| f.body.clone())
        .unwrap_or_else(|| panic!("fixture has the {method} reply body"));
    (req, res)
}

/// A fake WS server answering the board-2 RPCs with the recorded bodies
/// (`<WORKSPACE>` → a concrete root), and recording `(method, params)`.
struct Board2Server {
    base_url: String,
    received: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
}

impl Board2Server {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(Mutex::new(Vec::new()));
        let rx = received.clone();

        // The recorded bodies, with the scrub tokens made concrete.
        let (_, ws_list) = r2_pair("onboarding/workspace_list");
        let ws_list = serde_json::to_string(&ws_list)
            .unwrap()
            .replace("<WORKSPACE>", "/tmp/ws29b");
        let ws_list: serde_json::Value = serde_json::from_str(&ws_list).unwrap();
        let (_, perm_list) = r2_pair("permission/profile/list");
        let (_, llm_list) = r2_pair("profile/llm/list");
        let (_, llm_select) = r2_pair("profile/llm/select");

        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let Ok(wsstream) = tokio_tungstenite::accept_async(stream).await else {
                return;
            };
            let (mut tx, mut rx_in) = wsstream.split();
            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                let params = v["params"].clone();
                rx.lock().unwrap().push((method.clone(), params.clone()));

                let result = match method.as_str() {
                    "session/open" => serde_json::json!({"opened": {
                        "session_id": params["session_id"].as_str().unwrap_or("dsflash:main"),
                        "active_profile_id": "dsflash",
                        "cursor": {"stream": "dsflash:main", "seq": 1},
                        "capabilities": {
                            "version": {"protocol": "octos-ui/v1alpha1",
                                        "schema_version": 1, "jsonrpc": "2.0"},
                            "capabilities_schema_version": 1,
                            "supported_methods": ["session/open", "session/list",
                                                  "onboarding/workspace_list",
                                                  "onboarding/workspace_create",
                                                  "permission/profile/list",
                                                  "permission/profile/set",
                                                  "profile/llm/list", "profile/llm/select"],
                            "supported_notifications": [],
                            "supported_features": []
                        }
                    }}),
                    "onboarding/workspace_list" => ws_list.clone(),
                    "permission/profile/list" => perm_list.clone(),
                    // Read-back: the applied mode comes back as `current`
                    // (permissions-section.test.tsx:51).
                    "permission/profile/set" => serde_json::json!({
                        "applied": true,
                        "current": {"mode": params["update"]["mode"], "network": "allow"},
                        "session_id": params["session_id"]
                    }),
                    "profile/llm/list" => llm_list.clone(),
                    "profile/llm/select" => llm_select.clone(),
                    "onboarding/workspace_create" => serde_json::json!({
                        "canonical_path": format!(
                            "{}/{}",
                            params["parent"].as_str().unwrap_or("/tmp/ws29b"),
                            params["name"].as_str().unwrap_or("x")
                        ),
                        "created": true
                    }),
                    _ => serde_json::json!({}),
                };
                let frame =
                    serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result});
                let _ = tx.send(Message::Text(frame.to_string().into())).await;
            }
        });

        Self {
            base_url: format!("http://{addr}"),
            received,
        }
    }

    fn methods(&self) -> Vec<String> {
        self.received
            .lock()
            .unwrap()
            .iter()
            .map(|(m, _)| m.clone())
            .collect()
    }

    fn params_of(&self, method: &str) -> Option<serde_json::Value> {
        self.received
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|(m, _)| m == method)
            .map(|(_, p)| p.clone())
    }
}

async fn wired() -> (Conversation, Board2Server, Arc<Store>, Mutex<FlowUi>) {
    let server = Board2Server::start().await;
    let (conv, _events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
        .expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    tokio::time::sleep(Duration::from_millis(80)).await;
    let store = Arc::new(octoscode_store::Store::new());
    let ui = Mutex::new(FlowUi::default());
    (conv, server, store, ui)
}

fn ctx<'a>(store: &'a Arc<Store>, ui: &'a Mutex<FlowUi>) -> Ctx<'a> {
    Ctx::new(store, ui)
}

/// §1 — the refresh reads the three recorded tables; the board-2 bindings
/// project them (picker rows, permission mode, model).
#[tokio::test]
async fn refresh_reads_the_three_tables_and_the_bindings_project_them() {
    let (conv, _server, store, ui) = wired().await;
    ws::apply(ws::Effect::Refresh, &conv)
        .await
        .expect("refresh");

    let c = ctx(&store, &ui);
    assert_eq!(
        ws::query(&c, "ws.server_folder").unwrap(),
        serde_json::json!("/tmp/ws29b"),
        "parity 163: the server root is the first picker entry"
    );
    let recent = ws::query(&c, "ws.recent").unwrap();
    assert_eq!(
        recent.as_array().unwrap().len(),
        10,
        "the recorded listing's rows reach the picker"
    );
    assert_eq!(
        ws::query(&c, "set.permission_mode").unwrap(),
        serde_json::json!("workspace_write"),
        "the recorded current mode"
    );
    assert_eq!(
        ws::query(&c, "set.model").unwrap(),
        serde_json::json!("deepseek-v4-flash"),
        "the recorded primary model"
    );
}

/// §2 — `ws.open[index]` opens a session AT the row's path (parity 162: an
/// explicit path reaches `session/open` as `cwd`).
#[tokio::test]
async fn ws_open_drives_session_open_at_the_picked_path() {
    let (conv, server, store, ui) = wired().await;
    ws::apply(ws::Effect::Refresh, &conv).await.expect("refresh");

    let c = ctx(&store, &ui);
    let effect = ws::resolve("ws.open", 1, &c);
    let path = match &effect {
        ws::Effect::Open(p) => p.clone(),
        other => panic!("expected Open, got {other:?}"),
    };
    ws::apply(effect, &conv).await.expect("open");
    tokio::time::sleep(Duration::from_millis(80)).await;

    assert!(
        server.methods().iter().any(|m| m == "session/open"),
        "the open went over the production client"
    );
    let params = server.params_of("session/open").expect("open params");
    assert_eq!(
        params["cwd"], serde_json::json!(path),
        "session/open carries the picked workspace path"
    );
}

/// §3 — the segment's mode reaches `permission/profile/set` and the reply's
/// `current` is the read-back (parity 102).
#[tokio::test]
async fn permission_set_sends_the_picked_mode_and_reads_back() {
    let (conv, server, store, ui) = wired().await;
    ws::apply(ws::Effect::Refresh, &conv).await.expect("refresh");

    let c = ctx(&store, &ui);
    // The recorded profiles[0] is read_only.
    let effect = ws::resolve("set.permission.set", 0, &c);
    assert_eq!(effect, ws::Effect::SetPermission("read_only".to_owned()));
    ws::apply(effect, &conv).await.expect("set");

    let params = server
        .params_of("permission/profile/set")
        .expect("set params");
    assert_eq!(params["update"]["mode"], serde_json::json!("read_only"));
    assert_eq!(params["session_id"], serde_json::json!("dsflash:main"));
    assert_eq!(
        ws::query(&c, "set.permission_mode").unwrap(),
        serde_json::json!("read_only"),
        "the reply's current mode is the read-back"
    );
}

/// §4 — the recorded `restart_required` disposition maps to the web's exact
/// message (SessionConfigPane.tsx:73-76).
#[tokio::test]
async fn model_select_maps_the_recorded_disposition_to_the_exact_copy() {
    let (conv, _server, store, ui) = wired().await;
    ws::apply(ws::Effect::Refresh, &conv).await.expect("refresh");

    let c = ctx(&store, &ui);
    ws::apply(ws::resolve("set.model.select", 0, &c), &conv)
        .await
        .expect("select");
    assert_eq!(
        ws::query(&c, "set.save_notice").unwrap(),
        serde_json::json!("Saved. The server keeps running deepseek-v4-flash until it restarts"),
        "the exact web copy for restart_required"
    );
    // The same reply's fixed-at-open stamp feeds the read-only sandbox line
    // (parity 103).
    assert_eq!(
        ws::query(&c, "set.sandbox").unwrap(),
        serde_json::json!("Enabled · Network allowed · workspace"),
        "the sandbox line is read from the stamp"
    );
}

/// §5a — without the advertised feature the browse action fails closed
/// (parity 166, workspace-browse-adapter.ts:25).
#[test]
fn browse_fails_closed_without_the_advertised_feature() {
    let store = Arc::new(octoscode_store::Store::new());
    let ui = Mutex::new(FlowUi::default());
    let c = ctx(&store, &ui);
    assert_eq!(
        ws::resolve("ws.browse", 0, &c),
        ws::Effect::Unhandled("ws.browse[not-advertised]".to_owned()),
        "no capability → Unhandled, never a request"
    );
    assert_eq!(
        ws::query(&c, "ws.browse_visible").unwrap(),
        serde_json::json!(false),
        "the affordance binding hides too"
    );
}

/// §5b — a folder name the web would refuse never reaches the wire
/// (parity 165, workspace-browse.ts:160-175).
#[tokio::test]
async fn create_folder_prevalidates_before_any_request() {
    let (conv, server, store, ui) = wired().await;
    ws::apply(ws::Effect::Refresh, &conv).await.expect("refresh");

    ui.lock().unwrap().set_draft_inner("bad/name");
    let c = ctx(&store, &ui);
    let err = ws::apply(ws::resolve("ws.create_folder", 0, &c), &conv)
        .await
        .expect_err("a separator name is refused locally");
    assert!(err.contains("separator"), "{err}");
    assert!(
        !server.methods().iter().any(|m| m == "onboarding/workspace_create"),
        "the refused name never reaches the wire"
    );

    // A valid name goes out under the server root.
    ui.lock().unwrap().set_draft_inner("r2-workspace-probe");
    ws::apply(ws::resolve("ws.create_folder", 0, &c), &conv)
        .await
        .expect("create");
    let params = server
        .params_of("onboarding/workspace_create")
        .expect("create params");
    assert_eq!(params["name"], serde_json::json!("r2-workspace-probe"));
    assert_eq!(params["parent"], serde_json::json!("/tmp/ws29b"));
}

/// Coverage contract: every declared board-2 action routes, and every declared
/// binding id resolves on a refreshed state (the same contract the
/// conversation table's tests keep).
#[tokio::test]
async fn every_declared_id_is_live() {
    let (conv, _server, store, ui) = wired().await;
    ws::apply(ws::Effect::Refresh, &conv).await.expect("refresh");
    // The picker's field carries the folder name; a non-empty draft keeps
    // ws.create_folder on its CreateFolder arm for this coverage walk.
    ui.lock().unwrap().set_draft_inner("probe");
    let c = ctx(&store, &ui);
    for (id, _) in ws::ACTIONS {
        assert!(ws::is_action(id));
        assert!(ws::is_routed(id), "{id} declared but unrouted");
        // Route each id once: an Unhandled here must be a documented gate
        // (browse without the feature), never a missing arm.
        match ws::resolve(id, 0, &c) {
            ws::Effect::Unhandled(u) => assert!(
                u.contains("not-advertised"),
                "{id} routed to Unhandled({u})"
            ),
            _ => {}
        }
    }
    for (id, _) in ws::BINDINGS {
        assert!(
            ws::query(&c, id).is_some(),
            "binding {id} declared but resolves None"
        );
    }
}
