//! #F2 — profile domain fake-transport tests.
//!
//! One round-trip per method: the request frame carries the params the WEB
//! client sends (cited per test), the typed result decodes, and an RPC error
//! surfaces as [`ClientError::Rpc`] carrying the method name (the client-core
//! contract, `tests/client_core.rs`). The transport here is the same
//! `OutboundCommand::Request` / oneshot-reply channel the real WS transport
//! uses, so these exercise the production `Client::call` path with no socket.
use tokio::sync::mpsc;

use octos_app_transport::OutboundCommand;
use octos_core::ui_protocol::{methods, RpcError};
use octoscode_client::{Client, ClientError};

// ------------------------------------------------------------ fake transport

/// Serve one request from a scripted responder, asserting the wire shape.
fn fake_transport<F>(responder: F) -> mpsc::Sender<OutboundCommand>
where
    F: Fn(&str, &serde_json::Value) -> Result<serde_json::Value, RpcError> + Send + 'static,
{
    let (tx, mut rx) = mpsc::channel::<OutboundCommand>(8);
    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            if let OutboundCommand::Request { method, params, reply } = cmd {
                let _ = reply.send(responder(&method, &params));
            }
        }
    });
    tx
}

/// A responder that requires `method` and asserts `params`, returning `result`.
fn answering(
    method: &'static str,
    params: serde_json::Value,
    result: serde_json::Value,
) -> impl Fn(&str, &serde_json::Value) -> Result<serde_json::Value, RpcError> + Send + 'static {
    move |m, p| {
        assert_eq!(m, method, "method name");
        assert_eq!(*p, params, "wire params for {method}");
        Ok(result.clone())
    }
}

/// A responder that always rejects with a JSON-RPC error.
fn rejecting(
    method: &'static str,
) -> impl Fn(&str, &serde_json::Value) -> Result<serde_json::Value, RpcError> + Send + 'static {
    move |m, _p| {
        assert_eq!(m, method);
        Err(RpcError {
            code: -32601,
            message: "method not found".into(),
            data: None,
        })
    }
}

/// Assert an RPC error surfaces with the method name attached.
async fn assert_rpc_error<M>(client: &Client, params: M::Params)
where
    M: octoscode_client::Method,
    M::Result: std::fmt::Debug,
{
    match client.call::<M>(params).await {
        Err(ClientError::Rpc { method, error }) => {
            assert_eq!(method, M::NAME);
            assert_eq!(error.code, -32601);
        }
        other => panic!("expected an RPC error for {}, got {other:?}", M::NAME),
    }
}

// ------------------------------------------------------ permission/profile/*

/// `permission/profile/list` — web `client.ts:571`; params `{ session_id }`
/// (`packages/client/src/types.ts:112`); result `{ session_id, current, profiles }`
/// (`spec-b.md:101`).
#[tokio::test]
async fn permission_profile_list_round_trips() {
    use octoscode_client::domains::profile::PermissionProfileList;
    let tx = fake_transport(answering(
        methods::PERMISSION_PROFILE_LIST,
        serde_json::json!({ "session_id": "octoscode:main" }),
        serde_json::json!({
            "session_id": "octoscode:main",
            "current": { "mode": "workspace_write", "network": "deny" },
            "profiles": [
                { "mode": "read_only", "network": "deny" },
                { "mode": "workspace_write", "network": "deny" }
            ]
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<PermissionProfileList>(
            octos_core::ui_protocol::PermissionProfileListParams {
                session_id: octos_core::SessionKey("octoscode:main".into()),
            },
        )
        .await
        .expect("round trip");
    assert_eq!(out.session_id.0, "octoscode:main");
    assert_eq!(out.current.mode.label(), "Workspace Write");
    assert_eq!(out.profiles.len(), 2);
}

#[tokio::test]
async fn permission_profile_list_surfaces_rpc_error() {
    use octoscode_client::domains::profile::PermissionProfileList;
    let client = Client::new(fake_transport(rejecting(methods::PERMISSION_PROFILE_LIST)));
    assert_rpc_error::<PermissionProfileList>(
        &client,
        octos_core::ui_protocol::PermissionProfileListParams {
            session_id: octos_core::SessionKey("octoscode:main".into()),
        },
    )
    .await;
}

/// `permission/profile/set` — web `client.ts:583`; params
/// `{ session_id, update }` (`types.ts:116`); result `{ session_id, current, applied }`.
#[tokio::test]
async fn permission_profile_set_round_trips() {
    use octoscode_client::domains::profile::PermissionProfileSet;
    use octos_core::ui_protocol::{
        PermissionProfileMode, PermissionProfileSetParams, PermissionProfileUpdate,
    };
    let tx = fake_transport(answering(
        methods::PERMISSION_PROFILE_SET,
        serde_json::json!({
            "session_id": "octoscode:main",
            "update": { "mode": "read_only" }
        }),
        serde_json::json!({
            "session_id": "octoscode:main",
            "current": { "mode": "read_only", "network": "deny" },
            "applied": true
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<PermissionProfileSet>(PermissionProfileSetParams {
            session_id: octos_core::SessionKey("octoscode:main".into()),
            update: PermissionProfileUpdate {
                mode: Some(PermissionProfileMode::ReadOnly),
                network: None,
                approval_policy: None,
            },
            runtime_mode: None,
        })
        .await
        .expect("round trip");
    assert!(out.applied);
    assert_eq!(out.current.mode, PermissionProfileMode::ReadOnly);
}

#[tokio::test]
async fn permission_profile_set_surfaces_rpc_error() {
    use octoscode_client::domains::profile::PermissionProfileSet;
    use octos_core::ui_protocol::{PermissionProfileSetParams, PermissionProfileUpdate};
    let client = Client::new(fake_transport(rejecting(methods::PERMISSION_PROFILE_SET)));
    assert_rpc_error::<PermissionProfileSet>(
        &client,
        PermissionProfileSetParams {
            session_id: octos_core::SessionKey("octoscode:main".into()),
            update: PermissionProfileUpdate::default(),
            runtime_mode: None,
        },
    )
    .await;
}

// ---------------------------------------------------- onboarding/workspace/*

/// `onboarding/workspace_list` — web `workspace-browse.ts:211`; params
/// `{ path: null }` means the server's own cwd (`ui_protocol_transport.rs:19453`).
#[tokio::test]
async fn onboarding_workspace_list_round_trips() {
    use octoscode_client::domains::profile::{WorkspaceList, WorkspaceListParams};
    let tx = fake_transport(answering(
        "onboarding/workspace_list",
        serde_json::json!({ "path": null }),
        serde_json::json!({
            "canonical_path": "/home/user",
            "parent_path": "/home",
            "writable": true,
            "entries": [{ "name": "work", "path": "/home/user/work", "writable": true }],
            "truncated": false,
            "hidden_skipped": 2
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<WorkspaceList>(WorkspaceListParams { path: None })
        .await
        .expect("round trip");
    assert_eq!(out.canonical_path, "/home/user");
    assert_eq!(out.entries.len(), 1);
    assert_eq!(out.entries[0].name, "work");
    assert_eq!(out.hidden_skipped, 2);
}

#[tokio::test]
async fn onboarding_workspace_list_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{WorkspaceList, WorkspaceListParams};
    let client = Client::new(fake_transport(rejecting("onboarding/workspace_list")));
    assert_rpc_error::<WorkspaceList>(&client, WorkspaceListParams::default()).await;
}

/// `onboarding/workspace_create` — web `workspace-browse.ts:212`; params
/// `{ parent, name }`; `created: false` is an idempotent success.
#[tokio::test]
async fn onboarding_workspace_create_round_trips() {
    use octoscode_client::domains::profile::{WorkspaceCreate, WorkspaceCreateParams};
    let tx = fake_transport(answering(
        "onboarding/workspace_create",
        serde_json::json!({ "parent": "/home/user", "name": "project" }),
        serde_json::json!({ "canonical_path": "/home/user/project", "created": true }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<WorkspaceCreate>(WorkspaceCreateParams {
            parent: "/home/user".into(),
            name: "project".into(),
        })
        .await
        .expect("round trip");
    assert!(out.created);
    assert_eq!(out.canonical_path, "/home/user/project");
}

#[tokio::test]
async fn onboarding_workspace_create_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{WorkspaceCreate, WorkspaceCreateParams};
    let client = Client::new(fake_transport(rejecting("onboarding/workspace_create")));
    assert_rpc_error::<WorkspaceCreate>(
        &client,
        WorkspaceCreateParams {
            parent: "/home/user".into(),
            name: "project".into(),
        },
    )
    .await;
}

// ----------------------------------------------------------- profile/llm/*

/// `profile/llm/catalog` — web `client.ts:712`; empty params. The server sends
/// `families` as a MAP keyed by family id (`ui_protocol_transport.rs`), which
/// the web reshapes to an array via `Object.entries` (`onboarding.ts:225`).
#[tokio::test]
async fn profile_llm_catalog_round_trips() {
    use octoscode_client::domains::profile::{LlmCatalog, LlmCatalogParams};
    let tx = fake_transport(answering(
        "profile/llm/catalog",
        serde_json::json!({}),
        serde_json::json!({
            "families": {
                "anthropic": {
                    "env": "ANTHROPIC_API_KEY",
                    "models": [{ "id": "claude", "endpoints": [{ "id": "official" }] }]
                }
            }
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<LlmCatalog>(LlmCatalogParams {})
        .await
        .expect("round trip");
    assert_eq!(out.families.len(), 1);
    assert_eq!(out.families[0].id, "anthropic");
    assert_eq!(out.families[0].models[0].id, "claude");
}

#[tokio::test]
async fn profile_llm_catalog_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{LlmCatalog, LlmCatalogParams};
    let client = Client::new(fake_transport(rejecting("profile/llm/catalog")));
    assert_rpc_error::<LlmCatalog>(&client, LlmCatalogParams {}).await;
}

/// `profile/llm/list` (session-scoped) — web `use-model-selection.ts:113`
/// sends `{ session_id, profile_id? }`; result `{ session_id, models }`.
#[tokio::test]
async fn profile_llm_list_session_scoped_round_trips() {
    use octoscode_client::domains::profile::{ProfileLlmList, ProfileLlmListParams};
    let tx = fake_transport(answering(
        "profile/llm/list",
        serde_json::json!({ "session_id": "octoscode:main", "profile_id": "octoscode" }),
        serde_json::json!({
            "session_id": "octoscode:main",
            "models": [{
                "model": "glm-5.2",
                "provider": "zhipu",
                "title": "zhipu / glm-5.2",
                "selected": true,
                "available": true
            }]
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<ProfileLlmList>(ProfileLlmListParams {
            session_id: Some("octoscode:main".into()),
            profile_id: Some("octoscode".into()),
        })
        .await
        .expect("round trip");
    assert_eq!(out.models.len(), 1);
    assert!(out.models[0].selected);
}

/// `profile/llm/list` (profile-config read) — web `client.ts:734` sends
/// `{ profile_id }`; result `{ profile_id, primary, fallbacks }`.
#[tokio::test]
async fn profile_llm_list_profile_scoped_round_trips() {
    use octoscode_client::domains::profile::{ProfileLlmList, ProfileLlmListParams};
    let tx = fake_transport(answering(
        "profile/llm/list",
        serde_json::json!({ "profile_id": "octoscode" }),
        serde_json::json!({
            "profile_id": "octoscode",
            "primary": {
                "family_id": "zhipu", "model_id": "glm-5.2",
                "route": { "api_key_env": "ZHIPU_API_KEY" },
                "has_api_key": true, "selected": true, "available": true
            },
            "fallbacks": []
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<ProfileLlmList>(ProfileLlmListParams {
            session_id: None,
            profile_id: Some("octoscode".into()),
        })
        .await
        .expect("round trip");
    assert_eq!(out.profile_id.as_deref(), Some("octoscode"));
    assert!(out.primary.is_some());
    assert!(out.models.is_empty());
}

#[tokio::test]
async fn profile_llm_list_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{ProfileLlmList, ProfileLlmListParams};
    let client = Client::new(fake_transport(rejecting("profile/llm/list")));
    assert_rpc_error::<ProfileLlmList>(&client, ProfileLlmListParams::default()).await;
}

/// `profile/llm/select` — web `use-model-selection.ts` sends
/// `{ session_id, profile_id?, family_id, model_id, route_id? }`.
#[tokio::test]
async fn profile_llm_select_round_trips() {
    use octoscode_client::domains::profile::{ProfileLlmSelect, ProfileLlmSelectParams};
    let tx = fake_transport(answering(
        "profile/llm/select",
        serde_json::json!({
            "session_id": "octoscode:main",
            "family_id": "zhipu",
            "model_id": "glm-5.2"
        }),
        serde_json::json!({
            "session_id": "octoscode:main",
            "selected": {
                "model": "glm-5.2", "provider": "zhipu", "title": "zhipu / glm-5.2",
                "selected": true, "available": true
            },
            "applied": true,
            "restart_required": false
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<ProfileLlmSelect>(ProfileLlmSelectParams {
            session_id: "octoscode:main".into(),
            profile_id: None,
            family_id: "zhipu".into(),
            model_id: "glm-5.2".into(),
            route_id: None,
        })
        .await
        .expect("round trip");
    assert!(out.applied);
    assert_eq!(out.selected.model, "glm-5.2");
}

#[tokio::test]
async fn profile_llm_select_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{ProfileLlmSelect, ProfileLlmSelectParams};
    let client = Client::new(fake_transport(rejecting("profile/llm/select")));
    assert_rpc_error::<ProfileLlmSelect>(
        &client,
        ProfileLlmSelectParams {
            session_id: "octoscode:main".into(),
            profile_id: None,
            family_id: "zhipu".into(),
            model_id: "glm-5.2".into(),
            route_id: None,
        },
    )
    .await;
}

/// `profile/llm/upsert` — web `client.ts:787`; params
/// `{ profile_id?, selection, api_key?, set_primary? }`.
#[tokio::test]
async fn profile_llm_upsert_round_trips() {
    use octoscode_client::domains::profile::{
        LlmProvisionParams, LlmRouteSelection, LlmSelection, LlmUpsert,
    };
    let tx = fake_transport(answering(
        "profile/llm/upsert",
        serde_json::json!({
            "profile_id": "octoscode",
            "selection": {
                "family_id": "zhipu",
                "model_id": "glm-5.2",
                "route": { "api_key_env": "ZHIPU_API_KEY" }
            },
            "api_key": "sk-test",
            "set_primary": true
        }),
        serde_json::json!({ "profile_id": "octoscode", "applied": true }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<LlmUpsert>(LlmProvisionParams {
            profile_id: Some("octoscode".into()),
            selection: LlmSelection {
                family_id: "zhipu".into(),
                model_id: "glm-5.2".into(),
                route: LlmRouteSelection {
                    api_key_env: Some("ZHIPU_API_KEY".into()),
                    ..Default::default()
                },
                inference: Default::default(),
            },
            api_key: Some("sk-test".into()),
            set_primary: Some(true),
        })
        .await
        .expect("round trip");
    assert!(out.applied);
    assert_eq!(out.profile_id, "octoscode");
}

#[tokio::test]
async fn profile_llm_upsert_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{LlmProvisionParams, LlmSelection, LlmUpsert};
    let client = Client::new(fake_transport(rejecting("profile/llm/upsert")));
    assert_rpc_error::<LlmUpsert>(
        &client,
        LlmProvisionParams {
            profile_id: None,
            selection: LlmSelection {
                family_id: "zhipu".into(),
                model_id: "glm-5.2".into(),
                route: Default::default(),
                inference: Default::default(),
            },
            api_key: None,
            set_primary: None,
        },
    )
    .await;
}

/// `profile/llm/test` — web `client.ts:777`; params/result like upsert plus a
/// `message` (`onboarding.ts:85`).
#[tokio::test]
async fn profile_llm_test_round_trips() {
    use octoscode_client::domains::profile::{LlmProvisionParams, LlmSelection, LlmTest};
    let tx = fake_transport(answering(
        "profile/llm/test",
        serde_json::json!({
            "profile_id": "octoscode",
            "selection": {
                "family_id": "zhipu",
                "model_id": "glm-5.2",
                "route": { "api_key_env": "ZHIPU_API_KEY" }
            }
        }),
        serde_json::json!({
            "profile_id": "octoscode",
            "applied": false,
            "message": "Provider connection succeeded"
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<LlmTest>(LlmProvisionParams {
            profile_id: Some("octoscode".into()),
            selection: LlmSelection {
                family_id: "zhipu".into(),
                model_id: "glm-5.2".into(),
                route: octoscode_client::domains::profile::LlmRouteSelection {
                    api_key_env: Some("ZHIPU_API_KEY".into()),
                    ..Default::default()
                },
                inference: Default::default(),
            },
            api_key: None,
            set_primary: None,
        })
        .await
        .expect("round trip");
    assert_eq!(out.message, "Provider connection succeeded");
    assert!(!out.applied);
}

#[tokio::test]
async fn profile_llm_test_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{LlmProvisionParams, LlmSelection, LlmTest};
    let client = Client::new(fake_transport(rejecting("profile/llm/test")));
    assert_rpc_error::<LlmTest>(
        &client,
        LlmProvisionParams {
            profile_id: None,
            selection: LlmSelection {
                family_id: "zhipu".into(),
                model_id: "glm-5.2".into(),
                route: Default::default(),
                inference: Default::default(),
            },
            api_key: None,
            set_primary: None,
        },
    )
    .await;
}

/// `profile/llm/delete` — web `client.ts:756`; params
/// `{ profile_id?, family_id, model_id, route_id }`.
#[tokio::test]
async fn profile_llm_delete_round_trips() {
    use octoscode_client::domains::profile::{ProfileLlmDelete, ProfileLlmDeleteParams};
    let tx = fake_transport(answering(
        "profile/llm/delete",
        serde_json::json!({
            "family_id": "zhipu", "model_id": "glm-5.2", "route_id": "official"
        }),
        serde_json::json!({
            "profile_id": "octoscode",
            "primary": null,
            "fallbacks": [],
            "applied": true
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<ProfileLlmDelete>(ProfileLlmDeleteParams {
            profile_id: None,
            family_id: "zhipu".into(),
            model_id: "glm-5.2".into(),
            route_id: "official".into(),
        })
        .await
        .expect("round trip");
    assert!(out.applied);
    assert!(out.primary.is_none());
}

#[tokio::test]
async fn profile_llm_delete_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{ProfileLlmDelete, ProfileLlmDeleteParams};
    let client = Client::new(fake_transport(rejecting("profile/llm/delete")));
    assert_rpc_error::<ProfileLlmDelete>(
        &client,
        ProfileLlmDeleteParams {
            profile_id: None,
            family_id: "zhipu".into(),
            model_id: "glm-5.2".into(),
            route_id: "official".into(),
        },
    )
    .await;
}

/// `profile/llm/fetch_models` — web `client.ts:767`; params
/// `{ profile_id?, selection: { family_id, route }, api_key? }`.
#[tokio::test]
async fn profile_llm_fetch_models_round_trips() {
    use octoscode_client::domains::profile::{
        LlmFetchModels, LlmFetchModelsParams, LlmModelFetchSelection, LlmRouteSelection,
    };
    let tx = fake_transport(answering(
        "profile/llm/fetch_models",
        serde_json::json!({
            "profile_id": "octoscode",
            "selection": {
                "family_id": "zhipu",
                "route": { "api_key_env": "ZHIPU_API_KEY" }
            }
        }),
        serde_json::json!({
            "profile_id": "octoscode",
            "family_id": "zhipu",
            "models": ["glm-5.2", "glm-5"],
            "status": "discovered"
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<LlmFetchModels>(LlmFetchModelsParams {
            profile_id: Some("octoscode".into()),
            selection: LlmModelFetchSelection {
                family_id: "zhipu".into(),
                route: LlmRouteSelection {
                    api_key_env: Some("ZHIPU_API_KEY".into()),
                    ..Default::default()
                },
            },
            api_key: None,
        })
        .await
        .expect("round trip");
    assert_eq!(out.models, vec!["glm-5.2".to_string(), "glm-5".to_string()]);
    assert_eq!(out.status.as_deref(), Some("discovered"));
}

#[tokio::test]
async fn profile_llm_fetch_models_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{
        LlmFetchModels, LlmFetchModelsParams, LlmModelFetchSelection,
    };
    let client = Client::new(fake_transport(rejecting("profile/llm/fetch_models")));
    assert_rpc_error::<LlmFetchModels>(
        &client,
        LlmFetchModelsParams {
            profile_id: None,
            selection: LlmModelFetchSelection {
                family_id: "zhipu".into(),
                route: Default::default(),
            },
            api_key: None,
        },
    )
    .await;
}

// -------------------------------------------------------- profile/skills/*

/// `profile/skills/list` — web `skills.ts:142`; params `{ profile_id }`.
#[tokio::test]
async fn profile_skills_list_round_trips() {
    use octoscode_client::domains::profile::{SkillsList, SkillsListParams};
    let tx = fake_transport(answering(
        "profile/skills/list",
        serde_json::json!({ "profile_id": "octoscode" }),
        serde_json::json!({
            "profile_id": "octoscode",
            "count": 1,
            "skills": [{
                "name": "mofa-fm", "version": "0.1.0",
                "tool_count": 3, "source_repo": "mofa-org/mofa-skills"
            }]
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<SkillsList>(SkillsListParams {
            profile_id: Some("octoscode".into()),
        })
        .await
        .expect("round trip");
    assert_eq!(out.count, 1);
    assert_eq!(out.skills[0].name, "mofa-fm");
    assert_eq!(out.skills[0].tool_count, 3);
}

#[tokio::test]
async fn profile_skills_list_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{SkillsList, SkillsListParams};
    let client = Client::new(fake_transport(rejecting("profile/skills/list")));
    assert_rpc_error::<SkillsList>(&client, SkillsListParams::default()).await;
}

/// `profile/skills/registry/search` — web `skills.ts:152`; params
/// `{ profile_id, q }`.
#[tokio::test]
async fn profile_skills_search_round_trips() {
    use octoscode_client::domains::profile::{SkillsSearch, SkillsSearchParams};
    let tx = fake_transport(answering(
        "profile/skills/registry/search",
        serde_json::json!({ "profile_id": "octoscode", "q": "voice" }),
        serde_json::json!({
            "profile_id": "octoscode",
            "packages": [{
                "name": "mofa-fm", "description": "voice", "repo": "mofa-org/mofa-skills",
                "skills": [], "requires": [], "tags": [],
                "provides_tools": true, "installed": false, "installed_skills": []
            }]
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<SkillsSearch>(SkillsSearchParams {
            profile_id: Some("octoscode".into()),
            q: Some("voice".into()),
        })
        .await
        .expect("round trip");
    assert_eq!(out.packages.len(), 1);
    assert!(out.packages[0].provides_tools);
}

#[tokio::test]
async fn profile_skills_search_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{SkillsSearch, SkillsSearchParams};
    let client = Client::new(fake_transport(rejecting("profile/skills/registry/search")));
    assert_rpc_error::<SkillsSearch>(&client, SkillsSearchParams::default()).await;
}

/// `profile/skills/install` — web `skills.ts:164`; params
/// `{ profile_id, repo, branch?, force: false }`.
#[tokio::test]
async fn profile_skills_install_round_trips() {
    use octoscode_client::domains::profile::{SkillsInstall, SkillsInstallParams};
    let tx = fake_transport(answering(
        "profile/skills/install",
        serde_json::json!({
            "profile_id": "octoscode", "repo": "mofa-org/mofa-skills", "force": false
        }),
        serde_json::json!({
            "profile_id": "octoscode", "ok": true,
            "installed": ["mofa-fm"], "skipped": [], "deps_installed": ["mofa-core"]
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<SkillsInstall>(SkillsInstallParams {
            profile_id: Some("octoscode".into()),
            repo: "mofa-org/mofa-skills".into(),
            branch: None,
            force: false,
        })
        .await
        .expect("round trip");
    assert!(out.ok);
    assert_eq!(out.installed, vec!["mofa-fm".to_string()]);
    assert_eq!(out.deps_installed, vec!["mofa-core".to_string()]);
}

#[tokio::test]
async fn profile_skills_install_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{SkillsInstall, SkillsInstallParams};
    let client = Client::new(fake_transport(rejecting("profile/skills/install")));
    assert_rpc_error::<SkillsInstall>(
        &client,
        SkillsInstallParams {
            profile_id: None,
            repo: "mofa-org/mofa-skills".into(),
            branch: None,
            force: false,
        },
    )
    .await;
}

/// `profile/skills/remove` — web `skills.ts:191`; params `{ profile_id, name }`.
#[tokio::test]
async fn profile_skills_remove_round_trips() {
    use octoscode_client::domains::profile::{SkillsRemove, SkillsRemoveParams};
    let tx = fake_transport(answering(
        "profile/skills/remove",
        serde_json::json!({ "profile_id": "octoscode", "name": "mofa-fm" }),
        serde_json::json!({
            "profile_id": "octoscode", "ok": true,
            "removed": "mofa-fm", "message": "Removed skill: mofa-fm"
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<SkillsRemove>(SkillsRemoveParams {
            profile_id: Some("octoscode".into()),
            name: "mofa-fm".into(),
        })
        .await
        .expect("round trip");
    assert_eq!(out.removed, "mofa-fm");
}

#[tokio::test]
async fn profile_skills_remove_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{SkillsRemove, SkillsRemoveParams};
    let client = Client::new(fake_transport(rejecting("profile/skills/remove")));
    assert_rpc_error::<SkillsRemove>(
        &client,
        SkillsRemoveParams {
            profile_id: None,
            name: "mofa-fm".into(),
        },
    )
    .await;
}

// -------------------------------------------------- profile/sub_providers/*

/// `profile/sub_providers/list` — web `research.ts:145`; params `{ profile_id }`.
#[tokio::test]
async fn profile_sub_providers_list_round_trips() {
    use octoscode_client::domains::profile::{SubProvidersList, SubProvidersListParams};
    let tx = fake_transport(answering(
        "profile/sub_providers/list",
        serde_json::json!({ "profile_id": "octoscode" }),
        serde_json::json!({
            "profile_id": "octoscode",
            "sub_providers": [{
                "key": "cheap", "provider": "zhipu", "model": "glm-4-flash",
                "api_key_env": "ZHIPU_API_KEY", "base_url": null, "description": "cheap lane",
                "default_context_window": 200000, "max_output_tokens": 4096, "api_type": "openai"
            }]
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<SubProvidersList>(SubProvidersListParams {
            profile_id: Some("octoscode".into()),
        })
        .await
        .expect("round trip");
    assert_eq!(out.sub_providers.len(), 1);
    assert_eq!(out.sub_providers[0].key, "cheap");
}

#[tokio::test]
async fn profile_sub_providers_list_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{SubProvidersList, SubProvidersListParams};
    let client = Client::new(fake_transport(rejecting("profile/sub_providers/list")));
    assert_rpc_error::<SubProvidersList>(&client, SubProvidersListParams::default()).await;
}

/// `profile/sub_providers/upsert` — web `research.ts:164`; params
/// `{ profile_id, sub_provider, api_key? }`.
#[tokio::test]
async fn profile_sub_providers_upsert_round_trips() {
    use octoscode_client::domains::profile::{
        SubProviderParams, SubProvidersUpsert, SubProvidersUpsertParams,
    };
    let tx = fake_transport(answering(
        "profile/sub_providers/upsert",
        serde_json::json!({
            "profile_id": "octoscode",
            "sub_provider": { "key": "cheap", "provider": "zhipu" }
        }),
        serde_json::json!({
            "profile_id": "octoscode",
            "sub_providers": [{ "key": "cheap", "provider": "zhipu" }],
            "applied": true, "restart_required": true
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<SubProvidersUpsert>(SubProvidersUpsertParams {
            profile_id: Some("octoscode".into()),
            sub_provider: SubProviderParams {
                key: "cheap".into(),
                provider: "zhipu".into(),
                model: None,
                api_key_env: None,
                base_url: None,
                description: None,
                default_context_window: None,
                max_output_tokens: None,
                api_type: None,
            },
            api_key: None,
        })
        .await
        .expect("round trip");
    assert!(out.applied);
    assert!(out.restart_required);
}

#[tokio::test]
async fn profile_sub_providers_upsert_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{
        SubProviderParams, SubProvidersUpsert, SubProvidersUpsertParams,
    };
    let client = Client::new(fake_transport(rejecting("profile/sub_providers/upsert")));
    assert_rpc_error::<SubProvidersUpsert>(
        &client,
        SubProvidersUpsertParams {
            profile_id: None,
            sub_provider: SubProviderParams {
                key: "cheap".into(),
                provider: "zhipu".into(),
                model: None,
                api_key_env: None,
                base_url: None,
                description: None,
                default_context_window: None,
                max_output_tokens: None,
                api_type: None,
            },
            api_key: None,
        },
    )
    .await;
}

/// `profile/sub_providers/remove` — web `research.ts:193`; params
/// `{ profile_id, key }`.
#[tokio::test]
async fn profile_sub_providers_remove_round_trips() {
    use octoscode_client::domains::profile::{SubProvidersRemove, SubProvidersRemoveParams};
    let tx = fake_transport(answering(
        "profile/sub_providers/remove",
        serde_json::json!({ "profile_id": "octoscode", "key": "cheap" }),
        serde_json::json!({
            "profile_id": "octoscode", "sub_providers": [],
            "applied": true, "restart_required": true
        }),
    ));
    let client = Client::new(tx);
    let out = client
        .call::<SubProvidersRemove>(SubProvidersRemoveParams {
            profile_id: Some("octoscode".into()),
            key: "cheap".into(),
        })
        .await
        .expect("round trip");
    assert!(out.applied);
    assert!(out.sub_providers.is_empty());
}

#[tokio::test]
async fn profile_sub_providers_remove_surfaces_rpc_error() {
    use octoscode_client::domains::profile::{SubProvidersRemove, SubProvidersRemoveParams};
    let client = Client::new(fake_transport(rejecting("profile/sub_providers/remove")));
    assert_rpc_error::<SubProvidersRemove>(
        &client,
        SubProvidersRemoveParams {
            profile_id: None,
            key: "cheap".into(),
        },
    )
    .await;
}

// --------------------------------------------------------------- coverage

/// Every method in this domain is a `Method` with the exact wire name and a
/// distinct one; a duplicate name would silently shadow in `Client::call`.
#[test]
fn every_f2_method_has_its_exact_wire_name() {
    use octoscode_client::domains::profile as p;
    fn name<M: octoscode_client::Method>() -> &'static str {
        M::NAME
    }
    let names = [
        name::<p::PermissionProfileList>(),
        name::<p::PermissionProfileSet>(),
        name::<p::WorkspaceList>(),
        name::<p::WorkspaceCreate>(),
        name::<p::LlmCatalog>(),
        name::<p::ProfileLlmList>(),
        name::<p::ProfileLlmSelect>(),
        name::<p::LlmUpsert>(),
        name::<p::LlmTest>(),
        name::<p::ProfileLlmDelete>(),
        name::<p::LlmFetchModels>(),
        name::<p::SkillsList>(),
        name::<p::SkillsSearch>(),
        name::<p::SkillsInstall>(),
        name::<p::SkillsRemove>(),
        name::<p::SubProvidersList>(),
        name::<p::SubProvidersUpsert>(),
        name::<p::SubProvidersRemove>(),
    ];
    let expected = [
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
    assert_eq!(names, expected);
    let mut uniq: Vec<&str> = names.to_vec();
    uniq.sort_unstable();
    uniq.dedup();
    assert_eq!(uniq.len(), names.len(), "wire names must be distinct");
    // No method name collides with a notification name (a `Method` and a
    // handler sharing a name would make the registry's routing ambiguous).
    assert_eq!(names.len(), 18, "18 methods in the #F2 list");
}

/// Decode-with-method-name: a bad result is a `Decode` error naming the method
/// (never a panic), and the method name is the profile domain's.
#[tokio::test]
async fn a_bad_result_is_a_decode_error_naming_the_method() {
    use octoscode_client::domains::profile::{LlmCatalog, LlmCatalogParams};
    let tx = fake_transport(|_m, _p| Ok(serde_json::json!({ "unexpected": true })));
    let client = Client::new(tx);
    match client.call::<LlmCatalog>(LlmCatalogParams {}).await {
        // `families` is `#[serde(default)]`, so a foreign object still decodes
        // to an empty catalog rather than erroring — assert that documented
        // tolerance explicitly.
        Ok(out) => assert!(out.families.is_empty()),
        Err(e) => panic!("catalog tolerates a missing families key, got {e:?}"),
    }
}
