//! F5 live integration against a real `octos serve` (`#[ignore]` by default).
//!
//! Run it explicitly, with MY serve (port 50150) already up:
//!
//! ```sh
//! tmp/f5/octos serve --solo --port 50150 --host 127.0.0.1 \
//!   --auth-token f5-dummy-token --data-dir tmp/f5/data --instance-data-dir tmp/f5/instance
//! ctest -p octoscode-client --test f5_live -- --ignored --nocapture
//! ```
//!
//! Card gate: call every **read-only** method this lane added
//! (`turn/state/get`, `thread/graph/get`, `approval/scopes/list`) through the
//! production `Client::call` path after `profile/local/create` +
//! `session/open`. **No model turns.** Mutating methods (`turn/steer`,
//! `user_question/respond`, `review/start`) are NOT called live.
use octos_app_transport::{ws, Capabilities, ProfileId, SecretString, TransportConfig};
use octos_core::ui_protocol::{ApprovalScopesListParams, ThreadGraphGetParams, TurnId, TurnStateGetParams};
use octos_core::SessionKey;
use octoscode_client::domains::approval::ApprovalScopesList;
use octoscode_client::domains::turn::{ThreadGraphGet, TurnStateGet};
use octoscode_client::{Client, Method};
use serde::Deserialize;
use url::Url;

fn base_url() -> String {
    std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50150".to_string())
}

/// `profile/local/create` — onboard a profile on a fresh solo serve (the
/// precondition the D9 spikes established).
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
impl Method for ProfileLocalCreate {
    const NAME: &'static str = "profile/local/create";
    type Params = CreateParams;
    type Result = CreateResult;
}

/// `session/open`.
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
    type Params = octos_core::ui_protocol::SessionOpenParams;
    type Result = SessionOpenResult;
}

/// Bring up the real transport, drain its events, hand back the client.
///
/// `thread/graph/get` and `turn/state/get` are feature-gated server-side
/// (`method_capability_gate`, octos `ui_protocol.rs`) and enabled only when
/// the client requests `state.thread_graph.v1` / `state.turn_state_get.v1` in
/// the WS handshake (`x-octos-ui-features`). `Capabilities::requested()`
/// (the transport's default) does NOT include them, so this test adds them via
/// the public `raw` map — the same handshake path a real client uses. Without
/// this the solo serve answers `-32004 method not supported` for both.
/// (`approval/scopes/list` is ungated and needs no feature.)
async fn connect() -> Client {
    let mut requested = Capabilities::requested();
    requested
        .raw
        .insert("state.thread_graph.v1".to_owned(), serde_json::Value::Bool(true));
    requested
        .raw
        .insert("state.turn_state_get.v1".to_owned(), serde_json::Value::Bool(true));
    let cfg = TransportConfig {
        base_url: Url::parse(&base_url()).expect("OCTOS_BASE_URL parses"),
        bearer: SecretString::new(
            std::env::var("OCTOS_BEARER").unwrap_or_else(|_| "f5-dummy-token".to_string()),
        ),
        profile_id: ProfileId::new(
            std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "f5".to_string()),
        ),
        cursor: None,
        cursor_file: None,
        requested_capabilities: requested,
        workspace_cwd: None,
        local_kernel: false,
    };
    let (cmd_tx, mut evt_rx) = ws::spawn(cfg);
    tokio::spawn(async move { while evt_rx.recv().await.is_some() {} });
    Client::new(cmd_tx)
}

#[tokio::test]
#[ignore = "needs a local octos serve on 50150; run with --ignored"]
async fn f5_readonly_methods_round_trip_against_a_live_serve() {
    let client = connect().await;
    let profile = format!("f5{}", std::process::id());

    // Precondition 1: profile/local/create.
    let created = client
        .call::<ProfileLocalCreate>(CreateParams {
            requested_id: profile.clone(),
            name: "F5".to_string(),
            username: profile.clone(),
        })
        .await
        .expect("profile/local/create");
    let profile_id = created.profile_id.clone();
    println!("profile/local/create -> profile_id={profile_id}");

    // Precondition 2: session/open.
    let open = client
        .call::<SessionOpen>(octos_core::ui_protocol::SessionOpenParams {
            session_id: SessionKey::new(&profile_id, "main"),
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

    let key = SessionKey(session_id.clone());

    // Read-only 1: `thread/graph/get` (current head: `at` absent).
    let graph = client
        .call::<ThreadGraphGet>(ThreadGraphGetParams {
            session_id: key.clone(),
            at: None,
        })
        .await
        .expect("thread/graph/get");
    println!(
        "thread/graph/get -> {} threads, {} orphans, cursor seq {}",
        graph.threads.len(),
        graph.orphans.len(),
        graph.cursor.seq
    );
    assert_eq!(graph.session_id.0, session_id, "the graph answers for this session");

    // Read-only 2: `approval/scopes/list`.
    let scopes = client
        .call::<ApprovalScopesList>(ApprovalScopesListParams { session_id: key.clone() })
        .await
        .expect("approval/scopes/list");
    println!("approval/scopes/list -> {} scopes", scopes.scopes.len());

    // Read-only 3: `turn/state/get` for a turn that never ran (fresh session):
    // the server answers `unknown`, never an error — this is UPCR-2026-011's
    // whole point (a client can stop holding for a turn it lost).
    let turn = TurnId::new();
    let state = client
        .call::<TurnStateGet>(TurnStateGetParams {
            session_id: key.clone(),
            turn_id: turn.clone(),
        })
        .await
        .expect("turn/state/get");
    println!(
        "turn/state/get -> state={} running={:?} committed_seqs={}",
        state.state.as_str(),
        state.running,
        state.committed_seqs.len()
    );
    assert_eq!(state.turn_id.0, turn.0, "the state answers for the turn asked");

    // A clean end: one more read, then drop.
    let again = client
        .call::<ThreadGraphGet>(ThreadGraphGetParams {
            session_id: key.clone(),
            at: None,
        })
        .await
        .expect("thread/graph/get (repeat)");
    println!("thread/graph/get repeat -> {} threads", again.threads.len());

    // A clean end: the client drops, closing the connection.
    let _ = client.request("session/list", serde_json::json!({})).await;
}
