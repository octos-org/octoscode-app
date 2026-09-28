//! `profile` state: profiles, the LLM provider/model surface, installed skills,
//! research lanes, the per-session permission mode, and workspace browsing.
//!
//! The store keeps its own copy of each wire shape (like `tool.rs` does for
//! `RuntimeTool`): the client crate depends on this one, so the store cannot
//! borrow the client's types. Field names match the octos-core types
//! (`permission/profile/*`) and the octos-cli AppUI `json!` shapes
//! (`profile/llm/*`, `profile/skills/*`, `profile/sub_providers/*`,
//! `onboarding/workspace_*`), so a UI can render them 1:1.
//!
//! Every setter is fed by the corresponding `Method`'s typed result on the
//! production path (`octoscode-client::domains::profile`). The `X`-typed
//! structs are the *result* projections the matrices describe
//! (`docs/parity-matrix.csv`: skills, models, research, workspace-create,
//! session-config, product-settings).
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// The permission mode (`PermissionProfileMode`, octos-core
/// `ui_protocol.rs:2312`; web `packages/client/src/types.ts:97`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionProfileMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl Default for PermissionProfileMode {
    fn default() -> Self {
        Self::WorkspaceWrite
    }
}

/// The network policy (`PermissionNetworkPolicy`, octos-core `:2344`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionNetworkPolicy {
    Allow,
    Deny,
}

impl Default for PermissionNetworkPolicy {
    fn default() -> Self {
        Self::Deny
    }
}

/// The effective selection (`PermissionProfileSelection`, octos-core `:2348`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionProfileSelection {
    pub mode: PermissionProfileMode,
    pub network: PermissionNetworkPolicy,
}

impl Default for PermissionProfileSelection {
    fn default() -> Self {
        Self {
            mode: PermissionProfileMode::default(),
            network: PermissionNetworkPolicy::default(),
        }
    }
}

/// One installed skill (`InstalledSkill`, web `skills.ts:6`; octos-cli
/// `skill_entry_with_status`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledSkill {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub tool_count: u64,
    #[serde(default)]
    pub source_repo: Option<String>,
}

/// One registry package (`SkillPackage`, web `skills.ts:12`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillPackage {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub provides_tools: bool,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub installed_skills: Vec<String>,
}

/// One research lane (`ResearchLane`, web `research.ts:7`; octos-cli
/// `sub_provider_json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubProvider {
    pub key: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub default_context_window: Option<u32>,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
    #[serde(default)]
    pub api_type: Option<String>,
}

/// One configured model in the session picker (`ProfileLlmModel`, web
/// `onboarding.ts:163`; octos-cli `configured_model_status_json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileLlmModel {
    pub model: String,
    pub provider: String,
    pub title: String,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub route: Option<String>,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub available: bool,
}

/// The last workspace-browse listing (`WorkspaceListResult`, web
/// `workspace-browse.ts:40`; octos-cli `onboarding_workspace_list_result`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceListing {
    pub canonical_path: String,
    #[serde(default)]
    pub parent_path: Option<String>,
    #[serde(default)]
    pub writable: bool,
    #[serde(default)]
    pub entries: Vec<WorkspaceEntry>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub hidden_skipped: u64,
}

/// One directory entry (`WorkspaceFolderEntry`, web `workspace-browse.ts:31`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub writable: bool,
}

/// The profile domain.
#[derive(Debug, Default)]
pub struct Profiles {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    /// The profile the connection is using (from `session/open`).
    current: Option<String>,
    /// LLM provider ids the server listed.
    providers: Vec<String>,
    /// The effective permission selection (`permission/profile/*`).
    permission: Option<PermissionProfileSelection>,
    /// Every permission mode the server offers for the session.
    permission_profiles: Vec<PermissionProfileSelection>,
    /// Installed skills (`profile/skills/list`).
    installed_skills: Vec<InstalledSkill>,
    /// Registry search results (`profile/skills/registry/search`).
    registry_packages: Vec<SkillPackage>,
    /// Research lanes (`profile/sub_providers/list|upsert|remove`).
    sub_providers: Vec<SubProvider>,
    /// The session-scoped model picker list (`profile/llm/list`).
    llm_models: Vec<ProfileLlmModel>,
    /// Whether the profile's last LLM write applied but needs a restart.
    llm_restart_required: bool,
    /// The last workspace listing (`onboarding/workspace_list`).
    workspace: Option<WorkspaceListing>,
}

impl Profiles {
    pub fn set_current(&self, profile_id: String) {
        self.inner.lock().unwrap().current = Some(profile_id);
    }

    pub fn current(&self) -> Option<String> {
        self.inner.lock().unwrap().current.clone()
    }

    pub fn set_providers(&self, providers: Vec<String>) {
        self.inner.lock().unwrap().providers = providers;
    }

    pub fn providers(&self) -> Vec<String> {
        self.inner.lock().unwrap().providers.clone()
    }

    // ---- permission/profile/* --------------------------------------------

    /// Record a read: the current selection plus the modes the server offers.
    pub fn set_permission(
        &self,
        current: PermissionProfileSelection,
        profiles: Vec<PermissionProfileSelection>,
    ) {
        let mut inner = self.inner.lock().unwrap();
        inner.permission = Some(current);
        if !profiles.is_empty() {
            inner.permission_profiles = profiles;
        }
    }

    /// Record an applied write: only the current selection changes
    /// (`permission/profile/set` → `{ session_id, current, applied }`).
    pub fn set_permission_current(&self, current: PermissionProfileSelection) {
        self.inner.lock().unwrap().permission = Some(current);
    }

    pub fn permission(&self) -> Option<PermissionProfileSelection> {
        self.inner.lock().unwrap().permission
    }

    pub fn permission_profiles(&self) -> Vec<PermissionProfileSelection> {
        self.inner.lock().unwrap().permission_profiles.clone()
    }

    // ---- profile/skills/* -------------------------------------------------

    pub fn set_installed_skills(&self, skills: Vec<InstalledSkill>) {
        self.inner.lock().unwrap().installed_skills = skills;
    }

    pub fn installed_skills(&self) -> Vec<InstalledSkill> {
        self.inner.lock().unwrap().installed_skills.clone()
    }

    pub fn set_registry_packages(&self, packages: Vec<SkillPackage>) {
        self.inner.lock().unwrap().registry_packages = packages;
    }

    pub fn registry_packages(&self) -> Vec<SkillPackage> {
        self.inner.lock().unwrap().registry_packages.clone()
    }

    // ---- profile/sub_providers/* -----------------------------------------

    pub fn set_sub_providers(&self, lanes: Vec<SubProvider>) {
        self.inner.lock().unwrap().sub_providers = lanes;
    }

    pub fn sub_providers(&self) -> Vec<SubProvider> {
        self.inner.lock().unwrap().sub_providers.clone()
    }

    // ---- profile/llm/* ---------------------------------------------------

    pub fn set_llm_models(&self, models: Vec<ProfileLlmModel>) {
        self.inner.lock().unwrap().llm_models = models;
    }

    pub fn llm_models(&self) -> Vec<ProfileLlmModel> {
        self.inner.lock().unwrap().llm_models.clone()
    }

    pub fn set_llm_restart_required(&self, required: bool) {
        self.inner.lock().unwrap().llm_restart_required = required;
    }

    pub fn llm_restart_required(&self) -> bool {
        self.inner.lock().unwrap().llm_restart_required
    }

    // ---- onboarding/workspace_* ------------------------------------------

    pub fn set_workspace(&self, listing: WorkspaceListing) {
        self.inner.lock().unwrap().workspace = Some(listing);
    }

    pub fn workspace(&self) -> Option<WorkspaceListing> {
        self.inner.lock().unwrap().workspace.clone()
    }
}
