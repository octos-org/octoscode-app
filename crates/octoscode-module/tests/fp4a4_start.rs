//! Card #P4a4 (A10 rewrite) — the fleet Start control flow (parity rows
//! `Start control flow`, `Fleet Start = acquire (CAS) -> await proof ->
//! dispatch once`) through the PRODUCTION path, `fleet_driver::start` (the
//! chain the Fleet pane's Start job runs).
//!
//! The old card locked in a wire that was NOT the web's: an acquire of
//! `{session_id, slug}` with no CAS, a camelCase dispatch (`driverId`,
//! `controlToken`, `kickoffInput`) with the literal model `"inherit"`, and
//! the raw brief as the kickoff. The web's chain (`fleet-start-sequencer.ts`,
//! `external-driver.ts:749-755`, `external-driver-peer-control.ts:632-644`)
//! is: acquire `{session_id, driver_id, expected_revision, lease_seconds:
//! 120}` CAS on the walked revision → `peer/prepare` → EXACTLY ONE snake_case
//! `peer/dispatch` carrying the Start's operation id, the REQUESTED lane, a
//! `new_brief` target titled with the staged slug, and the kickoff prompt.
//! The responder answers in those shapes (the faithful fixture's,
//! `a10-fleet-driver-synthetic.jsonl`), echoing the request's ids.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use octoscode_module::flow::Conversation;
use octoscode_module::screens::board3::{fleetview, host};
use octoscode_module::screens::fleet_driver::{self, StartOutcome};
use octoscode_module::screens::peers;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

/// The fixture's `session/open` (the RECORDED r6 reply + the advertised
/// external-driver methods and `external_driver_v1`); `advertise: false`
/// strips the driver methods and the feature again.
fn opened(advertise: bool) -> Value {
    let path = format!(
        "{}/../octoscode-client/tests/fixtures/a10-fleet-driver-synthetic.jsonl",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let mut opened = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["dir"] == "in" && v["method"] == "session/open")
        .map(|v| v["body"].clone())
        .expect("fixture session/open");
    if !advertise {
        let caps = &mut opened["capabilities"];
        caps["supported_features"].as_array_mut().unwrap().retain(|f| f != "external_driver_v1");
        caps["supported_methods"].as_array_mut().unwrap().retain(|m| {
            let m = m.as_str().unwrap_or("");
            !m.starts_with("session/driver/") && m != "peer/dispatch" && m != "peer/control"
        });
    }
    opened
}

struct Server {
    url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Server {
    /// `refuse`: answer `peer/dispatch` with this typed refusal kind.
    async fn start(advertise: bool, refuse: Option<&'static str>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let rec = seen.clone();
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (mut tx, mut rx) = ws.split();
            while let Some(Ok(msg)) = rx.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                let Some(id) = v.get("id").cloned() else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let p = v["params"].clone();
                rec.lock().unwrap().push((method.clone(), p.clone()));
                let reply: Result<Value, Value> = match method.as_str() {
                    "session/open" => {
                        let mut o = opened(advertise);
                        o["session_id"] = p["session_id"].clone();
                        Ok(json!({ "opened": o }))
                    }
                    "session/list" => Ok(json!({ "sessions": [] })),
                    "session/driver/get" => Ok(json!({
                        "mode": "external", "recovery": "none",
                        "binding": {"driver_id": "someone-else", "epoch": 7, "revision": 42, "lease_expires_at_ms": 0},
                        "operations": {"items": [], "snapshot": "s", "observed_revision": "42", "complete": true, "next_cursor": null}
                    })),
                    "session/driver/acquire" => Ok(json!({
                        "control_token": "tok-1", "recovery": "none",
                        "binding": {"driver_id": p["driver_id"], "epoch": 7, "revision": 43, "lease_expires_at_ms": 1770000000000u64}
                    })),
                    "peer/prepare" => Ok(json!({
                        "slug": "r6-smoke", "topic": "peer-r6-smoke", "profile_id": "dsflash",
                        "cwd": "/home/user/src/octos", "brief_path": "/home/user/.octos/peers/r6-smoke/brief.md"
                    })),
                    "peer/dispatch" => match refuse {
                        Some(kind) => Err(json!({"code": -32602, "message": "driver operation refused: peer/dispatch", "data": {"kind": kind}})),
                        None => Ok(json!({
                            "operation_id": p["operation_id"], "state": "accepted", "model": "glm-5.3",
                            "model_lane": p["model"], "workspace_root": "/home/user/src/octos", "scoped_goal": null,
                            "adopted_turn_id": "00000000-0000-4000-8000-0000000000d1",
                            "adopted_session_id": "dsflash:main#peer-r6-smoke", "slug": "r6-smoke",
                            "duplicate": false, "accepted_at_ms": 1, "payload_digest": "d"
                        })),
                    },
                    _ => Ok(json!({})),
                };
                let frame = match reply {
                    Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
                    Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
                };
                let _ = tx.send(Message::Text(frame.to_string().into())).await;
            }
        });
        Self { url: format!("http://{addr}"), seen }
    }

    fn sent(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }
}

async fn connect(server: &Server) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&server.url, "dummy", "dsflash", None, None).expect("connect");
    let conv = Arc::new(conv);
    let drv = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            peers::note_transport_event(&drv, &evt);
            let _ = drv.on_event(evt);
        }
    });
    conv.open_workspace(None).await.expect("open");
    for _ in 0..100 {
        if !conv.store.domains.config.supported_methods().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    conv
}

fn serial() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

fn lanes() -> Vec<String> {
    vec!["glm-53".into()]
}

#[tokio::test]
async fn the_gate_is_fail_closed_and_the_chain_sends_exactly_one_dispatch() {
    let _s = serial();
    fleet_driver::reset_seat();
    std::env::set_var("OCTOSCODE_DRIVER_ID_PATH", std::env::temp_dir().join("fp4a4-driver-id"));
    // WITHOUT the methods + feature: nothing is admitted, nothing is sent.
    let closed = Server::start(false, None).await;
    let conv = connect(&closed).await;
    fleet_driver::load_inventory(&conv).await.expect("an unadvertised walk is a no-op");
    assert!(!fleet_driver::control_ready(&conv.store));
    let out = fleet_driver::start(&conv, "op-1", "glm-53", &lanes(), "run the queue", &mut None).await;
    assert!(matches!(out, StartOutcome::Refused { .. }), "{out:?}");
    assert!(closed.sent("session/driver/acquire").is_empty() && closed.sent("peer/dispatch").is_empty());

    // WITH them: walk → acquire (CAS on the walked revision) → prepare →
    // EXACTLY ONE dispatch, the web's snake_case frame.
    fleet_driver::reset_seat();
    let server = Server::start(true, None).await;
    let conv = connect(&server).await;
    fleet_driver::load_inventory(&conv).await.expect("walk");
    assert!(fleet_driver::control_ready(&conv.store));
    let mut staged = None;
    let out = fleet_driver::start(&conv, "op-first", "glm-53", &lanes(), "run the queue", &mut staged).await;
    assert!(matches!(&out, StartOutcome::Accepted { operation_id, .. } if operation_id == "op-first"), "{out:?}");
    let acquire = &server.sent("session/driver/acquire")[0];
    assert_eq!(acquire["session_id"], "dsflash:main");
    assert_eq!(acquire["expected_revision"], 42, "CAS on the observed revision");
    assert_eq!(acquire["lease_seconds"], 120);
    assert!(acquire.get("slug").is_none(), "no slug-addressed seat");
    assert_eq!(server.sent("peer/dispatch").len(), 1, "one activation, one dispatch");
    let d = &server.sent("peer/dispatch")[0];
    assert_eq!(d["driver_id"], acquire["driver_id"]);
    assert_eq!(d["epoch"], 7);
    assert_eq!(d["control_token"], "tok-1");
    assert_eq!(d["operation_id"], "op-first");
    assert_eq!(d["model"], "glm-53", "the REQUESTED lane, never \"inherit\"");
    assert_eq!(d["dispatch"], json!({"kind": "new_brief", "brief": "run the queue", "title": "r6-smoke"}));
    assert!(d["kickoff_input"][0]["text"].as_str().unwrap().starts_with("You are a peer agent. Your brief:"));
    assert!(d.get("driverId").is_none() && d.get("kickoffInput").is_none(), "no camelCase wire");
    // A SECOND Start (a new staging) carries a DISTINCT operation id; the
    // held seat is reused (no second acquire).
    let out = fleet_driver::start(&conv, "op-second", "glm-53", &lanes(), "run it again", &mut None).await;
    assert!(matches!(&out, StartOutcome::Accepted { operation_id, .. } if operation_id == "op-second"));
    let ds = server.sent("peer/dispatch");
    assert_eq!(ds.len(), 2);
    assert_ne!(ds[0]["operation_id"], ds[1]["operation_id"]);
    assert_eq!(server.sent("session/driver/acquire").len(), 1, "the held seat is reused");
}

#[tokio::test]
async fn a_refused_dispatch_keeps_the_brief_for_retry() {
    let _s = serial();
    fleet_driver::reset_seat();
    host::reset();
    std::env::set_var("OCTOSCODE_DRIVER_ID_PATH", std::env::temp_dir().join("fp4a4-driver-id"));
    let server = Server::start(true, Some("driver_operation_conflict")).await;
    let conv = connect(&server).await;
    fleet_driver::load_inventory(&conv).await.expect("walk");
    {
        let mut st = host::state();
        st.fleet.lanes = vec![fleetview::LaneInfo { key: "glm-53".into(), ..Default::default() }];
        st.fleet.lane = "glm-53".into();
        st.fleet.brief = "hold the queue".into();
    }
    let job = match host::perform("b3.fleet.start", 0, &conv.store) {
        host::Outcome::Spawn(j) => j,
        other => panic!("{other:?}"),
    };
    host::run(job, &conv).await.expect("settles");
    assert_eq!(server.sent("peer/dispatch").len(), 1, "attempted exactly once");
    let st = host::state();
    assert!(matches!(&st.fleet.start, fleetview::StartState::Failed { kind, .. } if kind == "driver_operation_conflict"));
    assert_eq!(st.fleet.brief, "hold the queue", "a refused Start keeps the brief visible for retry");
    assert_eq!(
        fleet_driver::dispatch_refusal_label("driver_operation_conflict"),
        "A different request already used this id — nothing was sent"
    );
}
