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
use octos_core::ui_protocol::methods;
use octoscode_client::trace::FrameTrace;
use octoscode_store::Store;

/// The committed fixture (this card's artifact).
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/r3-session-a6ea8505.jsonl"
);

/// Placeholders a recorded frame uses in place of machine/lane-specific
/// absolute paths. The committed fixture is **hermetic**: it must decode and
/// assert identically on any clone, so no `/Users/…`, `/var/folders/…`,
/// `$TMPDIR` or `$HOME` appears in it. Only the `#[ignore]` recorder reads the
/// environment (to derive the prefixes); the replay tests read the placeholders.
///
/// Card #R3b: the one remaining machine path was `panes.git.repo_root` in the
/// `session/open` reply (the board's "session/workspace.get reply"), which the
/// server fills with the request's working directory.
const TMP_PLACEHOLDER: &str = "<TMP>";
const WORKSPACE_PLACEHOLDER: &str = "<WORKSPACE>";
const HOME_PLACEHOLDER: &str = "<HOME>";

/// The machine-specific absolute prefixes this recording run produced, mapped
/// to placeholders. Derived from the compile-time crate location (workspace
/// root and its parent) and the recorder's own `temp_dir()`. Longest first, so
/// the most specific prefix wins (`<TMP>` inside `<WORKSPACE>`, etc.).
///
/// **Recorder-only.** A replay test never calls this or reads the environment.
fn machine_path_prefixes() -> Vec<(String, &'static str)> {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // …/<repo>/crates/octoscode-client → …/<repo>
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

/// Replace every machine-specific absolute path in `text` with its placeholder,
/// so the fixture does not depend on the lane that recorded it.
fn scrub_machine_paths(text: &str) -> String {
    let mut out = text.to_owned();
    for (from, to) in machine_path_prefixes() {
        out = out.replace(&from, to);
    }
    out
}

/// The machine-specific absolute prefixes this fixture must not contain, as a
/// **hermetic** check a replay test can run without the environment.
///
/// Deliberately only the two the card names. A bare `/home/` would false-positive
/// on this fixture's *relative* `tmp/r3/home/…` data-dir paths, which are not
/// machine paths.
const FORBIDDEN_PATH_PREFIXES: &[&str] = &["/Users/", "/var/folders/"];

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

    // snapshot/restore against the throwaway dir with a snapshot id that does
    // not exist: exercises this owned method AND records a real error frame
    // (the server refuses a restore it cannot satisfy). Safe — throwaway dir.
    match call_recorded(
        &client,
        &trace,
        "snapshot/restore",
        serde_json::json!({"session_id": sid, "snapshot_id": "r3-does-not-exist"}),
    )
    .await
    {
        Ok(v) => println!("snapshot/restore -> {}", short(&v)),
        Err(e) => println!("snapshot/restore -> err: {e}"),
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

    // Scrub lane-specific absolute paths so the committed fixture is hermetic
    // (decodes + asserts identically on any clone). This is the only place the
    // recorder touches the environment; replay tests read the placeholders.
    let scrubbed = std::fs::read_to_string(&path)
        .map(|t| scrub_machine_paths(&t))
        .unwrap_or_default();
    if !scrubbed.is_empty() {
        std::fs::write(&path, &scrubbed).expect("rewrite the scrubbed fixture");
    }
    assert!(
        !scrubbed.contains("/Users/") && !scrubbed.contains("/var/folders/"),
        "the fixture must carry no machine-specific absolute path"
    );
    println!(
        "[r3 capture] wrote {} (scrubbed)",
        trace.path().unwrap_or(PathBuf::from(&path)).display()
    );
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

/// The body of the first fixture frame with `method` and `dir`.
fn frame_body(dir: &str, method: &str) -> serde_json::Value {
    load_frames(FIXTURE)
        .into_iter()
        .find(|f| f.dir == dir && f.method == method)
        .unwrap_or_else(|| panic!("fixture has a {dir} frame for {method}"))
        .body
}

/// Replay result: what the fixture carried and what each notification frame did.
#[derive(Debug, Default)]
struct Replayed {
    dispatched: usize,
    unknown: Vec<String>,
    not_a_notification: Vec<String>,
}

/// Feed every inbound frame of the fixture through the registry.
fn replay(reg: &mut octoscode_client::Registry) -> Replayed {
    let mut out = Replayed::default();
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
            // `res:*` / `err:*` / `state:*` / `session/open` are replies and
            // lifecycle transitions, not notifications — they decode as non-
            // notifications here by construction (the recorder names them).
            Err(_) => out.not_a_notification.push(f.method.clone()),
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

/// **Card #R3b's gate.** The committed fixture must be hermetic: no frame may
/// carry a machine-specific absolute path, so it decodes and asserts the same
/// on any clone. Read the raw text (not the decoded frames) so a path anywhere
/// in a line is caught.
#[test]
fn r3_fixture_carries_no_machine_path() {
    let text = std::fs::read_to_string(FIXTURE).expect("read the R3 fixture");
    for prefix in FORBIDDEN_PATH_PREFIXES {
        assert!(
            !text.contains(prefix),
            "the fixture must carry no machine-specific path ({prefix}); run the \
             #[ignore] recorder, which scrubs machine paths to placeholders"
        );
    }
    // The workspace placeholder must actually be present — the scrub ran.
    assert!(
        text.contains(WORKSPACE_PLACEHOLDER),
        "the recorded repo_root should be threaded through as {WORKSPACE_PLACEHOLDER}"
    );
    // And the specific field the board flagged is placeholder'd, not a real path.
    let repo_root = load_frames(FIXTURE)
        .into_iter()
        .find(|f| f.dir == "in" && f.method == "session/open")
        .and_then(|f| f.body.get("panes").and_then(|p| p.get("git")).and_then(|g| g.get("repo_root")).cloned());
    if let Some(serde_json::Value::String(root)) = repo_root {
        assert_eq!(
            root, WORKSPACE_PLACEHOLDER,
            "panes.git.repo_root must be the placeholder, not a machine path"
        );
    }
}

/// **The card's core test.** Replay the recorded real notifications through the
/// production registry into the store and assert the state they produce.
#[test]
fn r3_replay_real_notifications_into_the_store() {
    let (mut reg, store) = wired();
    let run = replay(&mut reg);
    println!(
        "replayed {} inbound frames; {} not-a-notification; unknown={:?}",
        run.dispatched, run.not_a_notification.len(), run.unknown
    );

    // The capture produced the two context-lifecycle notifications this domain
    // owns (reached via `session/compact` on the authorised profile).
    assert!(
        reg.handles(methods::CONTEXT_COMPACTION_STARTED),
        "the session domain owns context/compaction_started"
    );
    assert_eq!(
        store.seen_count(methods::CONTEXT_COMPACTION_STARTED),
        1,
        "context/compaction_started was seen once"
    );
    assert_eq!(
        store.seen_count(methods::CONTEXT_COMPACTION_COMPLETED),
        1,
        "context/compaction_completed was seen once"
    );

    // The store's context projection reflects the LAST lifecycle frame, which is
    // `compaction_completed` (real order: started → completed).
    let sid = "dsflash:api:main";
    let ctx = store
        .domains
        .session
        .context(sid)
        .expect("the context lifecycle projection is stored");
    assert_eq!(ctx.kind, "compaction_completed");
    // The real frame carried the server's compaction record.
    let detail = ctx.detail.expect("the completed frame carries its record");
    assert!(
        detail.get("compaction_id").and_then(|v| v.as_str()).is_some(),
        "the real compaction record has its id: {detail}"
    );

    // Every decodable notification the fixture carried was claimed by a handler:
    // nothing fell to the tolerated-unknown arm.
    assert!(
        run.unknown.is_empty(),
        "no recorded notification hit the debug! arm: {:?}",
        run.unknown
    );
}

/// **Value-parity test.** The server's REAL result frames must decode into this
/// crate's typed `Method::Result`. A hand-written fake could not prove this —
/// that is the whole point of the card.
#[test]
fn r3_real_results_decode_into_the_owned_types() {
    use octos_core::ui_protocol as core;

    // launch/resolve -> LaunchResolveResult -> store projection
    let launch: core::LaunchResolveResult =
        serde_json::from_value(frame_body("in", "res:launch/resolve")).expect("launch/resolve decodes");
    let res = octoscode_client::domains::config::launch_resolution_from(launch);
    assert_eq!(
        res.decision,
        octoscode_store::domains::config::LaunchDecision::Activate
    );
    assert_eq!(res.resolved_profile.as_deref(), Some("dsflash"));

    // snapshot/list -> our SnapshotListResult -> store projection
    let snaps: octoscode_client::domains::config::SnapshotListResult =
        serde_json::from_value(frame_body("in", "res:snapshot/list")).expect("snapshot/list decodes");
    assert_eq!(snaps.session_id, "dsflash:api:main");
    assert!(snaps.available);
    assert!(!snaps.enabled);
    assert!(snaps.snapshots.is_empty());
    let stored = snaps.into_store();
    assert!(stored.available && stored.snapshots.is_empty());

    // session/list -> our SessionListResult -> store rows
    let list: octoscode_client::domains::session::SessionListResult =
        serde_json::from_value(frame_body("in", "res:session/list")).expect("session/list decodes");
    assert!(list.into_sessions().is_empty(), "a fresh profile has no sessions");

    // session/status/read -> the identity check the web performs
    let status: octoscode_client::domains::session::SessionStatusReadResult =
        serde_json::from_value(frame_body("in", "res:session/status/read"))
            .expect("session/status/read decodes");
    assert_eq!(status.session_id, "dsflash:api:main");
    assert_eq!(status.profile_id.as_deref(), Some("dsflash"));

    // session/fork -> the pin's own type
    let fork: core::SessionForkResult =
        serde_json::from_value(frame_body("in", "res:session/fork")).expect("session/fork decodes");
    assert_eq!(fork.parent_session_id.0, "dsflash:api:main");
    assert_eq!(fork.new_session_id.0, "dsflash:api:r3child");
    assert_eq!(fork.copied_messages, 0);

    // session/rollback -> the pin's own type
    let roll: core::SessionRollbackResult = serde_json::from_value(frame_body("in", "res:session/rollback"))
        .expect("session/rollback decodes");
    assert_eq!(roll.dropped_turns, 0);

    // session/delete -> the empty result the pin models
    let _: core::SessionDeleteResult =
        serde_json::from_value(frame_body("in", "res:session/delete")).expect("session/delete decodes");

    // session/files.list -> the pin's own type
    let files: core::SessionFilesListResult =
        serde_json::from_value(frame_body("in", "res:session/files.list")).expect("files.list decodes");
    assert_eq!(files.files, serde_json::json!([]));

    // session/compact -> our SessionCompactResult (a real FAILED compaction)
    let compact: octoscode_client::domains::session::SessionCompactResult =
        serde_json::from_value(frame_body("in", "res:session/compact")).expect("session/compact decodes");
    assert_eq!(compact.session_id, "dsflash:api:main");
    assert!(!compact.compacted, "the empty-history compaction reports compacted=false");
    assert_eq!(compact.reason.as_deref(), Some("no_safe_semantic_boundary"));
    assert_eq!(compact.token_estimate_before, Some(1));

    // session/compact/mode/set -> our result
    let mode: octoscode_client::domains::session::SessionCompactModeSetResult =
        serde_json::from_value(frame_body("in", "res:session/compact/mode/set"))
            .expect("mode/set decodes");
    assert_eq!(mode.mode, "heuristic");
    assert_eq!(mode.session_id, "dsflash:api:main");

    // server/shutdown -> our result
    let shut: octoscode_client::domains::config::ServerShutdownResult =
        serde_json::from_value(frame_body("in", "res:server/shutdown")).expect("shutdown decodes");
    assert!(shut.stopping);
}

/// The one real **error** frame the capture produced must decode into the
/// fixture's error record and be recognizable by method.
///
/// With the authorised profile present, `session/compact` *succeeds* (its
/// empty-history pass is LLM-free), so the real error the capture yields is
/// `snapshot/restore` refusing a restore with no snapshots taken — a real,
/// server-authored failure that a hand-written fake never produced.
#[test]
fn r3_real_error_frame_is_recorded() {
    let frames = load_frames(FIXTURE);
    let err = frames
        .iter()
        .find(|f| f.dir == "in" && f.method == "error:snapshot/restore")
        .expect("the capture recorded the real snapshot/restore error");
    assert_eq!(err.body["code"], -32602);
    assert!(
        err.body["message"]
            .as_str()
            .unwrap_or("")
            .contains("no snapshots have been taken"),
        "the real error text is kept: {}",
        err.body["message"]
    );
    // Every recorded error frame names its method (`error:<method>`).
    for f in &frames {
        if let Some(m) = f.method.strip_prefix("error:") {
            assert!(
                !m.is_empty() && m.contains('/'),
                "an error frame must name a method, got {m:?}"
            );
        }
    }
    // The failed snapshot/restore also produced no `res:` frame.
    assert!(
        !frames
            .iter()
            .any(|f| f.dir == "in" && f.method == "res:snapshot/restore"),
        "a failed request records no result frame"
    );
}

/// **Step 4.** List every frame kind the capture saw that this domain does not
/// handle. These hit the registry's `debug!` arm (never fatal).
#[test]
fn r3_report_unhandled_frame_kinds() {
    let frames = load_frames(FIXTURE);
    let mut unhandled: Vec<String> = Vec::new();
    for f in &frames {
        if f.dir != "in" {
            continue;
        }
        if let Ok(n) = UiNotification::from_method_and_params(&f.method, f.body.clone()) {
            let (mut reg, _store) = wired();
            if !reg.dispatch(&n) {
                unhandled.push(f.method.clone());
            }
        }
    }
    unhandled.sort();
    unhandled.dedup();
    println!("unhandled notification kinds in the R3 fixture: {unhandled:?}");
    assert!(
        unhandled.is_empty(),
        "the fixture contains notification kinds no domain handles: {unhandled:?}"
    );
}
