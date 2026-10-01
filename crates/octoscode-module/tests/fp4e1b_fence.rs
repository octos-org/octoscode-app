//! P4e1b — the autonomy authority fence through the PRODUCTION path.
//!
//! Rows (`docs/parity-matrix.csv`, group `g-autonomy2`):
//! * 3 — every control needs BOTH its method and its `coding.*` feature
//!   (web `packages/client/src/autonomy.ts:98-116` `supportsMethodWithFeature`);
//! * 4 — authority epoch: a late result from a retired commands identity is
//!   dropped (`autonomy/store.ts:208-231` `#syncAuthority`, `:887-889`);
//! * 6 — refresh revision guard: a mid-refresh notification supersedes the
//!   stale snapshot, and a FOREIGN family's event does not discard another
//!   family's (`store.ts:350-357`);
//! * 8 — an error surfaces only while the op stays authorized (`store.ts`
//!   `#runGuarded`; the web renders it under `role="alert"`,
//!   `AutonomyPanel.tsx:227/:328/:485`);
//! * 9 — switching sessions drops prior autonomy state
//!   (`model.ts:101-104` `resetAutonomyForSession`).
//!
//! ## Recorded traffic
//!
//! The `session/open` reply is the RECORDED real one
//! (`crates/octoscode-client/tests/fixtures/r1-autonomy-a6ea8505.jsonl`): the
//! real server answers with 34 `supported_features` and 106
//! `supported_methods` as two SEPARATE arrays. Replayed verbatim, because row 3
//! is precisely about those halves arriving separately — and the recorded split
//! is the evidence for the fix (F1).
//!
//! The `session/goal/get` / `loop/list` / `monitor/list` replies are built from
//! the fixture's OWN recorded notification bodies (f30b's technique), so every
//! asserted value is a recorded one. Hermetic: fixture values only, no
//! environment (LESSONS "Replay tests must be hermetic").
//!
//! ## The F1 regression (row 3)
//!
//! The gate used to read `store.capabilities()` for METHOD names. Production
//! fills that list from `caps.raw.keys()`, and the transport's
//! `Capabilities::parse` (`capability/mod.rs:137-139`) early-returns
//! `from_supported_features` on a `supported_features` ARRAY, whose `:121`
//! inserts only FEATURE names. Methods land in `config.supported_methods`
//! (`flow.rs:1231`). So the old gate was false in production and every
//! autonomy control was dead. `recorded_open_gates_the_control` below is the
//! regression: over the recorded reply the control must be OPEN.
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, FlowUi};
use octoscode_module::screens::autonomy as au;
use octoscode_store::Store;

const R1: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../octoscode-client/tests/fixtures/r1-autonomy-a6ea8505.jsonl"
);

fn frames() -> Vec<Value> {
    std::fs::read_to_string(R1)
        .expect("read the recorded r1 fixture")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).expect("fixture line is JSON"))
        .collect()
}

/// The RECORDED real `session/open` reply body.
fn recorded_open() -> Value {
    frames()
        .into_iter()
        .find(|f| {
            f["dir"] == "in" && f["method"] == "session/open" && f["body"]["capabilities"].is_object()
        })
        .map(|f| f["body"].clone())
        .expect("r1 carries the real session/open reply")
}

fn recorded_notification(method: &str) -> Value {
    frames()
        .into_iter()
        .find(|f| f["dir"] == "in" && f["method"] == method)
        .map(|f| f["body"].clone())
        .unwrap_or_else(|| panic!("r1 carries a {method} notification"))
}

/// A fake WS server that answers with the recorded `session/open` reply and the
/// fixture's own goal/loop/monitor objects, and logs what it received.
///
/// * `hold` — stall the FIRST read of that method, so a test can act while a
///   request is genuinely in flight (rows 6 and 8);
/// * `fail` — answer that method with a JSON-RPC error, so the production
///   `apply()` takes its real failure path (row 8).
struct ReplayServer {
    base_url: String,
    received: Arc<Mutex<Vec<String>>>,
}

impl ReplayServer {
    async fn start(hold: Option<&'static str>, fail: Option<&'static str>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(Mutex::new(Vec::new()));
        let rx = received.clone();
        let open = recorded_open();
        let goal_upd = recorded_notification("session/goal/updated");
        let loop_upd = recorded_notification("loop/updated");
        let mon_upd = recorded_notification("monitor/updated");

        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else {
                return;
            };
            let (mut tx, mut rx_in) = ws.split();
            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                let params = v["params"].clone();
                let seen_here = {
                    rx.lock().unwrap().push(method.clone());
                    rx.lock().unwrap().iter().filter(|m| **m == method).count()
                };

                if let Some(h) = hold {
                    if method == h && seen_here == 1 {
                        tokio::time::sleep(Duration::from_millis(300)).await;
                    }
                }

                if let Some(f) = fail {
                    if method == f {
                        let frame = serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "error": {"code": -32000, "message": format!("{f} is not available")}
                        });
                        let _ = tx.send(Message::Text(frame.to_string().into())).await;
                        continue;
                    }
                }

                let result = match method.as_str() {
                    // The recorded `in` frame is the INNER opened object
                    // (`{capabilities, cursor, session_id}`); the wire result
                    // wraps it in `{"opened": …}`, which is what
                    // `UiRpcResult::from_method_and_result` and
                    // `Capabilities::parse` (which probes
                    // `/capabilities`, `/result/capabilities`,
                    // `/opened/capabilities`) read — the same shape f30b
                    // serves. Replayed verbatim inside that envelope.
                    "session/open" => serde_json::json!({"opened": open.clone()}),
                    "session/goal/get" => serde_json::json!({
                        "session_id": params["session_id"],
                        "profile_id": params["profile_id"],
                        "goal": goal_upd["goal"],
                        "generation": goal_upd["generation"],
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
                    _ => serde_json::json!({}),
                };
                let frame = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result});
                let _ = tx.send(Message::Text(frame.to_string().into())).await;
            }
        });
        Self { base_url: format!("http://{addr}"), received }
    }

    fn saw(&self, method: &str) -> bool {
        self.received.lock().unwrap().iter().any(|m| m == method)
    }
}

/// The screen cache is process-global (f30b's SERIAL note), so every wired
/// test holds this lock for its whole body.
static SERIAL: Mutex<()> = Mutex::new(());

/// A connected conversation over the RECORDED `session/open`.
///
/// `Arc` so a test can move it into `tokio::spawn` and have a request
/// genuinely in flight while the test acts (Rust futures are lazy, so an
/// un-awaited `apply()` would never actually run).
///
/// The inbound event stream is DRAINED into `Conversation::on_event` on its own
/// task — that is what actually runs `dispatch()`, and therefore the
/// `session/open` arm that fills the store's capability tables and binds the
/// authority identity. Tests that skip the drain (f30b does) must hand-feed the
/// store instead, which is exactly the artefact that hid F1.
async fn wired(
    hold: Option<&'static str>,
    fail: Option<&'static str>,
) -> (Arc<Conversation>, ReplayServer, Arc<Store>, Mutex<FlowUi>, MutexGuard<'static, ()>) {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    au::reset_state();
    let server = ReplayServer::start(hold, fail).await;
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
        .expect("connect");
    let conv = Arc::new(conv);
    {
        let conv = conv.clone();
        tokio::spawn(async move {
            while let Some(evt) = events.recv().await {
                conv.on_event(evt);
            }
        });
    }
    conv.open_workspace(None).await.expect("session/open");
    // Wait for the open reply to be folded (the production arm is async).
    let store = conv.store.clone();
    for _ in 0..40 {
        if store.domains.autonomy.epoch() > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(store.domains.autonomy.epoch() > 0, "the production session/open arm bound the authority");
    let _ = &conv;
    (conv, server, store, Mutex::new(FlowUi::default()), _guard)
}

fn ctx<'a>(store: &'a Arc<Store>, ui: &'a Mutex<FlowUi>) -> Ctx<'a> {
    Ctx::new(store, ui)
}

// ===========================================================================
// Row 3 — the gate needs BOTH halves
// ===========================================================================

/// The F1 regression, through the production `session/open` arm: the recorded
/// reply carries 34 features and 106 methods in TWO arrays, and the gate must
/// read each from its own list — so the goal controls are OPEN. With the old
/// gate (methods read from the feature list) every control was `false`.
#[tokio::test]
async fn recorded_open_gates_the_control() {
    let (conv, _server, store, ui, _serial) = wired(None, None).await;
    let c = ctx(&store, &ui);

    // The recorded split, asserted so this test fails loudly if the fixture is
    // ever swapped for a shape that does not exercise the two-list read.
    let caps = &recorded_open()["capabilities"];
    assert!(caps["supported_features"].is_array() && caps["supported_methods"].is_array());
    assert!(caps["supported_features"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("coding.goal_runtime.v1")));
    assert!(caps["supported_methods"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("session/goal/get")));

    // Production filled BOTH lists from that one reply...
    assert!(
        store.capabilities().contains(&"coding.goal_runtime.v1".to_owned()),
        "features land in capabilities()"
    );
    assert!(
        store
            .domains
            .config
            .supported_methods()
            .contains(&"session/goal/get".to_owned()),
        "methods land in config.supported_methods()"
    );
    // ...and the old gate would have been false, because capabilities() holds
    // no method name at all.
    assert!(
        !store.capabilities().contains(&"session/goal/get".to_owned()),
        "the feature list must never contain a method name (F1)"
    );

    // So the control is open, and its action routes to its real effect.
    assert_eq!(au::query(&c, "goal.available").unwrap(), serde_json::json!(true));
    assert_eq!(au::resolve("goal.refresh", 0, None, &c), au::Effect::RefreshGoal);
    assert!(matches!(
        au::resolve("goal.pause", 0, None, &c),
        au::Effect::Transition(s) if s == "paused"
    ));
    assert!(matches!(
        au::resolve("goal.set", 0, Some("probe | 2000"), &c),
        au::Effect::SetGoal { objective, token_budget }
            if objective == "probe" && token_budget == Some(2000)
    ));
}

/// Either half alone must fail closed — the web's `supportsMethodWithFeature`
/// is a conjunction (autonomy.ts:98-116), and the per-family test names this
/// ("shows all native unfinished-goal transitions only with both read and
/// write gates").
#[test]
fn row3_either_half_alone_fails_closed() {
    let store = Arc::new(Store::new());
    let ui = Mutex::new(FlowUi::default());
    let c = ctx(&store, &ui);

    // Nothing negotiated.
    assert_eq!(au::query(&c, "goal.available").unwrap(), serde_json::json!(false));

    // Method only.
    store
        .domains
        .config
        .set_supported_methods(vec!["session/goal/get".into(), "session/goal/set".into()]);
    assert_eq!(
        au::query(&c, "goal.available").unwrap(),
        serde_json::json!(false),
        "the method alone must not open the control"
    );
    assert!(matches!(
        au::resolve("goal.refresh", 0, None, &c),
        au::Effect::Unhandled(u) if u.contains("not-advertised")
    ));

    // Feature only.
    let store2 = Arc::new(Store::new());
    let ui2 = Mutex::new(FlowUi::default());
    let c2 = ctx(&store2, &ui2);
    store2.set_capabilities(vec!["coding.goal_runtime.v1".into()]);
    assert_eq!(au::query(&c2, "goal.available").unwrap(), serde_json::json!(false));

    // Both halves of THIS family -> open. The loops feature is a different
    // family, so it must not open the goal section.
    store.set_capabilities(vec!["coding.goal_runtime.v1".into(), "coding.loop_runtime.v1".into()]);
    assert_eq!(au::query(&c, "goal.available").unwrap(), serde_json::json!(true));
    assert_eq!(
        au::query(&c, "loops.available").unwrap(),
        serde_json::json!(false),
        "coding.loop_runtime.v1 must not open the goal section, and loop/list is unadvertised"
    );
    // goal.pause needs session/goal/set as well as session/goal/get.
    store
        .domains
        .config
        .set_supported_methods(vec!["session/goal/get".into()]);
    assert!(matches!(
        au::resolve("goal.pause", 0, None, &c),
        au::Effect::Unhandled(u) if u.contains("not-advertised")
    ));
}

// ===========================================================================
// Rows 4 + 9 — authority epoch and session binding
// ===========================================================================

/// The production `session/open` arm bound an identity and a session; a late
/// result captured under the old identity is refused, and a session switch
/// drops all prior autonomy state.
#[tokio::test]
async fn row4_and_row9_a_retired_identity_drops_the_late_result_and_the_state() {
    let (conv, _server, store, ui, _serial) = wired(None, None).await;
    let c = ctx(&store, &ui);
    let a = &store.domains.autonomy;

    assert_eq!(a.epoch(), 1, "the recorded open bound authority epoch 1");
    assert_eq!(a.bound_session().as_deref(), Some(conv.session_id().as_str()));
    assert!(a.epoch_admits(a.epoch()));

    // The screen cache is what the production refresh projects.
    au::apply(au::Effect::RefreshLists, &conv).await.expect("lists");
    assert_eq!(au::query(&c, "loops.count").unwrap(), serde_json::json!(1));
    assert_eq!(au::query(&c, "monitors.count").unwrap(), serde_json::json!(1));
    // The STORE's rows are folded by the notification handlers
    // (loop/updated, monitor/updated), which the replay server does not push,
    // so seed them through the same production store API those handlers call.
    store.domains.autonomy.upsert_loop(loop_row());
    store.domains.autonomy.upsert_monitor(monitor_row());
    assert_eq!(store.domains.autonomy.loop_count(), 1);
    assert_eq!(store.domains.autonomy.monitor_count(), 1);

    // Row 9: switching sessions drops ALL prior autonomy state
    // (`model.ts:101-104` `resetAutonomyForSession`).
    assert!(a.bind_session("other:main"), "a session switch rebinds");
    assert_eq!(store.domains.autonomy.loop_count(), 0, "loops are gone");
    assert_eq!(store.domains.autonomy.monitor_count(), 0, "monitors are gone");

    // Row 4: a NEW commands identity for the same session id bumps the epoch
    // and the old capture is refused — a stale store must never survive an
    // identity change under the same session id (`store.ts:208-213`).
    let stale = a.epoch();
    assert!(a.bind_identity("dsflash#2"), "a new identity bumps the epoch");
    assert!(!a.epoch_admits(stale), "the late result is dropped");
    assert!(a.epoch_admits(a.epoch()), "the current authority still admits");
}

/// The production `open_workspace_as` bumps the identity on every open, so the
/// second open of the SAME session id retires the first authority.
#[tokio::test]
async fn row4_a_second_open_retires_the_previous_authority() {
    let (conv, _server, store, _ui, _serial) = wired(None, None).await;
    let a = &store.domains.autonomy;
    let first = a.epoch();
    assert_eq!(first, 1);

    // Same session id, opened again (a reconnect / re-open in the web).
    conv.open_workspace(None).await.expect("second session/open");
    tokio::time::sleep(Duration::from_millis(150)).await;

    assert!(a.epoch() > first, "the second open retired the first authority");
    assert!(!a.epoch_admits(first), "a result from the retired identity is dropped");
}

// ===========================================================================
// Row 6 — refresh revision guard
// ===========================================================================

/// A notification landing mid-refresh supersedes that family's stale snapshot,
/// while a FOREIGN family's event does not discard another family's
/// (`store.ts:350-357`).
#[tokio::test]
async fn row6_a_mid_refresh_notification_supersedes_only_its_own_family() {
    let (conv, _server, store, ui, _serial) = wired(Some("loop/list"), None).await;
    let c = ctx(&store, &ui);
    let a = &store.domains.autonomy;

    // A loops refresh is genuinely in flight: the server stalls `loop/list`.
    let worker = {
        let conv = conv.clone();
        tokio::spawn(async move { au::apply(au::Effect::RefreshLists, &conv).await })
    };
    tokio::time::sleep(Duration::from_millis(80)).await;
    let in_flight = a.revision(au::FAMILY_LOOPS);
    assert!(in_flight > 0, "the refresh took a loops revision");

    // A FOREIGN-family owning event must NOT discard the loops snapshot
    // (`store.test.ts` "an unrelated-family owning event does not discard
    // another family's refresh snapshot"). `agents` is the family this refresh
    // does NOT read, so it is the true foreign case.
    a.supersede_family(au::FAMILY_AGENTS);
    assert_eq!(
        a.revision(au::FAMILY_LOOPS),
        in_flight,
        "a monitors event must not touch the loops revision"
    );
    assert!(
        au::authorized(&conv, au::FAMILY_LOOPS, a.epoch(), in_flight),
        "the loops snapshot is still authorized"
    );

    // The OWNING event (`loop/updated` -> `supersede_family("loops")`, the
    // production handler) makes the in-flight snapshot stale.
    a.supersede_family(au::FAMILY_LOOPS);
    assert!(
        !au::authorized(&conv, au::FAMILY_LOOPS, a.epoch(), in_flight),
        "the owning event supersedes the stale snapshot"
    );

    worker.await.expect("task").expect("refresh returns");

    // The superseded loops half did NOT land; the monitors half, untouched by
    // the loops event, DID.
    assert_eq!(
        au::query(&c, "loops.count").unwrap(),
        serde_json::json!(0),
        "the superseded loops snapshot must not land"
    );
    assert_eq!(
        au::query(&c, "monitors.count").unwrap(),
        serde_json::json!(1),
        "the untouched monitors snapshot lands"
    );
}

// ===========================================================================
// Row 8 — error surfacing under the authority
// ===========================================================================

/// A failed op files its error under its family, and the binding carries it —
/// the native half of the web's `role="alert"` live region
/// (`AutonomyPanel.tsx:227`).
#[tokio::test]
async fn row8_a_failure_surfaces_under_its_family() {
    let (conv, server, store, ui, _serial) = wired(None, Some("session/goal/set")).await;
    let c = ctx(&store, &ui);
    let a = &store.domains.autonomy;

    let err = au::apply(
        au::Effect::SetGoal { objective: "probe".to_owned(), token_budget: None },
        &conv,
    )
    .await
    .expect_err("the server refuses the set");
    assert!(err.contains("session/goal/set is not available"), "{err}");
    assert!(server.saw("session/goal/set"), "the request reached the wire");

    assert_eq!(a.error(au::FAMILY_GOAL).as_deref(), Some(err.as_str()));
    assert_eq!(au::query(&c, "goal.error").unwrap(), serde_json::json!(err));
    // Only the goal family carries it.
    assert_eq!(au::query(&c, "loops.error").unwrap(), serde_json::json!(""));
    assert_eq!(au::query(&c, "monitors.error").unwrap(), serde_json::json!(""));

    // A successful op in the same family clears it, as the web nulls the error.
    a.clear_error(au::FAMILY_GOAL);
    assert_eq!(au::query(&c, "goal.error").unwrap(), serde_json::json!(""));
}

/// A newer epoch superseding an in-flight request DROPS its error rather than
/// showing it (`store.ts` "drops the error when a newer epoch superseded the
/// request").
#[tokio::test]
async fn row8_an_error_is_dropped_when_a_newer_epoch_supersedes_the_request() {
    let (conv, _server, store, ui, _serial) = wired(Some("session/goal/set"), Some("session/goal/set"))
        .await;
    let c = ctx(&store, &ui);
    let a = &store.domains.autonomy;

    // The set is in flight when the authority is retired underneath it.
    let worker = {
        let conv = conv.clone();
        tokio::spawn(async move {
            au::apply(
                au::Effect::SetGoal { objective: "probe".to_owned(), token_budget: None },
                &conv,
            )
            .await
        })
    };
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(a.bind_identity("dsflash#2"), "the authority is retired mid-flight");
    let out = worker.await.expect("task");
    assert!(out.is_err(), "the op itself still failed");

    // The error is NOT shown: it belonged to a retired authority.
    assert_eq!(a.error(au::FAMILY_GOAL), None);
    assert_eq!(au::query(&c, "goal.error").unwrap(), serde_json::json!(""));

    // The same op under the CURRENT authority does surface.
    au::apply(
        au::Effect::SetGoal { objective: "probe".to_owned(), token_budget: None },
        &conv,
    )
    .await
    .expect_err("still refused");
    assert!(
        a.error(au::FAMILY_GOAL).is_some(),
        "an authorized op's error is shown"
    );
    assert!(!au::query(&c, "goal.error").unwrap().as_str().unwrap().is_empty());
}

// ===========================================================================
// Row fixtures — built from the RECORDED notification bodies, so every value is
// a recorded one (hermetic: no environment, no invented ids).
// ===========================================================================

fn loop_row() -> octoscode_store::domains::autonomy::LoopRecord {
    let l = recorded_notification("loop/updated")["loop"].clone();
    octoscode_store::domains::autonomy::LoopRecord {
        loop_id: l["loop_id"].as_str().unwrap_or_default().to_owned(),
        session_id: l["session_id"].as_str().unwrap_or_default().to_owned(),
        profile_id: l["profile_id"].as_str().map(str::to_owned),
        prompt: l["prompt"].as_str().unwrap_or_default().to_owned(),
        mode: l["mode"].as_str().unwrap_or_default().to_owned(),
        status: l["status"].as_str().unwrap_or_default().to_owned(),
        interval_seconds: l["interval_seconds"].as_u64(),
        next_run_at_ms: l["next_run_at_ms"].as_i64(),
        expires_at_ms: l["expires_at_ms"].as_i64().unwrap_or(0),
        updated_at_ms: l["updated_at_ms"].as_i64().unwrap_or(0),
        fires: 0,
    }
}

fn monitor_row() -> octoscode_store::domains::autonomy::MonitorRecord {
    let m = recorded_notification("monitor/updated")["monitor"].clone();
    octoscode_store::domains::autonomy::MonitorRecord {
        monitor_id: m["monitor_id"].as_str().unwrap_or_default().to_owned(),
        session_id: m["session_id"].as_str().unwrap_or_default().to_owned(),
        profile_id: m["profile_id"].as_str().map(str::to_owned),
        name: m["name"].as_str().unwrap_or_default().to_owned(),
        mode: m["mode"].as_str().unwrap_or_default().to_owned(),
        status: m["status"].as_str().unwrap_or_default().to_owned(),
        pause_reason: m["pause_reason"].as_str().map(str::to_owned),
        fires_used: m["fires_used"].as_u64().unwrap_or(0) as u32,
        last_fired_at_ms: m["last_fired_at_ms"].as_i64(),
        expires_at_ms: m["expires_at_ms"].as_i64(),
        updated_at_ms: m["updated_at_ms"].as_i64().unwrap_or(0),
    }
}
