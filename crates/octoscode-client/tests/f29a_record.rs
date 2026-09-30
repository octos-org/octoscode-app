//! #29a — capture REAL a6ea8505/dsflash onboarding traffic for the board-2
//! screens' protocol sequence into ONE hermetic fixture.
//!
//! `#[ignore]`d recorder only; the replay tests live in
//! `crates/octoscode-module/tests/f29a_replay.rs` and never touch the
//! environment (LESSONS "Replay tests must be hermetic").
//!
//! ## Run (my own serve port 50140, my own `tmp/` data dirs)
//! ```sh
//! tmp/octos-a6ea8505 serve --solo --port 50140 --auth-token mapb-dummy \
//!   --data-dir $PWD/tmp/r29a-data --instance-data-dir $PWD/tmp/r29a-data &
//! OCTOS_BASE_URL=http://127.0.0.1:50140 OCTOS_BEARER=mapb-dummy \
//!   ctest -p octoscode-client --test f29a_record -- --ignored --nocapture
//! ```
//! Then scrub + commit
//! `crates/octoscode-client/tests/fixtures/r29a-onboarding-a6ea8505.jsonl`.
//!
//! ## What is recorded (the screens' actions, exactly)
//! `config/capabilities/list` + `session/open` (the connect handshake), then
//! the onboarding sequence `screens::connect::run_onboarding` performs:
//! `profile/llm/catalog` → `profile/local/create` (requested_id `octos-dev`)
//! → `profile/llm/test` → `profile/llm/upsert` (`set_primary: true`).
//! Whatever the server really answers (including rpc errors — e.g. a provider
//! test without a real key) is the recorded truth the replay asserts against.
use octos_app_transport::{ws, Capabilities, ProfileId, SecretString, TransportConfig, TransportEvent};
use octoscode_client::trace::{wire_params, FrameTrace};
use octoscode_client::{Client, ClientError};
use serde_json::{json, Value};
use url::Url;

const SERVE_PORT: u16 = 50140;

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/r29a-onboarding-a6ea8505.jsonl")
}

// ------------------------------------------------------------- hermeticity

const TMP_PLACEHOLDER: &str = "<TMP>";
const WORKSPACE_PLACEHOLDER: &str = "<WORKSPACE>";
const HOME_PLACEHOLDER: &str = "<HOME>";

/// **Recorder-only.** The machine-specific prefixes this run produced,
/// longest first so the most specific wins.
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
    if let Some(ws_root) = &workspace_root {
        if !ws_root.is_empty() {
            prefixes.push((ws_root.clone(), WORKSPACE_PLACEHOLDER));
            if let Some(home) = std::path::Path::new(ws_root).parent() {
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

#[tokio::test]
#[ignore]
async fn capture_real_onboarding_frames() {
    let base = std::env::var("OCTOS_BASE_URL")
        .unwrap_or_else(|_| format!("http://127.0.0.1:{SERVE_PORT}"));
    let path = std::env::var("R29A_FIXTURE").unwrap_or_else(|_| fixture_path().to_string_lossy().to_string());
    let _ = std::fs::remove_file(&path);
    let trace = FrameTrace::open(&path);
    assert!(trace.is_enabled(), "trace file {path} is writable");

    let cfg = TransportConfig {
        base_url: Url::parse(&base).expect("base url"),
        bearer: SecretString::new(String::from("mapb-dummy")),
        profile_id: ProfileId::new(String::from("octoscode")),
        cursor: None,
        cursor_file: None,
        requested_capabilities: Capabilities::requested(),
        workspace_cwd: None,
        local_kernel: false,
    };
    let (cmd_tx, mut evt_rx) = ws::spawn(cfg);
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

    async fn r(
        client: &Client,
        trace: &FrameTrace,
        method: &str,
        params: Value,
    ) -> Value {
        match client.request(method, params.clone()).await {
            Ok(v) => {
                trace.result(method, None, &v);
                println!("[capture] {method} -> ok");
                v
            }
            Err(ClientError::Rpc { error, .. }) => {
                trace.error(method, None, &error.message, i64::from(error.code));
                println!("[capture] {method} -> rpc error {} ({})", error.code, error.message);
                Value::Null
            }
            Err(e) => {
                trace.error(method, None, &format!("{e}"), 0);
                println!("[capture] {method} -> transport error {e}");
                Value::Null
            }
        }
    }

    // 1. the connect handshake the screens drive before anything else.
    r(&client, &trace, "config/capabilities/list", json!({})).await;
    let session = "octoscode:main";
    r(&client, &trace, "session/open", json!({"session_id": session, "profile_id": "octoscode"})).await;

    // 2. the onboarding catalog (the panel's `prepare`).
    let catalog = r(&client, &trace, "profile/llm/catalog", json!({})).await;

    // Pick the provider the recorded catalog actually advertises (first of
    // deepseek/kimi/glm present), so the recorded test/upsert are the REAL
    // bodies for a radio this screen set offers.
    let pick = ["deepseek", "kimi", "glm"]
        .iter()
        .find(|id| catalog["families"].get(**id).is_some())
        .copied()
        .unwrap_or("deepseek");
    let family = catalog["families"][pick].clone();
    let model_id = family["models"][0]["id"].as_str().unwrap_or("").to_owned();
    let env = family["env"].as_str().unwrap_or("").to_owned();
    println!("[capture] provider={pick} model={model_id} env={env:?}");

    let selection = json!({
        "family_id": pick,
        "model_id": model_id,
        "route": {
            "route_id": pick,
            "label": "Official API",
            "api_key_env": env,
            "api_type": "openai",
        },
    });

    // 3. `profile/local/create` — the method this card adds a fixture for.
    let created = r(&client, &trace, "profile/local/create", json!({
        "requested_id": "octos-dev",
        "name": "octos-dev",
        "username": "",
        "email": "",
    }))
    .await;
    let created_id = created["profile_id"].as_str().unwrap_or("octos-dev").to_owned();

    // 4. test + upsert with the wire key the panel sends (the recorded reply
    // is the real server verdict for this key — failure is recorded as such).
    let provision = json!({
        "profile_id": created_id,
        "selection": selection,
        "api_key": "octoscode-web-keyless-probe",
    });
    r(&client, &trace, "profile/llm/test", provision.clone()).await;
    r(&client, &trace, "profile/llm/upsert", json!({
        "profile_id": created_id,
        "selection": selection,
        "api_key": "octoscode-web-keyless-probe",
        "set_primary": true,
    }))
    .await;

    // Deterministic reset for the NEXT capture: drop the created profile so a
    // re-run records the same `created: true` shape. Untraced.
    let _ = reset_client
        .request("profile/llm/delete", json!({"profile_id": created_id}))
        .await;

    // Scrub → hermetic fixture.
    let raw = std::fs::read_to_string(&path).expect("read the raw trace");
    let scrubbed = scrub_machine_paths(&raw);
    std::fs::write(&path, scrubbed).expect("write the scrubbed fixture");
    let lines = std::fs::read_to_string(&path).expect("reread").lines().count();
    println!("[capture] fixture: {path} ({lines} frames)");
}
