//! Card #24 — real dsflash traffic for the autonomy / background domains
//! (`agent`, `task`, `monitor`, `loop`, `goal`, `peer`, `visual`).
//!
//! Card #13 added `OCTOSCODE_TRACE_FILE` (a JSONL frame recorder at the client
//! boundary, `octoscode_client::trace`). Card #24 requires every remaining
//! protocol domain to replay **frames recorded from a real `octos serve`
//! (`a6ea8505`, the `dsflash` profile)**, not only hand-written fakes — the live
//! gate showed the real server wraps turn/message/tool updates in
//! `projection/envelope`, which fakes never did.
//!
//! ## Two halves
//!
//! * [`c24_capture_frames`] (`#[ignore]`) — drives **this lane's own** serve on
//!   port **50120** with `OCTOSCODE_TRACE_FILE` set and rewrites the committed
//!   fixture `tests/fixtures/c24-autonomy-a6ea8505.jsonl`.
//! * the `replay_*` tests — read the fixture and assert the recorded real
//!   traffic decodes and drives the store through the **real** `Registry`.
//!
//! ## Model turns: 0
//!
//! The `task/*` and `agent/*` notifications only appear during a turn, so the
//! capture uses the server's **deterministic M9 fixture turn** (`OCTOS_M9_PROTOCOL_FIXTURES=1`,
//! prompt `m9 task output fixture`), which emits `task/updated`,
//! `task/output/delta` **and** `agent/updated` with **no LLM inference** — the
//! same mechanism R4/R5 used. Reads are free; every mutation runs only against
//! the throwaway `tmp/24/` data dir (create then delete what it creates).
//!
//! ## Hermetic (R2 lesson, enforced by `fixtures_hermetic.rs`)
//!
//! The recorder scrubs machine-specific absolute paths to `<WORKSPACE>` /
//! `<TMP>` / `<HOME>` before writing, and asserts none remain. This is the only
//! place the recorder reads the environment; a replay test never does.
use std::sync::Arc;
use std::time::Duration;

use octos_app_transport::{
    ws, Capabilities, ProfileId, SecretString, TransportConfig, TransportEvent,
};
use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octoscode_client::domains::autonomy as au;
use octoscode_client::trace::{wire_params, FrameTrace};
use octoscode_client::{Client, ClientError, Registry};
use octoscode_store::Store;
use serde_json::{json, Value};
use url::Url;

/// This lane's serve port (LESSONS "Domain fan-out: your own `octos serve` port").
const SERVE_PORT: &str = "50120";

/// The M9 deterministic-fixture prompt; emits `task/updated` +
/// `task/output/delta` + `agent/updated` **without a model**
/// (`…ui_protocol_transport.rs:1899`, `run_m9_fixture_turn`, `TaskOutput`).
const FIXTURE_PROMPT: &str = "m9 task output fixture";

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/c24-autonomy-a6ea8505.jsonl")
}

// ------------------------------------------------------------- scrub helpers

/// The repo root this crate is built in — `…/crates/octoscode-client` → the repo
/// root two levels up. **Recorder-only** (`env!` is compile-time).
fn workspace_root() -> Option<String> {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_string_lossy().to_string())
}

/// Replace machine-specific absolute paths with placeholders so the committed
/// fixture stays **hermetic**. Longest/most-specific first: the system temp dir
/// may sit inside `$HOME`. **Recorder-only** — a replay test never reads the env.
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

/// Redact values that would trip the card's credential grep
/// (`sk-|bearer [a-z0-9]{20}`). The server's spawned-agent identifier
/// (`task-<profile>`, e.g. `task-octoscode24-1234`) contains the literal
/// `sk-` spelling inside `ta|sk-|…` — a benign collision, not a secret; the
/// same redaction R4 used. Also covers any real `api_key`/`token` echo.
fn scrub(v: Value) -> Value {
    match v {
        Value::String(s) if s.contains("sk-") => Value::String("<redacted-agent>".to_owned()),
        Value::Array(items) => Value::Array(items.into_iter().map(scrub).collect()),
        Value::Object(map) => Value::Object(map.into_iter().map(|(k, v)| (k, scrub(v))).collect()),
        other => other,
    }
}

/// Record an inbound `TransportEvent` into the trace as a `dir:"in"` line.
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

// ------------------------------------------------------------------ capture

#[tokio::test]
#[ignore = "records real frames from a local octos serve on :50120 with OCTOS_M9_PROTOCOL_FIXTURES=1"]
async fn c24_capture_frames() {
    let path = fixture_path();
    let _ = std::fs::remove_file(&path);
    let trace = FrameTrace::open(path.to_str().expect("utf8 path"));

    // The web's exact feature set (21), so the server advertises (and honours)
    // the autonomy/background domains (see `features.rs`).
    let mut capabilities: Capabilities = octoscode_client::features::web_capabilities();
    capabilities
        .raw
        .insert("debug".to_owned(), Value::Bool(true));

    let base = std::env::var("OCTOS_BASE_URL")
        .unwrap_or_else(|_| format!("http://127.0.0.1:{SERVE_PORT}"));
    let profile = format!("octoscode24-{}", std::process::id());
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

    // Onboard, then open a session scoped to this throwaway profile.
    let created = client
        .request(
            "profile/local/create",
            json!({"requested_id": profile, "name": "OctosCode 24", "username": profile}),
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

    // ---- read-only autonomy reads (production Client path) ----------------
    let list = || au::AutonomyListParams {
        session_id: Some(session_id.clone()),
        profile_id: Some(profile_id.clone()),
    };
    // Reads go through `request_recorded` so BOTH the outbound frame and the
    // server's real result line land in the fixture — `Client::call` delivers
    // the result only through its oneshot, which the drain never sees. The
    // macro uses each type's exact wire `NAME`, so the recorded method matches
    // the domain, and the replay test decodes each result into its typed
    // `Method::Result`.
    macro_rules! read_typed {
        ($label:expr, $ty:ty, $params:expr) => {{
            let params_value =
                serde_json::to_value(&$params).expect("autonomy params serialize");
            let r = request_recorded(
                &client,
                &trace,
                <$ty as octoscode_client::Method>::NAME,
                params_value,
            )
            .await;
            println!("[record] {} -> {}", $label, describe(&r));
        }};
    }

    read_typed!("agent/list", au::AgentList, list());
    read_typed!("loop/list", au::LoopList, list());
    read_typed!("monitor/list", au::MonitorList, list());
    let goal_params = || au::GoalSessionParams {
        session_id: session_id.clone(),
        profile_id: Some(profile_id.clone()),
    };
    read_typed!("session/goal/get", au::GoalGet, goal_params());

    // ---- loop: create → list → pause → resume → fire_now → delete --------
    let mut loop_id = None;
    match client
        .call::<au::LoopCreate>(au::LoopCreateParams {
            session_id: session_id.clone(),
            prompt: Some("c24 replay probe".to_owned()),
            command: None,
            interval_seconds: Some(3600),
            mode: Some("fixed_interval".to_owned()),
        })
        .await
    {
        Ok(r) => {
            println!("[record] loop/create -> {} ({})", r.loop_id, r.status);
            loop_id = Some(r.loop_id);
        }
        Err(e) => println!("[record] loop/create err: {e}"),
    }
    tokio::time::sleep(Duration::from_millis(700)).await;
    if let Some(id) = loop_id.clone() {
        let _ = read_typed!("loop/list", au::LoopList, list());
        let _ = read_typed!("loop/pause", au::LoopPause, au::LoopControlParams::new(&id));
        let _ = read_typed!("loop/resume", au::LoopResume, au::LoopControlParams::new(&id));
        let _ = read_typed!("loop/fire_now", au::LoopFireNow, au::LoopControlParams::new(&id));
        let _ = read_typed!("loop/delete", au::LoopDelete, au::LoopControlParams::new(&id));
    }

    // ---- monitor: create → list → pause → resume → delete ----------------
    let mut monitor_id = None;
    match client
        .call::<au::MonitorCreate>(au::MonitorCreateParams {
            session_id: session_id.clone(),
            name: "c24 replay monitor".to_owned(),
            argv: vec!["./scripts/watch.sh".to_owned()],
            filter_regex: Some("ERROR".to_owned()),
            mode: Some("poll".to_owned()),
            interval_seconds: Some(3600),
            batch_ms: None,
            timeout_secs: None,
            persistent: None,
            max_events_per_hour: None,
            goal_id: None,
        })
        .await
    {
        Ok(r) => {
            println!("[record] monitor/create -> {} ({})", r.monitor_id, r.status);
            monitor_id = Some(r.monitor_id);
        }
        Err(e) => println!("[record] monitor/create err: {e}"),
    }
    tokio::time::sleep(Duration::from_millis(700)).await;
    if let Some(id) = monitor_id.clone() {
        let _ = read_typed!("monitor/list", au::MonitorList, list());
        let _ = read_typed!("monitor/pause", au::MonitorPause, au::MonitorControlParams::new(&id));
        let _ = read_typed!("monitor/resume", au::MonitorResume, au::MonitorControlParams::new(&id));
        let _ = read_typed!("monitor/delete", au::MonitorDelete, au::MonitorControlParams::new(&id));
    }

    // ---- session goal: set → get → clear ---------------------------------
    let _ = read_typed!(
        "session/goal/set",
        au::GoalSet,
        au::GoalSetParams {
            session_id: session_id.clone(),
            objective: "c24 replay probe".to_owned(),
            profile_id: Some(profile_id.clone()),
            status: None,
            token_budget: None,
            transition_actor: Some("user".to_owned()),
        }
    );
    let _ = read_typed!("session/goal/get", au::GoalGet, goal_params());
    let _ = read_typed!("session/goal/clear", au::GoalClear, goal_params());

    // ---- peer: prepare + gather ------------------------------------------
    // `peer/prepare` needs a workspace root; an explicit `cwd` under the
    // throwaway `tmp/24` dir supplies one (`…ui_protocol_transport.rs`:
    // "explicit cwd … beats the calling session's root").
    let peer_cwd = std::env::temp_dir().join(format!("c24-peer-{profile_id}"));
    let _ = std::fs::create_dir_all(&peer_cwd);
    for (method, params) in [
        (
            "peer/prepare",
            json!({"session_id": session_id, "profile_id": profile_id,
                   "brief": "c24 replay probe brief", "n": 1, "title": "c24 peer",
                   "cwd": peer_cwd.to_string_lossy()}),
        ),
        ("peer/gather", json!({"profile_id": profile_id})),
    ] {
        let r = request_recorded(&client, &trace, method, params).await;
        println!("[record] {method} -> {}", describe(&r));
    }
    // Let a `peer/staged` / `peer/closed` land if the serve emits one.
    tokio::time::sleep(Duration::from_millis(800)).await;

    // ---- the deterministic fixture turn: task/* + agent/* with NO model ---
    let turn_id = "01920000-0000-7000-8000-000000000024";
    match client
        .request(
            "turn/start",
            json!({"session_id": session_id, "turn_id": turn_id,
                   "input": [{"kind": "text", "text": FIXTURE_PROMPT}]}),
        )
        .await
    {
        Ok(v) => trace.result("turn/start", None, &v),
        Err(ClientError::Rpc { method, error }) => {
            trace.error(&method, None, &error.message, error.code)
        }
        Err(e) => println!("[record] turn/start err: {e}"),
    }
    // Drain the fixture's notifications (task/updated ×2, task/output/delta,
    // agent/updated, projection/envelope, …).
    tokio::time::sleep(Duration::from_secs(3)).await;

    // ---- task/* + agent/* reads over the seeded fixture ------------------
    let tasks = request_recorded(
        &client,
        &trace,
        "task/list",
        json!({"session_id": session_id, "profile_id": profile_id}),
    )
    .await;
    let task_id = tasks["tasks"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|t| t["task_id"].as_str())
        .map(str::to_owned);
    if let Some(task_id) = task_id {
        let _ = request_recorded(
            &client,
            &trace,
            "task/output/read",
            json!({"session_id": session_id, "task_id": task_id,
                   "profile_id": profile_id, "cursor": {"offset": 0}, "limit_bytes": 65536}),
        )
        .await;
    }

    let agents = request_recorded(
        &client,
        &trace,
        "agent/list",
        json!({"session_id": session_id, "profile_id": profile_id}),
    )
    .await;
    let agent_id = agents["agents"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|a| a["agent_id"].as_str())
        .map(str::to_owned);
    if let Some(agent_id) = agent_id {
        let _ = request_recorded(
            &client,
            &trace,
            "agent/status/read",
            json!({"session_id": session_id, "profile_id": profile_id, "agent_id": agent_id}),
        )
        .await;
        let _ = request_recorded(
            &client,
            &trace,
            "agent/output/read",
            json!({"session_id": session_id, "profile_id": profile_id, "agent_id": agent_id}),
        )
        .await;
        let _ = request_recorded(
            &client,
            &trace,
            "agent/artifact/list",
            json!({"session_id": session_id, "profile_id": profile_id, "agent_id": agent_id}),
        )
        .await;
        let _ = request_recorded(
            &client,
            &trace,
            "agent/interrupt",
            json!({"session_id": session_id, "profile_id": profile_id, "agent_id": agent_id}),
        )
        .await;
        let _ = request_recorded(
            &client,
            &trace,
            "agent/close",
            json!({"session_id": session_id, "profile_id": profile_id, "agent_id": agent_id}),
        )
        .await;
    }

    // Let trailing frames land, then flush.
    tokio::time::sleep(Duration::from_secs(1)).await;
    drop(client);
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Hermetic (R2 lesson): scrub lane-specific absolute paths AND credential
    // collisions out of every line uniformly. `trace.result(...)` (the oneshot
    // replies I record directly) bypasses the drain's per-frame `scrub`, so the
    // post-pass re-scrubs each JSON line's value tree the same way the drain
    // does, then replaces machine paths in the serialized text.
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

/// One raw request recorded both ways (outbound params + the reply/typed error).
async fn request_recorded(
    client: &Client,
    trace: &FrameTrace,
    method: &str,
    params: Value,
) -> Value {
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

// The replay half (assertions over the recorded fixture) lives below.

// ============================================================== replay half
//
// The committed regression tests: read the fixture and assert the recorded
// REAL traffic decodes and drives the production store through the production
// `Registry`. No environment reads (hermetic).

/// One recorded frame (the JSONL line shape from `octoscode_client::trace`).
struct Frame {
    dir: String,
    method: String,
    body: Value,
    error: Option<Value>,
}

fn load_fixture() -> Vec<Frame> {
    let path = fixture_path();
    let text = std::fs::read_to_string(&path).expect("read the c24 fixture");
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
fn wired() -> (Registry, Arc<Store>) {
    let store = Arc::new(Store::new());
    let mut reg = Registry::new();
    octoscode_client::domains::register_all(&mut reg, store.clone());
    (reg, store)
}

/// Every recorded inbound *notification*, decoded by the transport's own
/// contract decoder. A frame that is not a notification method (a lifecycle
/// reply, a connection-state line, a capability handshake) simply fails to
/// decode and is skipped — the same filter R1 uses.
fn replayed_notifications() -> (Registry, Arc<Store>, Vec<(String, Value)>, Vec<String>) {
    let (mut reg, store) = wired();
    let mut decoded: Vec<(String, Value)> = Vec::new();
    let mut undecodable: Vec<String> = Vec::new();
    for f in load_fixture() {
        if f.dir != "in" || f.error.is_some() {
            continue;
        }
        match UiNotification::from_method_and_params(&f.method, f.body.clone()) {
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
    (reg, store, decoded, undecodable)
}

fn count_method(frames: &[(String, Value)], method: &str) -> usize {
    frames.iter().filter(|(m, _)| m == method).count()
}

/// The first body for `method` in the decoded set.
fn body_of<'a>(frames: &'a [(String, Value)], method: &str) -> Option<&'a Value> {
    frames.iter().find(|(m, _)| m == method).map(|(_, b)| b)
}

#[test]
fn c24_fixture_is_present_and_covers_the_domains() {
    let frames = load_fixture();
    assert!(
        frames.len() >= 40,
        "the fixture should carry a real recording; got {}",
        frames.len()
    );
    let (_, _, decoded, _) = replayed_notifications();
    // The real notifications this card targets — each really recorded.
    for method in [
        "agent/updated",
        "loop/updated",
        "loop/fired",
        "monitor/updated",
        "session/goal/updated",
        "session/goal/cleared",
        "task/updated",
        "task/output/delta",
    ] {
        assert!(
            count_method(&decoded, method) >= 1,
            "the recording must carry a real {method}; got {:?}",
            decoded.iter().map(|(m, _)| m).collect::<Vec<_>>()
        );
    }
}

#[test]
fn c24_autonomy_notifications_replay_into_the_store() {
    let (_, store, decoded, _) = replayed_notifications();

    // ---- loops: real create→pause→resume→fire→delete cycle ----------------
    // The last `loop/updated` is a `deleted: true` tombstone, so the row is
    // gone — the store must NOT still hold it (the live gate's lesson).
    assert_eq!(
        count_method(&decoded, "loop/updated"),
        store.seen_count("loop/updated"),
        "every recorded loop/updated reached a handler"
    );
    assert!(store.seen_count("loop/updated") >= 4);
    assert!(store.seen_count("loop/fired") >= 1, "loop/fired was recorded");
    assert_eq!(
        store.domains.autonomy.loop_count(),
        0,
        "the loop was deleted by the trailing tombstone"
    );

    // ---- monitors: same cycle, also tombstoned ---------------------------
    assert!(store.seen_count("monitor/updated") >= 4);
    assert_eq!(
        store.domains.autonomy.monitor_count(),
        0,
        "the monitor was deleted by the trailing tombstone"
    );

    // ---- session goal: set (gen 5) then clear (gen 6) --------------------
    assert_eq!(store.seen_count("session/goal/updated"), 1);
    assert_eq!(store.seen_count("session/goal/cleared"), 1);
    let goal_updated = body_of(&decoded, "session/goal/updated").expect("goal/updated body");
    let cleared = body_of(&decoded, "session/goal/cleared").expect("goal/cleared body");
    let session = goal_updated["session_id"].as_str().expect("session id");
    assert_eq!(cleared["session_id"].as_str(), Some(session));
    // The clear is the later generation and wins; the goal is gone but the
    // generation advanced (the #1959 guard).
    assert!(store.domains.autonomy.goal(session).is_none());
    assert_eq!(
        store.domains.autonomy.goal_generation(session),
        cleared["generation"].as_u64().expect("clear generation")
    );
    assert!(
        cleared["generation"].as_u64().unwrap() > goal_updated["generation"].as_u64().unwrap(),
        "the recording ordered set before clear"
    );
}

#[test]
fn c24_agent_and_task_notifications_replay_into_the_store() {
    let (_, store, decoded, _) = replayed_notifications();

    // ---- agents: three real `agent/updated`, last state `failed` ---------
    assert_eq!(count_method(&decoded, "agent/updated"), 3);
    assert_eq!(store.seen_count("agent/updated"), 3);
    assert_eq!(
        store.domains.autonomy.agent_count(),
        1,
        "the three updates name one agent id (upsert, not append)"
    );
    let last = decoded
        .iter()
        .filter(|(m, _)| m == "agent/updated")
        .last()
        .map(|(_, b)| b)
        .expect("an agent/updated body");
    let agent_id = last["agent"]["agent_id"].as_str().expect("agent id");
    let row = store.domains.autonomy.agent(agent_id).expect("agent row");
    assert_eq!(row.status, "failed", "the last real update is the failure");

    // ---- tasks: two `task/updated` + one `task/output/delta` -------------
    assert_eq!(count_method(&decoded, "task/updated"), 2);
    assert_eq!(store.seen_count("task/updated"), 2);
    assert!(
        store.domains.task.snapshot_count() >= 1,
        "the real task/updated produced a snapshot row"
    );
    let delta = body_of(&decoded, "task/output/delta").expect("task/output/delta body");
    let task_id = delta["task_id"].as_str().expect("task id");
    let text = store.domains.task.output(task_id);
    assert!(
        text.contains("fixture output line one"),
        "the real task/output/delta text must accumulate; got {text:?}"
    );
    assert_eq!(store.seen_count("task/output/delta"), 1);
}

#[test]
fn c24_real_results_decode_as_their_method_types() {
    let frames = load_fixture();
    // Each recorded request result must decode into its `Method::Result`: this
    // pins the captured real shapes (they are the contract the store consumes).
    let result_of = |method: &str| -> Option<Value> {
        frames
            .iter()
            .find(|f| f.dir == "in" && f.method == method && f.error.is_none() && !f.body.is_null())
            .map(|f| f.body.clone())
    };

    macro_rules! decodes {
        ($ty:ty, $method:expr) => {
            let body = result_of($method)
                .unwrap_or_else(|| panic!("fixture carries a {method} result", method = $method));
            serde_json::from_value::<<$ty as octoscode_client::Method>::Result>(body)
                .unwrap_or_else(|e| panic!("{} result must decode: {e}", $method));
        };
    }

    decodes!(au::AgentList, "agent/list");
    decodes!(au::LoopList, "loop/list");
    decodes!(au::MonitorList, "monitor/list");
    decodes!(au::GoalGet, "session/goal/get");
    decodes!(octoscode_client::domains::task::TaskList, "task/list");
    decodes!(octoscode_client::domains::peer::PeerGather, "peer/gather");
}

#[test]
fn c24_peer_frames_are_recorded_and_the_staging_limit_is_documented() {
    let frames = load_fixture();
    let out = |m: &str| frames.iter().find(|f| f.dir == "out" && f.method == m);
    assert!(out("peer/prepare").is_some(), "peer/prepare was recorded");
    assert!(out("peer/gather").is_some(), "peer/gather was recorded");
    // A real `peer/prepare` WROTE a durable brief under the throwaway profile
    // (the serve really staged the peer directory). `peer/staged` is emitted
    // only when the peer's turn goes through the supervisor; this solo
    // recording has none, so the report says so rather than fabricating one.
    let prepared = frames
        .iter()
        .find(|f| f.dir == "in" && f.method == "peer/prepare")
        .expect("peer/prepare replied");
    assert!(
        prepared.body["brief_path"].is_string(),
        "a real peer/prepare returns the durable brief path: {}",
        prepared.body
    );
}
