//! A10 — the Fleet's production paths at the wire, against the FAITHFUL
//! external-driver fixture (`crates/octoscode-client/tests/fixtures/
//! a10-fleet-driver-synthetic.jsonl`, built by `tools/fixtures/
//! a10_fleet_fixture.py` from the r6/r23/c24 recordings and the web's
//! protocol — no live recording exists for `peer/dispatch`, `peer/control`,
//! `session/driver/*` or `external_driver_v1`).
//!
//! Each test drives the SAME functions the Fleet pane's clicks run —
//! `board3::host::perform` (the tap through the shared `taps` path) and
//! `board3::host::run` (the job the host spawns) — while the transport's
//! events drain through `Conversation::on_event` + the peer manager's hook,
//! exactly like `lib.rs`'s drain loop. The fake server answers each request
//! from the fixture's bodies, echoing the per-request ids the web's own
//! fixture server echoes (operation id, requested lane, acquiring driver,
//! control target), and pushes the adopted peer session's recorded frames
//! after the background attach.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::Conversation;
use octoscode_module::screens::board3::fleetview::{self, StartState, Status};
use octoscode_module::screens::board3::host::{self, Job, Outcome};
use octoscode_module::screens::{fleet_driver, peers};

const SESSION: &str = "dsflash:main";
const PEER: &str = "dsflash:main#peer-fleet-review";
const ADOPTED_TURN: &str = "00000000-0000-4000-8000-0000000000d1";

// ------------------------------------------------------------- fixture

fn fixture() -> Vec<(String, String, Value)> {
    let path = format!(
        "{}/../octoscode-client/tests/fixtures/a10-fleet-driver-synthetic.jsonl",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).expect("json line");
            (
                v["dir"].as_str().unwrap_or("").to_owned(),
                v["method"].as_str().unwrap_or("").to_owned(),
                v["body"].clone(),
            )
        })
        .collect()
}

fn body(method: &str) -> Value {
    fixture()
        .into_iter()
        .find(|(d, m, _)| d == "in" && m == method)
        .map(|(_, _, b)| b)
        .unwrap_or_else(|| panic!("fixture has no {method}"))
}

// -------------------------------------------------------------- server

#[derive(Default)]
struct Knobs {
    /// Refuse every dispatch with this typed kind.
    refuse_dispatch: Option<String>,
    /// Fail every dispatch with a KIND-LESS error (an uncertain outcome).
    unknown_dispatch: bool,
    /// Advertise the lane source.
    no_lanes_method: bool,
}

struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
    knobs: Arc<Mutex<Knobs>>,
}

impl Server {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let knobs = Arc::new(Mutex::new(Knobs::default()));
        let (log, kn) = (seen.clone(), knobs.clone());
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (mut tx, mut rx) = ws.split();
            let mut dispatched: Vec<String> = Vec::new();
            while let Some(Ok(msg)) = rx.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                let Some(id) = v.get("id").cloned() else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let p = v["params"].clone();
                log.lock().unwrap().push((method.clone(), p.clone()));
                let mut pushes: Vec<(String, Value)> = Vec::new();
                let reply: Result<Value, Value> = match method.as_str() {
                    "session/open" => {
                        let mut opened = body("session/open");
                        if kn.lock().unwrap().no_lanes_method {
                            let m = opened["capabilities"]["supported_methods"].as_array_mut().unwrap();
                            m.retain(|x| x != "profile/sub_providers/list");
                        }
                        let sid = p["session_id"].as_str().unwrap_or(SESSION).to_owned();
                        opened["session_id"] = json!(sid);
                        if sid.contains("#peer-") {
                            // The background attach: the adopted session's
                            // own recorded frames follow.
                            pushes.push(("turn/started".into(), body("turn/started")));
                            pushes.push(("approval/requested".into(), body("approval/requested")));
                        }
                        Ok(json!({ "opened": opened }))
                    }
                    "session/list" => Ok(json!({ "sessions": [] })),
                    "session/driver/get" => Ok(body("session/driver/get")),
                    "profile/sub_providers/list" => Ok(body("profile/sub_providers/list")),
                    "session/driver/acquire" => {
                        let mut a = body("session/driver/acquire");
                        a["binding"]["driver_id"] = p["driver_id"].clone();
                        Ok(a)
                    }
                    "session/driver/release" => Ok(json!({
                        "mode": "external", "recovery": "none",
                        "binding": {"driver_id": p["driver_id"], "epoch": 7, "revision": 42, "lease_expires_at_ms": 0}
                    })),
                    "peer/prepare" => Ok(body("peer/prepare")),
                    "peer/dispatch" => {
                        let k = kn.lock().unwrap();
                        let lanes = ["lane-primary", "lane-review"];
                        if let Some(kind) = &k.refuse_dispatch {
                            Err(json!({"code": -32602, "message": "driver operation refused: peer/dispatch", "data": {"kind": kind}}))
                        } else if k.unknown_dispatch {
                            Err(json!({"code": -32603, "message": "internal"}))
                        } else if !lanes.contains(&p["model"].as_str().unwrap_or("")) {
                            Err(body("err:peer/dispatch"))
                        } else {
                            let op = p["operation_id"].as_str().unwrap_or("").to_owned();
                            let duplicate = dispatched.contains(&op);
                            dispatched.push(op.clone());
                            let mut r = body("peer/dispatch");
                            r["operation_id"] = json!(op);
                            r["model_lane"] = p["model"].clone();
                            r["duplicate"] = json!(duplicate);
                            Ok(r)
                        }
                    }
                    "peer/control" => {
                        let mut r = body("peer/control");
                        r["operation_id"] = p["operation_id"].clone();
                        r["target_operation_id"] = p["target_operation_id"].clone();
                        r["expected_turn_id"] = p["expected_turn_id"].clone();
                        match p["command"]["kind"].as_str() {
                            Some("approval_respond") => pushes.push(("approval/decided".into(), body("approval/decided"))),
                            Some("interrupt") => pushes.push(("turn/error".into(), body("turn/error"))),
                            _ => {}
                        }
                        Ok(r)
                    }
                    _ => Ok(json!({})),
                };
                let frame = match reply {
                    Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
                    Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
                };
                let _ = tx.send(Message::Text(frame.to_string().into())).await;
                for (m, params) in pushes {
                    let n = json!({"jsonrpc": "2.0", "method": m, "params": params});
                    let _ = tx.send(Message::Text(n.to_string().into())).await;
                }
            }
        });
        Self { base_url: format!("http://{addr}"), seen, knobs }
    }

    fn sent(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    fn methods(&self) -> Vec<String> {
        self.seen.lock().unwrap().iter().map(|(m, _)| m.clone()).collect()
    }
}

/// Connect, open, and DRAIN the transport like `lib.rs`: the peer manager's
/// hook first, then `on_event`.
async fn connect(server: &Server) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    let conv = Arc::new(conv);
    let drv = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            peers::note_transport_event(&drv, &evt);
            let _ = drv.on_event(evt);
        }
    });
    conv.open_workspace(None).await.expect("session/open");
    for _ in 0..100 {
        if conv.store.domains.config.supported_methods().iter().any(|m| m == "peer/dispatch") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    conv
}

async fn wait_until(mut f: impl FnMut() -> bool) -> bool {
    for _ in 0..200 {
        if f() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

fn spawn_of(o: Outcome) -> Job {
    match o {
        Outcome::Spawn(j) => j,
        other => panic!("expected a job, got {other:?}"),
    }
}

/// The board-3 state + the held seat are process statics: one test at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

fn fresh() {
    host::reset();
    fleet_driver::reset_seat();
    std::env::set_var("OCTOSCODE_DRIVER_ID_PATH", std::env::temp_dir().join("a10-fleet-driver-id"));
}

/// Open the Fleet (the sidebar footer tap) and run its opening reads.
async fn open_fleet(conv: &Conversation) {
    let job = spawn_of(host::perform("b3.open.fleet", 0, &conv.store));
    assert_eq!(job, Job::FleetLanes);
    host::run(job, conv).await.expect("the opening reads");
}

// ---------------------------------------------------------------- tests

/// Opening the Fleet reads the lanes (`profile/sub_providers/list`) and
/// WALKS the driver inventory (`session/driver/get` with an operations page);
/// the picker starts with NO lane; the inventory's accepted dispatch is a
/// union row labelled `Peer 1 · <resolved model>`, grouped by its goal.
#[tokio::test]
async fn opening_reads_the_lanes_and_walks_the_inventory_without_a_default_lane() {
    let _s = serial();
    fresh();
    let server = Server::start().await;
    let conv = connect(&server).await;
    open_fleet(&conv).await;
    assert_eq!(server.sent("profile/sub_providers/list")[0], json!({"profile_id": "dsflash"}));
    assert_eq!(server.sent("session/driver/get")[0], json!({"session_id": SESSION, "operations": {"limit": 50}}));
    assert!(fleet_driver::control_ready(&conv.store), "advertised + a complete walk");
    {
        let st = host::state();
        assert_eq!(st.fleet.lane_keys(), ["lane-primary", "lane-review"]);
        assert_eq!(st.fleet.chosen_lane(), None, "no implicit default lane");
    }
    let rows = fleetview::rows(&conv.store, peers::now_ms());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].label, "Peer 1 · glm-4.6", "the resolved model, never the slug");
    assert_eq!(rows[0].status, Status::Requested, "an inventory-only row");
    assert_eq!(rows[0].goal_id.as_deref(), Some("goal_01"));
    // The lowered pane names the goal group and offers the Advanced seat.
    let lowered = host::lower_open(&conv.store).expect("the Fleet pane lowers");
    assert!(lowered.dsl.contains("Goal goal_01") && lowered.dsl.contains("\"Advanced\""));
    assert!(!lowered.dsl.contains("lint-sweep"), "no slug in the product rows");
}

/// Start = acquire (CAS on the walked revision, the stable driver id, lease
/// 120) → prepare → EXACTLY ONE `peer/dispatch` carrying the Start's own
/// operation id → the adopted row → the background attach of the adopted
/// session. A SECOND Start mints a DISTINCT operation id.
#[tokio::test]
async fn start_acquires_with_cas_then_dispatches_exactly_once_and_adopts_the_row() {
    let _s = serial();
    fresh();
    let server = Server::start().await;
    let conv = connect(&server).await;
    open_fleet(&conv).await;
    // Not admitted without a lane (zero frames).
    host::input_changed("fleet.brief", "Review the reconnect diff");
    assert_eq!(host::perform("b3.fleet.start", 0, &conv.store), Outcome::Done);
    assert!(server.sent("peer/dispatch").is_empty());
    assert_eq!(host::perform("b3.fleet.lane", 0, &conv.store), Outcome::Done);
    let job = spawn_of(host::perform("b3.fleet.start", 0, &conv.store));
    let Job::FleetStart { operation_id, lane, brief } = job.clone() else { panic!("{job:?}") };
    assert_eq!((lane.as_str(), brief.as_str()), ("lane-primary", "Review the reconnect diff"));
    assert!(host::state().fleet.start.requesting(), "Starting… while in flight");
    assert_eq!(host::perform("b3.fleet.start", 0, &conv.store), Outcome::Done, "no repeat while requesting");
    host::run(job, &conv).await.expect("start");
    // The wire order and shapes.
    let order: Vec<String> = server
        .methods()
        .into_iter()
        .filter(|m| matches!(m.as_str(), "session/driver/acquire" | "peer/prepare" | "peer/dispatch"))
        .collect();
    assert_eq!(order, ["session/driver/acquire", "peer/prepare", "peer/dispatch"]);
    let acquire = &server.sent("session/driver/acquire")[0];
    assert_eq!(acquire["expected_revision"], json!(42), "CAS on the walked revision");
    assert_eq!(acquire["lease_seconds"], json!(120));
    assert!(acquire["driver_id"].as_str().unwrap().starts_with("octoscode-native:"), "{acquire}");
    assert_eq!(
        server.sent("peer/prepare")[0],
        json!({"brief": "Review the reconnect diff", "title": "Review the reconnect diff", "session_id": SESSION, "profile_id": "dsflash"})
    );
    let dispatches = server.sent("peer/dispatch");
    assert_eq!(dispatches.len(), 1, "EXACTLY ONE dispatch");
    let d = &dispatches[0];
    assert_eq!(d["operation_id"], json!(operation_id), "the Start's own operation id");
    assert_eq!(d["model"], json!("lane-primary"), "the REQUESTED lane");
    assert_eq!(d["epoch"], json!(7));
    assert_eq!(d["control_token"], json!("synthetic-control-token"));
    assert_eq!(d["driver_id"], acquire["driver_id"]);
    assert_eq!(d["dispatch"], json!({"kind": "new_brief", "brief": "Review the reconnect diff", "title": "fleet-review"}));
    let kickoff = d["kickoff_input"][0]["text"].as_str().unwrap();
    assert!(kickoff.starts_with("You are a peer agent. Your brief:\n\nReview the reconnect diff"), "{kickoff}");
    assert!(kickoff.contains("/peers/fleet-review/brief.md"));
    // The attach of the ADOPTED session (never the active one).
    assert!(wait_until(|| server.sent("session/open").iter().any(|p| p["session_id"] == PEER)).await);
    assert_eq!(conv.store.active_session().as_deref(), Some(SESSION), "the active session is untouched");
    let row = conv.store.domains.peer.row(PEER).expect("the adopted row");
    assert_eq!(row.operation_id.as_deref(), Some(operation_id.as_str()));
    assert_eq!(row.turn_id, ADOPTED_TURN);
    assert_eq!(row.model.as_deref(), Some("gpt-5.4"));
    {
        let st = host::state();
        assert_eq!(st.fleet.start, StartState::Idle, "an accepted settle clears the form");
        assert!(st.fleet.brief.is_empty());
    }
    // A second Start mints a DISTINCT operation id.
    host::input_changed("fleet.brief", "Another review");
    host::perform("b3.fleet.lane", 0, &conv.store);
    let Job::FleetStart { operation_id: second, .. } = spawn_of(host::perform("b3.fleet.start", 0, &conv.store)) else {
        panic!()
    };
    assert_ne!(second, operation_id);
}

/// The adopted peer session's OWN frames fold into its row (Working →
/// Waiting for your approval), never into the master's UI; each row action
/// sends EXACTLY ONE `peer/control` frame with the held fence, the accepted
/// operation id, the ADOPTED turn and a fresh operation id; the decided
/// approval returns the row to Working; Stop's `turn/error interrupted`
/// reads Stopped and is announced once.
#[tokio::test]
async fn peer_frames_fold_into_the_row_and_each_action_sends_one_control_frame() {
    let _s = serial();
    fresh();
    let server = Server::start().await;
    let conv = connect(&server).await;
    open_fleet(&conv).await;
    host::input_changed("fleet.brief", "Review the reconnect diff");
    host::perform("b3.fleet.lane", 0, &conv.store);
    let job = spawn_of(host::perform("b3.fleet.start", 0, &conv.store));
    host::run(job, &conv).await.expect("start");
    let waiting = || {
        fleetview::rows(&conv.store, peers::now_ms())
            .iter()
            .any(|r| r.key == PEER && r.status == Status::WaitingApproval)
    };
    assert!(wait_until(waiting).await, "turn/started + approval/requested folded");
    assert_eq!(conv.ui().lock().unwrap().active_turn(), None, "the master's UI never saw the peer's turn");
    // The rows the pane draws (the `#row` index the taps carry).
    host::lower_open(&conv.store).expect("lowers");
    let idx = host::state().fleet.drawn.iter().position(|r| r.key == PEER).expect("drawn");
    let lowered = host::lower_open(&conv.store).unwrap();
    assert!(lowered.dsl.contains(&format!("b3.fleet.approve#{idx}")), "Approve while waiting");
    assert!(lowered.dsl.contains("Waiting for your approval"));
    // Approve → ONE control frame.
    let job = spawn_of(host::perform("b3.fleet.approve", idx, &conv.store));
    host::run(job, &conv).await.expect("approve");
    let c = server.sent("peer/control");
    assert_eq!(c.len(), 1);
    let row = conv.store.domains.peer.row(PEER).unwrap();
    assert_eq!(c[0]["command"]["kind"], json!("approval_respond"));
    assert_eq!(c[0]["command"]["decision"], json!("approve"));
    assert_eq!(c[0]["command"]["approval_id"], json!("01a0eb92-9444-7101-aa6f-10065886f57e"), "the row's REAL pending id");
    assert_eq!(c[0]["target_operation_id"], json!(row.operation_id.clone().unwrap()));
    assert_eq!(c[0]["expected_turn_id"], json!(ADOPTED_TURN));
    assert_ne!(c[0]["operation_id"], c[0]["target_operation_id"], "a fresh control operation id");
    assert_eq!(c[0]["session_id"], json!(SESSION));
    let working = || {
        fleetview::rows(&conv.store, peers::now_ms()).iter().any(|r| r.key == PEER && r.status == Status::Working)
    };
    assert!(wait_until(working).await, "approval/decided returns the row to Working");
    // Steer with the row's OWN text → ONE steer frame.
    host::lower_open(&conv.store).unwrap();
    host::input_changed(&format!("fleet.steer#{idx}"), "Focus on the reconnect tests");
    let job = spawn_of(host::perform("b3.fleet.steer", idx, &conv.store));
    host::run(job, &conv).await.expect("steer");
    let c = server.sent("peer/control");
    assert_eq!(c.len(), 2);
    assert_eq!(c[1]["command"], json!({"kind": "steer", "input": [{"kind": "text", "text": "Focus on the reconnect tests"}]}));
    assert_eq!(host::state().fleet.row_note.get(PEER).map(String::as_str), Some("Sent"), "an acknowledgment, not an outcome");
    // Stop → ONE interrupt; the peer's turn/error interrupted reads Stopped.
    let job = spawn_of(host::perform("b3.fleet.stop", idx, &conv.store));
    host::run(job, &conv).await.expect("stop");
    let c = server.sent("peer/control");
    assert_eq!(c.len(), 3);
    assert_eq!(c[2]["command"], json!({"kind": "interrupt"}));
    let stopped = || {
        fleetview::rows(&conv.store, peers::now_ms()).iter().any(|r| r.key == PEER && r.status == Status::Stopped)
    };
    assert!(wait_until(stopped).await);
    host::lower_open(&conv.store).unwrap();
    let note = host::state().fleet.announcement.clone().unwrap_or_default();
    assert!(note.ends_with("stopped"), "announced: {note}");
    // Terminal rows offer no affordance: nothing more can be sent.
    let idx = host::state().fleet.drawn.iter().position(|r| r.key == PEER).unwrap();
    assert_eq!(host::perform("b3.fleet.stop", idx, &conv.store), Outcome::Done);
    assert_eq!(server.sent("peer/control").len(), 3, "never retried, never repeated");
}

/// A typed refusal keeps the brief and shows the BOUNDED label (never server
/// copy); an uncertain outcome offers Retry with the SAME operation id; a
/// lane outside the advertised keys never reaches the wire.
#[tokio::test]
async fn refusals_keep_the_brief_and_retry_reuses_the_operation_id() {
    let _s = serial();
    fresh();
    let server = Server::start().await;
    let conv = connect(&server).await;
    open_fleet(&conv).await;
    server.knobs.lock().unwrap().refuse_dispatch = Some("driver_fence_stale".into());
    host::input_changed("fleet.brief", "Review the reconnect diff");
    host::perform("b3.fleet.lane", 1, &conv.store);
    let job = spawn_of(host::perform("b3.fleet.start", 0, &conv.store));
    host::run(job, &conv).await.expect("start settles");
    {
        let st = host::state();
        assert!(matches!(&st.fleet.start, StartState::Failed { kind, .. } if kind == "driver_fence_stale"));
        assert_eq!(st.fleet.brief, "Review the reconnect diff", "the brief is kept for retry");
    }
    let dsl = host::lower_open(&conv.store).unwrap().dsl;
    assert!(dsl.contains("Couldn't start: Your control of this session expired"), "the bounded label");
    assert!(!dsl.contains("driver operation refused"), "never server copy");
    // An uncertain (kind-less) outcome → Retry resends the SAME id.
    {
        let mut k = server.knobs.lock().unwrap();
        k.refuse_dispatch = None;
        k.unknown_dispatch = true;
    }
    host::input_changed("fleet.brief", "Review the reconnect diff");
    let job = spawn_of(host::perform("b3.fleet.start", 0, &conv.store));
    let Job::FleetStart { operation_id, .. } = job.clone() else { panic!() };
    host::run(job, &conv).await.expect("start settles");
    assert!(matches!(host::state().fleet.start, StartState::Unknown { .. }));
    assert!(host::lower_open(&conv.store).unwrap().dsl.contains("Not sure it started — Retry resends the same request."));
    server.knobs.lock().unwrap().unknown_dispatch = false;
    let before_prepare = server.sent("peer/prepare").len();
    let job = spawn_of(host::perform("b3.fleet.retry", 0, &conv.store));
    host::run(job, &conv).await.expect("retry");
    let last = server.sent("peer/dispatch").last().cloned().unwrap();
    assert_eq!(last["operation_id"], json!(operation_id), "Retry reuses the SAME operation id");
    assert_eq!(server.sent("peer/prepare").len(), before_prepare, "the retry re-stages nothing");
    // A lane the profile does not advertise is unconstructible: zero frames.
    let before = server.sent("peer/dispatch").len();
    host::state().fleet.lane = "lane-ghost".into();
    host::input_changed("fleet.brief", "x");
    assert_eq!(host::perform("b3.fleet.start", 0, &conv.store), Outcome::Done);
    assert_eq!(server.sent("peer/dispatch").len(), before);
    assert!(matches!(
        fleet_driver::start(&conv, "op-x", "lane-ghost", &["lane-primary".into()], "b", &mut None).await,
        fleet_driver::StartOutcome::Refused { ref kind } if kind == "driver_model_unavailable"
    ));
    assert_eq!(server.sent("peer/dispatch").len(), before, "the local lane refusal sends nothing");
}

/// The lane source is fail-closed: an unadvertised `profile/sub_providers/
/// list` is never read and the picker stays disabled.
#[tokio::test]
async fn an_unadvertised_lane_source_is_never_read() {
    let _s = serial();
    fresh();
    let server = Server::start().await;
    server.knobs.lock().unwrap().no_lanes_method = true;
    let conv = connect(&server).await;
    open_fleet(&conv).await;
    assert!(server.sent("profile/sub_providers/list").is_empty(), "no read");
    assert!(host::state().fleet.lane_keys().is_empty());
    assert_eq!(host::perform("b3.fleet.lane.toggle", 0, &conv.store), Outcome::Done);
    assert!(!host::state().fleet.picker_open, "a disabled picker never opens");
}

/// Advanced: the driver disclosure (External controller, its facts), the
/// console's explicit Release (next external) and Acquire seat, its Dispatch
/// with the operator's OWN title (held seat only, no implicit acquire), and
/// the control seat (pending work + the master's live turn) sending ONE
/// frame per command.
#[tokio::test]
async fn the_session_controller_discloses_releases_reacquires_and_dispatches() {
    let _s = serial();
    fresh();
    let server = Server::start().await;
    let conv = connect(&server).await;
    open_fleet(&conv).await;
    assert_eq!(host::perform("b3.fleet.advanced", 0, &conv.store), Outcome::Done);
    assert_eq!(host::perform("b3.fleet.console.disclosure", 0, &conv.store), Outcome::Done);
    let dsl = host::lower_open(&conv.store).unwrap().dsl;
    for want in ["External controller", "No recovery pending", "Lease expires", "Peer controller", "Model lane"] {
        assert!(dsl.contains(want), "{want}");
    }
    // The console never acquires on its own: Dispatch needs a held seat.
    host::input_changed("fleet.console.brief", "Console brief");
    host::perform("b3.fleet.console.lane", 1, &conv.store);
    assert_eq!(host::perform("b3.fleet.console.dispatch", 0, &conv.store), Outcome::Done);
    assert!(server.sent("session/driver/acquire").is_empty());
    // Acquire via a Start, then Release (park) and re-Acquire.
    fleet_driver::acquire_seat(&conv).await.expect("acquire");
    let job = spawn_of(host::perform("b3.fleet.console.release", 0, &conv.store));
    host::run(job, &conv).await.expect("release");
    assert_eq!(server.sent("session/driver/release")[0]["next"], json!("external"));
    assert!(fleet_driver::seat_parked(SESSION));
    assert!(host::lower_open(&conv.store).unwrap().dsl.contains("Acquire seat"));
    let job = spawn_of(host::perform("b3.fleet.console.acquire", 0, &conv.store));
    host::run(job, &conv).await.expect("re-acquire");
    assert!(fleet_driver::seat_held(SESSION));
    // The console's Dispatch carries the operator's title.
    host::input_changed("fleet.console.title", "console-title");
    let job = spawn_of(host::perform("b3.fleet.console.dispatch", 0, &conv.store));
    host::run(job, &conv).await.expect("console dispatch");
    assert_eq!(server.sent("peer/prepare").last().unwrap()["title"], json!("console-title"));
    assert_eq!(server.sent("peer/dispatch").last().unwrap()["model"], json!("lane-review"));
    // The control seat: pending work[0] + the master's live turn.
    host::set_live_turn(Some("00000000-0000-4000-8000-0000000000c1".into()));
    assert!(host::lower_open(&conv.store).unwrap().dsl.contains("Respond to approval"));
    let before = server.sent("peer/control").len();
    let job = spawn_of(host::perform("b3.fleet.console.seat", 3, &conv.store));
    host::run(job, &conv).await.ok();
    let c = server.sent("peer/control");
    assert_eq!(c.len(), before + 1, "ONE frame per command");
    assert_eq!(c.last().unwrap()["target_operation_id"], json!("synthetic-pending-op"));
    assert_eq!(c.last().unwrap()["command"], json!({"kind": "interrupt"}));
}
