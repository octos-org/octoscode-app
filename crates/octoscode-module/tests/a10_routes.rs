//! A10 — the Profile's configured model providers (web
//! `ModelManagementSection` over `model-settings.ts`) through the
//! production path: `board3::host::open(Dialog::Routes)` (what the Models
//! dialog's "Manage providers" runs), `host::perform` for its controls and
//! `host::run` for its jobs (`board3::routes::{load, fetch, save, delete}`)
//! against a WebSocket server answering with r2-profile's RECORDED config
//! (primary + the r2-route fallback), upsert and delete replies, and
//! `a10-routes-faithful.jsonl` (fetch_models and a passing test — no
//! recording carries them; r29a's recorded test is the 401 refusal).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::Conversation;
use octoscode_module::screens::board3::host::{self, Dialog, Job, Outcome};
use octoscode_module::screens::board3::routes;

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
const FAITHFUL: &str = "a10-routes-faithful.jsonl";

fn recorded_open() -> Value {
    dir("r1-autonomy-a6ea8505.jsonl", "in", "session/open")
        .into_iter()
        .find(|b| b.get("active_profile_id").is_some())
        .expect("r1 open")
}

/// The recorded config WITH the r2-route fallback (r2 line 35).
fn r2_config() -> Value {
    dir(R2, "in", "profile/llm/list")
        .into_iter()
        .find(|b| b["fallbacks"].as_array().is_some_and(|a| !a.is_empty()))
        .expect("r2 config with the fallback")
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
                let frame = json!({"jsonrpc": "2.0", "id": id, "result": body});
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
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
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

async fn run(conv: &Conversation, out: Outcome) -> Option<Result<String, String>> {
    match out {
        Outcome::Spawn(job) => Some(host::run(job, conv).await),
        Outcome::Done => None,
        other => panic!("{other:?}"),
    }
}

fn canned(test_reply: Value) -> Vec<(String, Value)> {
    vec![
        ("session/open".into(), json!({"opened": recorded_open()})),
        ("profile/llm/list".into(), r2_config()),
        ("profile/llm/fetch_models".into(), dir(FAITHFUL, "in", "profile/llm/fetch_models").remove(0)),
        ("profile/llm/test".into(), test_reply),
        ("profile/llm/upsert".into(), dir(R2, "in", "profile/llm/upsert").remove(0)),
        ("profile/llm/delete".into(), dir(R2, "in", "profile/llm/delete").remove(0)),
    ]
}

/// The recorded config lists the primary and the r2-route fallback; on the
/// fallback: "Fetch available models" (the web's routeSelection), pick a
/// suggestion, Save = ONE provision tested then upserted (set_primary:false,
/// as r2 recorded), the config re-read; then Delete behind the exact typed
/// phrase sends r2's recorded delete and the receipt's config replaces the
/// list with the web's success line.
#[tokio::test]
async fn providers_fetch_pick_save_and_delete_with_the_typed_phrase() {
    let _s = serial();
    let server = Server::start(canned(dir(FAITHFUL, "in", "profile/llm/test").remove(0))).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    host::reset();
    run(&conv, host::open(Dialog::Routes)).await.expect("a load").expect("listed");
    assert_eq!(server.sent("profile/llm/list").last().unwrap(), &json!({"profile_id": "dsflash"}));
    let names: Vec<(String, String, bool)> =
        host::state().routes.routes.iter().map(|r| (r.name(), r.route_id.clone(), r.primary)).collect();
    assert_eq!(
        names,
        [
            ("deepseek · deepseek-v4-flash".to_owned(), "deepseek".to_owned(), true),
            ("deepseek · deepseek-v4-flash".to_owned(), "r2-route".to_owned(), false),
        ]
    );
    // Fetch available models on the fallback, pick, save.
    assert_eq!(host::perform("b3.routes.add", 1, &store), Outcome::Done);
    run(&conv, host::perform("b3.routes.fetch", 0, &store)).await.expect("a job").expect("fetched");
    let selection = json!({"family_id": "deepseek",
        "route": {"route_id": "r2-route", "api_key_env": "<redacted>", "api_type": "openai", "base_url": "http://127.0.0.1:9/v1"}});
    assert_eq!(server.sent("profile/llm/fetch_models")[0], json!({"profile_id": "dsflash", "selection": selection}));
    assert_eq!(host::state().routes.fetched, ["deepseek-v4-flash", "deepseek-v4-pro", "deepseek-reasoner"]);
    host::perform("b3.routes.pick", 1, &store);
    assert_eq!(host::state().routes.model, "deepseek-v4-pro", "the suggestion fills the Model ID");
    let r = run(&conv, host::perform("b3.routes.save", 0, &store)).await.expect("a job");
    assert_eq!(r.as_deref(), Ok(routes::SAVED));
    let test = server.sent("profile/llm/test")[0].clone();
    let upsert = server.sent("profile/llm/upsert")[0].clone();
    assert_eq!(test["selection"]["model_id"], "deepseek-v4-pro");
    assert_eq!(upsert["selection"], test["selection"], "one provision: Test and Save cannot drift");
    assert_eq!(upsert["set_primary"], false);
    let recorded = dir(R2, "out", "profile/llm/upsert").remove(0);
    assert_eq!(upsert["set_primary"], recorded["set_primary"]);
    assert_eq!(upsert["selection"]["route"]["route_id"], recorded["selection"]["route"]["route_id"]);
    assert_eq!(server.sent("profile/llm/list").len(), 2, "the config is re-read after the save");
    // Delete the fallback: the exact, case-sensitive phrase arms it.
    assert_eq!(host::perform("b3.routes.delete", 1, &store), Outcome::Done);
    {
        let m = host::lower_open(&store).expect("open");
        for want in ["Delete model provider?", "Type DELETE deepseek/deepseek-v4-flash to confirm", "Existing sessions may still refer to it."] {
            assert!(m.dsl.contains(want), "{want}");
        }
    }
    host::input_changed("routes.phrase", "DELETE deepseek/deepseek-v4-FLASH");
    assert_eq!(host::perform("b3.routes.confirm_delete", 0, &store), Outcome::Done, "case-sensitive");
    host::input_changed("routes.phrase", "DELETE deepseek/deepseek-v4-flash");
    let r = run(&conv, host::perform("b3.routes.confirm_delete", 0, &store)).await.expect("a job");
    assert_eq!(r.as_deref(), Ok(routes::DELETED));
    assert_eq!(server.sent("profile/llm/delete"), dir(R2, "out", "profile/llm/delete"), "the recorded delete params");
    let left: Vec<String> = host::state().routes.routes.iter().map(|r| r.route_id.clone()).collect();
    assert_eq!(left, ["deepseek"], "the receipt's configuration replaces the list");
    assert!(host::lower_open(&store).unwrap().dsl.contains(routes::DELETED));
    host::close();
}

/// A refused test (r29a's recorded 401) saves nothing.
#[tokio::test]
async fn a_failed_test_never_upserts() {
    let _s = serial();
    let refused = dir("r29a-onboarding-a6ea8505.jsonl", "in", "profile/llm/test").remove(0);
    let mut refused = refused;
    refused["profile_id"] = json!("dsflash");
    let server = Server::start(canned(refused)).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    host::reset();
    run(&conv, host::open(Dialog::Routes)).await;
    host::perform("b3.routes.add", 0, &store);
    host::input_changed("routes.model", "deepseek-v4-pro");
    let r = run(&conv, host::perform("b3.routes.save", 0, &store)).await.expect("a job");
    assert!(r.unwrap_err().contains("authentication failed"), "the provider's refusal is shown");
    assert!(server.sent("profile/llm/upsert").is_empty(), "no upsert after a failed test");
    assert!(host::state().routes.error.is_some());
    host::close();
    let _ = Job::RoutesLoad(0);
}
