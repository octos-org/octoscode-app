//! Live integration against a real `octos serve` (`#[ignore]` by default).
//!
//! Run it explicitly, with the serve already up:
//!
//! ```sh
//! tmp/octos-target/release/octos serve --solo --port 50082 --host 127.0.0.1 \
//!   --auth-token spike-dummy-token --data-dir <tmp> --instance-data-dir <tmp>
//! ctest -p octoscode-client --test live_serve -- --ignored --nocapture
//! ```
//!
//! The card's gate: `profile/local/create` → `session/open` → `session/list`
//! → `config/capabilities/list` → `tool/status/list`, every one through
//! [`Client::call`] (the production path). **No model turns.**
use std::future::Future;
use std::time::Duration;

use octos_app_transport::{
    ws, Capabilities, OutboundCommand, ProfileId, SecretString, TransportConfig,
};
use octos_core::ui_protocol::SessionOpenParams;
use octoscode_client::{Client, Method};
use serde::Deserialize;
use url::Url;

fn base_url() -> String {
    std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50082".to_string())
}

/// `profile/local/create` — onboard a profile on a fresh solo serve.
/// (Not a [`Method`]: it is in no matrix row; it is the *precondition* the D9
/// spikes established, and the web does it through its onboarding flow.)
struct ProfileLocalCreate;
#[derive(serde::Serialize)]
struct CreateParams {
    requested_id: String,
    name: String,
    username: String,
}
#[derive(Deserialize)]
struct CreateResult {
    profile_id: String,
}

/// `session/open` — via the generic path, to prove one `Method` covers both
/// lifecycle and extension calls.
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

/// `session/list`.
struct SessionList;
#[derive(Debug, Deserialize)]
struct SessionListResult {
    #[serde(default)]
    sessions: Vec<serde_json::Value>,
}
impl Method for SessionList {
    const NAME: &'static str = "session/list";
    type Params = serde_json::Value;
    type Result = SessionListResult;
}

/// `config/capabilities/list`.
struct CapabilitiesList;
#[derive(Debug, Deserialize)]
struct CapabilitiesResult {
    #[serde(default)]
    capabilities: ServerCaps,
}
#[derive(Debug, Default, Deserialize)]
struct ServerCaps {
    #[serde(default)]
    supported_methods: Vec<String>,
    #[serde(default)]
    supported_notifications: Vec<String>,
}
impl Method for CapabilitiesList {
    const NAME: &'static str = "config/capabilities/list";
    type Params = serde_json::Value;
    type Result = CapabilitiesResult;
}

/// `tool/status/list` (an AppUI extension method).
struct ToolStatusList;
#[derive(Debug, Deserialize)]
struct ToolStatusResult {
    #[serde(default)]
    tools: Vec<serde_json::Value>,
}
#[derive(serde::Serialize)]
struct ToolStatusParams {
    session_id: String,
    profile_id: String,
    include_denied: bool,
}
impl Method for ToolStatusList {
    const NAME: &'static str = "tool/status/list";
    type Params = ToolStatusParams;
    type Result = ToolStatusResult;
}

/// Bring up the real transport on a runtime thread; drain its events so the
/// channel never back-pressures; hand back the client.
async fn connect() -> Client {
    let cfg = TransportConfig {
        base_url: Url::parse(&base_url()).expect("OCTOS_BASE_URL parses"),
        bearer: SecretString::new(
            std::env::var("OCTOS_BEARER").unwrap_or_else(|_| "spike-dummy-token".to_string()),
        ),
        profile_id: ProfileId::new(
            std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "octoscode".to_string()),
        ),
        cursor: None,
        cursor_file: None,
        requested_capabilities: Capabilities::requested(),
        workspace_cwd: None,
        local_kernel: false,
    };
    let (cmd_tx, mut evt_rx) = ws::spawn(cfg);
    // Drain: the transport completes generic replies on its own oneshots, but
    // the event channel is bounded and must not fill.
    tokio::spawn(async move { while evt_rx.recv().await.is_some() {} });
    Client::new(cmd_tx)
}

/// Poll `f` until it is `Some` or `timeout` elapses.
async fn wait_for<T, F: FnMut() -> Option<T>>(
    mut f: F,
    timeout: Duration,
) -> Option<T> {
    let start = std::time::Instant::now();
    loop {
        if let Some(v) = f() {
            return Some(v);
        }
        if start.elapsed() > timeout {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
#[ignore = "needs a local octos serve; run with --ignored"]
async fn core_methods_round_trip_against_a_live_serve() {
    let client = connect().await;
    let profile = format!("octoscode{}", std::process::id());

    // 1. profile/local/create — the precondition on a fresh solo serve.
    let created = client
        .request(
            "profile/local/create",
            serde_json::json!({"requested_id": profile, "name": "OctosCode", "username": profile}),
        )
        .await
        .expect("profile/local/create");
    let profile_id = created["profile_id"].as_str().expect("profile_id").to_owned();
    println!("profile/local/create -> profile_id={profile_id}");

    // 2. session/open via Client::call (generic path).
    let open = client
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
    println!("session/open -> session_id={}", open.opened.session_id);

    // 3. session/list (needs auxiliary.rest_to_ws.v1, which `requested()` asks).
    let list = client
        .call::<SessionList>(serde_json::json!({}))
        .await
        .expect("session/list");
    println!("session/list -> {} sessions", list.sessions.len());

    // 4. config/capabilities/list (result is an OBJECT, not a bare list).
    let caps = client
        .call::<CapabilitiesList>(serde_json::json!({}))
        .await
        .expect("config/capabilities/list");
    println!(
        "config/capabilities/list -> {} methods, {} notifications",
        caps.capabilities.supported_methods.len(),
        caps.capabilities.supported_notifications.len()
    );
    assert!(
        !caps.capabilities.supported_methods.is_empty(),
        "a live serve advertises supported methods"
    );

    // 5. tool/status/list (AppUI extension).
    let tools = client
        .call::<ToolStatusList>(ToolStatusParams {
            session_id: open.opened.session_id.clone(),
            profile_id: profile_id.clone(),
            include_denied: true,
        })
        .await
        .expect("tool/status/list");
    println!("tool/status/list -> {} tools", tools.tools.len());

    // And leave: a clean disconnect.
    let _ = client.request("session/list", serde_json::json!({})).await;
}
