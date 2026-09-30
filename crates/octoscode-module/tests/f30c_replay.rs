//! Entry #30c — the board 3.6 Fleet / 3.7 Tasks gates, on RECORDED real
//! traffic (RULES "Tests replay recorded real traffic"). No model is called.
//!
//! 1. **the folds** — the r6-peer `peer/gather` body and the r4-task
//!    `task/updated` body fold into store rows the bindings compose from;
//! 2. **the pure action table** — `(action, row, store, steer) → (method,
//!    params)` with the web's wire shapes (`peer/control` steer command,
//!    `task/cancel`, `task/output/read`);
//! 3. **replay** — a fake WS server serving the RECORDED bodies:
//!    `fleet::refresh` folds both reads through the production client, then
//!    the routed actions go out and the server SEES `peer/control` +
//!    `task/cancel` (neither method has a fixture — LESSONS: the request the
//!    production client actually sends is the assertion);
//! 4. **bindings coverage + card agreement** — every COPY_SLOTS id resolves
//!    (Some, incl. deliberate Nulls) on the capture store, and the lowered
//!    cards carry the live strings.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, FlowUi};
use octoscode_module::screens::fleet;
use octoscode_store::Store;

/// The FIRST recorded IN body for `method` (r4's two `task/updated` frames
/// are one task's lifecycle: running, then completed — the first is the
/// live-row fixture).
fn recorded(fixtures: &[&str], method: &str) -> serde_json::Value {
    for fixture in fixtures {
        let text = std::fs::read_to_string(fixture).unwrap_or_default();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if v["dir"].as_str() == Some("in") && v["method"].as_str() == Some(method) {
                return v.get("body").cloned().unwrap_or(serde_json::Value::Null);
            }
        }
    }
    panic!("no fixture carries a recorded {method} reply")
}

/// The LAST recorded IN body — r6's `peer/gather` appears twice (the
/// connection's first empty poll, then the populated blackboard), and the
/// populated one is the fixture shape.
fn recorded_last(fixtures: &[&str], method: &str) -> serde_json::Value {
    let mut found = None;
    for fixture in fixtures {
        let text = std::fs::read_to_string(fixture).unwrap_or_default();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if v["dir"].as_str() == Some("in") && v["method"].as_str() == Some(method) {
                found = Some(v.get("body").cloned().unwrap_or(serde_json::Value::Null));
            }
        }
    }
    found.unwrap_or_else(|| panic!("no fixture carries a recorded {method} reply"))
}

const R4_TASK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../octoscode-client/tests/fixtures/r4-task-a6ea8505.jsonl"
);
const R6_PEER: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../octoscode-client/tests/fixtures/r6-peer-a6ea8505.jsonl"
);

/// A fake WS server that answers each request with the RECORDED real body
/// (`dir == "in"` frames, matched by method); anything else gets
/// `{result: null}` — exactly the reply the transport decodes for the
/// not-yet-recorded methods. Records what it receives.
struct ReplayServer {
    url: String,
    seen: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
}

impl ReplayServer {
    async fn start(bodies: Vec<(String, serde_json::Value)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let rec = seen.clone();
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (mut tx, mut rx_in) = ws.split();
            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                let params = v.get("params").cloned().unwrap_or(serde_json::Value::Null);
                rec.lock().unwrap().push((method.clone(), params.clone()));
                let result = bodies
                    .iter()
                    .find(|(m, _)| *m == method)
                    .map(|(_, b)| b.clone())
                    .unwrap_or(serde_json::Value::Null);
                let reply = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result});
                let _ = tx.send(Message::Text(reply.to_string().into())).await;
            }
        });
        Self {
            url: format!("ws://{addr}"),
            seen,
        }
    }

    fn sent(&self, method: &str) -> Option<serde_json::Value> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .find(|(m, _)| m == method)
            .map(|(_, p)| p.clone())
    }
}

// --------------------------------------------------------------- 1. the folds

#[test]
fn the_recorded_bodies_fold_into_the_fleet_and_tasks_state() {
    let store = Store::new();
    store.domains.session.set_active(Some("dsflash:main".into()));

    // The r6 `peer/gather` result: one staged peer, not closed.
    let gather = recorded_last(&[R6_PEER], "peer/gather");
    fleet::fold_peer_gather(gather, &store);
    let peers = store.domains.peer.list();
    assert_eq!(peers.len(), 1, "the r6 blackboard has one peer");
    assert_eq!(peers[0].name, "r6-smoke");
    assert_eq!(peers[0].topic.as_deref(), Some("peer-r6-smoke"));
    assert!(!peers[0].closed);

    // The r4 `task/updated` notification: a running task with a title.
    let updated = recorded(&[R4_TASK], "task/updated");
    fleet::fold_task_updated(updated, &store);
    let running = store
        .domains
        .task
        .snapshots()
        .into_iter()
        .find(|t| t.state == "running")
        .expect("the recorded update creates the running row");
    assert_eq!(
        running.id,
        "01a0e767-56d1-7572-a9d4-36bff7b52514",
        "the recorded task id"
    );
    assert_eq!(
        running.tool_name, "M9 task output fixture",
        "a live update's title labels a NEW row (the domain's merge contract)"
    );

    // The r4 `task/list` reply is EMPTY (`tasks: []`) — folding it adds no
    // row and must not wipe the live one (the web keeps live merges too).
    let list = recorded(&[R4_TASK], "task/list");
    fleet::fold_task_list(list, &store);
    assert_eq!(store.domains.task.snapshot_count(), 1);
}

// ------------------------------------------------------- 2. the action table

#[test]
fn the_action_table_maps_the_card_controls_to_their_wire_frames() {
    let store = Arc::new(Store::new());
    store.domains.session.set_active(Some("dsflash:main".into()));
    for name in ["tests", "docs", "review"] {
        store.domains.peer.upsert(octoscode_store::domains::peer::Peer::named(name));
    }
    store.domains.task.upsert_snapshot(
        octoscode_store::domains::task::TaskSnapshot::from_list_row(
            "t-run".into(),
            "cargo test".into(),
            "running".into(),
            "running".into(),
            None,
            None,
            None,
            None,
            0,
            Vec::new(),
            None,
            None,
        ),
    );

    // Steer the THIRD row = one `peer/control` steer frame with the typed
    // text (the command shape of `external-driver-peer-control.ts:99/147`;
    // the driver-seat capture keys are the reported native gap). Rows are
    // slug-ordered (docs, review, tests), so row 2 is `tests`.
    let (method, params) =
        fleet::action_params("peer.steer", 2, &store, "hold the queue").expect("steer routes");
    assert_eq!(method, "peer/control");
    assert_eq!(params["session_id"], "dsflash:main");
    assert_eq!(params["slug"], "tests");
    assert_eq!(params["command"]["kind"], "steer");
    assert_eq!(params["command"]["input"][0]["kind"], "text");
    assert_eq!(params["command"]["input"][0]["text"], "hold the queue");

    // Cancel = one `task/cancel` for the RUNNING task (parity row 348).
    let (method, params) =
        fleet::action_params("task.cancel", 0, &store, "").expect("cancel routes");
    assert_eq!(method, "task/cancel");
    assert_eq!(params["task_id"], "t-run");
    assert_eq!(params["session_id"], "dsflash:main");

    // Open the running card = one `task/output/read` with a byte cursor —
    // the recorded c24b request shape (parity row 346).
    let (method, params) =
        fleet::action_params("task.open.running", 0, &store, "").expect("open routes");
    assert_eq!(method, "task/output/read");
    assert_eq!(params["task_id"], "t-run");
    assert_eq!(params["cursor"]["offset"], 0);
    assert_eq!(params["limit_bytes"], 65536);

    // The row-pinned control ids are unambiguous (one-owner rule): the bare
    // event takes the router's index, the pinned names take their own row.
    let ui = Mutex::new(FlowUi::default());
    let ctx = Ctx::new(&store, &ui);
    assert!(matches!(
        fleet::resolve("peer_3_steer", 9, &ctx),
        fleet::Effect::Steer { row: 2, .. }
    ));
    assert!(fleet::resolve("nope", 0, &ctx).is_unhandled());
}

// ------------------------------------------------------------- 3. the replay

#[test]
fn replay_refresh_then_the_actions_over_the_recorded_frames() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("runtime");

    let bodies = vec![
        ("task/list".to_owned(), recorded(&[R4_TASK], "task/list")),
        ("peer/gather".to_owned(), recorded_last(&[R6_PEER], "peer/gather")),
    ];
    let server = rt.block_on(ReplayServer::start(bodies));

    // `Conversation::connect` is synchronous but spawns the transport, so the
    // call must carry the runtime context (block_on provides it).
    let (conv, mut evt_rx) = rt
        .block_on(async {
            Conversation::connect(&server.url, "mapb-dummy", "dsflash", None, None)
        })
        .expect("the conversation connects to the replay server");
    let conv = Arc::new(conv);
    {
        let drain = conv.clone();
        rt.spawn(async move {
            while let Some(evt) = evt_rx.recv().await {
                let _ = drain.on_event(evt);
            }
        });
    }

    let store = Arc::new(Store::new());
    store.domains.session.set_active(Some("dsflash:main".into()));

    // The load path: both reads fold (the entry's "drive each action and
    // assert the store").
    let n = rt
        .block_on(fleet::refresh(&conv, &store))
        .expect("refresh over the recorded frames");
    assert_eq!(n, 2, "task/list + peer/gather folded");
    let peers = store.domains.peer.list();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].name, "r6-smoke");

    // The live task row comes from the RECORDED `task/updated` (the
    // `task/list` reply in this recording is empty) — the same fold the
    // domain's notification handler runs in production.
    fleet::fold_task_updated(recorded(&[R4_TASK], "task/updated"), &store);
    assert!(store
        .domains
        .task
        .snapshots()
        .iter()
        .any(|t| t.state == "running"));

    // The actions go through the module's own spawn (lib.rs's arm): the
    // server must SEE the two frames the production client sends — neither
    // method has a fixture, so the wire shape IS the evidence.
    let ui = Mutex::new(FlowUi::default());
    ui.lock().unwrap().set_draft_inner("hold the queue");
    let ctx = Ctx::new(&store, &ui);
    fleet::spawn(
        fleet::resolve("peer.steer", 0, &ctx),
        &rt,
        &conv,
        &ui,
        &store,
    );
    fleet::spawn(
        fleet::resolve("task.cancel", 0, &ctx),
        &rt,
        &conv,
        &ui,
        &store,
    );

    // Steer's draft is consumed (the web clears the row's draft on send).
    assert!(ui.lock().unwrap().draft().is_empty());

    rt.block_on(async {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let have_both = server.seen.lock().unwrap().iter().any(|(m, _)| m == "peer/control")
                && server.seen.lock().unwrap().iter().any(|(m, _)| m == "task/cancel");
            if have_both || tokio::time::Instant::now() > deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });

    let steer = server.sent("peer/control").expect("peer/control went out");
    assert_eq!(steer["slug"], "r6-smoke");
    assert_eq!(steer["command"]["kind"], "steer");
    assert_eq!(steer["command"]["input"][0]["text"], "hold the queue");
    let cancel = server.sent("task/cancel").expect("task/cancel went out");
    assert_eq!(cancel["session_id"], "dsflash:main");
    // The running task here is the one the RECORDED update created.
    assert_eq!(cancel["task_id"], "01a0e767-56d1-7572-a9d4-36bff7b52514");
}

// --------------------------------------- 4. bindings coverage + card agreement

#[test]
fn the_bindings_cover_the_cards_and_the_lowered_cards_carry_the_live_values() {
    let store = fleet::capture_store();
    let ui = Mutex::new(FlowUi::default());
    let ctx = Ctx::new(&store, &ui);

    // Coverage: every declared id resolves (Some) — a deliberate no-live-value
    // slot resolves to Null (the authored copy stays), never None.
    for (copy_id, binding) in fleet::COPY_SLOTS {
        assert!(
            fleet::query_binding(&ctx, binding).is_some(),
            "{binding} (slot {copy_id}) must resolve"
        );
    }

    // The Fleet values the capture store pins: the roster count, the goal,
    // the one terminal word the store owns.
    assert_eq!(
        fleet::query_binding(&ctx, "fleet.title").unwrap(),
        serde_json::json!("Fleet · 3 peers")
    );
    assert_eq!(
        fleet::query_binding(&ctx, "fleet.goal").unwrap(),
        serde_json::json!("Fix steer queue")
    );
    // `review` sorts to row 2 (docs, review, tests) and the model closed it —
    // the one terminal word the store owns.
    assert_eq!(
        fleet::query_binding(&ctx, "fleet.peer2.status").unwrap(),
        serde_json::json!("Done")
    );
    assert!(fleet::query_binding(&ctx, "fleet.peer3.status").unwrap().is_null());
    assert!(fleet::query_binding(&ctx, "fleet.peer1.meta").unwrap().is_null());

    // The Tasks values: the running/settled commands and the four output
    // lines the store accumulated.
    assert_eq!(
        fleet::query_binding(&ctx, "tasks.run_cmd").unwrap(),
        serde_json::json!("cargo test -p octos-cli steer_queue")
    );
    assert_eq!(
        fleet::query_binding(&ctx, "tasks.log3").unwrap(),
        serde_json::json!("running 12 tests …")
    );

    // Card agreement: the LOWERED cards carry the live strings (the capture
    // PNGs cannot drift from the wiring — the f29c contract).
    let fleet_dsl = fleet::lower("autonomy-06", &ctx).expect("fleet lowers");
    for needle in ["Fleet · 3 peers", "Fix steer queue", "r6-smoke-is-not-here"] {
        if needle == "r6-smoke-is-not-here" {
            assert!(!fleet_dsl.contains(needle));
        } else {
            assert!(fleet_dsl.contains(needle), "fleet DSL must carry {needle:?}");
        }
    }
    let tasks_dsl = fleet::lower("autonomy-07", &ctx).expect("tasks lowers");
    for needle in [
        "cargo test -p octos-cli steer_queue",
        "running 12 tests …",
        "Compiling octos-cli v0.24.1 (/workspace/crates/octos-cli)",
    ] {
        assert!(tasks_dsl.contains(needle), "tasks DSL must carry {needle:?}");
    }
}
