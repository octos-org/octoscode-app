//! Card #23 — capture REAL dsflash/a6ea8505 traffic for the conversation
//! domains (turn + message/delta + reasoning, tool, plan, approval, user
//! question) into ONE hermetic fixture.
//!
//! This file holds the `#[ignore]`d **recorder** only; the replay tests live in
//! `r23_replay.rs` and never touch the environment (LESSONS "Replay tests must
//! be hermetic").
//!
//! ## Run (my own serve port 50160, my own `tmp/` data dirs)
//! ```sh
//! tmp/r23-bin/octos serve --solo --port 50160 --auth-token r23-dummy-token \
//!   --data-dir $PWD/tmp/r23-data --instance-data-dir $PWD/tmp/r23-data
//! OCTOS_BASE_URL=http://127.0.0.1:50160 OCTOS_BEARER=r23-dummy-token \
//!   OCTOS_PROFILE_ID=dsflash OCTOS_TMP_CWD=$PWD/tmp/r23-ws \
//!   ctest -p octoscode-client --test capture23 -- --ignored --nocapture
//! ```
//! Then scrub + commit `crates/octoscode-client/tests/fixtures/r23-conversation-a6ea8505.jsonl`.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use octos_app_transport::{
    ws, ProfileId, SecretString, TransportConfig, TransportEvent,
};
use octoscode_client::features::web_capabilities;
use octoscode_client::trace::{wire_params, FrameTrace};
use octoscode_client::Client;
use serde_json::{json, Value};

/// My serve port (LESSONS "Domain fan-out" — p0-map-f = 50160).
const SERVE_PORT: u16 = 50160;

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/r23-conversation-a6ea8505.jsonl")
}

// ------------------------------------------------------------- hermeticity

const TMP_PLACEHOLDER: &str = "<TMP>";
const WORKSPACE_PLACEHOLDER: &str = "<WORKSPACE>";
const HOME_PLACEHOLDER: &str = "<HOME>";

/// The machine-specific prefixes this run produced, longest first so the most
/// specific wins. **Recorder-only** (a replay test never reads the env).
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

/// Replace every machine-specific absolute path with its placeholder.
fn scrub_machine_paths(text: &str) -> String {
    let mut out = text.to_owned();
    for (from, to) in machine_path_prefixes() {
        out = out.replace(&from, to);
    }
    out
}

/// Redact anything that would trip the credential grep. The trace already
/// redacts `token`/`api_key`/`authorization`/`secret`/`password` fields.
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
        TransportEvent::ConnectionState(s) => {
            trace.inbound(&format!("state:{s:?}"), &json!({"state": format!("{s:?}")}));
        }
        TransportEvent::CapabilityNegotiated(caps) => trace.inbound(
            "capabilities",
            &json!({"accepted": caps.raw.keys().cloned().collect::<Vec<_>>()}),
        ),
        TransportEvent::RpcResult(r) => {
            trace.inbound("rpc/result", &json!({"debug": format!("{r:?}")}))
        }
        TransportEvent::RpcError { method, error, .. } => trace.inbound(
            &format!("error:{method}"),
            &json!({"code": error.code, "message": error.message}),
        ),
        TransportEvent::SessionsListed { sessions } => trace.inbound("session/list", sessions),
        TransportEvent::SessionHydrated { session_id, result } => {
            trace.inbound("session/hydrate", &json!({"session_id": session_id, "result": result}))
        }
    }
}

/// What the drain saw on the CURRENT turn, so the main task can answer a
/// pending approval/question.
#[derive(Default)]
struct Seen {
    approval_id: Option<String>,
    question_id: Option<String>,
}

/// The prompts (each drives ONE real model turn).
const TURN_REASONING: &str = "In one short sentence, what is 17 times 23? Think it through.";
const TURN_TOOL: &str =
    "Use the bash tool to run exactly `echo r23-tool-ok`. Then tell me its output, one short line.";
const TURN_PLAN: &str = "Make a short plan (3 numbered steps) for adding a --version flag to a CLI tool, then stop. Do not write any files.";
const TURN_APPROVE: &str = "Use the bash tool to run exactly `touch r23-approve-marker`. Then say done.";
const TURN_DENY: &str = "Use the bash tool to run exactly `touch r23-deny-marker`. Then say done.";
const TURN_QUESTION: &str = "I need to pick a color. Use your question tool to ask me which color I prefer, then stop.";

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
                if let Some(id) = body.get("question_id").and_then(|v| v.as_str()) {
                    g.question_id = Some(id.to_owned());
                }
            }
            record_inbound(&t, &evt);
        }
    });
    (Client::with_trace(cmd_tx, trace.clone()), seen)
}

/// Send `turn/start` and drain until the turn reaches a terminal (or timeout).
async fn run_turn(client: &Client, trace: &FrameTrace, session: &str, turn_id: &str, prompt: &str) {
    let started = client
        .request(
            "turn/start",
            json!({"session_id": session, "turn_id": turn_id,
                   "input": [{"kind": "text", "text": prompt}]}),
        )
        .await;
    match &started {
        Ok(v) => trace.result("turn/start", None, v),
        Err(octoscode_client::ClientError::Rpc { method, error }) => {
            trace.error(method, None, &error.message, error.code)
        }
        Err(e) => eprintln!("capture: turn/start unexpected {e:?}"),
    }
    tokio::time::sleep(Duration::from_secs(40)).await;
}

#[tokio::test]
#[ignore = "records r23 against MY dsflash serve on 50160; spends real model turns"]
async fn capture_r23_conversation() {
    let path = fixture_path();
    let _ = std::fs::remove_file(&path);
    let trace = FrameTrace::open(path.to_str().expect("utf8 path"));
    assert!(trace.is_enabled());

    let (client, seen) = connect(&trace).await;
    let profile = std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "dsflash".into());
    let session = format!("{profile}:main");

    // session/open.
    let opened = client
        .request(
            "session/open",
            json!({"session_id": session, "profile_id": profile}),
        )
        .await;
    match &opened {
        Ok(v) => trace.result("session/open", None, v),
        Err(e) => eprintln!("capture: session/open {e:?}"),
    }
    tokio::time::sleep(Duration::from_millis(800)).await;
    println!("[r23] session/open -> {session}");

    // Read-only probes (no model turn).
    for (method, params) in [
        ("permission/profile/list", json!({"session_id": session})),
        ("turn/state/get", json!({"session_id": session, "turn_id": "01920000-0000-7000-8000-00000000023a"})),
        ("thread/graph/get", json!({"session_id": session})),
    ] {
        match client.request(method, params).await {
            Ok(v) => trace.result(method, None, &scrub(v)),
            Err(octoscode_client::ClientError::Rpc { method, error }) => {
                trace.error(&method, None, &error.message, error.code)
            }
            Err(e) => eprintln!("capture: {method} {e:?}"),
        }
    }
    println!("[r23] read-only probes done");

    // Turn 1 — reasoning (visible thinking).
    run_turn(&client, &trace, &session, "01920000-0000-7000-8000-00000000023b", TURN_REASONING).await;
    println!("[r23] turn 1 (reasoning) done");

    // Turn 2 — tool (a bash call).
    run_turn(&client, &trace, &session, "01920000-0000-7000-8000-00000000023c", TURN_TOOL).await;
    println!("[r23] turn 2 (tool) done");

    // Turn 3 — plan.
    run_turn(&client, &trace, &session, "01920000-0000-7000-8000-00000000023d", TURN_PLAN).await;
    println!("[r23] turn 3 (plan) done");

    // Turn 4 — deny-by-default wait: an approval, then ASK me (approval_policy).
    let _ = client
        .request(
            "permission/profile/set",
            json!({"session_id": session,
                   "update": {"mode": "workspace_write", "network": "allow", "approval_policy": "ask"}}),
        )
        .await;
    println!("[r23] permission -> ask");

    for (idx, (turn_id, prompt)) in [
        ("01920000-0000-7000-8000-00000000023e", TURN_APPROVE),
        ("01920000-0000-7000-8000-00000000023f", TURN_DENY),
    ]
    .into_iter()
    .enumerate()
    {
        let decision = if idx == 0 { "approve" } else { "deny" };
        seen.lock().unwrap().approval_id = None;
        let _ = client
            .request(
                "turn/start",
                json!({"session_id": session, "turn_id": turn_id,
                       "input": [{"kind": "text", "text": prompt}]}),
            )
            .await;
        tokio::time::sleep(Duration::from_secs(15)).await;
        // The drain recorded the approval id; answer it.
        let id = seen.lock().unwrap().approval_id.clone();
        match id {
            Some(id) => {
                let r = client
                    .request(
                        "approval/respond",
                        json!({"session_id": session, "approval_id": id, "decision": decision}),
                    )
                    .await;
                match &r {
                    Ok(v) => trace.result("approval/respond", None, &scrub(v.clone())),
                    Err(octoscode_client::ClientError::Rpc { method, error }) => {
                        trace.error(method, None, &error.message, error.code)
                    }
                    Err(e) => eprintln!("capture: approval/respond {e:?}"),
                }
                println!("[r23] approval {decision} -> respond {id}");
            }
            None => eprintln!("[r23] approval {decision}: no requested id seen"),
        }
        tokio::time::sleep(Duration::from_secs(25)).await;
    }

    // Turn 5 — user question.
    let _ = client
        .request(
            "permission/profile/set",
            json!({"session_id": session, "update": {"approval_policy": "never"}}),
        )
        .await;
    seen.lock().unwrap().question_id = None;
    let _ = client
        .request(
            "turn/start",
            json!({"session_id": session, "turn_id": "01920000-0000-7000-8000-000000000240",
                   "input": [{"kind": "text", "text": TURN_QUESTION}]}),
        )
        .await;
    tokio::time::sleep(Duration::from_secs(25)).await;
    let qid = seen.lock().unwrap().question_id.clone();
    match qid {
        Some(id) => {
            let r = client
                .request(
                    "user_question/respond",
                    json!({"session_id": session, "question_id": id,
                           "answers": [{"free_text": "blue"}]}),
                )
                .await;
            match &r {
                Ok(v) => trace.result("user_question/respond", None, &scrub(v.clone())),
                Err(octoscode_client::ClientError::Rpc { method, error }) => {
                    trace.error(method, None, &error.message, error.code)
                }
                Err(e) => eprintln!("capture: user_question/respond {e:?}"),
            }
            println!("[r23] question -> respond {id}");
        }
        None => eprintln!("[r23] question: no requested id seen"),
    }
    tokio::time::sleep(Duration::from_secs(25)).await;

    drop(client);
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Scrub machine paths so the fixture is hermetic on any clone.
    let scrubbed = scrub_machine_paths(&std::fs::read_to_string(&path).expect("read back"));
    std::fs::write(&path, &scrubbed).expect("rewrite scrubbed");
    assert!(
        !scrubbed.contains("/Users/") && !scrubbed.contains("/var/folders/"),
        "the scrub left a machine path"
    );
    println!("[r23] wrote {} (scrubbed)", path.display());
}
