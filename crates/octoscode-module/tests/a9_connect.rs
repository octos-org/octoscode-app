//! A9 — the Connect failure probe at the wire: one HTTP request to the
//! endpoint the socket dials, with the same bearer; each answer becomes its
//! §5.1 kind through the production hook (`a9_connect::on_gave_up` ->
//! `ConnectUi::note_connect_error`), and a refused token keeps the typed
//! value with the focus flag set.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use octoscode_module::screens::a9_connect::{self, Verdict};
use octoscode_module::screens::connect::{ConnectUi, FailureKind};

/// A one-route HTTP server answering every request with `status` + `body`,
/// recording the request heads.
async fn http_server(status: &'static str, body: &'static str) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else { return };
            let seen = s2.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                seen.lock().unwrap().push(String::from_utf8_lossy(&buf[..n]).to_string());
                let resp = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            });
        }
    });
    (base, seen)
}

#[tokio::test]
async fn a_refused_token_is_told_apart_and_rides_the_same_bearer() {
    let (base, seen) = http_server("401 Unauthorized", "unauthorized").await;
    assert_eq!(a9_connect::probe(&base, "typed-token").await, Verdict::Rejected);
    let head = seen.lock().unwrap()[0].clone();
    assert!(head.starts_with("GET /api/ui-protocol/ws "), "{head}");
    assert!(head.to_ascii_lowercase().contains("authorization: bearer typed-token"), "the socket's own bearer");
}

#[tokio::test]
async fn origin_reachable_and_closed_ports_get_their_own_verdicts() {
    let (base, _) = http_server("403 Forbidden", "Origin not allowed").await;
    assert_eq!(a9_connect::probe(&base, "").await, Verdict::OriginNotAllowed);
    let (base, _) = http_server("426 Upgrade Required", "").await;
    assert_eq!(a9_connect::probe(&base, "t").await, Verdict::Reachable);
    // A port nothing listens on (bound then dropped).
    let free = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = free.local_addr().unwrap();
    drop(free);
    assert_eq!(a9_connect::probe(&format!("http://{addr}"), "t").await, Verdict::Unreachable);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_give_up_hook_classifies_with_the_probe_and_keeps_the_typed_token() {
    let (base, _) = http_server("401 Unauthorized", "").await;
    let dir = std::env::temp_dir().join(format!("a9-connect-cred-{}", std::process::id()));
    std::env::set_var("OCTOSCODE_CREDENTIALS_DIR", &dir);
    octoscode_module::credentials::remember_token(&base, "typed-token").unwrap();
    let mut ui = ConnectUi { server: base.clone(), token: "typed-token".into(), connecting: true, ..ConnectUi::default() };
    let handle = tokio::runtime::Handle::current();
    // The transport gave up: the first pass starts the probe (the card keeps
    // "Connecting…"), a later pass classifies with its verdict.
    a9_connect::on_gave_up(&mut ui, Some(&handle));
    assert!(ui.failure.is_none() && ui.connecting, "probing");
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        a9_connect::on_gave_up(&mut ui, Some(&handle));
        if ui.failure.is_some() {
            break;
        }
    }
    let f = ui.failure.clone().expect("classified");
    assert_eq!(f.kind, FailureKind::RejectedToken);
    assert_eq!(f.message, "The server refused this token");
    assert!(f.focus_token, "the token field takes the focus");
    assert_eq!(ui.token, "typed-token", "the typed value is kept");
    assert!(!ui.connecting);
    assert_eq!(a9_connect::hint(), None, "a definite verdict carries no hedge");
    // The refused token is not prefilled again (the per-origin store).
    assert_eq!(octoscode_module::credentials::token_for(&base), None);
    let _ = std::fs::remove_dir_all(&dir);
}
