//! A16 — the parity re-audit's production-path tests: each drives the SAME
//! functions the native UI calls (`board3::host::{open, perform, run,
//! lower_open, command}` for the board-3 surfaces, `screens::dialog` +
//! `screens::models::perform` for the Skills dialog) against a WebSocket
//! server answering with recorded traffic (r1 open, r2-profile, r3 status)
//! and the A10 faithful fixtures, then asserts what reached the wire and
//! what the surface now shows.
//!
//! - the permission menu's control state (web `permissionControlState` +
//!   `permissionOptions`, `permission-projection.ts:11-52`): loading,
//!   ready (whole presets, the current first), error + Retry, unavailable;
//! - the restart truth (`profileDefaultNeedsRestart`,
//!   `product-projection.ts:3-19`, and the Profile model notice,
//!   `ModelsSettingsContent.tsx:85-107`) in the Session settings pane;
//! - the Profile extensions (`ProfileExtensionsDialog.tsx`: Skills and
//!   Research share ONE Profile scope and ONE mutation lease / lock).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, FlowUi};
use octoscode_module::screens::board3::host::{self, Dialog, Job, Outcome};
use octoscode_module::screens::board3::{research, seats};
use octoscode_module::screens::{dialog, models};

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

/// r3's recorded `session/status/read` reply (runtime `deepseek-v4-flash` /
/// `deepseek`), addressed to `session`, with `model` replaced when given.
fn recorded_status(session: &str, model: Option<(&str, &str)>) -> Value {
    let mut v = frames("r3-session-a6ea8505.jsonl")
        .into_iter()
        .find(|f| f["method"] == "res:session/status/read")
        .map(|f| f["body"].clone())
        .expect("r3 records a status reply");
    v.as_object_mut().unwrap().remove("capabilities");
    v["session_id"] = json!(session);
    if let Some((m, p)) = model {
        v["model"] = json!({"model": m, "provider": p, "selected": true});
    }
    v
}

struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Server {
    /// Replies per method in order (the last repeats); a body
    /// `{"__error__": {..}}` answers a JSON-RPC error; `delays` hold a
    /// method's replies back (ms).
    async fn start(canned: Vec<(String, Value)>, delays: Vec<(&'static str, u64)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let mut queues: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for (m, b) in canned {
            queues.entry(m).or_default().push(b);
        }
        let delays: BTreeMap<&str, u64> = delays.into_iter().collect();
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (tx, mut rx) = ws.split();
            let tx = Arc::new(tokio::sync::Mutex::new(tx));
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
                let delay = delays.get(method.as_str()).copied().unwrap_or(0);
                let tx = tx.clone();
                tokio::spawn(async move {
                    if delay > 0 {
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                    }
                    let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
                });
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

/// The board-3 state, the dialog host and the store-wide lease are
/// process-global: one test at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|e| e.into_inner());
    host::reset();
    let dir = std::env::temp_dir().join(format!("a16-parity-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    std::env::set_var("OCTOSCODE_RECENTS_DIR", &dir);
    std::env::set_var("OCTOSCODE_SHOW_THINKING_FILE", dir.join("show-thinking.json"));
    std::env::set_var("OCTOSCODE_PANE_ADVANCED_FILE", dir.join("pane-advanced.json"));
    g
}

fn open_canned() -> Vec<(String, Value)> {
    vec![("session/open".into(), json!({"opened": recorded_open()}))]
}

/// Run what an open or a control routed (one job) and return its result.
async fn run(conv: &Conversation, out: Outcome) -> Option<Result<String, String>> {
    match out {
        Outcome::Spawn(job) => Some(host::run(job, conv).await),
        Outcome::Done | Outcome::Close => None,
        other => panic!("{other:?}"),
    }
}

fn lowered(conv: &Conversation) -> host::Lowered {
    host::lower_open(&conv.store).expect("a board-3 surface is open")
}

fn taps(m: &host::Lowered) -> Vec<String> {
    m.taps.iter().map(|(_, e)| e.clone()).collect()
}

// ------------------------------------------------- the permission projection

/// `permissionControlState` + `permissionOptions` through the seat's menu:
/// loading while the first read is in flight (no options yet), ready with the
/// whole presets (the current one first, `mode:network` ids, the web's
/// labels, Full access the dangerous one), error with its Retry, and
/// unavailable once the server stops advertising the read.
#[tokio::test]
async fn the_permission_menu_reads_loading_ready_error_and_unavailable() {
    let _s = serial();
    let mut canned = open_canned();
    let list = dir(R2, "in", "permission/profile/list").remove(0);
    canned.push(("permission/profile/list".into(), list.clone()));
    canned.push(("permission/profile/list".into(), json!({"__error__": {"code": -32000, "message": "permission store unavailable"}})));
    canned.push(("permission/profile/list".into(), list));
    let server = Server::start(canned, vec![("permission/profile/list", 400)]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    assert!(store.domains.profile.permission().is_none(), "nothing read yet");
    // Loading: the seat's click opens the menu and spawns the read.
    let Outcome::Spawn(job) = host::open(Dialog::Permission) else { panic!("the open reads the presets") };
    assert_eq!(job, Job::PermissionLoad);
    let c = conv.clone();
    let read = tokio::spawn(async move { host::run(job, &c).await });
    tokio::time::sleep(Duration::from_millis(120)).await;
    let m = lowered(&conv);
    assert!(m.dsl.contains(seats::PERMISSION_LOADING), "loading while the read is in flight with no presets");
    assert!(!taps(&m).iter().any(|t| t.starts_with("b3.perm.choose")), "nothing to choose yet");
    read.await.unwrap().expect("listed");
    // Ready: the whole presets, the current one first.
    let o = seats::permission_options(&store);
    let ids: Vec<&str> = o.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(ids, ["workspace_write:allow", "read_only:deny", "workspace_write:deny", "danger_full_access:allow"]);
    let labels: Vec<(&str, &str, bool)> = o.iter().map(|o| (o.mode_label, o.network_label, o.dangerous)).collect();
    assert_eq!(
        labels,
        [
            ("Write", "Network allowed", false),
            ("Read", "Network blocked", false),
            ("Write", "Network blocked", false),
            ("Full access", "Network allowed", true)
        ]
    );
    let m = lowered(&conv);
    assert!(!m.dsl.contains(seats::PERMISSION_LOADING));
    for want in ["Write · Network allowed", "Full access · Network allowed", "b3_shield_danger.svg", "b3_perm_opt_0_check"] {
        assert!(m.dsl.contains(want), "{want}");
    }
    // Error: the retried read fails; the menu says so and offers Retry.
    let r = run(&conv, host::perform("b3.perm.retry", 0, &store)).await.expect("a job");
    assert!(r.is_err());
    let m = lowered(&conv);
    assert!(m.dsl.contains("permission store unavailable") && m.dsl.contains("b3_perm_error_retry"), "the error line + Retry");
    assert!(taps(&m).contains(&"b3.perm.retry".to_owned()));
    // Retry: ready again, the error gone.
    run(&conv, host::perform("b3.perm.retry", 0, &store)).await.expect("a job").expect("listed");
    assert!(!lowered(&conv).dsl.contains("permission store unavailable"));
    // Unavailable: the server no longer advertises the read — the seat goes
    // and an open menu says so, with no preset to choose.
    let methods: Vec<String> =
        store.domains.config.supported_methods().into_iter().filter(|m| m != "permission/profile/list").collect();
    store.domains.config.set_supported_methods(methods);
    assert!(!seats::permission_seat(&store));
    let m = lowered(&conv);
    assert!(m.dsl.contains(seats::PERMISSION_UNAVAILABLE));
    assert!(!taps(&m).iter().any(|t| t.starts_with("b3.perm.choose")), "an unavailable menu routes nothing");
    assert_eq!(server.sent("permission/profile/list").len(), 3);
    host::close();
}

// -------------------------------------------------------- the restart truth

/// The strip's click (its wired `b3.strip.settings` tap) opens the Session
/// settings pane; the pane's reads run; its lowered DSL comes back.
async fn open_pane(conv: &Conversation) -> String {
    let job = match host::perform("b3.strip.settings", 0, &conv.store) {
        Outcome::Spawn(job) => job,
        other => panic!("the strip opens the pane: {other:?}"),
    };
    assert_eq!(host::open_dialog(), Some(Dialog::SessionPane));
    host::run(job, conv).await.expect("pane load");
    lowered(conv).dsl
}

/// The Session settings pane's Model card (`Saved for this profile` /
/// `Session runtime`): no restart notice while the status's runtime IS the
/// Profile default; after the model menu's selection of the r2-route
/// fallback answers r2's recorded restart_required, the pane opened from the
/// strip shows the web's notice; a reloaded answer puts the hint out; a
/// runtime that differs from the Profile default does not imply a restart.
#[tokio::test]
async fn the_pane_shows_the_restart_truth_of_the_runtime_and_the_last_answer() {
    let _s = serial();
    let session = "dsflash:main";
    let mut canned = open_canned();
    for _ in 0..3 {
        canned.push(("session/status/read".into(), recorded_status(session, None)));
    }
    canned.push(("session/status/read".into(), recorded_status(session, Some(("glm-5", "zhipu")))));
    canned.push(("profile/llm/list".into(), dir(SEATS, "in", "profile/llm/list").remove(0)));
    canned.push(("profile/llm/select".into(), dir(R2, "in", "profile/llm/select").remove(0)));
    canned.push(("profile/llm/select".into(), dir(SEATS, "in", "profile/llm/select").remove(0)));
    canned.push(("permission/profile/list".into(), dir(R2, "in", "permission/profile/list").remove(0)));
    let server = Server::start(canned, vec![]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    assert_eq!(conv.session_id(), session);
    const NOTICE: &str = "Profile default is DeepSeek V4 Flash. This Octos process is still serving deepseek-v4-flash. \
                          Restart Octos to apply the new default.";
    let pane = open_pane(&conv).await;
    for want in ["Saved for this profile:", "DeepSeek V4 Flash", "Session runtime", "deepseek-v4-flash"] {
        assert!(pane.contains(want), "{want}");
    }
    assert!(!pane.contains("Restart Octos"), "the runtime IS the Profile default and nothing hinted a restart");
    host::close();
    // The composer's model menu: the r2-route fallback -> restart_required.
    run(&conv, host::open(Dialog::ModelMenu)).await.expect("a load").expect("listed");
    let r = run(&conv, host::perform("b3.model.choose", 1, &store)).await.expect("a job");
    assert_eq!(r.as_deref(), Ok("Saved. The server keeps running deepseek-v4-flash until it restarts"));
    host::close();
    let pane = open_pane(&conv).await;
    assert!(pane.contains(NOTICE), "the last selection answered restart_required:\n{pane}");
    assert!(pane.contains("b3_sc_restart_text"));
    host::close();
    // Kimi answers reloaded: the hint goes out; the runtime the status
    // reports is still the listed default, so no notice.
    run(&conv, host::open(Dialog::ModelMenu)).await.expect("a load").expect("listed");
    run(&conv, host::perform("b3.model.choose", 2, &store)).await.expect("a job").expect("reloaded");
    host::close();
    let pane = open_pane(&conv).await;
    assert!(!pane.contains("Restart Octos"), "a reloaded answer puts the hint out");
    host::close();
    // A different runtime can be an admitted turn or a pending refresh. It
    // does not override the server's explicit successful reload response.
    let pane = open_pane(&conv).await;
    assert!(pane.contains("glm-5"), "the actual runtime remains visible: {pane}");
    assert!(!pane.contains("Restart Octos"), "a model mismatch alone must not infer a restart: {pane}");
    assert_eq!(server.sent("session/status/read").len(), 4);
    assert_eq!(server.sent("profile/llm/select").len(), 2);
    host::close();
}

// ------------------------------------------------ the Profile extensions

/// Skills and Research are the web's ProfileExtensionsDialog surfaces: ONE
/// Profile scope (both read and mutate `profile_id` dsflash, both headed
/// "Server Profile: dsflash") and ONE lease — a skill removal in flight
/// locks the Research lanes (the lock line, no Edit / Remove / Review wired,
/// a Remove routes nothing) until it lands; a lane removal in flight locks
/// the Skills dialog (its lock line, no mutation wired) and a skill
/// mutation then is refused with nothing sent.
#[tokio::test]
async fn skills_and_research_share_one_profile_scope_and_one_lease() {
    let _s = serial();
    let mut canned = open_canned();
    canned.push(("profile/skills/list".into(), dir(R2, "in", "profile/skills/list").remove(0)));
    canned.push(("profile/skills/remove".into(), dir(R2, "in", "profile/skills/remove").remove(0)));
    // r2's second lane list carries `r2-lane` (row 2), whose recorded
    // removal receipt follows.
    canned.push(("profile/sub_providers/list".into(), dir(R2, "in", "profile/sub_providers/list").remove(1)));
    canned.push(("profile/sub_providers/remove".into(), dir(R2, "in", "profile/sub_providers/remove").remove(0)));
    let server =
        Server::start(canned, vec![("profile/skills/remove", 400), ("profile/sub_providers/remove", 400)]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    // Research, opened the way the palette's /research row does.
    let job = match host::command("research", "", &conv) {
        Some(Outcome::Spawn(job @ Job::ResearchLoad(_))) => job,
        other => panic!("/research opens the lanes: {other:?}"),
    };
    host::run(job, &conv).await.expect("lanes listed");
    let lanes: Vec<String> = store.domains.profile.sub_providers().iter().map(|l| l.key.clone()).collect();
    assert_eq!(lanes, ["strong", "cheap", "r2-lane"]);
    let m = lowered(&conv);
    assert!(m.dsl.contains("Server Profile: dsflash") && !m.dsl.contains(research::LOCKED));
    assert!(taps(&m).contains(&"b3.research.remove#2".to_owned()));
    // The Skills dialog, the same Profile.
    dialog::open(dialog::Dialog::Skills);
    store.domains.profile.set_installed_skills(vec![octoscode_store::domains::profile::InstalledSkill {
        name: "r2-no-such-skill".into(),
        version: None,
        tool_count: 0,
        source_repo: None,
    }]);
    let ui = Mutex::new(FlowUi::default());
    {
        let ctx = Ctx::new(&store, &ui);
        let s = dialog::lower(dialog::Dialog::Skills, &ctx, 990.0, 603.0).expect("skills");
        assert!(s.dsl.contains("Server Profile: dsflash") && !s.dsl.contains(dialog::SKILLS_LOCKED));
    }
    // 1. A skill removal holds the lease: Research is locked.
    let (c2, s2) = (conv.clone(), store.clone());
    let skill = tokio::spawn(async move { models::perform(&c2, "skills.remove_0", &s2).await });
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert!(store.domains.profile.profile_busy(), "the skill removal holds the Profile lease");
    let m = lowered(&conv);
    assert!(m.dsl.contains(research::LOCKED), "the Research lock line");
    let t = taps(&m);
    assert!(
        !t.iter().any(|e| e.starts_with("b3.research.edit") || e.starts_with("b3.research.remove") || e == "b3.research.review"),
        "no lane mutation is wired while the Profile is busy: {t:?}"
    );
    assert_eq!(host::perform("b3.research.remove", 2, &store), Outcome::Done);
    assert!(host::state().research.confirm.is_none(), "a Remove routes nothing while locked");
    skill.await.unwrap().expect("the skill removal lands");
    assert!(!store.domains.profile.profile_busy());
    let m = lowered(&conv);
    assert!(!m.dsl.contains(research::LOCKED), "the lease released: Research unlocks");
    assert!(taps(&m).contains(&"b3.research.remove#2".to_owned()));
    // 2. A lane removal holds the lease: Skills is locked.
    assert_eq!(host::perform("b3.research.remove", 2, &store), Outcome::Done, "Remove asks first");
    let job = match host::perform("b3.research.confirm", 0, &store) {
        Outcome::Spawn(job @ Job::ResearchRemove(..)) => job,
        other => panic!("Confirm dispatches the removal: {other:?}"),
    };
    let c3 = conv.clone();
    let lane = tokio::spawn(async move { host::run(job, &c3).await });
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert!(store.domains.profile.profile_busy(), "the lane removal holds the Profile lease");
    {
        let ctx = Ctx::new(&store, &ui);
        let s = dialog::lower(dialog::Dialog::Skills, &ctx, 990.0, 603.0).expect("skills");
        assert!(s.dsl.contains(dialog::SKILLS_LOCKED), "the Skills lock line");
        assert!(!s.taps.iter().any(|(_, e)| e.starts_with("dialog.ask.skills.")), "no skill mutation is wired: {:?}", s.taps);
    }
    assert!(models::perform(&conv, "skills.remove_0", &store).await.is_err(), "a skill mutation is refused");
    lane.await.unwrap().expect("the lane removal lands");
    assert!(!store.domains.profile.profile_busy());
    // ONE scope on the wire: every read and mutation names Profile dsflash.
    assert_eq!(server.sent("profile/skills/remove"), [json!({"name": "r2-no-such-skill", "profile_id": "dsflash"})]);
    assert_eq!(server.sent("profile/sub_providers/remove"), [json!({"key": "r2-lane", "profile_id": "dsflash"})]);
    assert!(server.sent("profile/sub_providers/list").iter().all(|p| p["profile_id"] == "dsflash"));
    dialog::close();
    host::close();
}
