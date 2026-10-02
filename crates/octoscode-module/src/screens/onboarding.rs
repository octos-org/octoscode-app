//! A17 — the solo ONBOARDING panel (parity rows 93-98, feature
//! "onboarding"): the native port of the web's
//! `features/onboarding/use-onboarding.ts` (the runtime state, `prepare`,
//! `submit`, the request gate and the secret redaction),
//! `onboarding-submission.ts` (the deferred submission and
//! `selectionFromCatalog`) and `OnboardingPanel.tsx` (the panel).
//!
//! ## Where it shows (the web's own decision)
//!
//! The web never opens onboarding on its own: a workspace launch asks the
//! server first (`use-octos-session.ts:3010-3060` `resolveInitialLaunch`),
//! and when Core answers `no_profile` it records the decision and calls
//! `onboardingController.prepare()` (`:3051-3058`); the launch panel then
//! renders THIS panel in place of the profile choice
//! (`LaunchDecisionPanel.tsx:33-48`, mounted by `App.tsx:2471-2489`).
//! Natively the same decision lives in [`crate::screens::launch::create`]
//! (A8's launch), which calls [`prepare_begin`] + [`fetch_catalog`] for a
//! `no_profile` answer, and [`crate::screens::launch::build`] draws [`build`]
//! for it — the board-3 dialog A8's launch panel already is.
//!
//! ## What the panel guarantees (each with its web line)
//!
//! - **Capability gate** — every `APPUI_ONBOARDING_METHODS` method must be
//!   advertised (`use-onboarding.ts:12,79-85`,
//!   `packages/client/src/onboarding-methods.ts:4-13`), else the canonical
//!   `octoscode onboard` fallback shows (`OnboardingPanel.tsx:110-111,
//!   288-306`) and nothing is requested: no catalog, no create, no submit.
//! - **Selection from the catalog** — the Official API route is DERIVED from
//!   the server family, a catalog endpoint is preserved as advertised, a
//!   stale family/model/route is rejected before anything is created
//!   (`onboarding-submission.ts:134-176`, [`selection_from_catalog`]).
//! - **Deferred submission** — `profile/local/create` once; the created
//!   profile is RETAINED, so a retry after a failed provider test only
//!   repeats `profile/llm/test` + `profile/llm/upsert`
//!   (`onboarding-submission.ts:52-83`, `use-onboarding.ts:62,141-169`).
//! - **Latest request wins** — `prepare` and `submit` each take a new
//!   generation (the web's `RequestGate`, `features/async/request-gate.ts`);
//!   a reply that comes back after a newer request, a cancel or a reset is
//!   dropped and publishes nothing (`use-onboarding.ts:96-101,109-115,144-147,
//!   158-163`), and a submit never runs on another connection than the one the
//!   panel was prepared on (`options.client() !== client`).
//! - **Redaction** — every submit error has the typed API key replaced by
//!   `[redacted]` and is cut at 1000 characters (`use-onboarding.ts:168,
//!   180-183`); the key is never part of a job, a log line, the DSL or any
//!   file (the masked input holds it, [`post_mount_texts`] puts it back after
//!   a remount).
use std::sync::{Mutex, MutexGuard, OnceLock};

use octoscode_client::domains::profile::{
    LlmCatalog, LlmCatalogFamily, LlmCatalogModel, LlmCatalogParams, LlmCatalogResult, LlmProvisionParams,
    LlmRouteSelection, LlmSelection, LlmTest, LlmUpsert, ProfileLocalCreate,
};
use octoscode_store::Store;

use super::board3::host::{Job, Outcome};
use super::board3::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};

/// `OFFICIAL_ROUTE` (`onboarding-submission.ts:13`).
pub const OFFICIAL_ROUTE: &str = "__official__";

/// `APPUI_ONBOARDING_METHODS` (`onboarding-methods.ts:4-13`): the panel needs
/// EVERY one of them (`use-onboarding.ts:12`).
pub const REQUIRED_METHODS: [&str; 8] = [
    "profile/local/create",
    "profile/llm/catalog",
    "profile/llm/delete",
    "profile/llm/fetch_models",
    "profile/llm/list",
    "profile/llm/select",
    "profile/llm/test",
    "profile/llm/upsert",
];

// ------------------------------------------------------------------ copy
// `OnboardingPanel.tsx`, verbatim — except the two lines that name the web
// client ("from the Web", "this browser client"), said of this app instead.

pub const EYEBROW: &str = "Octoscode setup";
pub const TITLE: &str = "Create your local coding profile";
pub const LEAD: &str = "This follows Octoscode’s solo onboarding: create a named profile, test a server-advertised model route, save it, then open the canonical coding session.";
pub const LOADING: &str = "Loading providers from Octos…";
pub const CATALOG_UNAVAILABLE: &str = "Provider catalog unavailable";
pub const CATALOG_INVALID: &str = "The server returned an invalid catalog.";
pub const FALLBACK_HEAD: &str = "This server cannot onboard from this app";
pub const FALLBACK_BODY: &str = "Run the canonical setup on the server, then reconnect this workspace.";
pub const FALLBACK_COMMAND: &str = "octoscode onboard";
pub const KEY_HINT: &str = "Sent only to your Octos server for test and save; never stored by this app.";
pub const KEYLESS_HEAD: &str = "No API key required";
pub const DEFAULT_LABEL: &str = "Use as the default local profile";
pub const DISCONNECT: &str = "Disconnect";
pub const RETRY: &str = "Retry";
pub const SUBMIT: &str = "Test, save & open";
pub const WORKING: &str = "Working…";

/// The client's own "invalid result" refusal for a catalog the web parser
/// rejects (`client.ts:870`, `onboarding.ts:223-273`).
pub const CATALOG_REJECTED: &str = "profile/llm/catalog returned an invalid result";

/// `OnboardingPhase` (`use-onboarding.ts:14-21`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    LoadingCatalog,
    Ready,
    CreatingProfile,
    TestingProvider,
    SavingProvider,
    OpeningSession,
}

/// `onboardingStatus` (`OnboardingPanel.tsx:308-321`).
pub fn status(phase: Phase) -> &'static str {
    match phase {
        Phase::CreatingProfile => "Creating profile",
        Phase::TestingProvider => "Testing provider",
        Phase::SavingProvider => "Saving provider",
        Phase::OpeningSession => "Opening coding session",
        _ => "Server-verified setup",
    }
}

/// `CreatedProfileBinding` (`use-onboarding.ts:47-50`): the profile id the
/// person asked for, and the one Core assigned (normalised, maybe suffixed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedProfile {
    pub requested_id: String,
    pub profile_id: String,
}

/// The panel's form (`OnboardingPanel.tsx:25-31`, `OnboardingSubmission`
/// `use-onboarding.ts:31-39`). `Debug` never prints the key.
#[derive(Clone, PartialEq, Eq)]
pub struct Form {
    pub profile_id: String,
    pub profile_name: String,
    pub make_default: bool,
    pub family_id: String,
    pub model_id: String,
    pub route_id: String,
    pub api_key: String,
}

impl Default for Form {
    /// The web's initial values (`OnboardingPanel.tsx:25-31`).
    fn default() -> Self {
        Self {
            profile_id: "coding".into(),
            profile_name: "Coding".into(),
            make_default: true,
            family_id: String::new(),
            model_id: String::new(),
            route_id: OFFICIAL_ROUTE.into(),
            api_key: String::new(),
        }
    }
}

impl std::fmt::Debug for Form {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Form")
            .field("profile_id", &self.profile_id)
            .field("profile_name", &self.profile_name)
            .field("make_default", &self.make_default)
            .field("family_id", &self.family_id)
            .field("model_id", &self.model_id)
            .field("route_id", &self.route_id)
            .field("api_key", &if self.api_key.is_empty() { "" } else { "[redacted]" })
            .finish()
    }
}

/// Which select is unfolded (one at a time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Select {
    Provider,
    Model,
    Route,
}

impl Select {
    fn key(self) -> &'static str {
        match self {
            Select::Provider => "provider",
            Select::Model => "model",
            Select::Route => "route",
        }
    }
}

/// `OnboardingRuntimeState` (`use-onboarding.ts:23-29`) plus the hook's refs
/// (`requestsRef`, `createdProfileRef`, `submissionActiveRef`) and the
/// panel's own form state.
#[derive(Debug, Clone, Default)]
pub struct State {
    pub phase: Phase,
    pub supported: bool,
    pub catalog: Option<LlmCatalogResult>,
    pub created_profile_id: Option<String>,
    pub error: Option<String>,
    /// The server's own plain message for a failed provider test (its
    /// `message`, e.g. "Provider connection failed"), shown as the error's
    /// lead with the raw `error` under it — redacted like the error.
    pub error_lead: Option<String>,
    /// The request gate's generation (`RequestGate`): every prepare, submit
    /// and reset moves it; a reply holding an older one is dropped.
    pub generation: u64,
    /// The connection (its authority epoch) the panel was prepared on.
    pub connection: u64,
    /// `createdProfileRef` — the profile a submit already created.
    pub binding: Option<CreatedProfile>,
    /// `submissionActiveRef`.
    pub submission_active: bool,
    /// The live form (the inputs' typed text lands here, no remount).
    pub form: Form,
    /// The Profile ID / Profile name texts the next mount embeds (synced from
    /// `form` on every action, never while typing).
    pub snap: (String, String),
    pub open_select: Option<Select>,
}

impl State {
    pub fn busy(&self) -> bool {
        self.phase != Phase::Ready
    }

    fn family(&self) -> Option<&LlmCatalogFamily> {
        self.catalog.as_ref()?.families.iter().find(|f| f.id == self.form.family_id)
    }

    fn model(&self) -> Option<&LlmCatalogModel> {
        self.family()?.models.iter().find(|m| m.id == self.form.model_id)
    }

    /// `requiresApiKey` (`OnboardingPanel.tsx:40-44`).
    pub fn requires_key(&self) -> bool {
        self.catalog.as_ref().is_some_and(|c| requires_key(c, &self.form))
    }

    /// The submit button's gate (`OnboardingPanel.tsx:271-276`).
    pub fn can_submit(&self) -> bool {
        !self.busy()
            && !self.form.family_id.is_empty()
            && !self.form.model_id.is_empty()
            && (!self.requires_key() || !self.form.api_key.trim().is_empty())
    }

    /// The route select's options (`OnboardingPanel.tsx:69-78`): the
    /// Official API first, then the model's catalog endpoints. An endpoint's
    /// detail line names its host (or its id when it has no URL), so two
    /// routes with one label ("Official API") stay told apart.
    pub fn routes(&self) -> Vec<Opt> {
        let mut out = vec![Opt { id: OFFICIAL_ROUTE.into(), label: "Official API".into(), detail: None }];
        if let Some(m) = self.model() {
            for e in &m.endpoints {
                let detail = match e.base_url.as_deref().and_then(host_of) {
                    Some(host) => host,
                    None => e.id.clone(),
                };
                out.push(Opt { id: e.id.clone(), label: e.label.clone().unwrap_or_else(|| e.id.clone()), detail: Some(detail) });
            }
        }
        out
    }

    fn options(&self, which: Select) -> Vec<Opt> {
        let plain = |id: &String| Opt { id: id.clone(), label: id.clone(), detail: None };
        match which {
            Select::Provider => self.catalog.iter().flat_map(|c| c.families.iter().map(|f| plain(&f.id))).collect(),
            Select::Model => self.family().map(|f| f.models.iter().map(|m| plain(&m.id)).collect()).unwrap_or_default(),
            Select::Route => self.routes(),
        }
    }

    fn selected(&self, which: Select) -> &str {
        match which {
            Select::Provider => &self.form.family_id,
            Select::Model => &self.form.model_id,
            Select::Route => &self.form.route_id,
        }
    }
}

/// `https://www.autodl.art/api/v1` -> `www.autodl.art`.
fn host_of(url: &str) -> Option<String> {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_owned))
}

/// One option of a select.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opt {
    pub id: String,
    pub label: String,
    pub detail: Option<String>,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn lock() -> MutexGuard<'static, State> {
    STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

/// A copy of the onboarding state (tests and the walk's assertions).
pub fn snapshot() -> State {
    lock().clone()
}

/// `reset` (`use-onboarding.ts:66-71`): invalidate every in-flight request,
/// forget the created profile, back to the empty state — and the panel's form
/// with it (the web unmounts the panel, dropping its state; the typed key goes
/// too).
pub fn reset() {
    let mut st = lock();
    let generation = st.generation + 1;
    *st = State { generation, ..Default::default() };
}

fn wake() {
    makepad_widgets::SignalToUI::set_ui_signal();
}

// ------------------------------------------------------------- the oracle

/// Whether the server advertises every onboarding method
/// (`use-onboarding.ts:79-85`).
pub fn supported(store: &Store) -> bool {
    let methods = store.domains.config.supported_methods();
    REQUIRED_METHODS.iter().all(|m| methods.iter().any(|x| x == m))
}

/// `requiresApiKey` (`OnboardingPanel.tsx:40-44`): the Official API needs the
/// family's env; an endpoint its own env, else the family's.
pub fn requires_key(catalog: &LlmCatalogResult, form: &Form) -> bool {
    let family = catalog.families.iter().find(|f| f.id == form.family_id);
    let env = if form.route_id == OFFICIAL_ROUTE {
        family.map(|f| f.env.clone())
    } else {
        family
            .and_then(|f| f.models.iter().find(|m| m.id == form.model_id))
            .and_then(|m| m.endpoints.iter().find(|e| e.id == form.route_id))
            .and_then(|e| e.api_key_env.clone())
            .or_else(|| family.map(|f| f.env.clone()))
    };
    env.is_some_and(|e| !e.is_empty())
}

/// `selectionFromCatalog` (`onboarding-submission.ts:134-176`): the Official
/// API route is DERIVED from the family (its id, "Official API", the family
/// env — kept even when empty: a keyless family names no env); a catalog
/// endpoint is preserved as advertised; a family/model/route the catalog no
/// longer carries is the web's exact refusal.
pub fn selection_from_catalog(
    catalog: &LlmCatalogResult,
    family_id: &str,
    model_id: &str,
    route_id: &str,
) -> Result<LlmSelection, String> {
    let family = catalog.families.iter().find(|f| f.id == family_id);
    let model = family.and_then(|f| f.models.iter().find(|m| m.id == model_id));
    let (Some(family), Some(model)) = (family, model) else {
        return Err("The selected provider or model is no longer advertised.".into());
    };
    if route_id == OFFICIAL_ROUTE {
        return Ok(LlmSelection {
            family_id: family.id.clone(),
            model_id: model.id.clone(),
            route: LlmRouteSelection {
                route_id: Some(family.id.clone()),
                label: Some("Official API".into()),
                base_url: None,
                api_key_env: Some(family.env.clone()),
                api_type: Some("openai".into()),
            },
            inference: Default::default(),
        });
    }
    let Some(endpoint) = model.endpoints.iter().find(|e| e.id == route_id) else {
        return Err("The selected provider route is no longer advertised.".into());
    };
    let some = |v: &Option<String>| v.clone().filter(|s| !s.is_empty());
    Ok(LlmSelection {
        family_id: family.id.clone(),
        model_id: model.id.clone(),
        route: LlmRouteSelection {
            route_id: Some(endpoint.id.clone()),
            label: some(&endpoint.label),
            base_url: some(&endpoint.base_url),
            api_key_env: Some(some(&endpoint.api_key_env).unwrap_or_else(|| family.env.clone())),
            api_type: Some(some(&endpoint.api_type).unwrap_or_else(|| "openai".into())),
        },
        inference: Default::default(),
    })
}

/// `redactSecret` (`use-onboarding.ts:180-183`): the key replaced by
/// `[redacted]` wherever it appears, the message cut at 1000 characters.
pub fn redact_secret(message: &str, secret: &str) -> String {
    let redacted = if secret.is_empty() { message.to_owned() } else { message.replace(secret, "[redacted]") };
    redacted.chars().take(1_000).collect()
}

/// The web's catalog parser (`onboarding.ts:223-273`): empty optional texts
/// are absent, a family without models is dropped, and a catalog with no
/// family left is invalid.
pub fn validate_catalog(mut catalog: LlmCatalogResult) -> Result<LlmCatalogResult, String> {
    let blank = |v: &mut Option<String>| {
        if v.as_deref().is_some_and(|s| s.trim().is_empty()) {
            *v = None;
        }
    };
    for f in &mut catalog.families {
        for m in &mut f.models {
            for e in &mut m.endpoints {
                blank(&mut e.label);
                blank(&mut e.base_url);
                blank(&mut e.api_key_env);
                blank(&mut e.api_type);
            }
        }
    }
    catalog.families.retain(|f| !f.id.is_empty() && !f.models.is_empty());
    if catalog.families.is_empty() {
        return Err(CATALOG_REJECTED.into());
    }
    Ok(catalog)
}

/// The panel's catalog effect (`OnboardingPanel.tsx:46-59`): keep the family
/// and model when the catalog still has them, else its first ones; the route
/// goes back to the Official API.
fn adopt_catalog(form: &mut Form, catalog: &LlmCatalogResult) {
    let family = if catalog.families.iter().any(|f| f.id == form.family_id) {
        form.family_id.clone()
    } else {
        catalog.families.first().map(|f| f.id.clone()).unwrap_or_default()
    };
    let models = catalog.families.iter().find(|f| f.id == family).map(|f| &f.models);
    let model = if models.is_some_and(|ms| ms.iter().any(|m| m.id == form.model_id)) {
        form.model_id.clone()
    } else {
        models.and_then(|ms| ms.first()).map(|m| m.id.clone()).unwrap_or_default()
    };
    form.family_id = family;
    form.model_id = model;
    form.route_id = OFFICIAL_ROUTE.into();
    clear_key_if_keyless(form, catalog);
}

/// `useEffect(() => { if (!requiresApiKey) setApiKey(""); })`
/// (`OnboardingPanel.tsx:65-67`).
fn clear_key_if_keyless(form: &mut Form, catalog: &LlmCatalogResult) {
    if !requires_key(catalog, form) {
        form.api_key.clear();
    }
}

// ------------------------------------------------------------ the gate

/// One request's ownership: the gate generation and the connection it was
/// issued on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub generation: u64,
    pub connection: u64,
}

fn current(st: &State, t: Token) -> bool {
    st.generation == t.generation && st.connection == t.connection
}

/// Whether `t` still owns the panel (`requestsRef.current.isCurrent(...) &&
/// options.client() === client`).
pub fn is_current(t: Token) -> bool {
    current(&lock(), t)
}

// --------------------------------------------------------------- prepare

/// `prepare`'s synchronous half (`use-onboarding.ts:73-93`): a new
/// generation, the created profile forgotten; without every onboarding method
/// the empty state (the `octoscode onboard` fallback) and `None` — nothing is
/// requested; else the loading state and the token the catalog reply needs.
pub fn prepare_begin(store: &Store, connection: u64) -> Option<Token> {
    let mut st = lock();
    st.generation += 1;
    st.connection = connection;
    st.binding = None;
    st.catalog = None;
    st.created_profile_id = None;
    st.error = None;
    st.error_lead = None;
    st.open_select = None;
    if !supported(store) {
        st.phase = Phase::Idle;
        st.supported = false;
        return None;
    }
    st.phase = Phase::LoadingCatalog;
    st.supported = true;
    Some(Token { generation: st.generation, connection })
}

/// `prepare`'s reply half (`use-onboarding.ts:94-123`): published only while
/// `t` is current (a superseded reply returns `false` and changes nothing).
pub fn prepare_finish(t: Token, reply: Result<LlmCatalogResult, String>) -> bool {
    let mut st = lock();
    if !current(&st, t) {
        makepad_widgets::log!("[octoscode] onboarding: a superseded profile/llm/catalog reply was dropped");
        return false;
    }
    st.phase = Phase::Ready;
    st.created_profile_id = None;
    st.error_lead = None;
    match reply.and_then(validate_catalog) {
        Ok(catalog) => {
            let mut form = st.form.clone();
            adopt_catalog(&mut form, &catalog);
            st.snap = (form.profile_id.clone(), form.profile_name.clone());
            st.form = form;
            st.catalog = Some(catalog);
            st.error = None;
        }
        Err(e) => {
            st.catalog = None;
            st.error = Some(e);
        }
    }
    true
}

/// The catalog read for a begun prepare (`client.getLlmCatalog()`).
pub async fn fetch_catalog(client: &octoscode_client::Client, t: Token) -> Result<String, String> {
    let reply = client
        .call::<LlmCatalog>(LlmCatalogParams {})
        .await
        .map_err(|e| crate::screens::dialog::display_error(&e.to_string()));
    let ok = reply.is_ok();
    let published = prepare_finish(t, reply);
    wake();
    Ok(match (published, ok) {
        (false, _) => "superseded: the reply was dropped".into(),
        (true, true) => "catalog ready".into(),
        (true, false) => "catalog unavailable".into(),
    })
}

/// `prepare` (`use-onboarding.ts:73-124`) — the launch's first call and the
/// failure card's Retry.
pub async fn prepare(client: &octoscode_client::Client, store: &Store, connection: u64) -> Result<String, String> {
    let Some(t) = prepare_begin(store, connection) else {
        wake();
        return Ok("unsupported: the `octoscode onboard` fallback".into());
    };
    wake();
    fetch_catalog(client, t).await
}

// ---------------------------------------------------------------- submit

/// What a submit came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Submitted {
    /// The provider was tested and saved, and the session opened under this
    /// (server-assigned) profile.
    Configured(String),
    /// The submission failed; the panel shows this (redacted) error.
    Failed(String),
    /// A newer request, a cancel or a reset took the panel: nothing published.
    Superseded,
    /// The panel could not submit (unsupported, no catalog, busy, already
    /// submitting, or another connection than the one it was prepared on).
    Refused,
}

/// Move the panel to `phase` (clearing the error) while `t` owns it.
fn set_phase(t: Token, phase: Phase) {
    let mut st = lock();
    if current(&st, t) {
        st.phase = phase;
        st.error = None;
        st.error_lead = None;
        st.open_select = None;
        if phase == Phase::OpeningSession {
            // `if (state.phase === "opening_session") setApiKey("")`
            // (`OnboardingPanel.tsx:61-63`).
            st.form.api_key.clear();
        }
    }
    drop(st);
    wake();
}

fn client_error(e: octoscode_client::ClientError) -> String {
    crate::screens::dialog::display_error(&e.to_string())
}

/// `submitOnboarding` (`onboarding-submission.ts:27-132`). `Ok(None)` = the
/// panel was superseded at an await point (nothing more is sent or shown).
async fn run_submission<F, Fut>(
    client: &octoscode_client::Client,
    t: Token,
    catalog: &LlmCatalogResult,
    form: &Form,
    binding: Option<CreatedProfile>,
    on_configured: F,
    lead: &mut Option<String>,
) -> Result<Option<String>, String>
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    let profile_id = form.profile_id.trim();
    let profile_name = form.profile_name.trim();
    let api_key = form.api_key.trim();
    let selection = selection_from_catalog(catalog, &form.family_id, &form.model_id, &form.route_id)?;
    let requires_key = selection.route.api_key_env.as_deref().is_some_and(|e| !e.is_empty());
    if profile_id.is_empty() || profile_name.is_empty() || (requires_key && api_key.is_empty()) {
        return Err(if requires_key {
            "Profile ID, profile name, and API key are required."
        } else {
            "Profile ID and profile name are required."
        }
        .into());
    }
    // The keyless compatibility probe (`onboarding-submission.ts:48-51`).
    let wire_key = if api_key.is_empty() { crate::screens::connect::KEYLESS_PROBE } else { api_key };
    if !api_key.is_empty() {
        // The protocol trace scrubs this value wherever it shows up, including
        // a provider error that echoes it (trace::register_secret).
        octoscode_client::trace::register_secret(api_key);
    }
    let created = match binding {
        None => {
            set_phase(t, Phase::CreatingProfile);
            let created = client
                .call::<ProfileLocalCreate>(octos_core::ui_protocol::ProfileLocalCreateParams {
                    requested_id: Some(profile_id.to_owned()),
                    name: profile_name.to_owned(),
                    username: String::new(),
                    email: String::new(),
                    make_default: Some(form.make_default),
                })
                .await
                .map_err(client_error)?;
            let mut st = lock();
            if !current(&st, t) {
                return Ok(None);
            }
            let b = CreatedProfile { requested_id: profile_id.to_owned(), profile_id: created.profile_id };
            st.binding = Some(b.clone());
            st.created_profile_id = Some(b.profile_id.clone());
            b
        }
        Some(b) if b.requested_id != profile_id => {
            return Err(format!(
                "Profile {} was already created. Reconnect to choose another identity.",
                b.profile_id
            ));
        }
        Some(b) => b,
    };
    let created_id = created.profile_id;

    set_phase(t, Phase::TestingProvider);
    let tested = client
        .call::<LlmTest>(LlmProvisionParams {
            profile_id: Some(created_id.clone()),
            selection: selection.clone(),
            api_key: Some(wire_key.to_owned()),
            set_primary: None,
        })
        .await
        .map_err(client_error)?;
    if !is_current(t) {
        return Ok(None);
    }
    let test_error = tested.error.clone().filter(|e| !e.is_empty());
    if tested.profile_id != created_id || !tested.applied || test_error.is_some() {
        let message = test_error
            .or_else(|| Some(tested.message.clone()).filter(|m| !m.is_empty()))
            .unwrap_or_else(|| "The provider test did not pass.".into());
        // The server's plain `message` leads when it says something the
        // raw `error` does not.
        *lead = Some(tested.message.clone()).filter(|m| !m.is_empty() && *m != message);
        return Err(message);
    }

    set_phase(t, Phase::SavingProvider);
    let saved = client
        .call::<LlmUpsert>(LlmProvisionParams {
            profile_id: Some(created_id.clone()),
            selection,
            api_key: Some(wire_key.to_owned()),
            set_primary: Some(true),
        })
        .await
        .map_err(client_error)?;
    if !is_current(t) {
        return Ok(None);
    }
    if saved.profile_id != created_id || !saved.applied {
        return Err("The server did not apply the tested provider.".into());
    }

    set_phase(t, Phase::OpeningSession);
    on_configured(created_id.clone()).await?;
    Ok(Some(created_id))
}

/// `submit` (`use-onboarding.ts:126-175`) over the production client.
/// `connection` is the connection the job runs on; `on_configured` is the
/// launch's candidate open (`use-octos-session.ts:2665-2692`; natively
/// [`crate::screens::launch::open_onboarded`]).
pub async fn submit<F, Fut>(client: &octoscode_client::Client, connection: u64, on_configured: F) -> Submitted
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    let (t, form, catalog, binding) = {
        let mut st = lock();
        let refuse = !st.supported
            || st.catalog.is_none()
            || st.phase != Phase::Ready
            || st.submission_active
            || st.connection != connection;
        if refuse {
            return Submitted::Refused;
        }
        st.generation += 1;
        st.submission_active = true;
        st.open_select = None;
        let t = Token { generation: st.generation, connection: st.connection };
        (t, st.form.clone(), st.catalog.clone().unwrap_or(LlmCatalogResult { families: Vec::new() }), st.binding.clone())
    };
    let secret = form.api_key.trim().to_owned();
    let mut lead = None;
    let result = run_submission(client, t, &catalog, &form, binding, on_configured, &mut lead).await;
    let out = {
        let mut st = lock();
        let out = match result {
            Ok(Some(profile)) => Submitted::Configured(profile),
            Ok(None) => Submitted::Superseded,
            Err(_) if !current(&st, t) => Submitted::Superseded,
            Err(e) => {
                st.phase = Phase::Ready;
                st.created_profile_id = st.binding.as_ref().map(|b| b.profile_id.clone());
                let message = redact_secret(&e, &secret);
                st.error = Some(message.clone());
                st.error_lead = lead.map(|l| redact_secret(&l, &secret));
                Submitted::Failed(message)
            }
        };
        if current(&st, t) {
            st.submission_active = false;
        }
        out
    };
    if out == Submitted::Superseded {
        makepad_widgets::log!("[octoscode] onboarding: a superseded submission was dropped");
    }
    wake();
    out
}

// ---------------------------------------------------------------- actions

/// Route one `b3.onb.*` action (the panel's controls). The jobs carry no
/// form data: the submit reads the live form (and its key) when it runs.
pub fn perform(action: &str, index: usize) -> Outcome {
    let mut st = lock();
    st.snap = (st.form.profile_id.clone(), st.form.profile_name.clone());
    let busy = st.busy();
    match action {
        // The dialog's close glyph (and Escape, `host::close`): leaving the
        // panel cancels the launch (`cancelLaunch`, `use-octos-session.ts:
        // 3334-3342`) — except while the new Session is being opened.
        "b3.onb.close" => {
            if st.phase == Phase::OpeningSession {
                return Outcome::Done;
            }
            Outcome::Close
        }
        // "Disconnect" = `onCancel` = `cancelLaunch`, disabled while busy
        // (`OnboardingPanel.tsx:265-267`; the fallback's, `:300-302`).
        "b3.onb.disconnect" => {
            if busy && st.supported && st.phase != Phase::Idle {
                return Outcome::Done;
            }
            Outcome::Close
        }
        // The catalog failure card's Retry (`onRetry` = `prepare`).
        "b3.onb.retry" => {
            if st.phase == Phase::Ready && st.catalog.is_none() && st.supported {
                Outcome::Spawn(Job::OnboardingPrepare)
            } else {
                Outcome::Done
            }
        }
        "b3.onb.submit" => {
            if st.can_submit() && st.catalog.is_some() && st.supported {
                super::board3::host::request_blur();
                Outcome::Spawn(Job::OnboardingSubmit)
            } else {
                Outcome::Done
            }
        }
        "b3.onb.default" => {
            if !busy && st.created_profile_id.is_none() {
                st.form.make_default = !st.form.make_default;
            }
            Outcome::Done
        }
        "b3.onb.select.provider" | "b3.onb.select.model" | "b3.onb.select.route" => {
            if !busy && st.catalog.is_some() {
                let which = match action {
                    "b3.onb.select.provider" => Select::Provider,
                    "b3.onb.select.model" => Select::Model,
                    _ => Select::Route,
                };
                st.open_select = if st.open_select == Some(which) { None } else { Some(which) };
            }
            Outcome::Done
        }
        "b3.onb.pick" => {
            if busy {
                return Outcome::Done;
            }
            let Some(which) = st.open_select else { return Outcome::Done };
            let Some(opt) = st.options(which).get(index).cloned() else { return Outcome::Done };
            let Some(catalog) = st.catalog.clone() else { return Outcome::Done };
            let form = &mut st.form;
            match which {
                // `OnboardingPanel.tsx:172-180`: a new provider takes its
                // first model and the Official API.
                Select::Provider => {
                    form.model_id = catalog
                        .families
                        .iter()
                        .find(|f| f.id == opt.id)
                        .and_then(|f| f.models.first())
                        .map(|m| m.id.clone())
                        .unwrap_or_default();
                    form.family_id = opt.id;
                    form.route_id = OFFICIAL_ROUTE.into();
                }
                // `:195-198`: a new model goes back to the Official API.
                Select::Model => {
                    form.model_id = opt.id;
                    form.route_id = OFFICIAL_ROUTE.into();
                }
                Select::Route => form.route_id = opt.id,
            }
            clear_key_if_keyless(form, &catalog);
            st.open_select = None;
            Outcome::Done
        }
        _ => Outcome::Unrouted,
    }
}

/// A form input changed (no remount: the live value only).
pub fn input_changed(key: &str, text: &str) {
    let mut st = lock();
    match key {
        "onb.profile_id" => st.form.profile_id = text.to_owned(),
        "onb.profile_name" => st.form.profile_name = text.to_owned(),
        "onb.api_key" => st.form.api_key = text.to_owned(),
        _ => {}
    }
}

/// Return in a form field submits the form when its submit button is enabled
/// (the web's `<form onSubmit>`; a disabled default button does not submit).
pub fn input_returned(key: &str) -> Outcome {
    if key.starts_with("onb.") {
        return perform("b3.onb.submit", 0);
    }
    Outcome::Done
}

/// The live gates the inputs drive without a remount: the submit button's
/// enabled / disabled variants.
pub fn visibility() -> Vec<(String, bool)> {
    let st = lock();
    let on = st.can_submit();
    vec![("b3_onb_submit_on".into(), on), ("b3_onb_submit_off".into(), !on)]
}

/// The masked input's text after a remount (the key never rides the DSL):
/// `(widget id, text)` for the host to set.
pub fn post_mount_texts() -> Vec<(String, String)> {
    let st = lock();
    if st.phase == Phase::Ready && st.catalog.is_some() && st.requires_key() && !st.form.api_key.is_empty() {
        return vec![("b3_onb_apikey".into(), st.form.api_key.clone())];
    }
    Vec::new()
}

/// A job could not run (no live connection): the panel says so.
pub fn job_unavailable(job: &Job) {
    let mut st = lock();
    let msg = "The Octos server connection is not ready.".to_owned();
    match job {
        Job::OnboardingPrepare => {
            st.phase = Phase::Ready;
            st.catalog = None;
            st.error = Some(msg);
        }
        Job::OnboardingSubmit => st.error = Some(msg),
        _ => {}
    }
}

// ------------------------------------------------------------------ view

/// Line-count estimate of a wrapped run (for the body's height budget).
fn lines(s: &str, px: f64, face: Face, w: f64) -> f64 {
    (ui::text_w(s, px, face) / w.max(1.0)).ceil().max(1.0)
}

/// A wrapped label's line height in px — measured with /snap on the kit's
/// text style (`font_size = px * 0.75`, `line_spacing 1.25`): the 13 px lead
/// draws 4 lines in 75 px, i.e. `px * 1.4423`.
fn line_h(px: f64) -> f64 {
    px * 1.4423
}

fn field_box(d: &mut Dsl, id: &str, enabled: bool, open: bool) {
    let (fill, border) = if !enabled {
        (tok::SURFACE2, tok::HAIRLINE)
    } else if open {
        (tok::SURFACE, tok::BLUE)
    } else {
        (tok::SURFACE, "#d9d9dcff")
    };
    d.surface(&format!("{id}_field"), "width: Fill height: 38 flow: Overlay", fill, 8.0, Some(border));
}

/// A disabled text field: the value on the grey well, no input (the web's
/// `disabled` input).
fn disabled_field(d: &mut Dsl, id: &str, value: &str, mono: bool, w: f64) {
    field_box(d, id, false, false);
    let row = d.anon();
    d.view(&row, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} padding: Inset{left: 10 right: 10 top: 0 bottom: 0}");
    let face = if mono { Face::Mono } else { Face::Regular };
    d.text(&format!("{id}_value"), &ui::fit_w(value, w - 24.0, 13.0, face), &Txt::new(13.0, face, tok::FAINT).w(W::Fill));
    d.close();
    d.close();
}

/// One labelled text field of the form.
#[allow(clippy::too_many_arguments)]
fn text_field(d: &mut Dsl, id: &str, label: &str, key: &str, snap: &str, live: &str, mono: bool, enabled: bool, w: W, px_w: f64) {
    let col = d.anon();
    d.view(&col, &format!("width: {} height: Fit flow: Down spacing: 6", w_dsl(w)));
    ui::field_label(d, &format!("{id}_label"), label);
    if enabled {
        d.input(id, key, snap, "", mono, 38.0);
    } else {
        disabled_field(d, id, live, mono, px_w);
    }
    d.close();
}

fn w_dsl(w: W) -> String {
    match w {
        W::Fit => "Fit".into(),
        W::Fill => "Fill".into(),
        W::Px(v) => format!("{}", v.floor()),
    }
}

/// One select (`<select>`): the field shows the choice and a chevron; a tap
/// unfolds its options under it (rows, the selected one ticked), in a box
/// capped at `list_max` so it stays inside the dialog body.
#[allow(clippy::too_many_arguments)]
fn select_field(d: &mut Dsl, st: &State, which: Select, label: &str, value: &str, mono: bool, enabled: bool, w: W, px_w: f64, list_max: f64) {
    let id = format!("b3_onb_{}", which.key());
    let open = enabled && st.open_select == Some(which);
    let col = d.anon();
    d.view(&col, &format!("width: {} height: Fit flow: Down spacing: 6", w_dsl(w)));
    ui::field_label(d, &format!("{id}_label"), label);
    field_box(d, &id, enabled, open);
    let row = d.anon();
    d.view(&row, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{left: 10 right: 10 top: 0 bottom: 0}");
    let face = if mono { Face::Mono } else { Face::Regular };
    let ink = if enabled { tok::TEXT } else { tok::FAINT };
    let px = 13.0; // the kit input's size: a select reads like the text fields beside it
    d.text(&format!("{id}_value"), &ui::fit_w(value, px_w - 46.0, px, face), &Txt::new(px, face, ink).w(W::Fill));
    d.icon(&format!("{id}_chevron"), "b3_chevron_down_dark.svg", 12.0, tok::MUTED);
    d.close();
    if enabled {
        d.tap(&id, &format!("b3.onb.select.{}", which.key()));
    }
    d.close();
    if open {
        options_list(d, st, which, mono, px_w, Some(list_max));
    }
    d.close();
}

/// The options of the open select. `max_h`: a dropdown's capped box with its
/// own scroll (desktop); `None` lays every row out in the dialog body (the
/// phone's picker sheet scrolls with the body).
fn options_list(d: &mut Dsl, st: &State, which: Select, mono: bool, px_w: f64, max_h: Option<f64>) {
    let opts = st.options(which);
    let chosen = st.selected(which).to_owned();
    d.surface(
        "b3_onb_list",
        "width: Fill height: Fit flow: Down padding: Inset{left: 4 right: 4 top: 4 bottom: 4}",
        tok::SURFACE,
        10.0,
        Some(tok::HAIRLINE),
    );
    // A long list (a real catalog has 20 families; one family 33 models)
    // scrolls inside its own capped box, like a native menu.
    match max_h {
        Some(h) => d.open(
            "b3_onb_list_scroll",
            "ScrollYView",
            &format!("width: Fill height: Fit max_height: {} flow: Down padding: Inset{{left: 0 top: 0 right: 6 bottom: 0}}", h.floor()),
        ),
        None => d.view("b3_onb_list_scroll", "width: Fill height: Fit flow: Down"),
    }
    let face = if mono { Face::Mono } else { Face::Regular };
    for (i, opt) in opts.iter().enumerate() {
        let on = opt.id == chosen;
        let id = format!("b3_onb_opt_{i}");
        // Centred: a touch platform raises the row's tap target to 44 px
        // (measured on the phone page: a one-line row 31 -> 44), and the
        // content must sit in its middle, not at its top.
        d.surface(
            &format!("{id}_box"),
            "width: Fill height: Fit flow: Overlay align: Align{x: 0.0 y: 0.5}",
            if on { tok::SURFACE2 } else { tok::TRANSPARENT },
            7.0,
            None,
        );
        let row = d.anon();
        d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{left: 10 right: 10 top: 8 bottom: 8}");
        let text_col = d.anon();
        d.view(&text_col, "width: Fill height: Fit flow: Down spacing: 2");
        let px = 13.0; // the kit input's size: a select reads like the text fields beside it
        d.text(&format!("{id}_label"), &opt.label, &Txt::new(px, face, tok::TEXT).w(W::Fill).wrap());
        if let Some(detail) = &opt.detail {
            d.text(&format!("{id}_detail"), &ui::fit_w(detail, px_w - 60.0, 11.5, Face::Mono), &Txt::new(11.5, Face::Mono, tok::MUTED).w(W::Fill));
        }
        d.close();
        if on {
            d.icon(&format!("{id}_check"), "b3_check.svg", 14.0, tok::TEXT);
        } else {
            d.gap(W::Px(14.0), 14.0);
        }
        d.close();
        d.tap(&id, &format!("b3.onb.pick#{i}"));
        d.close();
    }
    d.close();
    d.close();
}

fn note_box(d: &mut Dsl, id: &str) {
    d.surface(id, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{left: 10 right: 10 top: 8 bottom: 8}", tok::SURFACE2, 8.0, None);
}

/// The panel (`OnboardingPanel.tsx:94-285`) in the board-3 dialog frame A8's
/// launch panel uses (the web mounts it in the same `launch-decision`
/// container, `LaunchDecisionPanel.tsx:33-48`).
pub fn build(d: &mut Dsl, frame: &Frame) {
    let st = snapshot();
    let width = frame.dialog_w(760.0);
    let compact = frame.compact(width);
    let pad = ui::dialog_pad(frame, width);
    // The body's text width: the card padding and the scroll bar's gutter.
    let inner_w = width - 2.0 * pad - 10.0;
    ui::shell_open(d, frame, width);

    // ---- header: eyebrow, title + close, the lead paragraph
    d.text("b3_onb_eyebrow", EYEBROW, &Txt::new(11.5, Face::Medium, tok::MUTED));
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5}");
    d.text("b3_title", TITLE, &ui::title().w(W::Fill).wrap());
    ui::close_glyph(d, "b3.onb.close");
    d.close();
    d.gap(W::Fill, 6.0);
    d.text("b3_onb_lead", LEAD, &Txt::new(13.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    d.gap(W::Fill, 12.0);

    // The chrome around the body: the header above and the footer below —
    // the footer carrying the form's retained-profile note and its error
    // (they sit right above the actions, as in the web, and never scroll
    // away: on a phone the form is taller than the screen).
    let title_lines = lines(TITLE, 17.0, Face::Semibold, width - 2.0 * pad - 28.0);
    let lead_lines = lines(LEAD, 13.0, Face::Regular, width - 2.0 * pad);
    let notes = form_notes(&st);
    let note_w = inner_w - 10.0 - 20.0 - 22.0;
    let notes_h: f64 = notes.iter().map(|n| 8.0 + n.height(note_w)).sum();
    let footer = if compact { 12.0 + line_h(12.0) + 8.0 + 36.0 } else { 12.0 + 36.0 };
    let header = line_h(11.5) + (title_lines * line_h(17.0)).max(28.0) + 6.0 + lead_lines * line_h(13.0) + 12.0;
    // + a margin for the estimate's error, so the card never outgrows the frame.
    let chrome = header + footer + notes_h + 8.0;
    ui::body_open(d, frame, width, chrome);
    let body = d.anon();
    d.view(&body, "width: Fill height: Fit flow: Down spacing: 12");

    // What the body can show before it scrolls (`ui::body_open`'s cap).
    let body_max = (frame.dialog_max_h() - 2.0 * pad - chrome).max(120.0).floor();
    let footer_kind = if !st.supported {
        fallback(d);
        Foot::Fallback
    } else if st.phase == Phase::LoadingCatalog {
        d.view("b3_onb_progress", "width: Fill height: 84 flow: Right spacing: 8 align: Align{x: 0.0 y: 0.5}");
        d.dot(tok::AMBER, 8.0);
        d.text("b3_onb_loading", LOADING, &Txt::new(12.5, Face::Regular, tok::MUTED));
        d.close();
        Foot::None
    } else if st.catalog.is_none() {
        d.surface("b3_onb_failure", "width: Fill height: Fit flow: Down spacing: 9 padding: Inset{left: 14 right: 14 top: 14 bottom: 14}", tok::SURFACE2, 10.0, Some(tok::HAIRLINE));
        d.text("b3_onb_failure_head", CATALOG_UNAVAILABLE, &Txt::new(13.0, Face::Semibold, tok::TEXT).w(W::Fill).wrap());
        let why = st.error.clone().unwrap_or_else(|| CATALOG_INVALID.into());
        d.text("b3_onb_failure_body", &why, &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
        d.close();
        Foot::Failure
    } else {
        form(d, &st, compact, inner_w, body_max);
        Foot::Form
    };
    d.close();
    ui::body_close(d);

    // ---- footer: the form's notes, the status line and the actions
    // (`.launch-actions`), on the body's right edge (its scroll gutter).
    if footer_kind != Foot::None {
        d.gap(W::Fill, 12.0);
        let foot = d.anon();
        d.view(&foot, "width: Fill height: Fit flow: Down spacing: 8 padding: Inset{left: 0 right: 10 top: 0 bottom: 0}");
        if footer_kind == Foot::Form {
            for note in &notes {
                note.draw(d);
            }
        }
        footer_row(d, &st, footer_kind, compact);
        d.close();
    }
    ui::shell_close(d);
}

/// The tallest the error's raw cause grows in the footer before it scrolls
/// inside its own box (~4 lines): a provider's 1000-character body must not
/// push the actions off a phone screen, and it stays readable whole.
const CAUSE_MAX_H: f64 = 66.0;

/// A note the form shows above its actions.
enum Note {
    /// `OnboardingPanel.tsx:250-256`: the created profile is kept.
    Recovery(String),
    /// `OnboardingPanel.tsx:257-261`: the (redacted) error. `lead` is a
    /// sentence for people (the server's own `message`, or a plain line
    /// over developer wording, A13's dialog convention); `cause` the error
    /// text the web prints, under it. No lead: the error alone.
    Error { lead: Option<String>, cause: String },
}

impl Note {
    /// The note's drawn height at text width `w` (the body's budget).
    fn height(&self, w: f64) -> f64 {
        match self {
            Note::Recovery(t) => 16.0 + lines(t, 12.5, Face::Regular, w) * line_h(12.5),
            Note::Error { lead: Some(l), cause } => {
                16.0 + lines(l, 13.0, Face::Medium, w) * line_h(13.0)
                    + 2.0
                    + (lines(cause, 11.5, Face::Regular, w) * line_h(11.5)).min(CAUSE_MAX_H)
            }
            Note::Error { lead: None, cause } => 16.0 + lines(cause, 12.5, Face::Regular, w) * line_h(12.5),
        }
    }

    fn draw(&self, d: &mut Dsl) {
        match self {
            Note::Recovery(t) => {
                note_box(d, "b3_onb_recovery");
                d.icon("b3_onb_recovery_icon", "b3_info.svg", 14.0, tok::MUTED);
                d.text("b3_onb_recovery_text", t, &Txt::new(12.5, Face::Regular, tok::TEXT).w(W::Fill).wrap());
                d.close();
            }
            Note::Error { lead, cause } => {
                d.surface(
                    "b3_onb_error",
                    "width: Fill height: Fit flow: Right spacing: 8 align: Align{x: 0.0 y: 0.0} padding: Inset{left: 10 right: 10 top: 8 bottom: 8}",
                    tok::RED_BG,
                    8.0,
                    Some("#f3c4c7ff"),
                );
                d.icon("b3_onb_error_icon", "b3_warning.svg", 14.0, tok::RED);
                let col = d.anon();
                d.view(&col, "width: Fill height: Fit flow: Down spacing: 2");
                match lead {
                    Some(l) => {
                        d.text("b3_onb_error_text", l, &Txt::new(13.0, Face::Medium, tok::RED).w(W::Fill).wrap());
                        d.open(
                            "b3_onb_error_scroll",
                            "ScrollYView",
                            &format!("width: Fill height: Fit max_height: {CAUSE_MAX_H} flow: Down padding: Inset{{left: 0 top: 0 right: 6 bottom: 0}}"),
                        );
                        d.text("b3_onb_error_detail", cause, &Txt::new(11.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
                        d.close();
                    }
                    None => d.text("b3_onb_error_text", cause, &Txt::new(12.5, Face::Regular, tok::RED).w(W::Fill).wrap()),
                }
                d.close();
                d.close();
            }
        }
    }
}

/// The plain line over an error worded for developers (no server message):
/// what failed, in the panel's own words.
const SETUP_FAILED: &str = "Octos could not finish the setup.";

/// The form's notes, in the web's order: the retained profile, then the
/// error. Only the form shows them (the catalog failure card carries its
/// own error).
fn form_notes(st: &State) -> Vec<Note> {
    if !st.supported || st.catalog.is_none() || st.phase == Phase::LoadingCatalog {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Some(created) = &st.created_profile_id {
        out.push(Note::Recovery(format!("Profile {created} exists. A retry only repeats provider test and save.")));
    }
    if let Some(e) = &st.error {
        let lead = st
            .error_lead
            .clone()
            .or_else(|| ui::is_protocol_error(e).then(|| SETUP_FAILED.to_owned()));
        out.push(Note::Error { lead, cause: e.clone() });
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Foot {
    None,
    Fallback,
    Failure,
    Form,
}

/// `OnboardingFallback` (`OnboardingPanel.tsx:288-306`): the canonical TUI
/// setup and no way to start here.
fn fallback(d: &mut Dsl) {
    d.surface("b3_onb_fallback", "width: Fill height: Fit flow: Down spacing: 9 padding: Inset{left: 14 right: 14 top: 14 bottom: 14}", tok::SURFACE2, 10.0, Some(tok::HAIRLINE));
    let head = d.anon();
    d.view(&head, "width: Fill height: Fit flow: Right spacing: 8 align: Align{x: 0.0 y: 0.5}");
    d.icon("b3_onb_fallback_icon", "b3_terminal.svg", 16.0, tok::TEXT);
    d.text("b3_onb_fallback_head", FALLBACK_HEAD, &Txt::new(13.0, Face::Semibold, tok::TEXT).w(W::Fill).wrap());
    d.close();
    d.text("b3_onb_fallback_body", FALLBACK_BODY, &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    let w = ui::text_w(FALLBACK_COMMAND, 12.0, Face::Mono) + 16.0;
    d.surface(
        "b3_onb_fallback_cmd_box",
        &format!("width: {} height: 28 flow: Right align: Align{{x: 0.5 y: 0.5}}", w.ceil()),
        tok::SURFACE,
        6.0,
        Some(tok::HAIRLINE),
    );
    d.text("b3_onb_fallback_cmd", FALLBACK_COMMAND, &Txt::new(12.0, Face::Mono, tok::TEXT));
    d.close();
    d.close();
}

/// The phone's picker sheet: the open select's options take the body (a list
/// unfolded under a lower field would open below the screen's edge), with
/// a back control to the form.
fn picker_sheet(d: &mut Dsl, st: &State, which: Select, inner_w: f64) {
    let (title, mono) = match which {
        Select::Provider => ("Provider", true),
        Select::Model => ("Model", true),
        Select::Route => ("Route", true),
    };
    let head = d.anon();
    d.view(&head, "width: Fill height: 36 flow: Right spacing: 4 align: Align{x: 0.0 y: 0.5}");
    d.view("b3_onb_back_box", "width: 32 height: 32 flow: Overlay align: Align{x: 0.5 y: 0.5}");
    d.icon("b3_onb_back_icon", "b1_chevron_left.svg", 16.0, tok::TEXT);
    d.tap("b3_onb_back", &format!("b3.onb.select.{}", which.key()));
    d.close();
    d.text("b3_onb_sheet_title", title, &Txt::new(14.0, Face::Semibold, tok::TEXT).w(W::Fill));
    d.close();
    options_list(d, st, which, mono, inner_w, None);
}

/// The form (`OnboardingPanel.tsx:133-282`). `body_max` is the body's
/// visible height (a dropdown is capped to stay inside it).
fn form(d: &mut Dsl, st: &State, compact: bool, inner_w: f64, body_max: f64) {
    let busy = st.busy();
    let identity_open = !busy && st.created_profile_id.is_none();
    if compact && !busy {
        if let Some(which) = st.open_select {
            picker_sheet(d, st, which, inner_w);
            return;
        }
    }

    // Profile ID | Profile name (`.profileFields`: 0.72fr / 1.28fr; stacked
    // on a narrow frame, `@media (max-width: 760px)`).
    let (id_w, name_w) = if compact {
        (inner_w, inner_w)
    } else {
        let free = inner_w - 10.0;
        ((free * 0.36).floor(), free - (free * 0.36).floor())
    };
    let grid = d.anon();
    d.view(&grid, &format!("width: Fill height: Fit flow: {} spacing: 10", if compact { "Down" } else { "Right" }));
    let (cw_id, cw_name) = if compact { (W::Fill, W::Fill) } else { (W::Px(id_w), W::Px(name_w)) };
    // Every field value in the code font (the web's `.form input, .form
    // select { font: … var(--dsw-font-family-code) }`).
    text_field(d, "b3_onb_profile_id", "Profile ID", "onb.profile_id", &st.snap.0, &st.form.profile_id, true, identity_open, cw_id, id_w);
    text_field(d, "b3_onb_profile_name", "Profile name", "onb.profile_name", &st.snap.1, &st.form.profile_name, true, identity_open, cw_name, name_w);
    d.close();

    // The default checkbox (`OnboardingPanel.tsx:156-164`): the board's blue
    // toggle colour (a checkbox is a toggle; blue only for toggles + links).
    let label_w = ui::text_w(DEFAULT_LABEL, 13.0, Face::Regular);
    d.view("b3_onb_default_box", &format!("width: {} height: 32 flow: Overlay align: Align{{x: 0.0 y: 0.5}}", (label_w + 22.0 + 8.0 + 4.0).ceil()));
    let check = d.anon();
    d.view(&check, "width: Fill height: Fill flow: Right spacing: 8 align: Align{x: 0.0 y: 0.5}");
    d.icon("b3_onb_default_icon", if st.form.make_default { "cv_check_on.svg" } else { "cv_check_off.svg" }, 22.0, tok::BLUE);
    d.text("b3_onb_default_label", DEFAULT_LABEL, &Txt::new(13.0, Face::Regular, if identity_open { tok::TEXT } else { tok::FAINT }));
    d.close();
    if identity_open {
        d.tap("b3_onb_default", "b3.onb.default");
    }
    d.close();

    // Provider | Model | Route (`.fields`, three columns). The model id is
    // the long value (`claude-3-5-haiku-20241022`), so its column takes 40 %
    // where the web splits in thirds — the id reads whole instead of cut.
    let free = inner_w - 20.0;
    let (pw, mw) = ((free * 0.3).floor(), (free * 0.4).floor());
    let rw = free - pw - mw;
    let (pw, mw, rw) = if compact { (inner_w, inner_w, inner_w) } else { (pw, mw, rw) };
    let px = |w: f64| if compact { W::Fill } else { W::Px(w) };
    // The dropdown stays inside the body: what is above it (the identity
    // row, the checkbox, the select's own label + field) comes off.
    let list_max = (body_max - (59.0 + 12.0 + 32.0 + 12.0 + 59.0 + 6.0) - 14.0).clamp(110.0, 236.0);
    let grid = d.anon();
    d.view(&grid, &format!("width: Fill height: Fit flow: {} spacing: 10", if compact { "Down" } else { "Right" }));
    let route_label = st.routes().into_iter().find(|r| r.id == st.form.route_id).map(|r| r.label).unwrap_or_else(|| "Official API".into());
    select_field(d, st, Select::Provider, "Provider", &st.form.family_id, true, !busy, px(pw), pw, list_max);
    select_field(d, st, Select::Model, "Model", &st.form.model_id, true, !busy, px(mw), mw, list_max);
    select_field(d, st, Select::Route, "Route", &route_label, true, !busy, px(rw), rw, list_max);
    d.close();

    // The API key, or the keyless note (`OnboardingPanel.tsx:223-249`).
    if st.requires_key() {
        let col = d.anon();
        d.view(&col, "width: Fill height: Fit flow: Down spacing: 6");
        ui::field_label(d, "b3_onb_apikey_label", "API key");
        if busy {
            let dots: String = "•".repeat(st.form.api_key.chars().count().clamp(8, 24));
            disabled_field(d, "b3_onb_apikey", &dots, false, inner_w);
        } else {
            d.input_secret("b3_onb_apikey", "onb.api_key", "", 38.0);
        }
        d.text("b3_onb_apikey_hint", KEY_HINT, &Txt::new(11.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
        d.close();
    } else {
        note_box(d, "b3_onb_keyless");
        d.icon("b3_onb_keyless_icon", "b3_info.svg", 14.0, tok::MUTED);
        let col = d.anon();
        d.view(&col, &format!("width: Fill height: Fit flow: {} spacing: {}", if compact { "Down" } else { "Right" }, if compact { 2 } else { 7 }));
        d.text("b3_onb_keyless_head", KEYLESS_HEAD, &Txt::new(12.5, Face::Medium, tok::TEXT));
        d.text(
            "b3_onb_keyless_body",
            &format!("{} is marked keyless by the Core catalog.", st.form.family_id),
            &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
        );
        d.close();
        d.close();
    }

    // The retained profile and the error follow in the footer (`form_notes`).
}

fn pill_w(label: &str) -> f64 {
    (ui::text_w(label, 13.0, Face::Medium) + 32.0).ceil()
}

/// The actions row (`.launch-actions`): the status on the left, the buttons
/// on the right (a narrow frame puts the status on its own line).
fn footer_row(d: &mut Dsl, st: &State, kind: Foot, compact: bool) {
    let busy = st.busy();
    let row = d.anon();
    d.view(
        &row,
        &format!(
            "width: Fill height: Fit flow: {} spacing: 8 align: Align{{x: 0.0 y: 0.5}}",
            if compact && kind == Foot::Form { "Down" } else { "Right" }
        ),
    );
    if kind == Foot::Form {
        d.text("b3_onb_status", status(st.phase), &Txt::new(12.0, Face::Regular, tok::MUTED).w(if compact { W::Fill } else { W::Fit }));
    }
    let acts = d.anon();
    d.view(&acts, "width: Fill height: Fit flow: Right spacing: 8 align: Align{x: 1.0 y: 0.5}");
    match kind {
        Foot::Fallback => {
            d.button("b3_onb_disconnect", DISCONNECT, "b3.onb.disconnect", Btn::Outline, W::Fit, 36.0);
        }
        Foot::Failure => {
            d.button("b3_onb_retry", RETRY, "b3.onb.retry", Btn::Primary, W::Fit, 36.0);
            d.button("b3_onb_disconnect", DISCONNECT, "b3.onb.disconnect", Btn::Outline, W::Fit, 36.0);
        }
        Foot::Form => {
            d.button("b3_onb_disconnect", DISCONNECT, "b3.onb.disconnect", if busy { Btn::OutlineOff } else { Btn::Outline }, W::Fit, 36.0);
            // The submit's two variants swap by live visibility while the
            // key is typed (no remount).
            let label = if busy { WORKING } else { SUBMIT };
            let w = if compact { W::Fill } else { W::Px(pill_w(SUBMIT).max(pill_w(WORKING))) };
            let both = d.anon();
            d.view(&both, &format!("width: {} height: 36 flow: Overlay", w_dsl(w)));
            d.view("b3_onb_submit_off", "width: Fill height: Fit flow: Right");
            d.button("b3_onb_submit_disabled", label, "b3.onb.submit", Btn::Disabled, W::Fill, 36.0);
            d.close();
            // Armed whenever the panel is not busy: live visibility shows it
            // only while the gate holds, and `perform` re-checks the gate.
            d.view("b3_onb_submit_on", "width: Fill height: Fit flow: Right");
            d.button("b3_onb_submit", label, "b3.onb.submit", if busy { Btn::Disabled } else { Btn::Primary }, W::Fill, 36.0);
            d.close();
            d.close();
        }
        Foot::None => {}
    }
    d.close();
    d.close();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The web's own unit-test catalog (`use-onboarding.test.ts:10-31`).
    fn web_catalog() -> LlmCatalogResult {
        serde_json::from_value(json!({"families": {
            "deepseek": {"env": "DEEPSEEK_API_KEY", "models": [{"id": "deepseek-chat", "endpoints": [{
                "id": "openrouter", "label": "OpenRouter", "base_url": "https://openrouter.ai/api/v1",
                "api_key_env": "OPENROUTER_API_KEY", "api_type": "openai"}]}]}
        }}))
        .unwrap()
    }

    // use-onboarding.test.ts:34 "derives the official route from the server family"
    #[test]
    fn derives_the_official_route_from_the_server_family() {
        let s = selection_from_catalog(&web_catalog(), "deepseek", "deepseek-chat", OFFICIAL_ROUTE).unwrap();
        assert_eq!(
            serde_json::to_value(&s).unwrap(),
            json!({"family_id": "deepseek", "model_id": "deepseek-chat", "route": {
                "route_id": "deepseek", "label": "Official API", "api_key_env": "DEEPSEEK_API_KEY", "api_type": "openai"}})
        );
    }

    // use-onboarding.test.ts:53 "preserves a catalog endpoint and rejects stale selections"
    #[test]
    fn preserves_a_catalog_endpoint_and_rejects_stale_selections() {
        let r = selection_from_catalog(&web_catalog(), "deepseek", "deepseek-chat", "openrouter").unwrap().route;
        assert_eq!(r.route_id.as_deref(), Some("openrouter"));
        assert_eq!(r.base_url.as_deref(), Some("https://openrouter.ai/api/v1"));
        assert_eq!(r.api_key_env.as_deref(), Some("OPENROUTER_API_KEY"));
        let stale = selection_from_catalog(&web_catalog(), "deepseek", "removed", OFFICIAL_ROUTE).unwrap_err();
        assert!(stale.contains("no longer advertised"), "{stale}");
        assert_eq!(
            selection_from_catalog(&web_catalog(), "deepseek", "deepseek-chat", "gone").unwrap_err(),
            "The selected provider route is no longer advertised."
        );
    }

    // use-onboarding.test.ts:74 "preserves a keyless official route without inventing an env name"
    #[test]
    fn preserves_a_keyless_official_route_without_inventing_an_env_name() {
        let c: LlmCatalogResult =
            serde_json::from_value(json!({"families": {"ollama": {"env": "", "models": [{"id": "qwen3", "endpoints": []}]}}})).unwrap();
        let r = selection_from_catalog(&c, "ollama", "qwen3", OFFICIAL_ROUTE).unwrap().route;
        assert_eq!(r.route_id.as_deref(), Some("ollama"));
        assert_eq!(serde_json::to_value(&r).unwrap()["api_key_env"], json!(""));
        let form = Form { family_id: "ollama".into(), model_id: "qwen3".into(), ..Default::default() };
        assert!(!requires_key(&c, &form), "a keyless family asks for no key");
    }

    #[test]
    fn the_secret_is_redacted_everywhere_and_the_message_capped() {
        let msg = "HTTP 401: Your api key: sk-test-dummy is invalid (sk-test-dummy)";
        let out = redact_secret(msg, "sk-test-dummy");
        assert!(!out.contains("sk-test-dummy") && out.matches("[redacted]").count() == 2, "{out}");
        assert_eq!(redact_secret(&"x".repeat(1500), "k").chars().count(), 1000);
        let form = Form { api_key: "sk-test-dummy".into(), ..Default::default() };
        assert!(!format!("{form:?}").contains("sk-test-dummy"), "Debug never prints the key");
    }

    #[test]
    fn an_empty_catalog_is_invalid_and_modelless_families_drop() {
        let empty: LlmCatalogResult = serde_json::from_value(json!({"families": {}})).unwrap();
        assert_eq!(validate_catalog(empty).unwrap_err(), CATALOG_REJECTED);
        let c: LlmCatalogResult = serde_json::from_value(json!({"families": {
            "a": {"env": "A", "models": []}, "b": {"env": "", "models": [{"id": "m", "endpoints": [{"id": "e", "label": ""}]}]}
        }}))
        .unwrap();
        let c = validate_catalog(c).unwrap();
        assert_eq!(c.families.len(), 1);
        assert_eq!(c.families[0].models[0].endpoints[0].label, None, "an empty label is absent");
    }

    #[test]
    fn the_catalog_effect_keeps_a_valid_choice_and_resets_the_route() {
        let mut form = Form { family_id: "deepseek".into(), model_id: "deepseek-chat".into(), route_id: "openrouter".into(), ..Default::default() };
        adopt_catalog(&mut form, &web_catalog());
        assert_eq!((form.family_id.as_str(), form.model_id.as_str(), form.route_id.as_str()), ("deepseek", "deepseek-chat", OFFICIAL_ROUTE));
        let mut form = Form { family_id: "gone".into(), ..Default::default() };
        adopt_catalog(&mut form, &web_catalog());
        assert_eq!(form.family_id, "deepseek", "a stale family falls back to the first");
    }
}
