//! A10 — the composer's two control seats through their production path
//! (web `SessionControlBar`): `board3::host::open(Dialog::Permission |
//! Dialog::ModelMenu)` (what the seat clicks run), `host::perform` for the
//! menu rows, `host::run` for the jobs (`board3::seats::{load_permission,
//! set_permission, load_models, select_model}`) against a WebSocket server
//! answering with the recorded r2-profile permission list / set and select
//! replies, and `a10-seats-faithful.jsonl` (the session-scoped model list,
//! a reloaded select and a refused one — no recording carries them).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::Conversation;
use octoscode_module::screens::board3::host::{self, Dialog, Job, Outcome};
use octoscode_module::screens::board3::seats;
use octoscode_store::domains::models::Disposition;

fn frames(name: &str) -> Vec<Value> {
    let path = format!("{}/../octoscode-client/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("JSON"))
        .collect()
}

fn dir(name: &str, d: &str, method: &str) -> Vec<Value> {
    frames(name)
        .into_iter()
        .filter(|f| f["dir"] == d && f["method"] == method && f.get("body").is_some())
        .map(|f| f["body"].clone())
        .collect()
}

const R2: &str = "r2-profile-a6ea8505.jsonl";
const SEATS: &str = "a10-seats-faithful.jsonl";

fn recorded_open() -> Value {
    dir("r1-autonomy-a6ea8505.jsonl", "in", "session/open")
        .into_iter()
        .find(|b| b.get("active_profile_id").is_some())
        .expect("r1 open")
}

struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Server {
    async fn start(canned: Vec<(String, Value)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let mut queues: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for (m, b) in canned {
            queues.entry(m).or_default().push(b);
        }
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (mut tx, mut rx) = ws.split();
            let mut pos: BTreeMap<String, usize> = BTreeMap::new();
            while let Some(Ok(msg)) = rx.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                log.lock().unwrap().push((method.clone(), v["params"].clone()));
                let body = match queues.get(&method) {
                    Some(list) => {
                        let k = pos.entry(method.clone()).or_insert(0);
                        let b = list[(*k).min(list.len() - 1)].clone();
                        *k += 1;
                        b
                    }
                    None => json!({}),
                };
                let frame = match body.get("__error__") {
                    Some(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
                    None => json!({"jsonrpc": "2.0", "id": id, "result": body}),
                };
                let _ = tx.send(Message::Text(frame.to_string().into())).await;
            }
        });
        Self { base_url: format!("http://{addr}"), seen }
    }

    fn sent(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }
}

async fn connect(server: &Server) -> Arc<Conversation> {
    let (conv, mut events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    let conv = Arc::new(conv);
    let drv = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            let _ = drv.on_event(evt);
        }
    });
    conv.open_workspace(None).await.expect("session/open");
    conv.store.domains.session.set_active(Some(conv.session_id()));
    conv.store.domains.profile.set_current("dsflash".into());
    conv
}

fn serial() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

fn base() -> Vec<(String, Value)> {
    vec![("session/open".into(), json!({"opened": recorded_open()}))]
}

/// Run what an open or a control routed (one job) and return its result.
async fn run(conv: &Conversation, out: Outcome) -> Option<Result<String, String>> {
    match out {
        Outcome::Spawn(job) => Some(host::run(job, conv).await),
        Outcome::Done => None,
        other => panic!("{other:?}"),
    }
}

fn dsl(conv: &Conversation) -> host::Lowered {
    host::lower_open(&conv.store).expect("a menu is open")
}

fn taps(m: &host::Lowered) -> Vec<String> {
    m.taps.iter().map(|(_, e)| e.clone()).collect()
}

/// The permission seat: the recorded list (current first, then the other
/// profiles), a standard preset set directly with the web's update shape,
/// the recorded read-back, the list re-read, the menu closed.
#[tokio::test]
async fn the_permission_seat_lists_the_presets_and_sets_a_standard_one() {
    let _s = serial();
    let mut canned = base();
    let list = dir(R2, "in", "permission/profile/list").remove(0);
    let set = dir(R2, "in", "permission/profile/set").remove(0);
    // The re-read after the set reports the set's read-back as current.
    let mut after = list.clone();
    after["current"] = set["current"].clone();
    canned.push(("permission/profile/list".into(), list));
    canned.push(("permission/profile/list".into(), after));
    canned.push(("permission/profile/set".into(), set));
    let server = Server::start(canned).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    host::reset();
    run(&conv, host::open(Dialog::Permission)).await.expect("a load").expect("listed");
    assert_eq!(server.sent("permission/profile/list"), dir(R2, "out", "permission/profile/list"), "the recorded list params");
    let names: Vec<String> = seats::permission_options(&store).iter().map(|o| o.name()).collect();
    assert_eq!(names, ["Write · Network allowed", "Read · Network blocked", "Write · Network blocked", "Full access · Network allowed"]);
    let m = dsl(&conv);
    for want in ["Permission", "Read · Network blocked", "Full access · Network allowed", "b3_shield_danger.svg"] {
        assert!(m.dsl.contains(want), "{want}");
    }
    let t = taps(&m);
    assert!(t.contains(&"b3.perm.choose#1".to_owned()) && t.contains(&"b3.perm.choose#3".to_owned()));
    // The selected preset only closes the menu (`choose` -> `closeMenu()`,
    // intent none): checked last, below.
    assert!(t.contains(&"b3.perm.choose#0".to_owned()));
    assert!(t.contains(&"b3.close".to_owned()), "an outside press dismisses the menu");
    // A standard preset: set directly.
    let out = host::perform("b3.perm.choose", 1, &store);
    assert_eq!(out, Outcome::Spawn(Job::PermissionSet("read_only", "deny")));
    run(&conv, out).await.expect("a job").expect("applied");
    assert_eq!(
        server.sent("permission/profile/set")[0],
        json!({"session_id": conv.session_id(), "update": {"mode": "read_only", "network": "deny"}}),
        "the web's update: the whole preset (mode + network)"
    );
    // The recorded read-back (read_only / allow) is current; the list re-read.
    let cur = store.domains.profile.permission().expect("current");
    assert_eq!((seats::mode_wire(cur.mode), seats::network_wire(cur.network)), ("read_only", "allow"));
    assert_eq!(server.sent("permission/profile/list").len(), 2);
    assert!(!host::is_open(), "the menu closes on a choice");
    // Choosing the selected preset closes the menu and sends nothing.
    run(&conv, host::open(Dialog::Permission)).await;
    assert_eq!(host::perform("b3.perm.choose", 0, &store), Outcome::Done);
    assert!(!host::is_open());
    assert_eq!(server.sent("permission/profile/set").len(), 1);
}

/// A dangerous preset is never a direct select: the confirmation shows the
/// web's copy, Enable is inert until the box is ticked, Cancel sends nothing.
#[tokio::test]
async fn full_access_goes_through_the_acknowledged_confirmation() {
    let _s = serial();
    let mut canned = base();
    canned.push(("permission/profile/list".into(), dir(R2, "in", "permission/profile/list").remove(0)));
    canned.push(("permission/profile/set".into(), json!({"applied": true, "current": {"mode": "danger_full_access", "network": "allow"}, "session_id": "dsflash:main"})));
    let server = Server::start(canned).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    host::reset();
    run(&conv, host::open(Dialog::Permission)).await;
    assert_eq!(host::perform("b3.perm.choose", 3, &store), Outcome::Done, "no direct select");
    let m = dsl(&conv);
    for want in [seats::RISK_TITLE, seats::RISK_DESCRIPTION, "Filesystem access", "Full access", "Network allowed", seats::RISK_ACK, seats::RISK_HINT] {
        assert!(m.dsl.contains(want), "{want}");
    }
    assert!(!taps(&m).contains(&"b3.perm.confirm".to_owned()), "inert until ticked");
    assert_eq!(host::perform("b3.perm.confirm", 0, &store), Outcome::Done);
    // Cancel: nothing sent, back to the menu.
    host::perform("b3.perm.cancel", 0, &store);
    assert!(dsl(&conv).dsl.contains("Read · Network blocked"));
    // Tick, then Enable full access.
    host::perform("b3.perm.choose", 3, &store);
    host::perform("b3.perm.ack", 0, &store);
    let m = dsl(&conv);
    assert!(taps(&m).contains(&"b3.perm.confirm".to_owned()) && !m.dsl.contains(seats::RISK_HINT));
    let out = host::perform("b3.perm.confirm", 0, &store);
    assert_eq!(out, Outcome::Spawn(Job::PermissionSet("danger_full_access", "allow")));
    assert!(server.sent("permission/profile/set").is_empty(), "nothing before the confirm");
    run(&conv, out).await.expect("a job").expect("applied");
    assert_eq!(server.sent("permission/profile/set")[0]["update"], json!({"mode": "danger_full_access", "network": "allow"}));
}

/// The reply checks: an unapplied set and a wrong-session list are errors
/// shown in the menu; an unadvertised preset never reaches the wire.
#[tokio::test]
async fn permission_replies_are_checked() {
    let _s = serial();
    let mut canned = base();
    let mut list = dir(R2, "in", "permission/profile/list").remove(0);
    canned.push(("permission/profile/list".into(), list.clone()));
    list["session_id"] = json!("someone:else");
    canned.push(("permission/profile/list".into(), list));
    canned.push(("permission/profile/set".into(), json!({"applied": false, "current": {"mode": "workspace_write", "network": "allow"}, "session_id": "dsflash:main"})));
    let server = Server::start(canned).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    host::reset();
    run(&conv, host::open(Dialog::Permission)).await;
    let r = run(&conv, host::perform("b3.perm.choose", 2, &store)).await.expect("a job");
    assert_eq!(r, Err("The server did not apply the permission change".into()));
    assert!(dsl(&conv).dsl.contains("The server did not apply the permission change"));
    let r = run(&conv, host::perform("b3.perm.retry", 0, &store)).await.expect("a job");
    assert_eq!(r, Err("permission/profile/list returned another session".into()));
    // A preset the server never offered.
    let r = seats::set_permission(&conv, "danger_full_access", "deny").await;
    assert_eq!(r, Err("The requested permission profile was not advertised for this session".into()));
    assert_eq!(server.sent("permission/profile/set").len(), 1);
}

/// The model seat: the session list grouped by provider; selecting the r2
/// fallback sends exactly the recorded select and answers with its
/// restart_required disposition ("keeps naming the model the server still
/// runs"); a reloaded select for another selection keeps that notice and
/// adds its own; the seat names the selected model.
#[tokio::test]
async fn the_model_seat_groups_selects_and_keeps_the_notice_board() {
    let _s = serial();
    let mut canned = base();
    let list = dir(SEATS, "in", "profile/llm/list").remove(0);
    canned.push(("profile/llm/list".into(), list));
    canned.push(("profile/llm/select".into(), dir(R2, "in", "profile/llm/select").remove(0)));
    canned.push(("profile/llm/select".into(), dir(SEATS, "in", "profile/llm/select").remove(0)));
    let server = Server::start(canned).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    let session = conv.session_id();
    host::reset();
    run(&conv, host::open(Dialog::ModelMenu)).await.expect("a load").expect("listed");
    assert_eq!(server.sent("profile/llm/list")[0], json!({"session_id": session, "profile_id": "dsflash"}));
    let m = dsl(&conv);
    for want in ["Deepseek", "Moonshot", "Zhipu", "DeepSeek V4 Flash", "Kimi K3", "GLM-4 Flash", seats::MODEL_UNAVAILABLE_REASON] {
        assert!(m.dsl.contains(want), "{want}");
    }
    let t = taps(&m);
    assert!(t.contains(&"b3.model.choose#1".to_owned()) && t.contains(&"b3.model.choose#2".to_owned()));
    // The selected model stays pressable but asks for nothing
    // (`modelSelectionIntent` none: `choose` returns).
    assert_eq!(host::perform("b3.model.choose", 0, &store), Outcome::Done);
    assert!(!t.contains(&"b3.model.choose#3".to_owned()), "an unavailable model routes nothing");
    assert_eq!(seats::model_seat_label(&store), "DeepSeek V4 Flash");
    // The r2-route fallback: the recorded select, exactly.
    let out = host::perform("b3.model.choose", 1, &store);
    assert_eq!(out, Outcome::Spawn(Job::ModelSelect(1)));
    let r = run(&conv, out).await.expect("a job");
    assert_eq!(r.as_deref(), Ok("Saved. The server keeps running deepseek-v4-flash until it restarts"));
    let mut recorded = dir(R2, "out", "profile/llm/select").remove(0);
    recorded["session_id"] = json!(session);
    assert_eq!(server.sent("profile/llm/select")[0], recorded, "the recorded select params");
    assert_eq!(server.sent("profile/llm/list").len(), 2, "the list is re-read after the select");
    let board = store.domains.models.board(&session);
    assert_eq!(board.latest().map(|n| n.kind), Some(Disposition::RestartRequired));
    assert!(dsl(&conv).dsl.contains("Saved. The server keeps running deepseek-v4-flash until it restarts"));
    // Kimi: reloaded — its own notice; the other selection's restart stays.
    run(&conv, host::perform("b3.model.choose", 2, &store)).await.expect("a job").expect("reloaded");
    let board = store.domains.models.board(&session);
    let kinds: Vec<Disposition> = board.notices.iter().map(|n| n.kind).collect();
    assert_eq!(kinds, [Disposition::RestartRequired, Disposition::Reloaded]);
    assert_eq!(board.latest().unwrap().message, "Saved. Your next message uses kimi-k3");
    assert!(!board.saving);
}

/// A refused select (applied:false) is "Couldn't save: …", the selection
/// reverts; the board is per Session and survives a reopen.
#[tokio::test]
async fn a_refused_select_reverts_and_the_board_is_per_session() {
    let _s = serial();
    let mut canned = base();
    let mut list = dir(SEATS, "in", "profile/llm/list").remove(0);
    // The primary is not selected here, so selecting it is a real request.
    list["models"][0]["selected"] = json!(false);
    list["models"][2]["selected"] = json!(true);
    canned.push(("profile/llm/list".into(), list));
    canned.push(("profile/llm/select".into(), dir(SEATS, "in", "profile/llm/select").remove(1)));
    let server = Server::start(canned).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    let session = conv.session_id();
    host::reset();
    run(&conv, host::open(Dialog::ModelMenu)).await;
    let r = run(&conv, host::perform("b3.model.choose", 0, &store)).await.expect("a job");
    assert_eq!(r, Err("Couldn't save: The server did not apply the model selection".into()));
    let board = store.domains.models.board(&session);
    assert!(board.selection_reverted);
    assert_eq!(seats::model_seat_label(&store), "Kimi K3", "the selection did not change");
    // Close and reopen: the notice is sticky for this Session …
    host::close();
    run(&conv, host::open(Dialog::ModelMenu)).await;
    assert!(dsl(&conv).dsl.contains("Couldn't save: The server did not apply the model selection"));
    // … and another Session has its own board.
    assert!(store.domains.models.board("dsflash:other").notices.is_empty());
    assert_eq!(server.sent("profile/llm/select").len(), 1);
}

/// The seats exist only when the server offers what they open.
#[tokio::test]
async fn the_seats_follow_the_capabilities() {
    let _s = serial();
    let server = Server::start(base()).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    assert!(seats::permission_seat(&store) && seats::model_seat(&store), "r1 advertises both");
    let methods: Vec<String> = store
        .domains
        .config
        .supported_methods()
        .into_iter()
        .filter(|m| m != "permission/profile/list" && m != "profile/llm/list")
        .collect();
    store.domains.config.set_supported_methods(methods);
    assert!(!seats::permission_seat(&store) && !seats::model_seat(&store), "a missing capability removes its seat");
    tokio::time::sleep(Duration::from_millis(10)).await;
}
