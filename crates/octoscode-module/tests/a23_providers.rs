//! A23 — parity rows 281 (model provider management) and 282 (the model
//! management projection) through the PRODUCTION path: the providers dialog
//! as the Models dialog's "Manage providers" opens it
//! (`board3::host::open(Dialog::Routes)`, `host::perform` for its controls,
//! `host::run` for its jobs, `host::lower_open` for what it draws) and the
//! board-1 provider editor it hands Edit / Add provider to (`board1::route`
//! for a click, `board1::execute` for the work it returns — the future
//! `board1::spawn` runs), against a WebSocket server answering with the
//! RECORDED traffic: r1's session open (its advertised methods), r2-profile's
//! configuration and provider catalog, r29a's recorded 401 for a rejected
//! key, and `a10-routes-faithful.jsonl`'s passing test and fetch replies.
//! The two extra configured rows are the WEB unit tests' own values
//! (`model-management-projection.test.ts:11` `strong: false`, `:53` the
//! inference overrides). Only dummy keys (`sk-test-dummy`, `sk-test-rejected`).

use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::Conversation;
use octoscode_module::screens::board3::host::{self, Dialog, Outcome};
use octoscode_module::screens::{board1, provider};

const DUMMY: &str = "sk-test-dummy";
const REJECTED: &str = "sk-test-rejected";

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

/// The recorder's trace redacts every field whose NAME contains `api_key`,
/// so r2's rows read `"has_api_key": "<redacted>"` (a boolean on the wire)
/// and `"api_key_env": "<redacted>"` (an env name): restored.
fn restore(mut v: Value) -> Value {
    fn walk(v: &mut Value) {
        match v {
            Value::Object(m) => {
                for (k, x) in m.iter_mut() {
                    if k == "has_api_key" && x.is_string() {
                        *x = json!(true);
                    } else if k == "api_key_env" && x == "<redacted>" {
                        *x = json!("DEEPSEEK_API_KEY");
                    } else {
                        walk(x);
                    }
                }
            }
            Value::Array(a) => a.iter_mut().for_each(walk),
            _ => {}
        }
    }
    walk(&mut v);
    v
}

/// r2's recorded configuration with the r2-route fallback.
fn r2_config() -> Value {
    restore(
        dir(R2, "in", "profile/llm/list")
            .into_iter()
            .find(|b| b["fallbacks"].as_array().is_some_and(|a| !a.is_empty()))
            .expect("r2 config with the fallback"),
    )
}

/// The web unit tests' two rows: `strong: false` (edit-blocked) and a row
/// with configured inference overrides (`top_p: null` included).
fn extra_rows() -> Vec<Value> {
    vec![
        json!({"family_id": "moonshot", "model_id": "kimi-k2", "model": "kimi-k2", "provider": "moonshot",
            "route": {"route_id": "moonshot", "label": "Official API", "api_type": "openai", "api_key_env": "MOONSHOT_API_KEY"},
            "route_id": "moonshot", "has_api_key": true, "selected": false, "available": true, "strong": false}),
        json!({"family_id": "zai", "model_id": "glm-5.3-flash", "model": "glm-5.3-flash", "provider": "zai",
            "route": {"route_id": "zai", "label": "Official API", "api_type": "openai", "api_key_env": "ZAI_API_KEY"},
            "route_id": "zai", "has_api_key": true, "selected": false, "available": true,
            "temperature": 0, "top_p": null, "context_window": 131072, "reasoning_effort": "max",
            "model_hints": {"fixed_temperature": false, "reasoning_style": "effort_low_high_max"}}),
    ]
}

fn inference() -> Value {
    json!({"temperature": 0, "top_p": null, "context_window": 131072, "reasoning_effort": "max",
           "model_hints": {"fixed_temperature": false, "reasoning_style": "effort_low_high_max"}})
}

/// The provider half of an `octos serve`, faithful to the recordings: the
/// configuration changes with each upsert (an existing identity replaced, a
/// new one appended) and delete; a refused key gets r29a's recorded 401.
struct Sim {
    config: Value,
    catalog: Value,
    tested: Value,
    fetched: Value,
    rejected: Value,
    /// Refuse this many profile-config reads first (the unread state).
    fail_list: usize,
    /// The provider test answers this failure for any key (a non-key fault).
    test_fault: Option<String>,
}

impl Sim {
    fn new(config: Value) -> Sim {
        let faithful = |m: &str| dir(FAITHFUL, "in", m).remove(0);
        Sim {
            config,
            catalog: dir(R2, "in", "profile/llm/catalog").remove(0),
            tested: faithful("profile/llm/test"),
            fetched: faithful("profile/llm/fetch_models"),
            rejected: dir("r29a-onboarding-a6ea8505.jsonl", "in", "profile/llm/test").remove(0),
            fail_list: 0,
            test_fault: None,
        }
    }

    fn answer(&mut self, method: &str, p: &Value) -> Result<Value, Value> {
        match method {
            "profile/llm/list" if self.fail_list > 0 => {
                self.fail_list -= 1;
                Err(json!({"code": -32000, "message": "profile store unavailable"}))
            }
            "profile/llm/list" => Ok(self.config.clone()),
            "profile/llm/catalog" => Ok(self.catalog.clone()),
            "profile/llm/test" if p["api_key"] == REJECTED => {
                let mut r = self.rejected.clone();
                r["profile_id"] = p["profile_id"].clone();
                Ok(r)
            }
            "profile/llm/test" => match &self.test_fault {
                Some(e) => Ok(json!({"profile_id": p["profile_id"], "applied": false, "message": "Provider connection failed", "error": e})),
                None => Ok(self.tested.clone()),
            },
            "profile/llm/fetch_models" => {
                let mut r = self.fetched.clone();
                r["family_id"] = p["selection"]["family_id"].clone();
                Ok(r)
            }
            "profile/llm/upsert" => {
                let sel = &p["selection"];
                let same = |m: &Value| {
                    m["family_id"] == sel["family_id"] && m["model_id"] == sel["model_id"] && m["route"]["route_id"] == sel["route"]["route_id"]
                };
                let mut row = json!({"family_id": sel["family_id"], "model_id": sel["model_id"], "route": sel["route"],
                    "has_api_key": true, "available": true, "selected": false});
                for k in ["temperature", "top_p", "context_window", "reasoning_effort", "model_hints"] {
                    if let Some(v) = sel.get(k) {
                        row[k] = v.clone();
                    }
                }
                if same(&self.config["primary"]) {
                    row["selected"] = json!(true);
                    self.config["primary"] = row;
                } else {
                    let f = self.config["fallbacks"].as_array_mut().unwrap();
                    match f.iter().position(same) {
                        Some(i) => f[i] = row,
                        None => f.push(row),
                    }
                }
                let mut r = self.config.clone();
                r["applied"] = json!(true);
                Ok(r)
            }
            "profile/llm/delete" => {
                let hit = |m: &Value| m["family_id"] == p["family_id"] && m["model_id"] == p["model_id"] && m["route"]["route_id"] == p["route_id"];
                self.config["fallbacks"].as_array_mut().unwrap().retain(|m| !hit(m));
                if hit(&self.config["primary"]) {
                    self.config["primary"] = Value::Null;
                }
                let mut r = self.config.clone();
                r["applied"] = json!(true);
                Ok(r)
            }
            _ => Ok(json!({})),
        }
    }
}

struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Server {
    /// `drop`: methods withdrawn from the recorded open's advertised set.
    async fn start(sim: Sim, drop: &[&str]) -> Self {
        let mut open = frames("r1-autonomy-a6ea8505.jsonl")
            .into_iter()
            .find(|f| f["dir"] == "in" && f["method"] == "session/open" && f["body"].get("active_profile_id").is_some())
            .map(|f| f["body"].clone())
            .expect("r1 open");
        if let Some(m) = open["capabilities"]["supported_methods"].as_array_mut() {
            m.retain(|x| !drop.iter().any(|d| x == d));
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let sim = Arc::new(Mutex::new(sim));
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (mut tx, mut rx) = ws.split();
            while let Some(Ok(msg)) = rx.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].clone();
                log.lock().unwrap().push((method.clone(), v["params"].clone()));
                let r = if method == "session/open" {
                    Ok(json!({"opened": open.clone()}))
                } else {
                    sim.lock().unwrap().answer(&method, &v["params"])
                };
                let frame = match r {
                    Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
                    Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
                };
                let _ = tx.send(Message::Text(frame.to_string().into())).await;
            }
        });
        Server { base_url: format!("http://{addr}"), seen }
    }

    fn sent(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    fn all(&self) -> String {
        format!("{:?}", self.seen.lock().unwrap())
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
    for _ in 0..100 {
        if !conv.store.domains.config.supported_methods().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    conv.store.domains.session.set_active(Some(conv.session_id()));
    conv.store.domains.profile.set_current("dsflash".into());
    // What lib.rs `sync_board1` feeds board 1 on every sync.
    board1::note_context(&board1::Context { methods: conv.store.domains.config.supported_methods(), ..Default::default() });
    conv
}

fn serial() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

/// Run a board-3 outcome's job (what lib.rs `board3_outcome` spawns).
async fn run(conv: &Conversation, out: Outcome) -> Option<Result<String, String>> {
    match out {
        Outcome::Spawn(job) => Some(host::run(job, conv).await),
        _ => None,
    }
}

/// One board-1 click: route it, run the work it returns (what lib.rs
/// `perform_board1` does; UI-integration work is the host's).
async fn click(action: &str, value: Option<&str>, conv: &Arc<Conversation>) -> Vec<String> {
    let work = board1::route(action, value);
    let shown: Vec<String> = work.iter().map(|w| format!("{w:?}")).collect();
    for w in work {
        let _ = board1::execute(w, Some(conv.clone())).await;
    }
    shown
}

fn dialog(conv: &Conversation) -> String {
    host::lower_open(&conv.store).map(|l| l.dsl).unwrap_or_default()
}

fn editor() -> String {
    board1::mark_dirty();
    board1::view(990.0, 603.0).unwrap_or_default()
}

/// The index of the open select's option labelled `label` (its row label
/// `b1_prov_opt_t<i>`).
fn option_index(view: &str, label: &str) -> Option<usize> {
    (0..64).find(|i| {
        view.find(&format!("b1_prov_opt_t{i} := Label"))
            .and_then(|at| view[at..].find("text: ").map(|t| at + t))
            .is_some_and(|t| view[t..].starts_with(&format!("text: {label:?}")))
    })
}

async fn open_providers(conv: &Conversation) {
    host::reset();
    board1::close_all();
    let out = host::open(Dialog::Routes);
    run(conv, out).await;
}

/// Row 281 "read-only catalog" + row 282 capability states: without
/// `profile/llm/upsert` the directory is read-only (the web's notice, no
/// Add / Edit; Delete only on its own method); without `profile/llm/list`
/// it is unavailable and reads nothing — never an empty list.
#[tokio::test]
async fn the_directory_fails_closed_into_read_only_and_unavailable_states() {
    let _s = serial();
    let server = Server::start(Sim::new(r2_config()), &["profile/llm/upsert"]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    open_providers(&conv).await;
    let dsl = dialog(&conv);
    assert!(dsl.contains("Provider configuration is read-only on this server."), "the read-only notice");
    for absent in ["b3_routes_add_provider", "b3_routes_row_0_edit", "b3_routes_row_1_edit", "b3_routes_row_0_add"] {
        assert!(!dsl.contains(absent), "{absent} on a read-only server");
    }
    assert!(dsl.contains("b3_routes_row_1_delete"), "Delete rides its own advertised method");
    assert!(dsl.contains("Deepseek V4 Flash") && dsl.contains("Credential configured"), "the rows stay visible");
    assert_eq!(host::perform("b3.routes.add_provider", 0, &store), Outcome::Done);
    assert_eq!(host::perform("b3.routes.edit", 1, &store), Outcome::Done);
    assert!(server.sent("profile/llm/test").is_empty() && server.sent("profile/llm/upsert").is_empty());
    host::close();

    let server = Server::start(Sim::new(r2_config()), &["profile/llm/list"]).await;
    let conv = connect(&server).await;
    open_providers(&conv).await;
    let dsl = dialog(&conv);
    assert!(dsl.contains("This Octos server cannot report the active Profile’s configured providers."), "{dsl}");
    assert!(!dsl.contains("No model providers configured"), "unavailable is never an empty list");
    assert!(server.sent("profile/llm/list").is_empty(), "nothing read without the method");
    host::close();
}

/// Row 282 fail-closed unread-vs-empty (`model-management-projection.test.ts:136`):
/// before the first read lands the dialog is loading; a refused read is an
/// error with "Try again" — never "No model providers configured" — and Try
/// again reads it; an EMPTY configuration that was read is the empty state.
#[tokio::test]
async fn an_unread_configuration_is_an_error_with_try_again_never_an_empty_list() {
    let _s = serial();
    let mut sim = Sim::new(r2_config());
    sim.fail_list = 1;
    let server = Server::start(sim, &[]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    host::reset();
    board1::close_all();
    let out = host::open(Dialog::Routes);
    assert!(dialog(&conv).contains("Loading model providers…"), "the first read in flight is loading, not a failure");
    run(&conv, out).await;
    let dsl = dialog(&conv);
    assert!(dsl.contains("profile store unavailable") && dsl.contains("Try again"), "{dsl}");
    assert!(!dsl.contains("No model providers configured") && !dsl.contains("b3_routes_row_0"), "unread is not empty");
    let again = host::perform("b3.routes.retry", 0, &store);
    assert!(matches!(again, Outcome::Spawn(_)), "Try again reads again: {again:?}");
    run(&conv, again).await;
    let dsl = dialog(&conv);
    assert!(dsl.contains("b3_routes_row_1_name") && !dsl.contains("Try again"), "the read landed");
    assert_eq!(server.sent("profile/llm/list").len(), 2);
    host::close();

    let server = Server::start(Sim::new(json!({"profile_id": "dsflash", "primary": null, "fallbacks": []})), &[]).await;
    let conv = connect(&server).await;
    open_providers(&conv).await;
    let dsl = dialog(&conv);
    assert!(dsl.contains("No model providers configured") && dsl.contains("Add a provider route to make a model available to Core."));
    assert!(dsl.contains("b3_routes_add_provider"), "an empty configuration can add");
    host::close();
}

/// Row 281's reachability for a Profile with NO provider yet: the Models
/// dialog (the entry to the providers dialog) still offers "Manage
/// providers" under its empty line, and the providers dialog it opens is the
/// web's empty state with "Add provider" (`ModelManagementSection.tsx:1270`,
/// `:1403-1408`); the first model added becomes the primary.
#[tokio::test]
async fn an_empty_profile_reaches_add_provider_from_the_models_dialog() {
    let _s = serial();
    let server = Server::start(Sim::new(json!({"profile_id": "dsflash", "primary": null, "fallbacks": []})), &[]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    let ui = Mutex::new(octoscode_module::flow::FlowUi::default());
    let ctx = octoscode_module::bindings::Ctx::new(&store, &ui);
    let models = octoscode_module::screens::dialog::lower(octoscode_module::screens::dialog::Dialog::Models, &ctx, 990.0, 603.0)
        .expect("the Models dialog lowers")
        .dsl;
    assert!(models.contains("No models are configured for this Profile."));
    assert!(models.contains("Manage providers") && models.contains("b3.open.routes"), "the way to the providers stays");
    open_providers(&conv).await;
    assert_eq!(host::perform("b3.routes.add_provider", 0, &store), Outcome::Action("b1.open.provider.routes".into()));
    click("b1.open.provider.routes", None, &conv).await;
    assert!(editor().contains("claude-3-5-haiku-20241022 (default)"), "the first model of an empty Profile is its primary");
    board1::route("provider.key", Some(DUMMY));
    click("provider.save", None, &conv).await;
    let ups = server.sent("profile/llm/upsert");
    assert_eq!(ups.len(), 1);
    assert_eq!(ups[0]["set_primary"], true, "an empty configuration's first provider is the primary");
    board1::take_pending();
}

/// Row 282 edit_blocked (`model-management-projection.test.ts:11`): a row
/// carrying a key the closed write schema cannot preserve stays visible and
/// deletable, shows why, and never opens the editor; the Settings entry's
/// editor for such a primary is read-only and sends nothing.
#[tokio::test]
async fn an_edit_blocked_row_is_visible_and_deletable_but_never_edited() {
    let _s = serial();
    let mut config = r2_config();
    config["fallbacks"].as_array_mut().unwrap().extend(extra_rows());
    let server = Server::start(Sim::new(config), &[]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    open_providers(&conv).await;
    let dsl = dialog(&conv);
    let row = &dsl[dsl.find("b3_routes_row_2 :=").expect("row 2")..dsl.find("b3_routes_row_3 :=").expect("row 3")];
    assert!(row.contains("Kimi K2") && row.contains("This entry contains settings the editor cannot preserve. Edit it through Core configuration instead."));
    assert!(row.contains("b3_routes_row_2_delete") && !row.contains("b3_routes_row_2_edit"), "deletable, not editable");
    assert!(dsl.contains("b3_routes_row_3_edit"), "the inference row is editable");
    assert_eq!(host::perform("b3.routes.edit", 2, &store), Outcome::Done, "no editor for a blocked row");
    assert_ne!(board1::top(), Some(board1::Surface::Provider));
    // Delete it behind the exact phrase.
    host::perform("b3.routes.delete", 2, &store);
    let dsl = dialog(&conv);
    assert!(dsl.contains("Type DELETE moonshot/kimi-k2 to confirm") && dsl.contains("This removes the configured route for Kimi K2."));
    host::input_changed("routes.phrase", "DELETE moonshot/kimi-k2");
    run(&conv, host::perform("b3.routes.confirm_delete", 0, &store)).await;
    assert_eq!(server.sent("profile/llm/delete"), [json!({"profile_id": "dsflash", "family_id": "moonshot", "model_id": "kimi-k2", "route_id": "moonshot"})]);
    let dsl = dialog(&conv);
    assert!(dsl.contains("Provider deleted. Restart Octos before relying on the updated runtime policy.") && !dsl.contains("Kimi K2"));
    host::close();

    // The Settings entry (A2's editor) for an edit-blocked PRIMARY.
    let mut config = r2_config();
    config["primary"]["strong"] = json!(false);
    let server = Server::start(Sim::new(config), &[]).await;
    let conv = connect(&server).await;
    board1::close_all();
    click("b1.open.provider", None, &conv).await;
    let v = editor();
    assert!(v.contains("This entry contains settings the editor cannot preserve. Edit it through Core configuration instead."), "the reason");
    assert!(!v.contains("b1_prov_save :=") && v.contains("is_read_only: true"), "read-only, no Save");
    board1::route("provider.key", Some(DUMMY));
    assert!(click("provider.save", None, &conv).await.is_empty(), "Save routes nothing");
    assert!(server.sent("profile/llm/test").is_empty() && server.sent("profile/llm/upsert").is_empty(), "nothing on the wire");
    board1::route("provider.cancel", None);
    provider::state().key.clear();
}

/// Row 281 "edit ANY configured provider" + row 282 "retains configured
/// inference values through a label-only existing edit"
/// (`model-management-projection.test.ts:53`): Edit on a FALLBACK opens the
/// board-1 editor for that row; a label-only Save tests then upserts ONE
/// provision that carries every configured inference value (null included),
/// keeps the row a fallback, sends no key (blank keeps the stored one), and
/// returns to the providers dialog with the web's line.
#[tokio::test]
async fn edit_any_row_keeps_its_configured_inference_through_a_label_only_save() {
    let _s = serial();
    let mut config = r2_config();
    config["fallbacks"].as_array_mut().unwrap().extend(extra_rows());
    let server = Server::start(Sim::new(config), &[]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    open_providers(&conv).await;
    assert_eq!(host::perform("b3.routes.edit", 3, &store), Outcome::Action("b1.open.provider.routes".into()));
    assert!(host::lower_open(&store).is_none(), "the editor takes the dialog's place");
    click("b1.open.provider.routes", None, &conv).await;
    assert_eq!(board1::top(), Some(board1::Surface::Provider));
    let v = editor();
    assert!(v.contains("Edit provider") && v.contains("Z.AI · Official API") && v.contains("glm-5.3-flash"), "the row's editor");
    assert!(v.contains("Provider identity is fixed. Add another provider to change it."), "identity fixed");
    board1::route("provider.name", Some("Z.AI · Renamed only"));
    let work = click("provider.save", None, &conv).await;
    assert!(work.iter().all(|w| !w.contains(DUMMY)), "{work:?}");
    let (tests, ups) = (server.sent("profile/llm/test"), server.sent("profile/llm/upsert"));
    assert_eq!((tests.len(), ups.len()), (1, 1), "test then upsert");
    assert_eq!(tests[0]["selection"], ups[0]["selection"], "ONE provision: test and save cannot drift");
    let sel = &ups[0]["selection"];
    for (k, want) in inference().as_object().unwrap() {
        assert_eq!(sel.get(k), Some(want), "{k} carried through the edit");
    }
    assert!(sel.as_object().unwrap().contains_key("top_p") && sel["top_p"].is_null(), "an explicit null is kept");
    assert!(sel.get("max_output_tokens").is_none(), "nothing added");
    assert_eq!(sel["route"]["label"], "Renamed only");
    assert_eq!((sel["family_id"].as_str(), sel["model_id"].as_str(), sel["route"]["route_id"].as_str()), (Some("zai"), Some("glm-5.3-flash"), Some("zai")));
    assert_eq!(ups[0]["set_primary"], false, "a fallback stays a fallback");
    assert!(ups[0].get("api_key").is_none(), "a blank key keeps the stored one");
    assert_ne!(board1::top(), Some(board1::Surface::Provider), "a saved editor closes");
    let back: Vec<String> = board1::take_pending().iter().map(|w| format!("{w:?}")).collect();
    assert!(
        back.iter().any(|w| w.contains("ReturnToProviders") && w.contains("Provider saved. Restart Octos before relying on this route or credential change.")),
        "back to the providers dialog with the web's line: {back:?}"
    );
}

/// Row 282 through A2's Settings entry: the primary's configured inference
/// values ride its Save (the old editor sent `LlmInferenceOverrides::default()`).
#[tokio::test]
async fn the_settings_editor_saves_the_primary_with_its_configured_inference() {
    let _s = serial();
    let mut config = r2_config();
    for (k, v) in inference().as_object().unwrap() {
        config["primary"][k] = v.clone();
    }
    let server = Server::start(Sim::new(config), &[]).await;
    let conv = connect(&server).await;
    board1::close_all();
    click("b1.open.provider", None, &conv).await;
    click("provider.save", None, &conv).await;
    let ups = server.sent("profile/llm/upsert");
    assert_eq!(ups.len(), 1, "saved: {}", server.all());
    for (k, want) in inference().as_object().unwrap() {
        assert_eq!(ups[0]["selection"].get(k), Some(want), "{k}");
    }
    assert_eq!(ups[0]["set_primary"], true, "the primary stays the primary");
}

/// Row 281 "Add provider" (catalog-driven), "Test connection", "Fetch
/// available models" and Save, with row 282's key rule: the editor opens on
/// the first catalog model NOT configured yet; the family select is the
/// catalog's; an existing identity and a missing key are refused before the
/// wire; Test sends `profile/llm/test` ALONE; Fetch the unsaved endpoint;
/// Save test + upsert; the key rides only those requests' `api_key` — never
/// a selection, a work item or the dialog's state.
#[tokio::test]
async fn add_provider_is_catalog_driven_and_tests_fetches_saves_with_the_key_as_an_argument() {
    let _s = serial();
    let server = Server::start(Sim::new(r2_config()), &[]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    open_providers(&conv).await;
    assert!(!server.sent("profile/llm/catalog").is_empty(), "the catalog is read with the configuration");
    assert_eq!(host::perform("b3.routes.add_provider", 0, &store), Outcome::Action("b1.open.provider.routes".into()));
    click("b1.open.provider.routes", None, &conv).await;
    let v = editor();
    assert!(v.contains("Add provider") && v.contains("Anthropic") && v.contains("claude-3-5-haiku-20241022"), "the first unconfigured catalog model");
    // The catalog's families; DeepSeek's first model and its Official API.
    click("provider.select.family", None, &conv).await;
    let v = editor();
    let deepseek = option_index(&v, "DeepSeek").expect("DeepSeek is a catalog option");
    click(&format!("provider.option.{deepseek}"), None, &conv).await;
    let v = editor();
    assert!(v.contains("DeepSeek · Official API") && v.contains("deepseek-v4-flash") && v.contains("deepseek-v4-pro"));
    // deepseek-v4-flash on the Official API is the configured primary.
    assert!(click("provider.save", None, &conv).await.is_empty(), "an existing identity never saves as new");
    assert!(editor().contains("This model route already exists. Use Edit instead."));
    click("provider.model.1", None, &conv).await;
    assert!(click("prov.test", None, &conv).await.is_empty(), "no key, no test");
    assert!(editor().contains("Enter an API key."));
    assert!(server.sent("profile/llm/test").is_empty() && server.sent("profile/llm/upsert").is_empty());
    board1::route("provider.key", Some(DUMMY));
    // Test connection: the test alone.
    let work = click("prov.test", None, &conv).await;
    assert!(work.iter().all(|w| !w.contains(DUMMY)), "the work carries no key: {work:?}");
    let tests = server.sent("profile/llm/test");
    assert_eq!(tests.len(), 1);
    assert!(server.sent("profile/llm/upsert").is_empty(), "Test never saves");
    assert_eq!(tests[0]["api_key"], DUMMY, "the key is the operation's argument");
    assert_eq!(
        tests[0]["selection"],
        json!({"family_id": "deepseek", "model_id": "deepseek-v4-pro",
               "route": {"route_id": "deepseek", "label": "Official API", "api_key_env": "DEEPSEEK_API_KEY", "api_type": "openai"}})
    );
    assert!(editor().contains("Connection succeeded."));
    // Fetch available models: the unsaved endpoint, family + route.
    click("provider.fetch", None, &conv).await;
    let fetches = server.sent("profile/llm/fetch_models");
    assert_eq!(fetches.len(), 1);
    assert_eq!(fetches[0]["selection"]["family_id"], "deepseek");
    assert_eq!(fetches[0]["api_key"], DUMMY);
    let v = editor();
    assert!(v.contains("3 available models found.") && v.contains("Available from endpoint") && v.contains("deepseek-reasoner"));
    // Save: test then upsert, a new fallback.
    click("provider.save", None, &conv).await;
    let (tests, ups) = (server.sent("profile/llm/test"), server.sent("profile/llm/upsert"));
    assert_eq!((tests.len(), ups.len()), (2, 1));
    assert_eq!(tests[1]["selection"], ups[0]["selection"]);
    assert_eq!((ups[0]["set_primary"].clone(), ups[0]["api_key"].clone()), (json!(false), json!(DUMMY)));
    let back: Vec<String> = board1::take_pending().iter().map(|w| format!("{w:?}")).collect();
    assert!(back.iter().any(|w| w.contains("ReturnToProviders") && w.contains("\"Provider saved.\"")), "{back:?}");
    // The key never entered a selection, the dialog's state, or a work item.
    for (_, p) in server.seen.lock().unwrap().iter() {
        if let Some(sel) = p.get("selection") {
            assert!(!sel.to_string().contains(DUMMY), "a selection carried the key");
        }
    }
    assert!(!format!("{:?}", host::state().routes).contains(DUMMY), "the dialog's state never holds the key");
    assert!(back.iter().all(|w| !w.contains(DUMMY)));
    // The providers dialog lists the new route.
    open_providers(&conv).await;
    assert!(dialog(&conv).contains("Deepseek V4 Pro"), "the saved provider is listed");
    host::close();
}

/// Row 281 Test connection failures: a key the provider refuses is board
/// 1's p4-07 (the recorded 401, the draft kept, nothing saved); any other
/// failure is the web's fixed sentence — never the server's prose.
#[tokio::test]
async fn a_failed_test_connection_keeps_the_draft_and_never_saves() {
    let _s = serial();
    let mut config = r2_config();
    config["fallbacks"].as_array_mut().unwrap().extend(extra_rows());
    let server = Server::start(Sim::new(config.clone()), &[]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    open_providers(&conv).await;
    assert_eq!(host::perform("b3.routes.edit", 1, &store), Outcome::Action("b1.open.provider.routes".into()));
    click("b1.open.provider.routes", None, &conv).await;
    board1::route("provider.key", Some(REJECTED));
    click("prov.test", None, &conv).await;
    let v = editor();
    assert!(v.contains("The provider rejected this key (401).") && v.contains("Your draft is kept."), "p4-07");
    assert!(!v.contains("Authentication Fails"), "no provider prose");
    assert!(provider::state().error.as_deref().is_some_and(|e| !e.contains(REJECTED)), "the kept failure is redacted");
    assert!(server.sent("profile/llm/upsert").is_empty(), "a failed test saves nothing");
    {
        let p = provider::state();
        assert_eq!((p.family.as_str(), p.route.as_str(), p.key.as_str()), ("deepseek", "r2-route", REJECTED), "the draft is kept");
    }
    click("provider.cancel", None, &conv).await;
    provider::state().key.clear();

    let mut sim = Sim::new(config);
    sim.test_fault = Some("upstream timeout after 30s".into());
    let server = Server::start(sim, &[]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    open_providers(&conv).await;
    host::perform("b3.routes.edit", 1, &store);
    click("b1.open.provider.routes", None, &conv).await;
    click("prov.test", None, &conv).await;
    let v = editor();
    assert!(v.contains("Connection failed. Check the endpoint, protocol, model, and credential."));
    assert!(!v.contains("upstream timeout"), "the server's prose stays out");
    click("provider.cancel", None, &conv).await;
}
