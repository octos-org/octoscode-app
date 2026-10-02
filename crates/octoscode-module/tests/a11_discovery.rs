//! A11 — pairing discovery through the PRODUCTION path (walk row 114,
//! `e2e/pairing-link.spec.ts` "a pairing-capable server this browser has seen
//! offers itself once"; row 113 is the 404 twin).
//!
//! `screens::discovery::start` is what lib.rs's `start` calls
//! (`start_once` = `start(&Inputs::current())`): the decision, then the ONE
//! unauthenticated `GET <origin>/pair/info` on its own thread through
//! `octoscode_client::pairing::probe_pairing_info`. The Connect card renders
//! the offer with `fluid::connect_card_with_offer` (what lib.rs mounts), and
//! its button routes `b1.open.discovered` through `board1::route` (what
//! `board1::collect` -> `perform_board1` calls).
//!
//! Traffic: the RECORDED answer of a real local `octos serve` a6ea8505
//! (`crates/octoscode-client/tests/fixtures/pairing-a6ea8505.jsonl`, the
//! "discovery" line), served by a loopback server that counts requests.
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use octoscode_client::pairing::{probe_pairing_info, PairingProbe};
use octoscode_module::fluid::{connect_card_with_offer, ConnectView};
use octoscode_module::screens::{board1, discovery, pairing};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// The recorded `/pair/info` reply (status, body).
fn recorded_info() -> (u16, serde_json::Value) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../octoscode-client/tests/fixtures/pairing-a6ea8505.jsonl");
    let text = std::fs::read_to_string(path).expect("the pairing fixture");
    let row: serde_json::Value = text
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|r| r["path"] == "/pair/info")
        .expect("a recorded /pair/info");
    (row["status"].as_u64().unwrap() as u16, row["body"].clone())
}

/// A loopback server answering `/pair/info` with `status` + `body`; returns
/// (port, the number of requests it saw).
async fn info_server(status: u16, body: serde_json::Value) -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(AtomicUsize::new(0));
    let seen2 = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else { return };
            let (seen, body) = (seen2.clone(), body.clone());
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                if head.starts_with("GET /pair/info ") {
                    seen.fetch_add(1, Ordering::SeqCst);
                }
                let text = if status == 404 { String::new() } else { body.to_string() };
                let resp = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
                    text.len()
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            });
        }
    });
    (port, seen)
}

fn inputs(server: &str) -> discovery::Inputs {
    discovery::Inputs { remembered: Some(server.to_owned()), ..Default::default() }
}

async fn eventually<T>(mut f: impl FnMut() -> Option<T>) -> Option<T> {
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        if let Some(v) = f() {
            return Some(v);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    None
}

/// The whole production path, once (the live state is once per process, as
/// the web's probe is once per page load).
#[tokio::test(flavor = "multi_thread")]
async fn a_remembered_pairing_capable_server_is_offered_once_and_leads_into_pairing() {
    let (status, body) = recorded_info();
    assert_eq!(status, 200);
    let (port, seen) = info_server(status, body.clone()).await;
    let origin = format!("http://127.0.0.1:{port}");

    // lib.rs `start`: the remembered server, no credential -> ONE probe.
    assert_eq!(discovery::start(&inputs(&origin)).as_deref(), Ok(origin.as_str()));
    let offer = eventually(discovery::offer).await.expect("the capable server is offered");
    assert_eq!(seen.load(Ordering::SeqCst), 1, "one unauthenticated GET /pair/info");
    // readInfo: the server's own loopback echo names the offer (the recording
    // was made on :8422), and the recording says a token is required.
    let echoed = body["server_origin"].as_str().unwrap();
    assert_eq!(offer.origin, echoed);
    assert_eq!(offer.label, echoed.trim_start_matches("http://"));
    assert!(offer.pairing_required);
    assert_eq!(offer.message(), format!("Found Octos on {}.", offer.label));

    // Never a second probe in the run.
    assert_eq!(discovery::start(&inputs(&origin)), Err(discovery::Skip::Once));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(seen.load(Ordering::SeqCst), 1);

    // The card lib.rs mounts carries the offer exactly once, as a status
    // under the title (never an error), its button routed to board 1.
    let m = octoscode_module::conv_layout::Metrics::for_window(990.0, true);
    let card = connect_card_with_offer(&ConnectView::default(), &m, 261.0, Some(&offer));
    assert_eq!(card.matches(&offer.message()).count(), 1);
    assert_eq!(card.matches(&offer.action()).count(), 1);
    assert!(!card.contains("connect_error :="), "a status, not an alert");
    assert!(board1::entry_controls().iter().any(|(id, a)| id == "connect_offer" && a == "b1.open.discovered"));

    // The button (board1::collect -> perform_board1 -> route): the form names
    // the server and pairing (p4-01) opens with it, once.
    board1::close_all();
    let work = board1::route("b1.open.discovered", None);
    assert!(
        work.iter().any(|w| matches!(w, board1::Work::LeaveToForm { server: Some(s) } if s == echoed)),
        "{work:?}"
    );
    assert_eq!(board1::top(), Some(board1::Surface::Pairing));
    assert_eq!(pairing::state().screen, pairing::Screen::Pair);
    assert_eq!(pairing::state().server, echoed, "p4-03/p4-04 fall back with this origin");
    assert_eq!(discovery::offer(), None, "used: the offer is gone");
    assert!(board1::route("b1.open.discovered", None).is_empty(), "offered once");
    // A live connection later keeps it gone.
    board1::note_context(&board1::Context { live: true, ..Default::default() });
    assert_eq!(discovery::offer(), None);
    board1::close_all();
}

/// Row 113's discovery half: a server that 404s /pair/info is simply not a
/// pairing server — one GET, no offer, no complaint. Driven through the same
/// `Discovery` state machine the live state wraps, with the real probe.
#[tokio::test(flavor = "multi_thread")]
async fn a_404_server_gets_one_probe_and_no_offer() {
    let (port, seen) = info_server(404, serde_json::Value::Null).await;
    let origin = format!("http://127.0.0.1:{port}");
    let mut d = discovery::Discovery::default();
    let (o, g) = d.begin(&inputs(&origin)).unwrap();
    let probe = probe_pairing_info(&o).await;
    assert_eq!(probe, PairingProbe::Unsupported);
    assert_eq!(d.finish(g, &probe), None, "no offer");
    assert_eq!(d.appeared(), None, "nothing to show");
    assert_eq!(seen.load(Ordering::SeqCst), 1);
}

/// The rules that keep a request from being sent at all.
#[test]
fn no_request_without_a_remembered_loopback_server_or_with_a_credential() {
    let mut d = discovery::Discovery::default();
    assert_eq!(d.begin(&inputs("http://192.168.1.20:50190")), Err(discovery::Skip::NotLoopback));
    let mut with_token = inputs("http://127.0.0.1:50190");
    with_token.stored_token = true;
    assert_eq!(discovery::decide(&with_token), Err(discovery::Skip::Credential));
    let mut link = inputs("http://127.0.0.1:50190");
    link.pairing_link = true;
    assert_eq!(discovery::decide(&link), Err(discovery::Skip::PairingLink));
    assert_eq!(discovery::decide(&discovery::Inputs::default()), Err(discovery::Skip::NoServer));
}

/// A server that runs without a token (`pairing_required: false`): the
/// button connects at once, tokenless (the web's `useDiscoveredOrigin`).
#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_needs_no_token_connects_at_once() {
    let (_, mut body) = recorded_info();
    body["pairing_required"] = serde_json::Value::Bool(false);
    let (port, _) = info_server(200, body.clone()).await;
    let origin = format!("http://127.0.0.1:{port}");
    let mut d = discovery::Discovery::default();
    let (o, g) = d.begin(&inputs(&origin)).unwrap();
    let probe = probe_pairing_info(&o).await;
    let offer = d.finish(g, &probe).cloned().expect("offered");
    assert!(!offer.pairing_required);
    // The same branch board1::route takes for the live offer.
    assert_eq!(d.take().map(|o| o.pairing_required), Some(false));
}
