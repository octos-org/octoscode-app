//! [F4] Live integration against a real `octos serve` (`#[ignore]` by default).
//!
//! Run it explicitly, against **this lane's own** serve on port 50140 (LESSONS
//! "Domain fan-out: your own `octos serve` port"), with a throwaway data dir:
//!
//! ```sh
//! tmp/octos serve --solo --port 50140 --host 127.0.0.1 \
//!   --auth-token spike-dummy-token --data-dir tmp/f4-data --instance-data-dir tmp/f4-data
//! OCTOS_BASE_URL=http://127.0.0.1:50140 \
//!   ctest -p octoscode-client --test live_f4 -- --ignored --nocapture
//! ```
//!
//! **Read-only only:** `task/list`, `task/artifact/list`, `task/artifact/read`
//! and `mcp/status/list`. `task/cancel` is **mutating** and is deliberately NOT
//! called live (its wire round-trip is covered by `tests/f4_task_tool.rs`).
//! **No model turns.**
use std::time::Duration;

use octos_app_transport::{
    ws, Capabilities, ProfileId, SecretString, TransportConfig,
};
use octos_core::ui_protocol::SessionOpenParams;
use octoscode_client::domains::task::{TaskArtifactList, TaskArtifactRead, TaskList};
use octoscode_client::domains::tool::{McpStatusList, McpStatusListParams};
use octoscode_client::{Client, ClientError, Method};
use serde::Deserialize;
use url::Url;

fn base_url() -> String {
    // This lane's port is 50140 (LESSONS "Domain fan-out" table).
    std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50140".to_string())
}

/// A fixed, syntactically valid task UUID for the artifact calls when the fresh
/// serve has no tasks (the calls then prove the wire path and the typed error).
const SYNTHETIC_TASK: &str = "00000000-0000-7000-8000-0000000000f4";

/// `session/open` via the generic path.
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
    tokio::spawn(async move { while evt_rx.recv().await.is_some() {} });
    // Give the socket a moment to reach Live before the first request.
    tokio::time::sleep(Duration::from_millis(300)).await;
    Client::new(cmd_tx)
}

#[tokio::test]
#[ignore = "needs a local octos serve on 50140; run with --ignored"]
async fn f4_read_only_methods_round_trip_against_a_live_serve() {
    let client = connect().await;
    let profile = format!("octoscode{}", std::process::id());

    // Precondition on a fresh solo serve.
    let created = client
        .request(
            "profile/local/create",
            serde_json::json!({"requested_id": profile, "name": "OctosCode", "username": profile}),
        )
        .await
        .expect("profile/local/create");
    let profile_id = created["profile_id"].as_str().expect("profile_id").to_owned();
    println!("profile/local/create -> profile_id={profile_id}");

    // A session to scope the task calls to.
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
    let session_id = open.opened.session_id.clone();
    println!("session/open -> session_id={session_id}");

    // ---- task/list (read-only) -------------------------------------------
    let listed = client
        .call::<TaskList>(octos_core::ui_protocol::TaskListParams {
            session_id: octos_core::SessionKey(session_id.clone()),
            topic: None,
        })
        .await
        .expect("task/list round trip");
    println!(
        "task/list -> {} tasks (session={})",
        listed.tasks.len(),
        listed.session_id.0
    );
    // The first real task id, if any, drives the artifact calls.
    let real_task = listed.tasks.first().map(|t| t.id.0.to_string());
    let task_id = real_task.clone().unwrap_or_else(|| SYNTHETIC_TASK.to_owned());

    // ---- task/artifact/list (read-only) ----------------------------------
    match client
        .call::<TaskArtifactList>(octos_core::ui_protocol::TaskArtifactListParams {
            session_id: octos_core::SessionKey(session_id.clone()),
            task_id: task_id.parse().expect("task uuid"),
            profile_id: Some(profile_id.clone()),
            agent_id: None,
        })
        .await
    {
        Ok(r) => println!("task/artifact/list -> {} artifacts", r.artifacts.len()),
        Err(ClientError::Rpc { method, error }) => {
            println!("task/artifact/list -> typed rpc error {method}: {}", error.message)
        }
        Err(other) => panic!("task/artifact/list unexpected: {other:?}"),
    }

    // ---- task/artifact/read (read-only) ----------------------------------
    match client
        .call::<TaskArtifactRead>(octos_core::ui_protocol::TaskArtifactReadParams {
            session_id: octos_core::SessionKey(session_id.clone()),
            task_id: task_id.parse().expect("task uuid"),
            artifact_id: Some("art-1".into()),
            path: None,
            cursor: None,
            limit_bytes: Some(262_144),
            profile_id: Some(profile_id.clone()),
            agent_id: None,
        })
        .await
    {
        Ok(r) => println!(
            "task/artifact/read -> {} bytes (has_more={})",
            r.content.as_deref().map(str::len).unwrap_or(0),
            r.has_more
        ),
        Err(ClientError::Rpc { method, error }) => {
            println!("task/artifact/read -> typed rpc error {method}: {}", error.message)
        }
        Err(other) => panic!("task/artifact/read unexpected: {other:?}"),
    }

    // ---- mcp/status/list (read-only AppUI extension) ---------------------
    match client
        .call::<McpStatusList>(McpStatusListParams {
            session_id: session_id.clone(),
            profile_id: profile_id.clone(),
            include_disabled: true,
        })
        .await
    {
        Ok(r) => println!(
            "mcp/status/list -> {} servers (connected={})",
            r.servers.len(),
            r.summary.connected
        ),
        Err(ClientError::Rpc { method, error }) => {
            println!("mcp/status/list -> typed rpc error {method}: {}", error.message)
        }
        Err(other) => panic!("mcp/status/list unexpected: {other:?}"),
    }

    // task/cancel is mutating: intentionally NOT called live.
    println!("task/cancel -> skipped (mutating; covered by tests/f4_task_tool.rs)");

    // A clean teardown: drop the command channel so the transport task exits.
    drop(client);
    println!("done: read-only F4 methods round-tripped against the live serve");
}
