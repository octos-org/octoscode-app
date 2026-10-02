//! `profile/*` + `onboarding/*` + `permission/profile/*` — profiles, LLM
//! providers, skills, workspace browsing, and the per-session permission mode.
//!
//! The three request families here are the fan-out lane's (board #F2):
//!
//! - **Core methods** (`permission/profile/list|set`): typed from octos-core
//!   `a6ea8505` — the `PermissionProfile*` types in
//!   `crates/octos-core/src/ui_protocol.rs` (list params `:2396`, set params
//!   `:2400+`, mode `:2312`, selection `:2348`, update `:2373`,
//!   `PermissionNetworkPolicy`). Their wire shape is what the web sends from
//!   `packages/client/src/types.ts:97-131` (`PermissionProfileMode`,
//!   `PermissionNetworkPolicy`, `PermissionProfileSelection`,
//!   `PermissionProfileUpdate`, `PermissionProfileList/SetParams`), so the
//!   octos-core types ARE the parity shape — no re-declaration needed.
//! - **AppUI extensions** (`profile/llm/*`, `profile/skills/*`,
//!   `profile/sub_providers/*`, `onboarding/workspace_*`): octos-core declares
//!   NO types for these; they are served by octos-cli's raw AppUI transport
//!   (`crates/octos-cli/src/api/ui_protocol_transport.rs`). Params and results
//!   below mirror the *web client's* wire payloads (`packages/client/src/`,
//!   cited per method) and the octos-cli handlers' `json!` shapes, cross-checked
//!   in `docs/protocol-ext-matrix.csv` (rows for this domain).
//!
//! This file owns no notifications: the profile/onboarding extension surface is
//! request-only. `skill/action/job/updated` (the one related notification) is
//! owned by `skill_jobs.rs` (A31, parity row 15).
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use octos_core::ui_protocol::methods;
use octoscode_store::Store;

use crate::method::Method;
use crate::registry::Registry;

// ------------------------------------------------------------------ core

/// `permission/profile/list` (`packages/client/src/client.ts:571`).
///
/// Params/result are the octos-core typed pair (see module docs); the web's
/// `PermissionProfileListParams { session_id }` (`types.ts:112`) is the same
/// shape, so we reuse the core types directly.
pub struct PermissionProfileList;

impl Method for PermissionProfileList {
    const NAME: &'static str = methods::PERMISSION_PROFILE_LIST;
    type Params = octos_core::ui_protocol::PermissionProfileListParams;
    type Result = octos_core::ui_protocol::PermissionProfileListResult;
}

/// `permission/profile/set` (`packages/client/src/client.ts:583`).
///
/// The web sends `{ session_id, update, runtime_mode? }`
/// (`types.ts:116-120`), which is exactly `PermissionProfileSetParams`; the
/// server replies `{ session_id, current, applied }` (`spec-b.md:101-104`).
pub struct PermissionProfileSet;

impl Method for PermissionProfileSet {
    const NAME: &'static str = methods::PERMISSION_PROFILE_SET;
    type Params = octos_core::ui_protocol::PermissionProfileSetParams;
    type Result = octos_core::ui_protocol::PermissionProfileSetResult;
}

// ------------------------------------------------------- onboarding/* (ext)

/// `onboarding/workspace_list` — AppUI extension
/// (`packages/client/src/workspace-browse.ts:211`, `onboarding-methods.ts:23`).
///
/// The web sends `{ path: string | null }` (`workspace-browse.ts:29`); the
/// octos-cli handler accepts the params omitted entirely, meaning the same as
/// `{ "path": null }` — list the server's own working directory
/// (`ui_protocol_transport.rs:19453`).
#[derive(Debug, Default, Clone, Serialize)]
pub struct WorkspaceListParams {
    /// Absolute, `~`-prefixed, or `None` for the server's own working dir.
    pub path: Option<String>,
}

/// One directory row (`WorkspaceFolderEntry`, `workspace-browse.ts:31`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WorkspaceFolderEntry {
    pub name: String,
    /// The canonical absolute path of this subdirectory.
    pub path: String,
    /// Whether a folder could be created inside this subdirectory.
    pub writable: bool,
}

/// The `onboarding/workspace_list` result (`WorkspaceListResult`,
/// `workspace-browse.ts:40`; octos-cli `:19520`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WorkspaceListResult {
    pub canonical_path: String,
    /// `None` at the filesystem root, or when the parent is a banned path.
    #[serde(default)]
    pub parent_path: Option<String>,
    pub writable: bool,
    /// DIRECTORIES ONLY, sorted case-insensitively by name.
    #[serde(default)]
    pub entries: Vec<WorkspaceFolderEntry>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub hidden_skipped: u64,
}

pub struct WorkspaceList;

impl Method for WorkspaceList {
    const NAME: &'static str = "onboarding/workspace_list";
    type Params = WorkspaceListParams;
    type Result = WorkspaceListResult;
}

/// `onboarding/workspace_create` — AppUI extension
/// (`packages/client/src/workspace-browse.ts:212`, `onboarding-methods.ts:25`).
/// Params `{ parent, name }` (`workspace-browse.ts:52`).
#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceCreateParams {
    pub parent: String,
    pub name: String,
}

/// `WorkspaceCreateResult` (`workspace-browse.ts:57`; octos-cli `:19556`).
/// `created: false` is an idempotent success — a directory of that name
/// already existed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WorkspaceCreateResult {
    pub canonical_path: String,
    pub created: bool,
}

pub struct WorkspaceCreate;

impl Method for WorkspaceCreate {
    const NAME: &'static str = "onboarding/workspace_create";
    type Params = WorkspaceCreateParams;
    type Result = WorkspaceCreateResult;
}

// -------------------------------------------------------- profile/llm/* (ext)

/// The closed, per-model inference/routing overrides
/// (`LlmInferenceOverrides`, `packages/client/src/onboarding.ts:51`). Unknown
/// keys are rejected by the server (`reject_unknown_llm_upsert_fields`), so
/// every field is explicit and skipped when unset. `null` ≡ absent ≡ inherit
/// (`onboarding.ts:50`).
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmInferenceOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_hints: Option<serde_json::Value>,
}

/// A provisioning route (`LlmRouteSelection`,
/// `packages/client/src/onboarding.ts:42`; octos-cli `RawLlmRoute`).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmRouteSelection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_type: Option<String>,
}

/// One complete model selection (`LlmSelection`, `onboarding.ts:71`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmSelection {
    pub family_id: String,
    pub model_id: String,
    #[serde(default)]
    pub route: LlmRouteSelection,
    #[serde(flatten)]
    pub inference: LlmInferenceOverrides,
}

/// `profile/llm/upsert` and `profile/llm/test` params
/// (`LlmProvisionParams`, `onboarding.ts:77`; client.ts:777/787).
#[derive(Debug, Clone, Serialize)]
pub struct LlmProvisionParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub selection: LlmSelection,
    /// Omit to reuse the saved secret for `selection.route.api_key_env`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_primary: Option<bool>,
}

/// `LlmUpsertResult` (`onboarding.ts:92`; octos-cli
/// `profile_llm_mutation_result`).
#[derive(Debug, Clone, Deserialize)]
pub struct LlmUpsertResult {
    pub profile_id: String,
    #[serde(default)]
    pub applied: bool,
}

/// `LlmTestResult` (`onboarding.ts:85`; octos-cli `profile_llm_test_result`).
#[derive(Debug, Clone, Deserialize)]
pub struct LlmTestResult {
    pub profile_id: String,
    #[serde(default)]
    pub applied: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub error: Option<String>,
}

pub struct LlmUpsert;
impl Method for LlmUpsert {
    const NAME: &'static str = "profile/llm/upsert";
    type Params = LlmProvisionParams;
    type Result = LlmUpsertResult;
}

pub struct LlmTest;
impl Method for LlmTest {
    const NAME: &'static str = "profile/llm/test";
    type Params = LlmProvisionParams;
    type Result = LlmTestResult;
}

/// `profile/llm/catalog` params — empty (`client.ts:712`).
#[derive(Debug, Default, Clone, Serialize)]
pub struct LlmCatalogParams {}

/// One provisioning endpoint (`LlmCatalogEndpoint`, `onboarding.ts:19`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LlmCatalogEndpoint {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub api_type: Option<String>,
}

/// One model in the catalog (`LlmCatalogModel`, `onboarding.ts:27`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LlmCatalogModel {
    pub id: String,
    #[serde(default)]
    pub endpoints: Vec<LlmCatalogEndpoint>,
}

/// One model family (`LlmCatalogFamily`, `onboarding.ts:32`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LlmCatalogFamily {
    pub id: String,
    #[serde(default)]
    pub env: String,
    #[serde(default)]
    pub models: Vec<LlmCatalogModel>,
}

/// The wire body of one family. The server sends `families` as a **map keyed
/// by family id** (`json!({ "families": Value::Object(families) })`,
/// `ui_protocol_transport.rs`), and the web reshapes it to an array carrying
/// the key as `id` (`Object.entries(value.families)`, `onboarding.ts:225`).
/// This body is the map's value; [`LlmCatalogResult`] injects the key as `id`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LlmCatalogFamilyBody {
    #[serde(default)]
    env: String,
    #[serde(default)]
    models: Vec<LlmCatalogModel>,
}

/// `profile/llm/catalog` wire shape: `{ families: { <id>: { env, models } } }`.
#[derive(Debug, Clone, Deserialize)]
struct LlmCatalogWire {
    #[serde(default)]
    families: std::collections::BTreeMap<String, LlmCatalogFamilyBody>,
}

/// `LlmCatalogResult` (`onboarding.ts:38`) — the web's ARRAY projection of the
/// server's family map, each row carrying its map key as `id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmCatalogResult {
    pub families: Vec<LlmCatalogFamily>,
}

impl<'de> Deserialize<'de> for LlmCatalogResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = LlmCatalogWire::deserialize(deserializer)?;
        Ok(Self {
            families: wire
                .families
                .into_iter()
                .map(|(id, body)| LlmCatalogFamily {
                    id,
                    env: body.env,
                    models: body.models,
                })
                .collect(),
        })
    }
}

pub struct LlmCatalog;
impl Method for LlmCatalog {
    const NAME: &'static str = "profile/llm/catalog";
    type Params = LlmCatalogParams;
    type Result = LlmCatalogResult;
}

/// `profile/llm/list` params. The web sends `{ session_id, profile_id? }` for
/// the session-scoped model picker (`ProfileLlmListParams`,
/// `onboarding.ts:158`) OR `{ profile_id? }` for the profile-config read
/// (`ProfileLlmConfigReadParams`, `onboarding.ts:98`); the server branches on
/// the presence of `session_id` (`ui_protocol_transport.rs:19258`).
#[derive(Debug, Default, Clone, Serialize)]
pub struct ProfileLlmListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
}

/// One model row of the session-scoped picker (`ProfileLlmModel`,
/// `onboarding.ts:163`; octos-cli `configured_model_status_json`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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

/// A secret-free projection of one configured model (`ProfileLlmConfiguredModel`,
/// `onboarding.ts:111`; octos-cli `configured_provider_json`). Extra inference
/// keys (temperature/top_p/…) ride in `inference` and are only present when
/// configured.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ProfileLlmConfiguredModel {
    #[serde(default)]
    pub family_id: String,
    #[serde(default)]
    pub model_id: String,
    #[serde(default)]
    pub route: LlmRouteSelection,
    #[serde(default)]
    pub has_api_key: bool,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub edit_blocked: Option<bool>,
    #[serde(flatten)]
    pub inference: LlmInferenceOverrides,
}

/// The `profile/llm/list` result. Its shape depends on the request (see
/// [`ProfileLlmListParams`]): the profile-config read returns
/// `{ profile_id, primary, fallbacks, … }` (`ProfileLlmConfigResult`,
/// `onboarding.ts:122`), the session-scoped read returns
/// `{ session_id, models }` (`ProfileLlmListResult`, `onboarding.ts:173`).
/// Both are surfaced; the absent half stays empty.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProfileLlmListResult {
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub primary: Option<ProfileLlmConfiguredModel>,
    #[serde(default)]
    pub fallbacks: Vec<ProfileLlmConfiguredModel>,
    #[serde(default)]
    pub models: Vec<ProfileLlmModel>,
}

pub struct ProfileLlmList;
impl Method for ProfileLlmList {
    const NAME: &'static str = "profile/llm/list";
    type Params = ProfileLlmListParams;
    type Result = ProfileLlmListResult;
}

/// `profile/llm/select` params (`ProfileLlmSelectParams`,
/// `onboarding.ts:178`; octos-cli `RawProfileLlmSelectParams`).
#[derive(Debug, Default, Clone, Serialize)]
pub struct ProfileLlmSelectParams {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub family_id: String,
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_id: Option<String>,
}

/// `ProfileLlmSelectResult` (`onboarding.ts:184`; octos-cli
/// `raw_profile_llm_select`).
#[derive(Debug, Clone, Deserialize)]
pub struct ProfileLlmSelectResult {
    pub session_id: String,
    pub selected: ProfileLlmModel,
    #[serde(default)]
    pub applied: bool,
    #[serde(default)]
    pub restart_required: Option<bool>,
    #[serde(default)]
    pub runtime_policy_stamp: Option<serde_json::Value>,
}

pub struct ProfileLlmSelect;
impl Method for ProfileLlmSelect {
    const NAME: &'static str = "profile/llm/select";
    type Params = ProfileLlmSelectParams;
    type Result = ProfileLlmSelectResult;
}

/// `profile/llm/delete` params (`ProfileLlmDeleteParams`, `onboarding.ts:147`;
/// octos-cli `RawProfileLlmDeleteParams`).
#[derive(Debug, Clone, Serialize)]
pub struct ProfileLlmDeleteParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub family_id: String,
    pub model_id: String,
    pub route_id: String,
}

/// `ProfileLlmDeleteResult` = the config read plus `applied`
/// (`onboarding.ts:154`; octos-cli `profile_llm_mutation_result`).
#[derive(Debug, Clone, Deserialize)]
pub struct ProfileLlmDeleteResult {
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub primary: Option<ProfileLlmConfiguredModel>,
    #[serde(default)]
    pub fallbacks: Vec<ProfileLlmConfiguredModel>,
    #[serde(default)]
    pub applied: bool,
}

pub struct ProfileLlmDelete;
impl Method for ProfileLlmDelete {
    const NAME: &'static str = "profile/llm/delete";
    type Params = ProfileLlmDeleteParams;
    type Result = ProfileLlmDeleteResult;
}

/// `profile/llm/fetch_models` params (`LlmFetchModelsParams`,
/// `onboarding.ts:133`; octos-cli reuses `RawProfileLlmUpsertParams`).
#[derive(Debug, Clone, Serialize)]
pub struct LlmFetchModelsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub selection: LlmModelFetchSelection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// The fetch selection is family + route, no model (`LlmModelFetchSelection`,
/// `onboarding.ts:128`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LlmModelFetchSelection {
    pub family_id: String,
    pub route: LlmRouteSelection,
}

/// `LlmFetchModelsResult` (`onboarding.ts:140`; octos-cli
/// `raw_profile_llm_fetch_models` — adds a typed `status`).
#[derive(Debug, Clone, Deserialize)]
pub struct LlmFetchModelsResult {
    pub profile_id: String,
    #[serde(default)]
    pub family_id: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

pub struct LlmFetchModels;
impl Method for LlmFetchModels {
    const NAME: &'static str = "profile/llm/fetch_models";
    type Params = LlmFetchModelsParams;
    type Result = LlmFetchModelsResult;
}

// ----------------------------------------------------- profile/skills/* (ext)

/// `profile/skills/list` params (`skills.ts:142`; octos-cli
/// `RawProfileSkillsListParams`).
#[derive(Debug, Default, Clone, Serialize)]
pub struct SkillsListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
}

/// One installed skill (`InstalledSkill`, `packages/client/src/skills.ts:6`;
/// octos-cli `skill_entry_with_status`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InstalledSkill {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub tool_count: u64,
    #[serde(default)]
    pub source_repo: Option<String>,
}

/// `profile/skills/list` result (`skills.ts:44`; octos-cli `:11668`).
#[derive(Debug, Clone, Deserialize)]
pub struct SkillsListResult {
    pub profile_id: String,
    #[serde(default)]
    pub count: u64,
    #[serde(default)]
    pub skills: Vec<InstalledSkill>,
}

pub struct SkillsList;
impl Method for SkillsList {
    const NAME: &'static str = "profile/skills/list";
    type Params = SkillsListParams;
    type Result = SkillsListResult;
}

/// `profile/skills/registry/search` params (`skills.ts:152`; octos-cli
/// `RawProfileSkillsRegistrySearchParams` — `q` also accepts the alias `query`).
#[derive(Debug, Default, Clone, Serialize)]
pub struct SkillsSearchParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
}

/// One registry package (`SkillPackage`, `skills.ts:12`; octos-cli
/// `raw_profile_skills_registry_search`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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

/// `profile/skills/registry/search` result (`skills.ts:79`; octos-cli `:13126`).
#[derive(Debug, Clone, Deserialize)]
pub struct SkillsSearchResult {
    pub profile_id: String,
    #[serde(default)]
    pub packages: Vec<SkillPackage>,
}

pub struct SkillsSearch;
impl Method for SkillsSearch {
    const NAME: &'static str = "profile/skills/registry/search";
    type Params = SkillsSearchParams;
    type Result = SkillsSearchResult;
}

/// `profile/skills/install` params (`skills.ts:164`; octos-cli
/// `RawProfileSkillsInstallParams`). `branch` defaults to `main` server-side.
#[derive(Debug, Clone, Serialize)]
pub struct SkillsInstallParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub repo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default)]
    pub force: bool,
}

/// `SkillInstallResult` (`skills.ts:26`; octos-cli `:13161`).
#[derive(Debug, Clone, Deserialize)]
pub struct SkillInstallResult {
    pub profile_id: String,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub installed: Vec<String>,
    #[serde(default)]
    pub skipped: Vec<String>,
    /// The web renames this to `dependenciesInstalled` (`skills.ts:186`).
    #[serde(default)]
    pub deps_installed: Vec<String>,
}

pub struct SkillsInstall;
impl Method for SkillsInstall {
    const NAME: &'static str = "profile/skills/install";
    type Params = SkillsInstallParams;
    type Result = SkillInstallResult;
}

/// `profile/skills/remove` params (`skills.ts:191`; octos-cli
/// `RawProfileSkillsRemoveParams`).
#[derive(Debug, Clone, Serialize)]
pub struct SkillsRemoveParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub name: String,
}

/// `profile/skills/remove` result (`skills.ts:194`; octos-cli `:13182`).
#[derive(Debug, Clone, Deserialize)]
pub struct SkillsRemoveResult {
    pub profile_id: String,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub removed: String,
    #[serde(default)]
    pub message: String,
}

pub struct SkillsRemove;
impl Method for SkillsRemove {
    const NAME: &'static str = "profile/skills/remove";
    type Params = SkillsRemoveParams;
    type Result = SkillsRemoveResult;
}

// ----------------------------------------------- profile/sub_providers/* (ext)

/// `profile/sub_providers/list` params (`research.ts:145`; octos-cli
/// `RawProfileParams`).
#[derive(Debug, Default, Clone, Serialize)]
pub struct SubProvidersListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
}

/// One named provider lane (`ResearchLane`, `packages/client/src/research.ts:7`;
/// octos-cli `sub_provider_json`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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

/// The `profile/sub_providers/list` result (`research.ts:37`; octos-cli
/// `sub_providers_list_result`).
#[derive(Debug, Clone, Deserialize)]
pub struct SubProvidersListResult {
    pub profile_id: String,
    #[serde(default)]
    pub sub_providers: Vec<SubProvider>,
    #[serde(default)]
    pub runtime_policy_stamp: Option<serde_json::Value>,
}

pub struct SubProvidersList;
impl Method for SubProvidersList {
    const NAME: &'static str = "profile/sub_providers/list";
    type Params = SubProvidersListParams;
    type Result = SubProvidersListResult;
}

/// The lane payload an upsert sends (`researchLaneParams`, `research.ts:117`;
/// octos-cli `RawSubProvider`) — an explicit whitelist, never a spread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubProviderParams {
    pub key: String,
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_context_window: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_type: Option<String>,
}

/// `profile/sub_providers/upsert` params (`research.ts:164`; octos-cli
/// `RawProfileSubProvidersUpsertParams`).
#[derive(Debug, Clone, Serialize)]
pub struct SubProvidersUpsertParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub sub_provider: SubProviderParams,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// The mutation receipt (`ResearchLaneMutation`, `research.ts:22`; octos-cli
/// `sub_providers_mutation_result` — adds `applied` + `restart_required`).
#[derive(Debug, Clone, Deserialize)]
pub struct SubProvidersMutationResult {
    pub profile_id: String,
    #[serde(default)]
    pub sub_providers: Vec<SubProvider>,
    #[serde(default)]
    pub applied: bool,
    #[serde(default)]
    pub restart_required: bool,
    #[serde(default)]
    pub runtime_policy_stamp: Option<serde_json::Value>,
}

pub struct SubProvidersUpsert;
impl Method for SubProvidersUpsert {
    const NAME: &'static str = "profile/sub_providers/upsert";
    type Params = SubProvidersUpsertParams;
    type Result = SubProvidersMutationResult;
}

/// `profile/sub_providers/remove` params (`research.ts:194`; octos-cli
/// `RawProfileSubProvidersRemoveParams`).
#[derive(Debug, Clone, Serialize)]
pub struct SubProvidersRemoveParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub key: String,
}

pub struct SubProvidersRemove;
impl Method for SubProvidersRemove {
    const NAME: &'static str = "profile/sub_providers/remove";
    type Params = SubProvidersRemoveParams;
    type Result = SubProvidersMutationResult;
}

/// No notifications in this domain (see the module docs): the profile and
/// onboarding extension surfaces are request-only.
pub fn register(_reg: &mut Registry, _store: Arc<Store>) {}

/// `profile/local/create` — onboard a local profile (card #13 §3).
///
/// **Truly missing** before this card: unlike `turn/start` / `session/hydrate`
/// there is NO typed transport command for it (`octos-app-transport`'s
/// `OutboundCommand` has no variant), so it must ride the client's generic
/// request path. The web calls it during onboarding
/// (`packages/client/src/client.ts`), and a fresh solo serve needs it before a
/// session can open. Params/result are the octos-core types.
pub struct ProfileLocalCreate;

impl Method for ProfileLocalCreate {
    const NAME: &'static str = methods::PROFILE_LOCAL_CREATE;
    type Params = octos_core::ui_protocol::ProfileLocalCreateParams;
    type Result = octos_core::ui_protocol::ProfileLocalCreateResult;
}
