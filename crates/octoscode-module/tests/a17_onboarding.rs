//! A17 — the solo onboarding panel's production paths, at the wire (parity
//! rows 93-98).
//!
//! A fake Octos server (the a8/f29a pattern: one listener, the AppUI
//! WebSocket) answers in the RECORDED a6ea8505 shapes of
//! `crates/octoscode-client/tests/fixtures/r29a-onboarding-a6ea8505.jsonl`:
//! the advertised capabilities are its line 2, the catalog its line 6 (the
//! recorder's `<redacted>` endpoint env NAMES restored), `profile/local/
//! create` / `profile/llm/test` / `profile/llm/upsert` its lines 8/10/12, with
//! the web e2e fixture's rules (`src-web/apps/web/scripts/mock-ui-server.mjs:
//! 3472-3707`): a `/no-profile` folder resolves `no_profile` until a profile
//! exists, `coding` is created as `coding-2` (Core owns the id), the key
//! `sk-test-rejected` is refused with the recorded 401 — echoing the key, the
//! misbehaviour the panel's redaction exists for. Every request is recorded
//! (method + params) and any reply can be HELD until a test releases it.
//!
//! Each test drives the functions the native UI calls: `launch::create` (the
//! folder launch every "New session" / "Use this folder" control runs),
//! `board3::host::perform` (each mounted control's tap), `host::input_changed`
//! (the typed text) and `host::run` (the job the host spawns) — then asserts
//! what reached the wire, what the panel shows and what it holds.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::{Conversation, FlowEvent};
use octoscode_module::screens::board3::host::{self, Dialog, Job, Outcome};
use octoscode_module::screens::launch;
use octoscode_module::screens::onboarding::{self as onb, Phase, Submitted};
use octoscode_module::screens::recents::MemoryStore;

const FOLDER: &str = "/srv/work/no-profile";
/// Obvious dummies — never a real key.
const GOOD_KEY: &str = "sk-test-dummy";
const REJECTED_KEY: &str = "sk-test-rejected";
const PROBE: &str = "octoscode-web-keyless-probe";

fn fixture() -> Vec<Value> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../octoscode-client/tests/fixtures/r29a-onboarding-a6ea8505.jsonl");
    std::fs::read_to_string(path)
        .expect("read the r29a fixture")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("fixture line is JSON"))
        .collect()
}

fn recorded(method: &str) -> Value {
    fixture()
        .into_iter()
        .find(|f| f["dir"] == "in" && f["method"] == method)
        .unwrap_or_else(|| panic!("r29a records {method}"))["body"]
        .clone()
}

/// r29a line 6, the endpoints' `<redacted>` env names restored.
fn recorded_catalog() -> Value {
    let mut body = recorded("profile/llm/catalog");
    for family in body["families"].as_object_mut().unwrap().values_mut() {
        for model in family.get_mut("models").and_then(|m| m.as_array_mut()).into_iter().flatten() {
            for e in model.get_mut("endpoints").and_then(|e| e.as_array_mut()).into_iter().flatten() {
                if e["api_key_env"] == json!("<redacted>") {
                    let name = e["id"].as_str().unwrap().to_uppercase().replace('-', "_");
                    e["api_key_env"] = json!(format!("{name}_API_KEY"));
                }
            }
        }
    }
    body
}

/// r29a line 2's advertised methods (every onboarding method among them).
fn recorded_methods() -> Vec<String> {
    recorded("config/capabilities/list")["capabilities"]["supported_methods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m.as_str().unwrap().to_owned())
        .collect()
}

struct World {
    methods: Vec<String>,
    created: Option<String>,
    seen: Vec<(String, Value)>,
    holds: HashMap<String, Arc<Notify>>,
}

struct FakeServer {
    base_url: String,
    world: Arc<Mutex<World>>,
}

impl FakeServer {
    async fn start(drop_methods: &[&str]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let methods = recorded_methods().into_iter().filter(|m| !drop_methods.contains(&m.as_str())).collect();
        let world = Arc::new(Mutex::new(World { methods, created: None, seen: Vec::new(), holds: HashMap::new() }));
        let w2 = world.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                tokio::spawn(serve(stream, w2.clone()));
            }
        });
        Self { base_url, world }
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.world.lock().unwrap().seen.iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    /// Hold the NEXT `method` reply until the returned notify fires.
    fn hold(&self, method: &str) -> Arc<Notify> {
        let n = Arc::new(Notify::new());
        self.world.lock().unwrap().holds.insert(method.to_owned(), n.clone());
        n
    }
}

fn reply(w: &mut World, method: &str, p: &Value) -> Value {
    let session = p["session_id"].as_str().unwrap_or("dsflash:main").to_owned();
    match method {
        "session/open" => json!({"opened": {
            "session_id": session,
            "active_profile_id": p["profile_id"].as_str().unwrap_or("dsflash"),
            "workspace_root": p["cwd"].as_str().unwrap_or("/srv/work/octos"),
            "cursor": {"stream": session, "seq": 1},
            "capabilities": {
                "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                "capabilities_schema_version": 2,
                "supported_methods": w.methods,
                "supported_notifications": ["turn/started"],
                "supported_features": ["session.workspace_cwd.v1", "state.session_hydrate.v1"]
            }
        }}),
        "session/list" => json!({"sessions": []}),
        "launch/resolve" => match (&w.created, p["cwd"].as_str().unwrap_or("").ends_with("/no-profile")) {
            (Some(id), true) => json!({"decision": "activate", "resolved_profile": id}),
            (None, true) => json!({"decision": "no_profile"}),
            _ => json!({"decision": "resume", "resolved_profile": p["profile_id"]}),
        },
        "profile/llm/catalog" => recorded_catalog(),
        // r29a line 8 (Core owns the id: `coding` -> `coding-2`).
        "profile/local/create" => {
            let requested = p["requested_id"].as_str().unwrap_or("").to_owned();
            let id = if requested == "coding" { "coding-2".to_owned() } else { requested };
            w.created = Some(id.clone());
            let mut r = recorded("profile/local/create");
            for k in ["profile_id", "user_id", "username"] {
                r[k] = json!(id);
            }
            r["name"] = p["name"].clone();
            r["email"] = json!(format!("{id}@solo.local"));
            r
        }
        // r29a line 10: the recorded 401 body, the key echoed raw.
        "profile/llm/test" => {
            let key = p["api_key"].as_str().unwrap_or("");
            let keyless = p["selection"]["route"]["api_key_env"].as_str().unwrap_or("").is_empty();
            let mut r = recorded("profile/llm/test");
            r["profile_id"] = p["profile_id"].clone();
            if key == REJECTED_KEY {
                r["error"] = json!(format!(
                    "API error (deepseek@api/deepseek-v4-flash, api_style=openai_chat_completions): authentication failed — \
                     HTTP 401 - {{\"error\":{{\"message\":\"Authentication Fails, Your api key: {key} is invalid\"}}}}"
                ));
            } else if keyless && key != PROBE {
                r["error"] = json!("Keyless compatibility probe missing");
            } else {
                r = json!({"applied": true, "message": "Provider test succeeded", "profile_id": p["profile_id"]});
            }
            r
        }
        // r29a line 12.
        "profile/llm/upsert" => {
            let mut r = recorded("profile/llm/upsert");
            r["profile_id"] = p["profile_id"].clone();
            r
        }
        _ => json!({}),
    }
}

async fn serve(stream: TcpStream, world: Arc<Mutex<World>>) {
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
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
        if v.get("id").is_none() {
            continue;
        }
        let method = v["method"].as_str().unwrap_or("").to_owned();
        let params = v["params"].clone();
        // One task per request: a HELD reply never blocks the others.
        let (world, out) = (world.clone(), out.clone());
        tokio::spawn(async move {
            let hold = {
                let mut w = world.lock().unwrap();
                w.seen.push((method.clone(), params.clone()));
                w.holds.remove(&method)
            };
            if let Some(n) = hold {
                n.notified().await;
            }
            let result = reply(&mut world.lock().unwrap(), &method, &params);
            let _ = out.send(json!({"jsonrpc": "2.0", "id": v["id"], "result": result}).to_string());
        });
    }
}

async fn connected(server: &FakeServer) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    let conv = Arc::new(conv);
    conv.open_workspace(Some("/srv/work/octos".into())).await.expect("session/open");
    let mut opened = false;
    for _ in 0..80 {
        if opened && conv.store.is_live() {
            break;
        }
        match tokio::time::timeout(Duration::from_millis(200), events.recv()).await {
            Ok(Some(evt)) => {
                if matches!(conv.on_event(evt), FlowEvent::WorkspaceOpened(_)) {
                    opened = true;
                }
            }
            _ => break,
        }
    }
    // Keep folding replies (the later session/open of the onboarded Session).
    let drain = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            let _ = drain.on_event(evt);
        }
    });
    conv
}

/// The board-3 / launch / onboarding state and the storage seams are
/// process-global: one test at a time, each from a clean slate, no file
/// outside a per-run temp dir.
fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    host::reset();
    launch::reset();
    octoscode_module::screens::drafts::reset();
    octoscode_module::screens::drafts::set_storage(Arc::new(MemoryStore::new()));
    octoscode_module::screens::session_defaults::set_storage(Arc::new(MemoryStore::new()));
    let dir = std::env::temp_dir().join(format!("a17-onboarding-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    std::env::set_var("OCTOSCODE_RECENTS_DIR", &dir);
    std::env::set_var("OCTOSCODE_SHOW_THINKING_FILE", dir.join("show-thinking.json"));
    g
}

async fn until(what: &str, mut pred: impl FnMut() -> bool) {
    for _ in 0..100 {
        if pred() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    panic!("timed out waiting for {what}");
}

/// The folder launch the "Use this folder" / "New session" controls run, up
/// to the panel's loaded form.
async fn launch_to_form(server: &FakeServer, conv: &Conversation) {
    assert_eq!(launch::create(conv, FOLDER.into()).await, launch::Launched::AwaitingChoice);
    assert_eq!(host::open_dialog(), Some(Dialog::Launch));
    until("the catalog", || onb::snapshot().phase == Phase::Ready).await;
    assert_eq!(server.params_of("profile/llm/catalog").len(), 1, "prepare reads the catalog once");
}

/// Pick the option labelled `label` from the select `which`, by its taps.
fn pick(which: &str, label: &str) {
    assert_eq!(host::perform(&format!("b3.onb.select.{which}"), 0, &octoscode_store::Store::new()), Outcome::Done);
    let dsl = host::lower_open(&octoscode_store::Store::new()).expect("the dialog lowers").dsl;
    let index = dsl
        .split("b3_onb_opt_")
        .skip(1)
        .find_map(|chunk| {
            let (i, rest) = chunk.split_once('_')?;
            rest.starts_with("label := Label").then_some(())?;
            rest.contains(&format!("text: {label:?}")).then(|| i.parse::<usize>().ok())?
        })
        .unwrap_or_else(|| panic!("the {which} list offers {label}"));
    assert_eq!(host::perform("b3.onb.pick", index, &octoscode_store::Store::new()), Outcome::Done);
}

fn type_key(key: &str) {
    host::input_changed("onb.api_key", key);
}

/// The submit tap: it must spawn the submission job (no form data in it).
fn submit_job() -> Job {
    match host::perform("b3.onb.submit", 0, &octoscode_store::Store::new()) {
        Outcome::Spawn(job) => {
            assert_eq!(job, Job::OnboardingSubmit);
            job
        }
        other => panic!("the submit tap spawns the submission, got {other:?}"),
    }
}

// --------------------------------------------------------------- row 93 + 94

/// use-onboarding.test.ts:34 "derives the official route from the server
/// family" + e2e product.spec.ts:1774 "onboards an empty solo server from
/// workspace creation": the folder launch shows the panel, the official
/// route is derived from the recorded family, the profile is created once
/// (Core's id kept), tested, saved as primary, and the coding Session opens
/// in the folder under the created profile.
#[tokio::test]
async fn a_no_profile_launch_onboards_with_the_official_route_derived_from_the_family() {
    let _g = lock();
    let server = FakeServer::start(&[]).await;
    let conv = connected(&server).await;
    launch_to_form(&server, &conv).await;
    let dsl = host::lower_open(&conv.store).unwrap().dsl;
    for t in [onb::TITLE, "Profile ID", "Provider", "Model", "Route", "API key", onb::SUBMIT, onb::DISCONNECT] {
        assert!(dsl.contains(t), "the panel shows {t}");
    }
    assert!(!dsl.contains("Create the local profile"), "the panel replaces A8's bare create");
    pick("provider", "deepseek");
    let st = onb::snapshot();
    assert_eq!((st.form.model_id.as_str(), st.form.route_id.as_str()), ("deepseek-v4-flash", onb::OFFICIAL_ROUTE));
    type_key(GOOD_KEY);
    let job = submit_job();
    assert_eq!(host::run(job, &conv).await.map_err(|e| e.to_string()).unwrap(), "onboarded: coding-2 tested, saved and opened");

    let create = &server.params_of("profile/local/create")[0];
    assert_eq!(create, &json!({"requested_id": "coding", "name": "Coding", "username": "", "email": "", "make_default": true}));
    let test = &server.params_of("profile/llm/test")[0];
    assert_eq!(test["profile_id"], json!("coding-2"), "Core's id, not the requested one");
    assert_eq!(
        test["selection"],
        json!({"family_id": "deepseek", "model_id": "deepseek-v4-flash", "route": {
            "route_id": "deepseek", "label": "Official API", "api_key_env": "DEEPSEEK_API_KEY", "api_type": "openai"}})
    );
    assert_eq!(test["api_key"], json!(GOOD_KEY));
    assert!(test.get("set_primary").is_none(), "the test never asks for primary");
    let save = &server.params_of("profile/llm/upsert")[0];
    assert_eq!((&save["selection"], &save["set_primary"]), (&test["selection"], &json!(true)), "saved exactly as tested");
    let open = server.params_of("session/open").last().unwrap().clone();
    assert_eq!(open["cwd"], json!(FOLDER));
    assert_eq!(open["profile_id"], json!("coding-2"));
    assert!(open["session_id"].as_str().unwrap().starts_with("coding-2:api:"), "{open}");
    assert_eq!(host::open_dialog(), None, "the panel closes once the Session opened");
    assert_eq!(onb::snapshot().form.api_key, "", "the key is gone with the panel");
}

/// use-onboarding.test.ts:53 "preserves a catalog endpoint and rejects stale
/// selections": a catalog endpoint reaches the wire as advertised (id,
/// label, base URL, its own env); a family/model/route the recorded catalog
/// does not carry is refused before anything is created.
#[tokio::test]
async fn a_catalog_endpoint_is_preserved_on_the_wire_and_a_stale_selection_is_rejected() {
    let _g = lock();
    let server = FakeServer::start(&[]).await;
    let conv = connected(&server).await;
    launch_to_form(&server, &conv).await;
    pick("provider", "deepseek");
    pick("route", "AutoDL");
    type_key(GOOD_KEY);
    host::run(submit_job(), &conv).await.unwrap();
    assert_eq!(
        server.params_of("profile/llm/test")[0]["selection"]["route"],
        json!({"route_id": "autodl", "label": "AutoDL", "base_url": "https://www.autodl.art/api/v1",
               "api_key_env": "AUTODL_API_KEY", "api_type": "openai"})
    );

    let catalog: octoscode_client::domains::profile::LlmCatalogResult = serde_json::from_value(recorded_catalog()).unwrap();
    for (family, model, route) in [("deepseek", "removed", onb::OFFICIAL_ROUTE), ("gone", "x", onb::OFFICIAL_ROUTE)] {
        assert_eq!(
            onb::selection_from_catalog(&catalog, family, model, route).unwrap_err(),
            "The selected provider or model is no longer advertised."
        );
    }
    assert_eq!(
        onb::selection_from_catalog(&catalog, "deepseek", "deepseek-v4-flash", "gone").unwrap_err(),
        "The selected provider route is no longer advertised."
    );
}

/// e2e product.spec.ts:1823 "preserves Octoscode keyless-provider onboarding
/// semantics": a keyless family asks for no key and the test carries the
/// probe value (row 96's negotiation, now through the panel).
#[tokio::test]
async fn a_keyless_family_needs_no_key_and_sends_the_probe() {
    let _g = lock();
    let server = FakeServer::start(&[]).await;
    let conv = connected(&server).await;
    launch_to_form(&server, &conv).await;
    pick("provider", "ollama");
    let dsl = host::lower_open(&conv.store).unwrap().dsl;
    assert!(dsl.contains(onb::KEYLESS_HEAD) && dsl.contains("ollama is marked keyless by the Core catalog."));
    assert!(!dsl.contains("b3_onb_apikey :="), "no key field");
    host::run(submit_job(), &conv).await.unwrap();
    let test = &server.params_of("profile/llm/test")[0];
    assert_eq!((&test["api_key"], &test["selection"]["route"]["api_key_env"]), (&json!(PROBE), &json!("")));
}

// --------------------------------------------------------------------- row 95

/// use-onboarding.test.ts:148 "retains the created profile when retrying a
/// failed provider test" + e2e :1774's retry: the failed test keeps the
/// created profile (shown, its identity locked), and the retry repeats only
/// test + save (+ the open) — `profile/local/create` stays at ONE.
#[tokio::test]
async fn the_created_profile_is_retained_across_a_failed_test_and_the_retry() {
    let _g = lock();
    let server = FakeServer::start(&[]).await;
    let conv = connected(&server).await;
    launch_to_form(&server, &conv).await;
    pick("provider", "deepseek");
    type_key(REJECTED_KEY);
    let err = host::run(submit_job(), &conv).await.unwrap_err();
    assert!(err.contains("authentication failed"), "{err}");
    let st = onb::snapshot();
    assert_eq!(st.phase, Phase::Ready);
    assert_eq!(st.created_profile_id.as_deref(), Some("coding-2"));
    assert_eq!(st.binding.as_ref().map(|b| (b.requested_id.as_str(), b.profile_id.as_str())), Some(("coding", "coding-2")));
    assert_eq!(st.error_lead.as_deref(), Some("Provider connection failed"), "the server's own message leads");
    assert!(server.params_of("profile/llm/upsert").is_empty(), "no save after a failed test");
    let dsl = host::lower_open(&conv.store).unwrap().dsl;
    assert!(dsl.contains("Profile coding-2 exists. A retry only repeats provider test and save."));
    assert!(!dsl.contains("b3_onb_profile_id := TextInput"), "the identity is locked once created");
    assert_eq!(host::perform("b3.onb.default", 0, &conv.store), Outcome::Done);
    assert!(onb::snapshot().form.make_default, "the default checkbox is locked too");

    type_key(GOOD_KEY);
    host::run(submit_job(), &conv).await.unwrap();
    assert_eq!(server.params_of("profile/local/create").len(), 1, "the retry never re-creates");
    assert_eq!(server.params_of("profile/llm/test").len(), 2);
    assert_eq!(server.params_of("profile/llm/upsert").len(), 1);
    assert!(server.params_of("profile/llm/test").iter().all(|t| t["profile_id"] == json!("coding-2")));
    assert_eq!(server.params_of("session/open").last().unwrap()["profile_id"], json!("coding-2"));
    assert_eq!(host::open_dialog(), None);
}

// --------------------------------------------------------------------- row 97

/// OnboardingPanel.test.tsx:6 "retains the canonical TUI fallback when Core
/// lacks Web onboarding": one onboarding method missing -> the `octoscode
/// onboard` fallback, no catalog read, and nothing can start.
#[tokio::test]
async fn a_server_missing_an_onboarding_method_gets_the_octoscode_onboard_fallback_and_nothing_starts() {
    let _g = lock();
    let server = FakeServer::start(&["profile/llm/fetch_models"]).await;
    let conv = connected(&server).await;
    assert_eq!(launch::create(&conv, FOLDER.into()).await, launch::Launched::AwaitingChoice);
    tokio::time::sleep(Duration::from_millis(200)).await;
    let st = onb::snapshot();
    assert!(!st.supported && st.phase == Phase::Idle);
    let dsl = host::lower_open(&conv.store).unwrap().dsl;
    for t in [onb::FALLBACK_HEAD, onb::FALLBACK_BODY, "octoscode onboard", onb::DISCONNECT] {
        assert!(dsl.contains(t), "the fallback shows {t}");
    }
    assert!(!dsl.contains(onb::SUBMIT) && !dsl.contains("b3.onb.submit"), "no way to start");
    assert!(server.params_of("profile/llm/catalog").is_empty(), "no catalog read");
    assert_eq!(host::perform("b3.onb.submit", 0, &conv.store), Outcome::Done, "a stray submit spawns nothing");
    let refused = onb::submit(conv.client(), conv.scope().authority_epoch, |_| async { Ok(()) }).await;
    assert_eq!(refused, Submitted::Refused);
    assert!(server.params_of("profile/local/create").is_empty(), "nothing is created");
    // Disconnect cancels the launch (`onCancel` = `cancelLaunch`).
    assert_eq!(host::perform("b3.onb.disconnect", 0, &conv.store), Outcome::Done);
    assert_eq!(host::open_dialog(), None);
    assert_eq!(launch::snapshot().phase, launch::Phase::Idle);
}

// --------------------------------------------------------------------- row 98

/// The key is never echoed back: the server's 401 quotes it raw, yet the
/// panel's error, the job's result, the logged job, the mounted DSL and the
/// panel state after Disconnect carry no trace of it.
#[tokio::test]
async fn the_api_key_never_reaches_an_error_a_job_the_dsl_or_the_state() {
    let _g = lock();
    let server = FakeServer::start(&[]).await;
    let conv = connected(&server).await;
    launch_to_form(&server, &conv).await;
    pick("provider", "deepseek");
    type_key(REJECTED_KEY);
    let job = submit_job();
    assert!(!format!("{job:?}").contains(REJECTED_KEY), "the logged job carries no key");
    let err = host::run(job, &conv).await.unwrap_err();
    assert!(
        server.params_of("profile/llm/test")[0]["api_key"] == json!(REJECTED_KEY),
        "the key does reach the server (test + save need it)"
    );
    assert!(!err.contains(REJECTED_KEY) && err.contains("[redacted]"), "{err}");
    let st = onb::snapshot();
    let shown = format!("{:?} {:?}", st.error, st.error_lead);
    assert!(!shown.contains(REJECTED_KEY) && shown.contains("[redacted]"), "{shown}");
    assert!(!format!("{st:?}").contains(REJECTED_KEY), "the state's Debug never prints the key");
    let dsl = host::lower_open(&conv.store).unwrap().dsl;
    assert!(!dsl.contains(REJECTED_KEY), "the masked input never embeds the key");
    assert!(dsl.contains("is_password: true"));
    // Only the host's post-mount restore holds it (the masked input's text).
    assert_eq!(host::post_mount_texts(), vec![("b3_onb_apikey".to_owned(), REJECTED_KEY.to_owned())]);
    // Leaving drops it.
    assert_eq!(host::perform("b3.onb.disconnect", 0, &conv.store), Outcome::Done);
    assert_eq!(onb::snapshot().form.api_key, "");
    assert!(host::post_mount_texts().is_empty());
}

/// A superseded PREPARE cannot publish: the first catalog reply comes back
/// after a newer launch took the panel — it is dropped (the panel keeps
/// loading), and only the newer reply fills the form.
#[tokio::test]
async fn a_superseded_catalog_reply_is_dropped() {
    let _g = lock();
    let server = FakeServer::start(&[]).await;
    let conv = connected(&server).await;
    let first = server.hold("profile/llm/catalog");
    assert_eq!(launch::create(&conv, FOLDER.into()).await, launch::Launched::AwaitingChoice);
    until("the first catalog request", || server.params_of("profile/llm/catalog").len() == 1).await;
    let second = server.hold("profile/llm/catalog");
    // The person leaves and launches again (a new lease, a new generation).
    assert_eq!(host::perform("b3.onb.close", 0, &conv.store), Outcome::Done);
    assert_eq!(launch::create(&conv, FOLDER.into()).await, launch::Launched::AwaitingChoice);
    until("the second catalog request", || server.params_of("profile/llm/catalog").len() == 2).await;
    first.notify_one();
    tokio::time::sleep(Duration::from_millis(250)).await;
    let st = onb::snapshot();
    assert_eq!(st.phase, Phase::LoadingCatalog, "the stale reply published nothing");
    assert!(st.catalog.is_none());
    second.notify_one();
    until("the current catalog", || onb::snapshot().phase == Phase::Ready).await;
    assert!(onb::snapshot().catalog.is_some());
}

/// use-onboarding.test.ts:171 "does not continue after authority changes
/// during create / test / save": the panel is left while each step is in
/// flight; its reply arrives later and NOTHING follows — no next request, no
/// error, no opened Session.
#[tokio::test]
async fn a_superseded_submission_does_not_continue_at_create_test_or_save() {
    for (step, next) in [
        ("profile/local/create", "profile/llm/test"),
        ("profile/llm/test", "profile/llm/upsert"),
        ("profile/llm/upsert", "session/open"),
    ] {
        let _g = lock();
        let server = FakeServer::start(&[]).await;
        let conv = connected(&server).await;
        launch_to_form(&server, &conv).await;
        pick("provider", "deepseek");
        type_key(GOOD_KEY);
        let job = submit_job();
        let held = server.hold(step);
        let opens = server.params_of("session/open").len();
        let conv2 = conv.clone();
        let run = tokio::spawn(async move { host::run(job, &conv2).await });
        until(step, || !server.params_of(step).is_empty()).await;
        // The person closes the panel mid-flight (the close glyph / Escape).
        assert_eq!(host::perform("b3.onb.close", 0, &conv.store), Outcome::Done, "{step}");
        held.notify_one();
        let out = run.await.unwrap();
        assert_eq!(out, Ok("superseded: nothing published".into()), "{step}");
        tokio::time::sleep(Duration::from_millis(150)).await;
        let n_next = server.params_of(next).len();
        if next == "session/open" {
            assert_eq!(n_next, opens, "{step}: no Session opened");
        } else {
            assert_eq!(n_next, 0, "{step}: {next} never sent");
        }
        let st = onb::snapshot();
        assert!(st.error.is_none() && st.binding.is_none(), "{step}: nothing published");
        assert_eq!(host::open_dialog(), None, "{step}");
    }
}
