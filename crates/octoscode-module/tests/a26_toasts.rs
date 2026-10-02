//! A26 — parity row `error/error` "error toasts (transient, bounded queue)".
//!
//! The web oracle has no toast (A9: `grep -ri toast apps/web/src` -> 0); the
//! operator chose to build them for the failures that reached ONLY the app
//! log. The production path proven here: a New chat whose `session/open` the
//! server refuses — the open is fire-and-forget (`flow::open_workspace_with`
//! returns at once and the window switches to the empty new Session), and
//! before A26 the refusal (`TransportEvent::RpcError`) landed nowhere on
//! screen: no history read waits on a fresh id, so `on_open_error` dropped it
//! (measured on main b1fe23fb with `replay_serve --refuse session/open@1`: four
//! "New chat" rows, nothing said). Now the same arm queues the toast with the
//! server's own reason, and the stack lowers on the board-3 notice kit.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::Conversation;
use octoscode_module::screens::toasts;

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

const REFUSAL: &str = "session limit reached: close a session first";

/// A fake server: the FIRST `session/open` opens; every later one is refused
/// with a JSON-RPC error carrying `REFUSAL`.
async fn start_server() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let base_url = format!("http://{}", listener.local_addr().expect("addr"));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        let Ok((stream, _)) = listener.accept().await else { return };
        let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
        let (mut tx, mut rx) = ws.split();
        let mut opens = 0;
        while let Some(Ok(msg)) = rx.next().await {
            let Message::Text(text) = msg else { continue };
            let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
            let method = v["method"].as_str().unwrap_or("").to_owned();
            log.lock().unwrap().push(method.clone());
            let id = v["id"].clone();
            let frame = match method.as_str() {
                "session/open" => {
                    opens += 1;
                    if opens == 1 {
                        let session = v["params"]["session_id"].as_str().unwrap_or("a26:main").to_owned();
                        json!({"jsonrpc": "2.0", "id": id, "result": {"opened": {
                            "session_id": session,
                            "active_profile_id": "a26",
                            "workspace_root": "/home/user/src/octos",
                            "cursor": {"stream": "a26", "seq": 1},
                            "capabilities": {
                                "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                                "capabilities_schema_version": 1,
                                "supported_methods": ["session/open", "session/list"],
                                "supported_notifications": ["turn/started"],
                                "supported_features": []
                            }
                        }}})
                    } else {
                        json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32603, "message": REFUSAL}})
                    }
                }
                _ => json!({"jsonrpc": "2.0", "id": id, "result": {}}),
            };
            let _ = tx.send(Message::Text(frame.to_string().into())).await;
        }
    });
    (base_url, seen)
}

/// Fold the conversation's events for `ms`, the way the host's event loop
/// does (`conv.on_event` on every transport event).
macro_rules! fold {
    ($conv:expr, $events:expr, $ms:expr) => {{
        let end = tokio::time::Instant::now() + Duration::from_millis($ms);
        while tokio::time::Instant::now() < end {
            match tokio::time::timeout(Duration::from_millis(50), $events.recv()).await {
                Ok(Some(evt)) => {
                    let _ = $conv.on_event(evt);
                }
                Ok(None) => break,
                Err(_) => {}
            }
        }
    }};
}

#[tokio::test]
async fn a_refused_new_chat_is_toasted_with_the_servers_reason() {
    let _g = serial();
    toasts::reset();
    let (base, seen) = start_server().await;
    let (conv, mut events) = Conversation::connect(&base, "dummy", "a26", None, None).expect("connect");
    conv.open_workspace(None).await.expect("the first open is sent");
    fold!(conv, events, 600);
    assert!(toasts::snapshot().0.is_empty(), "the accepted open is not an error");

    // The sidebar's New chat (`session.new` -> `conv.new_chat`).
    let id = conv.new_chat(None).await.expect("the open is sent (fire-and-forget)");
    assert_eq!(conv.session_id(), id, "the window switched to the new Session at once");
    fold!(conv, events, 800);
    assert_eq!(seen.lock().unwrap().iter().filter(|m| *m == "session/open").count(), 2);

    // The refusal is no longer silent: one toast, the plain lead, the
    // server's own reason as the cause.
    let (items, dropped) = toasts::snapshot();
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0].lead, "Couldn't start a new chat.");
    assert_eq!(items[0].cause, REFUSAL);
    assert_eq!(dropped, 0);

    // It lowers on the notice kit with its × routed through the host table.
    let low = toasts::lower(360.0, 600.0).expect("the stack");
    assert!(low.dsl.contains("Couldn't start a new chat.") && low.dsl.contains(REFUSAL), "{}", low.dsl);
    assert_eq!(low.taps.len(), 1);
    let (_, event) = &low.taps[0];
    assert_eq!(event, &format!("toast.dismiss#{}", items[0].id));
    assert!(toasts::routes("toast.dismiss"));
    assert!(toasts::dismiss(items[0].id));
    assert!(toasts::snapshot().0.is_empty());
    toasts::reset();
}

#[tokio::test]
async fn the_stack_is_bounded_transient_and_held_under_a_dialog() {
    let _g = serial();
    toasts::reset();
    // Four different refusals: three stay, the oldest goes, and the stack
    // says so.
    for i in 1..=4 {
        toasts::failed(toasts::Op::NewChat, &format!("{REFUSAL} ({i})"));
    }
    let (items, dropped) = toasts::snapshot();
    assert_eq!((items.len(), dropped), (toasts::CAPACITY, 1));
    assert_eq!(items[0].cause, format!("{REFUSAL} (2)"), "the oldest was pushed out");
    let low = toasts::lower(360.0, 600.0).unwrap();
    assert!(low.dsl.contains("1 earlier error no longer shown"), "{}", low.dsl);
    // Held while a dialog is open: no time passes for them.
    toasts::tick(1_000, false);
    toasts::tick(1_000 + 3 * toasts::SHOW_MS, true);
    assert_eq!(toasts::snapshot().0.len(), toasts::CAPACITY);
    // Transient: once back on screen, they leave on their own.
    toasts::tick(1_000 + 4 * toasts::SHOW_MS, false);
    assert_eq!(toasts::snapshot(), (vec![], 0), "gone, and the note with them");
    toasts::reset();
}
