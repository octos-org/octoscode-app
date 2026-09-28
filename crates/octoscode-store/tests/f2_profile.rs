//! #F2 — profile store-domain tests.
//!
//! Exercise the store's `profile` domain directly (no transport): each setter
//! is fed the typed result the corresponding `Method` decodes, and the state
//! shaped for the UI the parity matrix describes is asserted back out. Field
//! names match the wire (`docs/parity-matrix.csv`: models, research, skills,
//! workspace-create, session-config).
use octoscode_store::domains::profile::{
    InstalledSkill, PermissionNetworkPolicy, PermissionProfileMode, PermissionProfileSelection,
    ProfileLlmModel, SkillPackage, SubProvider, WorkspaceEntry, WorkspaceListing,
};

#[test]
fn permission_selection_round_trips() {
    let store = octoscode_store::Store::new();
    assert!(store.domains.profile.permission().is_none());

    // `permission/profile/list` result: current + offered modes.
    store.domains.profile.set_permission(
        PermissionProfileSelection {
            mode: PermissionProfileMode::WorkspaceWrite,
            network: PermissionNetworkPolicy::Deny,
        },
        vec![
            PermissionProfileSelection {
                mode: PermissionProfileMode::ReadOnly,
                network: PermissionNetworkPolicy::Deny,
            },
            PermissionProfileSelection {
                mode: PermissionProfileMode::WorkspaceWrite,
                network: PermissionNetworkPolicy::Deny,
            },
        ],
    );
    assert_eq!(
        store.domains.profile.permission().unwrap().mode,
        PermissionProfileMode::WorkspaceWrite
    );
    assert_eq!(store.domains.profile.permission_profiles().len(), 2);

    // `permission/profile/set` result: only `current` changes.
    store
        .domains
        .profile
        .set_permission_current(PermissionProfileSelection {
            mode: PermissionProfileMode::ReadOnly,
            network: PermissionNetworkPolicy::Deny,
        });
    assert_eq!(
        store.domains.profile.permission().unwrap().mode,
        PermissionProfileMode::ReadOnly
    );
    // The offered set is unchanged by a write.
    assert_eq!(store.domains.profile.permission_profiles().len(), 2);
}

#[test]
fn installed_skills_replace_on_list() {
    let store = octoscode_store::Store::new();
    store.domains.profile.set_installed_skills(vec![InstalledSkill {
        name: "mofa-fm".into(),
        version: Some("0.1.0".into()),
        tool_count: 3,
        source_repo: Some("mofa-org/mofa-skills".into()),
    }]);
    assert_eq!(store.domains.profile.installed_skills().len(), 1);
    assert_eq!(store.domains.profile.installed_skills()[0].tool_count, 3);

    // A re-list REPLACES (never appends): the wire carries the full inventory.
    store.domains.profile.set_installed_skills(vec![]);
    assert!(store.domains.profile.installed_skills().is_empty());
}

#[test]
fn registry_packages_replace_on_search() {
    let store = octoscode_store::Store::new();
    store.domains.profile.set_registry_packages(vec![SkillPackage {
        name: "mofa-fm".into(),
        description: "voice".into(),
        repo: "mofa-org/mofa-skills".into(),
        version: None,
        author: None,
        license: None,
        skills: vec![],
        requires: vec![],
        tags: vec![],
        provides_tools: true,
        installed: false,
        installed_skills: vec![],
    }]);
    assert_eq!(store.domains.profile.registry_packages().len(), 1);
    assert!(store.domains.profile.registry_packages()[0].provides_tools);
}

#[test]
fn research_lanes_replace_on_list() {
    let store = octoscode_store::Store::new();
    store.domains.profile.set_sub_providers(vec![
        SubProvider {
            key: "cheap".into(),
            provider: "zhipu".into(),
            model: Some("glm-4-flash".into()),
            api_key_env: Some("ZHIPU_API_KEY".into()),
            base_url: None,
            description: Some("cheap lane".into()),
            default_context_window: Some(200_000),
            max_output_tokens: Some(4096),
            api_type: Some("openai".into()),
        },
        SubProvider {
            key: "strong".into(),
            provider: "anthropic".into(),
            model: None,
            api_key_env: None,
            base_url: None,
            description: None,
            default_context_window: None,
            max_output_tokens: None,
            api_type: None,
        },
    ]);
    assert_eq!(store.domains.profile.sub_providers().len(), 2);
    assert_eq!(store.domains.profile.sub_providers()[0].key, "cheap");

    // A remove receipt carries the remaining lanes.
    store
        .domains
        .profile
        .set_sub_providers(vec![store.domains.profile.sub_providers().pop().unwrap()]);
    assert_eq!(store.domains.profile.sub_providers().len(), 1);
    assert_eq!(store.domains.profile.sub_providers()[0].key, "strong");
}

#[test]
fn llm_models_and_restart_flag_track_the_write() {
    let store = octoscode_store::Store::new();
    assert!(store.domains.profile.llm_models().is_empty());
    assert!(!store.domains.profile.llm_restart_required());

    store.domains.profile.set_llm_models(vec![ProfileLlmModel {
        model: "glm-5.2".into(),
        provider: "zhipu".into(),
        title: "zhipu / glm-5.2".into(),
        family: Some("zhipu".into()),
        route: Some("official".into()),
        selected: true,
        available: true,
    }]);
    assert_eq!(store.domains.profile.llm_models().len(), 1);
    assert!(store.domains.profile.llm_models()[0].selected);

    // `profile/llm/select` may answer `restart_required: true` for a pinned
    // profile; the flag is surfaced so the UI never claims the change is live.
    store.domains.profile.set_llm_restart_required(true);
    assert!(store.domains.profile.llm_restart_required());
}

#[test]
fn workspace_listing_is_kept_for_the_browser() {
    let store = octoscode_store::Store::new();
    assert!(store.domains.profile.workspace().is_none());
    store.domains.profile.set_workspace(WorkspaceListing {
        canonical_path: "/home/user".into(),
        parent_path: Some("/home".into()),
        writable: true,
        entries: vec![WorkspaceEntry {
            name: "work".into(),
            path: "/home/user/work".into(),
            writable: true,
        }],
        truncated: false,
        hidden_skipped: 2,
    });
    let listing = store.domains.profile.workspace().unwrap();
    assert_eq!(listing.canonical_path, "/home/user");
    assert_eq!(listing.entries.len(), 1);
    assert_eq!(listing.hidden_skipped, 2);
}

#[test]
fn current_and_providers_are_unchanged_for_existing_consumers() {
    // The pre-existing accessors (session/open handler + modules) must keep
    // working exactly as before this lane widened the domain.
    let store = octoscode_store::Store::new();
    store.domains.profile.set_current("octoscode".into());
    store.domains.profile.set_providers(vec!["anthropic".into()]);
    assert_eq!(store.domains.profile.current().as_deref(), Some("octoscode"));
    assert_eq!(store.domains.profile.providers().len(), 1);
}
