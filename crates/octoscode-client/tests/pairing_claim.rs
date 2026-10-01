//! #A2 board 1 — the pairing exchange (`octoscode_client::pairing`) against
//! **recorded real traffic**: `tests/fixtures/pairing-a6ea8505.jsonl` is what a
//! real `octos serve` a6ea8505 (`--web-url`, loopback) answered on
//! `/pair/info` and `/pair/claim` — discovery, a malformed body, a wrong code,
//! the one good claim (token redacted), the same code again, and a locked code.
//! A tiny HTTP server replays those answers so the production functions
//! (`claim_pairing_code`, `probe_pairing_info`) run their real reqwest path.
//!
//! The 404 ("this server does not offer pairing") answer cannot be recorded
//! from a loopback client (octos answers 404 only to non-loopback peers or a
//! pairing-less deployment, `crates/octos-cli/src/api/pairing.rs:325-338`), so
//! that one arm serves the contract's bare 404 and says so.
use std::sync::{Arc, Mutex};

use octoscode_client::pairing::{
    self, claim_pairing_code, probe_pairing_info, read_pairing_link, PairingErrorKind, PairingLink,
    PairingProbe,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// The code our replay server treats as "the printed one".
const PRINTED: &str = "3QK7ZP2M";
/// What the replay server hands out where the recording has `<redacted>`.
const TOKEN: &str = "fixture-api-token-0001";

#[derive(Clone)]
struct Recorded {
    note: String,
    status: u16,
    body: serde_json::Value,
}

fn fixture() -> Vec<Recorded> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/pairing-a6ea8505.jsonl");
    std::fs::read_to_string(path)
        .expect("read the pairing fixture")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).expect("fixture line is JSON");
            Recorded {
                note: v["note"].as_str().unwrap_or_default().to_owned(),
                status: v["status"].as_u64().unwrap_or(0) as u16,
                body: v["body"].clone(),
            }
        })
        .collect()
}

fn recorded(note_part: &str) -> Recorded {
    fixture()
        .into_iter()
        .find(|r| r.note.contains(note_part))
        .unwrap_or_else(|| panic!("the fixture has no {note_part:?} line"))
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Replay the recording: one good claim, then single-use.
    Replay,
    /// The recorded locked answer for every claim.
    Locked,
    /// The contract's bare 404 for every route (see the module docs).
    NotSupported,
}

struct Server {
    port: u16,
    /// (method, path, body) of every request that reached the server.
    seen: Arc<Mutex<Vec<(String, String, String)>>>,
}

async fn serve(mode: Mode) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().unwrap().port();
    let seen: Arc<Mutex<Vec<(String, String, String)>>> = Arc::default();
    let rec = seen.clone();
    let claimed = Arc::new(Mutex::new(false));
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else { return };
            let rec = rec.clone();
            let claimed = claimed.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                // Read the head, then exactly content-length bytes of body.
                let (head, mut body) = loop {
                    let n = stream.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..i]).to_string();
                        break (head, buf[i + 4..].to_vec());
                    }
                };
                let len = head
                    .lines()
                    .find_map(|l| {
                        let (k, v) = l.split_once(':')?;
                        k.eq_ignore_ascii_case("content-length").then(|| v.trim().parse().ok())?
                    })
                    .unwrap_or(0usize);
                while body.len() < len {
                    let n = stream.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    body.extend_from_slice(&chunk[..n]);
                }
                let mut first = head.lines().next().unwrap_or("").split_whitespace();
                let method = first.next().unwrap_or("").to_owned();
                let path = first.next().unwrap_or("").to_owned();
                let body = String::from_utf8_lossy(&body).to_string();
                rec.lock().unwrap().push((method.clone(), path.clone(), body.clone()));
                let (status, reply): (u16, Option<serde_json::Value>) = match (mode, method.as_str(), path.as_str()) {
                    (Mode::NotSupported, _, _) => (404, None),
                    (_, "GET", "/pair/info") => {
                        let r = recorded("discovery");
                        (r.status, Some(r.body))
                    }
                    (Mode::Locked, "POST", "/pair/claim") => {
                        let r = recorded("pair_code_locked");
                        (r.status, Some(r.body))
                    }
                    (Mode::Replay, "POST", "/pair/claim") => {
                        let code = serde_json::from_str::<serde_json::Value>(&body)
                            .ok()
                            .and_then(|v| v["code"].as_str().map(str::to_owned))
                            .unwrap_or_default();
                        if code.len() != 8 {
                            let r = recorded("malformed body");
                            (r.status, Some(r.body))
                        } else if code == PRINTED && !std::mem::replace(&mut *claimed.lock().unwrap(), true) {
                            let r = recorded("claims the token once");
                            let text = r.body.to_string().replace("<redacted>", TOKEN);
                            (r.status, serde_json::from_str(&text).ok())
                        } else if code == PRINTED {
                            let r = recorded("single use");
                            (r.status, Some(r.body))
                        } else {
                            let r = recorded("wrong well-formed code");
                            (r.status, Some(r.body))
                        }
                    }
                    _ => (404, None),
                };
                let text = reply.map(|v| v.to_string()).unwrap_or_default();
                let reason = match status {
                    200 => "OK",
                    400 => "Bad Request",
                    404 => "Not Found",
                    _ => "Other",
                };
                let resp = format!(
                    "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
                    text.len()
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    Server { port, seen }
}

fn link(port: u16, code: &str) -> PairingLink {
    read_pairing_link(&format!(
        "http://app.invalid/?octos=http://127.0.0.1:{port}&pair={code}"
    ))
    .expect("a well-formed printed link")
}

#[tokio::test]
async fn the_printed_code_claims_the_token_once_then_is_spent() {
    let server = serve(Mode::Replay).await;
    // The operator's lowercase copy still claims: the wire code is uppercased.
    let claim = claim_pairing_code(&link(server.port, &PRINTED.to_lowercase()))
        .await
        .expect("the first claim of the printed code succeeds");
    assert_eq!(claim.token, TOKEN);
    // The recording's server echoed its own (loopback) origin; that echo is
    // what the client connects to next (`readClaim`, pairing.ts:283-294).
    assert_eq!(claim.server_origin, "http://127.0.0.1:8422");
    {
        let seen = server.seen.lock().unwrap();
        let (method, path, body) = &seen[0];
        assert_eq!((method.as_str(), path.as_str()), ("POST", "/pair/claim"));
        let sent: serde_json::Value = serde_json::from_str(body).expect("the body is JSON");
        assert_eq!(sent, serde_json::json!({ "code": PRINTED }), "uppercased, nothing else");
    }
    // Single use: the same link again is the recorded pair_code_unknown.
    let again = claim_pairing_code(&link(server.port, PRINTED)).await;
    assert_eq!(again, Err(PairingErrorKind::CodeUnknown));
}

#[tokio::test]
async fn a_wrong_code_and_a_malformed_body_get_their_own_wire_kinds() {
    let server = serve(Mode::Replay).await;
    assert_eq!(
        claim_pairing_code(&link(server.port, "ZZZZZZZZ")).await,
        Err(PairingErrorKind::CodeUnknown)
    );
    // A short code passes the client's own check (1..=64) but the server's
    // body rule refuses it: the recorded pair_code_invalid.
    assert_eq!(
        claim_pairing_code(&link(server.port, "abc")).await,
        Err(PairingErrorKind::CodeInvalid)
    );
}

#[tokio::test]
async fn a_locked_code_is_pair_code_locked() {
    let server = serve(Mode::Locked).await;
    assert_eq!(
        claim_pairing_code(&link(server.port, PRINTED)).await,
        Err(PairingErrorKind::CodeLocked)
    );
}

#[tokio::test]
async fn a_404_is_simply_pairing_not_supported() {
    let server = serve(Mode::NotSupported).await;
    assert_eq!(
        claim_pairing_code(&link(server.port, PRINTED)).await,
        Err(PairingErrorKind::NotSupported)
    );
    assert_eq!(probe_pairing_info(&format!("http://127.0.0.1:{}", server.port)).await, PairingProbe::Unsupported);
}

#[tokio::test]
async fn an_off_machine_origin_is_refused_without_any_request() {
    let server = serve(Mode::Replay).await;
    let foreign = read_pairing_link("?octos=http://192.168.1.20:50190&pair=3QK7ZP2M").unwrap();
    assert_eq!(claim_pairing_code(&foreign).await, Err(PairingErrorKind::OriginNotLoopback));
    assert!(server.seen.lock().unwrap().is_empty(), "nothing may reach any server");
    // An over-long code is refused before a request too.
    let long = link(server.port, &"A".repeat(pairing::MAX_CODE_LENGTH + 5));
    assert_eq!(claim_pairing_code(&long).await, Err(PairingErrorKind::CodeInvalid));
    assert!(server.seen.lock().unwrap().is_empty(), "the over-long code never left");
}

#[tokio::test]
async fn nobody_listening_is_unreachable() {
    // Bind then drop: the port is free and refuses connections.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    assert_eq!(claim_pairing_code(&link(port, PRINTED)).await, Err(PairingErrorKind::Unreachable));
    assert_eq!(probe_pairing_info(&format!("http://127.0.0.1:{port}")).await, PairingProbe::Unavailable);
}

#[tokio::test]
async fn discovery_reads_the_recorded_pair_info() {
    let server = serve(Mode::Replay).await;
    match probe_pairing_info(&format!("http://127.0.0.1:{}/ignored/path", server.port)).await {
        PairingProbe::Available(info) => {
            assert_eq!(info.product, "octos");
            assert!(info.pairing_required);
            assert_eq!(info.server_origin, "http://127.0.0.1:8422");
        }
        other => panic!("the recorded /pair/info is a capable server: {other:?}"),
    }
    let seen = server.seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "ONE request, on one origin — never a port range");
    assert_eq!((seen[0].0.as_str(), seen[0].1.as_str()), ("GET", "/pair/info"));
}

#[test]
fn the_fixture_carries_no_secret() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/pairing-a6ea8505.jsonl"
    ))
    .unwrap();
    assert!(text.contains("<redacted>"), "the recorded token is redacted");
    assert!(text.contains("<printed-code>"), "the printed code is redacted");
    for r in fixture() {
        assert!(r.status == 200 || r.status == 400, "{}: {}", r.note, r.status);
    }
}
