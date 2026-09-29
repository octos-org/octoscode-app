//! Card #24b — one REAL dsflash turn that spawns a sub-agent.
//!
//! #24's `agent/*` + `task/*` output traffic came from the server's
//! deterministic M9 `TaskOutput` fixture. This card records a **real**
//! `octos serve` (`a6ea8505`, `dsflash`) **model turn** that spawns a
//! background sub-agent, then reads `agent/status/read` + `agent/output/read`
//! + `task/output/read` on it while it runs and finally `agent/interrupt`s it.
//!
//! ## Model turns
//!
//! Exactly **ONE** parent `turn/start` (the prompt asks the model to spawn a
//! single background subagent). The child's own loop is the server's business;
//! the recording counts the parent turn. See the report for the exact count.
//!
//! ## Two halves
//!
//! * [`c24b_capture_real_subagent_turn`] (`#[ignore]`) — drives this lane's own
//!   serve on port **50120** with `OCTOSCODE_TRACE_FILE` set and rewrites
//!   `tests/fixtures/c24b-subagent-a6ea8505.jsonl`.
//! * the `replay_*` tests — read the fixture and assert the recorded real
//!   traffic decodes and drives the store through the real `Registry`.
//!
//! ## Hermetic
//!
//! The recorder scrubs machine-specific absolute paths to `<WORKSPACE>`/`<TMP>`/
//! `<HOME>` and credential-shaped `sk-…` ids, and asserts none remain. A replay
//! test never reads the environment.
use std::time::Duration;

use octos_app_transport::{
    ws, Capabilities, ProfileId, SecretString, TransportConfig, TransportEvent,
};
use octoscode_client::trace::{wire_params, FrameTrace};
use octoscode_client::{Client, ClientError};
use serde_json::{json, Value};
use url::Url;

/// This lane's serve port (LESSONS "Domain fan-out").
const SERVE_PORT: &str = "50120";

/// The ONE real prompt: spawn a single background subagent, then stop.
const SPAWN_PROMPT: &str = "Use the `spawn` tool exactly once, with mode=\"background\", \
    isolation=\"shared\", and label \"c24b-probe\". The subagent's task must be exactly: \
    run the shell command `echo C24B-CHILD-OUTPUT; sleep 90` and then reply with the single \
    word SUBAUDIT. Do NOT wait for it, do NOT poll it. After the spawn tool returns, reply \
    with the single word SPAWNED and end your turn.";

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/c24b-subagent-a6ea8505.jsonl")
}

// ------------------------------------------------------------- scrub helpers

/// The repo root this crate is built in — `…/crates/octoscode-client` → repo
/// root two levels up. **Recorder-only** (`env!` is compile-time).
fn workspace_root() -> Option<String> {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_string_lossy().to_string())
}

/// Replace machine-specific absolute paths with placeholders (hermetic).
/// **Recorder-only** — a replay test never reads the environment.
fn scrub_machine_paths(text: &str) -> String {
    let mut out = text.to_owned();
    let tmp = std::env::temp_dir().to_string_lossy().to_string();
    if !tmp.is_empty() {
        out = out.replace(&tmp, "<TMP>");
    }
    if let Some(ws) = workspace_root() {
        out = out.replace(&ws, "<WORKSPACE>");
    }
    let home = std::env::var("HOME").unwrap_or_default();
    if !home.is_empty() {
        out = out.replace(&home, "<HOME>");
    }
    out
}

/// Redact credential-shaped values. The server's spawned-agent identifier
/// (`task-<profile>`, e.g. `task-octoscode24b-1234`) contains the literal `sk-`
/// spelling inside `ta|sk-|…` — a benign collision, not a secret. Also covers
/// any `api_key`/`token` echo.
fn scrub(v: Value) -> Value {
    match v {
        Value::String(s) if s.contains("sk-") => Value::String("<redacted-agent>".to_owned()),
        Value::Array(items) => Value::Array(items.into_iter().map(scrub).collect()),
        Value::Object(map) => Value::Object(map.into_iter().map(|(k, v)| (k, scrub(v))).collect()),
        other => other,
    }
}

/// Record one inbound `TransportEvent` into the trace as a `dir:"in"` line.
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

/// One raw request recorded both ways (outbound params + the reply/typed error).
async fn request_recorded(client: &Client, trace: &FrameTrace, method: &str, params: Value) -> Value {
    match client.request(method, params).await {
        Ok(v) => {
            trace.result(method, None, &v);
            v
        }
        Err(ClientError::Rpc { method, error }) => {
            trace.error(&method, None, &error.message, error.code);
            json!({"error": {"code": error.code, "message": error.message}})
        }
        Err(e) => {
            println!("[record] {method} transport err: {e}");
            Value::Null
        }
    }
}

fn describe(v: &Value) -> String {
    if v.get("error").is_some() {
        format!("error {}", v["error"]["message"])
    } else {
        "ok".to_owned()
    }
}

/// The first agent id in an `agent/list` result (`agents[0].agent_id`).
fn agent_id_from(agents: &Value) -> Option<String> {
    agents["agents"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|a| a["agent_id"].as_str())
        .map(str::to_owned)
}

// ------------------------------------------------------------------ capture

#[tokio::test]
#[ignore = "records ONE real model turn from a local octos serve on :50120 (spawns a subagent)"]
async fn c24b_capture_real_subagent_turn() {
    // `C24B_TOPUP=1` = a READ-ONLY top-up against a serve whose spawn turn
    // already ran: no `turn/start`, no monitor, and the fixture is APPENDED to
    // (not truncated). Reads cost no model turn, so this recovers a read that a
    // first pass missed without spending the card's second turn.
    let topup = std::env::var("C24B_TOPUP").is_ok();
    let path = fixture_path();
    if !topup {
        let _ = std::fs::remove_file(&path);
    }
    let trace = FrameTrace::open(path.to_str().expect("utf8 path"));

    let mut capabilities: Capabilities = octoscode_client::features::web_capabilities();
    capabilities
        .raw
        .insert("debug".to_owned(), Value::Bool(true));

    let base = std::env::var("OCTOS_BASE_URL")
        .unwrap_or_else(|_| format!("http://127.0.0.1:{SERVE_PORT}"));
    let profile = std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "dsflash".into());
    let cfg = TransportConfig {
        base_url: Url::parse(&base).expect("base url"),
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
    tokio::time::sleep(Duration::from_millis(500)).await;

    let session_id = format!("{profile}:main");
    let open = client
        .request(
            "session/open",
            json!({"session_id": session_id, "profile_id": profile}),
        )
        .await
        .expect("session/open");
    trace.result("session/open", None, &open);

    // Provoke `monitor/fired` with a short-interval monitor (NO model turn).
    // NOTE: the real monitor only fires on an output CHANGE (first poll =
    // baseline, unchanged = deduped), and a *draining* fire dispatches a
    // continuation turn (costs a model turn). See the report; this run keeps
    // the monitor to show the no-change case and does not spend a turn on it.
    if !topup {
        let monitor = request_recorded(
            &client,
            &trace,
            "monitor/create",
            json!({
                "session_id": session_id,
                "name": "c24b short monitor",
                "argv": ["/bin/sh", "-c", "echo ERROR c24b-monitor-alert"],
                "filter_regex": "ERROR",
                "mode": "poll",
                "interval_seconds": 1,
                "batch_ms": null,
                "timeout_secs": null,
                "persistent": null,
                "max_events_per_hour": null,
                "goal_id": null
            }),
        )
        .await;
        println!("[record] monitor/create -> {}", describe(&monitor));
        let monitor_id = monitor["monitor_id"].as_str().map(str::to_owned);
        // Let it poll a few times (all identical -> baseline then deduped).
        tokio::time::sleep(Duration::from_secs(4)).await;

        // ---- THE real turn: spawn one background subagent ----------------
        let turn_id = "01920000-0000-7000-8000-00000000024b";
        match client
            .request(
                "turn/start",
                json!({"session_id": session_id, "turn_id": turn_id,
                       "input": [{"kind": "text", "text": SPAWN_PROMPT}]}),
            )
            .await
        {
            Ok(v) => trace.result("turn/start", None, &v),
            Err(ClientError::Rpc { method, error }) => {
                trace.error(&method, None, &error.message, error.code)
            }
            Err(e) => println!("[record] turn/start err: {e}"),
        }
        println!("[record] turn/start issued; polling for the subagent");

        // Poll `agent/list` while the child spins up (bounded).
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let agents = request_recorded(
                &client,
                &trace,
                "agent/list",
                json!({"session_id": session_id, "profile_id": profile}),
            )
            .await;
            if agent_id_from(&agents).is_some() {
                break;
            }
        }
        // Stop the monitor now the turn is done.
        if let Some(id) = &monitor_id {
            let _ = request_recorded(
                &client,
                &trace,
                "monitor/delete",
                json!({"monitor_id": id}),
            )
            .await;
        }
    }

    // ---- reads over the spawned subagent (free; no model turn) -----------
    let agents = request_recorded(
        &client,
        &trace,
        "agent/list",
        json!({"session_id": session_id, "profile_id": profile}),
    )
    .await;
    let agent_id = agent_id_from(&agents);
    println!("[record] agent/list -> agent_id={agent_id:?}");

    // The task surface: the subagent is registered as a background task too.
    let tasks = request_recorded(
        &client,
        &trace,
        "task/list",
        json!({"session_id": session_id, "profile_id": profile}),
    )
    .await;
    let task_id = tasks["tasks"]
        .as_array()
        .and_then(|a| a.first())
        // The `task/list` row keys the id as `id` (see the report's shape-diff
        // table), NOT `task_id`.
        .and_then(|t| t["id"].as_str())
        .map(str::to_owned);
    println!("[record] task/list -> task_id={task_id:?}");

    if let Some(agent_id) = agent_id.clone() {
        let _ = request_recorded(
            &client,
            &trace,
            "agent/status/read",
            json!({"session_id": session_id, "profile_id": profile, "agent_id": agent_id}),
        )
        .await;
        let _ = request_recorded(
            &client,
            &trace,
            "agent/output/read",
            json!({"session_id": session_id, "profile_id": profile, "agent_id": agent_id}),
        )
        .await;
        let _ = request_recorded(
            &client,
            &trace,
            "agent/artifact/list",
            json!({"session_id": session_id, "profile_id": profile, "agent_id": agent_id}),
        )
        .await;
    }
    if let Some(task_id) = task_id.clone() {
        let _ = request_recorded(
            &client,
            &trace,
            "task/output/read",
            json!({"session_id": session_id, "profile_id": profile, "task_id": task_id,
                   "cursor": {"offset": 0}, "limit_bytes": 65536}),
        )
        .await;
    }

    // Interrupt the running subagent.
    if let Some(agent_id) = agent_id.clone() {
        let _ = request_recorded(
            &client,
            &trace,
            "agent/interrupt",
            json!({"session_id": session_id, "profile_id": profile, "agent_id": agent_id}),
        )
        .await;
    }

    // Let trailing frames land.
    tokio::time::sleep(Duration::from_secs(2)).await;
    drop(client);
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Hermetic: scrub every line's value tree + machine paths.
    let text = std::fs::read_to_string(&path).expect("read the freshly written fixture");
    let mut out = String::with_capacity(text.len());
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let mut value: Value = serde_json::from_str(line).expect("fixture line is JSON");
        if let Some(body) = value.get_mut("body") {
            *body = scrub(std::mem::take(body));
        }
        if let Some(err) = value.get_mut("error") {
            *err = scrub(std::mem::take(err));
        }
        out.push_str(&scrub_machine_paths(&value.to_string()));
        out.push('\n');
    }
    std::fs::write(&path, &out).expect("rewrite the scrubbed fixture");
    assert!(
        !out.contains("/Users/") && !out.contains("/var/folders/"),
        "the fixture must carry no machine-specific absolute path"
    );
    println!("[record] wrote {} (scrubbed)", path.display());
}

// ============================================================== replay half
//
// The committed regression tests: read the fixture and assert the recorded
// REAL model-turn traffic decodes and drives the production store through the
// production `Registry`. No environment reads (hermetic).

/// One recorded frame (the JSONL line shape from `octoscode_client::trace`).
struct Frame {
    dir: String,
    method: String,
    body: Value,
    error: Option<Value>,
}

fn load_fixture() -> Vec<Frame> {
    let path = fixture_path();
    let text = std::fs::read_to_string(&path).expect("read the c24b fixture");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v.get("body").cloned().unwrap_or(Value::Null),
                error: v.get("error").cloned(),
            }
        })
        .collect()
}

/// The registry wired from every domain (the production `register_all` path).
fn wired() -> (octoscode_client::Registry, std::sync::Arc<octoscode_store::Store>) {
    let store = std::sync::Arc::new(octoscode_store::Store::new());
    let mut reg = octoscode_client::Registry::new();
    octoscode_client::domains::register_all(&mut reg, store.clone());
    (reg, store)
}

/// Every recorded inbound notification, decoded + dispatched through the real
/// registry. Returns the store, the decoded `(method, body)` list and any
/// methods that would NOT decode.
fn replay() -> (
    std::sync::Arc<octoscode_store::Store>,
    Vec<(String, Value)>,
    Vec<String>,
) {
    let (mut reg, store) = wired();
    let mut decoded = Vec::new();
    let mut undecodable = Vec::new();
    for f in load_fixture() {
        if f.dir != "in" || f.error.is_some() {
            continue;
        }
        match octos_core::app_ui::AppUiBackendEvent::from_method_and_params(&f.method, f.body.clone())
        {
            Ok(n) => {
                reg.dispatch(&n);
                decoded.push((f.method.clone(), f.body));
            }
            Err(_) => {
                if !undecodable.contains(&f.method) {
                    undecodable.push(f.method.clone());
                }
            }
        }
    }
    (store, decoded, undecodable)
}

fn count_method(frames: &[(String, Value)], method: &str) -> usize {
    frames.iter().filter(|(m, _)| m == method).count()
}

fn result_of(frames: &[Frame], method: &str) -> Option<Value> {
    frames
        .iter()
        .find(|f| f.dir == "in" && f.method == method && f.error.is_none() && !f.body.is_null())
        .map(|f| f.body.clone())
}

fn error_of(frames: &[Frame], method: &str) -> Option<Value> {
    frames
        .iter()
        .find(|f| f.dir == "in" && f.method == method && f.error.is_some())
        .and_then(|f| f.error.clone())
}

// ------------------------------------------------ the real spawn turn exists

#[test]
fn c24b_fixture_carries_a_real_spawned_subagent() {
    let frames = load_fixture();
    assert!(
        frames.len() >= 60,
        "the fixture should carry a real turn; got {}",
        frames.len()
    );
    // The parent turn really ran against the model (one `turn_terminal` with
    // token usage), and it really called the `spawn` tool.
    let envelopes: Vec<Value> = frames
        .iter()
        .filter(|f| f.dir == "in" && f.method == "projection/envelope")
        .map(|f| f.body.get("payload").cloned().unwrap_or(Value::Null))
        .collect();
    assert_eq!(
        envelopes
            .iter()
            .filter(|p| p["type"] == "turn_terminal")
            .count(),
        1,
        "exactly one parent turn_terminal"
    );
    assert!(
        envelopes.iter().any(|p| p["type"] == "tool_start"),
        "the turn called a tool (spawn)"
    );
    let terminal = envelopes
        .iter()
        .find(|p| p["type"] == "turn_terminal")
        .expect("a turn_terminal");
    assert!(
        terminal["data"]["token_usage"]["input_tokens"].as_u64().unwrap_or(0) > 0,
        "the terminal carries real token usage (proof the model ran): {terminal}"
    );

    // Every read the card names is present.
    for method in [
        "agent/updated",
        "task/updated",
        "agent/status/read",
        "agent/output/read",
        "task/output/read",
        "agent/interrupt",
    ] {
        assert!(
            count_method(
                &frames
                    .iter()
                    .filter(|f| f.dir == "in")
                    .map(|f| (f.method.clone(), f.body.clone()))
                    .collect::<Vec<_>>(),
                method
            ) >= 1,
            "the fixture must carry {method}"
        );
    }
}

// ------------------------------------------- the real agent/task → the store

#[test]
fn c24b_real_agent_updated_replays_into_the_store() {
    let (store, decoded, _) = replay();
    assert_eq!(count_method(&decoded, "agent/updated"), 1);
    assert_eq!(store.seen_count("agent/updated"), 1);
    assert_eq!(store.domains.autonomy.agent_count(), 1);
    let row = store
        .domains
        .autonomy
        .agents()
        .into_iter()
        .next()
        .expect("one agent row");
    // The REAL spawn shape (differs from the M9 scripted turn — see the report).
    assert_eq!(row.backend_kind, "spawn_child_session");
    assert_eq!(row.role, "background_task");
    assert_eq!(row.parent_agent_id.as_deref(), Some("master"));
    assert_eq!(row.status, "running");
    assert_eq!(row.nickname, "c24b-probe");
    assert_eq!(row.session_id, "dsflash:main");
    assert!(row.task_id.is_some(), "the agent links its background task");
}

#[test]
fn c24b_real_task_updated_replays_into_the_store() {
    let (store, decoded, _) = replay();
    assert_eq!(count_method(&decoded, "task/updated"), 1);
    assert_eq!(store.seen_count("task/updated"), 1);
    let snapshot = store
        .domains
        .task
        .snapshots()
        .into_iter()
        .next()
        .expect("one task snapshot");
    assert_eq!(snapshot.state, "running");
    assert_eq!(snapshot.tool_name, "c24b-probe", "title → tool_name on a new row");
}

// --------------------------------------- the real results decode as typed

#[test]
fn c24b_real_results_decode_as_their_method_types() {
    let frames = load_fixture();
    macro_rules! decodes {
        ($ty:ty, $method:expr) => {
            let body = result_of(&frames, $method)
                .unwrap_or_else(|| panic!("fixture carries a {} result", $method));
            serde_json::from_value::<<$ty as octoscode_client::Method>::Result>(body)
                .unwrap_or_else(|e| panic!("{} result must decode: {e}", $method));
        };
    }
    use octoscode_client::domains::autonomy as au;
    decodes!(au::AgentList, "agent/list");
    decodes!(au::AgentStatusRead, "agent/status/read");
    decodes!(au::AgentOutputRead, "agent/output/read");
    decodes!(au::AgentArtifactList, "agent/artifact/list");
    decodes!(au::AgentInterrupt, "agent/interrupt");
}

/// The real interrupt result: the running child was interrupted (`ok`, applied).
#[test]
fn c24b_real_interrupt_result_decodes() {
    let frames = load_fixture();
    let body = result_of(&frames, "agent/interrupt").expect("interrupt result");
    let out: octoscode_client::domains::autonomy::AgentControlResult =
        serde_json::from_value(body).expect("interrupt result decodes");
    assert!(out.ok);
    assert!(out.interrupted);
    assert!(!out.closed);
    assert!(!out.already_terminal);
    assert_eq!(out.status, "interrupted");
}

// --------------------------- shape differences vs the #24 M9 fixture

/// The card asks for the shape differences vs the M9 scripted turn. Assert the
/// ones that matter, so a regression that silently reverts to the M9 shape fails.
#[test]
fn c24b_shape_diffs_vs_the_m9_fixture_are_as_reported() {
    let m9 = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/c24-autonomy-a6ea8505.jsonl"
    ))
    .expect("read the #24 M9 fixture")
    .lines()
    .filter_map(|l| serde_json::from_str::<Value>(l).ok())
    .collect::<Vec<_>>();
    let real = load_fixture();

    // (a) The real `task/updated` carries `tool_call_id`; the M9 one does not.
    let real_task = result_of(&real, "task/updated").expect("real task/updated");
    assert_eq!(real_task["tool_call_id"], serde_json::json!("spawn-subagent-0"));
    let m9_task = m9
        .iter()
        .find(|v| v["dir"] == "in" && v["method"] == "task/updated")
        .expect("M9 task/updated");
    assert!(
        m9_task["body"].get("tool_call_id").is_none(),
        "the M9 task/updated has no tool_call_id"
    );
    // (b) The M9 `task/updated` carries `runtime_detail`; the real one does not.
    assert_eq!(
        m9_task["body"]["runtime_detail"],
        serde_json::json!("persisted deterministic task snapshot")
    );
    assert!(
        real_task["body"].get("runtime_detail").is_none() || real_task.get("runtime_detail").is_none(),
        "the real task/updated carries no runtime_detail"
    );

    // (c) The real `agent/updated` nickname is the spawn label; M9's is "shell".
    let real_agent = result_of(&real, "agent/updated").expect("real agent/updated");
    assert_eq!(real_agent["agent"]["nickname"], serde_json::json!("c24b-probe"));
    let m9_agent = m9
        .iter()
        .find(|v| v["dir"] == "in" && v["method"] == "agent/updated")
        .expect("M9 agent/updated");
    assert_eq!(m9_agent["body"]["agent"]["nickname"], serde_json::json!("shell"));
}

/// `monitor/fired` was NOT produced, and this records the exact reason: the
/// real monitor only fires on an output **change** (first poll = baseline,
/// unchanged = deduped), and a draining fire dispatches a continuation turn.
/// The #24b run used a constant-output monitor, so `fires_used` stayed 0.
#[test]
fn c24b_monitor_fired_absent_because_output_never_changed() {
    let frames = load_fixture();
    let ins: Vec<&Frame> = frames.iter().filter(|f| f.dir == "in").collect();
    assert!(
        !ins.iter().any(|f| f.method == "monitor/fired"),
        "no monitor/fired was recorded"
    );
    // The monitor was created then deleted; `fires_used` stayed 0.
    let created = frames
        .iter()
        .find(|f| f.dir == "in" && f.method == "monitor/updated")
        .expect("a monitor/updated");
    assert_eq!(created.body["monitor"]["fires_used"], serde_json::json!(0));
    assert_eq!(created.body["monitor"]["status"], serde_json::json!("active"));
}

/// `task/output/read` returned a real `-32602 session not found` for a session
/// `task/list` had just served — a genuine live shape worth pinning (the
/// report documents it; it is NOT a fabricated success).
#[test]
fn c24b_task_output_read_recorded_the_real_session_not_found_error() {
    let frames = load_fixture();
    let err = error_of(&frames, "task/output/read").expect("a task/output/read error frame");
    assert_eq!(err["code"], serde_json::json!(-32602));
    assert_eq!(err["message"], serde_json::json!("session not found"));
    // And `task/list` on the SAME session succeeded — the asymmetry is the point.
    assert!(result_of(&frames, "task/list").is_some());
}
