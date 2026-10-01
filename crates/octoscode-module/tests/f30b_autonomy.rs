//! #30b — Stage C wiring for board 3.3/3.4/3.5, replayed over recorded real
//! traffic (`crates/octoscode-client/tests/fixtures/r1-autonomy-a6ea8505.jsonl`
//! carries `session/goal/*`, `loop/*`, `monitor/*`; `c24b-subagent-a6ea8505.jsonl`
//! carries `monitor/create`). The fake server answers the read RPCs with the
//! objects the RECORDING's notifications carried (goal_01 "r1 replay probe"
//! active with a 100000000 budget; loop_01 fixed_interval 3600; monitor_01
//! poll/ERROR) — hermetic: fixture values only, no environment (LESSONS).
//!
//! §1 goal refresh + bindings project the recorded goal.
//! §2 goal.set parses "objective | budget" strictly and sends the recorded
//!     param shape (`transition_actor: "user"`; blank budget omitted).
//! §3 goal.pause is the two-step TUI transition: fresh goal/get, then
//!     goal/set status=paused (store.ts:545-560).
//! §4 generation admission: a stale result cannot regress a newer stamp.
//! §5 loop rows route per-action with the recorded loop_01 id.
//! §6 monitor.create parses "name | argv json | filter" into the recorded
//!     body shape; a non-array argv never reaches the wire.
//! §7 the family gates fail closed without the advertised methods.
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, FlowUi};
use octoscode_module::screens::autonomy as au;
use octoscode_store::Store;

/// A recorded `dir`/`method`/`body` line.
#[derive(Debug, Clone)]
struct Frame {
    dir: String,
    method: String,
    body: serde_json::Value,
}

fn load(path: &str) -> Vec<Frame> {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../octoscode-client/tests/fixtures");
    let text = std::fs::read_to_string(base.join(path)).expect("read fixture");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v.get("body").cloned().unwrap_or(serde_json::Value::Null),
            }
        })
        .collect()
}

fn r1() -> Vec<Frame> {
    load("r1-autonomy-a6ea8505.jsonl")
}

/// The recorded notification body for `method` whose value satisfies `pred` —
/// a recording carries the SAME method several times over a run (r1 streams
/// monitor/updated active -> paused -> active -> deleted), so a test that
/// wants the paused frame must select it rather than take the first.
fn notification_where(
    method: &str,
    pred: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    r1()
        .iter()
        .find(|f| f.dir == "in" && f.method == method && pred(&f.body))
        .map(|f| f.body.clone())
        .unwrap_or_else(|| panic!("r1 carries a matching {method} notification"))
}

/// The first recorded notification body for `method`.
fn notification(method: &str) -> serde_json::Value {
    r1()
        .iter()
        .find(|f| f.dir == "in" && f.method == method)
        .map(|f| f.body.clone())
        .unwrap_or_else(|| panic!("r1 carries a {method} notification"))
}

/// The first recorded outbound body for `method`.
fn recorded_out(method: &str) -> serde_json::Value {
    r1()
        .iter()
        .find(|f| f.dir == "out" && f.method == method)
        .map(|f| f.body.clone())
        .unwrap_or_else(|| panic!("r1 carries an outbound {method}"))
}

/// A fake WS server that answers the autonomy RPCs with the recording's own
/// objects, and records `(method, params)`.
struct AutonomyServer {
    base_url: String,
    received: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
}

impl AutonomyServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(Mutex::new(Vec::new()));
        let rx = received.clone();

        // The recorded objects (from the notifications the run streamed back).
        let goal_upd = notification("session/goal/updated");
        let loop_upd = notification("loop/updated");
        let mon_upd = notification("monitor/updated");
        let mon_create = {
            let frames = load("c24b-subagent-a6ea8505.jsonl");
            frames
                .iter()
                .find(|f| f.dir == "out" && f.method == "monitor/create")
                .map(|f| f.body.clone())
                .expect("c24b carries monitor/create")
        };

        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let Ok(wsstream) = tokio_tungstenite::accept_async(stream).await else {
                return;
            };
            let (mut tx, mut rx_in) = wsstream.split();
            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                let params = v["params"].clone();
                rx.lock().unwrap().push((method.clone(), params.clone()));

                let result = match method.as_str() {
                    "session/open" => serde_json::json!({"opened": {
                        "session_id": params["session_id"].as_str().unwrap_or("dsflash:main"),
                        "active_profile_id": "dsflash",
                        "cursor": {"stream": "dsflash:main", "seq": 1},
                        "capabilities": {
                            "version": {"protocol": "octos-ui/v1alpha1",
                                        "schema_version": 1, "jsonrpc": "2.0"},
                            "capabilities_schema_version": 1,
                            "supported_methods": ["session/open",
                                                  "session/goal/get", "session/goal/set",
                                                  "session/goal/clear",
                                                  "loop/list", "loop/pause", "loop/resume",
                                                  "loop/delete", "loop/fire_now",
                                                  "monitor/list", "monitor/create",
                                                  "monitor/pause", "monitor/resume",
                                                  "monitor/delete"],
                            "supported_notifications": ["session/goal/updated",
                                                        "session/goal/cleared",
                                                        "loop/updated", "monitor/updated"],
                            "supported_features": []
                        }
                    }}),
                    // The GET answers with the object the recording streamed.
                    "session/goal/get" => {
                        let mut goal = goal_upd["goal"].clone();
                        if params["status"] == "paused" {
                            goal["status"] = serde_json::json!("paused");
                        }
                        serde_json::json!({
                            "session_id": params["session_id"],
                            "profile_id": params["profile_id"],
                            "goal": goal,
                            "generation": goal_upd["generation"],
                        })
                    }
                    "session/goal/set" => {
                        let mut goal = goal_upd["goal"].clone();
                        goal["objective"] = params["objective"].clone();
                        if let Some(status) = params.get("status") {
                            goal["status"] = status.clone();
                        }
                        if let Some(budget) = params.get("token_budget") {
                            goal["token_budget"] = budget.clone();
                        }
                        serde_json::json!({
                            "session_id": params["session_id"],
                            "profile_id": params["profile_id"],
                            "goal": goal,
                            "generation": 2,
                            "transition_actor": params["transition_actor"],
                        })
                    }
                    "session/goal/clear" => serde_json::json!({
                        "session_id": params["session_id"],
                        "profile_id": params["profile_id"],
                        "cleared": true,
                        "goal": serde_json::Value::Null,
                        "generation": 3,
                        "transition_actor": "user",
                    }),
                    "loop/list" => serde_json::json!({
                        "session_id": params["session_id"],
                        "profile_id": params["profile_id"],
                        "loops": [loop_upd["loop"]],
                    }),
                    "monitor/list" => serde_json::json!({
                        "session_id": params["session_id"],
                        "profile_id": params["profile_id"],
                        "monitors": [mon_upd["monitor"]],
                    }),
                    "monitor/create" => serde_json::json!({
                        "monitor": {
                            "monitor_id": "monitor_new",
                            "name": params["name"],
                            "argv": params["argv"],
                            "mode": params["mode"],
                            "status": "active",
                            "interval_seconds": params["interval_seconds"],
                        }
                    }),
                    m if m.starts_with("loop/") || m.starts_with("monitor/") => {
                        serde_json::json!({"ok": true})
                    }
                    _ => serde_json::json!({}),
                };
                let frame = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result});
                let _ = tx.send(Message::Text(frame.to_string().into())).await;
            }
        });

        Self {
            base_url: format!("http://{addr}"),
            received,
        }
    }

    fn params_of(&self, method: &str) -> Option<serde_json::Value> {
        self.received
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|(m, _)| m == method)
            .map(|(_, p)| p.clone())
    }
}

/// The screen cache behind [`au::reset_state`] is process-global, and cargo
/// runs one suite's tests on parallel threads: every wired test holds this
/// lock for its WHOLE body, so reset → refresh → assert stays atomic
/// (without it a sibling's wired() wipes the cache mid-test and a row action
/// silently degrades to Unhandled).
static SERIAL: Mutex<()> = Mutex::new(());

async fn wired() -> (
    Conversation,
    AutonomyServer,
    Arc<Store>,
    Mutex<FlowUi>,
    MutexGuard<'static, ()>,
) {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // Every wired test starts from an empty slate (while holding the lock).
    au::reset_state();
    let server = AutonomyServer::start().await;
    let (conv, _events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
        .expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    tokio::time::sleep(Duration::from_millis(80)).await;
    // #P4e1b: bind the authority the production `session/open` arm binds
    // (`flow.rs`, the `SessionOpen` reply arm). This harness never drains the
    // event stream, so that arm does not run here and the autonomy fence would
    // — correctly, fail-closed (web `autonomy/store.ts`: "with no active command
    // authority in the current epoch … a notification must never repopulate
    // state") — refuse every result, leaving the screen cache empty.
    conv.store.domains.autonomy.bind_identity("f30b#1");
    conv.store.domains.autonomy.bind_session(&conv.session_id());
    let store = Arc::new(Store::new());
    // #P4e1b F1: a real `session/open` reply fills TWO lists — the METHODS into
    // `config.supported_methods` and the `coding.*` FEATURES into
    // `capabilities()` (the transport's `Capabilities::parse`
    // `capability/mod.rs:137-139` early-returns `from_supported_features` on a
    // `supported_features` array, and `:121` puts only feature names in `raw`;
    // methods arrive separately, `flow.rs:1231`). This helper used to seed
    // METHOD NAMES into `capabilities()`, which no production path does — that
    // un-production-shaped fixture is what let the old single-list gate read as
    // "proven". The corrected gate (`screens/autonomy.rs::gated`) requires
    // BOTH halves, so seed them the way production does.
    store
        .domains
        .config
        .set_supported_methods(vec![
            "session/goal/get".into(),
            "session/goal/set".into(),
            "session/goal/clear".into(),
            "loop/list".into(),
            "loop/pause".into(),
            "loop/resume".into(),
            "loop/delete".into(),
            "loop/fire_now".into(),
            "monitor/list".into(),
            "monitor/create".into(),
            "monitor/pause".into(),
            "monitor/resume".into(),
            "monitor/delete".into(),
        ]);
    store.set_capabilities(vec![
        "coding.goal_runtime.v1".into(),
        "coding.loop_runtime.v1".into(),
        "coding.monitor_runtime.v1".into(),
    ]);
    let ui = Mutex::new(FlowUi::default());
    (conv, server, store, ui, _guard)
}

fn ctx<'a>(store: &'a Arc<Store>, ui: &'a Mutex<FlowUi>) -> Ctx<'a> {
    Ctx::new(store, ui)
}

/// The recorded shapes the assertions lean on (fixture-faithful, hermetic).
fn recorded_ids() -> (String, String) {
    let goal = notification("session/goal/updated")["goal"]["goal_id"]
        .as_str()
        .unwrap_or("goal_01")
        .to_owned();
    let loop_id = recorded_out("loop/pause")["loop_id"]
        .as_str()
        .unwrap_or("loop_01")
        .to_owned();
    (goal, loop_id)
}

/// §1 — the refresh reads the recorded goal; the bindings project it
/// (objective, status, budget line, transition availability).
#[tokio::test]
async fn goal_refresh_projects_the_recorded_goal() {
    let (conv, _server, store, ui, _serial) = wired().await;
    let c = ctx(&store, &ui);
    au::apply(au::Effect::RefreshGoal, &conv).await.expect("get");

    assert_eq!(
        au::query(&c, "goal.objective").unwrap(),
        serde_json::json!("r1 replay probe"),
        "the recorded objective reaches the card"
    );
    assert_eq!(au::query(&c, "goal.status").unwrap(), serde_json::json!("active"));
    assert_eq!(
        au::query(&c, "goal.budget").unwrap(),
        serde_json::json!("0 / 100M"),
        "model.ts:126/129-136 formatTokens; the ' tokens' suffix is dropped (the card's label names the unit) to fit the measured slot"
    );
    assert_eq!(
        au::query(&c, "goal.fill").unwrap(),
        serde_json::json!(0.0),
        "#30b2: the bar's fill is bound to used/budget (0 of the recorded 100M)"
    );
    assert_eq!(au::query(&c, "goal.can_transition").unwrap(), serde_json::json!(true));
    assert_eq!(au::query(&c, "goal.elapsed").unwrap(), serde_json::json!("0s"));
}

/// §2 — goal.set parses "objective | budget" strictly and sends the recorded
/// param shape; a bare objective omits token_budget entirely (model.ts:195).
#[tokio::test]
async fn goal_set_sends_the_recorded_param_shape() {
    let (conv, server, store, ui, _serial) = wired().await;
    let c = ctx(&store, &ui);

    let effect = au::resolve("goal.set", 0, Some("Ship the port | 2000"), &c);
    assert_eq!(
        effect,
        au::Effect::SetGoal {
            objective: "Ship the port".to_owned(),
            token_budget: Some(2000)
        }
    );
    au::apply(effect, &conv).await.expect("set");

    let params = server.params_of("session/goal/set").expect("set params");
    assert_eq!(params["objective"], serde_json::json!("Ship the port"));
    assert_eq!(params["token_budget"], serde_json::json!(2000));
    assert_eq!(
        params["transition_actor"],
        serde_json::json!("user"),
        "operator transitions carry the user actor (parity 43/45)"
    );
    assert_eq!(params["session_id"], serde_json::json!("dsflash:main"));

    // A bare objective: the budget field is OMITTED, not defaulted.
    au::apply(au::resolve("goal.set", 0, Some("Ship it"), &c), &conv)
        .await
        .expect("set");
    let params = server.params_of("session/goal/set").expect("set params");
    assert!(params.get("token_budget").is_none(), "blank budget is omitted");
    // The exact set shape matches the recording's own keys (plus status-free).
    let recorded = recorded_out("session/goal/set");
    for key in recorded.as_object().unwrap().keys() {
        assert!(
            key == "objective" || params.get(key).is_some(),
            "recorded key {key} missing from the sent params"
        );
    }
}

/// §3 — goal.pause is the two-step TUI transition: fresh goal/get, then
/// goal/set status=paused with the fresh objective (store.ts:545-560).
#[tokio::test]
async fn pause_is_a_two_step_read_then_set() {
    let (conv, server, store, ui, _serial) = wired().await;
    let c = ctx(&store, &ui);
    au::apply(au::Effect::RefreshGoal, &conv).await.expect("get");
    let before = server.params_of("session/goal/get").map(|_| ()).is_some();

    au::apply(au::resolve("goal.pause", 0, None, &c), &conv)
        .await
        .expect("transition");

    let gets = server
        .received
        .lock()
        .unwrap()
        .iter()
        .filter(|(m, _)| m == "session/goal/get")
        .count();
    assert!(before && gets >= 2, "the transition re-read the goal first");
    let set = server.params_of("session/goal/set").expect("set params");
    assert_eq!(set["status"], serde_json::json!("paused"));
    assert_eq!(
        set["objective"],
        serde_json::json!("r1 replay probe"),
        "the fresh objective rides the transition set"
    );
    assert_eq!(au::query(&c, "goal.status").unwrap(), serde_json::json!("paused"));
}

/// #P4e1a row 44 — `goal.clear` reaches the wire through the PRODUCTION path
/// (`resolve` → `apply` → `Conversation::client`), the body matches the
/// recording's own outbound frame key-for-key, and the goal leaves the
/// screen state so the card renders its no-goal form.
#[tokio::test]
async fn goal_clear_sends_the_recorded_body_and_empties_the_card() {
    let (conv, server, store, ui, _serial) = wired().await;
    let c = ctx(&store, &ui);
    au::apply(au::Effect::RefreshGoal, &conv).await.expect("get");
    assert_eq!(
        au::query(&c, "goal.objective").unwrap(),
        serde_json::json!("r1 replay probe"),
        "precondition: the recorded goal is on the card"
    );

    au::apply(au::resolve("goal.clear", 0, None, &c), &conv)
        .await
        .expect("clear");

    // The send happened, and its keys are exactly the recording's own.
    let params = server.params_of("session/goal/clear").expect("clear params");
    let recorded = recorded_out("session/goal/clear");
    for key in recorded.as_object().unwrap().keys() {
        assert!(
            params.get(key).is_some(),
            "recorded key {key} missing from the sent clear params: {params}"
        );
    }
    assert_eq!(params["session_id"], serde_json::json!("dsflash:main"));
    // The card no longer shows the goal (the web's cleared form).
    assert_eq!(au::query(&c, "goal.objective").unwrap(), serde_json::Value::Null);
    assert_eq!(au::query(&c, "goal.status").unwrap(), serde_json::Value::Null);
    assert_eq!(au::query(&c, "goal.can_transition").unwrap(), serde_json::json!(false));
    assert_eq!(au::query(&c, "goal.fill").unwrap(), serde_json::Value::Null);
}

/// #P4e1a row 49 — the monitor row actions pause/resume/delete reach the wire
/// with the RECORDED monitor id (`monitor_01`), and an out-of-range row never
/// leaves the module (parity 49).
#[tokio::test]
async fn monitor_rows_route_with_the_recorded_id() {
    let (conv, server, store, ui, _serial) = wired().await;
    let c = ctx(&store, &ui);
    au::apply(au::Effect::RefreshLists, &conv).await.expect("lists");

    let rows = au::query(&c, "monitors").unwrap();
    let row = &rows.as_array().unwrap()[0];
    assert_eq!(row["id"], serde_json::json!("monitor_01"));
    assert_eq!(row["name"], serde_json::json!("r1 replay monitor"));

    for (id, method) in [
        ("monitor.pause", "monitor/pause"),
        ("monitor.resume", "monitor/resume"),
        ("monitor.delete", "monitor/delete"),
    ] {
        au::apply(au::resolve(id, 0, None, &c), &conv)
            .await
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        let params = server.params_of(method).unwrap_or_else(|| panic!("{method} sent"));
        assert_eq!(
            params["monitor_id"],
            serde_json::json!("monitor_01"),
            "{method} carries the recorded monitor id"
        );
    }

    // A row index the cached list does not have never reaches the wire.
    assert_eq!(
        au::resolve("monitor.pause", 5, None, &c),
        au::Effect::Unhandled("monitor.pause[5]".to_owned())
    );
}

/// #P4e1a row 51 — the flood-pause reason SURFACES: a monitor the recording
/// paused with `pause_reason` renders as `status (reason)` on the row
/// (AutonomyPanel.tsx:354-356), and the reason survives the store's
/// `monitor/expired` path (store/autonomy.rs:279-291 `mark_monitor_expired`).
#[test]
fn a_paused_monitor_surfaces_its_pause_reason() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    au::reset_state();
    let store = Arc::new(Store::new());
    let ui = Mutex::new(FlowUi::default());
    let c = ctx(&store, &ui);

    // The PAUSED recorded monitor/updated frame (r1 at_ms 9621) carries
    // pause_reason:"user" — the first in-bound frame is the active one.
    let recorded =
        notification_where("monitor/updated", |b| b["monitor"]["status"] == "paused")
            ["monitor"]
            .clone();
    assert_eq!(
        recorded["pause_reason"],
        serde_json::json!("user"),
        "the recording is the source of the reason"
    );
    au::update_state(|st| st.monitors = vec![recorded.clone()]);

    // The card text is the status with the reason in parentheses.
    let st = au::AutonomyState { monitors: vec![recorded], ..Default::default() };
    let lowered = au::lower_tree(au::Screen3::Monitors, &st).unwrap();
    let inv = |id: &str| {
        lowered
            .inventory
            .iter()
            .find(|v| v["original_id"] == serde_json::json!(id))
            .unwrap_or_else(|| panic!("no {id} in the inventory"))
            .clone()
    };
    assert_eq!(
        inv("mon_1_state")["text"],
        serde_json::json!("paused (user)"),
        "the reason rides the status text (AutonomyPanel.tsx:354)"
    );

    // The store's flood path sets status=expired AND keeps the reason.
    let mon_store = octoscode_store::Store::new();
    mon_store.domains.autonomy.set_monitors(vec![octoscode_store::domains::autonomy::MonitorRecord {
        monitor_id: "monitor_01".into(),
        session_id: "dsflash:main".into(),
        profile_id: Some("_main".into()),
        name: "r1 replay monitor".into(),
        mode: "poll".into(),
        status: "paused".into(),
        pause_reason: Some("flood".into()),
        fires_used: 0,
        last_fired_at_ms: None,
        expires_at_ms: None,
        updated_at_ms: 0,
    }]);
    assert!(
        mon_store
            .domains
            .autonomy
            .mark_monitor_expired("monitor_01", Some("flood cap".into())),
        "monitor/expired lands for a known id"
    );
    let after = mon_store
        .domains
        .autonomy
        .monitor("monitor_01")
        .expect("the monitor is still there");
    assert_eq!(after.status, "expired");
    assert_eq!(
        after.pause_reason.as_deref(),
        Some("flood cap"),
        "the pause reason is the surfaced flood reason"
    );
}

/// §4 — generation admission (parity 46): a stale result cannot regress a
/// newer stamp; generation 0 (unstamped legacy) always applies.
#[test]
fn generation_admission_matches_the_web_rule() {
    assert!(au::admits(0, 0) && au::admits(0, 7), "unstamped always applies");
    assert!(au::admits(3, 3) && au::admits(3, 4), "same-or-newer lands");
    assert!(!au::admits(3, 2), "a stale generation is dropped");
}

/// §5 — loop row actions route per-action with the recorded loop_01 id
/// (parity 47); the cached list feeds both the rows and the ids.
#[tokio::test]
async fn loop_rows_route_with_the_recorded_ids() {
    let (conv, server, store, ui, _serial) = wired().await;
    let c = ctx(&store, &ui);
    au::apply(au::Effect::RefreshLists, &conv).await.expect("lists");

    let loops = au::query(&c, "loops").unwrap();
    let row = &loops.as_array().unwrap()[0];
    assert_eq!(row["id"], serde_json::json!("loop_01"));
    assert_eq!(row["cadence"], serde_json::json!("hourly"), "the web interval ladder (model.ts:138-146)");
    assert_eq!(au::query(&c, "loops.count").unwrap(), serde_json::json!(1));
    assert_eq!(au::query(&c, "loops.empty").unwrap(), serde_json::json!(""));
    assert_eq!(row["name"], serde_json::json!("r1 replay probe"));

    au::apply(au::resolve("loop.pause", 0, None, &c), &conv)
        .await
        .expect("pause");
    let params = server.params_of("loop/pause").expect("pause params");
    assert_eq!(params["loop_id"], serde_json::json!("loop_01"));

    au::apply(au::resolve("loop.fire_now", 0, None, &c), &conv)
        .await
        .expect("fire");
    assert!(server.params_of("loop/fire_now").is_some());

    // An out-of-range row never reaches the wire.
    assert_eq!(
        au::resolve("loop.delete", 5, None, &c),
        au::Effect::Unhandled("loop.delete[5]".to_owned())
    );
}

/// §6 — monitor.create parses "name | argv json | filter" into the recorded
/// body shape (poll mode + optional filter_regex); a non-array argv never
/// reaches the wire (model.ts:215 pre-validation).
#[tokio::test]
async fn monitor_create_parses_and_matches_the_recorded_body() {
    let (conv, server, store, ui, _serial) = wired().await;
    let c = ctx(&store, &ui);

    au::apply(
        au::resolve(
            "monitor.create",
            0,
            Some("r1 replay monitor | [\"./scripts/watch.sh\"] | ERROR"),
            &c,
        ),
        &conv,
    )
    .await
    .expect("create");

    let params = server.params_of("monitor/create").expect("create params");
    assert_eq!(params["name"], serde_json::json!("r1 replay monitor"));
    assert_eq!(params["argv"], serde_json::json!(["./scripts/watch.sh"]));
    assert_eq!(params["filter_regex"], serde_json::json!("ERROR"));
    assert_eq!(params["mode"], serde_json::json!("poll"));
    // The recorded body's keys are all present (the create matches the wire).
    let frames = load("c24b-subagent-a6ea8505.jsonl");
    let recorded = frames
        .iter()
        .find(|f| f.dir == "out" && f.method == "monitor/create")
        .map(|f| f.body.clone())
        .unwrap();
    for key in recorded.as_object().unwrap().keys() {
        assert!(
            params.get(key).is_some(),
            "recorded key {key} missing from the sent params"
        );
    }

    // A non-array argv is refused locally by resolve — the pure router never
    // produces an effect for it, so nothing can reach the wire.
    assert_eq!(
        au::resolve("monitor.create", 0, Some("m | not-json"), &c),
        au::Effect::Unhandled("monitor.create[argv-not-array]".to_owned())
    );
}

/// §7 — without the advertised methods every action in the family fails
/// closed and the section bindings hide (the web renders these sections only
/// when advertised).
#[test]
fn families_fail_closed_without_the_advertised_methods() {
    let store = Arc::new(Store::new());
    let ui = Mutex::new(FlowUi::default());
    let c = ctx(&store, &ui);
    for id in ["goal.refresh", "goal.set", "goal.pause", "loops.refresh", "loop.pause", "monitors.refresh", "monitor.create"] {
        let gated = au::resolve(id, 0, Some("x"), &c);
        assert!(
            matches!(&gated, au::Effect::Unhandled(u) if u.contains("not-advertised")),
            "{id} must be gated: {gated:?}"
        );
    }
    assert_eq!(au::query(&c, "goal.available").unwrap(), serde_json::json!(false));
    assert_eq!(au::query(&c, "loops.available").unwrap(), serde_json::json!(false));
    assert_eq!(au::query(&c, "monitors.available").unwrap(), serde_json::json!(false));
}

/// Coverage contract: every declared action routes (Unhandled only = the
/// documented gates), every binding resolves once lists are loaded.
#[tokio::test]
async fn every_declared_id_is_live() {
    let (conv, _server, store, ui, _serial) = wired().await;
    au::apply(au::Effect::RefreshGoal, &conv).await.expect("goal");
    au::apply(au::Effect::RefreshLists, &conv).await.expect("lists");
    ui.lock().unwrap().set_draft_inner("probe | [\"echo\"] | ERROR");
    let c = ctx(&store, &ui);

    for (id, _) in au::ACTIONS {
        assert!(au::is_action(id));
        assert!(au::is_routed(id), "{id} declared but unrouted");
        // Per-id entry text so the walk exercises the ROUTE, not a parse gate:
        // goal.set takes a bare objective (blank budget omitted), the monitor
        // takes its full "name | argv | filter" form.
        let value = match *id {
            "goal.set" => Some("probe"),
            "monitor.create" => Some("probe | [\"echo\"] | ERROR"),
            _ => Some("probe"),
        };
        match au::resolve(id, 0, value, &c) {
            au::Effect::Unhandled(u) => {
                assert!(u.contains("not-advertised"), "{id} routed to Unhandled({u})")
            }
            _ => {}
        }
    }
    for (id, _) in au::BINDINGS {
        assert!(au::query(&c, id).is_some(), "binding {id} resolves None");
    }
}

/// #30b2 — rows follow the data: 0 items show the empty line, 1 shows one
/// row, 3 show three; the footer pluralizes; the command is ellipsized and
/// the interval uses the web's ladder (model.ts:138-146).
#[test]
fn row_counts_follow_the_data_and_strings_use_the_web_formats() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    au::reset_state();
    let store = Arc::new(Store::new());
    let ui = Mutex::new(FlowUi::default());
    let c = ctx(&store, &ui);
    let monitor = |name: &str, argv: &str| {
        serde_json::json!({
            "monitor_id": format!("mon_{name}"), "name": name,
            "argv": [argv], "status": "active", "interval_seconds": 3600,
        })
    };

    // 0 items: the atlas empty-state line, no rows.
    au::update_state(|st| st.monitors.clear());
    assert_eq!(au::query(&c, "monitors.count").unwrap(), serde_json::json!(0));
    assert_eq!(
        au::query(&c, "monitors.empty").unwrap(),
        serde_json::json!("No monitors in this session."),
        "AutonomyPanel.tsx:338"
    );

    // 1 item: exactly one row, pluralized footer, web interval, ellipsized cmd.
    au::update_state(|st| st.monitors = vec![monitor("a", "./scripts/watch.sh")]);
    let rows = au::query(&c, "monitors").unwrap().as_array().unwrap().clone();
    assert_eq!(rows.len(), 1, "exactly the store's items");
    assert_eq!(
        au::query(&c, "monitors.footer").unwrap(),
        serde_json::json!("1 monitor · 1 active")
    );
    assert_eq!(
        rows[0]["interval"],
        serde_json::json!("1h"),
        "the compact ladder the entry names (model.ts:138-146 steps)"
    );
    assert_eq!(
        rows[0]["cmd"],
        serde_json::json!("./scripts/wa…"),
        "ellipsized to the narrow slot, never mid-word clipped"
    );

    // 3 items: three rows, plural footer.
    let three: Vec<serde_json::Value> =
        (0..3).map(|k| monitor(&format!("m{k}"), "echo")).collect();
    au::update_state(|st| st.monitors = three);
    assert_eq!(au::query(&c, "monitors.count").unwrap(), serde_json::json!(3));
    assert_eq!(
        au::query(&c, "monitors.footer").unwrap(),
        serde_json::json!("3 monitors · 3 active")
    );

    // Loops: the same count contract.
    au::update_state(|st| st.loops.clear());
    assert_eq!(
        au::query(&c, "loops.empty").unwrap(),
        serde_json::json!("No loops in this session."),
        "AutonomyPanel.tsx:238"
    );
}

/// #30b2 — the budget bar's fill is the used/budget binding (the design's
/// static ~40% fill was the bug).
#[test]
fn budget_fill_binds_used_over_budget() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    au::reset_state();
    let store = Arc::new(Store::new());
    let ui = Mutex::new(FlowUi::default());
    let c = ctx(&store, &ui);
    au::update_state(|st| {
        st.goal = Some(serde_json::json!({
            "tokens_used": 40000000u64, "token_budget": 100000000u64,
            "status": "active", "time_used_seconds": 2400u64,
        }))
    });
    assert_eq!(au::query(&c, "goal.fill").unwrap(), serde_json::json!(0.4));
    assert_eq!(
        au::query(&c, "goal.elapsed").unwrap(),
        serde_json::json!("40m"),
        "atlas-granular elapsed"
    );
}

/// #30b4 — the APP's lowered output follows the data: card height, divider
/// count and the empty-line slot are decided in `lower_tree` (the production
/// path the mount renders), not in capture tooling.
#[test]
fn lowered_output_follows_the_store() {
    use octoscode_module::screens::autonomy::Screen3;
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    au::reset_state();
    let loop_row = serde_json::json!({
        "loop_id": "loop_01", "prompt": "r1 replay probe",
        "mode": "fixed_interval", "interval_seconds": 3600, "status": "active",
    });
    let monitor = |name: &str, argv: &str| {
        serde_json::json!({
            "monitor_id": format!("mon_{name}"), "name": name, "argv": [argv],
            "status": "active", "interval_seconds": 3600,
        })
    };
    let at = |inv: &[serde_json::Value], id: &str| -> serde_json::Value {
        inv.iter()
            .find(|v| v["original_id"] == serde_json::json!(id))
            .cloned()
            .unwrap_or_else(|| panic!("no {id} in the inventory"))
    };

    // 1 loop: the compact card; both dividers collapse.
    let st1 = au::AutonomyState { loops: vec![loop_row.clone()], ..Default::default() };
    let l1 = au::lower_tree(Screen3::Loops, &st1).unwrap();
    // One visible row: card = row1's bottom (~219.6, the dot) + the authored
    // pad, up from the card top 141 — about 126, NOT the authored 370.
    let h1 = at(&l1.inventory, "loops_card")["height"].as_f64().unwrap();
    assert!(
        (h1 - 125.6).abs() < 3.0 && h1 < 370.0,
        "the card sizes to its one row (got {h1})"
    );
    assert_eq!(at(&l1.inventory, "loop_2_divider")["width"], serde_json::json!(0.0));
    assert_eq!(at(&l1.inventory, "loop_3_divider")["width"], serde_json::json!(0.0));
    assert_eq!(
        at(&l1.inventory, "loop_1_name")["text"],
        serde_json::json!("r1 replay probe")
    );

    // 0 loops: the empty line renders in the BODY-font name slot (the mono
    // command slot pattern is monitors'; loop_1_cad collapses).
    let l0 = au::lower_tree(Screen3::Loops, &au::AutonomyState::default()).unwrap();
    let h0 = at(&l0.inventory, "loops_card")["height"].as_f64().unwrap();
    // Measured: name bottom 193 + the authored pad 28.72 - card top 141.
    assert!(
        (h0 - 80.72).abs() < 1.0 && h0 < h1,
        "the empty card is the name box + pad (got {h0})"
    );
    assert_eq!(
        at(&l0.inventory, "loop_1_name")["text"],
        serde_json::json!("No loops in this session.")
    );
    assert_eq!(at(&l0.inventory, "loop_1_cad")["width"], serde_json::json!(0.0));

    // 3 loops: the authored card height; every row carries data.
    let st3 = au::AutonomyState {
        loops: vec![loop_row.clone(), loop_row.clone(), loop_row],
        ..Default::default()
    };
    let l3 = au::lower_tree(Screen3::Loops, &st3).unwrap();
    assert_eq!(at(&l3.inventory, "loops_card")["height"], serde_json::json!(370.0));
    assert_eq!(
        at(&l3.inventory, "loop_3_name")["text"],
        serde_json::json!("r1 replay probe")
    );

    // Monitors 0: the empty line on the body-font footer label; the mono
    // command slot carries nothing.
    let m0 = au::lower_tree(Screen3::Monitors, &au::AutonomyState::default()).unwrap();
    assert_eq!(
        at(&m0.inventory, "monitors_footer_label")["text"],
        serde_json::json!("No monitors in this session.")
    );
    assert_eq!(at(&m0.inventory, "mon_1_cmd")["text"], serde_json::json!(""));

    // Monitors 1: card 2 collapses; the footer follows card 1.
    let st1m = au::AutonomyState {
        monitors: vec![monitor("a", "./scripts/watch.sh")],
        ..Default::default()
    };
    let m1 = au::lower_tree(Screen3::Monitors, &st1m).unwrap();
    assert_eq!(at(&m1.inventory, "mon_2")["width"], serde_json::json!(0.0));
    assert_eq!(
        at(&m1.inventory, "monitors_footer_label")["text"],
        serde_json::json!("1 monitor · 1 active")
    );

    // Monitors 3: the cloned third card carries the THIRD item's values; the
    // footer lands below it.
    let st3m = au::AutonomyState {
        monitors: vec![
            monitor("a", "./scripts/watch.sh"),
            monitor("b", "tail -n 50 app.log"),
            serde_json::json!({
                "monitor_id": "mon_c", "name": "c",
                "argv": ["git", "status", "--short"],
                "status": "paused", "interval_seconds": 300,
            }),
        ],
        ..Default::default()
    };
    let m3 = au::lower_tree(Screen3::Monitors, &st3m).unwrap();
    assert_eq!(
        at(&m3.inventory, "mon_3_cmd")["text"],
        serde_json::json!("git status --short"),
        "the cloned row's cmd renders verbatim under the ONE budget (#32c2 item 2): 18 chars fit the 177px budget (capacity 19)"
    );
    assert_eq!(at(&m3.inventory, "mon_3_int")["text"], serde_json::json!("5m"));
    assert_eq!(
        at(&m3.inventory, "monitors_footer_label")["text"],
        serde_json::json!("3 monitors · 2 active")
    );
    // Geometry (y) is not in inspectable's snapshot — read `measured`.
    assert!(
        m3.measured["monitors_footer"].0 > m3.measured["mon_3"].0,
        "the footer follows the last card ({:?} vs {:?})",
        m3.measured["monitors_footer"],
        m3.measured["mon_3"]
    );
}
