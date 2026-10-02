//! A10 — the Research provider lanes dialog (`/research`, alias `/lanes`)
//! through its production path: `board3::host::command` opens it,
//! `host::perform` routes its controls, `host::run` dispatches its jobs
//! (`board3::research::{load, mutate}`) through the real client against a
//! WebSocket server answering with recorded traffic — r2-profile's
//! list -> upsert -> list -> remove lifecycle — and with
//! `a10-research-faithful.jsonl` (a credential-bearing save whose server
//! reports no restart requirement; no recording carries one) plus the
//! failure shapes the web's parsers reject.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::Conversation;
use octoscode_module::screens::board3::host::{self, Job, Outcome};
use octoscode_module::screens::board3::research::{self, Confirm};

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
const FAITHFUL: &str = "a10-research-faithful.jsonl";
const LIST: &str = "profile/sub_providers/list";
const UPSERT: &str = "profile/sub_providers/upsert";
const REMOVE: &str = "profile/sub_providers/remove";

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
    /// Replies per method in order (the last repeats); a body
    /// `{"__error__": {..}}` answers a JSON-RPC error.
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

fn serial() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

fn open_canned() -> Vec<(String, Value)> {
    vec![("session/open".into(), json!({"opened": recorded_open()}))]
}

/// Open the dialog the way the palette row / typed command does and run its
/// list job.
async fn open(conv: &Conversation) -> u64 {
    host::reset();
    let job = match host::command("research", "", conv) {
        Some(Outcome::Spawn(job @ Job::ResearchLoad(_))) => job,
        other => panic!("/research opens the dialog and lists: {other:?}"),
    };
    let Job::ResearchLoad(generation) = job else { unreachable!() };
    host::run(job, conv).await.expect("list");
    generation
}

fn dsl(conv: &Conversation) -> host::Lowered {
    host::lower_open(&conv.store).expect("the dialog is open")
}

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().expect("object").keys().cloned().collect();
    k.sort();
    k
}

fn type_draft(fields: &[(&str, &str)]) {
    for (k, v) in fields {
        host::input_changed(&format!("research.{k}"), v);
    }
}

/// Run what a control routed (a job) and return its result.
async fn click(conv: &Conversation, action: &str, index: usize) -> Option<Result<String, String>> {
    match host::perform(action, index, &conv.store) {
        Outcome::Spawn(job) => Some(host::run(job, conv).await),
        Outcome::Done => None,
        other => panic!("{action}: {other:?}"),
    }
}

/// The recorded lifecycle: the list (`{profile_id}`), a save through the
/// confirmation (the recorded wire shape, no credential), the receipt's
/// lanes + the restart notice, a refresh, the removal (the recorded params
/// exactly) and its receipt.
#[tokio::test]
async fn the_recorded_lifecycle_lists_saves_and_removes_through_the_confirm() {
    let _s = serial();
    let mut canned = open_canned();
    for m in [LIST, UPSERT, REMOVE] {
        for b in dir(R2, "in", m) {
            canned.push((m.into(), b));
        }
    }
    let server = Server::start(canned, vec![]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    let generation = open(&conv).await;
    assert_eq!(server.sent(LIST), dir(R2, "out", LIST)[..1].to_vec(), "the recorded list params");
    let lanes: Vec<String> = store.domains.profile.sub_providers().iter().map(|l| l.key.clone()).collect();
    assert_eq!(lanes, ["strong", "cheap"]);
    let m = dsl(&conv);
    for want in ["Research provider lanes", "Server Profile: dsflash", "moonshot · kimi-k3", "API style: openai", research::INTRO] {
        assert!(m.dsl.contains(want), "{want:?}");
    }
    let taps: Vec<&str> = m.taps.iter().map(|(_, e)| e.as_str()).collect();
    for want in ["b3.research.edit#0", "b3.research.remove#1", "b3.research.refresh", "b3.research.clear", "b3.close"] {
        assert!(taps.contains(&want), "{want}: {taps:?}");
    }
    // The draft (no credential) -> Review -> the confirmation -> Confirm.
    type_draft(&[
        ("key", "r2-lane"),
        ("provider", "zhipu"),
        ("model", "glm-4-flash"),
        ("api_key_env", "ZHIPU_API_KEY"),
        ("description", "r2 throwaway"),
    ]);
    assert!(host::live_visibility(&store).contains(&("b3_research_review_on".into(), true)));
    assert_eq!(click(&conv, "b3.research.review", 0).await, None);
    let m = dsl(&conv);
    assert!(m.dsl.contains("Confirm lane save") && m.dsl.contains("r2-lane · zhipu · glm-4-flash"));
    assert!(m.dsl.contains("This changes server Profile dsflash. The response will report whether a restart is required."));
    assert!(server.sent(UPSERT).is_empty(), "nothing is sent before the confirmation");
    let r = click(&conv, "b3.research.confirm", 0).await.expect("a job");
    assert_eq!(r.as_deref(), Ok(octoscode_module::screens::research::mutation_notice(true, true)));
    let sent = server.sent(UPSERT);
    let recorded = dir(R2, "out", UPSERT).remove(0);
    assert_eq!(keys(&sent[0]), keys(&recorded), "the recorded envelope (no api_key)");
    assert_eq!(keys(&sent[0]["sub_provider"]), keys(&recorded["sub_provider"]), "the whitelist, absent = omitted");
    for k in ["key", "provider", "model", "description"] {
        assert_eq!(sent[0]["sub_provider"][k], recorded["sub_provider"][k], "{k}");
    }
    assert_eq!(sent[0]["profile_id"], "dsflash");
    assert_eq!(store.domains.profile.sub_providers().len(), 3, "the receipt's lanes replace the list");
    assert!(!store.domains.profile.profile_busy(), "the lease is released");
    {
        let st = host::state();
        assert_eq!(st.research.notice.as_deref(), Some(octoscode_module::screens::research::mutation_notice(true, true)));
        assert!(st.research.confirm.is_none() && !st.research.busy && st.research.generation == generation);
    }
    // Refresh: the recorded second list.
    click(&conv, "b3.research.refresh", 0).await.expect("job").expect("list");
    assert_eq!(server.sent(LIST).len(), 2);
    // Remove r2-lane (row 2): asks, then the recorded params exactly.
    assert_eq!(click(&conv, "b3.research.remove", 2).await, None);
    let m = dsl(&conv);
    assert!(m.dsl.contains("Confirm lane removal") && m.dsl.contains("Removing a lane requires re-adding its configuration"));
    click(&conv, "b3.research.confirm", 0).await.expect("job").expect("removed");
    assert_eq!(server.sent(REMOVE), dir(R2, "out", REMOVE), "the recorded remove params");
    let lanes: Vec<String> = store.domains.profile.sub_providers().iter().map(|l| l.key.clone()).collect();
    assert_eq!(lanes, ["strong", "cheap"]);
    host::close();
}

/// The credential (faithful fixture): read only at dispatch into `api_key`,
/// never in the draft, the confirmation, the state or the store; cleared
/// once sent (the next save carries none). The no-restart notice.
#[tokio::test]
async fn a_credential_rides_only_the_dispatch() {
    let _s = serial();
    let mut canned = open_canned();
    for m in [LIST, UPSERT, REMOVE] {
        for b in dir(FAITHFUL, "in", m) {
            canned.push((m.into(), b));
        }
    }
    let server = Server::start(canned, vec![]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    open(&conv).await;
    const SECRET: &str = "not-a-real-credential-0000";
    type_draft(&[
        ("key", "web"),
        ("provider", "zhipu"),
        ("model", "glm-4-flash"),
        ("api_key_env", "ZHIPU_API_KEY"),
        ("description", "Web research lane"),
        ("credential", SECRET),
    ]);
    assert!(!format!("{:?}", host::state().research).contains(SECRET), "never a state field");
    click(&conv, "b3.research.review", 0).await;
    let m = dsl(&conv);
    // The masked field is the only place it is drawn (re-seeded on remount).
    assert_eq!(m.dsl.matches(SECRET).count(), 1, "exactly one place: its masked field");
    for (i, _) in m.dsl.match_indices(SECRET) {
        let block = m.dsl[..i].rfind(":= ").map(|n| m.dsl[..n].rfind('\n').map(|l| l + 1).unwrap_or(0)).unwrap();
        assert!(m.dsl[block..i].starts_with("b3_research_credential := TextInput"), "leaked outside its field");
        let end = (i + 200).min(m.dsl.len());
        assert!(m.dsl[block..end].contains("is_password: true"));
    }
    assert!(!m.dsl.contains(&format!("web · zhipu · glm-4-flash · {SECRET}")), "not in the confirmation");
    let r = click(&conv, "b3.research.confirm", 0).await.expect("job");
    assert_eq!(r.as_deref(), Ok("Saved on the server; the server reports no restart requirement."));
    let sent = server.sent(UPSERT);
    assert_eq!(sent[0]["api_key"], SECRET, "the credential rides the dispatch");
    assert_eq!(keys(&sent[0]), ["api_key", "profile_id", "sub_provider"]);
    assert!(sent[0]["sub_provider"].get("api_key").is_none());
    assert_eq!(
        sent[0]["sub_provider"],
        json!({"key": "web", "provider": "zhipu", "model": "glm-4-flash", "api_key_env": "ZHIPU_API_KEY", "description": "Web research lane"})
    );
    assert!(!format!("{:?}", host::state().research).contains(SECRET));
    assert!(!format!("{:?}", store.domains.profile.sub_providers()).contains(SECRET));
    assert!(!dsl(&conv).dsl.contains(SECRET), "the masked field remounts empty");
    // Edit the saved lane and save again: no credential is re-sent.
    let web = store.domains.profile.sub_providers().iter().position(|l| l.key == "web").expect("web");
    click(&conv, "b3.research.edit", web).await;
    assert_eq!(host::state().research.snap.description, "Web research lane", "Edit fills the form");
    click(&conv, "b3.research.review", 0).await;
    click(&conv, "b3.research.confirm", 0).await.expect("job").expect("saved");
    assert!(server.sent(UPSERT)[1].get("api_key").is_none(), "cleared at dispatch");
    host::close();
}

/// Every mutation failure shows the web's one copy and releases the lease:
/// a credential without an env name (refused before the wire), a server
/// error (its text never shown or returned with the credential), a receipt
/// that omits the saved lane; a wrong-profile list is rejected.
#[tokio::test]
async fn failures_show_the_webs_copy_and_release_the_lease() {
    let _s = serial();
    let mut omitted = dir(FAITHFUL, "in", UPSERT).remove(0);
    omitted["sub_providers"].as_array_mut().unwrap().retain(|l| l["key"] != "web");
    let mut canned = open_canned();
    canned.push((LIST.into(), dir(FAITHFUL, "in", LIST).remove(0)));
    canned.push((LIST.into(), json!({"profile_id": "someone-else", "sub_providers": []})));
    canned.push((UPSERT.into(), json!({"__error__": {"code": -32000, "message": "provider rejected not-a-real-credential-1111"}})));
    canned.push((UPSERT.into(), omitted));
    let server = Server::start(canned, vec![]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    open(&conv).await;
    let error = || host::state().research.error.clone();
    // 1. A credential with no env name: refused before the wire.
    type_draft(&[("key", "web"), ("provider", "zhipu"), ("credential", "not-a-real-credential-1111")]);
    click(&conv, "b3.research.review", 0).await;
    let r = click(&conv, "b3.research.confirm", 0).await.expect("job");
    assert_eq!(r, Err("An API key environment name is required for a new credential".into()));
    assert!(server.sent(UPSERT).is_empty(), "nothing sent");
    assert_eq!(error().as_deref(), Some(research::MUTATION_FAILED));
    assert!(!store.domains.profile.profile_busy());
    // 2. A server error on a credential-bearing save: the copy, and the
    // returned cause never carries the credential.
    type_draft(&[("api_key_env", "ZHIPU_API_KEY"), ("credential", "not-a-real-credential-1111")]);
    click(&conv, "b3.research.review", 0).await;
    let r = click(&conv, "b3.research.confirm", 0).await.expect("job");
    let cause = r.expect_err("fails");
    assert!(!cause.contains("not-a-real-credential"), "{cause}");
    assert_eq!(error().as_deref(), Some(research::MUTATION_FAILED));
    assert!(!store.domains.profile.profile_busy());
    // 3. A receipt that omits the saved lane.
    click(&conv, "b3.research.review", 0).await;
    let r = click(&conv, "b3.research.confirm", 0).await.expect("job");
    assert_eq!(r, Err("Research save receipt omitted the requested lane; refresh before retrying".into()));
    assert_eq!(error().as_deref(), Some(research::MUTATION_FAILED));
    assert_eq!(store.domains.profile.sub_providers().len(), 2, "a rejected receipt never folds");
    // 4. A wrong-profile list.
    let r = click(&conv, "b3.research.refresh", 0).await.expect("job");
    assert_eq!(r, Err("Invalid or wrong-profile research lanes".into()));
    assert_eq!(error().as_deref(), Some("Invalid or wrong-profile research lanes"));
    assert!(dsl(&conv).dsl.contains("Invalid or wrong-profile research lanes"));
    host::close();
}

/// The generation guard (latest request wins): a list still in flight when
/// the dialog is closed and reopened never publishes; the new one does. A
/// mutation in flight holds the lease (the lock line) and the dialog does
/// not close under it.
#[tokio::test]
async fn a_superseded_request_never_publishes() {
    let _s = serial();
    let first = dir(R2, "in", LIST).remove(0); // strong, cheap
    let mut second = first.clone();
    second["sub_providers"].as_array_mut().unwrap().truncate(1); // strong
    let mut canned = open_canned();
    canned.push((LIST.into(), first));
    canned.push((LIST.into(), second));
    canned.push((REMOVE.into(), dir(R2, "in", REMOVE).remove(0)));
    let server = Server::start(canned, vec![(LIST, 250), (REMOVE, 250)]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    host::reset();
    let Some(Outcome::Spawn(job1)) = host::command("research", "", &conv) else { panic!("opens") };
    let c1 = conv.clone();
    let old = tokio::spawn(async move { host::run(job1, &c1).await });
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(dsl(&conv).dsl.contains("Waiting for the server…"));
    host::close();
    let Some(Outcome::Spawn(job2)) = host::command("lanes", "", &conv) else { panic!("the alias opens") };
    let c2 = conv.clone();
    let new = tokio::spawn(async move { host::run(job2, &c2).await });
    assert_eq!(old.await.unwrap(), Ok("superseded".into()), "the closed generation's reply is dropped");
    new.await.unwrap().expect("the current list");
    let lanes: Vec<String> = store.domains.profile.sub_providers().iter().map(|l| l.key.clone()).collect();
    assert_eq!(lanes, ["strong"], "only the current generation published");
    assert_eq!(server.sent(LIST).len(), 2);
    // A mutation in flight: the lease and the lock line; Close is refused.
    store.domains.profile.set_sub_providers(
        vec![octoscode_store::domains::profile::SubProvider {
            key: "r2-lane".into(),
            provider: "zhipu".into(),
            model: None,
            api_key_env: None,
            base_url: None,
            description: None,
            default_context_window: None,
            max_output_tokens: None,
            api_type: None,
        }],
    );
    click(&conv, "b3.research.remove", 0).await;
    let Outcome::Spawn(job) = host::perform("b3.research.confirm", 0, &store) else { panic!("confirm") };
    assert_eq!(job, Job::ResearchRemove(host::state().research.generation, "r2-lane".into()));
    let c3 = conv.clone();
    let pending = tokio::spawn(async move { host::run(job, &c3).await });
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(store.domains.profile.profile_busy(), "the lease is held");
    let m = dsl(&conv);
    assert!(m.dsl.contains(research::LOCKED) && m.dsl.contains("Waiting for the server…"));
    assert!(!m.taps.iter().any(|(_, e)| e == "b3.close"), "Close is inert while mutating");
    host::close();
    assert!(host::is_open(), "the dialog does not close under a mutation");
    pending.await.unwrap().expect("removed");
    assert!(!store.domains.profile.profile_busy());
    host::close();
    assert!(!host::is_open());
}

/// Known Profile work (a running turn) pauses every mutation: the lock
/// line, no mutation control wired, a stale tap routes nothing.
#[tokio::test]
async fn the_lock_pauses_every_mutation() {
    let _s = serial();
    let mut canned = open_canned();
    canned.push((LIST.into(), dir(FAITHFUL, "in", LIST).remove(0)));
    let server = Server::start(canned, vec![]).await;
    let conv = connect(&server).await;
    open(&conv).await;
    host::note_turn_busy(true);
    type_draft(&[("key", "web"), ("provider", "zhipu")]);
    let m = dsl(&conv);
    assert!(m.dsl.contains(research::LOCKED));
    assert!(!m
        .taps
        .iter()
        .any(|(_, e)| e.starts_with("b3.research.edit") || e.starts_with("b3.research.remove") || e == "b3.research.review"));
    assert_eq!(click(&conv, "b3.research.review", 0).await, None);
    assert_eq!(click(&conv, "b3.research.remove", 0).await, None);
    assert!(host::state().research.confirm.is_none());
    // A confirmation opened before the turn began cannot be confirmed.
    host::state().research.confirm = Some(Confirm::Remove("strong".into()));
    assert_eq!(click(&conv, "b3.research.confirm", 0).await, None);
    assert!(server.sent(UPSERT).is_empty() && server.sent(REMOVE).is_empty());
    host::note_turn_busy(false);
    host::close();
}
