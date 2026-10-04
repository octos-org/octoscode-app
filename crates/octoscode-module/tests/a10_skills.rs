//! A10 — the Skills dialog's remaining production paths: the registry
//! search rows with the web's fields, install from a registry row and from
//! a free-form repo/branch (both through the confirm card), removal, the
//! Profile lease (one mutation at a time; every mutation pauses while the
//! Profile is busy or a turn runs). Replies: r2-profile (list, remove — the
//! recording) and `a10-skills-faithful.jsonl` (search, install successes —
//! no recording carries them).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, FlowUi};
use octoscode_module::screens::{dialog, models};

#[test]
fn skills_catalog_does_not_render_plugins_or_mcp_as_instruction_skills() {
    let _s = serial();
    let store = Arc::new(octoscode_store::Store::new());
    store.domains.profile.set_effective_skills(Some(json!({
        "session_id":"dev:api:chat", "effective_skills":[
            {"name":"review-guide","kind":"instructions","scope":"profile","path":"/profile/skills/review-guide/SKILL.md","available":true},
            {"name":"tool-plugin-only","kind":"plugin","scope":"global","path":"/plugins/tool-plugin-only","available":true},
            {"name":"mcp-server-only","kind":"mcp","scope":"global","available":true}
        ]
    })));
    let ui = Mutex::new(FlowUi::default());
    let ctx = Ctx::new(&store, &ui);
    let view = dialog::lower(dialog::Dialog::Skills, &ctx, 990.0, 760.0).unwrap();
    assert!(view.dsl.contains("review-guide"));
    assert!(!view.dsl.contains("tool-plugin-only"));
    assert!(!view.dsl.contains("mcp-server-only"));
    assert!(!view.dsl.contains("tools / plugin"));
}

fn frames(name: &str) -> Vec<Value> {
    let path = format!("{}/../octoscode-client/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("JSON"))
        .collect()
}

fn replies(name: &str, method: &str) -> Vec<Value> {
    frames(name)
        .into_iter()
        .filter(|f| f["dir"] == "in" && f["method"] == method && f.get("body").is_some())
        .map(|f| f["body"].clone())
        .collect()
}

fn recorded_open() -> Value {
    replies("r1-autonomy-a6ea8505.jsonl", "session/open")
        .into_iter()
        .find(|b| b.get("active_profile_id").is_some())
        .expect("r1 open")
}

struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Server {
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
                let frame = json!({"jsonrpc": "2.0", "id": id, "result": body});
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

    fn sent_all(&self, method: &str) -> Vec<Value> {
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

fn canned() -> Vec<(String, Value)> {
    let mut v: Vec<(String, Value)> = vec![("session/open".into(), json!({"opened": recorded_open()}))];
    for m in ["profile/skills/list", "profile/skills/remove"] {
        v.push((m.into(), replies("r2-profile-a6ea8505.jsonl", m).remove(0)));
    }
    for f in frames("a10-skills-faithful.jsonl").into_iter().filter(|f| f["dir"] == "in") {
        v.push((f["method"].as_str().unwrap().to_owned(), f["body"].clone()));
    }
    v
}

/// Search -> the registry rows show the web's fields; Install (row 0) asks
/// first, then sends `{profile_id, repo, force:false}` (no branch), shows the
/// receipt line, clears the searched packages and re-reads the Profile list.
#[tokio::test]
async fn registry_rows_show_the_webs_fields_and_install_through_the_confirm() {
    let _s = serial();
    let server = Server::start(canned(), vec![]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    let ui = Mutex::new(FlowUi::default());
    dialog::open(dialog::Dialog::Skills);
    dialog::set_skills_query(Some("lint".into()));
    let n = models::search_registry(&conv, &store, "lint").await.expect("search");
    assert_eq!(n, 2);
    assert_eq!(server.sent_all("profile/skills/registry/search")[0], json!({"q": "lint", "profile_id": "dsflash"}));
    let ctx = Ctx::new(&store, &ui);
    let m = dialog::lower(dialog::Dialog::Skills, &ctx, 990.0, 603.0).expect("skills");
    for want in [
        "Lint Rust and TypeScript changes before they are committed.",
        "octos-org/code-linter",
        "Provides executable tools · MIT",
        "Requires: ripgrep, cargo-clippy",
        "Instruction skills · License not reported",
        "Installed: api-client",
        dialog::SKILLS_WARNING,
        "Install from source",
    ] {
        assert!(m.dsl.contains(want), "the dialog shows {want:?}");
    }
    let events: Vec<&str> = m.taps.iter().map(|(_, e)| e.as_str()).collect();
    assert!(events.contains(&"dialog.ask.skills.install_3") && events.contains(&"dialog.ask.skills.install_4"), "{events:?}");
    assert!(events.contains(&"dialog.ask.skills.install_source"));
    // The confirm card names the repo and the default branch.
    let c = dialog::confirmation_for("skills.install_3", &store).expect("asks");
    assert_eq!(c.detail, "octos-org/code-linter · branch main");
    models::perform(&conv, "skills.install_3", &store).await.expect("install");
    assert_eq!(
        server.sent_all("profile/skills/install")[0],
        json!({"profile_id": "dsflash", "repo": "octos-org/code-linter", "force": false})
    );
    assert_eq!(
        dialog::notice_tone(dialog::Dialog::Skills),
        Some(("Server installed: code-linter. Skipped: none. Dependencies: ripgrep.".into(), false))
    );
    assert!(store.domains.profile.registry_packages().is_empty(), "the searched packages are cleared");
    assert_eq!(server.sent_all("profile/skills/list").last().unwrap(), &json!({"profile_id": "dsflash"}));
    assert!(!store.domains.profile.profile_busy(), "the lease is released");
    dialog::close();
}

/// "Install from source": the typed repo/branch go to the confirm card and
/// then to the wire (`branch` only when typed); Remove sends the recorded
/// r2 shape and reports it.
#[tokio::test]
async fn install_from_source_and_remove() {
    let _s = serial();
    let mut canned = canned();
    // The source install is the faithful fixture's SECOND install reply.
    canned.retain(|(m, b)| !(m == "profile/skills/install" && b["installed"][0] == "code-linter"));
    let server = Server::start(canned, vec![]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    dialog::open(dialog::Dialog::Skills);
    dialog::set_skills_source(" octos-org/review-kit ", " dev ");
    let c = dialog::confirmation_for("skills.install_source", &store).expect("asks");
    assert_eq!(c.detail, "octos-org/review-kit · branch dev");
    assert!(c.body.contains("Applies to Profile dsflash"));
    models::perform(&conv, "skills.install_source", &store).await.expect("installs");
    assert_eq!(
        server.sent_all("profile/skills/install")[0],
        json!({"profile_id": "dsflash", "repo": "octos-org/review-kit", "branch": "dev", "force": false})
    );
    assert_eq!(
        dialog::notice(dialog::Dialog::Skills).as_deref(),
        Some("Server installed: review-kit. Skipped: code-linter. Dependencies: none.")
    );
    // Remove (the recorded r2 reply).
    store.domains.profile.set_installed_skills(vec![octoscode_store::domains::profile::InstalledSkill {
        name: "r2-no-such-skill".into(),
        version: None,
        tool_count: 0,
        source_repo: None,
    }]);
    models::perform(&conv, "skills.remove_0", &store).await.expect("removes");
    assert_eq!(server.sent_all("profile/skills/remove")[0], json!({"name": "r2-no-such-skill", "profile_id": "dsflash"}));
    assert_eq!(dialog::notice(dialog::Dialog::Skills).as_deref(), Some("Removed r2-no-such-skill from server Profile dsflash."));
    dialog::close();
}

/// The Profile lease: while a mutation is in flight a second one is refused
/// (deduplicated) with nothing sent; a running turn also pauses mutations —
/// the dialog shows the web's lock line and wires no mutation control.
#[tokio::test]
async fn the_profile_lock_pauses_mutations_and_dedupes() {
    let _s = serial();
    let server = Server::start(canned(), vec![("profile/skills/remove", 300)]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    dialog::open(dialog::Dialog::Skills);
    store.domains.profile.set_installed_skills(vec![octoscode_store::domains::profile::InstalledSkill {
        name: "r2-no-such-skill".into(),
        version: None,
        tool_count: 0,
        source_repo: None,
    }]);
    let (c2, s2) = (conv.clone(), store.clone());
    let first = tokio::spawn(async move { models::perform(&c2, "skills.remove_0", &s2).await });
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(store.domains.profile.profile_busy(), "the lease is held while the remove is in flight");
    let ui = Mutex::new(FlowUi::default());
    {
        let ctx = Ctx::new(&store, &ui);
        let m = dialog::lower(dialog::Dialog::Skills, &ctx, 990.0, 603.0).unwrap();
        assert!(m.dsl.contains(dialog::SKILLS_LOCKED), "the lock line");
        assert!(!m.taps.iter().any(|(_, e)| e.starts_with("dialog.ask.skills.")), "no mutation is wired: {:?}", m.taps);
    }
    // A second mutation while the first holds the lease: refused, not sent.
    assert!(models::perform(&conv, "skills.remove_0", &store).await.is_err());
    first.await.unwrap().expect("the first completes");
    assert_eq!(server.sent_all("profile/skills/remove").len(), 1, "deduplicated");
    assert!(!store.domains.profile.profile_busy());
    // A running turn pauses mutations too.
    ui.lock().unwrap().begin_turn_now("t-running");
    let ctx = Ctx::new(&store, &ui);
    let m = dialog::lower(dialog::Dialog::Skills, &ctx, 990.0, 603.0).unwrap();
    assert!(m.dsl.contains(dialog::SKILLS_LOCKED));
    ui.lock().unwrap().end_turn_now(true);
    // (the re-read recorded list is empty: put the row back to see its control)
    store.domains.profile.set_installed_skills(vec![octoscode_store::domains::profile::InstalledSkill {
        name: "r2-no-such-skill".into(),
        version: None,
        tool_count: 0,
        source_repo: None,
    }]);
    let ctx = Ctx::new(&store, &ui);
    let m = dialog::lower(dialog::Dialog::Skills, &ctx, 990.0, 603.0).unwrap();
    assert!(!m.dsl.contains(dialog::SKILLS_LOCKED) && m.taps.iter().any(|(_, e)| e == "dialog.ask.skills.remove_0"));
    dialog::close();
}
