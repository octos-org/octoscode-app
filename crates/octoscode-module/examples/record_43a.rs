//! #43a — record the REAL frames the fixture-limited walk rows need (the
//! live gate, the `dsflash` profile, ≤ 80 runner-counted turns; this run
//! spends at most 3).
//!
//! Mirrors `record_autonomy.rs`: drives the production [`Conversation`]
//! (registry + store + trace) against the RUNNING live gate
//! (`127.0.0.1:50190`), into `OCTOSCODE_TRACE_FILE`, then scrubs machine
//! paths so the committed fixture is hermetic. The bearer token comes from
//! the env (`OCTOS_BEARER`, set by the runner from the gate's token file) and
//! is NEVER printed or written to the trace (the transport redacts it; the
//! leak scan greps the committed file anyway).
//!
//! ```sh
//! OCTOS_BEARER="$(cat "$OCTOS_LIVE_TOKEN_FILE")" \   # the gate's token file (never committed)
//! OCTOS_PROFILE_ID=dsflash OCTOS_43A_WORKSPACE="$PWD/tmp/43a-ws" \
//! OCTOSCODE_TRACE_FILE="$PWD/crates/octoscode-client/tests/fixtures/r43a-recovery-a6ea8505.jsonl" \
//! cargo run -p octoscode-module --example record_43a
//! ```
//!
//! Frames this captures (the #41c fixture rows' unblockers):
//! * one REAL turn — `turn/start`, `message/delta`, `message/reasoning_delta`
//!   (when the model emits reasoning), `turn/completed`, the surrounding
//!   `projection/envelope`s;
//! * a second connection hydrating that session — `session/hydrate` +
//!   its authoritative chat-state reply (the recovery rows' shape);
//! * an approval-provoking turn (a one-line file write) — `approval/requested`
//!   held PENDING across a mid-turn `session/hydrate`, then
//!   `approval/respond` (rows 174/187/204's pending-state shapes);
//! * optionally a peer-staging ask — `peer/staged` is agent-initiated; if the
//!   gate's model has no staging tooling the three peer/staged rows are
//!   reported can't-produce (never faked).
use std::time::Duration;

use octos_app_transport::TransportEvent;
use octoscode_module::flow::{Conversation, FlowEvent};
use tokio::sync::mpsc::Receiver;

const TMP_PLACEHOLDER: &str = "<TMP>";
const WORKSPACE_PLACEHOLDER: &str = "<WORKSPACE>";
const HOME_PLACEHOLDER: &str = "<HOME>";

/// **Recorder-only** — machine-path prefixes → placeholders (longest first).
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
            if let Some(home) = std::path::Path::new(ws).parent() {
                let home = home.to_string_lossy().to_string();
                if !home.is_empty() {
                    prefixes.push((home, HOME_PLACEHOLDER));
                }
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

/// Drain pending transport events through the conversation, recording every
/// inbound frame, until `deadline_ms` elapses or the turn settles.
async fn drain_until_turn_end(
    conv: &Conversation,
    rx: &mut Receiver<TransportEvent>,
    deadline_ms: u64,
) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(deadline_ms);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return false;
        }
        match tokio::time::timeout(deadline - now, rx.recv()).await {
            Ok(Some(evt)) => {
                if let FlowEvent::TurnEnded { .. } = conv.on_event(evt) {
                    return true;
                }
            }
            Ok(None) => return false,
            Err(_) => return false,
        }
    }
}

/// Drain for a fixed window (handshakes, hydrate replies, notifications).
async fn drain(conv: &Conversation, rx: &mut Receiver<TransportEvent>, ms: u64) {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(ms);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return;
        }
        match tokio::time::timeout(deadline - now, rx.recv()).await {
            Ok(Some(evt)) => {
                let _ = conv.on_event(evt);
            }
            Ok(None) => return,
            Err(_) => return,
        }
    }
}

async fn connect() -> (Conversation, Receiver<TransportEvent>) {
    let base = std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50190".into());
    let bearer = std::env::var("OCTOS_BEARER").expect("OCTOS_BEARER (from the gate token file)");
    let profile = std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "dsflash".into());
    let (conv, events) = Conversation::connect(&base, &bearer, &profile, None, None)
        .expect("connect to the live gate");
    (conv, events)
}

#[tokio::main]
async fn main() {
    let trace_path = std::env::var("OCTOSCODE_TRACE_FILE")
        .unwrap_or_else(|_| "crates/octoscode-client/tests/fixtures/r43a-recovery-a6ea8505.jsonl".into());

    // ---- supplement mode: record ONLY a session/hydrate reply (a read; 0
    // model turns) into the existing fixture — the first pass recorded the
    // two hydrate REQUESTS but the replies went through
    // `Client::request`'s return value, not the conversation's trace.
    if std::env::var("OCTOS_43A_HYDRATE_ONLY").as_deref() == Ok("1") {
        let scratch = trace_path.replace(".jsonl", "-supplement.jsonl");
        std::env::set_var("OCTOSCODE_TRACE_FILE", &scratch);
        let (conv, mut rx) = connect().await;
        drain(&conv, &mut rx, 1200).await;
        let session = std::env::var("OCTOS_43A_SESSION").unwrap_or_else(|_| conv.session_id().to_owned());
        println!("[43a-supplement] hydrate {session}");
        let v = conv
            .client()
            .request("session/hydrate", serde_json::json!({ "session_id": session }))
            .await
            .expect("session/hydrate reply");
        // The reply never touches the conversation's trace — append it here.
        let hydrate_trace = octoscode_client::trace::FrameTrace::open(&trace_path);
        hydrate_trace.result("session/hydrate", None, &v);
        drop(hydrate_trace);
        let raw = std::fs::read_to_string(&trace_path).expect("read the fixture");
        let scrubbed = scrub_machine_paths(&raw);
        std::fs::write(&trace_path, scrubbed).expect("write the scrubbed fixture");
        println!("[43a-supplement] appended the session/hydrate reply to {trace_path}");
        return;
    }

    let _ = std::fs::remove_file(&trace_path);
    let _ = std::fs::create_dir_all(std::path::Path::new(&trace_path).parent().unwrap());

    // ---- connection 1 + one REAL turn (deltas / completed / reasoning) ----
    let (conv, mut rx) = connect().await;
    let cwd = std::env::var("OCTOS_43A_WORKSPACE").unwrap_or_else(|_| "tmp/43a-ws".into());
    let cwd = std::fs::canonicalize(&cwd).expect("neutral workspace exists").to_string_lossy().to_string();
    conv.open_workspace(Some(cwd.clone())).await.expect("session/open");
    drain(&conv, &mut rx, 1200).await;
    let session = conv.session_id().to_owned();
    println!("[43a] session: {session}");

    println!("[43a] turn 1/80: neutral ask");
    conv.start_turn("Recording fixture. Reply with exactly: ok").await.expect("turn/start");
    let settled = drain_until_turn_end(&conv, &mut rx, 180_000).await;
    println!("[43a] turn 1 settled: {settled}");
    drain(&conv, &mut rx, 1500).await;

    // ---- connection 2 hydrates that session (the recovery shape) ----------
    let sid = session.clone();
    drop(conv);
    let (conv2, mut rx2) = connect().await;
    conv2.open_workspace(Some(cwd.clone())).await.expect("session/open 2");
    drain(&conv2, &mut rx2, 1200).await;
    println!("[43a] hydrate #1 (completed-turn session)");
    match conv2
        .client()
        .request("session/hydrate", serde_json::json!({ "session_id": sid }))
        .await
    {
        Ok(v) => println!("[43a] hydrate #1 -> ok ({} bytes)", v.to_string().len()),
        Err(e) => println!("[43a] hydrate #1 -> error {e}"),
    }
    drain(&conv2, &mut rx2, 2500).await;

    // ---- turn 2: approval-provoking write, hydrate #2 while it is PENDING -
    println!("[43a] turn 2/80: one-line file write (approval path)");
    conv2
        .start_turn("Create a file named marker43a.txt in the workspace root containing exactly: ok")
        .await
        .expect("turn/start 2");
    // Give the model a moment to request the write, then hydrate mid-pending.
    drain(&conv2, &mut rx2, 20_000).await;
    let new_session = conv2.session_id().to_owned();
    println!("[43a] hydrate #2 (mid-turn session {new_session})");
    match conv2
        .client()
        .request("session/hydrate", serde_json::json!({ "session_id": new_session }))
        .await
    {
        Ok(v) => println!("[43a] hydrate #2 -> ok ({} bytes)", v.to_string().len()),
        Err(e) => println!("[43a] hydrate #2 -> error {e}"),
    }
    drain(&conv2, &mut rx2, 2_500).await;
    // Let the turn settle (approve if the gate asked).
    let settled2 = drain_until_turn_end(&conv2, &mut rx2, 180_000).await;
    println!("[43a] turn 2 settled: {settled2}");
    drain(&conv2, &mut rx2, 1500).await;
    drop(conv2);

    // ---- turn 3 (optional): agent-initiated peer staging ------------------
    println!("[43a] turn 3/80: peer-staging attempt");
    let (conv3, mut rx3) = connect().await;
    conv3.open_workspace(Some(cwd)).await.expect("session/open 3");
    drain(&conv3, &mut rx3, 1200).await;
    conv3
        .start_turn(
            "If you have a peer staging tool, stage one peer for a trivial no-op task; otherwise reply: no peer tooling",
        )
        .await
        .expect("turn/start 3");
    let settled3 = drain_until_turn_end(&conv3, &mut rx3, 180_000).await;
    println!("[43a] turn 3 settled: {settled3}");
    drain(&conv3, &mut rx3, 1500).await;
    drop(conv3);

    // ---- scrub → hermetic fixture -----------------------------------------
    let raw = std::fs::read_to_string(&trace_path).expect("read the raw trace");
    let scrubbed = scrub_machine_paths(&raw);
    std::fs::write(&trace_path, scrubbed).expect("write the scrubbed fixture");
    let text = std::fs::read_to_string(&trace_path).expect("reread");
    let lines = text.lines().count();
    let mut methods = std::collections::BTreeMap::new();
    for l in text.lines() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(l) {
            if let Some(m) = v["method"].as_str() {
                *methods.entry(m.to_owned()).or_insert(0usize) += 1;
            }
        }
    }
    println!("[43a] fixture: {trace_path} ({lines} frames)");
    for (m, n) in &methods {
        println!("[43a]   {m}: {n}");
    }
    // Assembled so this source file stays free of the literals it bans.
    let users = format!("/{}s/", "User");
    let home_home = format!("<{}/home/", "HOME");
    for banned in [users, home_home] {
        assert!(!text.contains(&banned), "machine path {banned} survived the scrub");
    }
}
