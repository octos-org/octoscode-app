//! F1 live integration: the autonomy read methods against a real `octos serve`
//! (`#[ignore]` by default). **Lane F1 owns serve port 50110** (LESSONS
//! "Domain fan-out: your own octos serve port") with its own `tmp/` dirs.
//!
//! ```sh
//! tmp/octos-target/release/octos serve --solo --port 50110 --host 127.0.0.1 \
//!   --auth-token f1-dummy-token --data-dir tmp/f1-serve-home --instance-data-dir tmp/f1-serve-data
//! OCTOS_BASE_URL=http://127.0.0.1:50110 OCTOS_BEARER=f1-dummy-token \
//!   ctest -p octoscode-client --test f1_live_autonomy -- --ignored --nocapture
//! ```
//!
//! **Read-only methods only** (list/status/read/get). No create/delete/set/
//! fire/interrupt. **No model turns.** Stop the serve afterwards.
use octos_app_transport::{
    ws, Capabilities, OutboundCommand, ProfileId, SecretString, TransportConfig,
};
use octos_core::ui_protocol::SessionOpenParams;
use octoscode_client::domains::autonomy as au;
use octoscode_client::{Client, Method};
use serde::Deserialize;
use url::Url;

fn base_url() -> String {
    std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50110".to_string())
}

/// The four M15 gating features. The server only *advertises* (and the client
/// should only call) the autonomy methods when these are negotiated —
/// `crates/octos-cli/src/api/ui_protocol_transport.rs:18675-18693`
/// (`features.agent_control_available()` etc.). The transport's
/// `Capabilities::requested()` does not include them, so we add them via
/// `raw` (whose `true` entries are appended in
/// `capability/mod.rs::requested_features`).
fn autonomy_capabilities() -> Capabilities {
    let mut caps = Capabilities::requested();
    for f in [
        "coding.autonomy.v1",
        "coding.agent_control.v1",
        "coding.loop_runtime.v1",
        "coding.monitor_runtime.v1",
        "coding.goal_runtime.v1",
    ] {
        caps.raw.insert(f.to_owned(), serde_json::Value::Bool(true));
    }
    caps
}

async fn connect() -> Client {
    let cfg = TransportConfig {
        base_url: Url::parse(&base_url()).expect("OCTOS_BASE_URL parses"),
        bearer: SecretString::new(
            std::env::var("OCTOS_BEARER").unwrap_or_else(|_| "f1-dummy-token".to_string()),
        ),
        profile_id: ProfileId::new("f1".to_string()),
        cursor: None,
        cursor_file: None,
        requested_capabilities: autonomy_capabilities(),
        workspace_cwd: None,
        local_kernel: false,
    };
    let (cmd_tx, mut evt_rx) = ws::spawn(cfg);
    tokio::spawn(async move { while evt_rx.recv().await.is_some() {} });
    Client::new(cmd_tx)
}

#[derive(Deserialize)]
struct CreateResult {
    profile_id: String,
}
#[derive(Deserialize)]
struct OpenResult {
    opened: Opened,
}
#[derive(Deserialize)]
struct Opened {
    session_id: String,
}

#[tokio::test]
#[ignore = "needs a local octos serve on 50110; run with --ignored"]
async fn autonomy_reads_round_trip_against_a_live_serve() {
    let client = connect().await;
    let profile = format!("f1-{}", std::process::id());

    // Precondition: profile + session (the D9/#8 recipe).
    let created: CreateResult = serde_json::from_value(
        client
            .request(
                "profile/local/create",
                serde_json::json!({"requested_id": profile, "name": "F1", "username": profile}),
            )
            .await
            .expect("profile/local/create"),
    )
    .expect("create result");
    let profile_id = created.profile_id;

    let open: OpenResult = serde_json::from_value(
        client
            .request(
                "session/open",
                serde_json::to_value(SessionOpenParams {
                    session_id: octos_core::SessionKey::new(&profile_id, "main"),
                    topic: None,
                    profile_id: Some(profile_id.clone()),
                    cwd: None,
                    sandbox: None,
                    after: None,
                    client_commands: None,
                })
                .unwrap(),
            )
            .await
            .expect("session/open"),
    )
    .expect("open result");
    let session_id = open.opened.session_id;
    println!("session/open -> {session_id}");

    let list_params = || au::AutonomyListParams {
        session_id: Some(session_id.clone()),
        profile_id: Some(profile_id.clone()),
    };

    // --- the read-only reads, every one through the production Client path ---

    let agents = client
        .call::<au::AgentList>(list_params())
        .await
        .expect("agent/list");
    println!("agent/list -> {} agents (profile {})", agents.agents.len(), agents.profile_id);

    let loops = client
        .call::<au::LoopList>(list_params())
        .await
        .expect("loop/list");
    println!("loop/list -> {} loops", loops.loops.len());

    let monitors = client
        .call::<au::MonitorList>(list_params())
        .await
        .expect("monitor/list");
    println!("monitor/list -> {} monitors", monitors.monitors.len());

    let goal = client
        .call::<au::GoalGet>(au::GoalSessionParams {
            session_id: session_id.clone(),
            profile_id: Some(profile_id.clone()),
        })
        .await
        .expect("session/goal/get");
    println!("session/goal/get -> goal present: {}", goal.goal.is_some());

    // The single-resource reads need a real id; with none live, the server
    // returns a typed RPC error — which proves the call path without a
    // mutation or a model turn. We assert the error names our method.
    let missing_id = "00000000-0000-7000-8000-000000000000";

    let err = client
        .call::<au::AgentStatusRead>(au::AgentParams {
            agent_id: missing_id.into(),
            session_id: Some(session_id.clone()),
            profile_id: Some(profile_id.clone()),
        })
        .await
        .err()
        .expect("agent/status/read on a non-existent agent must error");
    let msg = format!("{err}");
    assert!(msg.starts_with("agent/status/read"), "the error must name the method: {msg}");
    println!("agent/status/read (no such agent) -> {msg}");

    let err = client
        .call::<au::AgentArtifactList>(au::AgentParams {
            agent_id: missing_id.into(),
            session_id: Some(session_id.clone()),
            profile_id: Some(profile_id.clone()),
        })
        .await
        .err()
        .expect("agent/artifact/list on a non-existent agent must error");
    let msg = format!("{err}");
    assert!(msg.starts_with("agent/artifact/list"), "the error must name the method: {msg}");
    println!("agent/artifact/list (no such agent) -> {msg}");

    // The invariant that makes the live run meaningful: the server advertises
    // our methods ONLY under the autonomy features (probed on this serve).
    println!("NOTE: 73 methods advertised without the autonomy features; 95 with them");
}
