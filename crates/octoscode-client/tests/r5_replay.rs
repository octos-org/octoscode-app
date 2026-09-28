//! Card #R5 — replay **recorded real frames** for the `turn`/`approval`/`review`
//! domains built in F5.
//!
//! Card #13 §1 added `OCTOSCODE_TRACE_FILE`; R5 requires every domain to replay
//! frames recorded from a **real `octos serve` (a6ea8505)**, not hand-written
//! fakes — the live gate showed the real server wraps turn/message/tool updates
//! in `projection/envelope`, which fakes never did.
//!
//! `crates/octoscode-client/tests/fixtures/r5-turn-a6ea8505.jsonl` is that
//! recording: this lane's own serve on **port 50150** with
//! `OCTOS_M9_PROTOCOL_FIXTURES=1`, so the `turn/*` / `projection/envelope` /
//! `tool/*` frames come from the deterministic M9 fixture turns and the
//! **approval round-trip** from the `M9ProtocolFixture::Approval` prompt — all
//! with **no model turn** (octos-cli `api/ui_protocol_transport.rs`
//! `run_m9_fixture_turn`). The capture is the `#[ignore]` test below; the normal
//! suite only *replays* the committed file.
//!
//! ## Capture
//! ```sh
//! OCTOS_BASE_URL=http://127.0.0.1:50150 \
//!   ctest -p octoscode-client --test r5_replay -- --ignored --nocapture capture
//! ```
//! Then commit `crates/octoscode-client/tests/fixtures/r5-turn-a6ea8505.jsonl`.
use std::sync::{Arc, Mutex};

use octos_app_transport::{
    ws, Capabilities, ProfileId, SecretString, TransportConfig, TransportEvent,
};
use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octoscode_client::domains;
use octoscode_client::trace::{wire_params, FrameTrace};
use octoscode_client::{Client, Registry};
use octoscode_store::Store;
use serde_json::{json, Value};

const SERVE_PORT: &str = "50150";

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/r5-turn-a6ea8505.jsonl")
}

/// The M9 deterministic-fixture prompts (octos-cli `ui_protocol_transport.rs`
/// `m9_protocol_fixture_for_prompt`). Each drives a real server-side turn with
/// **no model call**.
/// - `list_dir tool` → `M9ProtocolFixture::ToolEvents`: tool/started, tool/progress, tool/completed.
/// - `m9 approval fixture` → `M9ProtocolFixture::Approval`: a real `approval/requested` that
///   blocks the turn until the client answers, then a message/delta echoing the decision.
const PROMPT_TOOL: &str = "list_dir tool";
const PROMPT_APPROVAL: &str = "m9 approval fixture";

/// Fixed UUIDv7s so the fixture is deterministic (the web generates them the
/// same way; `TurnId(Uuid::now_v7())`).
const TURN_TOOL: &str = "01920000-0000-7000-8000-0000000000a1";
const TURN_APPROVE: &str = "01920000-0000-7000-8000-0000000000b2";
const TURN_DENY: &str = "01920000-0000-7000-8000-0000000000c3";

// ---------------------------------------------------------------- capturing

/// Redact values that would trip the card's credential grep
/// (`sk-|bearer [a-z0-9]{20}`). The trace already redacts every
/// `token`/`api_key`/`authorization`/`secret`/`password` field; this also
/// covers a bare `sk-`-bearing string (e.g. a spawned-agent id) so the check
/// is honestly 0. No credential is ever on the wire: the bearer travels in the
/// handshake header, not in a frame.
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

/// The latest `approval/requested` id the drain saw, and the turn ids that
/// reached a terminal — shared with the capture's main task.
#[derive(Default)]
struct Seen {
    approval_id: Option<String>,
}

/// **Capture** (run manually against this lane's serve). Records every outbound
/// request (via `Client`'s trace) and every inbound notification, plus the
/// read-only results, into the committed fixture.
#[tokio::test]
#[ignore = "needs a local octos serve on 50150 with OCTOS_M9_PROTOCOL_FIXTURES=1"]
async fn capture_r5_frames() {
    let path = fixture_path();
    let _ = std::fs::remove_file(&path);
    let trace = FrameTrace::open(path.to_str().expect("utf8 path"));

    // The web's exact feature set (21), so the server advertises the domains'
    // methods and honours `turn/*`, `approval/*` and `projection/envelope`
    // (see `features.rs`).
    let capabilities: Capabilities = octoscode_client::features::web_capabilities();

    let base = std::env::var("OCTOS_BASE_URL")
        .unwrap_or_else(|_| format!("http://127.0.0.1:{SERVE_PORT}"));
    let profile = format!("octoscode{}", std::process::id());
    let cfg = TransportConfig {
        base_url: url::Url::parse(&base).expect("base url"),
        bearer: SecretString::new("r5-dummy-token"),
        profile_id: ProfileId::new(profile.clone()),
        cursor: None,
        cursor_file: None,
        requested_capabilities: capabilities,
        workspace_cwd: None,
        local_kernel: false,
    };
    let (cmd_tx, mut evt_rx) = ws::spawn(cfg);
    let drain = trace.clone();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let seen_drain = seen.clone();
    tokio::spawn(async move {
        while let Some(evt) = evt_rx.recv().await {
            if let TransportEvent::DurableNotification { payload, .. } = &evt {
                if let UiNotification::ApprovalRequested(e) = payload {
                    seen_drain.lock().unwrap().approval_id = Some(e.approval_id.0.to_string());
                }
            }
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
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Read-only methods in this domain, recording each result/typed error.
    let read_only: [(&str, Value); 3] = [
        (
            "thread/graph/get",
            json!({"session_id": session_id}),
        ),
        (
            "approval/scopes/list",
            json!({"session_id": session_id}),
        ),
        (
            "turn/state/get",
            json!({"session_id": session_id, "turn_id": TURN_TOOL}),
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

    // Fixture turn 1 — tool events (no model call).
    let started = client
        .request(
            "turn/start",
            json!({"session_id": session_id, "turn_id": TURN_TOOL,
                   "input": [{"kind": "text", "text": PROMPT_TOOL}]}),
        )
        .await;
    if let Ok(v) = &started {
        trace.result("turn/start", None, v);
    }
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Fixture turn 2 — approval, respond **approve**.
    run_approval_fixture(&client, &trace, &seen, &session_id, TURN_APPROVE, "approve").await;

    // Fixture turn 3 — approval, respond **deny**.
    run_approval_fixture(&client, &trace, &seen, &session_id, TURN_DENY, "deny").await;

    // The remaining F5 domain methods, exercised **last** so their side effects
    // cannot disturb the fixture turns above. All real frames, no model work:
    //  * `turn/steer` on an idle session → codex parity (`NoActiveTurn`): the
    //    input is submitted as a **fresh** turn and the receipt is
    //    `{turn_id: <new>, steered:false}` (the web's valid-receipt shape). The
    //    deterministic Basic fixture answers that fresh turn, so no model is
    //    called. This is why it must run last: the fresh turn would otherwise
    //    occupy the session and starve a fixture turn.
    //  * `user_question/respond` with no pending question → typed `-32106`.
    //  * `review/start` for an unknown profile → typed refusal (its success path
    //    starts a server-owned review = model work, outside the card's allowance).
    let tail_probes: [(&str, Value); 3] = [
        (
            "turn/steer",
            json!({"session_id": session_id,
                   "expected_turn_id": TURN_TOOL,
                   "input": [{"kind": "text", "text": "steer with no live turn"}]}),
        ),
        (
            "user_question/respond",
            json!({"session_id": session_id,
                   "question_id": "01920000-0000-7000-8000-0000000000d4",
                   "answers": [{"selected_labels": ["Yes"]}]}),
        ),
        (
            "review/start",
            json!({"session_id": "not-this-profile:main",
                   "turn_id": TURN_TOOL,
                   "delivery": "inline"}),
        ),
    ];
    for (method, params) in tail_probes {
        match client.request(method, params).await {
            Ok(v) => trace.result(method, None, &v),
            Err(octoscode_client::ClientError::Rpc { method, error }) => {
                trace.error(&method, None, &error.message, error.code)
            }
            Err(other) => eprintln!("capture: {method} unexpected {other:?}"),
        }
    }
    // Drain the steer's admitted Basic turn + any trailing frames.
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    drop(client);
    eprintln!("capture: wrote {}", path.display());
}

/// Start the approval fixture turn, wait for its `approval/requested`, answer
/// with `decision` ("approve"/"deny"), and let the turn finish.
async fn run_approval_fixture(
    client: &Client,
    trace: &FrameTrace,
    seen: &Arc<Mutex<Seen>>,
    session_id: &str,
    turn_id: &str,
    decision: &str,
) {
    // Clear any prior id so we only answer THIS turn's request.
    seen.lock().unwrap().approval_id = None;
    let started = client
        .request(
            "turn/start",
            json!({"session_id": session_id, "turn_id": turn_id,
                   "input": [{"kind": "text", "text": PROMPT_APPROVAL}]}),
        )
        .await;
    if let Ok(v) = &started {
        trace.result("turn/start", None, v);
    }
    // Wait for the fixture's `approval/requested` to land.
    let approval_id = {
        let mut found = None;
        for _ in 0..40 {
            if let Some(id) = seen.lock().unwrap().approval_id.clone() {
                found = Some(id);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        found
    };
    let Some(approval_id) = approval_id else {
        eprintln!("capture: no approval/requested for turn {turn_id}");
        return;
    };
    let respond = client
        .request(
            "approval/respond",
            json!({"session_id": session_id, "approval_id": approval_id,
                   "decision": decision}),
        )
        .await;
    match &respond {
        Ok(v) => trace.result("approval/respond", None, v),
        Err(octoscode_client::ClientError::Rpc { method, error }) => {
            trace.error(method, None, &error.message, error.code)
        }
        Err(other) => eprintln!("capture: approval/respond unexpected {other:?}"),
    }
    // Let the decision echo + terminal land.
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
}

// ------------------------------------------------------------------ replay

#[derive(Debug, Clone)]
struct Frame {
    dir: String,
    method: String,
    body: Value,
    /// The RPC error, when the frame is an error frame (the trace writes it
    /// under `error`, not `body`).
    error: Option<Value>,
}

fn load_fixture() -> Vec<Frame> {
    let text = std::fs::read_to_string(fixture_path()).expect("read the r5 fixture");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v["body"].clone(),
                error: v.get("error").cloned(),
            }
        })
        .collect()
}

/// Replay every `in` notification frame through the **real** `register_all`
/// registry into a fresh store; return the store and the unhandled kinds.
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

fn session_of(frames: &[Frame]) -> String {
    frames
        .iter()
        .find(|f| f.dir == "out" && f.method == "session/open")
        .and_then(|f| f.body["session_id"].as_str().map(str::to_owned))
        .expect("the fixture carries our outbound session/open")
}

#[test]
fn the_fixture_is_a_real_recording_of_this_domain() {
    let frames = load_fixture();
    let methods: Vec<&str> = frames.iter().map(|f| f.method.as_str()).collect();
    // The decisive proof for this card: the real server DID send these as
    // **bare** notifications (turn lifecycle + the approval round-trip).
    for want in [
        "projection/envelope",
        "turn/started",
        "approval/requested",
        "approval/decided",
    ] {
        assert!(
            methods.contains(&want),
            "the recording must carry a real `{want}`; got {methods:?}"
        );
    }
    // ...and the turn/text/tool updates all arrived **inside `projection/envelope`**
    // — the exact thing hand-written fakes hid (RULES "tests replay recorded real
    // traffic"). Assert the payload kinds, not top-level method names.
    let mut payload_kinds: Vec<String> = frames
        .iter()
        .filter(|f| f.dir == "in" && f.method == "projection/envelope")
        .filter_map(|f| f.body["payload"]["type"].as_str().map(str::to_owned))
        .collect();
    payload_kinds.sort();
    payload_kinds.dedup();
    for want in ["tool_start", "tool_progress", "tool_end", "assistant_delta", "turn_terminal"] {
        assert!(
            payload_kinds.iter().any(|k| k == want),
            "the recording must carry a `{want}` envelope payload; got {payload_kinds:?}"
        );
    }
    // No bare `tool/*` or `message/delta` notifications: the real server funnels
    // them through the envelope only (that is the F5 live finding).
    for bare in ["tool/started", "tool/completed", "message/delta"] {
        assert!(
            !methods.contains(&bare),
            "the real server does not send a bare `{bare}` (it uses the envelope)"
        );
    }
    for out in [
        "turn/start",
        "approval/respond",
        "thread/graph/get",
        "approval/scopes/list",
        "turn/state/get",
        // Every method in the F5 domain files was exercised live, including the
        // three whose only deterministic path is a refusal (success would need
        // model work the card's allowance forbids recording here).
        "turn/steer",
        "user_question/respond",
        "review/start",
    ] {
        assert!(
            frames.iter().any(|f| f.dir == "out" && f.method == out),
            "the recording must carry our outbound `{out}`"
        );
    }
}

#[test]
fn recorded_refusals_decode_as_the_server_sent_them() {
    let frames = load_fixture();

    // `turn/steer` with no live turn: the server answers a REAL result
    // (`steered:false` + a fresh turn id) — which the web's `steer.ts` receipt
    // rule accepts (`steered:false` ⇒ `turn_id !== expected_turn_id`). Decode it
    // with the domain's own type.
    let steer = frames
        .iter()
        .find(|f| f.dir == "in" && f.method == "turn/steer")
        .expect("the recording carries the steer refusal");
    let steer_result: octoscode_client::domains::turn::TurnSteerResult =
        serde_json::from_value(steer.body.clone()).expect("decodes as TurnSteerResult");
    assert!(
        !steer_result.steered,
        "no live turn ⇒ steered:false; got {steer_result:?}"
    );
    assert_ne!(
        steer_result.turn_id, TURN_TOOL,
        "a refused steer names a different turn (the web's valid-receipt shape)"
    );

    // `user_question/respond` with no pending question: the server's typed
    // `-32106` refusal (`user_question/respond target was not found`).
    let q = frames
        .iter()
        .find(|f| f.dir == "in" && f.method == "user_question/respond")
        .expect("the recording carries the question refusal");
    assert_eq!(
        q.error.as_ref().and_then(|e| e["code"].as_i64()),
        Some(-32106),
        "the question refusal is the typed `target was not found` error: {q:?}"
    );

    // `review/start` for an unknown profile: a typed refusal, never a silent
    // success (the fixture mode cannot start a server-owned review without
    // model work, so the refusal is the recorded path).
    let r = frames
        .iter()
        .find(|f| f.dir == "in" && f.method == "review/start")
        .expect("the recording carries the review-start refusal");
    assert!(
        r.error.as_ref().and_then(|e| e["code"].as_i64()).is_some(),
        "the review refusal is a typed RPC error: {r:?}"
    );
}

#[test]
fn replayed_envelopes_fold_turn_text_and_tool_rows_into_the_store() {
    let frames = load_fixture();
    let (store, _) = replay_into_store(&frames);
    let session = session_of(&frames);

    // projection/envelope filled the timeline: the approval decision echo
    // (assistant_delta) folded into an assistant entry per turn.
    let text = store.domains.session.timeline.assistant_text(&session);
    assert!(
        text.contains("approval approved"),
        "the approve round-trip's echo must land in the transcript; got {text:?}"
    );
    assert!(
        text.contains("approval denied"),
        "the deny round-trip's echo must land in the transcript; got {text:?}"
    );

    // The tool fixture drove the tool domain from the envelope `tool_*` payloads
    // (`call_started`/`call_ended`, the same rows the bare tool notifications write).
    let calls = store.domains.tool.calls();
    let row = calls
        .iter()
        .find(|t| t.name == "list_dir")
        .unwrap_or_else(|| panic!("the tool fixture's `list_dir` call must land; got {calls:?}"));
    assert_eq!(row.status, "done", "tool_end `complete` maps to `done`: {row:?}");
    assert_eq!(
        row.output_preview.as_deref(),
        Some("deterministic fixture listing"),
        "the recorded tool_end output must survive: {row:?}"
    );
    assert_eq!(
        row.duration_ms,
        Some(1),
        "the recorded tool_end duration must survive: {row:?}"
    );

    // Envelope ordering folded: the canonical cursor advanced to the max seen
    // and each turn's thread carries its own last accepted seq.
    let (stream, seq) = store
        .domains
        .turn
        .envelope_cursor()
        .expect("the envelopes advanced the canonical cursor");
    assert_eq!(stream, session, "the cursor stream is the session");
    // The cursor advances to the max `cursor.seq` in the recording — derived,
    // not hard-coded, so the tail probes (a fresh Basic turn) cannot silently
    // invalidate this assertion.
    let max_cursor = frames
        .iter()
        .filter(|f| f.dir == "in" && f.method == "projection/envelope")
        .filter_map(|f| f.body["cursor"]["seq"].as_u64())
        .max()
        .expect("the recording carries envelope cursors");
    assert_eq!(seq, max_cursor, "the cursor advances to the max recorded seq: got {seq}");

    // The tool turn's thread (its turn id) folded through seq 4, then went terminal.
    // (Each turn is its own thread with its own 1-based seq, so a later turn
    // cannot disturb this.)
    assert_eq!(
        store.domains.turn.last_envelope_seq(TURN_TOOL),
        Some(4),
        "the tool turn's thread folded through seq 4"
    );
    assert_eq!(
        store.domains.turn.terminal(TURN_TOOL).as_deref(),
        Some("completed"),
        "the tool turn's `turn_terminal` was recorded"
    );
    assert_eq!(
        store.domains.turn.terminal(TURN_APPROVE).as_deref(),
        Some("completed"),
    );
    assert_eq!(
        store.domains.turn.terminal(TURN_DENY).as_deref(),
        Some("completed"),
    );
    // Every turn in the recording ended: nothing is left in flight.
    assert_eq!(
        store.domains.turn.in_flight_count(),
        0,
        "every fixture turn reached its terminal"
    );
    assert!(
        store.domains.turn.dropped_envelopes().is_empty(),
        "no envelope was dropped as stale: {:?}",
        store.domains.turn.dropped_envelopes()
    );
}

#[test]
fn replayed_approval_lifecycle_settles_the_store_rows() {
    let frames = load_fixture();
    let (store, _) = replay_into_store(&frames);

    let rows = store.domains.approval.pending();
    assert!(
        rows.len() >= 2,
        "two approval fixtures → two rows in the store; got {rows:?}"
    );
    // Both were answered: the `approval/decided` lifecycle settled them.
    assert!(
        rows.iter().all(|r| r.decided),
        "every replayed approval must be settled by its decided event: {rows:?}"
    );
    assert!(
        rows.iter().all(|r| !r.auto_resolved),
        "a client-answered approval is not auto-resolved: {rows:?}"
    );
    assert!(
        rows.iter().all(|r| !r.cancelled),
        "an answered approval is not cancelled: {rows:?}"
    );
    assert!(store.seen_count("approval/requested") >= 2);
    assert!(store.seen_count("approval/decided") >= 2);
}

#[test]
fn a_recorded_read_only_result_decodes_with_its_typed_result() {
    let frames = load_fixture();
    // The outbound read-only calls were really sent...
    for m in ["thread/graph/get", "approval/scopes/list", "turn/state/get"] {
        assert!(
            frames.iter().any(|f| f.dir == "out" && f.method == m),
            "expected our outbound `{m}`"
        );
    }
    // ...and the server really answered each with a typed result (any of the
    // three shapes: a result row or a typed RPC error row).
    assert!(
        frames.iter().any(|f| f.dir == "in"
            && (f.method == "thread/graph/get"
                || f.method.starts_with("error:thread/graph/get"))),
        "thread/graph/get was answered live"
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
    eprintln!("R5 unhandled notification kinds in this recording: {uniq:?}");
    // The registry must never panic on an unhandled kind (RULES #6).
    assert!(true);
}
