//! Card #F3 live integration against a real `octos serve` `a6ea8505`
//! (`#[ignore]` by default). **Only the read-only methods in this lane's list
//! are called** — `session/status/read`, `session/files.list`, `snapshot/list`,
//! `launch/resolve`. The mutating methods (`session/delete`, `session/fork`,
//! `session/rollback`, `session/compact`, `session/compact/mode/set`,
//! `snapshot/restore`, `server/shutdown`) and `session/btw` (which spends a
//! model call) are deliberately NOT called here.
//!
//! Run it with your own serve (lane port **50130**) per `.peer/LESSONS.md`
//! "Domain fan-out":
//!
//! ```sh
//! tmp/f3/octos serve --solo --port 50130 --host 127.0.0.1 \
//!   --auth-token f3-dummy-token --data-dir tmp/f3/home --instance-data-dir tmp/f3/home
//! ctest -p octoscode-client --test f3_live -- --ignored --nocapture
//! ```
//!
//! **No model turns.**
use std::time::Duration;

use octos_app_transport::{
    ws, Capabilities, OutboundCommand, ProfileId, SecretString, TransportConfig,
};
use octos_core::ui_protocol::SessionOpenParams;
use octoscode_client::{Client, Method};
use serde::Deserialize;
use url::Url;

fn base_url() -> String {
    std::env::var("F3_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50130".to_string())
}

/// `session/open` — via the generic path (it carries the replay bracket, so it
/// is not one of this crate's `Method`s).
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

async fn connect() -> Client {
    let cfg = TransportConfig {
        base_url: Url::parse(&base_url()).expect("F3_BASE_URL parses"),
        bearer: SecretString::new(
            std::env::var("F3_BEARER").unwrap_or_else(|_| "f3-dummy-token".to_string()),
        ),
        profile_id: ProfileId::new(
            std::env::var("F3_PROFILE_ID").unwrap_or_else(|_| "octoscode".to_string()),
        ),
        cursor: None,
        cursor_file: None,
        requested_capabilities: Capabilities::requested(),
        workspace_cwd: None,
        local_kernel: false,
    };
    let (cmd_tx, mut evt_rx) = ws::spawn(cfg);
    // The event channel is bounded and must not fill.
    tokio::spawn(async move { while evt_rx.recv().await.is_some() {} });
    Client::new(cmd_tx)
}

#[tokio::test]
#[ignore = "needs a local octos serve; run with --ignored"]
async fn f3_read_only_methods_round_trip_against_a_live_serve() {
    let client = connect().await;
    let profile = format!("f3{}", std::process::id());

    // 1. profile/local/create — precondition on a fresh solo serve.
    let created = client
        .request(
            "profile/local/create",
            serde_json::json!({"requested_id": profile, "name": "F3", "username": profile}),
        )
        .await
        .expect("profile/local/create");
    let profile_id = created["profile_id"].as_str().expect("profile_id").to_owned();
    println!("profile/local/create -> profile_id={profile_id}");

    // 2. session/open (generic path).
    //
    // The id MUST embed the profile in `{profile}:{channel}:{chat_id}` form:
    // every follow-up read routes the profile from the id ALONE. The web does
    // exactly this in `bindWebSessionIdToProfile` (`features/session/
    // session-identity.ts:22`, `${profile}:api:${id}`) precisely because
    // `session/hydrate` and `session/status/read` carry no `profile_id` param.
    // A 2-segment id falls back to `_main` and the read fails with
    // `profile_unresolved` (verified live).
    let open = client
        .call::<SessionOpen>(SessionOpenParams {
            session_id: octos_core::SessionKey::with_profile(&profile_id, "api", "main"),
            topic: None,
            profile_id: Some(profile_id.clone()),
            cwd: None,
            sandbox: None,
            after: None,
            client_commands: None,
        })
        .await
        .expect("session/open");
    let session_id = open.opened.session_id;
    println!("session/open -> session_id={session_id}");

    // 3. launch/resolve — the pre-session probe, read-only.
    let launch = client
        .call::<octoscode_client::domains::config::LaunchResolve>(
            octos_core::ui_protocol::LaunchResolveParams {
                cwd: std::env::temp_dir().to_string_lossy().into_owned(),
                profile_id: Some(profile_id.clone()),
            },
        )
        .await
        .expect("launch/resolve");
    println!(
        "launch/resolve -> decision={:?} resolved_profile={:?}",
        launch.decision, launch.resolved_profile
    );

    // 4. session/status/read — the status-pill poller, read-only.
    let status = client
        .call::<octoscode_client::domains::session::SessionStatusRead>(
            octoscode_client::domains::session::SessionStatusReadParams {
                session_id: session_id.clone(),
            },
        )
        .await
        .expect("session/status/read");
    assert_eq!(status.session_id, session_id, "status names the opened session");
    println!(
        "session/status/read -> profile_id={:?} has_context_state={}",
        status.profile_id,
        status.context_state.is_some()
    );

    // 5. session/files.list — read-only.
    let files = client
        .call::<octoscode_client::domains::session::SessionFilesList>(
            octos_core::ui_protocol::SessionFilesListParams {
                session_id: session_id.clone(),
            },
        )
        .await
        .expect("session/files.list");
    println!("session/files.list -> files={}", files.files);

    // 6. snapshot/list — read-only (snapshots are opt-in; disabled is a
    //    valid, well-formed answer, which is what a fresh home returns).
    let snaps = client
        .call::<octoscode_client::domains::config::SnapshotListMethod>(
            octoscode_client::domains::config::SnapshotListParams {
                session_id: session_id.clone(),
            },
        )
        .await
        .expect("snapshot/list");
    println!(
        "snapshot/list -> enabled={} available={} count={}",
        snaps.enabled,
        snaps.available,
        snaps.snapshots.len()
    );

    // Keep the runtime alive until the prints flush.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let _ = OutboundCommand::Disconnect;
    println!("F3 live read-only round-trips: OK");
}
