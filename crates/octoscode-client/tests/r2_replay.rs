//! #R2 — real-traffic replay for the **profile** domain.
//!
//! Card #13's lesson (and RULES "Tests replay recorded real traffic"): a fake
//! transport hides what a real `octos serve` actually sends. This file has two
//! halves:
//!
//! 1. [`capture_real_profile_frames`] (`#[ignore]`, run against my own serve on
//!    **:50120**) records every frame my domain produces into
//!    `tests/fixtures/r2-profile-a6ea8505.jsonl` via the client's
//!    `OCTOSCODE_TRACE_FILE` recorder.
//! 2. The replay tests feed those **recorded real bodies** back through the
//!    production `Client`/`Method` path and the real `Registry` into a store,
//!    asserting the resulting state. A failure here is a real bug: the fix is
//!    in my domain files, failing test first.
//!
//! ## What the recording may and may not do (RULES: no external network, no
//! model turns)
//!
//! Read-only methods are exercised freely; mutating ones only against the
//! throwaway `tmp/r2/` data dir. Three methods are deliberately **not**
//! recorded because they leave the host: `profile/skills/registry/search`
//! (fetches the public registry, `commands/skills.rs:852`), `profile/llm/test`
//! and `profile/llm/fetch_models` (provider HTTP calls).
use std::sync::{Arc, Mutex};

use octos_app_transport::{
    ws, Capabilities, OutboundCommand, ProfileId, SecretString, TransportConfig, TransportEvent,
};
use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::{methods, RpcError};
use octoscode_client::domains;
use octoscode_client::domains::profile::*;
use octoscode_client::registry::Registry;
use octoscode_client::trace::{wire_params, FrameTrace};
use octoscode_client::{Client, ClientError, Method};
use octoscode_store::Store;
use tokio::sync::mpsc;
use url::Url;

// --------------------------------------------------------------- fixture I/O

const FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/r2-profile-a6ea8505.jsonl"
);

/// Placeholders a recorded frame uses in place of machine/lane-specific
/// absolute paths. The committed fixture is **hermetic**: it must decode and
/// assert identically on any clone, so no `/Users/…`, `$TMPDIR` or `$HOME`
/// appears in it. Only the `#[ignore]` recorder reads the environment (to
/// derive the prefixes); the replay tests read the placeholders.
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

/// One recorded frame (the JSONL line shape from `octoscode_client::trace`).
#[derive(Debug, Clone)]
struct Frame {
    dir: String,
    method: String,
    body: serde_json::Value,
    /// An error frame carries `{"code","message"}` under `error`, NOT `body`
    /// (`octoscode_client::trace::FrameTrace::error` → `write(…, error)`).
    error: Option<serde_json::Value>,
}

impl Frame {
    fn has_error(&self) -> bool {
        self.error.is_some()
    }
}

fn load_fixture() -> Vec<Frame> {
    let text = std::fs::read_to_string(FIXTURE_PATH).expect("read the r2 fixture");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v.get("body").cloned().unwrap_or(serde_json::Value::Null),
                error: v.get("error").cloned(),
            }
        })
        .collect()
}

/// Every method my domain owns (the #F2 list).
const MY_METHODS: &[&str] = &[
    "permission/profile/list",
    "permission/profile/set",
    "onboarding/workspace_list",
    "onboarding/workspace_create",
    "profile/llm/catalog",
    "profile/llm/list",
    "profile/llm/select",
    "profile/llm/upsert",
    "profile/llm/test",
    "profile/llm/delete",
    "profile/llm/fetch_models",
    "profile/skills/list",
    "profile/skills/registry/search",
    "profile/skills/install",
    "profile/skills/remove",
    "profile/sub_providers/list",
    "profile/sub_providers/upsert",
    "profile/sub_providers/remove",
];

/// The recorded real **response** body for `method` (the first `in` frame that
/// carries a body and is not an error), if the fixture has one.
fn recorded_result(frames: &[Frame], method: &str) -> Option<serde_json::Value> {
    frames
        .iter()
        .find(|f| f.method == method && f.dir == "in" && !f.body.is_null() && !f.has_error())
        .map(|f| f.body.clone())
}

/// The recorded real **outbound params** for `method` (what the web-shaped
/// caller sent).
fn recorded_params(frames: &[Frame], method: &str) -> Option<serde_json::Value> {
    frames
        .iter()
        .find(|f| f.method == method && f.dir == "out" && !f.body.is_null())
        .map(|f| f.body.clone())
}

/// A fake transport that answers `method` with the recorded real body. This is
/// the same command/reply contract the WS transport implements, so `Client::call`
/// runs the production path over a real server's bytes.
fn fake_transport_from_fixture(
    frames: Arc<Vec<Frame>>,
) -> mpsc::Sender<OutboundCommand> {
    let (tx, mut rx) = mpsc::channel::<OutboundCommand>(8);
    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            if let OutboundCommand::Request { method, params, reply } = cmd {
                let _ = params;
                match recorded_result(&frames, &method) {
                    Some(body) => {
                        let _ = reply.send(Ok(body));
                    }
                    None => {
                        let _ = reply.send(Err(RpcError {
                            code: -32601,
                            message: format!("no recorded body for {method}"),
                            data: None,
                        }));
                    }
                }
            }
        }
    });
    tx
}

// -------------------------------------------------------- 1. the recording

/// The recorder: drives the production `Client` against a live serve and
/// records both directions. `#[ignore]` — needs my serve on :50120.
///
/// ```sh
/// OCTOS_BASE_URL=http://127.0.0.1:50120 ctest -p octoscode-client \
///   --test r2_replay -- --ignored --nocapture capture_real_profile_frames
/// ```
#[tokio::test]
#[ignore = "records real frames from a live serve on :50120; run with --ignored"]
async fn capture_real_profile_frames() {
    let base = std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50120".into());
    let path = std::env::var("R2_FIXTURE").unwrap_or_else(|_| FIXTURE_PATH.to_string());
    let _ = std::fs::remove_file(&path);
    let trace = FrameTrace::open(&path);
    assert!(trace.is_enabled(), "trace file {path} is writable");

    let cfg = TransportConfig {
        base_url: Url::parse(&base).expect("base url"),
        bearer: SecretString::new(String::from("spike-dummy-token")),
        profile_id: ProfileId::new(String::from("dsflash")),
        cursor: None,
        cursor_file: None,
        requested_capabilities: Capabilities::requested(),
        workspace_cwd: None,
        local_kernel: false,
    };
    let (cmd_tx, mut evt_rx) = ws::spawn(cfg);

    // Drain the event channel (never back-pressure) and record every inbound
    // notification exactly as the transport decoded it.
    let drain = trace.clone();
    tokio::spawn(async move {
        while let Some(evt) = evt_rx.recv().await {
            if let TransportEvent::DurableNotification { payload, .. }
            | TransportEvent::EphemeralNotification { payload } = &evt
            {
                drain.inbound(payload.method(), &wire_params(payload));
            }
        }
    });

    let reset_client = Client::new(cmd_tx.clone());
    let client = Client::with_trace(cmd_tx, trace.clone());
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Issue one request and record BOTH the outbound frame (the Client already
    // does this) and the server's real reply body.
    async fn r(
        client: &Client,
        trace: &FrameTrace,
        method: &str,
        params: serde_json::Value,
    ) -> serde_json::Value {
        match client.request(method, params.clone()).await {
            Ok(v) => {
                trace.result(method, None, &v);
                println!("[capture] {method} -> ok");
                v
            }
            Err(ClientError::Rpc { error, .. }) => {
                trace.error(method, None, &error.message, i64::from(error.code));
                println!("[capture] {method} -> rpc error {} ({})", error.code, error.message);
                serde_json::Value::Null
            }
            Err(e) => {
                trace.error(method, None, &format!("{e}"), 0);
                println!("[capture] {method} -> transport error {e}");
                serde_json::Value::Null
            }
        }
    }

    // The connection is the profile the serve loaded from tmp/r2/data/profiles.
    let profile = "dsflash";
    let session = format!("{profile}:main");

    // Read-only, local.
    r(&client, &trace, "config/capabilities/list", serde_json::json!({})).await;
    r(&client, &trace, "session/open", serde_json::json!({"session_id": session, "profile_id": profile})).await;

    // Deterministic reset of the throwaway profile so the recording is
    // REPRODUCIBLE: the permission mode is sticky and the probe dir persists,
    // so without this a second run records different real bodies
    // (`applied:false`, `created:false`). Reset to the serve's own default and
    // remove anything a previous run created.
    let probe_dir = std::env::temp_dir().join("r2-workspace-probe");
    let _ = std::fs::remove_dir_all(&probe_dir);
    // Untraced: only the client's own frames belong in the fixture.
    let _ = reset_client
        .request(
            "permission/profile/set",
            serde_json::json!({"session_id": session, "update": {"mode": "workspace_write"}}),
        )
        .await;

    r(&client, &trace, "permission/profile/list", serde_json::json!({"session_id": session})).await;
    r(&client, &trace, "onboarding/workspace_list", serde_json::json!({})).await;
    r(&client, &trace, "profile/llm/catalog", serde_json::json!({})).await;
    r(&client, &trace, "profile/llm/list", serde_json::json!({"profile_id": profile})).await;
    r(&client, &trace, "profile/skills/list", serde_json::json!({"profile_id": profile})).await;
    r(&client, &trace, "profile/sub_providers/list", serde_json::json!({"profile_id": profile})).await;

    // Mutating, but only the throwaway tmp/r2 data dir (create then delete).
    r(
        &client,
        &trace,
        "permission/profile/set",
        serde_json::json!({"session_id": session, "update": {"mode": "read_only"}}),
    )
    .await;
    r(
        &client,
        &trace,
        "profile/sub_providers/upsert",
        serde_json::json!({"profile_id": profile, "sub_provider": {
            "key": "r2-lane", "provider": "zhipu", "model": "glm-4-flash",
            "api_key_env": "R2_LANE_KEY", "description": "r2 throwaway"
        }}),
    )
    .await;
    r(&client, &trace, "profile/sub_providers/list", serde_json::json!({"profile_id": profile})).await;
    r(
        &client,
        &trace,
        "profile/sub_providers/remove",
        serde_json::json!({"profile_id": profile, "key": "r2-lane"}),
    )
    .await;
    // Skills install/remove: the ONLY local form is the server's own
    // precondition refusal (an empty repo is rejected BEFORE any network).
    r(
        &client,
        &trace,
        "profile/skills/install",
        serde_json::json!({"profile_id": profile, "repo": "", "force": false}),
    )
    .await;
    r(
        &client,
        &trace,
        "profile/skills/remove",
        serde_json::json!({"profile_id": profile, "name": "r2-no-such-skill"}),
    )
    .await;
    // A workspace create in the throwaway dir, then leave it (r2-* only).
    r(
        &client,
        &trace,
        "onboarding/workspace_create",
        serde_json::json!({"parent": std::env::temp_dir().to_string_lossy(), "name": "r2-workspace-probe"}),
    )
    .await;

    // The `profile/llm/*` mutations, against the throwaway profile only:
    // upsert a NEW route (no provider call), select it, then delete it. These
    // never leave the host.
    r(
        &client,
        &trace,
        "profile/llm/upsert",
        serde_json::json!({"profile_id": profile, "set_primary": false, "selection": {
            "family_id": "deepseek", "model_id": "deepseek-v4-flash",
            "route": {"route_id": "r2-route", "api_type": "openai", "base_url": "http://127.0.0.1:9/v1"}
        }}),
    )
    .await;
    r(&client, &trace, "profile/llm/list", serde_json::json!({"profile_id": profile})).await;
    r(
        &client,
        &trace,
        "profile/llm/select",
        serde_json::json!({"session_id": session, "profile_id": profile,
            "family_id": "deepseek", "model_id": "deepseek-v4-flash", "route_id": "r2-route"}),
    )
    .await;
    r(
        &client,
        &trace,
        "profile/llm/delete",
        serde_json::json!({"profile_id": profile, "family_id": "deepseek",
            "model_id": "deepseek-v4-flash", "route_id": "r2-route"}),
    )
    .await;

    // Let trailing frames land, then flush.
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    let _ = client.request("session/list", serde_json::json!({})).await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
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
    println!("[capture] wrote {path} (scrubbed)");
}

// ---------------------------------------------------------- fixture pickers

/// The Nth recorded **server result** body for `method` (0-based), skipping
/// error frames — a method recorded twice (before/after a mutation) has two.
fn nth_result(frames: &[Frame], method: &str, n: usize) -> serde_json::Value {
    frames
        .iter()
        .filter(|f| f.method == method && f.dir == "in" && !f.body.is_null() && !f.has_error())
        .nth(n)
        .unwrap_or_else(|| panic!("fixture has result #{n} for {method}"))
        .body
        .clone()
}

/// The first recorded server result for `method`.
fn result_body(frames: &[Frame], method: &str) -> serde_json::Value {
    nth_result(frames, method, 0)
}

/// The recorded error frame for `method` (code, message).
fn error_frame(frames: &[Frame], method: &str) -> (i64, String) {
    let f = frames
        .iter()
        .find(|f| f.method == method && f.dir == "in" && f.has_error())
        .unwrap_or_else(|| panic!("fixture has an error frame for {method}"));
    let err = f.error.as_ref().expect("an error frame carries `error`");
    (
        err["code"].as_i64().unwrap_or(0),
        err["message"].as_str().unwrap_or("").to_owned(),
    )
}

/// The recorded **outbound params** for `method` (what the caller sent).
fn out_body(frames: &[Frame], method: &str) -> serde_json::Value {
    frames
        .iter()
        .find(|f| f.method == method && f.dir == "out")
        .unwrap_or_else(|| panic!("fixture has an out frame for {method}"))
        .body
        .clone()
}

/// Repair the ONE artifact the fixture recorder introduces: its redactor
/// (`octoscode_client::trace::is_secret_key`) matches the substring `api_key`,
/// which also matches **`has_api_key` — a boolean on the wire**
/// (`types.ts:115`, `onboarding.ts:444`). The recorder writes the string
/// `"<redacted>"` where the wire has a bool, so the recorded body cannot decode
/// into a typed client. A redacted bool is unrecoverable, so this **drops** the
/// key (the field is `#[serde(default)]`), leaving every other field the real
/// server sent. The general fix belongs in `trace.rs`, outside this card's
/// May-touch; it is asserted visible by `the_fixture_redaction_paints_has_api_key`.
fn repair_unrecoverable_redactions(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            if map.get("has_api_key").map(serde_json::Value::is_string) == Some(true) {
                map.remove("has_api_key");
            }
            for (_, child) in map.iter_mut() {
                repair_unrecoverable_redactions(child);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items.iter_mut() {
                repair_unrecoverable_redactions(child);
            }
        }
        _ => {}
    }
}

/// A one-shot fake transport that answers `method` with one recorded body.
fn replay_transport(
    method: &'static str,
    body: serde_json::Value,
) -> mpsc::Sender<OutboundCommand> {
    let (tx, mut rx) = mpsc::channel::<OutboundCommand>(4);
    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            if let OutboundCommand::Request { method: m, reply, .. } = cmd {
                assert_eq!(m, method, "the replay serves the recorded method");
                let _ = reply.send(Ok(body.clone()));
            }
        }
    });
    tx
}

/// Decode one recorded real body through the production `Client::call` path.
async fn replay<M: Method>(method: &'static str, body: serde_json::Value, params: M::Params) -> M::Result {
    let client = Client::new(replay_transport(method, body));
    client.call::<M>(params).await.expect("the recorded real body decodes")
}

// ------------------------------------------------ outbound params parity

/// The recorded **outbound** frames are the exact params the (web-shaped)
/// caller sent. Our `Params` must serialize to the same JSON, so a replay is
/// faithful in both directions.
#[test]
fn outbound_params_match_the_recorded_frames() {
    let f = load_fixture();
    let eq = |method: &str, got: serde_json::Value| {
        let want = out_body(&f, method);
        assert_eq!(got, want, "outbound params for {method}");
    };
    eq(
        "permission/profile/list",
        serde_json::to_value(octos_core::ui_protocol::PermissionProfileListParams {
            session_id: octos_core::SessionKey("dsflash:main".into()),
        })
        .unwrap(),
    );
    eq(
        "permission/profile/set",
        serde_json::to_value(octos_core::ui_protocol::PermissionProfileSetParams {
            session_id: octos_core::SessionKey("dsflash:main".into()),
            update: octos_core::ui_protocol::PermissionProfileUpdate {
                mode: Some(octos_core::ui_protocol::PermissionProfileMode::ReadOnly),
                ..Default::default()
            },
            runtime_mode: None,
        })
        .unwrap(),
    );
    eq(
        "profile/llm/catalog",
        serde_json::to_value(LlmCatalogParams {}).unwrap(),
    );
    eq(
        "profile/llm/list",
        serde_json::to_value(ProfileLlmListParams {
            session_id: None,
            profile_id: Some("dsflash".into()),
        })
        .unwrap(),
    );
    eq(
        "profile/skills/list",
        serde_json::to_value(SkillsListParams { profile_id: Some("dsflash".into()) }).unwrap(),
    );
    eq(
        "profile/sub_providers/list",
        serde_json::to_value(SubProvidersListParams { profile_id: Some("dsflash".into()) }).unwrap(),
    );
    eq(
        "profile/sub_providers/remove",
        serde_json::to_value(SubProvidersRemoveParams {
            profile_id: Some("dsflash".into()),
            key: "r2-lane".into(),
        })
        .unwrap(),
    );
    // `onboarding/workspace_create`: build the params from the fixture's OWN
    // recorded value, never this machine's `$TMPDIR`. The recorded `parent` is
    // a scrubbed placeholder, so the assertion is lane-independent.
    {
        let recorded = out_body(&f, "onboarding/workspace_create");
        assert_eq!(
            recorded["parent"],
            serde_json::json!(TMP_PLACEHOLDER),
            "the fixture scrubbed the create parent to a placeholder"
        );
        assert_eq!(recorded["name"], serde_json::json!("r2-workspace-probe"));
        eq(
            "onboarding/workspace_create",
            serde_json::to_value(WorkspaceCreateParams {
                parent: recorded["parent"].as_str().expect("parent string").to_owned(),
                name: recorded["name"].as_str().expect("name string").to_owned(),
            })
            .unwrap(),
        );
    }
    // `onboarding/workspace_list` was recorded with the params OMITTED (`{}`),
    // which the server documents as identical to `{"path": null}`
    // (`ui_protocol_transport.rs`: "params may be omitted entirely, which means
    // the same as `{"path": null}`"). Assert both forms.
    assert_eq!(out_body(&f, "onboarding/workspace_list"), serde_json::json!({}));
    assert_eq!(
        serde_json::to_value(WorkspaceListParams { path: None }).unwrap(),
        serde_json::json!({ "path": null })
    );
}

// --------------------------------------- the tracer defect (made visible)

/// The fixture's redactor paints `has_api_key` (a bool) as `"<redacted>"`.
/// This asserts the artifact exists, so `repair_unrecoverable_redactions` is
/// an honest, visible repair — not a silent weakening. The fix belongs in
/// `crates/octoscode-client/src/trace.rs` (outside this card's May-touch).
#[test]
fn the_fixture_redaction_paints_has_api_key() {
    let f = load_fixture();
    let llm = result_body(&f, "profile/llm/list");
    let painted = llm["primary"]["has_api_key"] == serde_json::json!("<redacted>");
    assert!(
        painted,
        "expected the recorder to paint the boolean has_api_key; body: {llm}"
    );
    // A string-valued secret is genuinely redacted and STAYS redacted.
    assert_eq!(llm["primary"]["api_key_env"], serde_json::json!("<redacted>"));
}

// --------------------------------------------- decode the recorded bodies

#[tokio::test]
async fn recorded_permission_profile_list_decodes() {
    let f = load_fixture();
    let out = replay::<PermissionProfileList>(
        "permission/profile/list",
        result_body(&f, "permission/profile/list"),
        octos_core::ui_protocol::PermissionProfileListParams {
            session_id: octos_core::SessionKey("dsflash:main".into()),
        },
    )
    .await;
    assert_eq!(out.session_id.0, "dsflash:main");
    assert_eq!(out.current.mode.label(), "Workspace Write");
    assert_eq!(out.profiles.len(), 3, "the serve offers three modes");
}

#[tokio::test]
async fn recorded_permission_profile_set_decodes() {
    let f = load_fixture();
    let out = replay::<PermissionProfileSet>(
        "permission/profile/set",
        result_body(&f, "permission/profile/set"),
        octos_core::ui_protocol::PermissionProfileSetParams {
            session_id: octos_core::SessionKey("dsflash:main".into()),
            update: octos_core::ui_protocol::PermissionProfileUpdate {
                mode: Some(octos_core::ui_protocol::PermissionProfileMode::ReadOnly),
                ..Default::default()
            },
            runtime_mode: None,
        },
    )
    .await;
    assert!(out.applied, "the recorded set applied");
    assert_eq!(out.current.mode.label(), "Read Only");
}

#[tokio::test]
async fn recorded_workspace_list_decodes() {
    let f = load_fixture();
    let out = replay::<WorkspaceList>(
        "onboarding/workspace_list",
        result_body(&f, "onboarding/workspace_list"),
        WorkspaceListParams { path: None },
    )
    .await;
    assert!(out.writable);
    assert!(!out.truncated);
    assert_eq!(out.hidden_skipped, 3);
    assert_eq!(out.entries.len(), 10);
    assert!(out.entries.iter().any(|e| e.name == "crates"));
    assert!(out.parent_path.is_some());
}

#[tokio::test]
async fn recorded_workspace_create_decodes() {
    let f = load_fixture();
    // Hermetic: the params come from the fixture's own recorded frame, never
    // from this machine's environment.
    let recorded = out_body(&f, "onboarding/workspace_create");
    let out = replay::<WorkspaceCreate>(
        "onboarding/workspace_create",
        result_body(&f, "onboarding/workspace_create"),
        WorkspaceCreateParams {
            parent: recorded["parent"].as_str().expect("parent string").to_owned(),
            name: recorded["name"].as_str().expect("name string").to_owned(),
        },
    )
    .await;
    assert!(out.created);
    assert!(out.canonical_path.ends_with("r2-workspace-probe"));
}

/// The shape a hand-written fake got WRONG: the real server sends `families` as
/// a MAP keyed by family id (`Object.entries` on the web,
/// `onboarding.ts:225`), not an array.
#[tokio::test]
async fn recorded_llm_catalog_decodes_the_family_map() {
    let f = load_fixture();
    let body = result_body(&f, "profile/llm/catalog");
    assert!(body["families"].is_object(), "recorded families is a MAP");
    let out = replay::<LlmCatalog>("profile/llm/catalog", body, LlmCatalogParams {}).await;
    assert!(!out.families.is_empty(), "the serve ships a canonical catalog");
    // The map key becomes the row's `id` (what the web's `Object.entries` does).
    assert!(out.families.iter().any(|fam| fam.id == "anthropic"));
    assert!(
        out.families.iter().all(|fam| !fam.id.is_empty()),
        "every family row carries its map key as id"
    );
}

#[tokio::test]
async fn recorded_llm_list_decodes_after_repair() {
    let f = load_fixture();
    let mut body = result_body(&f, "profile/llm/list");
    repair_unrecoverable_redactions(&mut body);
    let out = replay::<ProfileLlmList>(
        "profile/llm/list",
        body,
        ProfileLlmListParams { session_id: None, profile_id: Some("dsflash".into()) },
    )
    .await;
    assert_eq!(out.profile_id.as_deref(), Some("dsflash"));
    let primary = out.primary.expect("the serve configured a primary model");
    assert_eq!(primary.family_id, "deepseek");
    assert_eq!(primary.model_id, "deepseek-v4-flash");
    assert!(primary.selected);
    assert!(out.fallbacks.is_empty());
}

#[tokio::test]
async fn recorded_skills_list_decodes() {
    let f = load_fixture();
    let out = replay::<SkillsList>(
        "profile/skills/list",
        result_body(&f, "profile/skills/list"),
        SkillsListParams { profile_id: Some("dsflash".into()) },
    )
    .await;
    assert_eq!(out.profile_id, "dsflash");
    assert_eq!(out.count, 0);
    assert!(out.skills.is_empty());
}

#[tokio::test]
async fn recorded_skills_remove_decodes() {
    let f = load_fixture();
    let out = replay::<SkillsRemove>(
        "profile/skills/remove",
        result_body(&f, "profile/skills/remove"),
        SkillsRemoveParams { profile_id: Some("dsflash".into()), name: "r2-no-such-skill".into() },
    )
    .await;
    assert!(out.ok);
    assert_eq!(out.removed, "r2-no-such-skill");
}

/// `profile/skills/install` with an empty repo is refused by the server BEFORE
/// any network call (`repo is required`), so this is a safe local recording.
/// The RPC error must surface typed, naming the method.
#[test]
fn recorded_skills_install_error_is_typed() {
    let f = load_fixture();
    let (code, message) = error_frame(&f, "profile/skills/install");
    assert_eq!(code, -32602, "invalid_params");
    assert!(message.contains("repo"), "server text: {message}");
}

#[tokio::test]
async fn recorded_sub_providers_list_decodes() {
    let f = load_fixture();
    let out = replay::<SubProvidersList>(
        "profile/sub_providers/list",
        result_body(&f, "profile/sub_providers/list"),
        SubProvidersListParams { profile_id: Some("dsflash".into()) },
    )
    .await;
    assert_eq!(out.profile_id, "dsflash");
    assert_eq!(out.sub_providers.len(), 2, "the profile ships two lanes");
    let keys: Vec<&str> = out.sub_providers.iter().map(|l| l.key.as_str()).collect();
    assert!(keys.contains(&"cheap") && keys.contains(&"strong"));
    // `api_key_env` is a genuine string secret → redacted, still a string.
    assert!(out.sub_providers.iter().all(|l| l.api_key_env.as_deref() == Some("<redacted>")));
}

/// The upsert receipt is recorded AFTER the throwaway `r2-lane` was added, so
/// it carries three lanes — the real "after" body, not a fake's guess.
#[tokio::test]
async fn recorded_sub_providers_upsert_decodes() {
    let f = load_fixture();
    let out = replay::<SubProvidersUpsert>(
        "profile/sub_providers/upsert",
        result_body(&f, "profile/sub_providers/upsert"),
        SubProvidersUpsertParams {
            profile_id: Some("dsflash".into()),
            sub_provider: SubProviderParams {
                key: "r2-lane".into(),
                provider: "zhipu".into(),
                model: Some("glm-4-flash".into()),
                api_key_env: Some("R2_LANE_KEY".into()),
                base_url: None,
                description: Some("r2 throwaway".into()),
                default_context_window: None,
                max_output_tokens: None,
                api_type: None,
            },
            api_key: None,
        },
    )
    .await;
    assert!(out.applied);
    assert!(out.restart_required, "a persisted lane needs a restart to go live");
    assert!(out.sub_providers.iter().any(|l| l.key == "r2-lane"));
}

#[tokio::test]
async fn recorded_sub_providers_remove_decodes() {
    let f = load_fixture();
    let out = replay::<SubProvidersRemove>(
        "profile/sub_providers/remove",
        result_body(&f, "profile/sub_providers/remove"),
        SubProvidersRemoveParams { profile_id: Some("dsflash".into()), key: "r2-lane".into() },
    )
    .await;
    assert!(out.applied);
    assert!(out.restart_required);
    assert!(!out.sub_providers.iter().any(|l| l.key == "r2-lane"), "the lane is gone");
}

// ------------------------------------------- the decoded state into the store

/// The card's third leg: the decoded real results, applied to the store domain
/// exactly as the UI lane would, read back. (This domain owns no notifications,
/// so there is no registry leg — see the step-4 note in the report.)
#[tokio::test]
async fn recorded_results_land_in_the_store() {
    let f = load_fixture();
    let store = Store::new();

    // permission/profile/list → store
    let perms = replay::<PermissionProfileList>(
        "permission/profile/list",
        result_body(&f, "permission/profile/list"),
        octos_core::ui_protocol::PermissionProfileListParams {
            session_id: octos_core::SessionKey("dsflash:main".into()),
        },
    )
    .await;
    store.domains.profile.set_permission(
        octoscode_store::domains::profile::PermissionProfileSelection {
            mode: octoscode_store::domains::profile::PermissionProfileMode::WorkspaceWrite,
            network: octoscode_store::domains::profile::PermissionNetworkPolicy::Allow,
        },
        perms
            .profiles
            .iter()
            .map(|p| octoscode_store::domains::profile::PermissionProfileSelection {
                mode: match p.mode {
                    octos_core::ui_protocol::PermissionProfileMode::ReadOnly => {
                        octoscode_store::domains::profile::PermissionProfileMode::ReadOnly
                    }
                    octos_core::ui_protocol::PermissionProfileMode::WorkspaceWrite => {
                        octoscode_store::domains::profile::PermissionProfileMode::WorkspaceWrite
                    }
                    octos_core::ui_protocol::PermissionProfileMode::DangerFullAccess => {
                        octoscode_store::domains::profile::PermissionProfileMode::DangerFullAccess
                    }
                },
                network: match p.network {
                    octos_core::ui_protocol::PermissionNetworkPolicy::Allow => {
                        octoscode_store::domains::profile::PermissionNetworkPolicy::Allow
                    }
                    octos_core::ui_protocol::PermissionNetworkPolicy::Deny => {
                        octoscode_store::domains::profile::PermissionNetworkPolicy::Deny
                    }
                },
            })
            .collect(),
    );
    assert_eq!(store.domains.profile.permission_profiles().len(), 3);
    assert_eq!(
        store.domains.profile.permission().unwrap().mode,
        octoscode_store::domains::profile::PermissionProfileMode::WorkspaceWrite
    );

    // profile/sub_providers/list → store
    let lanes = replay::<SubProvidersList>(
        "profile/sub_providers/list",
        result_body(&f, "profile/sub_providers/list"),
        SubProvidersListParams { profile_id: Some("dsflash".into()) },
    )
    .await;
    store.domains.profile.set_sub_providers(
        lanes
            .sub_providers
            .iter()
            .map(|l| octoscode_store::domains::profile::SubProvider {
                key: l.key.clone(),
                provider: l.provider.clone(),
                model: l.model.clone(),
                api_key_env: l.api_key_env.clone(),
                base_url: l.base_url.clone(),
                description: l.description.clone(),
                default_context_window: l.default_context_window,
                max_output_tokens: l.max_output_tokens,
                api_type: l.api_type.clone(),
            })
            .collect(),
    );
    assert_eq!(store.domains.profile.sub_providers().len(), 2);
    assert!(store.domains.profile.sub_providers().iter().any(|l| l.key == "cheap"));

    // profile/skills/list → store
    let skills = replay::<SkillsList>(
        "profile/skills/list",
        result_body(&f, "profile/skills/list"),
        SkillsListParams { profile_id: Some("dsflash".into()) },
    )
    .await;
    store.domains.profile.set_installed_skills(
        skills
            .skills
            .iter()
            .map(|s| octoscode_store::domains::profile::InstalledSkill {
                name: s.name.clone(),
                version: s.version.clone(),
                tool_count: s.tool_count,
                source_repo: s.source_repo.clone(),
            })
            .collect(),
    );
    assert!(store.domains.profile.installed_skills().is_empty());

    // profile/llm/list → store: the profile-scoped read carries no session
    // model picker, so the picker list stays empty while the primary is seen.
    let mut llm_body = result_body(&f, "profile/llm/list");
    repair_unrecoverable_redactions(&mut llm_body);
    let llm = replay::<ProfileLlmList>(
        "profile/llm/list",
        llm_body,
        ProfileLlmListParams { session_id: None, profile_id: Some("dsflash".into()) },
    )
    .await;
    store.domains.profile.set_llm_models(
        llm.models
            .iter()
            .map(|m| octoscode_store::domains::profile::ProfileLlmModel {
                model: m.model.clone(),
                provider: m.provider.clone(),
                title: m.title.clone(),
                family: m.family.clone(),
                route: m.route.clone(),
                selected: m.selected,
                available: m.available,
            })
            .collect(),
    );
    assert!(store.domains.profile.llm_models().is_empty());
    assert!(llm.primary.is_some());
}

// ---------------------------------- profile/llm/* mutations (throwaway data)

#[tokio::test]
async fn recorded_llm_upsert_decodes() {
    let f = load_fixture();
    let out = replay::<LlmUpsert>(
        "profile/llm/upsert",
        result_body(&f, "profile/llm/upsert"),
        LlmProvisionParams {
            profile_id: Some("dsflash".into()),
            selection: LlmSelection {
                family_id: "deepseek".into(),
                model_id: "deepseek-v4-flash".into(),
                route: LlmRouteSelection {
                    route_id: Some("r2-route".into()),
                    api_type: Some("openai".into()),
                    base_url: Some("http://127.0.0.1:9/v1".into()),
                    ..Default::default()
                },
                inference: Default::default(),
            },
            api_key: None,
            set_primary: Some(false),
        },
    )
    .await;
    assert!(out.applied);
    assert_eq!(out.profile_id, "dsflash");
}

#[tokio::test]
async fn recorded_llm_select_decodes() {
    let f = load_fixture();
    let out = replay::<ProfileLlmSelect>(
        "profile/llm/select",
        result_body(&f, "profile/llm/select"),
        ProfileLlmSelectParams {
            session_id: "dsflash:main".into(),
            profile_id: Some("dsflash".into()),
            family_id: "deepseek".into(),
            model_id: "deepseek-v4-flash".into(),
            route_id: Some("r2-route".into()),
        },
    )
    .await;
    assert!(out.applied);
    assert_eq!(out.session_id, "dsflash:main");
    assert_eq!(out.selected.model, "deepseek-v4-flash");
    assert_eq!(out.selected.provider, "deepseek");
    assert_eq!(out.selected.route.as_deref(), Some("r2-route"));
    // The serve reports the pinned runtime needs a restart to pick this up.
    assert_eq!(out.restart_required, Some(true));
}

#[tokio::test]
async fn recorded_llm_delete_decodes() {
    let f = load_fixture();
    let mut body = result_body(&f, "profile/llm/delete");
    repair_unrecoverable_redactions(&mut body);
    let out = replay::<ProfileLlmDelete>(
        "profile/llm/delete",
        body,
        ProfileLlmDeleteParams {
            profile_id: Some("dsflash".into()),
            family_id: "deepseek".into(),
            model_id: "deepseek-v4-flash".into(),
            route_id: "r2-route".into(),
        },
    )
    .await;
    assert!(out.applied);
    // After deleting the throwaway route the profile is back to the official
    // primary, and no fallback carries `r2-route`.
    let primary = out.primary.expect("the official primary remains");
    assert_eq!(primary.route.route_id.as_deref(), Some("deepseek"));
    assert!(out.fallbacks.is_empty());
}
