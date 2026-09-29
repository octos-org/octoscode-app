//! Card #23, part 2 — append the domains the first pass missed to the SAME
//! fixture: `approval/requested` + `approval/respond` (+ `approval/decided`),
//! `plan/updated`, and `turn/steer_dropped` (via steer + interrupt).
//!
//! Why the first pass missed them (diagnosed, cited):
//! - `approval/*`: a plain `touch` is `Decision::Allow` under the
//!   workspace-write policy, so no approval is raised. `SafePolicy` returns
//!   `Ask` only for `sudo …` / `git push --force …`
//!   (octos `crates/octos-agent/src/policy.rs:375-382`, test `:436`).
//! - `plan/updated`: produced by the agent's `update_plan` tool
//!   (`crates/octos-agent/src/tools/coding_tools.rs:902`), which the model
//!   only calls if asked. The prompt now names the tool.
//!
//! `FrameTrace::open` appends, so this extends the committed fixture; the
//! end-of-run scrub re-runs over the whole file (idempotent).
//!
//! ```sh
//! OCTOS_BASE_URL=http://127.0.0.1:50160 OCTOS_BEARER=r23-dummy-token \
//!   OCTOS_PROFILE_ID=dsflash ctest -p octoscode-client --test capture23b -- --ignored --nocapture
//! ```
use std::sync::{Arc, Mutex};
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
        out = out.replace(&from, to);
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

#[derive(Default)]
struct Seen {
    approval_id: Option<String>,
    turn_id: Option<String>,
}

const P_ASK: &str =
    "Sandbox policy test. Use the bash tool to run exactly `sudo -n true` (the server intercepts it for approval). Then report what happened, one short line.";
const P_PLAN: &str =
    "Use your update_plan tool to record a 3-step plan for adding a --version flag to a CLI tool. Step 1: add the flag. Step 2: wire --help. Step 3: add a test. Then stop.";
const P_LONG: &str =
    "Write out the numbers from 1 to 60, one per line, slowly. Do not stop early.";

async fn connect(trace: &FrameTrace) -> (Client, Arc<Mutex<Seen>>) {
    let base = std::env::var("OCTOS_BASE_URL")
        .unwrap_or_else(|_| format!("http://127.0.0.1:{SERVE_PORT}"));
    let profile = std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "dsflash".into());
    let cfg = TransportConfig {
        base_url: url::Url::parse(&base).expect("base url"),
        bearer: SecretString::new(
            std::env::var("OCTOS_BEARER").unwrap_or_else(|_| "r23-dummy-token".into()),
        ),
        profile_id: ProfileId::new(profile),
        cursor: None,
        cursor_file: None,
        requested_capabilities: web_capabilities(),
        workspace_cwd: std::env::var("OCTOS_TMP_CWD").ok(),
        local_kernel: false,
    };
    let (cmd_tx, mut evt_rx) = ws::spawn(cfg);
    let t = trace.clone();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let s = seen.clone();
    tokio::spawn(async move {
        while let Some(evt) = evt_rx.recv().await {
            if let TransportEvent::DurableNotification { payload, .. }
            | TransportEvent::EphemeralNotification { payload } = &evt
            {
                let body = wire_params(payload);
                let mut g = s.lock().unwrap();
                if let Some(id) = body.get("approval_id").and_then(|v| v.as_str()) {
                    g.approval_id = Some(id.to_owned());
                }
                if let Some(id) = body.get("turn_id").and_then(|v| v.as_str()) {
                    g.turn_id = Some(id.to_owned());
                }
            }
            record_inbound(&t, &evt);
        }
    });
    (Client::with_trace(cmd_tx, trace.clone()), seen)
}

async fn request(client: &Client, trace: &FrameTrace, method: &str, params: Value) -> Option<Value> {
    match client.request(method, params).await {
        Ok(v) => {
            trace.result(method, None, &scrub(v.clone()));
            Some(v)
        }
        Err(octoscode_client::ClientError::Rpc { method, error }) => {
            trace.error(&method, None, &error.message, error.code);
            None
        }
        Err(e) => {
            eprintln!("capture23b: {method} unexpected {e:?}");
            None
        }
    }
}

async fn ask_then_respond(
    client: &Client,
    trace: &FrameTrace,
    seen: &Arc<Mutex<Seen>>,
    session: &str,
    turn_id: &str,
    decision: &str,
) {
    seen.lock().unwrap().approval_id = None;
    let _ = request(
        client,
        trace,
        "turn/start",
        json!({"session_id": session, "turn_id": turn_id,
               "input": [{"kind": "text", "text": P_ASK}]}),
    )
    .await;
    // Wait for the approval to land (the turn blocks on it).
    for _ in 0..60 {
        if seen.lock().unwrap().approval_id.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    let id = seen.lock().unwrap().approval_id.clone();
    match id {
        Some(id) => {
            let _ = request(
                client,
                trace,
                "approval/respond",
                json!({"session_id": session, "approval_id": id, "decision": decision}),
            )
            .await;
            println!("[r23b] approval {decision} -> respond {id}");
        }
        None => eprintln!("[r23b] approval {decision}: no approval/requested seen"),
    }
    tokio::time::sleep(Duration::from_secs(30)).await;
}

#[tokio::test]
#[ignore = "appends the missing #23 domains; spends real model turns"]
async fn capture_r23_part2() {
    let path = fixture_path();
    let trace = FrameTrace::open(path.to_str().expect("utf8 path"));
    assert!(trace.is_enabled());

    let (client, seen) = connect(&trace).await;
    let profile = std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "dsflash".into());
    let session = format!("{profile}:main");
    let _ = request(
        &client,
        &trace,
        "session/open",
        json!({"session_id": session, "profile_id": profile}),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(800)).await;

    // approval_policy "ask" so a sudo command raises approval/requested.
    let _ = request(
        &client,
        &trace,
        "permission/profile/set",
        json!({"session_id": session,
               "update": {"mode": "workspace_write", "network": "allow", "approval_policy": "ask"}}),
    )
    .await;

    // Turn 7 — approve.
    ask_then_respond(&client, &trace, &seen, &session, "01920000-0000-7000-8000-000000000241", "approve").await;
    println!("[r23b] turn 7 (approval approve) done");

    // Turn 8 — deny.
    ask_then_respond(&client, &trace, &seen, &session, "01920000-0000-7000-8000-000000000242", "deny").await;
    println!("[r23b] turn 8 (approval deny) done");

    // Turn 9 — plan via update_plan.
    let _ = request(
        &client,
        &trace,
        "permission/profile/set",
        json!({"session_id": session, "update": {"approval_policy": "never"}}),
    )
    .await;
    let _ = request(
        &client,
        &trace,
        "turn/start",
        json!({"session_id": session, "turn_id": "01920000-0000-7000-8000-000000000243",
               "input": [{"kind": "text", "text": P_PLAN}]}),
    )
    .await;
    tokio::time::sleep(Duration::from_secs(40)).await;
    println!("[r23b] turn 9 (plan) done");

    // Turn 10 — steer + interrupt: a queued steer the server cannot drain
    // becomes `turn/steer_dropped` (reason "interrupted").
    let long_turn = "01920000-0000-7000-8000-000000000244";
    let _ = request(
        &client,
        &trace,
        "turn/start",
        json!({"session_id": session, "turn_id": long_turn,
               "input": [{"kind": "text", "text": P_LONG}]}),
    )
    .await;
    tokio::time::sleep(Duration::from_secs(6)).await;
    let _ = request(
        &client,
        &trace,
        "turn/steer",
        json!({"session_id": session, "expected_turn_id": long_turn,
               "input": [{"kind": "text", "text": "also mention the word banana"}]}),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    let _ = request(
        &client,
        &trace,
        "turn/interrupt",
        json!({"session_id": session, "turn_id": long_turn}),
    )
    .await;
    tokio::time::sleep(Duration::from_secs(12)).await;
    println!("[r23b] turn 10 (steer + interrupt) done");

    drop(client);
    tokio::time::sleep(Duration::from_millis(500)).await;

    let scrubbed = scrub_machine_paths(&std::fs::read_to_string(&path).expect("read back"));
    std::fs::write(&path, &scrubbed).expect("rewrite scrubbed");
    assert!(
        !scrubbed.contains("/Users/") && !scrubbed.contains("/var/folders/"),
        "the scrub left a machine path"
    );
    println!("[r23b] appended + scrubbed {}", path.display());
}
