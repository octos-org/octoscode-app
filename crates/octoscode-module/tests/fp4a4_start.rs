//! Card #P4a4 — the fleet Start control flow (parity row `Start control
//! flow`) through the PRODUCTION path.
//!
//! One activation ⇒ the driver seat is acquired and then EXACTLY ONE
//! `peer/dispatch` goes out, in the web's wire encoding verbatim
//! (`control/peer-dispatch-commands.ts:90-106`: camelCase fence echo, the
//! operation id minted per activation, the REQUESTED model lane echoed,
//! `dispatch.kind = "new_brief"`, the kickoff input carried). A SECOND Start
//! mints a DISTINCT operation id. A REFUSED dispatch keeps the brief in the
//! composer for retry ("a refused Start keeps the brief visible for retry",
//! FleetView.harness.test.tsx:372); only a SUCCESS consumes it. The
//! fail-closed gate (`peerDispatchAdmitted`, :47) admits nothing without the
//! `external_driver_v1` feature.
//!
//! The responder answers the RECORDED-style bodies (an acquire fence, a
//! dispatch ack) — the same harness shape as f30c_replay, plus an error arm
//! for the refusal case.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, FlowUi};
use octoscode_module::screens::fleet;
use octoscode_store::Store;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

/// A fake WS server: each request is answered with its recorded body (by
/// method), or `{result: null}`; methods in `error_for` get a JSON-RPC ERROR
/// instead (the refusal case). Records every (method, params) it receives.
struct ReplayServer {
    url: String,
    seen: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
}

impl ReplayServer {
    async fn start(bodies: Vec<(String, serde_json::Value)>, error_for: Vec<String>) -> Self {
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
                let reply = if error_for.iter().any(|m| *m == method) {
                    serde_json::json!({
                        "jsonrpc": "2.0", "id": id,
                        "error": {"code": -32000, "message": "no seat for that peer"}
                    })
                } else {
                    let result = bodies
                        .iter()
                        .find(|(m, _)| *m == method)
                        .map(|(_, b)| b.clone())
                        .unwrap_or(serde_json::Value::Null);
                    serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result})
                };
                let _ = tx.send(Message::Text(reply.to_string().into())).await;
            }
        });
        Self {
            url: format!("ws://{addr}"),
            seen,
        }
    }

    fn sent(&self, method: &str) -> Vec<serde_json::Value> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|(m, _)| m == method)
            .map(|(_, p)| p.clone())
            .collect()
    }

    fn count(&self, method: &str) -> usize {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).count()
    }
}

fn store_with_peer() -> std::sync::Arc<Store> {
    // Returns Arc: Ctx::new takes &Arc<Store>; the &Store APIs deref-coerce.
    let store = Store::new();
    store.domains.session.set_active(Some("dsflash:main".into()));
    store.domains.peer.upsert(octoscode_store::domains::peer::Peer::named("r6-smoke"));
    std::sync::Arc::new(store)
}

/// Connect + drain, the f30c glue verbatim.
fn connect(
    rt: &tokio::runtime::Runtime,
    server: &ReplayServer,
) -> (
    Arc<Conversation>,
    tokio::sync::mpsc::Receiver<octos_app_transport::TransportEvent>,
) {
    let (conv, evt_rx) = rt
        .block_on(async { Conversation::connect(&server.url, "mapb-dummy", "dsflash", None, None) })
        .expect("the conversation connects to the replay server");
    let conv = Arc::new(conv);
    // run_start reads the session from the CONVERSATION's store (in
    // production it is the same store lib.rs passes to spawn); the test's
    // own store is a separate instance, so set the active session on both.
    conv.store
        .domains
        .session
        .set_active(Some("dsflash:main".into()));
    (conv, evt_rx)
}

fn poll_until(server: &ReplayServer, rt: &tokio::runtime::Runtime, want: usize) {
    rt.block_on(async {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if server.count("peer/dispatch") >= want
                || tokio::time::Instant::now() > deadline
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
}

#[test]
fn the_gate_is_fail_closed_and_the_chain_sends_exactly_one_dispatch() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("runtime");
    let store = store_with_peer();
    // The fence the (recorded-style) acquire hands back — echoed verbatim
    // into the dispatch frame.
    let bodies = vec![(
        "session/driver/acquire".to_owned(),
        serde_json::json!({"driverId": "drv-1", "epoch": 7, "controlToken": "tok-1"}),
    )];
    let server = rt.block_on(ReplayServer::start(bodies, vec![]));
    let (conv, mut evt_rx) = connect(&rt, &server);
    {
        let drain = conv.clone();
        rt.spawn(async move {
            while let Some(evt) = evt_rx.recv().await {
                let _ = drain.on_event(evt);
            }
        });
    }
    let ui = std::sync::Arc::new(Mutex::new(FlowUi::default()));

    // WITHOUT the feature the gate admits nothing — locally, before any wire
    // traffic (the web's peerDispatchAdmitted, fail-closed).
    assert!(!fleet::start_admitted(&store), "no capabilities yet");
    assert!(
        fleet::action_params("peer.start", 0, &store, "run the queue").is_none(),
        "the gate refuses before a frame is built"
    );
    let ctx = Ctx::new(&store, &ui);
    fleet::spawn(
        fleet::resolve("peer.start", 0, &ctx),
        &rt,
        &conv,
        &ui,
        &store,
    );
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(server.count("session/driver/acquire"), 0, "no acquire without the gate");
    assert_eq!(server.count("peer/dispatch"), 0, "no dispatch without the gate");

    // WITH the feature the chain runs: acquire -> exactly ONE dispatch.
    store.set_capabilities(vec!["external_driver_v1".into()]);
    assert!(fleet::start_admitted(&store));
    let (method, params) =
        fleet::action_params("peer.start", 0, &store, "run the queue").expect("admitted");
    assert_eq!(method, "peer.start");
    assert_eq!(params["slug"], "r6-smoke");
    ui.lock().unwrap().set_draft_inner("run the queue");
    fleet::spawn(
        fleet::resolve("peer.start", 0, &ctx),
        &rt,
        &conv,
        &ui,
        &store,
    );
    poll_until(&server, &rt, 1);

    // The fence was acquired for THIS session+slug…
    let acquire = &server.sent("session/driver/acquire")[0];
    assert_eq!(acquire["session_id"], "dsflash:main");
    assert_eq!(acquire["slug"], "r6-smoke");
    // …and EXACTLY ONE dispatch went out, the web's frame verbatim.
    assert_eq!(server.count("peer/dispatch"), 1, "one activation, one dispatch");
    let dispatch = &server.sent("peer/dispatch")[0];
    assert_eq!(dispatch["driverId"], "drv-1");
    assert_eq!(dispatch["epoch"], 7);
    assert_eq!(dispatch["controlToken"], "tok-1");
    assert_eq!(dispatch["model"], "inherit", "the REQUESTED lane is echoed");
    assert_eq!(dispatch["dispatch"]["kind"], "new_brief");
    assert_eq!(dispatch["dispatch"]["brief"], "run the queue");
    assert_eq!(dispatch["dispatch"]["title"], "r6-smoke");
    assert_eq!(dispatch["kickoffInput"][0]["kind"], "text");
    assert_eq!(dispatch["kickoffInput"][0]["text"], "run the queue");
    let first_op = dispatch["operationId"].as_str().expect("operation id").to_owned();
    assert!(!first_op.is_empty());

    // SUCCESS consumes the brief (the composer is clear).
    assert!(ui.lock().unwrap().draft().is_empty(), "a successful Start consumes the brief");

    // A SECOND Start mints a DISTINCT operation id (the row's clause).
    ui.lock().unwrap().set_draft_inner("run the queue again");
    fleet::spawn(
        fleet::resolve("peer.start", 0, &ctx),
        &rt,
        &conv,
        &ui,
        &store,
    );
    poll_until(&server, &rt, 2);
    let dispatches = server.sent("peer/dispatch");
    assert_eq!(dispatches.len(), 2);
    let second_op = dispatches[1]["operationId"].as_str().expect("operation id").to_owned();
    assert_ne!(first_op, second_op, "a second Start mints a distinct operation id");
}

#[test]
fn a_refused_dispatch_keeps_the_brief_for_retry() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("runtime");
    let store = store_with_peer();
    store.set_capabilities(vec!["external_driver_v1".into()]);
    let bodies = vec![(
        "session/driver/acquire".to_owned(),
        serde_json::json!({"driverId": "drv-2", "epoch": 3, "controlToken": "tok-2"}),
    )];
    // peer/dispatch REFUSES (a JSON-RPC error) — the seat was taken.
    let server = rt.block_on(ReplayServer::start(
        bodies,
        vec!["peer/dispatch".to_owned()],
    ));
    let (conv, mut evt_rx) = connect(&rt, &server);
    {
        let drain = conv.clone();
        rt.spawn(async move {
            while let Some(evt) = evt_rx.recv().await {
                let _ = drain.on_event(evt);
            }
        });
    }
    let ui = std::sync::Arc::new(Mutex::new(FlowUi::default()));
    let ctx = Ctx::new(&store, &ui);

    ui.lock().unwrap().set_draft_inner("hold the queue");
    fleet::spawn(
        fleet::resolve("peer.start", 0, &ctx),
        &rt,
        &conv,
        &ui,
        &store,
    );
    poll_until(&server, &rt, 1);

    // The dispatch WAS attempted (exactly once)…
    assert_eq!(server.count("peer/dispatch"), 1);
    // …and its refusal left the brief IN the composer for retry — task
    // words, never protocol vocabulary, on the log; the draft is untouched.
    assert_eq!(
        ui.lock().unwrap().draft(),
        "hold the queue",
        "a refused Start keeps the brief visible for retry"
    );
}
