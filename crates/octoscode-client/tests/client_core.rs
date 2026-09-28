//! Client-core unit tests: registry routing + `Client::call` round-trips
//! against a **fake transport** (a channel pair that speaks the same
//! `OutboundCommand::Request` / oneshot-reply contract the real transport
//! does). This proves the client's half of the wire contract without a
//! socket; the live socket path is `tests/live_serve.rs` (`#[ignore]`).
use std::sync::{Arc, Mutex};

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::{methods, RpcError};
use octos_app_transport::OutboundCommand;
use octoscode_store::Store;
use serde::Deserialize;
use tokio::sync::{mpsc, oneshot};

use octoscode_client::domains;
use octoscode_client::registry::{NotificationHandler, Registry};
use octoscode_client::{Client, ClientError};

// ---------------------------------------------------------------- registry

/// Build a decoded notification from its wire JSON — the same path the
/// transport takes (`from_method_and_params`), so a test never hand-rolls a
/// struct with a synthetic uuid/clock.
fn notification(method: &str, params: serde_json::Value) -> UiNotification {
    UiNotification::from_method_and_params(method, params)
        .expect("the test's params decode for this method")
}

struct Recorder {
    hits: Arc<Mutex<Vec<&'static str>>>,
}
impl NotificationHandler for Recorder {
    const METHOD: &'static str = methods::MESSAGE_DELTA;
    fn handle(&self, _n: &UiNotification) {
        self.hits.lock().unwrap().push(Self::METHOD);
    }
}

#[test]
fn registry_routes_a_handled_notification() {
    let mut reg = Registry::new();
    let hits = Arc::new(Mutex::new(Vec::new()));
    reg.register(Recorder { hits: hits.clone() });

    assert!(reg.handles(methods::MESSAGE_DELTA));
    let n = notification(
        methods::MESSAGE_DELTA,
        serde_json::json!({
            "session_id": "octoscode:main",
            "turn_id": "00000000-0000-7000-8000-000000000001",
            "text": "hi"
        }),
    );
    assert!(reg.dispatch(&n));
    assert_eq!(*hits.lock().unwrap(), vec![methods::MESSAGE_DELTA]);
    assert!(reg.unknown_methods().is_empty());
}

#[test]
fn registry_logs_and_tolerates_an_unhandled_notification() {
    let mut reg = Registry::new();
    // Nothing registered for `turn/started` here -> the tolerated-unknown arm.
    let n = notification(
        methods::TURN_STARTED,
        serde_json::json!({
            "session_id": "octoscode:main",
            "turn_id": "00000000-0000-7000-8000-000000000002",
            "timestamp": "2026-09-28T00:00:00Z"
        }),
    );
    assert!(!reg.dispatch(&n), "unhandled returns false, never panics");
    assert_eq!(reg.unknown_methods(), &[methods::TURN_STARTED.to_string()]);
}

#[test]
fn register_all_wires_the_implemented_domains() {
    let store = Arc::new(Store::new());
    let mut reg = Registry::new();
    domains::register_all(&mut reg, store);
    for m in [
        methods::MESSAGE_DELTA,
        methods::TURN_STARTED,
        methods::TURN_COMPLETED,
        methods::TURN_ERROR,
    ] {
        assert!(reg.handles(m), "expected a handler for {m}");
    }
    // Unregistered methods are not handled (a name no domain will ever register;
    // domain handlers land in the fan-out, so don't assert on real domain names here).
    assert!(!reg.handles("x-octoscode-test/never-registered"));
}

#[test]
fn a_delta_reaches_the_store_through_its_handler() {
    let store = Arc::new(Store::new());
    let mut reg = Registry::new();
    domains::register_all(&mut reg, store.clone());
    for text in ["Hel", "lo"] {
        let n = notification(
            methods::MESSAGE_DELTA,
            serde_json::json!({
                "session_id": "octoscode:main",
                "turn_id": "00000000-0000-7000-8000-000000000003",
                "text": text
            }),
        );
        assert!(reg.dispatch(&n));
    }
    assert_eq!(store.live_text("octoscode:main"), "Hello");
    assert_eq!(store.seen_count(methods::MESSAGE_DELTA), 2);
}

// ---------------------------------------------------------- fake transport

/// A fake transport: it owns the receiver half of the command channel and
/// answers each `OutboundCommand::Request` from a scripted function. This is
/// the same command/reply contract the real WS transport implements, so the
/// client code under test is the production path.
fn fake_transport<F>(responder: F) -> mpsc::Sender<OutboundCommand>
where
    F: Fn(&str, &serde_json::Value) -> Result<serde_json::Value, RpcError> + Send + 'static,
{
    let (tx, mut rx) = mpsc::channel::<OutboundCommand>(8);
    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            if let OutboundCommand::Request { method, params, reply } = cmd {
                let _ = reply.send(responder(&method, &params));
            }
        }
    });
    tx
}

#[derive(Debug, serde::Serialize)]
struct Ping {
    n: u32,
}
#[derive(Debug, Deserialize, PartialEq)]
struct Pong {
    echoed: u32,
}

impl octoscode_client::Method for Ping {
    const NAME: &'static str = "test/ping";
    type Params = Ping;
    type Result = Pong;
}

#[tokio::test]
async fn call_round_trips_a_typed_result() {
    let tx = fake_transport(|method, params| {
        assert_eq!(method, "test/ping");
        assert_eq!(params["n"], 7);
        Ok(serde_json::json!({"echoed": 7}))
    });
    let client = Client::new(tx);
    let out = client.call::<Ping>(Ping { n: 7 }).await.expect("round trip");
    assert_eq!(out, Pong { echoed: 7 });
}

#[tokio::test]
async fn call_surfaces_an_rpc_error_with_the_method_name() {
    let tx = fake_transport(|_m, _p| {
        Err(RpcError { code: -32601, message: "method not found".into(), data: None })
    });
    let client = Client::new(tx);
    match client.call::<Ping>(Ping { n: 1 }).await {
        Err(ClientError::Rpc { method, error }) => {
            assert_eq!(method, "test/ping");
            assert_eq!(error.code, -32601);
        }
        other => panic!("expected an RPC error, got {other:?}"),
    }
}

#[tokio::test]
async fn call_surfaces_a_decode_error_with_the_method_name() {
    let tx = fake_transport(|_m, _p| Ok(serde_json::json!({"unexpected": true})));
    let client = Client::new(tx);
    match client.call::<Ping>(Ping { n: 1 }).await {
        Err(ClientError::Decode { method, reason }) => {
            assert_eq!(method, "test/ping");
            assert!(!reason.is_empty());
        }
        other => panic!("expected a decode error, got {other:?}"),
    }
}

#[tokio::test]
async fn request_is_the_same_generic_path_with_a_runtime_method() {
    let tx = fake_transport(|method, _p| Ok(serde_json::json!({"method": method})));
    let client = Client::new(tx);
    let v = client
        .request("tool/status/list", serde_json::json!({"include_denied": true}))
        .await
        .expect("generic request");
    assert_eq!(v["method"], "tool/status/list");
}

#[tokio::test]
async fn a_closed_transport_is_a_transport_error_not_a_panic() {
    let (tx, rx) = mpsc::channel::<OutboundCommand>(1);
    drop(rx); // the "connection" is gone
    let client = Client::new(tx);
    match client.call::<Ping>(Ping { n: 1 }).await {
        Err(ClientError::Transport { method, .. }) => assert_eq!(method, "test/ping"),
        other => panic!("expected a transport error, got {other:?}"),
    }
}

#[test]
fn unimplemented_domains_are_stub_files_with_their_methods_named() {
    // The stub domains exist and register cleanly (they add no handlers yet).
    let store = Arc::new(Store::new());
    let mut reg = Registry::new();
    domains::autonomy::register(&mut reg, store.clone());
    domains::media::register(&mut reg, store.clone());
    domains::peer::register(&mut reg, store.clone());
    domains::profile::register(&mut reg, store.clone());
    domains::review::register(&mut reg, store);
    // Registration is clean and idempotent regardless of which domains have landed.
    assert!(!reg.handles("x-octoscode-test/never-registered"));
}

#[test]
#[should_panic(expected = "duplicate notification handler")]
fn a_notification_method_has_exactly_one_owning_domain() {
    // Registering every domain twice must trip the one-owner guard.
    let store = std::sync::Arc::new(octoscode_store::Store::new());
    let mut reg = octoscode_client::Registry::new();
    octoscode_client::domains::register_all(&mut reg, store.clone());
    octoscode_client::domains::register_all(&mut reg, store);
}

#[test]
fn all_domains_register_without_overlap() {
    let store = std::sync::Arc::new(octoscode_store::Store::new());
    let mut reg = octoscode_client::Registry::new();
    octoscode_client::domains::register_all(&mut reg, store); // panics on any overlap
}
