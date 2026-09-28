//! F6 — `peer/prepare` + `peer/gather` fake-transport round-trips, and the
//! `peer/staged` / `peer/closed` handlers.
//!
//! The fake transport speaks the same `OutboundCommand::Request` /
//! oneshot-reply contract the real one does (same pattern as
//! `tests/client_core.rs`), so this proves the client's half of the wire
//! contract without a socket.
use std::sync::{Arc, Mutex};

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::{methods, RpcError};
use octos_app_transport::OutboundCommand;
use octoscode_store::Store;
use tokio::sync::mpsc;

use octoscode_client::domains::peer::{PeerGather, PeerGatherParams, PeerPrepare, PeerPrepareParams};
use octoscode_client::domains;
use octoscode_client::{Client, ClientError};

/// A fake transport: it owns the receiver half of the command channel and
/// answers every generic request through `responder`.
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

fn notification(method: &str, params: serde_json::Value) -> UiNotification {
    UiNotification::from_method_and_params(method, params)
        .expect("the test's params decode for this method")
}

// ------------------------------------------------------------- peer/prepare

#[tokio::test]
async fn peer_prepare_sends_the_web_frame_and_decodes_the_fleet() {
    let sent = Arc::new(Mutex::new(None));
    let seen = sent.clone();
    let tx = fake_transport(move |method, params| {
        assert_eq!(method, "peer/prepare");
        *seen.lock().unwrap() = Some(params.clone());
        Ok(serde_json::json!({
            "slug": "edison",
            "topic": "peer-edison",
            "profile_id": "octoscode",
            "cwd": "/tmp/peers/edison",
            "brief_path": "/tmp/peers/edison/brief.md",
            "peers": [{
                "slug": "edison",
                "topic": "peer-edison",
                "profile_id": "octoscode",
                "cwd": "/tmp/peers/edison",
                "brief_path": "/tmp/peers/edison/brief.md"
            }]
        }))
    });
    let client = Client::new(tx);
    let result = client
        .call::<PeerPrepare>(PeerPrepareParams {
            brief: "do the thing".into(),
            n: None,
            title: None,
            names: None,
            worktree: Some(true),
            cwd: None,
            session_id: "octoscode:main".into(),
            profile_id: "octoscode".into(),
        })
        .await
        .expect("peer/prepare round trip");

    // Params match what the web sends: trimmed brief, only present optionals.
    let params = sent.lock().unwrap().clone().expect("params captured");
    assert_eq!(params["brief"], "do the thing");
    assert_eq!(params["worktree"], true);
    assert_eq!(params["session_id"], "octoscode:main");
    assert_eq!(params["profile_id"], "octoscode");
    for absent in ["n", "title", "names", "cwd"] {
        assert!(params.get(absent).is_none(), "{absent} must be omitted");
    }

    assert_eq!(result.slug, "edison");
    assert_eq!(result.topic, "peer-edison");
    assert_eq!(result.peers.len(), 1);
    assert_eq!(result.peers[0].brief_path, "/tmp/peers/edison/brief.md");
}

#[tokio::test]
async fn peer_prepare_rpc_error_carries_the_method_name() {
    let tx = fake_transport(|_m, _p| {
        Err(RpcError { code: -32602, message: "brief is required".into(), data: None })
    });
    let client = Client::new(tx);
    match client
        .call::<PeerPrepare>(PeerPrepareParams {
            brief: String::new(),
            session_id: "octoscode:main".into(),
            profile_id: "octoscode".into(),
            ..Default::default()
        })
        .await
    {
        Err(ClientError::Rpc { method, error }) => {
            assert_eq!(method, "peer/prepare");
            assert_eq!(error.code, -32602);
        }
        other => panic!("expected an RPC error, got {other:?}"),
    }
}

// -------------------------------------------------------------- peer/gather

#[tokio::test]
async fn peer_gather_sends_the_web_frame_and_decodes_rows() {
    let sent = Arc::new(Mutex::new(None));
    let seen = sent.clone();
    let tx = fake_transport(move |method, params| {
        assert_eq!(method, "peer/gather");
        *seen.lock().unwrap() = Some(params.clone());
        Ok(serde_json::json!({
            "profile_id": "octoscode",
            "peers": [{
                "slug": "edison",
                "topic": "peer-edison",
                "brief": "do the thing",
                "brief_truncated": false,
                "result": "done",
                "result_truncated": false,
                "result_updated_unix": 1750000000,
                "has_worktree": true,
                "closed": false
            }]
        }))
    });
    let client = Client::new(tx);
    let result = client
        .call::<PeerGather>(PeerGatherParams {
            session_id: "octoscode:main".into(),
            profile_id: "octoscode".into(),
            slugs: None,
        })
        .await
        .expect("peer/gather round trip");

    let params = sent.lock().unwrap().clone().expect("params captured");
    assert_eq!(params["session_id"], "octoscode:main");
    assert_eq!(params["profile_id"], "octoscode");
    assert!(params.get("slugs").is_none(), "an unfiltered gather omits slugs");

    assert_eq!(result.profile_id, "octoscode");
    assert_eq!(result.peers[0].slug, "edison");
    assert_eq!(result.peers[0].result.as_deref(), Some("done"));
    assert!(result.peers[0].has_worktree);
}

#[tokio::test]
async fn peer_gather_sends_slugs_only_when_filtered() {
    let sent = Arc::new(Mutex::new(None));
    let seen = sent.clone();
    let tx = fake_transport(move |_m, params| {
        *seen.lock().unwrap() = Some(params.clone());
        Ok(serde_json::json!({"profile_id": "octoscode", "peers": []}))
    });
    let client = Client::new(tx);
    let _ = client
        .call::<PeerGather>(PeerGatherParams {
            session_id: "s".into(),
            profile_id: "octoscode".into(),
            slugs: Some(vec!["edison".into()]),
        })
        .await
        .expect("filtered gather");
    let params = sent.lock().unwrap().clone().unwrap();
    assert_eq!(params["slugs"], serde_json::json!(["edison"]));
}

#[tokio::test]
async fn peer_gather_decode_error_carries_the_method_name() {
    let tx = fake_transport(|_m, _p| Ok(serde_json::json!({"unexpected": true})));
    let client = Client::new(tx);
    match client
        .call::<PeerGather>(PeerGatherParams {
            session_id: "s".into(),
            profile_id: "p".into(),
            slugs: None,
        })
        .await
    {
        Err(ClientError::Decode { method, reason }) => {
            assert_eq!(method, "peer/gather");
            assert!(!reason.is_empty());
        }
        other => panic!("expected a decode error, got {other:?}"),
    }
}

// ------------------------------------------------------------- notifications

#[test]
fn register_wires_both_peer_notifications() {
    let store = Arc::new(Store::new());
    let mut reg = octoscode_client::Registry::new();
    domains::peer::register(&mut reg, store);
    assert!(reg.handles(methods::PEER_STAGED));
    assert!(reg.handles(methods::PEER_CLOSED));
}

#[test]
fn peer_staged_reaches_the_store_roster() {
    let store = Arc::new(Store::new());
    let mut reg = octoscode_client::Registry::new();
    domains::peer::register(&mut reg, store.clone());
    let n = notification(
        methods::PEER_STAGED,
        serde_json::json!({
            "session_id": "octoscode:main",
            "topic": "peer-edison",
            "slug": "edison",
            "brief": "do the thing",
            "brief_path": "/tmp/peers/edison/brief.md",
            "cwd": "/tmp/peers/edison",
            "profile_id": "octoscode"
        }),
    );
    assert!(reg.dispatch(&n));
    let peers = store.domains.peer.list();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].name, "edison");
    assert!(!peers[0].closed);
    assert_eq!(peers[0].topic.as_deref(), Some("peer-edison"));
    assert_eq!(peers[0].origin_session_id.as_deref(), Some("octoscode:main"));
    assert_eq!(store.domains.peer.open_count(), 1);
    assert_eq!(store.seen_count(methods::PEER_STAGED), 1);
}

#[test]
fn peer_closed_marks_the_staged_peer_closed() {
    let store = Arc::new(Store::new());
    let mut reg = octoscode_client::Registry::new();
    domains::peer::register(&mut reg, store.clone());
    let staged = notification(
        methods::PEER_STAGED,
        serde_json::json!({
            "session_id": "octoscode:main",
            "topic": "peer-edison",
            "slug": "edison",
            "brief": "b",
            "brief_path": "/tmp/p/edison/brief.md",
            "cwd": "/tmp/p/edison",
            "profile_id": "octoscode"
        }),
    );
    assert!(reg.dispatch(&staged));
    let closed = notification(
        methods::PEER_CLOSED,
        serde_json::json!({
            "session_id": "octoscode:main",
            "topic": "peer-edison",
            "slug": "edison",
            "profile_id": "octoscode"
        }),
    );
    assert!(reg.dispatch(&closed));
    assert_eq!(store.domains.peer.open_count(), 0);
    assert!(store.domains.peer.get("edison").unwrap().closed);
    assert_eq!(store.seen_count(methods::PEER_CLOSED), 1);
}

#[test]
fn a_replayed_peer_staged_does_not_reopen_a_closed_peer() {
    // `peer/staged` is durable: reconnect replay redelivers it. A peer the
    // model has since closed must stay closed.
    let store = Arc::new(Store::new());
    let mut reg = octoscode_client::Registry::new();
    domains::peer::register(&mut reg, store.clone());
    let event = || {
        notification(
            methods::PEER_STAGED,
            serde_json::json!({
                "session_id": "octoscode:main",
                "topic": "peer-edison",
                "slug": "edison",
                "brief": "b",
                "brief_path": "/tmp/p/edison/brief.md",
                "cwd": "/tmp/p/edison",
                "profile_id": "octoscode"
            }),
        )
    };
    assert!(reg.dispatch(&event()));
    assert!(reg.dispatch(&notification(
        methods::PEER_CLOSED,
        serde_json::json!({
            "session_id": "octoscode:main",
            "topic": "peer-edison",
            "slug": "edison",
            "profile_id": "octoscode"
        }),
    )));
    assert!(reg.dispatch(&event()));
    assert!(store.domains.peer.get("edison").unwrap().closed);
    assert_eq!(store.domains.peer.open_count(), 0);
}
