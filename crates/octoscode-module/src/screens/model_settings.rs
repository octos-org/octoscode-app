//! A23 — parity rows 281/282: the Profile's model-provider management, ported
//! from the web's three layers so the native surfaces share ONE controller:
//!
//! - `features/models/model-settings.ts` — the controller: per-operation
//!   capabilities ([`Caps`]), the request shapes ([`selection`],
//!   [`route_selection`], [`provision`]), the operations ([`read`], [`test`],
//!   [`fetch_models`], [`save`], [`delete`]), and the rule that the API key is
//!   an operation ARGUMENT only: the controller draft ([`Draft`]) has no key
//!   field, and no controller state ever holds one (`model-settings.ts:55-64`,
//!   `:172-179`).
//! - `features/product-settings/model-management-projection.ts` — the
//!   projection ([`project`]): capability states (unavailable / loading /
//!   error / ready), fail-closed unread-vs-empty (an unread configuration is
//!   never an empty one, `:48-64`), and the per-row mutation safety
//!   (`editable` / `removable` / the read-only reasons, `:143-201`).
//! - `features/product-settings/ModelManagementSection.tsx` — the editor's
//!   pure half: the new-provider draft ([`create_draft`]), the configured
//!   draft ([`configured_draft`]) and the field validation ([`validate`]).
//!
//! The Core parse rules are the web client's (`packages/client/src/
//! onboarding.ts:298-640`): a configuration whose rows are malformed is an
//! invalid result (never a partial list), and a configured row that carries a
//! key the closed write schema cannot preserve is EDIT-BLOCKED
//! (`parseProfileLlmConfiguredModel`, `:459-470`): visible and deletable, its
//! editor refused ([`selection`] throws the web's sentence).
//!
//! ## Where each surface is drawn (the supervisor's board mapping)
//!
//! Every surface reuses an element an approved board already draws:
//!
//! | surface (web) | native | approved element |
//! |---|---|---|
//! | provider directory (`ModelManagementSection` rows) | `board3::routes` dialog | board 3 #4 Fleet card (name, status chip, mono model) + #1 MCP rows |
//! | Primary tag | `routes` chip | board 3 #5 blue "allowed" tag |
//! | credential indicator | `routes` dot + word | board 3 #1 MCP status dot ("● connected") |
//! | row read-only reason | `routes` lock row | board 3 #7 locked row ("This retained Session is closed.") |
//! | Add provider / Edit / Delete / Try again | `routes` pills | board 3 #4 black pill "Start", #9 outline "Preview"/"Download" |
//! | runtime warning | `routes` amber banner | board 3 #7 caution banner |
//! | loading / empty | `routes` grey lines | board 3 #4 "Loading models…", "No peers yet" |
//! | unavailable / read-only notice | `routes` + `provider` neutral callout | board 1 #4 "This server doesn't support pairing." |
//! | unread error + Try again | `routes` failure + outline pill | board 1 #3 red callout + #9 outline "Back to …" |
//! | add / edit editor | `provider` (A2's p4-06) | board 1 #6 Edit provider |
//! | family / protocol select | `provider` field + option list | board 3 #4 "Model" select + board 1 #6 Models check rows |
//! | fixed identity (edit) | `provider` key/value rows | board 1 #5 Connection rows + #8 grey footnote |
//! | test failure | `provider` red key outline + message | board 1 #7 Provider rejected |
//! | test success / fetch result | `provider` dot + word | board 3 #1 status dot |
//! | fetched models | `provider` Models list | board 1 #6 Models list + board 3 #1 group count |
//! | delete with typed phrase | `routes` delete card | board 3 #7 exact-match confirmation, board 1 #3 red tint |
use serde::Serialize;
use serde_json::{json, Map, Value};

/// The web's copy, verbatim.
pub mod copy {
    pub const TITLE: &str = "Model providers";
    pub const INTRO: &str = "Manage the active profile’s provider routes and credentials. Model selection and the current Session runtime remain separate.";
    pub const ADD_PROVIDER: &str = "Add provider";
    pub const READ_ONLY: &str = "Provider configuration is read-only on this server.";
    pub const RUNTIME_WARNING: &str = "Provider changes update the saved Profile configuration. Restart Octos before relying on route or credential changes in an already bootstrapped runtime.";
    pub const LOADING: &str = "Loading model providers…";
    pub const TRY_AGAIN: &str = "Try again";
    pub const UNAVAILABLE_READ: &str = "This Octos server cannot report the active Profile’s configured providers.";
    pub const NO_PROFILE: &str = "Octos did not identify an active Profile for model settings.";
    pub const LOAD_FAILED: &str = "Could not load the active Profile’s configured providers.";
    pub const EMPTY: &str = "No model providers configured";
    pub const EMPTY_HINT: &str = "Add a provider route to make a model available to Core.";
    pub const PRIMARY: &str = "Primary";
    pub const CREDENTIAL_CONFIGURED: &str = "Credential configured";
    pub const CREDENTIAL_MISSING: &str = "Credential not configured";
    pub const INCOMPLETE_IDENTITY: &str = "Core did not report a complete route identity. This entry is read-only.";
    pub const EDIT_BLOCKED: &str = "This entry contains settings the editor cannot preserve. Edit it through Core configuration instead.";
    pub const SELECTION_BLOCKED: &str = "This configured model contains settings this editor cannot preserve. Edit it through Core configuration instead.";
    pub const INFERENCE_UNSAFE: &str = "The configured model inference settings cannot be safely preserved.";
    pub const EDITOR_INTRO: &str = "Configure one Core model identity, endpoint route, and write-only credential.";
    pub const FIXED_PROVIDER: &str = "Provider identity is fixed. Add another provider to change it.";
    pub const FIXED_ROUTE: &str = "Route identity is fixed for existing configurations.";
    pub const ENV_HINT: &str = "Core stores the write-only key under this environment-name reference; the value itself is never read back.";
    pub const BASE_URL_HINT: &str = "Model discovery tests the unsaved endpoint shown here.";
    pub const KEY_KEEP_PLACEHOLDER: &str = "Leave blank to keep the configured key";
    pub const KEY_PLACEHOLDER: &str = "Enter API key";
    pub const KEY_CONFIGURED_HINT: &str = "Configured. Enter a value only to replace it; the stored key is never read back.";
    pub const KEY_WRITE_ONLY_HINT: &str = "Write-only. Sent to Core only when you test, fetch, or save.";
    pub const KEY_NONE: &str = "This provider uses its native or local authentication path.";
    pub const PROVIDER_CATALOG: &str = "Provider catalog";
    pub const FROM_ENDPOINT: &str = "Available from endpoint";
    pub const PARAMETERS_HEAD: &str = "Model request parameters";
    pub const PARAMETERS_BODY: &str = "Octos Core currently stores model identity, route, and credential only. Request parameters are read-only guidance here until Core advertises a writable parameter schema.";
    pub const COMPLETE_FIELDS: &str = "Complete the required provider fields to save.";
    pub const TEST: &str = "Test connection";
    pub const TESTING: &str = "Testing…";
    pub const FETCH: &str = "Fetch available models";
    pub const FETCHING: &str = "Fetching…";
    pub const SAVE: &str = "Save";
    pub const SAVING: &str = "Saving…";
    pub const CANCEL: &str = "Cancel";
    pub const CONNECTION_OK: &str = "Connection succeeded.";
    pub const CONNECTION_FAILED: &str = "Connection failed. Check the endpoint, protocol, model, and credential.";
    pub const NO_MODELS: &str = "The endpoint returned no models.";
    pub const FETCH_FAILED: &str = "Could not fetch available models. The unsaved provider draft was kept.";
    pub const SAVED: &str = "Provider saved.";
    pub const SAVED_EDIT: &str = "Provider saved. Restart Octos before relying on this route or credential change.";
    pub const SAVE_FAILED: &str = "Could not save this provider. The unsaved draft was kept.";
    pub const DELETED: &str = "Provider deleted. Restart Octos before relying on the updated runtime policy.";
    pub const DELETE_FAILED: &str = "Could not delete this provider. The configuration was kept; try again.";
    pub const OTHER_PROFILE: &str = "The model settings response belongs to another profile.";
    pub const OTHER_FAMILY: &str = "profile/llm/fetch_models returned another family";
    pub const TEST_DID_NOT_PASS: &str = "The provider test did not pass.";
    pub const NOT_SAVED: &str = "The server did not save the tested model configuration.";
    pub const GLM_TITLE: &str = "GLM-5.3-Flash recommended settings";
    pub const GLM_SUB: &str = "Read-only provider guidance";
    pub const GLM_LINK: &str = "Official guide";
    pub const GLM_URL: &str = "https://docs.bigmodel.cn/cn/guide/models/vlm/glm-5.3-flash";
    pub const GLM_NOTE: &str = "Z.AI requires thinking to stay enabled for this model and recommends preserving thinking across coding turns.";
    /// `GlmFlashGuidance` (`ModelManagementSection.tsx:419-426`).
    pub const GLM_GUIDANCE: [(&str, &str); 6] = [
        ("temperature", "1"),
        ("top_p", "0.95"),
        ("reasoning_effort", "max"),
        ("thinking.type", "enabled"),
        ("thinking.clear_thinking", "false"),
        ("stream + tool_stream", "true + true"),
    ];

    /// `"{count} available models found."`.
    pub fn models_found(n: usize) -> String {
        format!("{n} available models found.")
    }

    /// `fetchFailureMessage` (`model-settings.ts:603-609`).
    pub fn fetch_failure(reason: &str) -> String {
        match reason {
            "no_api_key" => "Add an API key before checking models.".to_owned(),
            "provider_unavailable" => "The provider did not return an available-model catalog.".to_owned(),
            other => format!("Could not check provider models: {}", super::redact(other, "")),
        }
    }
}

/// `MAX_TEXT` of the web parser (`onboarding.ts`) and the controller's 4096
/// cap (`model-settings.ts:617-629`).
const MAX_TEXT: usize = 4_096;
/// `MAX_MODELS` (`onboarding.ts`): a list longer than this is invalid.
const MAX_MODELS: usize = 1_000;

// ------------------------------------------------------------ capabilities

/// `ModelSettingsCapabilities` (`model-settings.ts:38-45`, `:156-170`): each
/// operation on its OWN advertised method; a missing method is never
/// inferred from another one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Caps {
    pub read: bool,
    pub catalog: bool,
    pub test: bool,
    pub save: bool,
    pub delete: bool,
    pub fetch_models: bool,
}

impl Caps {
    pub fn from_methods(methods: &[String]) -> Caps {
        let has = |m: &str| methods.iter().any(|x| x == m);
        Caps {
            read: has("profile/llm/list"),
            catalog: has("profile/llm/catalog"),
            test: has("profile/llm/test"),
            save: has("profile/llm/upsert"),
            delete: has("profile/llm/delete"),
            fetch_models: has("profile/llm/fetch_models"),
        }
    }

    /// Save is a test then an upsert (`ModelManagementSettings.tsx`:
    /// `onSave` only when `test && save`).
    pub fn can_save(&self) -> bool {
        self.test && self.save
    }
}

// ----------------------------------------------------------------- parsing

const INFERENCE_KEYS: [&str; 5] = ["temperature", "top_p", "context_window", "reasoning_effort", "model_hints"];
const ROUTE_KEYS: [&str; 5] = ["route_id", "label", "base_url", "api_key_env", "api_type"];
/// `CONFIGURED_KEYS` (`onboarding.ts`): rc11's configured row fields the
/// closed write schema can preserve.
const CONFIGURED_KEYS: [&str; 16] = [
    "provider",
    "model",
    "family_id",
    "model_id",
    "route",
    "route_id",
    "base_url",
    "api_key_env",
    "has_api_key",
    "selected",
    "available",
    "temperature",
    "top_p",
    "context_window",
    "reasoning_effort",
    "model_hints",
];
const HINT_BOOLEAN_KEYS: [&str; 4] = ["uses_completion_tokens", "fixed_temperature", "lacks_vision", "merge_system_messages"];
const REASONING_STYLES: [&str; 6] = [
    "none",
    "effort",
    "effort_and_thinking_toggle",
    "effort_max_only",
    "effort_low_high_max",
    "thinking_toggle",
];

/// `text()`: a non-empty string within the cap.
fn text(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty() && s.len() <= MAX_TEXT).map(str::to_owned)
}

/// `optionalText()`: absent / null / "" are absent (`Ok(None)`); any other
/// value must be a [`text`] (`Err` = invalid).
fn optional_text(v: Option<&Value>) -> Result<Option<String>, ()> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(other) => text(other).map(Some).ok_or(()),
    }
}

/// `parseLlmInferenceOverrides` (`onboarding.ts:537-610`): copies only the
/// Core-accepted fields; an explicit `null` is kept (null inherits, and a
/// round trip must not turn it into an override). `None` = invalid.
pub fn parse_inference(value: &Map<String, Value>) -> Option<Map<String, Value>> {
    if value.keys().any(|k| !INFERENCE_KEYS.contains(&k.as_str())) {
        return None;
    }
    let mut out = Map::new();
    for key in ["temperature", "top_p", "context_window"] {
        let Some(item) = value.get(key) else { continue };
        if item.is_null() {
            out.insert(key.into(), Value::Null);
            continue;
        }
        let n = item.as_f64().filter(|n| n.is_finite())?;
        let ok = if key == "context_window" {
            item.as_u64().is_some_and(|i| (1..=0xffff_ffff).contains(&i))
        } else {
            n >= 0.0 && n <= if key == "temperature" { 2.0 } else { 1.0 }
        };
        if !ok {
            return None;
        }
        out.insert(key.into(), item.clone());
    }
    if let Some(item) = value.get("reasoning_effort") {
        let ok = item.is_null() || matches!(item.as_str(), Some("none" | "low" | "medium" | "high" | "max"));
        if !ok {
            return None;
        }
        out.insert("reasoning_effort".into(), item.clone());
    }
    if let Some(hints) = value.get("model_hints") {
        if hints.is_null() {
            out.insert("model_hints".into(), Value::Null);
        } else {
            let h = hints.as_object()?;
            if h.keys().any(|k| k != "reasoning_style" && !HINT_BOOLEAN_KEYS.contains(&k.as_str())) {
                return None;
            }
            let mut parsed = Map::new();
            for key in HINT_BOOLEAN_KEYS {
                if let Some(item) = h.get(key) {
                    parsed.insert(key.into(), Value::Bool(item.as_bool()?));
                }
            }
            if let Some(style) = h.get("reasoning_style") {
                let s = style.as_str().filter(|s| REASONING_STYLES.contains(s))?;
                parsed.insert("reasoning_style".into(), Value::String(s.to_owned()));
            }
            out.insert("model_hints".into(), Value::Object(parsed));
        }
    }
    Some(out)
}

/// One configured route as Core reports it (`ProfileLlmConfiguredRoute`).
/// An empty string is an absent field.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfiguredRoute {
    pub route_id: String,
    pub label: String,
    pub base_url: String,
    pub api_key_env: String,
    pub api_type: String,
}

/// One configured primary or fallback (`ProfileLlmConfiguredModel`).
#[derive(Debug, Clone, PartialEq)]
pub struct ConfiguredModel {
    pub family_id: String,
    pub model_id: String,
    pub route: ConfiguredRoute,
    pub has_api_key: bool,
    pub selected: bool,
    pub available: bool,
    /// The inference overrides PRESENT on the row (an explicit null kept).
    pub inference: Map<String, Value>,
    /// The row carries configuration the closed write schema cannot preserve.
    pub edit_blocked: bool,
}

/// `ProfileLlmConfigResult` (also the delete receipt's configuration).
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub profile_id: String,
    pub primary: Option<ConfiguredModel>,
    pub fallbacks: Vec<ConfiguredModel>,
}

impl Config {
    /// Primary first, then the fallbacks (`model-management-projection.ts:28-33`).
    pub fn rows(&self) -> Vec<&ConfiguredModel> {
        self.primary.iter().chain(self.fallbacks.iter()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.primary.is_none() && self.fallbacks.is_empty()
    }
}

/// `parseProfileLlmConfiguredModel` (`onboarding.ts:438-490`).
pub fn parse_configured(value: &Value) -> Option<ConfiguredModel> {
    let v = value.as_object()?;
    let route = v.get("route")?.as_object()?;
    let family_id = text(v.get("family_id")?)?;
    let model_id = text(v.get("model_id")?)?;
    let has_api_key = v.get("has_api_key")?.as_bool()?;
    let selected = v.get("selected")?.as_bool()?;
    let available = v.get("available")?.as_bool()?;
    let field = |k: &str| optional_text(route.get(k));
    let r = ConfiguredRoute {
        route_id: field("route_id").ok()?.unwrap_or_default(),
        label: field("label").ok()?.unwrap_or_default(),
        base_url: field("base_url").ok()?.unwrap_or_default(),
        api_key_env: field("api_key_env").ok()?.unwrap_or_default(),
        api_type: field("api_type").ok()?.unwrap_or_default(),
    };
    let present: Map<String, Value> = INFERENCE_KEYS
        .iter()
        .filter_map(|k| v.get(*k).map(|x| ((*k).to_owned(), x.clone())))
        .collect();
    let inference = parse_inference(&present)?;
    let edit_blocked = v.iter().any(|(k, x)| {
        !CONFIGURED_KEYS.contains(&k.as_str()) && !((k == "cost_per_m" || k == "strong") && x.is_null())
    }) || route.keys().any(|k| !ROUTE_KEYS.contains(&k.as_str()));
    Some(ConfiguredModel { family_id, model_id, route: r, has_api_key, selected, available, inference, edit_blocked })
}

/// `parseProfileLlmConfig` (`onboarding.ts:419-436`): `None` = an invalid
/// result (the client's "returned an invalid result").
pub fn parse_config(value: &Value) -> Option<Config> {
    let v = value.as_object()?;
    let fallbacks = v.get("fallbacks")?.as_array()?;
    let profile_id = text(v.get("profile_id")?)?;
    if fallbacks.len() > MAX_MODELS {
        return None;
    }
    let primary = match v.get("primary") {
        Some(Value::Null) => None,
        Some(p) => {
            let p = parse_configured(p)?;
            if !p.selected {
                return None;
            }
            Some(p)
        }
        // `value.primary !== null` with the key absent parses `undefined`.
        None => return None,
    };
    let mut out = Vec::with_capacity(fallbacks.len());
    for f in fallbacks {
        let f = parse_configured(f)?;
        if f.selected {
            return None;
        }
        out.push(f);
    }
    Some(Config { profile_id, primary, fallbacks: out })
}

/// `parseProfileLlmDeleteResult`: `applied` must be a boolean, then the
/// configuration.
pub fn parse_delete(value: &Value) -> Option<(Config, bool)> {
    let applied = value.get("applied")?.as_bool()?;
    Some((parse_config(value)?, applied))
}

// -------------------------------------------------------------- projection

/// `ModelProviderRoute` (`ModelManagementSection.tsx:26-36`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ProviderRoute {
    pub id: String,
    pub label: String,
    /// Empty asks Core to use its provider default.
    pub base_url: String,
    pub api_protocol: String,
    pub api_key_env: String,
}

/// `ModelCatalogSuggestion`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub id: String,
    pub label: String,
    pub route: Option<ProviderRoute>,
}

/// `ModelCredentialRequirement`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Credential {
    Required,
    Optional,
    None,
}

/// `ModelProviderFamilyOption`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Family {
    pub id: String,
    pub label: String,
    pub env: String,
    pub models: Vec<Suggestion>,
    pub default_route: Option<ProviderRoute>,
    pub credential: Credential,
    pub requires_base_url: bool,
}

/// `ConfiguredModelProvider` — value-safe: a credential BOOLEAN, never a key.
#[derive(Debug, Clone, PartialEq)]
pub struct Provider {
    pub id: String,
    pub family_id: String,
    pub family_label: String,
    pub model_id: String,
    pub model_label: String,
    pub route: ProviderRoute,
    pub api_key_configured: bool,
    pub primary: bool,
    pub editable: bool,
    pub removable: bool,
    pub reason: Option<&'static str>,
    pub inference: Map<String, Value>,
    pub edit_blocked: bool,
}

impl Provider {
    /// `providerName`.
    pub fn name(&self) -> &str {
        if self.model_label.trim().is_empty() {
            &self.model_id
        } else {
            &self.model_label
        }
    }

    /// `providerDeleteConfirmation`: exact and case-sensitive.
    pub fn delete_phrase(&self) -> String {
        format!("DELETE {}/{}", self.family_id, self.model_id)
    }
}

/// `ModelManagementState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewState {
    Ready,
    Loading,
    Error(String),
    Unavailable(String),
}

/// `ModelManagementProjection`.
#[derive(Debug, Clone, PartialEq)]
pub struct Projection {
    pub state: ViewState,
    pub providers: Vec<Provider>,
    pub families: Vec<Family>,
    /// `(id, label)`, "openai" first.
    pub protocols: Vec<(String, String)>,
}

fn title_case(value: &str) -> String {
    value
        .split(['-', '_'])
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `familyLabel`.
pub fn family_label(id: &str) -> String {
    match id {
        "zai" => "Z.AI".into(),
        "zai-coding" => "Z.AI Coding Plan".into(),
        "deepseek" => "DeepSeek".into(),
        "ollama" => "Ollama".into(),
        "openai" => "OpenAI".into(),
        "anthropic" => "Anthropic".into(),
        other => title_case(other),
    }
}

/// `modelLabel`.
pub fn model_label(id: &str) -> String {
    if id == "glm-5.3-flash" {
        return "GLM-5.3-Flash".into();
    }
    id.split('-')
        .map(|part| {
            if part.eq_ignore_ascii_case("glm") {
                "GLM".to_owned()
            } else if !part.is_empty() && part.split('.').all(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())) {
                part.to_owned()
            } else {
                let mut c = part.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `routeLabel`.
pub fn route_label(id: &str) -> String {
    if id == "official" {
        "Official".into()
    } else {
        title_case(id)
    }
}

/// `protocolLabel`.
pub fn protocol_label(id: &str) -> String {
    match id {
        "openai" => "OpenAI-compatible".into(),
        "anthropic" => "Anthropic-compatible".into(),
        other => title_case(other),
    }
}

/// `projectFamily` (`model-management-projection.ts:111-141`).
pub fn project_family(family: &octoscode_client::domains::profile::LlmCatalogFamily) -> Family {
    let models: Vec<Suggestion> = family
        .models
        .iter()
        .map(|m| Suggestion {
            id: m.id.clone(),
            label: model_label(&m.id),
            route: m.endpoints.first().map(|e| ProviderRoute {
                id: e.id.clone(),
                label: e.label.clone().filter(|l| !l.is_empty()).unwrap_or_else(|| route_label(&e.id)),
                base_url: e.base_url.clone().unwrap_or_default(),
                api_protocol: e.api_type.clone().unwrap_or_else(|| "openai".into()),
                api_key_env: e.api_key_env.clone().unwrap_or_else(|| family.env.clone()),
            }),
        })
        .collect();
    let default_route = models.iter().find_map(|m| m.route.clone());
    Family {
        id: family.id.clone(),
        label: family_label(&family.id),
        env: family.env.clone(),
        models,
        default_route,
        credential: if family.env.is_empty() { Credential::None } else { Credential::Required },
        requires_base_url: false,
    }
}

/// `projectConfiguredModel` (`model-management-projection.ts:143-201`).
pub fn project_configured(model: &ConfiguredModel, families: &[Family]) -> Provider {
    let family = families.iter().find(|f| f.id == model.family_id);
    let route_id = model.route.route_id.trim().to_owned();
    let api_protocol = model.route.api_type.trim().to_owned();
    let mutation_safe = !route_id.is_empty() && !api_protocol.is_empty();
    let route = ProviderRoute {
        id: route_id.clone(),
        label: if !model.route.label.is_empty() {
            model.route.label.clone()
        } else if !route_id.is_empty() {
            route_label(&route_id)
        } else {
            "Route identity unavailable".into()
        },
        base_url: model.route.base_url.clone(),
        api_protocol,
        api_key_env: model.route.api_key_env.clone(),
    };
    let id = format!(
        "{}:{}:{}",
        model.family_id,
        model.model_id,
        if route_id.is_empty() { "unresolved-route" } else { &route_id }
    );
    Provider {
        id,
        family_id: model.family_id.clone(),
        family_label: family.map(|f| f.label.clone()).unwrap_or_else(|| family_label(&model.family_id)),
        model_id: model.model_id.clone(),
        model_label: model_label(&model.model_id),
        route,
        api_key_configured: model.has_api_key,
        primary: model.selected,
        editable: model.available && mutation_safe && !model.edit_blocked,
        removable: mutation_safe,
        reason: if !mutation_safe {
            Some(copy::INCOMPLETE_IDENTITY)
        } else if model.edit_blocked {
            Some(copy::EDIT_BLOCKED)
        } else {
            None
        },
        inference: model.inference.clone(),
        edit_blocked: model.edit_blocked,
    }
}

/// `projectModelManagement` (`model-management-projection.ts:24-75`): the
/// view state fails CLOSED — no `read` method is "unavailable", a first read
/// in flight is "loading", and an UNREAD configuration is an error (with the
/// controller's error or the web's fallback), never an empty list.
pub fn project(
    caps: Caps,
    loading: bool,
    config: Option<&Config>,
    catalog: Option<&octoscode_client::domains::profile::LlmCatalogResult>,
    error: Option<&str>,
) -> Projection {
    let families: Vec<Family> = catalog.map(|c| c.families.iter().map(project_family).collect()).unwrap_or_default();
    let providers: Vec<Provider> =
        config.map(|c| c.rows().into_iter().map(|m| project_configured(m, &families)).collect()).unwrap_or_default();
    let mut ids: Vec<String> = vec!["openai".into()];
    let mut add = |id: &str| {
        if !id.is_empty() && !ids.iter().any(|x| x == id) {
            ids.push(id.to_owned());
        }
    };
    for f in &families {
        for m in &f.models {
            if let Some(r) = &m.route {
                add(&r.api_protocol);
            }
        }
    }
    for p in &providers {
        add(&p.route.api_protocol);
    }
    let protocols = ids.iter().map(|id| (id.clone(), protocol_label(id))).collect();
    let state = if !caps.read {
        ViewState::Unavailable(copy::UNAVAILABLE_READ.into())
    } else if loading && config.is_none() {
        ViewState::Loading
    } else if config.is_none() {
        ViewState::Error(error.map(str::to_owned).unwrap_or_else(|| copy::LOAD_FAILED.into()))
    } else {
        ViewState::Ready
    };
    Projection { state, providers, families, protocols }
}

// ------------------------------------------------------------ editor draft

/// `ModelProviderDraft` — the EDITOR's unsaved state. Its `api_key` is the
/// write-only replacement the person typed; it never enters a [`Draft`].
/// `Debug` never prints it.
#[derive(Clone, Default, PartialEq)]
pub struct EditorDraft {
    pub family_id: String,
    pub model_id: String,
    pub route: ProviderRoute,
    pub api_key: String,
    pub inference: Option<Map<String, Value>>,
    pub edit_blocked: bool,
}

impl std::fmt::Debug for EditorDraft {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorDraft")
            .field("family_id", &self.family_id)
            .field("model_id", &self.model_id)
            .field("route", &self.route)
            .field("api_key", &if self.api_key.is_empty() { "" } else { "[redacted]" })
            .field("inference", &self.inference)
            .field("edit_blocked", &self.edit_blocked)
            .finish()
    }
}

/// `createModelProviderDraft` (`ModelManagementSection.tsx:168-203`): the
/// first catalog model whose identity is NOT configured yet — an existing
/// Core identity is never offered as a new provider.
pub fn create_draft(families: &[Family], protocols: &[(String, String)], configured: &[Provider]) -> EditorDraft {
    let available = families.iter().flat_map(|f| f.models.iter().map(move |s| (f, s))).find(|(f, s)| {
        let route = s.route.as_ref().or(f.default_route.as_ref());
        let rid = route.map(|r| r.id.clone()).unwrap_or_else(|| f.id.clone());
        !configured
            .iter()
            .any(|p| p.family_id == f.id && p.model_id == s.id && (p.route.id.is_empty() || p.route.id == rid))
    });
    let family = available.map(|(f, _)| f).or(families.first());
    let suggestion = available.map(|(_, s)| s);
    let route = suggestion.and_then(|s| s.route.clone()).or_else(|| family.and_then(|f| f.default_route.clone()));
    EditorDraft {
        family_id: family.map(|f| f.id.clone()).unwrap_or_default(),
        model_id: suggestion.map(|s| s.id.clone()).unwrap_or_default(),
        route: route.unwrap_or_else(|| ProviderRoute {
            id: family.map(|f| f.id.clone()).unwrap_or_default(),
            label: family.map(|f| f.label.clone()).unwrap_or_default(),
            base_url: String::new(),
            api_protocol: protocols.first().map(|p| p.0.clone()).unwrap_or_default(),
            // The family's own credential reference (`projectFamily` falls
            // back to `family.env` for every catalog route).
            api_key_env: family.map(|f| f.env.clone()).unwrap_or_default(),
        }),
        api_key: String::new(),
        inference: None,
        edit_blocked: false,
    }
}

/// `configuredProviderDraft` (`ModelManagementSection.tsx:205-219`): the
/// configured inference values are carried (never offered as new controls),
/// and the credential is write-only — never a fake value.
pub fn configured_draft(p: &Provider) -> EditorDraft {
    EditorDraft {
        family_id: p.family_id.clone(),
        model_id: p.model_id.clone(),
        route: p.route.clone(),
        api_key: String::new(),
        inference: Some(p.inference.clone()),
        edit_blocked: p.edit_blocked,
    }
}

/// The editor's fields (`ModelProviderDraftIssue.field`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    FamilyId,
    ModelId,
    RouteId,
    RouteLabel,
    BaseUrl,
    ApiProtocol,
    ApiKeyEnv,
    ApiKey,
}

/// `ModelProviderDraftIssue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Issue {
    pub field: Field,
    pub message: &'static str,
}

/// The validation options (`validateModelProviderDraft`'s second argument).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    pub credential_configured: bool,
    pub credential: Credential,
    pub requires_base_url: bool,
    /// `None` = not checked (a probe); `Some(false)` = the identity exists.
    pub identity_available: Option<bool>,
}

fn is_http_url(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    let rest = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://"));
    rest.is_some_and(|r| {
        let host = r.split(['/', '?', '#']).next().unwrap_or("");
        !host.is_empty() && !host.contains(char::is_whitespace)
    })
}

/// `validateModelProviderDraft` (`ModelManagementSection.tsx:221-286`).
pub fn validate(d: &EditorDraft, rules: Rules) -> Vec<Issue> {
    let mut out = Vec::new();
    let mut push = |field, message| out.push(Issue { field, message });
    if d.family_id.trim().is_empty() {
        push(Field::FamilyId, "Choose a provider family.");
    }
    if d.model_id.trim().is_empty() {
        push(Field::ModelId, "Enter a model ID.");
    } else if rules.identity_available == Some(false) {
        push(Field::ModelId, "This model route already exists. Use Edit instead.");
    }
    if d.route.id.trim().is_empty() {
        push(Field::RouteId, "Enter a route ID.");
    }
    if d.route.label.trim().is_empty() {
        push(Field::RouteLabel, "Enter a route label.");
    }
    let base = d.route.base_url.trim();
    if rules.requires_base_url && base.is_empty() {
        push(Field::BaseUrl, "Enter a base URL.");
    } else if !base.is_empty() && !is_http_url(base) {
        push(Field::BaseUrl, "Base URL must use http or https.");
    }
    if d.route.api_protocol.trim().is_empty() {
        push(Field::ApiProtocol, "Choose an API protocol.");
    }
    if rules.credential != Credential::None && d.route.api_key_env.trim().is_empty() {
        push(Field::ApiKeyEnv, "Enter a credential environment name.");
    }
    if rules.credential == Credential::Required && !rules.credential_configured && d.api_key.trim().is_empty() {
        push(Field::ApiKey, "Enter an API key.");
    }
    if d.api_key.contains(['\r', '\n']) {
        push(Field::ApiKey, "API key must be a single line.");
    }
    out
}

/// `probeIssues`: a probe (Fetch) ignores the model id and the label.
pub fn probe_issues(issues: &[Issue]) -> Vec<Issue> {
    issues.iter().copied().filter(|i| !matches!(i.field, Field::ModelId | Field::RouteLabel)).collect()
}

/// `configuredIdentityExists` (`ModelManagementSection.tsx:309-323`).
pub fn identity_exists(providers: &[Provider], editing: Option<&str>, d: &EditorDraft) -> bool {
    let (family, model, route) = (d.family_id.trim(), d.model_id.trim(), d.route.id.trim());
    providers.iter().any(|p| {
        Some(p.id.as_str()) != editing
            && p.family_id == family
            && p.model_id == model
            && (p.route.id.is_empty() || p.route.id == route)
    })
}

// -------------------------------------------------------- controller draft

/// `ModelRouteDraft`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RouteDraft {
    pub route_id: String,
    pub label: String,
    pub base_url: String,
    pub api_key_env: String,
    pub api_type: String,
}

/// `ModelSettingsDraft` (`model-settings.ts:55-64`): "A provider/model draft
/// deliberately contains no raw credential field". There is no key field to
/// fill: the key reaches an operation only as its argument.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Draft {
    pub family_id: String,
    pub model_id: String,
    pub route: RouteDraft,
    pub set_primary: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inference: Option<Map<String, Value>>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub edit_blocked: bool,
}

impl Draft {
    /// `modelSettingsDraftFromProvider` (`model-management-projection.ts:77-95`):
    /// the editor's api_key is NOT copied.
    pub fn from_editor(d: &EditorDraft, set_primary: bool) -> Draft {
        Draft {
            family_id: d.family_id.clone(),
            model_id: d.model_id.clone(),
            route: RouteDraft {
                route_id: d.route.id.clone(),
                label: d.route.label.clone(),
                base_url: d.route.base_url.clone(),
                api_key_env: d.route.api_key_env.clone(),
                api_type: d.route.api_protocol.clone(),
            },
            set_primary,
            inference: d.inference.clone(),
            edit_blocked: d.edit_blocked,
        }
    }

    /// `modelSettingsDeleteTarget`.
    pub fn delete_target(p: &Provider) -> Draft {
        Draft::from_editor(&EditorDraft { family_id: p.family_id.clone(), model_id: p.model_id.clone(), route: p.route.clone(), ..Default::default() }, false)
    }
}

fn required_text(value: &str, label: &str) -> Result<String, String> {
    let v = value.trim();
    if v.is_empty() {
        return Err(format!("{label} is required."));
    }
    if v.len() > MAX_TEXT {
        return Err(format!("{label} is too long."));
    }
    Ok(v.to_owned())
}

fn optional(value: &str) -> Result<Option<String>, String> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(None);
    }
    if v.len() > MAX_TEXT {
        return Err("Model setting is too long.".into());
    }
    Ok(Some(v.to_owned()))
}

/// `routeSelection` (`model-settings.ts:557-570`).
pub fn route_selection(route: &RouteDraft) -> Result<Value, String> {
    let route_id = required_text(&route.route_id, "Provider route")?;
    let mut r = Map::new();
    r.insert("route_id".into(), json!(route_id));
    if let Some(l) = optional(&route.label)? {
        r.insert("label".into(), json!(l));
    }
    if let Some(b) = optional(&route.base_url)? {
        r.insert("base_url".into(), json!(b));
    }
    r.insert("api_key_env".into(), json!(optional(&route.api_key_env)?.unwrap_or_default()));
    r.insert("api_type".into(), json!(optional(&route.api_type)?.unwrap_or_else(|| "openai".into())));
    Ok(Value::Object(r))
}

/// `selectionFromModelSettingsDraft` (`model-settings.ts:503-521`): an
/// edit-blocked draft is refused, the configured inference values are
/// carried exactly (null included), nothing else is added.
pub fn selection(d: &Draft) -> Result<Value, String> {
    if d.edit_blocked {
        return Err(copy::SELECTION_BLOCKED.into());
    }
    let inference = parse_inference(d.inference.as_ref().unwrap_or(&Map::new())).ok_or_else(|| copy::INFERENCE_UNSAFE.to_owned())?;
    let mut s = inference;
    s.insert("family_id".into(), json!(required_text(&d.family_id, "Provider family")?));
    s.insert("model_id".into(), json!(required_text(&d.model_id, "Model")?));
    s.insert("route".into(), route_selection(&d.route)?);
    Ok(Value::Object(s))
}

/// `normalizedSecret`.
fn secret(api_key: Option<&str>) -> Option<String> {
    api_key.map(str::trim).filter(|k| !k.is_empty()).map(str::to_owned)
}

/// `provisionParams` (`model-settings.ts:545-555`): the request a test or an
/// upsert sends — the ONLY place the key is joined to a draft, and it lives
/// no longer than the request.
pub fn provision(profile_id: &str, d: &Draft, api_key: Option<&str>) -> Result<Value, String> {
    let mut p = json!({ "profile_id": profile_id, "selection": selection(d)? });
    if let Some(k) = secret(api_key) {
        p["api_key"] = json!(k);
    }
    Ok(p)
}

/// `redactModelSettingsError` (`model-settings.ts:523-543`).
pub fn redact(reason: &str, secret: &str) -> String {
    crate::screens::provider::redact(reason, secret)
}

// --------------------------------------------------------------- operations

/// A `profile/llm/test` result made safe (`safeTestResult`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestResult {
    pub applied: bool,
    pub message: String,
    pub error: Option<String>,
}

impl TestResult {
    /// `ModelManagementSettings.testProvider`: anything but an applied,
    /// error-free result is a failure.
    pub fn passed(&self) -> bool {
        self.applied && self.error.is_none()
    }
}

/// A `profile/llm/fetch_models` result (`ModelSettingsFetchResult`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchResult {
    pub models: Vec<String>,
    pub reason: Option<String>,
}

/// What a save returns (`ModelSettingsSaveResult` + the re-read).
#[derive(Debug, Clone, PartialEq)]
pub struct SaveResult {
    pub test: TestResult,
    pub applied: bool,
    /// The configuration re-read after the mutation (`None` = not read).
    pub config: Option<Config>,
    /// "Saved, but could not refresh model settings: …".
    pub refresh_error: Option<String>,
}

fn client_error(e: &octoscode_client::ClientError, key: &str) -> String {
    redact(&crate::screens::dialog::display_error(&e.to_string()), key)
}

fn assert_profile(received: &Value, expected: &str) -> Result<(), String> {
    if received.get("profile_id") != Some(&json!(expected)) {
        return Err(copy::OTHER_PROFILE.into());
    }
    Ok(())
}

/// `refresh` (`model-settings.ts:211-257`): the configuration and the catalog
/// read side by side, each on its own advertised method. A failed read
/// leaves its half `Err` — the caller keeps the previous value (an unread
/// configuration stays unread, never empty).
pub async fn read(
    client: &octoscode_client::Client,
    profile_id: &str,
    caps: Caps,
) -> (Option<Result<Config, String>>, Option<Result<octoscode_client::domains::profile::LlmCatalogResult, String>>) {
    let config = async {
        if !caps.read {
            return None;
        }
        Some(match client.request("profile/llm/list", json!({ "profile_id": profile_id })).await {
            Ok(v) => assert_profile(&v, profile_id)
                .and_then(|_| parse_config(&v).ok_or_else(|| "profile/llm/list returned an invalid result".to_owned())),
            Err(e) => Err(client_error(&e, "")),
        })
    };
    let catalog = async {
        if !caps.catalog {
            return None;
        }
        Some(match client.request("profile/llm/catalog", json!({})).await {
            Ok(v) => serde_json::from_value::<octoscode_client::domains::profile::LlmCatalogResult>(v)
                .map_err(|_| "profile/llm/catalog returned an invalid result".to_owned()),
            Err(e) => Err(client_error(&e, "")),
        })
    };
    tokio::join!(config, catalog)
}

/// `test` (`model-settings.ts:259-291`).
pub async fn test(client: &octoscode_client::Client, profile_id: &str, d: &Draft, api_key: Option<&str>) -> Result<TestResult, String> {
    let key = secret(api_key).unwrap_or_default();
    let params = provision(profile_id, d, api_key)?;
    let v = client.request("profile/llm/test", params).await.map_err(|e| client_error(&e, &key))?;
    assert_profile(&v, profile_id)?;
    Ok(TestResult {
        applied: v["applied"] == json!(true),
        message: redact(v["message"].as_str().unwrap_or(""), &key),
        error: v["error"].as_str().filter(|e| !e.is_empty()).map(|e| redact(e, &key)),
    })
}

/// `fetchModels` (`model-settings.ts:293-343`): family + route, no model.
pub async fn fetch_models(client: &octoscode_client::Client, profile_id: &str, d: &Draft, api_key: Option<&str>) -> Result<FetchResult, String> {
    let key = secret(api_key).unwrap_or_default();
    let family = required_text(&d.family_id, "Provider family")?;
    let mut params = json!({ "profile_id": profile_id, "selection": { "family_id": family, "route": route_selection(&d.route)? } });
    if !key.is_empty() {
        params["api_key"] = json!(key);
    }
    let v = client.request("profile/llm/fetch_models", params).await.map_err(|e| client_error(&e, &key))?;
    assert_profile(&v, profile_id)?;
    if v.get("family_id") != Some(&json!(family)) {
        return Err(copy::OTHER_FAMILY.into());
    }
    let models: Vec<String> = v["models"].as_array().into_iter().flatten().filter_map(|m| text(m)).take(MAX_MODELS).collect();
    Ok(FetchResult { models, reason: v["reason"].as_str().filter(|r| !r.is_empty()).map(|r| redact(r, &key)) })
}

/// `save` (`model-settings.ts:345-400`): the provision is built ONCE (test and
/// upsert cannot drift), the upsert runs only after a passing test, then the
/// configuration is re-read.
pub async fn save(client: &octoscode_client::Client, profile_id: &str, d: &Draft, api_key: Option<&str>, caps: Caps) -> Result<SaveResult, String> {
    let key = secret(api_key).unwrap_or_default();
    let provision = provision(profile_id, d, api_key)?;
    let t = client.request("profile/llm/test", provision.clone()).await.map_err(|e| client_error(&e, &key))?;
    assert_profile(&t, profile_id)?;
    let tested = TestResult {
        applied: t["applied"] == json!(true),
        message: redact(t["message"].as_str().unwrap_or(""), &key),
        error: t["error"].as_str().filter(|e| !e.is_empty()).map(|e| redact(e, &key)),
    };
    if !tested.passed() {
        let why = tested.error.clone().or_else(|| Some(tested.message.clone()).filter(|m| !m.is_empty()));
        return Err(why.unwrap_or_else(|| copy::TEST_DID_NOT_PASS.into()));
    }
    let mut upsert = provision;
    upsert["set_primary"] = json!(d.set_primary);
    let u = client.request("profile/llm/upsert", upsert).await.map_err(|e| client_error(&e, &key))?;
    assert_profile(&u, profile_id)?;
    if u["applied"] != json!(true) {
        return Err(copy::NOT_SAVED.into());
    }
    let (config, refresh_error) = if caps.read {
        match read(client, profile_id, Caps { catalog: false, ..caps }).await.0 {
            Some(Ok(c)) => (Some(c), None),
            Some(Err(e)) => (None, Some(format!("Saved, but could not refresh model settings: {e}"))),
            None => (None, None),
        }
    } else {
        (None, None)
    };
    Ok(SaveResult { test: tested, applied: true, config, refresh_error })
}

/// `delete` (`model-settings.ts:402-433`): the receipt's configuration
/// replaces the list.
pub async fn delete(client: &octoscode_client::Client, profile_id: &str, d: &Draft) -> Result<(Config, bool), String> {
    let params = json!({
        "profile_id": profile_id,
        "family_id": required_text(&d.family_id, "Provider family")?,
        "model_id": required_text(&d.model_id, "Model")?,
        "route_id": required_text(&d.route.route_id, "Provider route")?,
    });
    let v = client.request("profile/llm/delete", params).await.map_err(|e| client_error(&e, ""))?;
    assert_profile(&v, profile_id)?;
    parse_delete(&v).ok_or_else(|| "profile/llm/delete returned an invalid result".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps_all() -> Caps {
        Caps { read: true, catalog: true, test: true, save: true, delete: true, fetch_models: true }
    }

    fn zai_family() -> Family {
        let route = ProviderRoute {
            id: "coding-plan".into(),
            label: "Z.AI Coding Plan".into(),
            base_url: "https://api.z.ai/api/coding/paas/v4".into(),
            api_protocol: "openai-chat-completions".into(),
            api_key_env: "ZAI_API_KEY".into(),
        };
        Family {
            id: "zai".into(),
            label: "Z.AI".into(),
            env: "ZAI_API_KEY".into(),
            models: vec![Suggestion { id: "glm-5.3-flash".into(), label: "GLM-5.3-Flash".into(), route: Some(route.clone()) }],
            default_route: Some(route),
            credential: Credential::Required,
            requires_base_url: true,
        }
    }

    fn configured_row(extra: Value) -> Value {
        let mut row = json!({"family_id": "zai", "model_id": "model", "route": {"route_id": "official", "api_type": "openai"},
            "has_api_key": true, "selected": true, "available": true});
        for (k, v) in extra.as_object().unwrap() {
            row[k] = v.clone();
        }
        row
    }

    /// model-management-projection.test.ts:11 — an unknown configured key
    /// blocks the edit but keeps the row visible and deletable; the
    /// selection refuses it.
    #[test]
    fn blocks_unknown_configured_edits_but_keeps_the_row_visible_and_deletable() {
        let c = parse_config(&json!({"profile_id": "coding", "primary": configured_row(json!({"strong": false})), "fallbacks": []})).unwrap();
        let p = project(Caps { catalog: false, fetch_models: false, ..caps_all() }, false, Some(&c), None, None);
        let row = &p.providers[0];
        assert!(!row.editable && row.removable && row.edit_blocked);
        assert_eq!(row.reason, Some(copy::EDIT_BLOCKED));
        let err = selection(&Draft::from_editor(&configured_draft(row), true)).unwrap_err();
        assert!(err.contains("cannot preserve"), "{err}");
        // A null `strong` / `cost_per_m` is preserved (rc11's redundant fields).
        let c = parse_config(&json!({"profile_id": "coding", "primary": configured_row(json!({"strong": null, "cost_per_m": null})), "fallbacks": []})).unwrap();
        assert!(!c.primary.unwrap().edit_blocked);
        // An unknown ROUTE key blocks too.
        let mut row = configured_row(json!({}));
        row["route"]["weight"] = json!(2);
        let c = parse_config(&json!({"profile_id": "coding", "primary": row, "fallbacks": []})).unwrap();
        assert!(c.primary.unwrap().edit_blocked);
    }

    /// model-management-projection.test.ts:53 — a label-only edit retains
    /// every configured inference value, null included, and adds none.
    #[test]
    fn retains_configured_inference_values_through_a_label_only_existing_edit() {
        let overrides = json!({"temperature": 0, "top_p": null, "context_window": 131072, "reasoning_effort": "max",
            "model_hints": {"fixed_temperature": false, "reasoning_style": "effort_low_high_max"}});
        let c = parse_config(&json!({"profile_id": "coding", "primary": configured_row(overrides.clone()), "fallbacks": []})).unwrap();
        let p = project(Caps { catalog: false, fetch_models: false, ..caps_all() }, false, Some(&c), None, None);
        let mut d = configured_draft(&p.providers[0]);
        d.route.label = "Renamed only".into();
        let s = selection(&Draft::from_editor(&d, true)).unwrap();
        for (k, v) in overrides.as_object().unwrap() {
            assert_eq!(s.get(k), Some(v), "{k}");
        }
        assert_eq!(s["route"]["label"], "Renamed only");
        assert!(s.get("max_output_tokens").is_none());
    }

    /// model-management-projection.test.ts:104 — the API key stays outside
    /// the controller draft.
    #[test]
    fn keeps_the_api_key_outside_the_controller_draft() {
        let d = Draft::from_editor(
            &EditorDraft {
                family_id: "zai".into(),
                model_id: "glm-5.3-flash".into(),
                route: ProviderRoute {
                    id: "official".into(),
                    label: "Z.AI".into(),
                    base_url: "https://api.z.ai/api/paas/v4".into(),
                    api_protocol: "openai".into(),
                    api_key_env: "ZAI_API_KEY".into(),
                },
                api_key: "not-published".into(),
                ..Default::default()
            },
            false,
        );
        assert_eq!(
            serde_json::to_value(&d).unwrap(),
            json!({"family_id": "zai", "model_id": "glm-5.3-flash", "set_primary": false,
                "route": {"route_id": "official", "label": "Z.AI", "base_url": "https://api.z.ai/api/paas/v4", "api_key_env": "ZAI_API_KEY", "api_type": "openai"}})
        );
        assert!(!format!("{d:?}").contains("not-published"));
        assert!(!format!("{:?}", EditorDraft { api_key: "not-published".into(), ..Default::default() }).contains("not-published"));
    }

    /// model-management-projection.test.ts:136 — an unread configuration is
    /// an error, never an empty list.
    #[test]
    fn never_treats_an_unread_profile_configuration_as_an_empty_one() {
        let p = project(caps_all(), false, None, None, Some("Profile configuration could not be read."));
        assert_eq!(p.state, ViewState::Error("Profile configuration could not be read.".into()));
        assert!(p.providers.is_empty());
        assert_eq!(project(caps_all(), false, None, None, None).state, ViewState::Error(copy::LOAD_FAILED.into()));
        assert_eq!(project(caps_all(), true, None, None, None).state, ViewState::Loading);
        assert_eq!(project(Caps::default(), true, None, None, None).state, ViewState::Unavailable(copy::UNAVAILABLE_READ.into()));
        let empty = parse_config(&json!({"profile_id": "coding", "primary": null, "fallbacks": []})).unwrap();
        assert_eq!(project(caps_all(), false, Some(&empty), None, None).state, ViewState::Ready);
    }

    /// model-management-projection.test.ts:161 — no route identity keeps the
    /// row read-only (neither editable nor removable).
    #[test]
    fn keeps_a_configured_row_read_only_when_core_omits_route_identity() {
        let c = parse_config(&json!({"profile_id": "coding", "primary": {"family_id": "zai", "model_id": "glm-5.3-flash", "route": {},
            "has_api_key": true, "selected": true, "available": true}, "fallbacks": []}))
        .unwrap();
        let row = &project(caps_all(), false, Some(&c), None, None).providers[0];
        assert!(!row.editable && !row.removable);
        assert_eq!((row.route.id.as_str(), row.route.api_protocol.as_str()), ("", ""));
        assert!(row.reason.unwrap().contains("read-only"));
    }

    /// ModelManagementSection.test.tsx:104 — an existing Core identity is
    /// never presented as a new provider.
    #[test]
    fn never_presents_an_existing_core_model_identity_as_a_new_provider() {
        let families = vec![zai_family()];
        let protocols = vec![("openai-chat-completions".to_owned(), "OpenAI Chat Completions".to_owned())];
        let existing = Provider {
            id: "zai:glm-5.3-flash:coding-plan".into(),
            family_id: "zai".into(),
            family_label: "Z.AI".into(),
            model_id: "glm-5.3-flash".into(),
            model_label: "GLM-5.3-Flash".into(),
            route: families[0].default_route.clone().unwrap(),
            api_key_configured: true,
            primary: true,
            editable: true,
            removable: true,
            reason: None,
            inference: Map::new(),
            edit_blocked: false,
        };
        let created = create_draft(&families, &protocols, std::slice::from_ref(&existing));
        assert_eq!((created.family_id.as_str(), created.model_id.as_str(), created.route.id.as_str()), ("zai", "", "coding-plan"));
        let issues = validate(
            &configured_draft(&existing),
            Rules { credential_configured: true, credential: Credential::Required, requires_base_url: true, identity_available: Some(false) },
        );
        assert!(issues.contains(&Issue { field: Field::ModelId, message: "This model route already exists. Use Edit instead." }));
    }

    /// ModelManagementSection.test.tsx:123 — identity, route and credential
    /// are validated without fake parameters.
    #[test]
    fn validates_core_identity_route_and_credential_without_fake_parameters() {
        let families = vec![zai_family()];
        let protocols = vec![("openai-chat-completions".to_owned(), "OpenAI Chat Completions".to_owned())];
        let mut d = create_draft(&families, &protocols, &[]);
        let rules = Rules { credential_configured: false, credential: Credential::Required, requires_base_url: true, identity_available: None };
        assert_eq!(validate(&d, rules), vec![Issue { field: Field::ApiKey, message: "Enter an API key." }]);
        d.api_key = "temporary-browser-draft".into();
        assert!(validate(&d, rules).is_empty());
    }

    #[test]
    fn labels_are_the_webs() {
        assert_eq!(model_label("deepseek-v4-flash"), "Deepseek V4 Flash");
        assert_eq!(model_label("glm-5.1"), "GLM 5.1");
        assert_eq!(model_label("glm-5.3-flash"), "GLM-5.3-Flash");
        assert_eq!(family_label("zai"), "Z.AI");
        assert_eq!(family_label("moonshot-coding"), "Moonshot Coding");
        assert_eq!(route_label("r2-route"), "R2 Route");
        assert_eq!(protocol_label("openai"), "OpenAI-compatible");
        assert_eq!(protocol_label("openai_chat"), "Openai Chat");
    }

    #[test]
    fn route_selection_is_the_webs_shape() {
        let r = RouteDraft { route_id: " r2-route ".into(), base_url: "http://127.0.0.1:9/v1".into(), ..Default::default() };
        assert_eq!(
            route_selection(&r).unwrap(),
            json!({"route_id": "r2-route", "base_url": "http://127.0.0.1:9/v1", "api_key_env": "", "api_type": "openai"})
        );
        assert_eq!(route_selection(&RouteDraft::default()).unwrap_err(), "Provider route is required.");
        let p = provision("p", &Draft { family_id: "f".into(), model_id: "m".into(), route: r, ..Default::default() }, Some("  k  ")).unwrap();
        assert_eq!(p["api_key"], "k");
        assert!(provision("p", &Draft { family_id: "f".into(), model_id: "m".into(), route: RouteDraft { route_id: "x".into(), ..Default::default() }, ..Default::default() }, Some("  ")).unwrap().get("api_key").is_none());
    }

    #[test]
    fn malformed_configurations_are_invalid_never_partial() {
        // A non-boolean has_api_key, a selected fallback, a missing primary key.
        let bad_key = json!({"profile_id": "p", "primary": configured_row(json!({"has_api_key": "yes"})), "fallbacks": []});
        assert!(parse_config(&bad_key).is_none());
        let mut fb = configured_row(json!({}));
        fb["selected"] = json!(true);
        assert!(parse_config(&json!({"profile_id": "p", "primary": null, "fallbacks": [fb]})).is_none());
        assert!(parse_config(&json!({"profile_id": "p", "fallbacks": []})).is_none());
        // An out-of-range inference value invalidates the row.
        assert!(parse_config(&json!({"profile_id": "p", "primary": configured_row(json!({"temperature": 3})), "fallbacks": []})).is_none());
    }

    #[test]
    fn validation_reads_base_url_protocol_and_env() {
        let d = EditorDraft {
            family_id: "x".into(),
            model_id: "m".into(),
            route: ProviderRoute { id: "r".into(), label: "R".into(), base_url: "ftp://h".into(), ..Default::default() },
            api_key: "a\nb".into(),
            ..Default::default()
        };
        let msgs: Vec<&str> = validate(&d, Rules { credential_configured: true, credential: Credential::Required, requires_base_url: false, identity_available: None })
            .iter()
            .map(|i| i.message)
            .collect();
        assert_eq!(
            msgs,
            ["Base URL must use http or https.", "Choose an API protocol.", "Enter a credential environment name.", "API key must be a single line."]
        );
        assert!(is_http_url("https://api.z.ai/api/paas/v4") && is_http_url("http://127.0.0.1:9/v1") && !is_http_url("https://"));
    }
}
