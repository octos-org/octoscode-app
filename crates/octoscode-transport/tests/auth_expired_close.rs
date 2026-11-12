//! A server close with 1008 `auth_expired` ends the WS transport: it reports
//! `AuthExpired` then `Failed` and never re-dials with the refused bearer.
//! Any other close still reconnects.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::SinkExt;
use octos_app_transport::{
    ws, Capabilities, ConnectionState, ProfileId, SecretString, TransportConfig, TransportEvent,
};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use url::Url;

/// A server that upgrades every connection and closes it at once with
/// `code`/`reason`. Returns its address and how many upgrades it accepted.
async fn closing_server(code: CloseCode, reason: &'static str) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr").to_string();
    let accepted = Arc::new(AtomicUsize::new(0));
    let count = accepted.clone();
    tokio::spawn(async move {
        while let Ok((sock, _)) = listener.accept().await {
            count.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let Ok(mut ws) = accept_async(sock).await else {
                    return;
                };
                let _ = ws
                    .send(tokio_tungstenite::tungstenite::Message::Close(Some(
                        CloseFrame {
                            code,
                            reason: reason.into(),
                        },
                    )))
                    .await;
                tokio::time::sleep(Duration::from_millis(200)).await;
            });
        }
    });
    (addr, accepted)
}

fn config(addr: &str) -> TransportConfig {
    TransportConfig {
        base_url: Url::parse(&format!("http://{addr}")).unwrap(),
        bearer: SecretString::new("tk-expired"),
        profile_id: ProfileId::new(""),
        cursor: None,
        cursor_file: None,
        requested_capabilities: Capabilities::requested(),
        workspace_cwd: None,
        local_kernel: false,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn an_auth_expired_close_fails_the_transport_without_a_redial() {
    let (addr, accepted) = closing_server(CloseCode::Policy, "auth_expired").await;
    let (_cmd_tx, mut events) = ws::spawn(config(&addr));

    let mut seen = Vec::new();
    let ended = tokio::time::timeout(Duration::from_secs(5), async {
        // The task exits after Failed: the channel closes.
        while let Some(evt) = events.recv().await {
            seen.push(format!("{evt:?}"));
        }
    })
    .await;
    assert!(ended.is_ok(), "the transport kept running: {seen:?}");

    let expired = seen
        .iter()
        .position(|e| e == "AuthExpired")
        .expect("AuthExpired reported");
    assert_eq!(
        seen.get(expired + 1).map(String::as_str),
        Some(
            format!(
                "{:?}",
                TransportEvent::ConnectionState(ConnectionState::Failed)
            )
            .as_str()
        ),
        "Failed follows AuthExpired: {seen:?}"
    );
    assert!(
        !seen.iter().any(|e| e.contains("Reconnecting")),
        "no re-dial: {seen:?}"
    );
    assert_eq!(accepted.load(Ordering::SeqCst), 1, "one upgrade, no retry");
}

#[tokio::test(flavor = "current_thread")]
async fn any_other_close_still_reconnects() {
    let (addr, _accepted) = closing_server(CloseCode::Normal, "bye").await;
    let (_cmd_tx, mut events) = ws::spawn(config(&addr));

    let mut seen = Vec::new();
    let reconnecting = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(evt) = events.recv().await {
            let reconnect = matches!(
                evt,
                TransportEvent::ConnectionState(ConnectionState::Reconnecting { .. })
            );
            seen.push(format!("{evt:?}"));
            if reconnect {
                return true;
            }
        }
        false
    })
    .await;
    assert_eq!(reconnecting, Ok(true), "a normal close re-dials: {seen:?}");
    assert!(!seen.iter().any(|e| e == "AuthExpired"), "{seen:?}");
}
