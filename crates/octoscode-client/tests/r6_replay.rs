//! R6 — real-traffic replay for the `peer` + `media` domains (supervisor 8.10).
//!
//! Card #13 added `OCTOSCODE_TRACE_FILE` (a JSONL frame recorder at the client
//! boundary, `octoscode_client::trace`). Card R6 requires every protocol domain
//! to replay **frames recorded from a real `octos serve` (a6ea8505)**, not only
//! hand-written fakes, so a fixture is committed at
//! `crates/octoscode-client/tests/fixtures/r6-peer-a6ea8505.jsonl` and replayed
//! here through the REAL registry into the store.
//!
//! ## Two tests
//! - `capture_r6_peer_media_traffic` is `#[ignore]`d: it drives MY local
//!   `octos serve` on port **50160** (LESSONS "Domain fan-out") and rewrites the
//!   fixture. Run it deliberately to re-record:
//!   `OCTOS_BASE_URL=http://127.0.0.1:50160 OCTOS_BEARER=… OCTOS_PROFILE_ID=dsflash \
//!    ctest -p octoscode-client --test r6_replay -- --ignored --nocapture`
//! - `replay_*` are the committed regression tests: they read the fixture and
//!   assert the recorded traffic decodes and drives the store.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use octos_app_transport::{
    ws, Capabilities, OutboundCommand, ProfileId, SecretString, TransportConfig, TransportEvent,
};
use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::SessionOpenParams;
use serde::Deserialize;
use url::Url;

use octoscode_client::domains::peer::{PeerGather, PeerGatherParams, PeerPrepare, PeerPrepareParams};
use octoscode_client::trace::FrameTrace;
use octoscode_client::{Client, Method, Registry};
use octoscode_store::Store;

/// The committed real-traffic fixture.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/r6-peer-a6ea8505.jsonl"
);

/// Every request method this lane's domain files own.
const MY_METHODS: &[&str] = &["peer/prepare", "peer/gather", "peer/dispatch", "peer/control"];

// =====================================================================
// capture (ignored) — records the fixture against MY local serve
// =====================================================================

fn base_url() -> String {
    std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50160".into())
}
fn bearer() -> String {
    std::env::var("OCTOS_BEARER").unwrap_or_else(|_| "r6-dummy-token".into())
}
fn profile() -> String {
    std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "dsflash".into())
}

/// Record one inbound transport event, mirroring the module's `trace_method` /
/// `trace_params` (private to `octoscode-module`), on the public
/// `octoscode_client::trace::wire_params`.
fn record_inbound(trace: &FrameTrace, evt: &TransportEvent) {
    use octoscode_client::trace::wire_params;
    match evt {
        TransportEvent::DurableNotification { payload, .. } => {
            trace.inbound(payload.method(), &wire_params(payload))
        }
        TransportEvent::EphemeralNotification { payload } => {
            trace.inbound(payload.method(), &wire_params(payload))
        }
        TransportEvent::ConnectionState(s) => {
            trace.inbound(&format!("state:{s:?}"), &serde_json::json!({"state": format!("{s:?}")}))
        }
        TransportEvent::CapabilityNegotiated(caps) => trace.inbound(
            "capabilities",
            &serde_json::json!({"accepted": caps.raw.keys().cloned().collect::<Vec<_>>()}),
        ),
        TransportEvent::RpcResult(r) => {
            trace.inbound("rpc/result", &serde_json::json!({"debug": format!("{r:?}")}))
        }
        TransportEvent::RpcError { method, error, .. } => trace.inbound(
            &format!("error:{method}"),
            &serde_json::json!({"code": error.code, "message": error.message}),
        ),
        TransportEvent::SessionsListed { sessions } => trace.inbound("session/list", sessions),
        TransportEvent::SessionHydrated { session_id, result } => trace.inbound(
            "session/hydrate",
            &serde_json::json!({"session_id": session_id, "result": result}),
        ),
    }
}

/// `session/open` through the generic path (the transport needs one to go Live).
struct SessionOpen;
#[derive(Deserialize)]
struct SessionOpenResult {
    opened: Opened,
}
#[derive(Deserialize)]
struct Opened {
    session_id: String,
}
impl Method for SessionOpen {
    const NAME: &'static str = "session/open";
    type Params = SessionOpenParams;
    type Result = SessionOpenResult;
}

#[tokio::test]
#[ignore = "records the r6 fixture against MY octos serve on 50160; run with --ignored"]
async fn capture_r6_peer_media_traffic() {
    let _ = std::fs::remove_file(FIXTURE);
    let trace = FrameTrace::open(FIXTURE);
    assert!(trace.is_enabled(), "the trace file must open");

    let cfg = TransportConfig {
        base_url: Url::parse(&base_url()).expect("OCTOS_BASE_URL"),
        bearer: SecretString::new(bearer()),
        profile_id: ProfileId::new(profile()),
        cursor: None,
        cursor_file: None,
        requested_capabilities: Capabilities::requested(),
        workspace_cwd: None,
        local_kernel: false,
    };
    let (cmd_tx, mut evt_rx) = ws::spawn(cfg);
    let client = Client::with_trace(cmd_tx, trace.clone());

    // Drain + record inbound frames (the handshake and any server notifications).
    let inbound = trace.clone();
    let drain = tokio::spawn(async move {
        while let Some(evt) = evt_rx.recv().await {
            record_inbound(&inbound, &evt);
        }
    });

    // 1. session/open — the transport's handshake (out frame is traced for us).
    let profile_id = profile();
    let opened = client
        .call::<SessionOpen>(SessionOpenParams {
            session_id: octos_core::SessionKey::new(&profile_id, "main"),
            topic: None,
            profile_id: Some(profile_id.clone()),
            cwd: None,
            sandbox: None,
            after: None,
            client_commands: None,
        })
        .await
        .expect("session/open");
    let session_id = opened.opened.session_id.clone();
    println!("[capture] session/open -> {session_id}");
    tokio::time::sleep(Duration::from_millis(300)).await;

    // 2. peer/gather (READ-ONLY) — record the real result frame by hand: the
    //    generic reply rides the transport's oneshot, so the transport itself
    //    never surfaces it as an event. `FrameTrace::result` records it as an
    //    inbound result frame, exactly as a replay needs.
    let gather_params = PeerGatherParams {
        session_id: session_id.clone(),
        profile_id: profile_id.clone(),
        slugs: None,
    };
    let gathered = client
        .call::<PeerGather>(gather_params.clone())
        .await
        .expect("peer/gather");
    trace.result("peer/gather", None, &serde_json::to_value(&gathered).unwrap());
    println!("[capture] peer/gather -> {} peers", gathered.peers.len());

    // 3. peer/prepare (MUTATING — throwaway tmp only): worktree=false, cwd under
    //    my tmp dir, so the only writes are a brief inside the serve's own
    //    --data-dir (also my tmp). No model turn.
    let cwd = std::env::var("OCTOS_TMP_CWD")
        .unwrap_or_else(|_| "tmp/r6-ws".into());
    let prepared = client
        .call::<PeerPrepare>(PeerPrepareParams {
            brief: "R6 recording smoke: throwaway peer, no worktree".into(),
            n: None,
            title: Some("r6 smoke".into()),
            names: None,
            worktree: Some(false),
            cwd: Some(cwd),
            session_id: session_id.clone(),
            profile_id: profile_id.clone(),
        })
        .await
        .expect("peer/prepare");
    trace.result("peer/prepare", None, &serde_json::to_value(&prepared).unwrap());
    println!("[capture] peer/prepare -> slug={}", prepared.slug);

    // 4. Re-gather so the fixture shows the blackboard edge (empty -> 1 row).
    let after = client
        .call::<PeerGather>(gather_params)
        .await
        .expect("peer/gather (after)");
    trace.result("peer/gather", None, &serde_json::to_value(&after).unwrap());
    println!("[capture] peer/gather (after) -> {} peers", after.peers.len());

    // 5. `content/list` — a real frame from my MEDIA domain's request surface
    //    (the server advertises it; the empty gallery is the fresh-serve shape).
    //    Raw request: `content/list` is listed in media.rs but declared a stub,
    //    so it has no `Method`; a raw frame is exactly what this domain needs to
    //    replay. Read-only.
    if let Ok(value) = client.request("content/list", serde_json::json!({})).await {
        trace.result("content/list", None, &value);
        println!("[capture] content/list -> recorded");
    }

    tokio::time::sleep(Duration::from_millis(300)).await;
    drain.abort();
    drop(client);
    println!("[capture] fixture written to {FIXTURE}");
}

// =====================================================================
// replay — the committed regression tests (read the fixture)
// =====================================================================

/// One line of the fixture, in the shape `FrameTrace` writes.
#[derive(Debug, Clone, Deserialize)]
struct RecordedFrame {
    dir: String,
    method: String,
    #[serde(default)]
    body: serde_json::Value,
}

fn fixture_frames() -> Vec<RecordedFrame> {
    let text = std::fs::read_to_string(FIXTURE).expect("read the r6 fixture");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("fixture line is a recorded frame"))
        .collect()
}

/// The last recorded inbound result for `method` (the server's own bytes).
fn last_inbound_body(frames: &[RecordedFrame], method: &str) -> serde_json::Value {
    frames
        .iter()
        .filter(|f| f.dir == "in" && f.method == method)
        .last()
        .map(|f| f.body.clone())
        .unwrap_or_else(|| panic!("the fixture carries an inbound {method}"))
}

#[test]
fn replay_fixture_carries_no_credentials() {
    let text = std::fs::read_to_string(FIXTURE).expect("read the r6 fixture");
    let lowered = text.to_ascii_lowercase();
    assert!(!lowered.contains("bearer "), "no bearer header may be recorded");
    assert!(!lowered.contains("sk-"), "no provider key may be recorded");
    // `token_estimate` / `token_budget` are ordinary data (the recorder's own
    // `is_secret_key` allowlist); what must never appear is an actual secret
    // field. Assert no credential KEY is present unless redacted.
    for key in ["\"token\":", "\"api_key\":", "\"authorization\":", "\"password\":", "\"secret\":"] {
        if let Some(at) = lowered.find(key) {
            let after = &lowered[at + key.len()..];
            assert!(
                after.trim_start().starts_with("\"<redacted>\""),
                "credential key {key} must be redacted"
            );
        }
    }
    // And the recorder's redaction marker works when a secret key IS present.
    let redacted = octoscode_client::trace::redact(serde_json::json!({"token": "abc123"}));
    assert_eq!(redacted["token"], "<redacted>");
}

#[test]
fn replay_peer_gather_result_decodes_via_the_production_type() {
    let frames = fixture_frames();
    let body = last_inbound_body(&frames, "peer/gather");
    let parsed: octoscode_client::domains::peer::PeerGatherResult =
        serde_json::from_value(body).expect("the real peer/gather result decodes");
    assert_eq!(parsed.profile_id, "dsflash");
    // The recording captured the blackboard EDGE: empty at first, one row after
    // `peer/prepare` staged a peer. The last frame is the post-prepare one.
    assert!(
        parsed.peers.iter().any(|row| row.slug == "r6-smoke"),
        "the post-prepare gather lists the staged peer: {:?}",
        parsed.peers.iter().map(|r| &r.slug).collect::<Vec<_>>()
    );
    let staged = parsed.peers.iter().find(|r| r.slug == "r6-smoke").unwrap();
    assert_eq!(staged.topic, "peer-r6-smoke");
    assert!(!staged.closed);
    assert!(!staged.has_worktree, "recorded with worktree:false");
}

#[test]
fn replay_peer_prepare_result_decodes_via_the_production_type() {
    let frames = fixture_frames();
    let body = last_inbound_body(&frames, "peer/prepare");
    let parsed: octoscode_client::domains::peer::PeerPrepareResult =
        serde_json::from_value(body).expect("the real peer/prepare result decodes");
    assert_eq!(parsed.slug, "r6-smoke");
    assert_eq!(parsed.topic, "peer-r6-smoke");
    assert_eq!(parsed.profile_id, "dsflash");
    assert!(parsed.brief_path.ends_with("/peers/r6-smoke/brief.md"), "{}", parsed.brief_path);
    assert_eq!(parsed.peers.len(), 1, "the fleet mirrors the single member");
}

#[test]
fn replay_recorded_frames_route_through_the_real_registry() {
    // Every recorded inbound frame is offered to the REAL `register_all`
    // registry. The recorded server frames in this domain are the session/open
    // result + connection states (owned by the session/turn domains), so my
    // peer/media domain must claim NONE of them — and, crucially, must not leak
    // any inbound method into the tolerated-unknown arm.
    let store = Arc::new(Store::new());
    let mut reg = Registry::new();
    octoscode_client::domains::register_all(&mut reg, store.clone());

    let mut seen = Vec::new();
    for frame in fixture_frames().iter().filter(|f| f.dir == "in") {
        // Connection-state pseudo-frames and generic result frames are not
        // `UiNotification`s; skip them (the module's `trace_method` names them
        // `state:*` / `rpc/result`, which no registry handles).
        if frame.method.starts_with("state:") || frame.method == "rpc/result" {
            continue;
        }
        if let Ok(n) = UiNotification::from_method_and_params(&frame.method, frame.body.clone()) {
            let claimed = reg.dispatch(&n);
            seen.push((frame.method.clone(), claimed));
        }
    }
    let mine = [
        "peer/staged",
        "peer/closed",
        "visual/generating",
        "visual/succeeded",
        "visual/failed",
        "voice/audio_chunk",
        "voice/exit",
        "file/attached",
    ];
    assert_eq!(MY_METHODS.len(), 4, "the four peer request methods");
    let leaked: Vec<_> = reg
        .unknown_methods()
        .iter()
        .filter(|m| mine.contains(&m.as_str()))
        .cloned()
        .collect();
    assert!(leaked.is_empty(), "my domain must claim its own frames: {leaked:?}");
    println!("[replay] dispatched {} recorded inbound frames: {seen:?}", seen.len());
}

/// The peer LIFECYCLE notifications (`peer/staged`, `peer/closed`) are emitted
/// by the model's `peer_handoff` / `peer_close` tools during a turn
/// (`crates/octos-cli/src/peers/mod.rs:3731`), so a no-model recording cannot
/// capture them (R6 report step 4). Their params are byte-shape-identical to
/// the fields the RECORDED `peer/prepare` / `peer/gather` results already
/// carry (`slug`, `topic`, `brief_path`, `cwd`, `profile_id`), so this test
/// replays them through the same real registry into the store to prove the
/// store's state machine, marked here as synthesized (not live).
#[test]
fn replay_peer_lifecycle_notifications_drive_the_store() {
    let frames = fixture_frames();
    let prepared: octoscode_client::domains::peer::PeerPrepareResult =
        serde_json::from_value(last_inbound_body(&frames, "peer/prepare")).unwrap();
    let origin = last_inbound_body(&frames, "peer/gather")["session_id"]
        .as_str()
        .unwrap_or("dsflash:main")
        .to_owned();

    let store = Arc::new(Store::new());
    let mut reg = Registry::new();
    octoscode_client::domains::register_all(&mut reg, store.clone());

    let staged = UiNotification::from_method_and_params(
        "peer/staged",
        serde_json::json!({
            "session_id": origin,
            "topic": prepared.topic,
            "slug": prepared.slug,
            "brief": "R6 recording smoke: throwaway peer, no worktree",
            "brief_path": prepared.brief_path,
            "cwd": prepared.cwd,
            "profile_id": prepared.profile_id,
        }),
    )
    .expect("peer/staged decodes");
    assert!(reg.dispatch(&staged), "the registry claims peer/staged");
    let row = store
        .domains
        .peer
        .get("r6-smoke")
        .expect("the staged peer is on the roster");
    assert_eq!(row.topic.as_deref(), Some("peer-r6-smoke"));
    assert_eq!(store.domains.peer.open_count(), 1);

    let closed = UiNotification::from_method_and_params(
        "peer/closed",
        serde_json::json!({
            "session_id": origin,
            "topic": prepared.topic,
            "slug": prepared.slug,
            "profile_id": prepared.profile_id,
        }),
    )
    .expect("peer/closed decodes");
    assert!(reg.dispatch(&closed), "the registry claims peer/closed");
    assert_eq!(store.domains.peer.open_count(), 0);
}
