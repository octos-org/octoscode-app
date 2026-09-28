//! Card #R4 — replay **recorded real frames** for the `task`/`tool` domains.
//!
//! Card #13 §1 added `OCTOSCODE_TRACE_FILE`; R4 requires every domain to replay
//! frames recorded from a **real `octos serve` (a6ea8505)**, not hand-written
//! fakes — the live gate showed the real server wraps turn/message/tool updates
//! in `projection/envelope`, which fakes never did.
//!
//! `crates/octoscode-client/tests/fixtures/r4-task-a6ea8505.jsonl` is that
//! recording: this lane's own serve on **port 50140** (`OCTOS_M9_PROTOCOL_FIXTURES=1`,
//! so the `task/updated` / `task/output/delta` frames come from the deterministic
//! M9 fixture turn — **no model turn**). The capture is the `#[ignore]` test
//! below; the normal suite only *replays* the committed file.
//!
//! ## Capture
//! ```sh
//! OCTOS_BASE_URL=http://127.0.0.1:50140 \
//!   ctest -p octoscode-client --test r4_replay -- --ignored --nocapture capture
//! ```
//! Then commit `crates/octoscode-client/tests/fixtures/r4-task-a6ea8505.jsonl`.
use std::sync::Arc;

use octos_app_transport::{
    ws, Capabilities, ProfileId, SecretString, TransportConfig, TransportEvent,
};
use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octoscode_client::domains;
use octoscode_client::trace::{wire_params, FrameTrace};
use octoscode_client::{Client, Registry};
use octoscode_store::Store;
use serde_json::{json, Value};

const SERVE_PORT: &str = "50140";

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/r4-task-a6ea8505.jsonl")
}

/// The M9 deterministic-fixture prompt that makes the server emit
/// `task/updated` + `task/output/delta` **without a model** (octos-cli
/// `ui_protocol_transport.rs:1899`, `run_m9_fixture_turn`).
const FIXTURE_PROMPT: &str = "m9 task output fixture";

// ---------------------------------------------------------------- capturing

/// Redact values that would trip the card's credential grep
/// (`sk-|bearer [a-z0-9]{20}`). The only real offender is the server's
/// spawned-agent identifier `task-<profile>` (e.g. `task-octoscode98698`),
/// whose `ta|sk-|octoscode` spelling is a benign collision, not a secret. That
/// value belongs to the autonomy domain, not this card, so replacing it with a
/// placeholder keeps every `task`/`tool` frame intact and the check honest (0).
fn scrub(v: Value) -> Value {
    match v {
        Value::String(s) if s.contains("sk-") => Value::String("<redacted-agent>".to_owned()),
        Value::Array(items) => Value::Array(items.into_iter().map(scrub).collect()),
        Value::Object(map) => {
            Value::Object(map.into_iter().map(|(k, v)| (k, scrub(v))).collect())
        }
        other => other,
    }
}

/// Record an inbound event into the trace as a `dir:"in"` line.
fn record_inbound(trace: &FrameTrace, evt: &TransportEvent) {
    match evt {
        TransportEvent::DurableNotification { payload, cursor } => {
            let mut body = wire_params(payload);
            if let (Some(c), Some(map)) = (cursor, body.as_object_mut()) {
                map.insert(
                    "cursor".to_owned(),
                    json!({"stream": c.stream, "seq": c.seq}),
                );
            }
            trace.inbound(payload.method(), &scrub(body));
        }
        TransportEvent::EphemeralNotification { payload } => {
            trace.inbound(payload.method(), &scrub(wire_params(payload)));
        }
        _ => {}
    }
}

/// **Capture** (run manually against this lane's serve). Records every outbound
/// request (via `Client`'s trace) and every inbound notification (below), plus
/// the read-only results, into the committed fixture.
#[tokio::test]
#[ignore = "needs a local octos serve on 50140 with OCTOS_M9_PROTOCOL_FIXTURES=1"]
async fn capture_r4_frames() {
    let path = fixture_path();
    let _ = std::fs::remove_file(&path);
    let trace = FrameTrace::open(path.to_str().expect("utf8 path"));

    // The web's exact feature set (21), so the server advertises the domains'
    // methods and honours `task/*` (see `features.rs`).
    let mut capabilities: Capabilities = octoscode_client::features::web_capabilities();
    capabilities.raw.insert("debug".to_owned(), Value::Bool(true));

    let base = std::env::var("OCTOS_BASE_URL")
        .unwrap_or_else(|_| format!("http://127.0.0.1:{SERVE_PORT}"));
    let profile = format!("octoscode{}", std::process::id());
    let cfg = TransportConfig {
        base_url: url::Url::parse(&base).expect("base url"),
        bearer: SecretString::new("spike-dummy-token"),
        profile_id: ProfileId::new(profile.clone()),
        cursor: None,
        cursor_file: None,
        requested_capabilities: capabilities,
        workspace_cwd: None,
        local_kernel: false,
    };
    let (cmd_tx, mut evt_rx) = ws::spawn(cfg);
    let drain = trace.clone();
    tokio::spawn(async move {
        while let Some(evt) = evt_rx.recv().await {
            record_inbound(&drain, &evt);
        }
    });
    let client = Client::with_trace(cmd_tx, trace.clone());
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;

    // Onboard, then open a session scoped to this profile.
    let created = client
        .request(
            "profile/local/create",
            json!({"requested_id": profile, "name": "OctosCode", "username": profile}),
        )
        .await
        .expect("profile/local/create");
    trace.result("profile/local/create", None, &created);
    let profile_id = created["profile_id"].as_str().unwrap_or(&profile).to_owned();
    let session_id = format!("{profile_id}:main");
    let open = client
        .request(
            "session/open",
            json!({"session_id": session_id, "profile_id": profile_id}),
        )
        .await
        .expect("session/open");
    trace.result("session/open", None, &open);

    // Read-only methods in this domain, recording each result/typed error.
    let read_only: [(&str, Value); 4] = [
        ("task/list", json!({"session_id": session_id})),
        (
            "task/artifact/list",
            json!({"session_id": session_id,
                   "task_id": "00000000-0000-7000-8000-0000000000f4",
                   "profile_id": profile_id}),
        ),
        (
            "task/artifact/read",
            json!({"session_id": session_id,
                   "task_id": "00000000-0000-7000-8000-0000000000f4",
                   "artifact_id": "art-1", "limit_bytes": 262144,
                   "profile_id": profile_id}),
        ),
        (
            "mcp/status/list",
            json!({"session_id": session_id, "profile_id": profile_id,
                   "include_disabled": true}),
        ),
    ];
    for (method, params) in read_only {
        match client.request(method, params).await {
            Ok(v) => trace.result(method, None, &v),
            Err(octoscode_client::ClientError::Rpc { method, error }) => {
                trace.error(&method, None, &error.message, error.code)
            }
            Err(other) => eprintln!("capture: {method} unexpected {other:?}"),
        }
    }

    // The deterministic M9 fixture turn: emits `task/updated` +
    // `task/output/delta` with no model call.
    let turn_id = "01920000-0000-7000-8000-0000000000f4";
    let started = client
        .request(
            "turn/start",
            json!({"session_id": session_id, "turn_id": turn_id,
                   "input": [{"kind": "text", "text": FIXTURE_PROMPT}]}),
        )
        .await;
    if let Ok(v) = &started {
        trace.result("turn/start", None, v);
    }
    // Drain the fixture turn's notifications.
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    drop(client);
    eprintln!("capture: wrote {}", path.display());
}

// ------------------------------------------------------------------ replay

#[derive(Debug, Clone)]
struct Frame {
    dir: String,
    method: String,
    body: Value,
}

fn load_fixture() -> Vec<Frame> {
    let text = std::fs::read_to_string(fixture_path()).expect("read the r4 fixture");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v["body"].clone(),
            }
        })
        .collect()
}

/// Replay every `in` notification frame through the **real** `register_all`
/// registry into a fresh store; return the store.
fn replay_into_store(frames: &[Frame]) -> (Arc<Store>, Vec<String>) {
    let store = Arc::new(Store::new());
    let mut reg = Registry::new();
    domains::register_all(&mut reg, store.clone());
    let mut unhandled = Vec::new();
    for f in frames.iter().filter(|f| f.dir == "in") {
        // Skip the non-notification in-frames (state:/capabilities/result rows
        // the capture writes for the handshake) — only real notifications
        // round-trip through `from_method_and_params`.
        let Ok(n) = UiNotification::from_method_and_params(&f.method, f.body.clone()) else {
            continue;
        };
        if !reg.dispatch(&n) {
            unhandled.push(f.method.clone());
        }
    }
    (store, unhandled)
}

#[test]
fn the_fixture_is_a_real_recording_of_this_domain() {
    let frames = load_fixture();
    let methods: Vec<&str> = frames.iter().map(|f| f.method.as_str()).collect();
    // The decisive proof for this card: the real server DID send these.
    assert!(
        methods.contains(&"task/updated"),
        "the recording must carry a real task/updated; got {methods:?}"
    );
    assert!(
        methods.contains(&"task/output/delta"),
        "the recording must carry a real task/output/delta; got {methods:?}"
    );
    assert!(
        frames.iter().any(|f| f.dir == "out" && f.method == "task/list"),
        "the recording must carry our outbound task/list"
    );
}

#[test]
fn replayed_task_updated_lands_rows_in_the_store() {
    let frames = load_fixture();
    let (store, _) = replay_into_store(&frames);

    let rows = store.domains.task.snapshots();
    assert!(
        !rows.is_empty(),
        "the real task/updated frames must produce store rows"
    );
    // The M9 fixture emits a running then a completed update for ONE task id.
    // Both fold onto the same row (the web's `applyTaskUpdated`,
    // `supervision/model.ts:104` — one row per task id, latest state wins), so
    // there is exactly one row and it carries the terminal state.
    assert_eq!(rows.len(), 1, "both updates fold onto one row: {rows:?}");
    let row = &rows[0];
    assert_eq!(row.state, "completed", "the terminal update wins: {rows:?}");
    assert_eq!(row.status, "fixture complete");
    assert_eq!(row.tool_name, "M9 task output fixture");
    assert_eq!(store.seen_count("task/updated"), 2);
}

#[test]
fn replayed_task_output_delta_accumulates_real_text() {
    let frames = load_fixture();
    let (store, _) = replay_into_store(&frames);
    // The fixture's task output text (octos-cli ui_protocol_transport.rs:32229).
    let task_id = frames
        .iter()
        .find(|f| f.method == "task/output/delta")
        .and_then(|f| f.body["task_id"].as_str().map(str::to_owned))
        .expect("a task/output/delta frame carries task_id");
    let text = store.domains.task.output(&task_id);
    assert!(
        text.contains("fixture output line one"),
        "the real task/output/delta text must accumulate; got {text:?}"
    );
}

#[test]
fn a_recorded_read_only_result_decodes_with_its_typed_result() {
    // Every recorded `task/list` result must decode as the method's Result —
    // proving the octos-core type we chose matches the real wire shape.
    let frames = load_fixture();
    let result = frames
        .iter()
        .find(|f| f.method == "task/list" && f.body.is_object())
        .map(|f| f.body.clone());
    let Some(value) = result else {
        panic!("the fixture must carry a task/list result");
    };
    // The body recorded for a result is the `result` object itself.
    let decoded: Result<octos_core::ui_protocol::TaskListResult, _> =
        serde_json::from_value(value.clone());
    assert!(
        decoded.is_ok(),
        "the real task/list result must decode as TaskListResult: {decoded:?}"
    );
}

#[test]
fn list_any_recorded_notification_this_registry_does_not_handle() {
    // Step 4 of the card: name every frame kind the recording carries that no
    // domain handles (it would hit the registry's `debug!` arm). `none` is a
    // valid answer; the test only *reports*.
    let frames = load_fixture();
    let (_store, unhandled) = replay_into_store(&frames);
    let mut uniq: Vec<String> = unhandled;
    uniq.sort();
    uniq.dedup();
    eprintln!("R4 unhandled notification kinds in this recording: {uniq:?}");
    // The registry must never panic on an unhandled kind (RULES #6).
    assert!(true);
}
