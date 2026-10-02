//! A9 — Settings > Connection's production path at the wire: Disconnect
//! closes the socket for real (the transport's voluntary disconnect: no
//! reconnect follows) and Forget server also removes the remembered address
//! and that origin's token, through the same functions the host runs
//! (`a9_settings::disconnect`, `a9_settings::forget_saved`).
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::Conversation;
use octoscode_module::screens::a9_settings::{self, LeaveKind};

#[derive(Default)]
struct Seen {
    connections: usize,
    closes: usize,
    ended: usize,
}

async fn fake_server() -> (String, Arc<Mutex<Seen>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Seen::default()));
    let s2 = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else { return };
            let seen = s2.clone();
            tokio::spawn(async move {
                let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
                seen.lock().unwrap().connections += 1;
                let (mut tx, mut rx) = ws.split();
                while let Some(msg) = rx.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                            if v.get("id").is_none() {
                                continue;
                            }
                            let session = v["params"]["session_id"].as_str().unwrap_or("a9:main").to_owned();
                            let result = match v["method"].as_str().unwrap_or("") {
                                "session/open" => json!({"opened": {
                                    "session_id": session, "active_profile_id": "a9",
                                    "cursor": {"stream": session, "seq": 1},
                                    "capabilities": {"version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                                        "capabilities_schema_version": 1, "supported_methods": ["session/open", "session/list"],
                                        "supported_notifications": [], "supported_features": []}
                                }}),
                                "session/list" => json!({"sessions": [{"id": "a9:main", "message_count": 1}]}),
                                _ => json!({}),
                            };
                            let frame = json!({"jsonrpc": "2.0", "id": v["id"].clone(), "result": result});
                            let _ = tx.send(Message::Text(frame.to_string().into())).await;
                        }
                        Ok(Message::Close(_)) => {
                            seen.lock().unwrap().closes += 1;
                            break;
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
                seen.lock().unwrap().ended += 1;
            });
        }
    });
    (base, seen)
}

#[tokio::test]
async fn disconnect_closes_the_socket_and_nothing_reconnects() {
    let (base, seen) = fake_server().await;
    let (conv, mut events) = Conversation::connect(&base, "dummy", "a9", None, None).expect("connect");
    conv.open_workspace(None).await.expect("open");
    for _ in 0..40 {
        if conv.store.is_live() {
            break;
        }
        if let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(200), events.recv()).await {
            let _ = conv.on_event(evt);
        }
    }
    assert!(conv.store.is_live(), "connected first");
    assert_eq!(seen.lock().unwrap().connections, 1);

    // The Settings > Disconnect path (a9_host -> a9_settings::disconnect).
    assert_eq!(a9_settings::request(LeaveKind::Disconnect, false, false), Some(LeaveKind::Disconnect));
    a9_settings::disconnect(&conv).await;
    assert_eq!(conv.store.connection(), "Offline");
    assert!(!conv.store.is_live());
    // The socket closed with a Close frame, and the transport did not dial
    // again (a dropped socket would have reconnected within its backoff).
    for _ in 0..30 {
        if seen.lock().unwrap().ended >= 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let s = seen.lock().unwrap();
    assert_eq!(s.closes, 1, "a voluntary close frame");
    assert_eq!(s.connections, 1, "nothing reconnects after Disconnect");
}

#[test]
fn forget_server_removes_the_remembered_address_and_its_token() {
    let dir = std::env::temp_dir().join(format!("a9-settings-cred-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::env::set_var("OCTOSCODE_CREDENTIALS_DIR", &dir);
    // A21: Forget also clears the tab drafts and the restore hints — on
    // this test's own files, never ~/.octoscode (brief §8).
    std::env::set_var("OCTOSCODE_DRAFTS_FILE", dir.join("composer-drafts.json"));
    std::env::set_var("OCTOSCODE_CONNECTION_FILE", dir.join("connection-v1.json"));
    octoscode_module::credentials::remember_server("http://127.0.0.1:50190").unwrap();
    octoscode_module::credentials::remember_token("http://127.0.0.1:50190", "tok-one").unwrap();
    octoscode_module::credentials::remember_token("http://10.0.0.9:50190", "tok-two").unwrap();
    assert_eq!(octoscode_module::credentials::prefill().0.as_deref(), Some("http://127.0.0.1:50190"));
    // The Settings > Forget server path (a9_host -> a9_settings::forget_saved).
    a9_settings::forget_saved("http://127.0.0.1:50190");
    assert_eq!(octoscode_module::credentials::prefill(), (None, None), "the next start prefills nothing");
    assert_eq!(octoscode_module::credentials::token_for("http://127.0.0.1:50190"), None);
    // A21 (row 195, the web's way): Forget clears every origin's saved token
    // (`ConnectionGate.tsx:319-356` -> `clearRememberedTokens`,
    // `remembered-token.ts`), not only this server's.
    assert_eq!(
        octoscode_module::credentials::token_for("http://10.0.0.9:50190"),
        None,
        "no other server's token survives Forget"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
