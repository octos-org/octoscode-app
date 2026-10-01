//! Card #29a — Stage C wiring for board 2 screens **2.1 Connect**, **2.2
//! Connect failed**, **2.3 Onboarding** (the Stage-B cards
//! `design/stage-b/setup/cards/setup-01|02|03`, board 2 frames 1/2/3).
//!
//! This file owns the screens' meaning, the way [`crate::bindings`] owns the
//! conversation's (8.8 condition 2): a card names binding/action ids, this
//! module maps them to the store state, the web-oracle behaviour and the
//! protocol methods (through the production client).
//!
//! ## Web oracle (behaviour, not names — RULES "values, not names")
//!
//! - **Endpoint validation before any socket opens or an address is
//!   remembered** — the web's `connectionEndpointError`
//!   (`src-web/apps/web/src/features/connection/validation.ts:2-17`, pinned by
//!   `validation.test.ts:5/16/28` + `e2e/onboarding.spec.ts:18`). Ported
//!   check-for-check, string-for-string in [`endpoint_error`].
//! - **§5.1 connect-failure classification** — unreachable / rejected-token /
//!   origin-not-allowed, each its own message + actions; an unrelated error
//!   stays unclassified, never a false diagnosis
//!   (`connect-failure.ts:16-72`, pinned by `connect-failure.test.ts:8/15/21/27`).
//!   Ported in [`failure_for`]. A rejected token focuses the token field and
//!   keeps the typed value (`ConnectionPanel.tsx:76-90`).
//! - **Onboarding sequence** — `profile/llm/catalog`, then
//!   `profile/local/create`, then `profile/llm/test`, then
//!   `profile/llm/upsert` with `set_primary: true`
//!   (`onboarding-submission.ts:34-126`), the selection derived from the
//!   catalog (`onboarding-submission.ts:128-171`: the `__official__` route is
//!   built from the family, a catalog endpoint is preserved, a stale
//!   family/model throws). Methods ride the production client
//!   (`octoscode_client::domains::profile`).
//!
//! ## What is deliberately NOT here
//!
//! Screen switching / drawer / first-run card area is #28e (not merged). The
//! in-app mount waits for its containers; until then the screens mount behind
//! the `OCTOSCODE_SCREEN` flag (see [`Screen::from_env`]) and the headless
//! captures lower the cards through [`lower_screen`].
use serde_json::{json, Value};

// --------------------------------------------------------------------- oracle

/// The web's `connectionEndpointError` (`validation.ts:2-17`), verbatim copy.
/// `None` = the address is valid; the socket may open.
pub fn endpoint_error(endpoint: &str) -> Option<&'static str> {
    if endpoint.trim().is_empty() {
        return Some("Enter the address of your Octos server.");
    }
    let url = match url::Url::parse(endpoint.trim()) {
        Ok(u) => u,
        Err(_) => {
            return Some("Enter a complete address, such as http://localhost:18032.");
        }
    };
    if !matches!(url.scheme(), "http" | "https" | "ws" | "wss") {
        return Some("Use an http, https, ws, or wss address.");
    }
    // `URL.username`/`password` are ""/None when absent; so are query/fragment.
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Some(
            "Use the server address without credentials, query parameters, or a fragment. Put your token in Auth token.",
        );
    }
    None
}

/// The three §5.1 kinds (`connect-failure.ts:4-8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    Unreachable,
    RejectedToken,
    OriginNotAllowed,
}

/// One classified failure with its §5.1 copy (`connect-failure.ts:31-72`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub kind: FailureKind,
    pub message: String,
    /// The exact action list the panel renders under the message.
    pub actions: &'static [&'static str],
    /// True only for a rejected token: focus the field, keep the value.
    pub focus_token: bool,
}

/// The web's `classifyConnectFailure` + `connectFailureCopy`
/// (`connect-failure.ts:16-72`): exact string matches, endpoint-dependent copy
/// filled from the typed address; anything else stays unclassified.
pub fn failure_for(raw: &str, endpoint: &str) -> Option<Failure> {
    let (kind, message, actions, focus_token) = match raw {
        "Could not open the Octos UI Protocol connection" => (
            FailureKind::Unreachable,
            format!("Can't reach {endpoint}"),
            &["check the address", "Retry"][..],
            false,
        ),
        "The server refused this token" => (
            FailureKind::RejectedToken,
            "The server refused this token".to_owned(),
            &["Re-enter the token", "Retry"][..],
            true,
        ),
        "Origin not allowed" => (
            FailureKind::OriginNotAllowed,
            // The web appends its window origin here
            // (`connect-failure.ts:51-56`); the native app has no browser
            // origin, so the copy stays at the owner ask.
            "This site isn't allowed to talk to that server — ask the server owner to allow it."
                .to_owned(),
            &["Open Settings › Providers"][..],
            false,
        ),
        _ => return None,
    };
    Some(Failure {
        kind,
        message,
        actions,
        focus_token,
    })
}

// --------------------------------------------------------------- screen state

/// Which board-2 screen the flag mounts (`OCTOSCODE_SCREEN=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// setup-01 — 2.1 Connect.
    Connect,
    /// setup-02 — 2.2 Connect failed.
    ConnectFailed,
    /// setup-03 — 2.3 Onboarding.
    Onboarding,
}

impl Screen {
    /// `(screen, card dir under design/stage-b/setup/cards)`.
    pub const ALL: [(Screen, &'static str); 3] = [
        (Screen::Connect, "setup-01"),
        (Screen::ConnectFailed, "setup-02"),
        (Screen::Onboarding, "setup-03"),
    ];

    pub fn card_dir(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(s, _)| *s == self)
            .map(|(_, dir)| *dir)
            .expect("every screen has a card")
    }

    /// `OCTOSCODE_SCREEN`'s value → [`Screen`] (`None` = the flag is off, the
    /// module keeps its pre-#29a behaviour).
    pub fn from_env() -> Option<Self> {
        match std::env::var("OCTOSCODE_SCREEN").as_deref() {
            Ok("connect") => Some(Self::Connect),
            Ok("connect_failed") => Some(Self::ConnectFailed),
            Ok("onboarding") => Some(Self::Onboarding),
            _ => None,
        }
    }
}

/// The onboarding provider radio (`setup-03`'s three rows, top to bottom;
/// each row's `onboarding.provider.<id>` action names the catalog family).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    DeepSeek,
    Kimi,
    Glm,
}

impl Provider {
    /// The radio's action suffix = the catalog family id it selects.
    pub fn id(self) -> &'static str {
        match self {
            Provider::DeepSeek => "deepseek",
            Provider::Kimi => "kimi",
            Provider::Glm => "glm",
        }
    }

    fn from_action_suffix(suffix: &str) -> Option<Self> {
        match suffix {
            "deepseek" => Some(Self::DeepSeek),
            "kimi" => Some(Self::Kimi),
            "glm" => Some(Self::Glm),
            _ => None,
        }
    }
}

/// What the wire never carries: the screens' typed drafts, the validation /
/// classification results, the onboarding selection. One instance lives on
/// the module's [`crate::Bridge`]; the lowered card reads it through
/// [`copies`], tests and the action path through the plain fields.
#[derive(Debug, Clone)]
pub struct ConnectUi {
    /// 2.1/2.2: the Server field's live text (the card's authored default is
    /// the solo serve address, `setup-01` copy `server_field_text`).
    pub server: String,
    /// The Access token field's live text. KEPT on a rejected token — the web
    /// focuses the field without clearing it (`ConnectionPanel.tsx:76-90`).
    pub token: String,
    /// The endpoint-validation error, live while typing (the web validates on
    /// every change before any socket opens, `ConnectionPanel.tsx:30-49`).
    pub endpoint_error: Option<&'static str>,
    /// The classified §5.1 failure of the last attempt, if any.
    pub failure: Option<Failure>,
    /// The raw error when it classified to nothing — shown verbatim, never
    /// replaced by a diagnosis (`connect-failure.ts:12-14`).
    pub raw_error: Option<String>,
    /// "Last tried 9:41 PM ·" (`setup-02`'s authored copy row `t_last_text`).
    pub last_tried: String,
    /// 2.3: the Profile name field (the card's authored default `octos-dev`).
    pub profile_name: String,
    /// The selected provider radio (the card's first row is pre-selected).
    pub provider: Provider,
    /// The API key field (the Kimi/GLM rows; DeepSeek's Official API takes
    /// the server-side env key, the web keeps the field key-optional then).
    pub api_key: String,
    /// The catalog fetch error, if `profile/llm/catalog` failed (the web's
    /// `phase: ready` + `error` state, `use-onboarding.ts:96-105`).
    pub catalog_error: Option<String>,
    /// The `profile/local/create` result's assigned profile id once created.
    pub created_profile: Option<String>,
    /// The onboarding error line (the panel's red message slot).
    pub onboarding_error: Option<String>,
}

impl Default for ConnectUi {
    fn default() -> Self {
        Self {
            server: "http://127.0.0.1:50190".to_owned(),
            token: String::new(),
            endpoint_error: None,
            failure: None,
            raw_error: None,
            last_tried: String::new(),
            profile_name: "octos-dev".to_owned(),
            provider: Provider::DeepSeek,
            api_key: String::new(),
            catalog_error: None,
            created_profile: None,
            onboarding_error: None,
        }
    }
}

impl ConnectUi {
    /// Record a connect attempt's outcome (called by the module's action path
    /// when the transport answers). `at` is the wall-clock "9:41 PM" string
    /// the card's `t_last_text` row shows.
    pub fn note_connect_error(&mut self, raw: &str, at: &str) {
        self.last_tried = format!("Last tried {at} ·");
        match failure_for(raw, &self.server) {
            Some(f) => {
                self.raw_error = None;
                self.failure = Some(f);
            }
            None => {
                self.failure = None;
                self.raw_error = Some(raw.to_owned());
            }
        }
    }
}

// ------------------------------------------------------------ binding table

/// The screens' data ids — resolved by [`query`]. Declared beside the
/// conversation's table (the audit pattern of `bindings::WEB_BINDINGS`).
pub const BINDINGS: &[(&str, &str)] = &[
    ("connect.endpoint_error", "the live endpoint-validation error or null"),
    (
        "connect.failure",
        "the §5.1 failure as {kind,message,actions[],focus_token} or null",
    ),
    ("connect.raw_error", "the unclassified last error or null"),
    ("onboarding.provider", "the selected provider id (deepseek|kimi|glm)"),
    ("onboarding.catalog_error", "the profile/llm/catalog fetch error or null"),
    ("onboarding.created_profile", "the created profile id or null"),
    ("onboarding.error", "the onboarding error line or null"),
];

/// Resolve a screen binding id. The conversation ids stay in
/// [`crate::bindings::query`]; an unknown id is `None` there and here.
pub fn query(ui: &ConnectUi, id: &str) -> Option<Value> {
    Some(match id {
        "connect.endpoint_error" => json!(ui.endpoint_error),
        "connect.failure" => match &ui.failure {
            Some(f) => json!({
                "kind": match f.kind {
                    FailureKind::Unreachable => "unreachable",
                    FailureKind::RejectedToken => "rejected-token",
                    FailureKind::OriginNotAllowed => "origin-not-allowed",
                },
                "message": f.message,
                "actions": f.actions,
                "focus_token": f.focus_token,
            }),
            None => Value::Null,
        },
        "connect.raw_error" => json!(ui.raw_error),
        "onboarding.provider" => json!(ui.provider.id()),
        "onboarding.catalog_error" => json!(ui.catalog_error),
        "onboarding.created_profile" => json!(ui.created_profile),
        "onboarding.error" => json!(ui.onboarding_error),
        _ => return None,
    })
}

/// The screens' action ids — exactly the three cards' `service-actions.json`
/// events (setup-01/02/03), each routed in [`resolve`].
pub const ACTIONS: &[(&str, &str)] = &[
    ("connect", "validate the endpoint, then open the connection (setup-01/02)"),
    ("connect.retry", "re-run the connect with the current fields (setup-02's Retry)"),
    ("connect.use_local_solo", "fill the solo serve address and connect"),
    ("input.server", "the Server field's live text"),
    ("input.token", "the Access token field's live text"),
    ("input.profile", "the Profile name field's live text"),
    ("input.apikey", "the API key field's live text"),
    ("onboarding.provider.deepseek", "select the DeepSeek radio"),
    ("onboarding.provider.kimi", "select the Kimi radio"),
    ("onboarding.provider.glm", "select the GLM radio"),
    (
        "create_profile",
        "catalog → local/create → llm/test → llm/upsert, then open the session",
    ),
];

/// Whether `id` is one of this screen set's action ids.
pub fn is_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id)
}

/// What an action means. UI-local effects land on [`ConnectUi`] in [`apply`];
/// the returned transport effect is performed by the caller (the module's
/// action path with the runtime + conversation at hand).
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Open the connection to this `(server, token)` — validation already
    /// passed in [`apply`].
    Connect { server: String, token: String },
    /// Re-run the connect with the current fields (resolved from
    /// `connect.retry`; [`apply`] expands it to [`Effect::Connect`]).
    Retry,
    /// `input.*`: a field's new live text (already applied).
    Input { field: &'static str, value: String },
    /// A provider radio was selected (already applied).
    SelectProvider(Provider),
    /// Run the onboarding protocol sequence.
    CreateProfile,
    /// The id was not one of this screen set's.
    Unhandled(String),
}

impl Effect {
    /// Test helper: did this effect fall through to Unhandled?
    pub fn is_unhandled(&self) -> bool {
        matches!(self, Effect::Unhandled(_))
    }
}

/// Route one action id to its effect. `value` carries the `input.*` payload
/// (the field's live text); the rest ignore it.
pub fn resolve(id: &str, value: Option<&str>) -> Effect {
    match id {
        "connect" => Effect::Connect {
            server: String::new(), // filled from the state in [`apply`]
            token: String::new(),
        },
        "connect.retry" => Effect::Retry,
        "connect.use_local_solo" => Effect::Connect {
            server: "http://127.0.0.1:50190".to_owned(),
            token: String::new(),
        },
        "input.server" => Effect::Input {
            field: "server",
            value: value.unwrap_or_default().to_owned(),
        },
        "input.token" => Effect::Input {
            field: "token",
            value: value.unwrap_or_default().to_owned(),
        },
        "input.profile" => Effect::Input {
            field: "profile",
            value: value.unwrap_or_default().to_owned(),
        },
        "input.apikey" => Effect::Input {
            field: "apikey",
            value: value.unwrap_or_default().to_owned(),
        },
        "onboarding.provider.deepseek" => Effect::SelectProvider(Provider::DeepSeek),
        "onboarding.provider.kimi" => Effect::SelectProvider(Provider::Kimi),
        "onboarding.provider.glm" => Effect::SelectProvider(Provider::Glm),
        "create_profile" => Effect::CreateProfile,
        other => Effect::Unhandled(other.to_owned()),
    }
}

/// Apply an effect to the screens' state (the UI-local half). Returns the
/// transport effect to perform, if any — validation runs BEFORE anything is
/// remembered or any socket opens (`ConnectionPanel.tsx:30-49` ordering).
pub fn apply(ui: &mut ConnectUi, effect: Effect) -> Option<Effect> {
    match effect {
        Effect::Input { field, value } => {
            match field {
                "server" => {
                    ui.endpoint_error = endpoint_error(&value);
                    ui.server = value;
                }
                "token" => ui.token = value,
                "profile" => ui.profile_name = value,
                "apikey" => ui.api_key = value,
                _ => unreachable!("resolve only emits the four fields above"),
            }
            None
        }
        Effect::SelectProvider(p) => {
            ui.provider = p;
            None
        }
        Effect::Connect { server, token } => {
            let server = if server.is_empty() { ui.server.clone() } else { server };
            if let Some(err) = endpoint_error(&server) {
                ui.endpoint_error = Some(err);
                return None;
            }
            ui.endpoint_error = None;
            ui.server = server.clone();
            Some(Effect::Connect { server, token })
        }
        Effect::Retry => apply(
            ui,
            Effect::Connect {
                server: ui.server.clone(),
                token: ui.token.clone(),
            },
        ),
        transport @ (Effect::CreateProfile | Effect::Unhandled(_)) => Some(transport),
    }
}

// ------------------------------------------------- the onboarding protocol run

/// The keyless-compatibility probe the web sends when the selected route has
/// no `api_key_env` (`onboarding-submission.ts:13`:
/// `octoscode-web-keyless-probe` — upsert never persists it).
pub const KEYLESS_PROBE: &str = "octoscode-web-keyless-probe";

/// The web's `selectionFromCatalog` (`onboarding-submission.ts:128-171`) for
/// the radio's Official-API default route (`OnboardingPanel.tsx:29-30`: the
/// family's first model, the `__official__` route). A stale family or model
/// throws the web's exact error.
pub fn selection_from_catalog(
    catalog: &octoscode_client::domains::profile::LlmCatalogResult,
    provider: Provider,
) -> Result<octoscode_client::domains::profile::LlmSelection, String> {
    use octoscode_client::domains::profile::{LlmRouteSelection, LlmSelection};
    let stale = || "The selected provider or model is no longer advertised.".to_owned();
    let family = catalog.families.iter().find(|f| f.id == provider.id()).ok_or_else(stale)?;
    let model = family.models.first().ok_or_else(stale)?;
    Ok(LlmSelection {
        family_id: family.id.clone(),
        model_id: model.id.clone(),
        route: LlmRouteSelection {
            route_id: Some(family.id.clone()),
            label: Some("Official API".to_owned()),
            base_url: None,
            api_key_env: Some(family.env.clone()),
            api_type: Some("openai".to_owned()),
        },
        inference: Default::default(),
    })
}

/// What a successful [`run_onboarding`] reports back to the caller.
#[derive(Debug, Clone, PartialEq)]
pub struct OnboardingOutcome {
    /// The server-assigned profile id (`ProfileLocalCreateResult::profile_id`,
    /// slug-collided server-side — may differ from the requested one).
    pub profile_id: String,
    /// The selection the provider was provisioned with.
    pub selection: octoscode_client::domains::profile::LlmSelection,
}

/// The onboarding sequence, the web's `submitOnboarding`
/// (`onboarding-submission.ts:34-126`) on the production client:
/// catalog → "profile/local/create" → "profile/llm/test" →
/// "profile/llm/upsert" (`set_primary: true`). `catalog` passes the panel's
/// already-fetched catalog (`None` = fetch it here, the `prepare` step,
/// `use-onboarding.ts:91`). Every failure is the web's own message.
pub async fn run_onboarding(
    client: &octoscode_client::Client,
    requested_id: &str,
    profile_name: &str,
    api_key: &str,
    provider: Provider,
    catalog: Option<octoscode_client::domains::profile::LlmCatalogResult>,
) -> Result<OnboardingOutcome, String> {
    use octoscode_client::domains::profile::{
        LlmCatalog, LlmCatalogParams, LlmProvisionParams, LlmTest, LlmUpsert,
        ProfileLocalCreate,
    };
    use octos_core::ui_protocol::ProfileLocalCreateParams;

    // 1. the catalog (the panel's `prepare`), unless the caller already has it.
    let catalog = match catalog {
        Some(c) => c,
        None => client
            .call::<LlmCatalog>(LlmCatalogParams {})
            .await
            .map_err(|e| e.to_string())?,
    };

    // 2. the selection from the catalog (stale → the web's exact error).
    let selection = selection_from_catalog(&catalog, provider)?;

    // 3. the required-field gate (`onboarding-submission.ts:44-51`): the key
    //    is required exactly when the route names an `api_key_env`.
    let requires_key = selection
        .route
        .api_key_env
        .as_deref()
        .unwrap_or("")
        .is_empty()
        == false;
    let id = requested_id.trim();
    let name = profile_name.trim();
    if id.is_empty() || name.is_empty() || (requires_key && api_key.trim().is_empty()) {
        return Err(if requires_key {
            "Profile ID, profile name, and API key are required."
        } else {
            "Profile ID and profile name are required."
        }
        .to_owned());
    }
    // The keyless probe: a non-empty test value the upsert never persists.
    let wire_key = if api_key.trim().is_empty() {
        KEYLESS_PROBE
    } else {
        api_key.trim()
    };

    // 4. create the local profile (`onboarding-submission.ts:57-71`).
    let created = client
        .call::<ProfileLocalCreate>(ProfileLocalCreateParams {
            requested_id: Some(id.to_owned()),
            name: name.to_owned(),
            username: String::new(),
            email: String::new(),
            make_default: None,
        })
        .await
        .map_err(|e| e.to_string())?;
    let created_profile_id = created.profile_id;

    // 5. test the provider (`onboarding-submission.ts:85-105`).
    let tested = client
        .call::<LlmTest>(LlmProvisionParams {
            profile_id: Some(created_profile_id.clone()),
            selection: selection.clone(),
            api_key: Some(wire_key.to_owned()),
            set_primary: None,
        })
        .await
        .map_err(|e| e.to_string())?;
    if tested.profile_id != created_profile_id || !tested.applied {
        let msg = match tested.error.filter(|e| !e.is_empty()) {
            Some(e) => e,
            None if !tested.message.is_empty() => tested.message,
            None => "The provider test did not pass.".to_owned(),
        };
        return Err(msg);
    }

    // 6. save it as the primary (`onboarding-submission.ts:107-125`).
    let saved = client
        .call::<LlmUpsert>(LlmProvisionParams {
            profile_id: Some(created_profile_id.clone()),
            selection: selection.clone(),
            api_key: Some(wire_key.to_owned()),
            set_primary: Some(true),
        })
        .await
        .map_err(|e| e.to_string())?;
    if saved.profile_id != created_profile_id || !saved.applied {
        return Err("The server did not apply the tested provider.".to_owned());
    }

    Ok(OnboardingOutcome {
        profile_id: created_profile_id,
        selection,
    })
}

/// `H:MM AM/PM` wall clock for the "Last tried" row (`setup-02`'s authored
/// copy shows a 12-hour time; the lane runs UTC+8, stated in the report).
pub fn clock_12h() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let sod = (now + 8 * 3600) % 86_400;
    let (h24, m) = (sod / 3600, (sod % 3600) / 60);
    let ampm = if h24 < 12 { "AM" } else { "PM" };
    let h12 = match h24 % 12 {
        0 => 12,
        h => h,
    };
    format!("{h12}:{m:02} {ampm}")
}

// ----------------------------------------------------------------- card lower

/// The live `copy` rewrites for one screen: the card's authored ids
/// (`setup-NN/page.card`) mapped to the live state, so the lowered screen
/// shows real values while its structure and kit stay untouched (the
/// [`crate::l0_host`] injection contract).
pub fn copies(screen: Screen, ui: &ConnectUi) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut push = |id: &str, v: &str| out.push((id.to_owned(), v.to_owned()));
    match screen {
        Screen::Connect | Screen::ConnectFailed => {
            push("server_field_text", &ui.server);
            push("token_field_text", "");
            push(
                "token_dots_text",
                if ui.token.is_empty() {
                    ""
                } else {
                    "••••••••••••••••••••"
                },
            );
            if screen == Screen::ConnectFailed {
                if let Some(f) = &ui.failure {
                    push("t_error_text", &f.message);
                } else if let Some(raw) = &ui.raw_error {
                    push("t_error_text", raw);
                }
                if !ui.last_tried.is_empty() {
                    push("t_last_text", &ui.last_tried);
                }
            }
        }
        Screen::Onboarding => {
            push("profile_field_text", &ui.profile_name);
            let masked = !ui.api_key.is_empty();
            push("apikey_field_text", "");
            push(
                "apikey_dots_text",
                if masked {
                    "••••••••••••••••••••"
                } else {
                    ""
                },
            );
            if let Some(err) = &ui.onboarding_error {
                push("t_keyhint_text", err);
            }
        }
    }
    out
}

/// Lower one screen's card to the module's DSL with the live copies injected —
/// the [`crate::l0_host`] chain (`l0::prepare` → `to_makepad_ui`) reading the
/// Stage-B card straight from `design/stage-b/setup/cards/<dir>/`. The
/// feature-flagged mount and the headless captures both use this.
pub fn lower_screen(screen: Screen, ui: &ConnectUi) -> Result<String, String> {
    let dir = format!(
        "{}/{}",
        crate::design::dir("stage-b/setup/cards").display(),
        screen.card_dir()
    );
    let card_src = std::fs::read_to_string(format!("{dir}/page.card"))
        .map_err(|e| format!("read {dir}/page.card: {e}"))?;
    let data: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{dir}/page.data.json"))
            .map_err(|e| format!("read {dir}/page.data.json: {e}"))?,
    )
    .map_err(|e| format!("parse page.data.json: {e}"))?;
    let card_src = crate::l0_host::apply_copies(&card_src, &copies(screen, ui));
    let prepared = octoscript_makepad::l0::prepare(
        &card_src,
        &data,
        std::path::Path::new(&format!("{dir}/kit")),
    )
    .map_err(|e| format!("l0::prepare: {e}"))?;
    let mut tree = prepared.tree;
    octoscript_makepad::l0::inspectable(&mut tree);
    let dsl = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(&tree))
        .map_err(|e| format!("to_makepad_ui: {e}"))?;
    let dsl = phone_layout(&dsl);
    let dsl = theme_surfaces(&dsl);
    Ok(mask_secret_inputs(&wire_events(&dsl, screen), screen))
}

/// #36b item 3: the Connect card must not stay light when the OS/app theme is
/// dark (the Android shell is dark, and the entry reported the light Connect
/// card against it).
///
/// The web's Connect is NOT light-only: `.gate`/`.card` paint from design
/// tokens (`var(--dsw-alias-bg-base)`, `--dsw-alias-bg-layer-1`) and
/// `features/connection/` has no `prefers-color-scheme` of its own, so it
/// follows the global theme.
///
/// There is no authored dark Connect twin on disk (`setup-01/kit/native/`
/// ships only `light`; `theme::card_for` has no connect entry), so the dark
/// palette is applied at the TOKEN level, the same way #31d's dark screens
/// carry the dark token set: the authored light hexes are remapped to the dark
/// role values from `theme::role_assignments` (#31d, theme.rs:556-566) —
/// `color_bg_app #1c1f22`, `color_outset_1 #38383a`. In `light` (and with no
/// preference resolvable) the card is left byte-identical to the authored
/// Stage B output, so the desktop look does not move.
/// #36b item 3: the Connect card must not stay light when the OS/app theme is
/// dark (the Android shell is dark, and the entry reported the light Connect
/// card against it).
///
/// The web's Connect is NOT light-only: `.gate`/`.card` paint from design
/// tokens (`var(--dsw-alias-bg-base)`, `--dsw-alias-bg-layer-1`) and
/// `features/connection/` has no `prefers-color-scheme` of its own, so it
/// follows the global theme.
///
/// There is no authored dark Connect twin on disk (`setup-01/kit/native/`
/// ships only `light`; `theme::card_for` has no connect entry), so the dark
/// palette is applied at the TOKEN level, the way #31d's dark screens carry the
/// dark token set: the authored light hexes are remapped to the dark role
/// values from `theme::role_assignments` (#31d, theme.rs:556-566) —
/// `color_bg_app #1c1f22`, `color_bg_even #2c2c2e`, `color_outset_1 #38383a`,
/// `color_fg_app #f5f5f7`, `color_text_muted #98989d`.
///
/// TEXT must be remapped too, or the dark surface carries dark-on-dark labels:
/// the authored set is `#1d1d1f` x2 (primary), `#6e6e73` / `#6d6d6e` (muted),
/// `#141415` / `#191919`, and the field borders `#d2d2d7` x2. The accent
/// `#1651f3` is the brand blue and is kept in both palettes.
///
/// `#000000ff` is AMBIGUOUS in this card — it is the "Connect to Octos" TEXT
/// colour AND the Connect button's surface fill — so it is remapped per
/// PROPERTY (`draw_text.color:` vs `draw_bg.color:`), never by value: a global
/// value replace would repaint one of them wrongly.
///
/// In `light` (and with no preference resolvable) the card is left
/// byte-identical to the authored Stage B output, so the desktop look does not
/// move.
fn theme_surfaces(dsl: &str) -> String {
    if crate::screens::theme::resolved() != "dark" {
        return dsl.to_owned();
    }
    let mut out = dsl
        // surfaces
        .replace("draw_bg.color: #ffffffff", "draw_bg.color: #1c1f22ff")
        // the Connect button pill: a black disc in light, the raised dark
        // surface in dark (`color_bg_even`)
        .replace("draw_bg.color: #000000ff", "draw_bg.color: #2c2c2eff")
        // borders: the card outline, the two field outlines
        .replace("draw_bg.border_color: #e5e5e7ff", "draw_bg.border_color: #38383aff")
        .replace("draw_bg.border_color: #d2d2d7ff", "draw_bg.border_color: #38383aff")
        // primary + near-black text
        .replace("draw_text.color: #1d1d1fff", "draw_text.color: #f5f5f7ff")
        .replace("draw_text.color: #141415ff", "draw_text.color: #f5f5f7ff")
        .replace("draw_text.color: #191919ff", "draw_text.color: #f5f5f7ff")
        // the "Connect to Octos" title — the ambiguous #000000, TEXT property only
        .replace("draw_text.color: #000000ff", "draw_text.color: #f5f5f7ff")
        // muted labels and the stored-for hint
        .replace("draw_text.color: #6e6e73ff", "draw_text.color: #98989dff")
        .replace("draw_text.color: #6d6d6eff", "draw_text.color: #98989dff");
    // The Connect button's own LABEL is authored light (#fdfdfd) on the black
    // disc; on the dark raised surface it still reads, so it is left alone.
    out
}

/// #36b: the Connect card at PHONE width (the 6T's 384x788 logical viewport).
///
/// Two measured defects, both traceable to the authored card
/// (`design/stage-b/setup/cards/setup-01/mapped.json`):
///
/// 1. **The card is flush right.** The artboard is a fixed 406 wide (`page w=406`,
///    `outer_card x=10 w=386` -> authored right edge 396), so at 384 it overflows:
///    `/snap` on main gave the card `[10, 32, 374, 734]` — left margin 10, right
///    margin **0**. The web gives EQUAL sides at phone width:
///    `ConnectionPanel.module.css:319-321` —
///    `@media (max-width: 560px) { .gate { padding: 20px 16px } }`.
///
/// 2. **The field inputs are not centred in their boxes, and one escapes.** Both
///    are authored at a fixed **+12** from the box top, but the two boxes differ
///    in height (server 44.3, token 34.15), so the same offset overflows the
///    shorter one. `/snap` on main: server insets 12/6 (6.8px off-centre), token
///    input bottom **381 vs box bottom 377** (4px outside). The token dots row
///    (insets 6.08/5.57) is the authored proof the intent is centring.
///
/// Both are no-ops at desktop widths: the clamps only fire when the authored
/// artboard does not fit the viewport.
fn phone_layout(dsl: &str) -> String {
    const VIEWPORT_W: f64 = 384.0;
    const WEB_INSET: f64 = 16.0;
    let Some(card) = node_rect(dsl, "beauty_0_0") else {
        return dsl.to_owned();
    };
    let mut out = dsl.to_owned();

    // (1) equal side margins, matching the web's phone padding
    if card.0 + card.2 > VIEWPORT_W {
        let inset = card.0.min(WEB_INSET);
        out = set_node_x(&out, "beauty_0_0", inset);
        out = set_node_w(&out, "beauty_0_0", VIEWPORT_W - inset * 2.0);
    }

    // (2) centre each TextInput in its field box (top inset == bottom inset).
    // The RENDERER snaps to integer pixels: `/snap` draws the token input at
    // h=26 (authored 25.5) inside a h=34 box (authored 34.15), so centring on
    // the authored floats leaves the live field 2px high (insets 5/3) even
    // though the DSL numbers balance. Centre on the drawn geometry — the
    // rounded box and input heights — so the seat is right on screen.
    for (bx, ix) in [
        ("beauty_0_0_2", "beauty_0_0_2_0"),
        ("beauty_0_0_4", "beauty_0_0_4_0"),
    ] {
        let (Some(b), Some(i)) = (node_rect(&out, bx), node_rect(&out, ix)) else {
            continue;
        };
        let drawn_bh = b.3.round();
        let drawn_ih = i.3.round();
        out = set_node_y(&out, ix, b.1 + (drawn_bh - drawn_ih) / 2.0);
    }
    out
}

/// `(x, y, w, h)` of one lowered node, read from its own head block.
fn node_rect(dsl: &str, node: &str) -> Option<(f64, f64, f64, f64)> {
    let head = node_head(dsl, node)?;
    let num = |key: &str| -> Option<f64> {
        let at = head.find(key)?;
        let tail = &head[at + key.len()..];
        let end = tail
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(tail.len());
        tail[..end].parse().ok()
    };
    let at = head.find("abs_pos: vec2(")?;
    let tail = &head[at + 14..];
    let close = tail.find(')')?;
    let mut it = tail[..close].split(',');
    Some((
        it.next()?.trim().parse().ok()?,
        it.next()?.trim().parse().ok()?,
        num("width: ")?,
        num("height: ")?,
    ))
}

/// One node's own lowered head — from `<node> := ` down to the next node
/// initializer or a closing brace. The head spans SEVERAL lines
/// (`width: 386 height: 756` is not on the `:=` line), so this cannot be
/// line-at-a-time.
fn node_head<'a>(dsl: &'a str, node: &str) -> Option<&'a str> {
    let at = dsl.find(&format!("{node} := "))?;
    let mut end = at;
    for line in dsl[at..].lines() {
        if end > at && (line.contains(" := ") || line.starts_with('}')) {
            break;
        }
        end += line.len() + 1;
    }
    Some(&dsl[at..end.min(dsl.len())])
}

/// One node's `x` in `abs_pos: vec2(x, y)`.
fn set_node_x(dsl: &str, node: &str, x: f64) -> String {
    let Some(r) = node_rect(dsl, node) else {
        return dsl.to_owned();
    };
    set_node_xy(dsl, node, x, r.1)
}

/// One node's `y` in `abs_pos: vec2(x, y)`.
fn set_node_y(dsl: &str, node: &str, y: f64) -> String {
    let Some(r) = node_rect(dsl, node) else {
        return dsl.to_owned();
    };
    set_node_xy(dsl, node, r.0, y)
}

/// One node's `abs_pos: vec2(x, y)`. The whole `(x, y)` payload is replaced —
/// the number span must run to the closing paren, or only `x` is replaced and
/// the old `y` survives (`vec2(10, 10, 10)`).
fn set_node_xy(dsl: &str, node: &str, x: f64, y: f64) -> String {
    const KEY: &str = "abs_pos: vec2(";
    let Some(head) = node_head(dsl, node) else {
        return dsl.to_owned();
    };
    let at = dsl.find(head).unwrap_or(0);
    let Some(k) = head.find(KEY) else {
        return dsl.to_owned();
    };
    let start = at + k;
    let inner = start + KEY.len();
    let close = dsl[inner..]
        .find(')')
        .map(|i| inner + i)
        .unwrap_or(dsl.len());
    let payload = format!("{x}, {y}");
    format!("{}{}{}", &dsl[..inner], payload, &dsl[close..])
}

/// One node's `width:`.
fn set_node_w(dsl: &str, node: &str, w: f64) -> String {
    replace_in_head(dsl, node, "width: ", &format!("{w}"))
}

/// Replace the number that follows `<key>` inside one node's head.
///
/// `value` is the bare number(s) — the key stays in the DSL, so inserting
/// `key + value` here would duplicate it (`width: width: 364`).
fn replace_in_head(dsl: &str, node: &str, key: &str, value: &str) -> String {
    let Some(head) = node_head(dsl, node) else {
        return dsl.to_owned();
    };
    let at = dsl.find(head).unwrap_or(0);
    let Some(k) = head.find(key) else {
        return dsl.to_owned();
    };
    let start = at + k + key.len();
    let end = start
        + dsl[start..]
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(0);
    format!("{}{}{}", &dsl[..start], value, &dsl[end..])
}

/// #32h item 1 (the dispatch layer, the outer loop's L4): the (widget name,
/// action id) pairs the wired taps created. The host maps the names to live
/// widget ids and routes the clicks — nothing dispatched clicks inside the
/// mounted screen to the screens' action tables (Stage C tested the tables
/// by calling ids directly, never by clicking).
// #35b item 1: the reader and the injector are card-agnostic, so they now live
// in [`super::taps`] and every docked screen's mount path uses them (this
// module was the ONLY caller, which is exactly why the palette/error cards
// mounted buttons that could never fire). Re-exported here so lib.rs:1833's
// existing connect-side call keeps its path.
pub use super::taps::wired_taps;

/// #32h item 5: the token / API-key fields must MASK typed input (the web:
/// ConnectionPanel.tsx:255 `type={showToken ? "text" : "password"}`); the
/// mapped card carries no password flag, so the lowered DesignInput echoes
/// the token in plain text (the device capture). Flip `is_password: true`
/// on the DesignInput blocks that sit inside a secret field's authored
/// rect. Non-secret inputs (Server, Profile name) keep their echo.
fn mask_secret_inputs(dsl: &str, screen: Screen) -> String {
    let Ok(text) = crate::design::file(&format!(
        "stage-b/setup/cards/{}/service-actions.json",
        screen.card_dir()
    )) else {
        return dsl.to_owned();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text.as_ref()) else {
        return dsl.to_owned();
    };
    let mut secrets: Vec<(f64, f64, f64, f64)> = Vec::new();
    if let Some(controls) = v.get("controls").and_then(|c| c.as_object()) {
        for (name, c) in controls {
            let ev = c.get("event").and_then(|e| e.as_str()).unwrap_or("");
            let secret = (name.contains("token") || name.contains("apikey"))
                && ev.starts_with("input.");
            if !secret {
                continue;
            }
            if let Some(b) = c.get("source_bounds").and_then(|b| b.as_array()) {
                let b: Vec<f64> = b.iter().filter_map(|x| x.as_f64()).collect();
                if b.len() == 4 {
                    secrets.push((b[0], b[1], b[2], b[3]));
                }
            }
        }
    }
    if secrets.is_empty() {
        return dsl.to_owned();
    }
    let lines: Vec<&str> = dsl.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut in_input = false;
    let mut mask_this = false;
    for l in lines {
        let trimmed = l.trim_end();
        if let Some(rest) = trimmed.strip_suffix(" {") {
            in_input = rest.split_once(":=").is_some_and(|(_, k)| k.trim() == "DesignInput");
            mask_this = false;
            out.push(l.to_owned());
            continue;
        }
        if trimmed == "}" {
            in_input = false;
            mask_this = false;
            out.push(l.to_owned());
            continue;
        }
        let mut line = l.to_owned();
        if in_input {
            if let Some(p) = trimmed.strip_prefix("abs_pos: vec2(") {
                let p = p.trim_end_matches(')');
                let mut it = p.split(',');
                if let (Some(px), Some(py)) = (it.next(), it.next()) {
                    if let (Ok(x), Ok(y)) = (px.trim().parse::<f64>(), py.trim().parse::<f64>()) {
                        mask_this = secrets.iter().any(|(bx, by, bw, bh)| {
                            x >= *bx && x <= bx + bw && y >= *by && y <= by + bh
                        });
                    }
                }
            }
            if mask_this && trimmed.contains("is_password: false") {
                // The flag rides the same line as empty_text:
                // `empty_text: "" is_password: false` — an exact-match
                // condition never fired (the 66c865a follow-up).
                line = l.replace("is_password: false", "is_password: true");
            }
        }
        out.push(line);
    }
    out.join("\n")
}

/// #32h item 1 (layer 2): the cards' own event contract
/// (`service-actions.json` controls) had NO consumer — the lowered button
/// blocks carried no on_click at all (the 118-line dump: on_click: 0), so a
/// tap hit the native Button and nothing fired it; the NAV global (now
/// registered) had nothing to route. Wire each CLICK control's event to the
/// emitted block whose abs_pos matches the control's authored
/// source_bounds: `on_click: || { NAV(t: "<event>") }` on the
/// DesignNativeButton (the only widget class the dialect attaches handlers
/// to — fork lib.rs:365-368). Field controls (input.*) stay unwired: they
/// are live text, and the masked ones are handled in item 5.
fn wire_events(dsl: &str, screen: Screen) -> String {
    // #35b item 1: the wiring body is the shared, card-dir-driven helper. This
    // module was its only caller, which is exactly why every other docked
    // screen mounted a card whose buttons could never fire.
    super::taps::wire_card_events(dsl, screen.card_dir())
}

/// Inject `on_click: || { NAV(t: "<event>") }` into the DesignNativeButton

#[cfg(test)]
mod tests {
    use super::*;

    // -- validation, the web's own cases (validation.test.ts:5/16/28) -------

    #[test]
    fn accepts_the_supported_transport_schemes() {
        for ok in [
            "http://localhost:18032",
            "https://octos.dev",
            "ws://127.0.0.1:50190",
            "wss://x.dev",
        ] {
            assert_eq!(endpoint_error(ok), None, "{ok} must validate");
        }
    }

    #[test]
    fn rejects_malformed_addresses_before_a_connection_attempt() {
        assert!(endpoint_error("").is_some());
        assert!(endpoint_error("   ").is_some());
        assert!(endpoint_error("not a url").is_some());
        assert!(endpoint_error("localhost:18032").is_some()); // parsed as scheme "localhost"
        assert!(endpoint_error("ftp://x.dev").is_some());
    }

    #[test]
    fn keeps_credentials_out_of_the_address_that_will_be_remembered() {
        assert!(endpoint_error("http://user:pw@x.dev").is_some());
        assert!(endpoint_error("http://x.dev/?a=b").is_some());
        assert!(endpoint_error("http://x.dev/#frag").is_some());
        // ...and the copy names the token field, like the web.
        assert!(endpoint_error("http://u@x.dev").unwrap().contains("Auth token"));
    }

    // -- §5.1 classification, the web's own cases (connect-failure.test.ts) --

    #[test]
    fn classifies_the_three_known_failures_with_their_own_copy() {
        let f = failure_for("Could not open the Octos UI Protocol connection", "http://x:1").unwrap();
        assert_eq!(f.kind, FailureKind::Unreachable);
        assert_eq!(f.message, "Can't reach http://x:1");
        assert_eq!(f.actions, &["check the address", "Retry"][..]);
        assert!(!f.focus_token);

        let f = failure_for("The server refused this token", "http://x").unwrap();
        assert_eq!(f.kind, FailureKind::RejectedToken);
        assert_eq!(f.message, "The server refused this token");
        assert_eq!(f.actions, &["Re-enter the token", "Retry"][..]);
        assert!(f.focus_token, "a rejected token focuses the token field");

        let f = failure_for("Origin not allowed", "http://x").unwrap();
        assert_eq!(f.kind, FailureKind::OriginNotAllowed);
        assert_eq!(f.actions, &["Open Settings › Providers"][..]);
    }

    #[test]
    fn leaves_an_unrelated_error_unclassified() {
        assert!(failure_for("some tls stack trace", "http://x").is_none());
    }

    // -- the screens' state machine ------------------------------------------

    #[test]
    fn input_server_validates_live_and_blocks_an_invalid_connect() {
        let mut ui = ConnectUi::default();
        assert!(apply(&mut ui, resolve("input.server", Some("ftp://x"))).is_none());
        assert!(ui.endpoint_error.is_some());
        // A connect with the invalid address is refused — no effect at all.
        let server = ui.server.clone();
        assert!(apply(&mut ui, Effect::Connect { server, token: String::new() }).is_none());
    }

    #[test]
    fn use_local_solo_fills_the_solo_address_and_connects() {
        let mut ui = ConnectUi::default();
        match apply(&mut ui, resolve("connect.use_local_solo", None)) {
            Some(Effect::Connect { server, .. }) => {
                assert_eq!(server, "http://127.0.0.1:50190");
            }
            other => panic!("expected a connect effect, got {other:?}"),
        }
    }

    #[test]
    fn retry_runs_with_the_current_fields() {
        let mut ui = ConnectUi::default();
        apply(&mut ui, resolve("input.token", Some("sk-test")));
        match apply(&mut ui, resolve("connect.retry", None)) {
            Some(Effect::Connect { token, .. }) => assert_eq!(token, "sk-test"),
            other => panic!("expected a connect effect, got {other:?}"),
        }
    }

    #[test]
    fn a_failed_attempt_is_classified_onto_the_state_and_the_copy_follows() {
        let mut ui = ConnectUi::default();
        apply(&mut ui, resolve("input.token", Some("sk-wrong")));
        ui.note_connect_error("The server refused this token", "9:41 PM");
        let f = ui.failure.clone().expect("classified");
        assert!(f.focus_token);
        assert_eq!(ui.token, "sk-wrong", "the typed value is KEPT");
        let copies = copies(Screen::ConnectFailed, &ui);
        assert!(copies.contains(&(
            "t_error_text".to_owned(),
            "The server refused this token".to_owned()
        )));
        assert!(copies
            .iter()
            .any(|(id, v)| id == "t_last_text" && v.contains("9:41 PM")));
    }

    #[test]
    fn an_unrelated_error_lands_verbatim_never_as_a_diagnosis() {
        let mut ui = ConnectUi::default();
        ui.note_connect_error("weird tls handshake", "9:41 PM");
        assert!(ui.failure.is_none());
        let copies = copies(Screen::ConnectFailed, &ui);
        assert!(copies.contains(&("t_error_text".to_owned(), "weird tls handshake".to_owned())));
    }

    #[test]
    fn provider_radios_round_trip_through_their_action_ids() {
        let mut ui = ConnectUi::default();
        assert!(apply(&mut ui, resolve("onboarding.provider.kimi", None)).is_none());
        assert_eq!(ui.provider, Provider::Kimi);
        assert_eq!(
            resolve("onboarding.provider.kimi", None),
            Effect::SelectProvider(Provider::Kimi)
        );
    }

    #[test]
    fn every_service_action_event_is_a_declared_routed_action() {
        // The three cards' committed service-actions.json events, audited.
        for id in [
            "connect",
            "connect.retry",
            "connect.use_local_solo",
            "input.server",
            "input.token",
            "input.profile",
            "input.apikey",
            "onboarding.provider.deepseek",
            "onboarding.provider.kimi",
            "onboarding.provider.glm",
            "create_profile",
        ] {
            assert!(is_action(id), "{id} must be declared");
            assert!(!resolve(id, Some("x")).is_unhandled(), "{id} must route");
        }
        assert!(resolve("nope", None).is_unhandled());
        assert!(!is_action("nope"));
    }

    // -- the binding table ----------------------------------------------------

    #[test]
    fn the_binding_table_resolves_and_stays_closed() {
        let ui = ConnectUi {
            failure: failure_for("The server refused this token", "http://x"),
            created_profile: Some("octos-dev".to_owned()),
            ..Default::default()
        };
        for (id, _) in BINDINGS {
            assert!(query(&ui, id).is_some(), "{id} must resolve");
        }
        assert!(query(&ui, "not.a.binding").is_none());
        let f = query(&ui, "connect.failure").unwrap();
        assert_eq!(f["kind"], "rejected-token");
        assert_eq!(f["focus_token"], true);
        assert_eq!(query(&ui, "onboarding.created_profile").unwrap(), "octos-dev");
    }

    // -- the selection derivation (onboarding-submission.ts:128-171) ----------

    #[test]
    fn the_official_route_is_derived_from_the_family() {
        let catalog = serde_json::from_value(json!({
            "families": {
                "deepseek": {"env": "DEEPSEEK_API_KEY", "models": [{"id": "deepseek-v4-flash", "endpoints": []}]},
                "kimi": {"env": "", "models": [{"id": "kimi-coding", "endpoints": []}]}
            }
        }))
        .expect("catalog json");
        let sel = selection_from_catalog(&catalog, Provider::DeepSeek).unwrap();
        assert_eq!(sel.family_id, "deepseek");
        assert_eq!(sel.model_id, "deepseek-v4-flash");
        assert_eq!(sel.route.route_id.as_deref(), Some("deepseek"));
        assert_eq!(sel.route.label.as_deref(), Some("Official API"));
        assert_eq!(sel.route.api_key_env.as_deref(), Some("DEEPSEEK_API_KEY"));
        assert_eq!(sel.route.api_type.as_deref(), Some("openai"));
    }

    #[test]
    fn a_stale_family_throws_the_web_error() {
        let catalog = serde_json::from_value(json!({"families": {}})).unwrap();
        assert_eq!(
            selection_from_catalog(&catalog, Provider::Glm).unwrap_err(),
            "The selected provider or model is no longer advertised."
        );
    }
}
