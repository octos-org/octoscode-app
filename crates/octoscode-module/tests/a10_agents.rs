//! A10 — the Agents panel's production paths, against RECORDED traffic.
//!
//! Each test runs the SAME functions the panel's clicks run
//! (`board3::agents::{load, read_status, read_output, list_artifacts,
//! read_artifact, control, spawn}` — `board3::host::run` dispatches them)
//! through a fake WS server answering with the committed recordings
//! (`c24b-subagent-a6ea8505.jsonl`: agent/list, agent/status/read,
//! agent/interrupt; the `r1-autonomy` open reply advertising every agent
//! method + `coding.agent_control.v1`) and, for the replies no recording
//! carries, the faithful `a10-agents-faithful.jsonl` frames (built from the
//! web's protocol types). It asserts the frames the server saw (session-scoped,
//! NO `profile_id` — the web's shape; c24 recorded the server refusing a
//! profile-scoped agent read) and what the panel folds.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::Conversation;
use octoscode_module::screens::board3::{agents, host, ui};
use octoscode_store::Store;

struct Frame {
    dir: String,
    method: String,
    body: Value,
}

fn fixture(name: &str) -> Vec<Frame> {
    let path = format!("{}/../octoscode-client/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v.get("body").cloned().unwrap_or(Value::Null),
            }
        })
        .collect()
}

fn recorded(frames: &[Frame], method: &str) -> Vec<Value> {
    frames
        .iter()
        .filter(|f| f.dir == "in" && (f.method == method || f.method == format!("res:{method}")))
        .map(|f| f.body.clone())
        .collect()
}

fn recorded_open() -> Value {
    recorded(&fixture("r1-autonomy-a6ea8505.jsonl"), "session/open")
        .into_iter()
        .find(|b| b.get("active_profile_id").is_some())
        .expect("r1 recorded the open result")
}

/// A fake WS server: each method's canned replies are served IN ORDER (the
/// last one repeating), `{}` otherwise; every (method, params) is logged.
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
                let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
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
    for _ in 0..50 {
        if conv.store.domains.autonomy.bound_session().is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    conv.store.domains.session.set_active(Some(conv.session_id()));
    conv
}

/// The board-3 host state is a process static: one test at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

fn canned() -> Vec<(String, Value)> {
    let c24b = fixture("c24b-subagent-a6ea8505.jsonl");
    let faithful = fixture("a10-agents-faithful.jsonl");
    let mut v: Vec<(String, Value)> = vec![("session/open".into(), json!({"opened": recorded_open()}))];
    for m in ["agent/list", "agent/status/read", "agent/interrupt"] {
        v.push((m.into(), recorded(&c24b, m).remove(0)));
    }
    for f in faithful.iter().filter(|f| f.dir == "in") {
        v.push((f.method.clone(), f.body.clone()));
    }
    v
}

fn lowered(store: &Store) -> String {
    let st = host::state().agents.clone();
    let mut d = ui::Dsl::new();
    agents::build(&mut d, &st, &ui::Frame::DESKTOP, store);
    d.finish()
}

const AGENT: &str = "<redacted-agent>";

/// `/agents` → the roster: the recorded `agent/list` reply folds into the
/// store (session-scoped request, no profile_id), and the panel shows the
/// recorded agent with the web's fields (nickname, role, status, last task).
#[tokio::test]
async fn the_roster_loads_the_recorded_agent_list_without_a_profile_scope() {
    let _s = serial();
    host::reset();
    let server = Server::start(canned()).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    // The click path: `/agents` opens the panel and asks for its load job.
    assert_eq!(host::command("agents", "", &conv), Some(host::Outcome::Spawn(host::Job::AgentsLoad)));
    host::run(host::Job::AgentsLoad, &conv).await.expect("agent/list");
    let sent = server.sent_all("agent/list");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["session_id"], conv.session_id().as_str());
    assert!(sent[0].get("profile_id").is_none(), "the web never scopes agent ops by profile: {}", sent[0]);
    // The recording lists two records whose ids the redaction collapsed to
    // one placeholder (running, then completed): the roster keeps one row
    // per agent id, the later record winning.
    let roster = store.domains.autonomy.agents();
    assert_eq!(roster.len(), 1);
    assert_eq!(roster[0].nickname, "c24b-probe");
    assert_eq!(roster[0].status, "completed");
    assert_eq!(roster[0].last_task.as_deref(), Some("c24b-probe completed"));
    let dsl = lowered(&store);
    for want in ["c24b-probe", "background_task", "completed", "c24b-probe completed", "Status: SUCCESS", "Read status", "Interrupt agent", "Close agent"] {
        assert!(dsl.contains(want), "the roster shows {want:?}");
    }
    assert!(!dsl.contains(agents::EMPTY));
    // A terminal row offers no interrupt/close (drawn disabled, not wired).
    let taps: Vec<String> = octoscode_module::screens::taps::wired_taps(&dsl).into_iter().map(|(_, e)| e).collect();
    assert!(taps.iter().any(|t| t == "b3.agents.status#0"), "{taps:?}");
    assert!(!taps.iter().any(|t| t == "b3.agents.interrupt#0" || t == "b3.agents.close#0"), "{taps:?}");
}

/// The recorded `agent/updated` (c24b line 142: the agent while running).
fn recorded_running_agent() -> octoscode_store::domains::autonomy::AgentRecord {
    let body = recorded(&fixture("c24b-subagent-a6ea8505.jsonl"), "agent/updated").remove(0);
    let rec: octos_core::ui_protocol::UiAgentRecord = serde_json::from_value(body["agent"].clone()).expect("agent");
    octoscode_client::domains::autonomy::agent_record(&rec)
}

/// Read status / Read output (+ Load more) / List artifacts / Read artifact:
/// each request carries the agent and session only; status, artifacts and
/// the artifact share ONE detail viewer (a new read replaces the old), the
/// output appends on Load more from the server's cursor, and the artifact's
/// markup is shown as text.
#[tokio::test]
async fn the_detail_and_output_viewers_fold_the_replies() {
    let _s = serial();
    host::reset();
    let server = Server::start(canned()).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    host::run(host::Job::AgentsLoad, &conv).await.expect("list");

    // Row 0 → Read status (the click's route: perform → the job).
    let out = host::perform("b3.agents.status", 0, &store);
    assert_eq!(out, host::Outcome::Spawn(host::Job::AgentStatus(AGENT.into())));
    host::run(host::Job::AgentStatus(AGENT.into()), &conv).await.expect("status");
    let p = &server.sent_all("agent/status/read")[0];
    assert_eq!(p, &json!({"agent_id": AGENT, "session_id": conv.session_id()}));
    let v = store.domains.autonomy.agent_viewer();
    assert_eq!(v.status.as_ref().map(|s| s.backend_kind.as_str()), Some("spawn_child_session"));
    assert!(lowered(&store).contains(&format!("Status — {AGENT}")));

    // List artifacts replaces the status in the one detail viewer.
    host::run(host::Job::AgentArtifacts(AGENT.into()), &conv).await.expect("artifacts");
    let v = store.domains.autonomy.agent_viewer();
    assert!(v.status.is_none(), "one detail viewer: the status card is replaced");
    let (agent, rows) = v.artifacts.clone().expect("artifacts");
    assert_eq!((agent.as_str(), rows.len(), rows[0].id.as_str()), (AGENT, 1, "artifact-1"));
    assert!(lowered(&store).contains("Result — text · ready"));
    // Read artifact (row 0, by id): exactly one selector on the wire.
    assert_eq!(
        host::perform("b3.agents.artifact", 0, &store),
        host::Outcome::Spawn(host::Job::AgentArtifactRead(AGENT.into(), Some("artifact-1".into()), None))
    );
    host::run(host::Job::AgentArtifactRead(AGENT.into(), Some("artifact-1".into()), None), &conv)
        .await
        .expect("artifact read");
    let p = &server.sent_all("agent/artifact/read")[0];
    assert_eq!(p["artifact_id"], "artifact-1");
    assert!(p.get("path").is_none() && p.get("profile_id").is_none(), "{p}");
    let dsl = lowered(&store);
    assert!(dsl.contains(&format!("Artifact — {AGENT} / artifact-1")));
    assert!(dsl.contains("<script>not executable</script>"), "content is shown as inert text");

    // Read output, then Load more from the server's cursor (appends).
    host::run(host::Job::AgentOutput(AGENT.into(), false), &conv).await.expect("output");
    let v = store.domains.autonomy.agent_viewer();
    let o = v.output.clone().expect("output");
    assert!(o.has_more && o.text.contains("12 checks queued"));
    assert!(lowered(&store).contains("Load more"));
    assert_eq!(host::perform("b3.agents.output_more", 0, &store), host::Outcome::Spawn(host::Job::AgentOutput(AGENT.into(), true)));
    host::run(host::Job::AgentOutput(AGENT.into(), true), &conv).await.expect("more");
    let sent = server.sent_all("agent/output/read");
    assert!(sent[0].get("cursor").is_none(), "a fresh read has no cursor: {}", sent[0]);
    assert_eq!(sent[1]["cursor"], json!({"offset": 61}), "load more continues from next_cursor");
    assert!(sent.iter().all(|p| p.get("limit").is_none() && p.get("profile_id").is_none()));
    let o = store.domains.autonomy.agent_viewer().output.expect("output");
    assert!(o.text.contains("12 checks queued") && o.text.ends_with("12 passed, 0 failed\n"), "appended: {:?}", o.text);
    assert!(!o.has_more);
    assert!(!lowered(&store).contains("Load more"), "no more output");
}

/// Interrupt (recorded c24b receipt) and Close (faithful receipt): the
/// receipt's status updates the roster row, the activity line names it, a
/// terminal agent offers no further control, and a second click on a pending
/// agent sends nothing.
#[tokio::test]
async fn interrupt_and_close_apply_the_receipt_and_guard_double_submission() {
    let _s = serial();
    host::reset();
    let server = Server::start(canned()).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    host::run(host::Job::AgentsLoad, &conv).await.expect("list");
    // The recorded `agent/updated` while running puts the row back in a
    // controllable state.
    store.domains.autonomy.upsert_agent(recorded_running_agent());
    assert_eq!(
        host::perform("b3.agents.interrupt", 0, &store),
        host::Outcome::Spawn(host::Job::AgentControl(AGENT.into(), "interrupt"))
    );
    // A second activation while the first is in flight is refused locally.
    let epoch = store.domains.autonomy.begin_agent_control(AGENT).expect("first");
    assert_eq!(host::perform("b3.agents.interrupt", 0, &store), host::Outcome::Done);
    // (a refusal releases the pending mark without applying a status)
    assert!(!store.domains.autonomy.finish_agent_control(AGENT, epoch, None));
    assert!(store.domains.autonomy.agent_viewer().pending.is_empty());
    host::run(host::Job::AgentControl(AGENT.into(), "interrupt"), &conv).await.expect("interrupt");
    assert_eq!(server.sent_all("agent/interrupt").len(), 1, "exactly one frame");
    assert_eq!(store.domains.autonomy.agents()[0].status, "interrupted");
    let v = store.domains.autonomy.agent_viewer();
    assert_eq!(v.activity.as_deref(), Some(format!("Agent {AGENT} interrupted").as_str()));
    // Terminal now: neither control routes.
    assert_eq!(host::perform("b3.agents.close", 0, &store), host::Outcome::Done);
    // The by-ID path still closes (the web passes terminal=false there).
    host::state().agents.query_id = AGENT.into();
    assert_eq!(
        host::perform("b3.agents.id.close", 0, &store),
        host::Outcome::Spawn(host::Job::AgentControl(AGENT.into(), "close"))
    );
    host::run(host::Job::AgentControl(AGENT.into(), "close"), &conv).await.expect("close");
    assert_eq!(server.sent_all("agent/close")[0], json!({"agent_id": AGENT, "session_id": conv.session_id()}));
    assert_eq!(store.domains.autonomy.agents()[0].status, "closed");
}

/// A detail read captured BEFORE an applied control cannot repaint stale
/// details (the `agents` revision moved), and a newer detail read supersedes
/// an older one in flight (latest wins) — the web store's single-viewer
/// rules (`store.test.ts` "one viewer rejects a late A status…").
#[tokio::test]
async fn a_superseded_detail_read_never_repaints() {
    let _s = serial();
    host::reset();
    let server = Server::start(canned()).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    let a = &store.domains.autonomy;
    let first = a.begin_agent_detail();
    let second = a.begin_agent_detail();
    assert!(a.finish_agent_detail(second, |v| v.artifacts = Some(("a2".into(), vec![]))));
    assert!(!a.finish_agent_detail(first, |v| v.status = None), "the late A status is dropped");
    assert_eq!(a.agent_viewer().artifacts.as_ref().map(|(x, _)| x.as_str()), Some("a2"));
    // A control applied while a status read is in flight drops that read.
    host::run(host::Job::AgentsLoad, &conv).await.expect("list");
    let t = a.begin_agent_detail();
    let e = a.begin_agent_control(AGENT).unwrap();
    assert!(a.finish_agent_control(AGENT, e, Some("closed")));
    assert!(!a.finish_agent_detail(t, |_| {}), "a read captured before the close cannot restore details");
    // A failure is recorded only while authorized, and shown as the alert.
    assert!(a.record_error(agents::FAMILY, a.epoch(), "agent/status/read: boom"));
    assert!(lowered(&store).contains("agent/status/read: boom"));
    assert!(!a.record_error(agents::FAMILY, a.epoch() + 7, "late"), "a superseded epoch never records");
    let _ = server;
}

/// "Request parallel agents": the web's composed text rides an ORDINARY
/// `turn/start` (never an agent RPC); a non-idle session is refused with the
/// web's copy and sends nothing.
#[tokio::test]
async fn the_spawn_is_an_idle_only_ordinary_turn() {
    let _s = serial();
    host::reset();
    let server = Server::start(canned()).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    {
        let mut st = host::state();
        st.agents.count = "2".into();
        st.agents.task = "audit the parser".into();
    }
    let out = host::perform("b3.agents.spawn", 0, &store);
    let text = "Spawn 2 agent(s) to accomplish in parallel: audit the parser".to_owned();
    assert_eq!(out, host::Outcome::Spawn(host::Job::AgentsSpawn(text.clone())));
    // Busy: a turn is running → refused, nothing sent.
    conv.ui().lock().unwrap().begin_turn_now("t-busy");
    assert!(host::run(host::Job::AgentsSpawn(text.clone()), &conv).await.is_err());
    assert_eq!(host::state().agents.spawn_error.as_deref(), Some(agents::SPAWN_REFUSED));
    assert!(server.sent_all("turn/start").is_empty(), "a refused spawn sends nothing");
    conv.ui().lock().unwrap().end_turn_now(true);
    store.set_connection("Live".into(), true);
    host::run(host::Job::AgentsSpawn(text.clone()), &conv).await.expect("queued");
    let sent = server.sent_all("turn/start");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["input"], json!([{"kind": "text", "text": text}]));
    assert!(host::state().agents.task.is_empty(), "the task clears; the count is kept");
    assert_eq!(host::state().agents.count, "2");
}
