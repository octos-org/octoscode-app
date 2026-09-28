//! F6 live integration against a real `octos serve` (`#[ignore]` by default).
//!
//! Run it explicitly, with MY serve already up on MY port (50160 — the F6 row
//! of `.peer/LESSONS.md` "Domain fan-out": six lanes run live tests at once,
//! so a lane uses only its own port + its own `tmp/` data dirs):
//!
//! ```sh
//! tmp/octos/octos serve --solo --port 50160 --host 127.0.0.1 \
//!   --auth-token f6-dummy-token --data-dir tmp/f6-data --instance-data-dir tmp/f6-data
//! ctest -p octoscode-client --test f6_peer_live -- --ignored --nocapture
//! ```
//!
//! The card's gate: `profile/local/create` → `session/open` → then every
//! **read-only** method in this lane's list. `peer/gather` is the read-only one
//! (it reads the profile's peer blackboard). `peer/prepare` is MUTATING (it
//! writes `peers/<slug>/brief.md`), so it runs only because both its data dir
//! and its `cwd` are throwaway `tmp/` paths, with `worktree: false` so no git
//! worktree is created. **No model turns.**
use std::time::Duration;

use octos_app_transport::{
    ws, Capabilities, ProfileId, SecretString, TransportConfig,
};
use octos_core::ui_protocol::SessionOpenParams;
use octoscode_client::domains::peer::{PeerGather, PeerGatherParams, PeerPrepare, PeerPrepareParams};
use octoscode_client::{Client, Method};
use serde::Deserialize;
use url::Url;

/// My own serve port (LESSONS "Domain fan-out" table).
fn base_url() -> String {
    std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50160".to_string())
}

/// A throwaway cwd for the (mutating) prepare call — inside the lane's tmp dir.
fn tmp_cwd() -> String {
    std::env::var("OCTOS_TMP_CWD").unwrap_or_else(|_| "tmp/f6-data/peer-ws".to_string())
}

/// `profile/local/create` params — onboard a profile on a fresh solo serve.
#[derive(serde::Serialize)]
struct CreateParams<'a> {
    requested_id: &'a str,
    name: &'a str,
    username: &'a str,
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

/// Bring up the real transport on a runtime thread; drain its events so the
/// channel never back-pressures; hand back the client.
async fn connect() -> Client {
    let cfg = TransportConfig {
        base_url: Url::parse(&base_url()).expect("OCTOS_BASE_URL parses"),
        bearer: SecretString::new(
            std::env::var("OCTOS_BEARER").unwrap_or_else(|_| "f6-dummy-token".to_string()),
        ),
        profile_id: ProfileId::new(
            std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "octoscode-f6".to_string()),
        ),
        cursor: None,
        cursor_file: None,
        requested_capabilities: Capabilities::requested(),
        workspace_cwd: None,
        local_kernel: false,
    };
    let (cmd_tx, mut evt_rx) = ws::spawn(cfg);
    tokio::spawn(async move { while evt_rx.recv().await.is_some() {} });
    Client::new(cmd_tx)
}

#[tokio::test]
#[ignore = "needs MY local octos serve on 50160; run with --ignored"]
async fn peer_methods_round_trip_against_a_live_serve() {
    let client = connect().await;
    let profile = format!("octoscodef6{}", std::process::id());

    // 1. profile/local/create — the precondition on a fresh solo serve.
    let created: CreateResult = serde_json::from_value(
        client
            .request(
                "profile/local/create",
                serde_json::to_value(CreateParams {
                    requested_id: &profile,
                    name: "OctosCode F6",
                    username: &profile,
                })
                .unwrap(),
            )
            .await
            .expect("profile/local/create"),
    )
    .expect("profile/local/create result");
    let profile_id = created.profile_id;
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
    let session_id = open.opened.session_id.clone();
    println!("session/open -> session_id={session_id}");

    // 3. peer/gather (READ-ONLY) — an empty blackboard on a fresh serve is fine.
    let gathered = client
        .call::<PeerGather>(PeerGatherParams {
            session_id: session_id.clone(),
            profile_id: profile_id.clone(),
            slugs: None,
        })
        .await
        .expect("peer/gather");
    println!(
        "peer/gather -> profile_id={} peers={}",
        gathered.profile_id,
        gathered.peers.len()
    );
    assert_eq!(gathered.profile_id, profile_id, "gather echoes the calling profile");

    // 4. peer/prepare (MUTATING, but throwaway-only): worktree=false and a cwd
    //    under my tmp dir, so the ONLY writes are the brief under the serve's
    //    own data dir (also my tmp) — no git worktree.
    let prepared = client
        .call::<PeerPrepare>(PeerPrepareParams {
            brief: "F6 live smoke: no-op peer, throwaway data dir only".into(),
            n: None,
            title: Some("f6 smoke".into()),
            names: None,
            worktree: Some(false),
            cwd: Some(tmp_cwd()),
            session_id: session_id.clone(),
            profile_id: profile_id.clone(),
        })
        .await
        .expect("peer/prepare");
    println!(
        "peer/prepare -> slug={} topic={} brief_path={} cwd={}",
        prepared.slug, prepared.topic, prepared.brief_path, prepared.cwd
    );
    assert_eq!(prepared.topic, format!("peer-{}", prepared.slug));
    assert_eq!(prepared.profile_id, profile_id);

    // 5. Re-gather: the staged peer must now appear on the blackboard.
    let after = client
        .call::<PeerGather>(PeerGatherParams {
            session_id: session_id.clone(),
            profile_id: profile_id.clone(),
            slugs: None,
        })
        .await
        .expect("peer/gather after prepare");
    println!("peer/gather (after) -> peers={}", after.peers.len());
    assert!(
        after.peers.iter().any(|row| row.slug == prepared.slug),
        "the staged peer appears on the blackboard"
    );

    // Small settle so the transport flushes before the test ends.
    tokio::time::sleep(Duration::from_millis(50)).await;
}
