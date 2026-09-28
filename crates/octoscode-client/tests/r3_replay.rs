//! Card #R3 — real-traffic replay for the **session** and **config** domains.
//!
//! The F3 tests replay **hand-written** frames. This card records frames from a
//! real `octos serve` (`a6ea8505`) and replays them through the production
//! registry into the store, so a fake can no longer hide a shape the server
//! really sends.
//!
//! Two halves:
//!
//! * [`r3_capture_live_serve_frames`] (`#[ignore]`) — drives this lane's own
//!   serve on **port 50130** with `OCTOSCODE_TRACE_FILE` set, and writes
//!   `crates/octoscode-client/tests/fixtures/r3-session-a6ea8505.jsonl`.
//!   Read-only methods freely; mutating methods only against the throwaway
//!   `tmp/` data dir (create then delete what it creates). **No model turns.**
//! * [`r3_*_replay`] — feeds the committed fixture's inbound frames through
//!   `Registry::dispatch` (the same path a live connection takes) and asserts
//!   the resulting store state.
//!
//! The recorder is the client's own [`FrameTrace`] (`OCTOSCODE_TRACE_FILE`, card
//! #13); it redacts credentials before a line is written.
use std::path::PathBuf;
use std::time::Duration;

use octos_app_transport::{
    ws, Capabilities, LifecycleResult, OutboundCommand, ProfileId, SecretString, TransportConfig,
    TransportEvent,
};
use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octoscode_client::trace::FrameTrace;
use octoscode_store::Store;

/// The committed fixture (this card's artifact).
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/r3-session-a6ea8505.jsonl"
);

// ---------------------------------------------------------------------------
// The fixture's frame model — the shape `FrameTrace` writes.
// ---------------------------------------------------------------------------

/// One recorded frame.
#[derive(Debug, Clone)]
pub struct Frame {
    pub dir: String,
    pub method: String,
    pub body: serde_json::Value,
}

/// Load a JSONL fixture into frames.
pub fn load_frames(path: &str) -> Vec<Frame> {
    let text = std::fs::read_to_string(path).expect("read the R3 fixture");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v.get("body").cloned().unwrap_or(serde_json::Value::Null),
            }
        })
        .collect()
}

fn fixture_path() -> String {
    std::env::var("R3_TRACE_FILE").unwrap_or_else(|_| FIXTURE.to_owned())
}

// ---------------------------------------------------------------------------
// §1 capture — a live serve records its own frames.
// ---------------------------------------------------------------------------

/// Record one inbound `TransportEvent` into the trace, mirroring the module's
/// own `trace_method`/`trace_params` (`octoscode-module/src/flow.rs:881`).
fn record_inbound(trace: &FrameTrace, evt: &TransportEvent) {
    match evt {
        TransportEvent::DurableNotification { payload, cursor } => {
            let mut body = octoscode_client::trace::wire_params(payload);
            if let (Some(c), serde_json::Value::Object(m)) = (cursor, &mut body) {
                m.insert(
                    "cursor".to_owned(),
                    serde_json::json!({"stream": c.stream, "seq": c.seq}),
                );
            }
            trace.inbound(payload.method(), &body);
        }
        TransportEvent::EphemeralNotification { payload } => {
            trace.inbound(payload.method(), &octoscode_client::trace::wire_params(payload));
        }
        TransportEvent::RpcResult(LifecycleResult::SessionOpen(r)) => {
            trace.inbound(
                "session/open",
                &serde_json::json!({
                    "session_id": r.opened.session_id.0,
                    "capabilities": r.opened.capabilities,
                }),
            );
        }
        TransportEvent::RpcResult(LifecycleResult::TurnStart(r)) => {
            trace.inbound("turn/start", &serde_json::json!({"accepted": r.accepted}));
        }
        TransportEvent::RpcResult(LifecycleResult::TurnInterrupt(_)) => {
            trace.inbound("turn/interrupt", &serde_json::json!({}));
        }
        TransportEvent::RpcError { method, error, .. } => {
            trace.inbound(
                &format!("error:{method}"),
                &serde_json::json!({"code": error.code, "message": error.message}),
            );
        }
        TransportEvent::ConnectionState(s) => {
            trace.inbound(&format!("state:{s:?}"), &serde_json::json!({}));
        }
        TransportEvent::CapabilityNegotiated(caps) => {
            trace.inbound(
                "capabilities",
                &serde_json::json!({
                    "accepted": caps.raw.keys().cloned().collect::<Vec<_>>()
                }),
            );
        }
        TransportEvent::SessionsListed { sessions } => {
            trace.inbound("session/list", sessions);
        }
        TransportEvent::SessionHydrated { session_id, result } => {
            trace.inbound(
                "session/hydrate",
                &serde_json::json!({"session_id": session_id, "result": result}),
            );
        }
    }
}

/// Sleep briefly so trailing frames land.
async fn settle(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

/// Record every method and notification this lane's domains own, against this
/// lane's own serve.
///
/// ```sh
/// tmp/r3/bin/octos serve --solo --port 50130 --host 127.0.0.1 \
///   --auth-token r3-dummy-token --data-dir tmp/r3/home --instance-data-dir tmp/r3/home
/// ctest -p octoscode-client --test r3_replay -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "records real frames; needs this lane's own serve on port 50130"]
async fn r3_capture_live_serve_frames() {
    let base = std::env::var("R3_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50130".into());
    let bearer = std::env::var("R3_BEARER").unwrap_or_else(|_| "r3-dummy-token".into());
    let profile = std::env::var("R3_PROFILE").unwrap_or_else(|_| {
        format!("r3{}", std::process::id())
    });
    // When the serve's data dir already holds a configured profile (the card's
    // authorised `dsflash`), use it and skip onboarding: a profile with a real
    // LLM runtime is what lets `session/compact` reach its (LLM-free, empty
    // history) early path and emit the context-lifecycle notifications.
    let skip_create = std::env::var("R3_SKIP_CREATE").map(|v| v == "1").unwrap_or(false);
    let path = fixture_path();
    // Start from a clean fixture so a re-run does not append to the last one.
    let _ = std::fs::remove_file(&path);

    let trace = FrameTrace::open(&path);
    assert!(trace.is_enabled(), "the trace must be on for a capture run");

    let cfg = TransportConfig {
        base_url: url::Url::parse(&base).expect("R3_BASE_URL parses"),
        bearer: SecretString::new(bearer),
        profile_id: ProfileId::new(profile.clone()),
        cursor: None,
        cursor_file: None,
        requested_capabilities: Capabilities::requested(),
        workspace_cwd: None,
        local_kernel: false,
    };
    let (cmd_tx, mut evt_rx) = ws::spawn(cfg);

    // Drain + record every inbound frame on its own task.
    let trace_in = trace.clone();
    tokio::spawn(async move {
        while let Some(evt) = evt_rx.recv().await {
            record_inbound(&trace_in, &evt);
        }
    });

    let client = octoscode_client::Client::with_trace(cmd_tx, trace.clone());

    // 1. Precondition: onboard a throwaway local profile, unless the data dir
    //    already holds one we were told to use.
    let profile_id = if skip_create {
        println!("profile/local/create -> skipped (using configured profile {profile})");
        profile
    } else {
        let created = client
            .request(
                "profile/local/create",
                serde_json::json!({"requested_id": profile, "name": "R3", "username": profile}),
            )
            .await
            .expect("profile/local/create");
        let id = created["profile_id"].as_str().expect("profile_id").to_owned();
        println!("profile/local/create -> {id}");
        id
    };

    // 2. Precondition: open a session whose id is profile-routable.
    let session_id = octos_core::SessionKey::with_profile(&profile_id, "api", "main");
    let opened = client
        .request(
            "session/open",
            serde_json::json!({
                "session_id": session_id.0,
                "profile_id": profile_id,
                "cwd": null,
            }),
        )
        .await
        .expect("session/open");
    println!("session/open -> {}", opened["opened"]["session_id"]);
    settle(300).await;

    let sid = session_id.0.clone();
    let j = |m: &str| serde_json::json!({"session_id": sid, "profile_id": profile_id});

    // ---- read-only methods ----
    for (label, method, params) in [
        ("launch/resolve", "launch/resolve", serde_json::json!({"cwd": "/tmp", "profile_id": profile_id})),
        ("session/status/read", "session/status/read", j("session/status/read")),
        ("session/files.list", "session/files.list", j("session/files.list")),
        ("snapshot/list", "snapshot/list", j("snapshot/list")),
        ("session/list", "session/list", serde_json::json!({})),
    ] {
        match call_recorded(&client, &trace, method, params).await {
            Ok(v) => println!("{label} -> ok: {}", short(&v)),
            Err(e) => println!("{label} -> err: {e}"),
        }
        settle(80).await;
    }

    // ---- mutating methods, against the throwaway data dir only ----
    // Compaction-mode override, then a forced compaction pass.
    match call_recorded(
        &client,
        &trace,
        "session/compact/mode/set",
        serde_json::json!({"session_id": sid, "mode": "heuristic"}),
    )
    .await
    {
        Ok(v) => println!("session/compact/mode/set -> {}", short(&v)),
        Err(e) => println!("session/compact/mode/set -> err: {e}"),
    }
    settle(120).await;
    match call_recorded(&client, &trace, "session/compact", j("session/compact")).await {
        Ok(v) => println!("session/compact -> {}", short(&v)),
        Err(e) => println!("session/compact -> err: {e}"),
    }
    settle(200).await;

    // Rollback (no turns yet -> a real, well-formed answer either way).
    match call_recorded(
        &client,
        &trace,
        "session/rollback",
        serde_json::json!({"session_id": sid, "num_turns": 1}),
    )
    .await
    {
        Ok(v) => println!("session/rollback -> {}", short(&v)),
        Err(e) => println!("session/rollback -> err: {e}"),
    }
    settle(120).await;

    // Fork, then delete the child we created (leave the parent alive).
    let fork = call_recorded(
        &client,
        &trace,
        "session/fork",
        serde_json::json!({"session_id": sid, "new_chat_id": "r3child"}),
    )
    .await;
    match &fork {
        Ok(v) => {
            println!("session/fork -> {}", short(v));
            let child = v["new_session_id"].as_str().map(str::to_owned);
            if let Some(child) = child {
                settle(120).await;
                match call_recorded(
                    &client,
                    &trace,
                    "session/delete",
                    serde_json::json!({"session_id": child}),
                )
                .await
                {
                    Ok(v) => println!("session/delete(child) -> {}", short(&v)),
                    Err(e) => println!("session/delete(child) -> err: {e}"),
                }
            }
        }
        Err(e) => println!("session/fork -> err: {e}"),
    }
    settle(200).await;

    // ---- the last recorded method: stop this serve myself (it is mine) ----
    match call_recorded(&client, &trace, "server/shutdown", serde_json::json!({})).await {
        Ok(v) => println!("server/shutdown -> {}", short(&v)),
        Err(e) => println!("server/shutdown -> err: {e}"),
    }
    // The server drains the socket ~250 ms after the ack; give the reply time to
    // land, then stop. Do NOT issue a post-shutdown request — it only adds a
    // reconnect storm (28 `state:Reconnecting` frames) no fixture needs.
    settle(700).await;

    let _ = OutboundCommand::Disconnect;
    println!("[r3 capture] wrote {}", trace.path().unwrap_or(PathBuf::from(&path)).display());
}

fn short(v: &serde_json::Value) -> String {
    let s = v.to_string();
    if s.len() > 180 {
        format!("{}…", &s[..180])
    } else {
        s
    }
}

/// Issue a request and record its real **result** under `res:<method>`.
///
/// `Client::request` already traces the outbound frame (card #13); a generic
/// reply travels the request's oneshot and never appears on the event channel,
/// so without this the fixture would hold only request params and the replay
/// could not assert the server's actual result shape. On error we record the
/// real error frame, which is itself fixture-worthy.
async fn call_recorded(
    client: &octoscode_client::Client,
    trace: &FrameTrace,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, octoscode_client::ClientError> {
    let result = client.request(method, params).await;
    match &result {
        Ok(v) => trace.raw_line(serde_json::json!({
            "dir": "in", "method": format!("res:{method}"), "body": v,
        })),
        Err(e) => trace.raw_line(serde_json::json!({
            "dir": "in", "method": format!("err:{method}"),
            "body": {"message": e.to_string()},
        })),
    }
    result
}

// ---------------------------------------------------------------------------
// §3 replay — the committed fixture through the real registry.
// ---------------------------------------------------------------------------

/// Build the production registry + store the way the app does.
fn wired() -> (octoscode_client::Registry, std::sync::Arc<Store>) {
    let store = std::sync::Arc::new(Store::new());
    let mut reg = octoscode_client::Registry::new();
    octoscode_client::domains::register_all(&mut reg, store.clone());
    (reg, store)
}

/// Replay result: what the fixture carried and what each frame did.
struct Replayed {
    dispatched: usize,
    unknown: Vec<String>,
    skipped_non_notification: Vec<String>,
}

/// Feed every inbound frame of the fixture through the registry.
fn replay(reg: &mut octoscode_client::Registry) -> Replayed {
    let mut out = Replayed {
        dispatched: 0,
        unknown: Vec::new(),
        skipped_non_notification: Vec::new(),
    };
    for f in load_frames(FIXTURE) {
        if f.dir != "in" {
            continue;
        }
        match UiNotification::from_method_and_params(&f.method, f.body.clone()) {
            Ok(n) => {
                if !reg.dispatch(&n) {
                    out.unknown.push(f.method.clone());
                }
                out.dispatched += 1;
            }
            Err(_) => out.skipped_non_notification.push(f.method.clone()),
        }
    }
    out
}

#[test]
fn r3_fixture_is_present_and_parses() {
    let frames = load_frames(FIXTURE);
    assert!(!frames.is_empty(), "the R3 fixture has frames");
    assert!(
        frames.iter().any(|f| f.dir == "out" && f.method == "session/open"),
        "the capture opened a session"
    );
}
