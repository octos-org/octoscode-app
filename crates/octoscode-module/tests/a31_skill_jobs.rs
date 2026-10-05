//! A31 — parity row 15: the Skills dialog's "Background jobs" section, on
//! the production path.
//!
//! The real `Conversation` connects to a scripted fake server: the
//! handshake, `session/open` (advertising `skill.action_jobs.v1` or not),
//! `skill/action/job/updated` notifications through the transport, the flow
//! and the client registry, and `skill/action/job/list` read by the same
//! `screens::skill_jobs::refresh` the host's dialog arm runs on open. The
//! dialog is lowered by `dialog::lower` (what the host mounts).
//!
//! The ordering bar (outer/LESSONS.md): the list reply and the
//! notifications apply on DIFFERENT tasks, so each test fixes one order of
//! application and the result must be the newest record per job, a terminal
//! status never regressing, and nothing of another Session or Profile.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, FlowUi};
use octoscode_module::i18n::{self, Lang};
use octoscode_module::screens::{dialog, skill_jobs};

const PROFILE: &str = "dsflash";

// ------------------------------------------------------------ fixtures

fn frames(name: &str) -> Vec<Value> {
    let path = format!("{}/../octoscode-client/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("JSON"))
        .collect()
}

/// The recorded r1 `session/open` reply (a6ea8505) under the Profile, with
/// the job feature and its two methods advertised or withdrawn.
fn open_reply(session: &str, advertise: bool) -> Value {
    let mut opened = frames("r1-autonomy-a6ea8505.jsonl")
        .into_iter()
        .filter(|f| f["dir"] == "in" && f["method"] == "session/open")
        .filter_map(|f| f.get("body").cloned())
        .find(|b| b.get("active_profile_id").is_some())
        .expect("r1 open");
    opened["session_id"] = json!(session);
    opened["active_profile_id"] = json!(PROFILE);
    let caps = &mut opened["capabilities"];
    let feats = caps["supported_features"].as_array_mut().expect("features");
    feats.retain(|f| f != skill_jobs::FEATURE);
    let meths = caps["supported_methods"].as_array_mut().expect("methods");
    meths.retain(|m| !m.as_str().unwrap_or("").starts_with("skill/action/job/"));
    if advertise {
        caps["supported_features"].as_array_mut().unwrap().push(json!(skill_jobs::FEATURE));
        let m = caps["supported_methods"].as_array_mut().unwrap();
        m.push(json!("skill/action/job/list"));
        m.push(json!("skill/action/job/read"));
    }
    opened
}

/// The server's RFC 3339 shape (chrono's `AutoSi`, UTC `Z`), `ago_ms` before now.
fn ts(ago_ms: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::milliseconds(ago_ms))
        .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
}

/// One `SkillActionJobRecord` as the server serializes it.
#[allow(clippy::too_many_arguments)]
fn job(profile: &str, session: &str, id: &str, status: &str, file: &str, updated_ago_ms: i64, extra: Value) -> Value {
    let mut j = json!({
        "job_id": id,
        "batch_id": "batch-a",
        "profile_id": profile,
        "session_id": session,
        "action_id": "source.import",
        "skill_id": "source-skill",
        "status": status,
        "input_path": format!("up://{file}"),
        "filename": file,
        "materialized_path": format!("uploads/{file}"),
        "created_at": ts(updated_ago_ms + 60_000),
        "updated_at": ts(updated_ago_ms),
    });
    if let (Some(o), Some(x)) = (j.as_object_mut(), extra.as_object()) {
        for (k, v) in x {
            o.insert(k.clone(), v.clone());
        }
    }
    j
}

fn updated(profile: &str, session: &str, job: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": "skill/action/job/updated",
           "params": {"profile_id": profile, "session_id": session, "job": job}})
}

// ---------------------------------------------------------- the server

/// A scripted UI-protocol server: answers `session/open` (advertising the
/// job feature or not) and every other read with `{}`, hands each
/// `skill/action/job/list` request to the test, and sends whatever frame
/// the test pushes, in order.
struct Fake {
    base_url: String,
    query_features: Arc<Mutex<Vec<String>>>,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
    push: mpsc::UnboundedSender<Value>,
    lists: tokio::sync::Mutex<mpsc::UnboundedReceiver<(String, Value)>>,
}

impl Fake {
    async fn start(advertise: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let query_features = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (push, mut push_rx) = mpsc::unbounded_channel::<Value>();
        let (list_tx, list_rx) = mpsc::unbounded_channel::<(String, Value)>();
        let (qf, log) = (query_features.clone(), seen.clone());
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let ws = tokio_tungstenite::accept_hdr_async(
                stream,
                move |req: &tokio_tungstenite::tungstenite::handshake::server::Request,
                      resp: tokio_tungstenite::tungstenite::handshake::server::Response| {
                    *qf.lock().unwrap() = req
                        .uri()
                        .query()
                        .map(|q| {
                            q.split('&')
                                .filter_map(|kv| kv.split_once('='))
                                .filter(|(k, _)| *k == "ui_feature")
                                .map(|(_, v)| v.replace("%2E", ".").replace("%5F", "_"))
                                .collect()
                        })
                        .unwrap_or_default();
                    Ok(resp)
                },
            )
            .await;
            let Ok(ws) = ws else { return };
            let (mut tx, mut rx) = ws.split();
            loop {
                tokio::select! {
                    msg = rx.next() => {
                        let Some(Ok(Message::Text(text))) = msg else {
                            if msg.is_none() { return; }
                            continue;
                        };
                        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                        let method = v["method"].as_str().unwrap_or("").to_owned();
                        let id = v["id"].as_str().unwrap_or("").to_owned();
                        log.lock().unwrap().push((method.clone(), v["params"].clone()));
                        let result = match method.as_str() {
                            "session/open" => {
                                let session = v["params"]["session_id"].as_str().unwrap_or("dsflash:main");
                                json!({"opened": open_reply(session, advertise)})
                            }
                            "skill/action/job/list" => {
                                let _ = list_tx.send((id, v["params"].clone()));
                                continue;
                            }
                            _ => json!({}),
                        };
                        let frame = json!({"jsonrpc": "2.0", "id": id, "result": result});
                        let _ = tx.send(Message::Text(frame.to_string().into())).await;
                    }
                    out = push_rx.recv() => {
                        let Some(frame) = out else { return };
                        let _ = tx.send(Message::Text(frame.to_string().into())).await;
                    }
                }
            }
        });
        Self {
            base_url: format!("http://{addr}"),
            query_features,
            seen,
            push,
            lists: tokio::sync::Mutex::new(list_rx),
        }
    }

    fn send(&self, frame: Value) {
        self.push.send(frame).expect("the fake server is up");
    }

    fn reply(&self, id: &str, result: Value) {
        self.send(json!({"jsonrpc": "2.0", "id": id, "result": result}));
    }

    /// The next `skill/action/job/list` request: (id, params).
    async fn next_list(&self) -> (String, Value) {
        tokio::time::timeout(Duration::from_secs(5), self.lists.lock().await.recv())
            .await
            .expect("a skill/action/job/list request")
            .expect("the server is up")
    }

    fn sent(&self, method: &str) -> usize {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).count()
    }
}

async fn wait_until(what: &str, mut f: impl FnMut() -> bool) {
    for _ in 0..250 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for {what}");
}

/// Connect, open the Session and let the flow apply the open reply (the
/// active Session, the Profile and the negotiated features).
async fn connect(fake: &Fake) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&fake.base_url, "dummy", PROFILE, None, None).expect("connect");
    let conv = Arc::new(conv);
    let drv = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            let _ = drv.on_event(evt);
        }
    });
    conv.open_workspace(None).await.expect("session/open");
    let store = conv.store.clone();
    wait_until("the open reply applied", || {
        store.active_session().is_some()
            && store.domains.profile.current().is_some()
            && !store.domains.config.supported_methods().is_empty()
    })
    .await;
    conv
}

fn serial() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

/// The open Skills dialog's DSL, lowered as the host lowers it.
fn skills_dsl(conv: &Conversation, w: f64, h: f64) -> String {
    let ui = Mutex::new(FlowUi::default());
    let ctx = Ctx::new(&conv.store, &ui);
    dialog::lower(dialog::Dialog::Skills, &ctx, w, h).expect("skills lowers").dsl
}

/// Every `text: "…"` of the lowered dialog, in document order.
fn texts(dsl: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = dsl;
    while let Some(at) = rest.find("text: \"") {
        let tail = &rest[at + 7..];
        let mut end = 0;
        let b = tail.as_bytes();
        while end < b.len() {
            if b[end] == b'\\' {
                end += 2;
                continue;
            }
            if b[end] == b'"' {
                break;
            }
            end += 1;
        }
        let raw = &tail[..end.min(tail.len())];
        out.push(serde_json::from_str::<String>(&format!("\"{raw}\"")).unwrap_or_else(|_| raw.to_owned()));
        rest = &tail[end.min(tail.len())..];
    }
    out
}

/// The text of widget `id` in the lowered DSL.
fn text_of(dsl: &str, id: &str) -> Option<String> {
    let at = dsl.find(&format!("{id} := Label"))?;
    texts(&dsl[at..]).into_iter().next()
}

/// Run the dialog's on-open loads the way the host does: every on-open id
/// is a dialog action, and the job read is the refresh arm's call.
async fn open_skills(conv: &Conversation) -> Vec<&'static str> {
    dialog::open(dialog::Dialog::Skills);
    let ids = dialog::Dialog::Skills.on_open().to_vec();
    for id in &ids {
        assert!(dialog::is_action(id), "{id} has the dialog host as its owner");
    }
    ids
}

// ---------------------------------------------------------------- tests

/// The handshake asks for `skill.action_jobs.v1` after the web's own list:
/// octos filters `skill/action/job/updated` out of every connection that
/// sent features without it (`ui_protocol_transport.rs`, the
/// `skill_action_jobs_available` gate), so without the token the event
/// never arrives.
#[tokio::test]
async fn the_handshake_asks_for_the_job_feature_after_the_webs_list() {
    let _s = serial();
    let fake = Fake::start(true).await;
    let _conv = connect(&fake).await;
    let got = fake.query_features.lock().unwrap().clone();
    let web = octoscode_client::features::WEB_UI_FEATURES;
    assert_eq!(&got[..web.len().min(got.len())], web, "the web's list first, in order");
    assert_eq!(&got[web.len().min(got.len())..], ["skill.action_jobs.v1"], "then the native extra: {got:?}");
}

/// Opening Skills reads the job list for the dialog's Profile AND Session,
/// and the section shows every status the server reports, newest first,
/// with the board's chips and message lines and the active-job count.
#[tokio::test]
async fn opening_skills_seeds_the_section_for_its_profile_and_session() {
    let _s = serial();
    let fake = Fake::start(true).await;
    let conv = connect(&fake).await;
    let session = conv.store.active_session().expect("session");
    let ids = open_skills(&conv).await;
    assert!(ids.contains(&dialog::ACTION_REFRESH_SKILL_JOBS), "Skills reads its jobs on open: {ids:?}");
    assert_eq!(dialog::resolve(dialog::ACTION_REFRESH_SKILL_JOBS), dialog::Effect::RefreshSkillJobs);
    assert!(skill_jobs::seeds(&conv.store), "the server advertises the feature and the list method");
    let (c2, s2) = (conv.clone(), conv.store.clone());
    let read = tokio::spawn(async move { skill_jobs::refresh(&c2, &s2).await });
    let (id, params) = fake.next_list().await;
    assert_eq!(params, json!({"profile_id": PROFILE, "session_id": session}), "scoped to the dialog's Profile + Session");
    let list = vec![
        job(PROFILE, &session, "j-scan", "failed", "scan-007.pdf", 300_000, json!({"error": "pdftotext exited with status 1"})),
        job(PROFILE, &session, "j-report", "running", "report.pdf", 12_000, json!({})),
        job(PROFILE, &session, "j-q3", "succeeded", "q3-review.pdf", 120_000, json!({"output": "Imported 14 pages"})),
        job(PROFILE, &session, "j-notes", "queued", "notes.md", 30_000, json!({})),
        job(PROFILE, &session, "j-deck", "cancelled", "q3-board.pptx", 1_200_000,
            json!({"skill_id": "deck-skill", "action_id": "deck.render"})),
        job(PROFILE, &session, "j-old", "abandoned", "archive.zip", 3_600_000, json!({"error": "orphaned across restart"})),
    ];
    fake.reply(&id, json!({"profile_id": PROFILE, "session_id": session, "count": list.len(), "jobs": list}));
    read.await.unwrap().expect("the list read");
    let dsl = skills_dsl(&conv, 990.0, 603.0);
    let all = texts(&dsl);
    let head = all.iter().position(|t| t == "Background jobs").expect("the section heading");
    let installed = all.iter().position(|t| t == "Installed instruction skills").expect("the Installed heading");
    assert!(head < installed, "the section sits at the top, above Installed: {all:?}");
    assert_eq!(text_of(&dsl, "dlg_skills_jobs_count").as_deref(), Some("1 running · 1 queued"));
    // Newest first (updated_at): report 12 s, notes 30 s, q3 2 m, scan 5 m, deck 20 m, archive 1 h.
    let names: Vec<String> = (0..6).filter_map(|i| text_of(&dsl, &format!("dlg_skills_job_{i}_name"))).collect();
    assert_eq!(names, ["report.pdf", "notes.md", "q3-review.pdf", "scan-007.pdf", "q3-board.pptx", "archive.zip"]);
    let chips: Vec<String> = (0..6).filter_map(|i| text_of(&dsl, &format!("dlg_skills_job_{i}_state"))).collect();
    assert_eq!(chips, ["● Running", "○ Queued", "✓ Done", "✕ Failed", "✕ Stopped", "✕ Stopped"]);
    assert_eq!(text_of(&dsl, "dlg_skills_job_2_msg").as_deref(), Some("Imported 14 pages"));
    assert_eq!(text_of(&dsl, "dlg_skills_job_3_msg").as_deref(), Some("Couldn't finish this job."));
    assert_eq!(text_of(&dsl, "dlg_skills_job_3_cause").as_deref(), Some("pdftotext exited with status 1"));
    assert_eq!(text_of(&dsl, "dlg_skills_job_5_msg").as_deref(), Some("The server restarted before this job finished."));
    for quiet in [0, 1, 4] {
        assert!(text_of(&dsl, &format!("dlg_skills_job_{quiet}_msg")).is_none(), "row {quiet} has no message line");
    }
    assert_eq!(text_of(&dsl, "dlg_skills_job_0_skill").as_deref(), Some("source-skill"));
    assert_eq!(text_of(&dsl, "dlg_skills_job_0_action").as_deref(), Some("· source.import"));
    assert_eq!(text_of(&dsl, "dlg_skills_job_4_skill").as_deref(), Some("deck-skill"));
    assert_eq!(text_of(&dsl, "dlg_skills_job_0_time").as_deref(), Some("now"));
    assert_eq!(text_of(&dsl, "dlg_skills_job_2_time").as_deref(), Some("2m"));
    assert!(text_of(&dsl, "dlg_skills_jobs_note").is_none(), "a seeding server needs no note");
    dialog::close();
}

/// The list reply races the notifications. An update the client applied
/// BEFORE the reply — a job queued after the server took its snapshot —
/// survives the reply that does not name it; and a notification still in
/// flight (applied AFTER the reply) that is older than the reply's record
/// changes nothing.
#[tokio::test]
async fn an_update_applied_before_the_list_reply_survives_it() {
    let _s = serial();
    let fake = Fake::start(true).await;
    let conv = connect(&fake).await;
    let session = conv.store.active_session().expect("session");
    open_skills(&conv).await;
    let (c2, s2) = (conv.clone(), conv.store.clone());
    let read = tokio::spawn(async move { skill_jobs::refresh(&c2, &s2).await });
    let (id, _) = fake.next_list().await;
    // The server announces a job its snapshot does not hold yet.
    fake.send(updated(PROFILE, &session, job(PROFILE, &session, "j-new", "queued", "notes.md", 1_000, json!({}))));
    let store = conv.store.clone();
    let sid = session.clone();
    wait_until("the update applied before the reply", || {
        store.domains.skill_jobs.jobs(PROFILE, &sid).iter().any(|j| j.job_id == "j-new")
    })
    .await;
    let snapshot = vec![job(PROFILE, &session, "j-q3", "succeeded", "q3-review.pdf", 60_000, json!({"output": "Imported 14 pages"}))];
    fake.reply(&id, json!({"profile_id": PROFILE, "session_id": session, "count": 1, "jobs": snapshot}));
    read.await.unwrap().expect("the list read");
    let jobs = conv.store.domains.skill_jobs.jobs(PROFILE, &session);
    let ids: Vec<&str> = jobs.iter().map(|j| j.job_id.as_str()).collect();
    assert_eq!(ids, ["j-new", "j-q3"], "the announced job survives the snapshot that predates it");
    // A notification still in flight, OLDER than the reply's record.
    fake.send(updated(PROFILE, &session, job(PROFILE, &session, "j-q3", "running", "q3-review.pdf", 90_000, json!({}))));
    // …then a fresh one, so the test knows the older one was applied.
    fake.send(updated(PROFILE, &session, job(PROFILE, &session, "j-marker", "queued", "m.txt", 0, json!({}))));
    let sid = session.clone();
    wait_until("the marker applied", || store.domains.skill_jobs.jobs(PROFILE, &sid).iter().any(|j| j.job_id == "j-marker"))
        .await;
    let q3 = conv.store.domains.skill_jobs.jobs(PROFILE, &session).into_iter().find(|j| j.job_id == "j-q3").unwrap();
    assert_eq!(q3.status, octoscode_store::domains::skill_jobs::JobStatus::Succeeded, "an older update never wins");
    let dsl = skills_dsl(&conv, 990.0, 603.0);
    assert_eq!(text_of(&dsl, "dlg_skills_jobs_count").as_deref(), Some("2 queued"));
    dialog::close();
}

/// A stale list reply — its snapshot taken before a newer update the client
/// already applied — never regresses that job: the newer record (by
/// `updated_at`) stays, and a terminal status never turns active again.
#[tokio::test]
async fn a_stale_list_reply_after_a_newer_update_never_regresses_it() {
    let _s = serial();
    let fake = Fake::start(true).await;
    let conv = connect(&fake).await;
    let session = conv.store.active_session().expect("session");
    open_skills(&conv).await;
    let (c2, s2) = (conv.clone(), conv.store.clone());
    let read = tokio::spawn(async move { skill_jobs::refresh(&c2, &s2).await });
    let (id, _) = fake.next_list().await;
    fake.send(updated(PROFILE, &session, job(PROFILE, &session, "j-report", "succeeded", "report.pdf", 2_000,
        json!({"output": "Imported 9 pages"}))));
    let store = conv.store.clone();
    let sid = session.clone();
    wait_until("the newer update applied", || {
        store.domains.skill_jobs.jobs(PROFILE, &sid).iter().any(|j| j.job_id == "j-report")
    })
    .await;
    // The snapshot predates it: report.pdf still running there.
    let stale = vec![
        job(PROFILE, &session, "j-report", "running", "report.pdf", 40_000, json!({})),
        job(PROFILE, &session, "j-notes", "queued", "notes.md", 30_000, json!({})),
    ];
    fake.reply(&id, json!({"profile_id": PROFILE, "session_id": session, "count": 2, "jobs": stale}));
    read.await.unwrap().expect("the list read");
    let dsl = skills_dsl(&conv, 990.0, 603.0);
    assert_eq!(text_of(&dsl, "dlg_skills_job_0_name").as_deref(), Some("report.pdf"));
    assert_eq!(text_of(&dsl, "dlg_skills_job_0_state").as_deref(), Some("✓ Done"), "the stale reply did not revive it");
    assert_eq!(text_of(&dsl, "dlg_skills_job_0_msg").as_deref(), Some("Imported 9 pages"));
    assert_eq!(text_of(&dsl, "dlg_skills_jobs_count").as_deref(), Some("1 queued"));
    // Even a record that claims to be newer cannot make a finished job active.
    fake.send(updated(PROFILE, &session, job(PROFILE, &session, "j-report", "running", "report.pdf", 0, json!({}))));
    fake.send(updated(PROFILE, &session, job(PROFILE, &session, "j-marker", "queued", "m.txt", 0, json!({}))));
    let sid = session.clone();
    wait_until("the marker applied", || store.domains.skill_jobs.jobs(PROFILE, &sid).iter().any(|j| j.job_id == "j-marker"))
        .await;
    let report = conv.store.domains.skill_jobs.jobs(PROFILE, &session).into_iter().find(|j| j.job_id == "j-report").unwrap();
    assert!(report.status.is_terminal(), "terminal stays terminal: {:?}", report.status);
    dialog::close();
}

/// Jobs belong to one (Profile, Session): a notification for another
/// Session, one for another Profile, and a foreign record inside a list
/// reply never show in this dialog — and the other Session's own dialog
/// shows its job.
#[tokio::test]
async fn a_job_of_another_session_or_profile_is_never_shown() {
    let _s = serial();
    let fake = Fake::start(true).await;
    let conv = connect(&fake).await;
    let session = conv.store.active_session().expect("session");
    let other = format!("{PROFILE}:other");
    fake.send(updated(PROFILE, &other, job(PROFILE, &other, "j-other-session", "running", "invoice.pdf", 5_000, json!({}))));
    fake.send(updated("someone-else", &session, job("someone-else", &session, "j-other-profile", "running", "secret.pdf", 5_000, json!({}))));
    // A record whose own scope disagrees with its envelope is refused too.
    fake.send(updated(PROFILE, &session, job(PROFILE, &other, "j-mislabelled", "running", "mislabelled.pdf", 5_000, json!({}))));
    fake.send(updated(PROFILE, &session, job(PROFILE, &session, "j-mine", "running", "report.pdf", 4_000, json!({}))));
    let store = conv.store.clone();
    let sid = session.clone();
    wait_until("this Session's own job applied", || {
        store.domains.skill_jobs.jobs(PROFILE, &sid).iter().any(|j| j.job_id == "j-mine")
    })
    .await;
    open_skills(&conv).await;
    let (c2, s2) = (conv.clone(), conv.store.clone());
    let read = tokio::spawn(async move { skill_jobs::refresh(&c2, &s2).await });
    let (id, _) = fake.next_list().await;
    let reply = vec![
        job(PROFILE, &session, "j-mine", "running", "report.pdf", 4_000, json!({})),
        job(PROFILE, &other, "j-foreign-in-list", "queued", "foreign.pdf", 3_000, json!({})),
    ];
    fake.reply(&id, json!({"profile_id": PROFILE, "session_id": session, "count": 2, "jobs": reply}));
    read.await.unwrap().expect("the list read");
    let dsl = skills_dsl(&conv, 990.0, 603.0);
    for foreign in ["invoice.pdf", "secret.pdf", "mislabelled.pdf", "foreign.pdf"] {
        assert!(!dsl.contains(foreign), "{foreign} is another Session's or Profile's job");
    }
    assert_eq!(text_of(&dsl, "dlg_skills_job_0_name").as_deref(), Some("report.pdf"));
    assert!(text_of(&dsl, "dlg_skills_job_1_name").is_none(), "exactly one row");
    assert_eq!(text_of(&dsl, "dlg_skills_jobs_count").as_deref(), Some("1 running"));
    // The other Session's dialog (its own scope) shows its own job only.
    conv.store.set_active(Some(other.clone()));
    let dsl = skills_dsl(&conv, 990.0, 603.0);
    assert_eq!(text_of(&dsl, "dlg_skills_job_0_name").as_deref(), Some("invoice.pdf"));
    assert!(!dsl.contains("report.pdf") && !dsl.contains("secret.pdf"));
    conv.store.set_active(Some(session));
    dialog::close();
}

/// Without `skill.action_jobs.v1` advertised the dialog reads no list: it
/// shows the jobs announced since connecting and says so in a muted line.
#[tokio::test]
async fn without_the_feature_only_announced_jobs_show_and_a_note_says_so() {
    let _s = serial();
    let fake = Fake::start(false).await;
    let conv = connect(&fake).await;
    let session = conv.store.active_session().expect("session");
    assert!(!skill_jobs::seeds(&conv.store));
    fake.send(updated(PROFILE, &session, job(PROFILE, &session, "j-scan", "failed", "scan-007.pdf", 30_000,
        json!({"error": "pdftotext exited with status 1"}))));
    let store = conv.store.clone();
    let sid = session.clone();
    wait_until("the announced job applied", || !store.domains.skill_jobs.jobs(PROFILE, &sid).is_empty()).await;
    open_skills(&conv).await;
    skill_jobs::refresh(&conv, &conv.store).await.expect("refresh answers without a read");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(fake.sent("skill/action/job/list"), 0, "no list is read from a server that does not advertise it");
    let dsl = skills_dsl(&conv, 990.0, 603.0);
    assert_eq!(text_of(&dsl, "dlg_skills_jobs_note").as_deref(), Some(skill_jobs::ANNOUNCED_ONLY));
    assert_eq!(text_of(&dsl, "dlg_skills_job_0_name").as_deref(), Some("scan-007.pdf"));
    assert_eq!(text_of(&dsl, "dlg_skills_job_0_state").as_deref(), Some("✕ Failed"));
    dialog::close();
}

/// The README's status table, one status at a time: the chip's glyph, word
/// and ink, and the message line.
#[test]
fn every_status_maps_to_the_boards_chip_and_message() {
    use octoscode_module::screens::board3::ui::tok;
    use octoscode_store::domains::skill_jobs::{JobStatus, SkillJob};
    let row = |status: &str, extra: Value| {
        let mut j = job(PROFILE, "dsflash:main", "j", status, "f.pdf", 0, extra);
        j["status"] = json!(status);
        SkillJob::from_wire(PROFILE, "dsflash:main", &j).expect("a server record")
    };
    let cases: Vec<(&str, Value, (&str, &str, &str, &str), skill_jobs::Message)> = vec![
        ("queued", json!({}), ("○", "Queued", tok::BLUE_TEXT, tok::BLUE_BG), skill_jobs::Message::None),
        ("running", json!({}), ("●", "Running", tok::GREEN_TEXT, tok::GREEN_BG), skill_jobs::Message::None),
        ("succeeded", json!({"output": "Imported 14 pages"}), ("✓", "Done", tok::TEXT, tok::CHIP),
            skill_jobs::Message::Output("Imported 14 pages".into())),
        ("failed", json!({"error": "pdftotext exited with status 1"}), ("✕", "Failed", tok::RED_TEXT, tok::RED_BG),
            skill_jobs::Message::Failure { lead: skill_jobs::FAILED_LEAD, cause: "pdftotext exited with status 1".into() }),
        ("cancelled", json!({}), ("✕", "Stopped", tok::TEXT, tok::CHIP), skill_jobs::Message::None),
        ("abandoned", json!({"error": "orphaned across restart"}), ("✕", "Stopped", tok::TEXT, tok::CHIP),
            skill_jobs::Message::Note(skill_jobs::ABANDONED_NOTE)),
    ];
    for (status, extra, (glyph, word, fg, bg), msg) in cases {
        let j = row(status, extra);
        let c = skill_jobs::chip(&j.status);
        assert_eq!((c.glyph, c.word, c.fg, c.bg), (glyph, word, fg, bg), "{status}");
        assert_eq!(skill_jobs::message(&j), msg, "{status}");
    }
    assert!(JobStatus::parse("succeeded").is_terminal() && JobStatus::parse("queued").is_active());
    // A succeeded job with no output, or a failed one with no error, keeps
    // the line honest: nothing, or the lead alone.
    assert_eq!(skill_jobs::message(&row("succeeded", json!({}))), skill_jobs::Message::None);
    assert_eq!(
        skill_jobs::message(&row("failed", json!({}))),
        skill_jobs::Message::Failure { lead: skill_jobs::FAILED_LEAD, cause: String::new() }
    );
}

/// The section's copy reads in Chinese (the web has no job UI, so its
/// strings are native copy with native zh) and the lowered section shows it.
#[tokio::test]
async fn the_section_reads_in_chinese() {
    let _s = serial();
    for (en, zh) in [
        (skill_jobs::HEADING, "后台作业"),
        (skill_jobs::FAILED_LEAD, "无法完成此作业。"),
        (skill_jobs::ABANDONED_NOTE, "服务器在此作业完成前已重启。"),
        (skill_jobs::EMPTY, "此会话没有后台作业。"),
        (skill_jobs::LOADING, "正在加载后台作业…"),
        (skill_jobs::LOAD_FAILED, "无法加载后台作业。"),
        ("Queued", "排队中"),
        ("Running", "运行中"),
        ("Done", "已完成"),
        ("Failed", "已失败"),
        ("Stopped", "已停止"),
    ] {
        assert_eq!(i18n::tr_in(Lang::Zh, en), zh, "{en}");
    }
    assert_eq!(i18n::text_in(Lang::Zh, skill_jobs::RUNNING_COUNT, &[("count", "1")]), "1 个运行中");
    assert_eq!(i18n::text_in(Lang::Zh, skill_jobs::QUEUED_COUNT, &[("value0", "2")]), "2 个排队中");
    let fake = Fake::start(true).await;
    let conv = connect(&fake).await;
    let session = conv.store.active_session().expect("session");
    fake.send(updated(PROFILE, &session, job(PROFILE, &session, "j-report", "running", "report.pdf", 1_000, json!({}))));
    let store = conv.store.clone();
    let sid = session.clone();
    wait_until("the job applied", || !store.domains.skill_jobs.jobs(PROFILE, &sid).is_empty()).await;
    dialog::open(dialog::Dialog::Skills);
    i18n::set_language(Lang::Zh);
    let dsl = skills_dsl(&conv, 360.0, 776.0);
    i18n::set_language(Lang::En);
    assert!(texts(&dsl).iter().any(|t| t == "后台作业"), "the heading in Chinese");
    assert_eq!(text_of(&dsl, "dlg_skills_job_0_state").as_deref(), Some("● 运行中"));
    assert_eq!(text_of(&dsl, "dlg_skills_jobs_count").as_deref(), Some("1 个运行中"));
    // Noto Sans SC carries the Chinese: the chip's text style names the
    // kit's CJK members, Noto Sans SC first (WenKai only as the lazy
    // rare-glyph fallback).
    let at = dsl.find("dlg_skills_job_0_state := Label").expect("the chip label");
    let block = &dsl[at..dsl[at..].find("\n}").map(|e| at + e).unwrap_or(dsl.len())];
    let noto = octoscode_module::design::cjk_face(500).expect("the bundled Noto Sans SC");
    assert!(noto.to_string_lossy().contains("NotoSansSC"));
    assert!(block.contains(&octoscode_module::design::cjk_members(500)), "the chip's style carries the CJK members");
    dialog::close();
}

/// A list of more jobs than the section draws keeps every active job and
/// says how many older ones it left out.
#[tokio::test]
async fn a_long_list_keeps_every_active_job_and_counts_the_rest() {
    let _s = serial();
    let fake = Fake::start(true).await;
    let conv = connect(&fake).await;
    let session = conv.store.active_session().expect("session");
    open_skills(&conv).await;
    let (c2, s2) = (conv.clone(), conv.store.clone());
    let read = tokio::spawn(async move { skill_jobs::refresh(&c2, &s2).await });
    let (id, _) = fake.next_list().await;
    let mut list: Vec<Value> = (0..30)
        .map(|i| job(PROFILE, &session, &format!("j-done-{i:02}"), "succeeded", &format!("f{i:02}.pdf"), 60_000 + i * 1_000, json!({})))
        .collect();
    // An old job still queued: older than every finished one.
    list.push(job(PROFILE, &session, "j-waiting", "queued", "waiting.pdf", 7_200_000, json!({})));
    fake.reply(&id, json!({"profile_id": PROFILE, "session_id": session, "count": list.len(), "jobs": list}));
    read.await.unwrap().expect("the list read");
    let sec = skill_jobs::section(&conv.store, octoscode_module::screens::board3::ui::now_ms()).expect("a section");
    assert_eq!(sec.rows.len(), skill_jobs::MAX_ROWS);
    assert!(sec.rows.iter().any(|r| r.job_id == "j-waiting"), "an active job is never left out");
    assert_eq!(sec.rows.last().map(|r| r.job_id.as_str()), Some("j-waiting"), "still in newest-first order");
    assert_eq!(sec.omitted, 31 - skill_jobs::MAX_ROWS);
    let dsl = skills_dsl(&conv, 990.0, 603.0);
    assert_eq!(text_of(&dsl, "dlg_skills_jobs_omitted").as_deref(), Some("(+11 omitted)"));
    dialog::close();
}
