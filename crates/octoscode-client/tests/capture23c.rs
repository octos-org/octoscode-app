//! Card #23, part 3 — ONE focused attempt at `turn/steer_dropped` (the only
//! reachable-but-missed method). `turn/error` is not provoked: it needs the
//! model/turn to fail (rate-limit, provider error), out of the card's spend.
//!
//! The method is emitted by `settle_leftover_steers`
//! (octos `crates/octos-cli/src/api/ui_protocol_transport.rs:43490`) when a
//! turn reaches Terminal with steers still in the buffer; the reason is
//! `interrupted` when the client interrupted. So: start a LONG generation,
//! steer it while it streams, interrupt immediately.
//!
//! ```sh
//! OCTOS_BASE_URL=http://127.0.0.1:50160 OCTOS_BEARER=r23-dummy-token \
//!   OCTOS_PROFILE_ID=dsflash ctest -p octoscode-client --test capture23c -- --ignored --nocapture
//! ```
use std::time::Duration;

use octos_app_transport::{ws, ProfileId, SecretString, TransportConfig, TransportEvent};
use octoscode_client::features::web_capabilities;
use octoscode_client::trace::{wire_params, FrameTrace};
use octoscode_client::Client;
use serde_json::{json, Value};

const SERVE_PORT: u16 = 50160;

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/r23-conversation-a6ea8505.jsonl")
}

const TMP_PLACEHOLDER: &str = "<TMP>";
const WORKSPACE_PLACEHOLDER: &str = "<WORKSPACE>";
const HOME_PLACEHOLDER: &str = "<HOME>";

fn machine_path_prefixes() -> Vec<(String, &'static str)> {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_string_lossy().to_string());
    let mut prefixes: Vec<(String, &'static str)> = Vec::new();
    let tmp = std::env::temp_dir().to_string_lossy().to_string();
    if !tmp.is_empty() {
        prefixes.push((tmp, TMP_PLACEHOLDER));
    }
    if let Some(ws) = &workspace_root {
        if !ws.is_empty() {
            prefixes.push((ws.clone(), WORKSPACE_PLACEHOLDER));
        }
        if let Some(home) = std::path::Path::new(ws).parent() {
            let home = home.to_string_lossy().to_string();
            if !home.is_empty() {
                prefixes.push((home, HOME_PLACEHOLDER));
            }
        }
    }
    prefixes.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    prefixes
}

fn scrub_machine_paths(text: &str) -> String {
    let mut out = text.to_owned();
    for (from, to) in machine_path_prefixes() {
        out = out.replace(&from, &to);
    }
    out
}

fn scrub(v: Value) -> Value {
    match v {
        Value::String(s) if s.contains("sk-") => Value::String("<redacted-agent>".to_owned()),
        Value::Array(items) => Value::Array(items.into_iter().map(scrub).collect()),
        Value::Object(map) => Value::Object(map.into_iter().map(|(k, v)| (k, scrub(v))).collect()),
        other => other,
    }
}

fn record_inbound(trace: &FrameTrace, evt: &TransportEvent) {
    match evt {
        TransportEvent::DurableNotification { payload, cursor } => {
            let mut body = wire_params(payload);
            if let (Some(c), Some(map)) = (cursor, body.as_object_mut()) {
                map.insert("cursor".to_owned(), json!({"stream": c.stream, "seq": c.seq}));
            }
            trace.inbound(payload.method(), &scrub(body));
        }
        TransportEvent::EphemeralNotification { payload } => {
            trace.inbound(payload.method(), &scrub(wire_params(payload)));
        }
        _ => {}
    }
}

/// A prompt that forces a LONG streaming answer, so a steer lands mid-turn.
const P_LONG: &str =
    "Count slowly from 1 to 300, one number per line, with a short adjective after each number. Do not skip any and do not summarize.";

#[tokio::test]
#[ignore = "one focused steer_dropped attempt; spends 1 model turn"]
async fn capture_r23_steer_dropped() {
    let path = fixture_path();
    let trace = FrameTrace::open(path.to_str().expect("utf8 path"));
    assert!(trace.is_enabled());

    let base = std::env::var("OCTOS_BASE_URL")
        .unwrap_or_else(|_| format!("http://127.0.0.1:{SERVE_PORT}"));
    let profile = std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "dsflash".into());
    let cfg = TransportConfig {
        base_url: url::Url::parse(&base).expect("base url"),
        bearer: SecretString::new(
            std::env::var("OCTOS_BEARER").unwrap_or_else(|_| "r23-dummy-token".into()),
        ),
        profile_id: ProfileId::new(profile.clone()),
        cursor: None,
        cursor_file: None,
        requested_capabilities: web_capabilities(),
        workspace_cwd: std::env::var("OCTOS_TMP_CWD").ok(),
        local_kernel: false,
    };
    let (cmd_tx, mut evt_rx) = ws::spawn(cfg);
    let t = trace.clone();
    tokio::spawn(async move {
        while let Some(evt) = evt_rx.recv().await {
            record_inbound(&t, &evt);
        }
    });
    let client = Client::with_trace(cmd_tx, trace.clone());

    let session = format!("{profile}:main");
    let _ = client
        .request("session/open", json!({"session_id": session, "profile_id": profile}))
        .await;
    tokio::time::sleep(Duration::from_millis(800)).await;

    let turn_id = "01920000-0000-7000-8000-000000000245";
    let started = client
        .request(
            "turn/start",
            json!({"session_id": session, "turn_id": turn_id,
                   "input": [{"kind": "text", "text": P_LONG}]}),
        )
        .await;
    if let Ok(v) = &started {
        trace.result("turn/start", None, &scrub(v.clone()));
    }
    println!("[r23c] long turn started");

    // Steer while the model streams, then interrupt a beat later.
    tokio::time::sleep(Duration::from_millis(1800)).await;
    let steered = client
        .request(
            "turn/steer",
            json!({"session_id": session, "expected_turn_id": turn_id,
                   "input": [{"kind": "text", "text": "also mention the word banana"}]}),
        )
        .await;
    match &steered {
        Ok(v) => {
            trace.result("turn/steer", None, &scrub(v.clone()));
            println!("[r23c] steer receipt: {}", serde_json::to_string(v).unwrap_or_default());
        }
        Err(octoscode_client::ClientError::Rpc { method, error }) => {
            trace.error(method, None, &error.message, error.code)
        }
        Err(e) => eprintln!("[r23c] steer {e:?}"),
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    let interrupted = client
        .request("turn/interrupt", json!({"session_id": session, "turn_id": turn_id}))
        .await;
    match &interrupted {
        Ok(v) => {
            trace.result("turn/interrupt", None, &scrub(v.clone()));
            println!("[r23c] interrupt receipt: {}", serde_json::to_string(v).unwrap_or_default());
        }
        Err(octoscode_client::ClientError::Rpc { method, error }) => {
            trace.error(method, None, &error.message, error.code)
        }
        Err(e) => eprintln!("[r23c] interrupt {e:?}"),
    }
    tokio::time::sleep(Duration::from_secs(8)).await;

    drop(client);
    tokio::time::sleep(Duration::from_millis(500)).await;

    let scrubbed = scrub_machine_paths(&std::fs::read_to_string(&path).expect("read back"));
    std::fs::write(&path, &scrubbed).expect("rewrite scrubbed");
    assert!(!scrubbed.contains("/Users/") && !scrubbed.contains("/var/folders/"));
    let has = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .contains("turn/steer_dropped");
    println!("[r23c] steer_dropped recorded: {has}");
}
