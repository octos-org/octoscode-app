//! #F2 live check against a real `octos serve` (`#[ignore]` by default).
//!
//! Read-only methods only, no model turns (RULES: no model/API spend). Bring up
//! my OWN serve on port **50120** with throwaway `tmp/` data dirs (LESSONS
//! "Domain fan-out: your own octos serve port"), then:
//!
//! ```sh
//! cp <WORKSPACE>/p0-build/tmp/octos-target/release/octos tmp/f2/octos
//! tmp/f2/octos serve --solo --port 50120 --host 127.0.0.1 \
//!   --auth-token spike-dummy-token --data-dir tmp/f2/data --instance-data-dir tmp/f2/instance
//! OCTOS_BASE_URL=http://127.0.0.1:50120 \
//!   ctest -p octoscode-client --test f2_live -- --ignored --nocapture
//! ```
//!
//! Then stop THAT serve (never another lane's).
//!
//! Read-only set exercised: `permission/profile/list`, `onboarding/workspace_list`,
//! `profile/llm/catalog`, `profile/llm/list`, `profile/skills/list`,
//! `profile/sub_providers/list`. **Excluded as non-local / mutating:**
//! `profile/skills/registry/search` (fetches the public registry over HTTP —
//! `commands/skills.rs:852` → `fetch_registry_from`), `profile/llm/test` and
//! `profile/llm/fetch_models` (provider network calls), and every mutating
//! method (`*/upsert`, `*/install`, `*/remove`, `*/delete`, `permission/profile/set`,
//! `onboarding/workspace_create`).
use std::time::Duration;

use octos_app_transport::{
    ws, Capabilities, ProfileId, SecretString, TransportConfig,
};
use octos_core::ui_protocol::SessionOpenParams;
use octoscode_client::domains::profile::{
    LlmCatalog, LlmCatalogParams, PermissionProfileList, ProfileLlmList, ProfileLlmListParams,
    SkillsList, SkillsListParams, SubProvidersList, SubProvidersListParams, WorkspaceList,
    WorkspaceListParams,
};
use octoscode_client::{Client, Method};
use url::Url;

fn base_url() -> String {
    std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50120".to_string())
}

/// `session/open` via the generic path (mirrors `live_serve.rs`).
struct SessionOpen;
#[derive(serde::Deserialize)]
struct SessionOpenResult {
    opened: Opened,
}
#[derive(serde::Deserialize)]
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
    Client::new(cmd_tx)
}

#[tokio::test]
#[ignore = "needs a local octos serve on :50120; run with --ignored"]
async fn f2_read_only_methods_round_trip_against_a_live_serve() {
    let client = connect().await;
    // The transport connects asynchronously; give it a moment to go Live.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let profile = format!("octoscode-f2-{}", std::process::id());

    // 0. profile/local/create + session/open — the live preconditions.
    let created = client
        .request(
            "profile/local/create",
            serde_json::json!({"requested_id": profile, "name": "OctosCode F2", "username": profile}),
        )
        .await
        .expect("profile/local/create");
    let profile_id = created["profile_id"].as_str().expect("profile_id").to_owned();
    println!("profile/local/create -> profile_id={profile_id}");

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

    // 1. permission/profile/list (core, typed from octos-core).
    let perms = client
        .call::<PermissionProfileList>(
            octos_core::ui_protocol::PermissionProfileListParams {
                session_id: octos_core::SessionKey(session_id.clone()),
            },
        )
        .await
        .expect("permission/profile/list");
    println!(
        "permission/profile/list -> current={:?}/{:?}, {} profiles",
        perms.current.mode,
        perms.current.network,
        perms.profiles.len()
    );
    assert_eq!(perms.session_id.0, session_id);

    // 2. onboarding/workspace_list (AppUI ext; read-only). path=null = server cwd.
    let ws = client
        .call::<WorkspaceList>(WorkspaceListParams { path: None })
        .await
        .expect("onboarding/workspace_list");
    println!(
        "onboarding/workspace_list -> canonical={}, {} entries, writable={}",
        ws.canonical_path,
        ws.entries.len(),
        ws.writable
    );

    // 3. profile/llm/catalog (read-only).
    let catalog = client
        .call::<LlmCatalog>(LlmCatalogParams {})
        .await
        .expect("profile/llm/catalog");
    println!("profile/llm/catalog -> {} families", catalog.families.len());
    assert!(
        !catalog.families.is_empty(),
        "a live serve ships a canonical model catalog"
    );

    // 4. profile/llm/list (profile-scoped read: { profile_id }).
    let llm = client
        .call::<ProfileLlmList>(ProfileLlmListParams {
            session_id: None,
            profile_id: Some(profile_id.clone()),
        })
        .await
        .expect("profile/llm/list");
    println!(
        "profile/llm/list -> profile_id={:?}, primary={}, fallbacks={}",
        llm.profile_id,
        llm.primary.is_some(),
        llm.fallbacks.len()
    );

    // 5. profile/skills/list (read-only).
    let skills = client
        .call::<SkillsList>(SkillsListParams {
            profile_id: Some(profile_id.clone()),
        })
        .await
        .expect("profile/skills/list");
    println!(
        "profile/skills/list -> {} skills (count field {})",
        skills.skills.len(),
        skills.count
    );

    // 6. profile/sub_providers/list (read-only).
    let lanes = client
        .call::<SubProvidersList>(SubProvidersListParams {
            profile_id: Some(profile_id.clone()),
        })
        .await
        .expect("profile/sub_providers/list");
    println!(
        "profile/sub_providers/list -> {} lanes",
        lanes.sub_providers.len()
    );

    // Leave cleanly.
    let _ = client.request("session/list", serde_json::json!({})).await;
}
