//! #D1/#A2 — the provider editor (atlas board 1 screens p4-06/07), one owner
//! per action id. A23 — the SAME editor serves every provider the web's
//! `ModelManagementSection` edits: opened from Settings > Model > Model
//! providers (the primary, A2's entry) and from the board-3 providers dialog
//! (`board3::routes`: "Edit" on ANY configured row, the catalog-driven "Add
//! provider"), with the web's separate "Test connection" and "Fetch available
//! models".
//!
//! The web's contract is `features/models/model-settings.ts` plus the spec
//! `e2e/model-management.spec.ts`. The properties the editor keeps, each with
//! its citation:
//!
//! 1. **Save tests first, with ONE request** — `save()` builds the provision
//!    params once, runs `profile/llm/test`, and only on a passing test sends
//!    `profile/llm/upsert` with `set_primary` (`model-settings.ts:345-380`:
//!    "Construct this ONCE. Test and save therefore cannot drift").
//! 2. **The draft is kept when the provider rejects the key** — walk row 88:
//!    after a failed test the family, model, route and the typed key all still
//!    hold their values (`model-management.spec.ts:182-196`).
//! 3. **The key never reaches the page text** — it is held in the editor's
//!    draft and in the password field's value only (the instrument shows
//!    `t: "•••"` and keeps the value in `val`, like an `<input>` value outside
//!    `textContent`); no label, copy id or binding ever carries it.
//! 4. **The failure is redacted, not echoed** — [`redact`] is the web's
//!    `redactModelSettingsError` (`model-settings.ts:523-543`); and the screen
//!    shows the board's own sentence, never the server's prose.
//! 5. **(A23) The key stays OUTSIDE the controller draft** — the transport
//!    builds [`model_settings::Draft`] (no key field) from the editor and hands
//!    the key to the operation as its argument (`model-settings.ts:55-64`,
//!    `ModelManagementSettings.tsx` `management.test(draft, apiKey)`); a `Work`
//!    never carries it.
//! 6. **(A23) Configured values survive an edit** — the row's inference
//!    overrides ride the draft unchanged (null included), and an EDIT-BLOCKED
//!    row is never edited (`model-management-projection.ts:186-199`).
use serde_json::{Map, Value};

use super::board1::{Layout, Ui};
use super::board1_kit::{self as kit, Field, Text};
use super::model_settings::{self as ms, copy, Credential, EditorDraft, Family, Provider, ProviderRoute};
use crate::i18n::{tr, tr1};

/// The web's own alert copy (`model-management.spec.ts:181-183`), shown when a
/// failure is not a key rejection.
pub const TEST_FAILED: &str = copy::CONNECTION_FAILED;

/// The web truncates a redacted message at this length (`model-settings.ts:542`).
const MAX_REDACTED: usize = 1_000;

/// The two editor cards (design/stage-b/phase4/cards).
pub const CARDS: &[(&str, &str)] = &[("provider", "p4-06"), ("provider_rejected", "p4-07")];

/// Which provider-editor card is showing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    /// `p4-06` — the plain editor.
    #[default]
    Editor,
    /// `p4-07` — the same editor after the provider rejected the draft.
    Rejected,
}

impl Screen {
    pub const ALL: [(Screen, &'static str); 2] = [(Screen::Editor, "p4-06"), (Screen::Rejected, "p4-07")];

    pub fn card_dir(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(s, _)| *s == self)
            .map(|(_, dir)| *dir)
            .expect("every screen has a card")
    }
}

/// Redact a provider failure the way the web's `redactModelSettingsError`
/// does (`model-settings.ts:523-543`): the secret itself — raw and URL-encoded
/// — then any `api_key:`/`api-key`/`apikey` run, then any `Bearer …` run, then
/// a 1000-char cut.
pub fn redact(reason: &str, secret: &str) -> String {
    let mut message = reason.to_owned();
    let trimmed = secret.trim();
    for candidate in [Some(secret), Some(trimmed)].into_iter().flatten().filter(|c| !c.is_empty()) {
        message = message.replace(candidate, "[redacted]");
    }
    if !trimmed.is_empty() {
        let encoded = percent_encode(trimmed);
        if encoded != trimmed {
            message = message.replace(&encoded, "[redacted]");
        }
    }
    let message = redact_keyed_runs(&message);
    let message = redact_bearer(&message);
    if message.chars().count() > MAX_REDACTED {
        message.chars().take(MAX_REDACTED).collect()
    } else {
        message
    }
}

/// The `api_key:`/`api-key=`/`apikey =` pass (`model-settings.ts:540`).
fn redact_keyed_runs(s: &str) -> String {
    const NAMES: [&str; 3] = ["apikey", "api_key", "api-key"];
    let bytes = s.as_bytes();
    let lower = s.to_ascii_lowercase();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    'outer: while i < s.len() {
        for name in NAMES {
            if lower[i..].starts_with(name) {
                let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
                if !before_ok {
                    continue;
                }
                let mut j = i + name.len();
                while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && (bytes[j] == b':' || bytes[j] == b'=') {
                    j += 1;
                    while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                        j += 1;
                    }
                    let start = j;
                    while j < bytes.len() {
                        let c = bytes[j] as char;
                        if c.is_whitespace() || c == ',' || c == ';' {
                            break;
                        }
                        j += 1;
                    }
                    if j > start {
                        out.push_str(&s[i..start]);
                        out.push_str("[redacted]");
                        i = j;
                        continue 'outer;
                    }
                }
            }
        }
        let ch = s[i..].chars().next().expect("i is a char boundary");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// The `Bearer <token>` pass (`model-settings.ts:541`). The 7-byte window is
/// taken with `get(..)`: the RECORDED 401 (r29a line 10) carries an em dash,
/// so a hard slice would land mid-character.
fn redact_bearer(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < s.len() {
        let tail = &s[i..];
        let is_bearer = tail.get(..7).is_some_and(|w| w.eq_ignore_ascii_case("bearer "));
        if !is_bearer {
            let ch = tail.chars().next().expect("i is a char boundary");
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        out.push_str(&s[i..i + 7]);
        let token = &s[i + 7..];
        let end = token
            .find(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .unwrap_or(token.len());
        out.push_str("[redacted]");
        i += 7 + end;
    }
    out
}

/// Minimal `encodeURIComponent` for the redaction pass (`:534`).
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The HTTP status a provider failure names ("… HTTP 401 - …" in the recorded
/// a6ea8505 reply), so the board's "(401)" is the server's number, not a guess.
pub fn http_status(reason: &str) -> Option<u16> {
    let at = reason.find("HTTP ")?;
    let digits: String = reason[at + 5..].chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok().filter(|n: &u16| (100..600).contains(n))
}

/// Whether a failure is the provider refusing the CREDENTIAL (401/403 or the
/// provider's own "authentication" wording) — p4-07's red key outline.
pub fn is_key_rejection(reason: &str) -> bool {
    matches!(http_status(reason), Some(401) | Some(403))
        || reason.to_ascii_lowercase().contains("authentication")
}

/// A family's display label and official base URL (the board's "DeepSeek ·
/// Official API" / "https://api.deepseek.com/v1"). A route WITHOUT its own
/// `base_url` uses the provider's official endpoint, so the field shows it and
/// the save omits `base_url` unless the operator changed it. A23: the label
/// is the web's `familyLabel` (`model-management-projection.ts:203-213`).
pub fn family_meta(family: &str) -> (String, &'static str) {
    let url = match family {
        "deepseek" => "https://api.deepseek.com/v1",
        "openai" => "https://api.openai.com/v1",
        "anthropic" => "https://api.anthropic.com",
        "moonshot" | "moonshot-coding" => "https://api.moonshot.ai/v1",
        "openrouter" => "https://openrouter.ai/api/v1",
        "zhipu" => "https://open.bigmodel.cn/api/paas/v4",
        "zai" | "zai-coding" => "https://api.z.ai/api/paas/v4",
        "gemini" => "https://generativelanguage.googleapis.com/v1beta",
        "groq" => "https://api.groq.com/openai/v1",
        "dashscope" => "https://dashscope.aliyuncs.com/compatible-mode/v1",
        "minimax" | "minimax-cn" => "https://api.minimax.io/v1",
        _ => "",
    };
    (ms::family_label(family), url)
}

/// Which editor operations the server advertises, each gated on its OWN
/// method and failing closed (`modelSettingsCapabilities`,
/// `model-settings.ts:156-170`; `supportsMethod`, client `interaction.ts:14`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Caps {
    /// `profile/llm/list` — the configured routes.
    pub read: bool,
    /// `profile/llm/catalog` — the families and their models.
    pub catalog: bool,
    /// `profile/llm/test`.
    pub test: bool,
    /// `profile/llm/upsert`.
    pub save: bool,
}

impl Caps {
    pub const ALL: Caps = Caps { read: true, catalog: true, test: true, save: true };

    /// From the open reply's `supported_methods` (the store's config domain).
    pub fn from_methods(methods: &[String]) -> Caps {
        let has = |m: &str| methods.iter().any(|x| x == m);
        Caps {
            read: has("profile/llm/list"),
            catalog: has("profile/llm/catalog"),
            test: has("profile/llm/test"),
            save: has("profile/llm/upsert"),
        }
    }

    /// Save is a test then an upsert, so it needs both (`save` begins only
    /// when `capabilities.test && capabilities.save`, `model-settings.ts:349`).
    pub fn can_save(&self) -> bool {
        self.test && self.save
    }
}

/// The web's notice when mutation is not advertised (`ModelManagementSection.tsx`).
pub const READ_ONLY: &str = copy::READ_ONLY;

/// A23 — which form the editor is (`OpenEditor.mode`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    /// An existing configured row: provider and route identity fixed.
    #[default]
    Edit,
    /// The catalog-driven "Add provider" (`openAdd`).
    Add,
}

/// A23 — where the editor was opened (where Back / Save return).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Origin {
    /// Settings > Model > Model providers (A2): back to Settings.
    #[default]
    Settings,
    /// The providers dialog (`board3::routes`): back to it, re-read.
    Routes,
}

/// A23 — the operation a button started (its busy label, its failure copy).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Op {
    Test,
    #[default]
    Save,
    Fetch,
}

/// A23 — the editor's selects (one open at a time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Select {
    Family,
    Protocol,
}

/// A23 — a probe's outcome line (`Feedback`, `ModelManagementSection.tsx:128`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feedback {
    pub ok: bool,
    pub text: String,
}

/// The editor's live draft (`ModelSettingsDraft` + `OpenEditor`): the values
/// are held so a failed test can keep them (row 88); the credential is a
/// separate field that no copy id, no binding and no controller draft ever
/// returns.
#[derive(Clone, Default, PartialEq)]
pub struct ProviderUi {
    /// The provider family id — `deepseek`.
    pub family: String,
    /// The family's display label — "DeepSeek".
    pub family_label: String,
    /// The default model id (the one `(default)` marks).
    pub model: String,
    /// The route id — `deepseek` (the official API) or a catalog endpoint id.
    pub route: String,
    /// The route's label — "Official API".
    pub route_label: String,
    /// The Name field's text ("DeepSeek · Official API").
    pub name: String,
    /// The Base URL field's text.
    pub base_url: String,
    /// The family's official endpoint (sent only when `base_url` differs).
    pub default_base_url: String,
    pub api_key_env: Option<String>,
    pub api_type: Option<String>,
    /// The typed key. Held so a rejected draft keeps it (row 88); shown only
    /// as the password field's masked value; handed to an operation as its
    /// argument, never stored in a controller draft.
    pub key: String,
    /// The profile already stores a key for this route (`has_api_key`): the
    /// field is blank and masked (walk 87: "editing shows a blank masked key").
    pub key_stored: bool,
    /// The eye toggle: the key field shows its value in the clear.
    pub key_revealed: bool,
    /// The model ids the route offers, top to bottom.
    pub models: Vec<String>,
    /// Which model row is the default.
    pub default_model: Option<String>,
    /// The redacted failure, if the last test failed. Never shown verbatim.
    pub error: Option<String>,
    /// The HTTP status the failure named (p4-07 "(401)").
    pub error_status: Option<u16>,
    /// The key field is outlined red (p4-07).
    pub key_rejected: bool,
    /// A save/test is in flight.
    pub busy: bool,
    /// The profile the editor writes (the session's own; `None` = server default).
    pub profile_id: Option<String>,
    /// The operator changed a field (a late load must not overwrite it).
    pub edited: bool,
    /// What the server lets this editor do (set when it opens).
    pub caps: Caps,
    /// Where we are.
    pub screen: Screen,
    // ---------------------------------------------------------------- A23
    /// Add or edit.
    pub mode: Mode,
    /// Where Back / Save return.
    pub origin: Origin,
    /// The edited row's id (`family:model:route`).
    pub provider_id: Option<String>,
    /// The edited row is the Profile's primary (Save keeps it the primary).
    pub primary: bool,
    /// The configuration was EMPTY when Add opened: the first model becomes
    /// the primary (`ModelManagementSettings.saveProvider`).
    pub config_empty: bool,
    /// The configured inference overrides, carried through an edit unchanged.
    pub inference: Option<Map<String, Value>>,
    /// The configured row carries settings the editor cannot preserve.
    pub edit_blocked: bool,
    /// The credential environment the stored key belongs to
    /// (`configuredApiKeyEnv`): the stored key counts only while unchanged.
    pub configured_key_env: Option<String>,
    /// `profile/llm/fetch_models` is advertised.
    pub can_fetch: bool,
    /// The catalog's families (the Add form's Provider select).
    pub families: Vec<Family>,
    /// The API protocols (`(id, label)`, "openai" first).
    pub protocols: Vec<(String, String)>,
    /// The configured rows (Add's "already exists" check).
    pub configured: Vec<Provider>,
    /// "Available from endpoint" (the last fetch).
    pub fetched: Vec<String>,
    /// The last probe's line under the key (test success).
    pub feedback: Option<Feedback>,
    /// The last fetch's line under the models.
    pub fetch_feedback: Option<Feedback>,
    /// The validation the last refused click found (`validateModelProviderDraft`).
    pub issues: Vec<ms::Issue>,
    /// The open select.
    pub open_select: Option<Select>,
    /// The last operation (its retry, its failure copy).
    pub last_op: Op,
    /// The last failure was the SAVE's upsert, not the provider test.
    pub save_refused: bool,
}

impl std::fmt::Debug for ProviderUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderUi")
            .field("mode", &self.mode)
            .field("family", &self.family)
            .field("route", &self.route)
            .field("model", &self.model)
            .field("models", &self.models)
            .field("key", &format_args!("<{} chars>", self.key.chars().count()))
            .field("screen", &self.screen)
            .finish()
    }
}

/// The credential reference a family's key is stored under before the
/// catalog is read (the catalog's `env` replaces it).
fn guess_env(family: &str) -> String {
    format!("{}_API_KEY", family.to_ascii_uppercase().replace('-', "_"))
}

impl ProviderUi {
    /// A new "Add provider" draft with the defaults the spec pins
    /// (`model-management.spec.ts:184-187`: model `deepseek-chat`, route
    /// `openrouter`) and the board's three rows.
    pub fn new(family: &str) -> Self {
        let (family_label, default_url) = family_meta(family);
        Self {
            family: family.to_owned(),
            family_label: family_label.clone(),
            model: "deepseek-chat".to_owned(),
            route: "openrouter".to_owned(),
            route_label: "Official API".to_owned(),
            name: format!("{family_label} \u{b7} Official API"),
            base_url: default_url.to_owned(),
            default_base_url: default_url.to_owned(),
            api_key_env: Some(guess_env(family)),
            api_type: Some("openai".to_owned()),
            models: vec![
                "deepseek-v4-flash".to_owned(),
                "deepseek-v4".to_owned(),
                "deepseek-chat".to_owned(),
            ],
            default_model: Some("deepseek-v4-flash".to_owned()),
            caps: Caps::ALL,
            screen: Screen::Editor,
            primary: true,
            protocols: vec![("openai".into(), ms::protocol_label("openai"))],
            ..Default::default()
        }
    }

    /// The key as the UI may describe it: dots, or nothing.
    pub fn key_display(&self) -> &'static str {
        if self.key.is_empty() && !self.key_stored {
            ""
        } else {
            "••••••••••••••••••"
        }
    }

    /// A failure came back: redact it against the live key, keep the draft,
    /// and move to p4-07. The single place a refusal enters the editor.
    pub fn reject(&mut self, raw_reason: &str) {
        let safe = redact(raw_reason, &self.key);
        self.error_status = http_status(&safe);
        self.key_rejected = is_key_rejection(&safe);
        self.error = Some(safe);
        self.busy = false;
        self.feedback = None;
        self.save_refused = false;
        self.screen = Screen::Rejected;
    }

    /// A23 — "Test connection" failed: redacted against the live key like
    /// [`ProviderUi::reject`], the draft kept, the editor stays (Save is not
    /// "Try again"); the line reads where Test was clicked, the key field
    /// outlined red when the provider refused the key.
    pub fn test_failed(&mut self, raw_reason: &str) {
        let safe = redact(raw_reason, &self.key);
        self.error_status = http_status(&safe);
        self.key_rejected = is_key_rejection(&safe);
        self.error = Some(safe);
        self.save_refused = false;
        self.busy = false;
        self.screen = Screen::Editor;
        self.feedback = Some(Feedback { ok: false, text: format!("{} {}", self.failure_line(), tr("Your draft is kept.")) });
    }

    /// The test passed: drop the error, leave the draft alone.
    pub fn accept(&mut self) {
        self.error = None;
        self.error_status = None;
        self.key_rejected = false;
        self.save_refused = false;
        self.busy = false;
        self.screen = Screen::Editor;
    }

    /// The board's p4-07 sentence for the current failure (never the raw text).
    pub fn failure_line(&self) -> String {
        if self.key_rejected {
            match self.error_status {
                Some(n) => tr1("The provider rejected this key ({value0}).", &n.to_string()),
                None => tr("The provider rejected this key.").to_owned(),
            }
        } else if self.save_refused {
            tr(copy::SAVE_FAILED).to_owned()
        } else {
            tr(TEST_FAILED).to_owned()
        }
    }

    /// The route label the Name field now names ("DeepSeek · Official API" →
    /// "Official API"; a bare edit is the label itself).
    pub fn label_from_name(&self) -> String {
        let n = self.name.trim();
        match n.split_once('\u{b7}') {
            Some((_, label)) if !label.trim().is_empty() => label.trim().to_owned(),
            _ if n.is_empty() => self.route_label.clone(),
            _ => n.to_owned(),
        }
    }

    /// The model this draft saves (the Models list's choice, else the
    /// configured one).
    pub fn model_id(&self) -> String {
        self.default_model.clone().unwrap_or_else(|| self.model.clone())
    }

    /// Whether Save makes this row the Profile's primary
    /// (`ModelManagementSettings.saveProvider`: the edited row's own primary
    /// flag, or the first model of an empty configuration).
    pub fn set_primary(&self) -> bool {
        match self.mode {
            Mode::Edit => self.primary,
            Mode::Add => self.config_empty,
        }
    }

    /// The editor's draft (`ModelProviderDraft`): the base URL is sent only
    /// when it differs from the family's official endpoint.
    pub fn editor_draft(&self) -> EditorDraft {
        let base = self.base_url.trim();
        let base_url = if base.is_empty() || base == self.default_base_url { String::new() } else { base.to_owned() };
        EditorDraft {
            family_id: self.family.trim().to_owned(),
            model_id: self.model_id().trim().to_owned(),
            route: ProviderRoute {
                id: self.route.trim().to_owned(),
                label: self.label_from_name(),
                base_url,
                api_protocol: self.api_type.clone().unwrap_or_default(),
                api_key_env: self.api_key_env.clone().unwrap_or_default(),
            },
            api_key: self.key.clone(),
            inference: self.inference.clone(),
            edit_blocked: self.edit_blocked,
        }
    }

    /// The CONTROLLER draft (`modelSettingsDraftFromProvider`): no key.
    pub fn draft(&self) -> ms::Draft {
        ms::Draft::from_editor(&self.editor_draft(), self.set_primary())
    }

    /// The family the draft names, when the catalog lists it.
    pub fn family_option(&self) -> Option<&Family> {
        self.families.iter().find(|f| f.id == self.family.trim())
    }

    /// `draftCredentialConfigured`: the stored key counts only while the
    /// credential environment it belongs to is unchanged.
    pub fn credential_configured(&self) -> bool {
        self.key_stored && self.configured_key_env.clone().unwrap_or_default() == self.api_key_env.clone().unwrap_or_default()
    }

    /// The family's credential rule (`family?.credentialRequirement ?? "optional"`).
    pub fn credential(&self) -> Credential {
        self.family_option().map(|f| f.credential).unwrap_or(Credential::Optional)
    }

    /// `validateModelProviderDraft` with this editor's rules: a probe (Fetch)
    /// ignores the model id and the label; Add checks the identity is new.
    pub fn validate(&self, op: Op) -> Vec<ms::Issue> {
        let family = self.family_option();
        let rules = ms::Rules {
            credential_configured: self.credential_configured(),
            credential: self.credential(),
            requires_base_url: family.map(|f| f.requires_base_url).unwrap_or(self.default_base_url.is_empty()),
            identity_available: (self.mode == Mode::Add && op != Op::Fetch)
                .then(|| !ms::identity_exists(&self.configured, None, &self.editor_draft())),
        };
        let issues = ms::validate(&self.editor_draft(), rules);
        if op == Op::Fetch {
            ms::probe_issues(&issues)
        } else {
            issues
        }
    }

    /// The editor is read-only: mutation not advertised, or the configured
    /// row cannot be edited safely (fail closed).
    pub fn read_only(&self) -> bool {
        !self.caps.can_save() || self.edit_blocked
    }

    /// A23 — `chooseFamily` (`ModelManagementSection.tsx:530-547`): the
    /// family's first catalog model and its route; a key typed for another
    /// provider never follows the identity switch.
    pub fn choose_family(&mut self, family: &Family) {
        let suggestion = family.models.first();
        let route = suggestion.and_then(|s| s.route.clone()).or_else(|| family.default_route.clone());
        let (label, url) = family_meta(&family.id);
        self.family = family.id.clone();
        self.family_label = label;
        self.default_base_url = url.to_owned();
        self.models = family.models.iter().map(|m| m.id.clone()).collect();
        self.default_model = suggestion.map(|s| s.id.clone());
        self.model = self.default_model.clone().unwrap_or_default();
        self.key.clear();
        self.fetched.clear();
        self.fetch_feedback = None;
        self.feedback = None;
        match route {
            Some(r) => self.apply_route(&r),
            None => {
                let label = family.label.clone();
                self.apply_route(&ProviderRoute {
                    id: family.id.clone(),
                    label,
                    base_url: String::new(),
                    api_protocol: self.api_type.clone().unwrap_or_else(|| "openai".into()),
                    api_key_env: family.env.clone(),
                });
            }
        }
    }

    /// A route's fields into the form (an empty base URL shows the family's
    /// official endpoint, which is then not sent back).
    fn apply_route(&mut self, r: &ProviderRoute) {
        self.route = r.id.clone();
        self.route_label = r.label.clone();
        self.name = format!("{} \u{b7} {}", self.family_label, r.label);
        self.base_url = if r.base_url.is_empty() { self.default_base_url.clone() } else { r.base_url.clone() };
        self.api_type = Some(r.api_protocol.clone()).filter(|p| !p.is_empty());
        self.api_key_env = Some(r.api_key_env.clone());
    }
}

/// A23 — what the providers dialog hands the editor when it opens it.
#[derive(Debug, Clone, Default)]
pub struct Seed {
    pub profile_id: Option<String>,
    pub caps: Caps,
    pub can_fetch: bool,
    pub families: Vec<Family>,
    pub protocols: Vec<(String, String)>,
    pub configured: Vec<Provider>,
    pub origin: Origin,
}

/// A23 — the editor for ONE configured row (`openEdit` +
/// `configuredProviderDraft`): its identity, its route, its configured
/// inference values, a blank write-only key.
pub fn for_row(p: &Provider, seed: &Seed) -> ProviderUi {
    let mut ui = ProviderUi {
        caps: seed.caps,
        can_fetch: seed.can_fetch,
        profile_id: seed.profile_id.clone(),
        families: seed.families.clone(),
        protocols: seed.protocols.clone(),
        configured: seed.configured.clone(),
        origin: seed.origin,
        mode: Mode::Edit,
        ..Default::default()
    };
    let d = ms::configured_draft(p);
    let (label, url) = family_meta(&p.family_id);
    ui.family = p.family_id.clone();
    ui.family_label = if p.family_label.is_empty() { label } else { p.family_label.clone() };
    ui.default_base_url = url.to_owned();
    ui.model = p.model_id.clone();
    ui.default_model = Some(p.model_id.clone());
    ui.apply_route(&d.route);
    ui.key_stored = p.api_key_configured;
    ui.configured_key_env = Some(p.route.api_key_env.clone());
    ui.primary = p.primary;
    ui.provider_id = Some(p.id.clone());
    ui.inference = d.inference;
    ui.edit_blocked = d.edit_blocked;
    let mut models: Vec<String> = ui.family_option().map(|f| f.models.iter().map(|m| m.id.clone()).collect()).unwrap_or_default();
    if !models.contains(&p.model_id) {
        models.insert(0, p.model_id.clone());
    }
    models.sort_by_key(|m| m != &p.model_id);
    ui.models = models;
    ui
}

/// A23 — the catalog-driven "Add provider" (`openAdd` +
/// `createModelProviderDraft`): the first catalog model not configured yet.
pub fn for_add(seed: &Seed, config_empty: bool) -> ProviderUi {
    let mut ui = ProviderUi {
        caps: seed.caps,
        can_fetch: seed.can_fetch,
        profile_id: seed.profile_id.clone(),
        families: seed.families.clone(),
        protocols: seed.protocols.clone(),
        configured: seed.configured.clone(),
        origin: seed.origin,
        mode: Mode::Add,
        config_empty,
        ..Default::default()
    };
    let d = ms::create_draft(&seed.families, &seed.protocols, &seed.configured);
    let (label, url) = family_meta(&d.family_id);
    ui.family = d.family_id.clone();
    ui.family_label = label;
    ui.default_base_url = url.to_owned();
    ui.models = ui.family_option().map(|f| f.models.iter().map(|m| m.id.clone()).collect()).unwrap_or_default();
    ui.model = d.model_id.clone();
    ui.default_model = Some(d.model_id.clone()).filter(|m| !m.is_empty());
    ui.apply_route(&d.route);
    if d.route.api_protocol.is_empty() {
        ui.api_type = seed.protocols.first().map(|p| p.0.clone());
    }
    ui
}

/// The action ids the two cards emit, with what each one means.
pub const ACTIONS: &[(&str, &str)] = &[
    ("provider.name", "the Name field's live text (family · route label)"),
    ("provider.url", "the Base URL field's live text"),
    ("provider.key", "the API key field's live text — held in the draft, never shown"),
    ("provider.key.reveal", "show/hide the key field's value (the eye)"),
    ("provider.model.0", "make the first listed model the default"),
    ("provider.model.1", "make the second listed model the default"),
    ("provider.model.2", "make the third listed model the default"),
    ("provider.model.3", "make the fourth listed model the default"),
    ("prov.test", "test the connection with the draft (profile/llm/test only)"),
    ("provider.save", "Save: profile/llm/test, then profile/llm/upsert with set_primary"),
    ("provider.retry", "Try again (p4-07): the operation that failed, with the draft kept"),
    ("provider.cancel", "discard the editor"),
    ("provider.back", "the back chevron: discard the editor"),
    // A23 — the web editor's other controls.
    ("provider.fetch", "Fetch available models (profile/llm/fetch_models, the unsaved endpoint)"),
    ("provider.model_id", "Add: the Model ID field's live text"),
    ("provider.route_id", "Add: the Route ID field's live text"),
    ("provider.env", "the Credential environment field's live text"),
    ("provider.family", "Add without a catalog: the Provider field's live text"),
    ("provider.select.family", "Add: open/close the Provider select (the catalog's families)"),
    ("provider.select.protocol", "open/close the API protocol select"),
    ("provider.guide", "GLM-5.3-Flash: open the official guide"),
];

/// The id families whose row rides in the id (`provider.model.<n>` past the
/// four the card declares, the open select's `provider.option.<n>`, the
/// fetched models' `provider.fetched.<n>`).
const INDEXED: [&str; 3] = ["provider.model.", "provider.option.", "provider.fetched."];

fn indexed(id: &str) -> Option<(&'static str, usize)> {
    INDEXED.iter().find_map(|p| id.strip_prefix(p).and_then(|n| n.parse().ok()).map(|n| (*p, n)))
}

/// The ids [`resolve`] routes.
pub const ROUTED: &[&str] = &[
    "provider.name",
    "provider.url",
    "provider.key",
    "provider.key.reveal",
    "provider.model.0",
    "provider.model.1",
    "provider.model.2",
    "provider.model.3",
    "prov.test",
    "provider.save",
    "provider.retry",
    "provider.cancel",
    "provider.back",
    "provider.fetch",
    "provider.model_id",
    "provider.route_id",
    "provider.env",
    "provider.family",
    "provider.select.family",
    "provider.select.protocol",
    "provider.guide",
];

pub fn is_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id) || indexed(id).is_some()
}

pub fn is_routed(id: &str) -> bool {
    ROUTED.contains(&id) || indexed(id).is_some()
}

pub fn unrouted() -> Vec<&'static str> {
    ACTIONS.iter().map(|(a, _)| *a).filter(|a| !ROUTED.contains(a)).collect()
}

/// The live text inputs (typing never rebuilds the view).
pub fn is_input(id: &str) -> bool {
    matches!(
        id,
        "provider.name" | "provider.url" | "provider.key" | "provider.model_id" | "provider.route_id" | "provider.env" | "provider.family"
    )
}

/// What an action means.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Input { field: &'static str, value: String },
    /// Make the model row at this index the default.
    SelectModel(usize),
    /// A23 — choose the fetched ("Available from endpoint") model at this index.
    SelectFetched(usize),
    /// A23 — open / close one select.
    ToggleSelect(Select),
    /// A23 — pick the open select's option at this index.
    PickOption(usize),
    ToggleKey,
    /// `profile/llm/test` alone.
    Test,
    /// `profile/llm/test` then `profile/llm/upsert`.
    Save,
    /// A23 — `profile/llm/fetch_models`.
    Fetch,
    /// A23 — the operation that failed, again.
    Retry,
    /// A23 — open a URL in the platform browser.
    OpenUrl(String),
    /// Leave the editor (no transport).
    Close,
    /// A redacted failure the caller just received.
    Failed { reason: String },
    Unhandled,
}

/// Route one action id to its effect. `value` carries an input payload.
pub fn resolve(id: &str, value: Option<&str>) -> Effect {
    let v = || value.unwrap_or_default().to_owned();
    if let Some((family, n)) = indexed(id) {
        return match family {
            "provider.model." => Effect::SelectModel(n),
            "provider.option." => Effect::PickOption(n),
            _ => Effect::SelectFetched(n),
        };
    }
    match id {
        "provider.name" => Effect::Input { field: "provider.name", value: v() },
        "provider.url" => Effect::Input { field: "provider.url", value: v() },
        "provider.key" => Effect::Input { field: "provider.key", value: v() },
        "provider.model_id" => Effect::Input { field: "provider.model_id", value: v() },
        "provider.route_id" => Effect::Input { field: "provider.route_id", value: v() },
        "provider.env" => Effect::Input { field: "provider.env", value: v() },
        "provider.family" => Effect::Input { field: "provider.family", value: v() },
        "provider.key.reveal" => Effect::ToggleKey,
        "provider.select.family" => Effect::ToggleSelect(Select::Family),
        "provider.select.protocol" => Effect::ToggleSelect(Select::Protocol),
        "prov.test" => Effect::Test,
        "provider.save" => Effect::Save,
        "provider.retry" => Effect::Retry,
        "provider.fetch" => Effect::Fetch,
        "provider.guide" => Effect::OpenUrl(copy::GLM_URL.to_owned()),
        "provider.cancel" | "provider.back" => Effect::Close,
        _ => Effect::Unhandled,
    }
}

/// The options the open select lists: `(id, label)`.
pub fn options(ui: &ProviderUi, which: Select) -> Vec<(String, String)> {
    match which {
        Select::Family => ui.families.iter().map(|f| (f.id.clone(), f.label.clone())).collect(),
        Select::Protocol => {
            let mut p = ui.protocols.clone();
            // The configured protocol is always offered (`protocolOptions`).
            if let Some(t) = ui.api_type.clone().filter(|t| !t.is_empty()) {
                if !p.iter().any(|(id, _)| *id == t) {
                    p.push((t.clone(), t));
                }
            }
            p
        }
    }
}

/// Apply one effect to the draft. Returns the transport effect to perform.
pub fn apply(ui: &mut ProviderUi, effect: Effect) -> Option<Effect> {
    match effect {
        Effect::Input { field, value } => {
            match field {
                "provider.name" => ui.name = value,
                "provider.url" => ui.base_url = value,
                "provider.key" => ui.key = value,
                "provider.env" => ui.api_key_env = Some(value),
                // The identity fields exist in the Add form only.
                "provider.model_id" if ui.mode == Mode::Add => {
                    ui.default_model = Some(value.clone());
                    ui.model = value;
                }
                "provider.route_id" if ui.mode == Mode::Add => ui.route = value,
                "provider.family" if ui.mode == Mode::Add => ui.family = value,
                _ => {}
            }
            ui.edited = true;
            None
        }
        Effect::SelectModel(i) => {
            if ui.read_only() || ui.busy {
                return None;
            }
            if let Some(m) = ui.models.get(i).cloned() {
                // `chooseSuggestion`: a catalog model may select a more
                // specific endpoint route (the Add form; an edit keeps its
                // fixed route identity).
                if ui.mode == Mode::Add {
                    let route = ui
                        .family_option()
                        .and_then(|f| f.models.iter().find(|s| s.id == m))
                        .and_then(|s| s.route.clone());
                    if let Some(r) = route {
                        if Some(&r.api_key_env) != ui.api_key_env.as_ref() {
                            ui.key.clear();
                        }
                        ui.apply_route(&r);
                    }
                }
                ui.model = m.clone();
                ui.default_model = Some(m);
                ui.edited = true;
                ui.issues.clear();
            }
            None
        }
        Effect::SelectFetched(i) => {
            if ui.read_only() || ui.busy {
                return None;
            }
            if let Some(m) = ui.fetched.get(i).cloned() {
                ui.model = m.clone();
                ui.default_model = Some(m);
                ui.edited = true;
                ui.issues.clear();
            }
            None
        }
        Effect::ToggleSelect(which) => {
            let allowed = !ui.read_only() && !ui.busy && (which == Select::Protocol || ui.mode == Mode::Add);
            if allowed {
                ui.open_select = if ui.open_select == Some(which) { None } else { Some(which) };
            }
            None
        }
        Effect::PickOption(i) => {
            let Some(which) = ui.open_select else { return None };
            if ui.busy {
                return None;
            }
            let Some((id, _)) = options(ui, which).get(i).cloned() else { return None };
            match which {
                Select::Family => {
                    if let Some(f) = ui.families.iter().find(|f| f.id == id).cloned() {
                        ui.choose_family(&f);
                    }
                }
                Select::Protocol => ui.api_type = Some(id),
            }
            ui.open_select = None;
            ui.edited = true;
            ui.issues.clear();
            None
        }
        Effect::ToggleKey => {
            ui.key_revealed = !ui.key_revealed;
            None
        }
        Effect::Failed { reason } => {
            ui.reject(&reason);
            None
        }
        Effect::Retry => {
            let again = match ui.last_op {
                Op::Test => Effect::Test,
                Op::Save => Effect::Save,
                Op::Fetch => Effect::Fetch,
            };
            apply(ui, again)
        }
        Effect::Test | Effect::Save | Effect::Fetch => {
            if ui.busy {
                return None; // one request at a time (latest-request-wins)
            }
            // Each operation on its own advertised method, failing closed;
            // an edit-blocked row is never written.
            let allowed = !ui.edit_blocked
                && match effect {
                    Effect::Test => ui.caps.test && ui.caps.save,
                    Effect::Save => ui.caps.can_save(),
                    _ => ui.can_fetch && ui.caps.save,
                };
            if !allowed {
                return None;
            }
            let op = match effect {
                Effect::Test => Op::Test,
                Effect::Save => Op::Save,
                _ => Op::Fetch,
            };
            // `validateModelProviderDraft`: a draft with issues reaches no wire.
            let issues = ui.validate(op);
            if !issues.is_empty() {
                ui.issues = issues;
                return None;
            }
            ui.issues.clear();
            ui.open_select = None;
            ui.last_op = op;
            ui.busy = true;
            Some(effect)
        }
        Effect::OpenUrl(u) => Some(Effect::OpenUrl(u)),
        Effect::Close => Some(Effect::Close),
        Effect::Unhandled => None,
    }
}

// --------------------------------------------------------------------- views

/// A monospace run (the board's SF-Mono-like face for ids).
fn mono(id: &str, s: &str, px: f64, color: &str) -> String {
    let name = if id.is_empty() { String::new() } else { format!("{id} := ") };
    format!(
        "{name}Label {{\nwidth: Fit height: Fit padding: 0 margin: 0 flow: Right max_lines: 1 text_overflow: TextOverflow.Ellipsis\nalign: Align{{x: 0.0 y: 0.5}}\ntext: {}\ndraw_text.color: {color}\ndraw_text.text_style: {}\n}}\n",
        kit::lit(s),
        super::board3::ui::text_style(super::board3::ui::Face::Mono, px)
    )
}

/// One field's validation message (board 1 #7's red line), when the last
/// refused click found one.
fn issue_line(v: &mut Ui, ui: &ProviderUi, field: ms::Field, id: &str) {
    if let Some(i) = ui.issues.iter().find(|i| i.field == field) {
        v.push(kit::gap(6.0));
        v.push(Text::new(id, tr(i.message)).px(14.0).color(kit::RED).fill().dsl());
    }
}

/// A grey footnote under a control (board 1's captions).
fn hint(v: &mut Ui, id: &str, s: &str) {
    v.push(kit::gap(6.0));
    v.push(Text::new(id, s).px(13.0).color(kit::MUTED).fill().dsl());
}

/// A select: the board's field showing the choice, a chevron, and — open —
/// the option rows with the chosen one checked (board 3 #4's select, board
/// 1 #6's check rows).
fn select_field(v: &mut Ui, l: &Layout, ui: &ProviderUi, which: Select, label: &str, value: &str, detail: &str) {
    let id = match which {
        Select::Family => "b1_prov_family",
        Select::Protocol => "b1_prov_protocol",
    };
    let action = match which {
        Select::Family => "provider.select.family",
        Select::Protocol => "provider.select.protocol",
    };
    let open = ui.open_select == Some(which);
    let enabled = !ui.read_only() && !ui.busy;
    v.push(Text::new("", tr(label)).px(15.0).fill().one_line().dsl());
    v.push(kit::gap(8.0));
    let edge = if open { kit::BLUE } else { kit::FIELD_EDGE };
    let ink = if enabled { kit::INK } else { kit::MUTED };
    let hit = if enabled { kit::hit(id, true) } else { String::new() };
    v.push(format!(
        "View {{ width: Fill height: 44 flow: Overlay\nDesignSurface {{\nwidth: Fill height: Fill flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 14 right: 12}} spacing: 8\ndraw_bg.color: {} draw_bg.radius: 10 draw_bg.border_width: 1 draw_bg.border_position: 1 draw_bg.border_color: {edge}\n{}{}{}}}\n{hit}}}\n",
        kit::WHITE,
        Text::new(&format!("{id}_value"), tr(value)).px(15.0).color(ink).fill().one_line().dsl(),
        mono("", detail, 12.0, kit::MUTED),
        kit::svg("", "b3_chevron_down_dark.svg", 13.0),
    ));
    if enabled {
        v.button(id, action);
    }
    if open {
        let opts = options(ui, which);
        let chosen = match which {
            Select::Family => ui.family.clone(),
            Select::Protocol => ui.api_type.clone().unwrap_or_default(),
        };
        let row_h = if l.phone { 46 } else { 40 };
        let rows: Vec<String> = opts
            .iter()
            .enumerate()
            .map(|(i, (oid, olabel))| {
                let rid = format!("b1_prov_opt_{i}");
                v.button(&rid, &format!("provider.option.{i}"));
                let mark = if *oid == chosen { kit::svg("", "b1_check_on.svg", 20.0) } else { "View { width: 20 height: 20 }\n".to_owned() };
                format!(
                    "View {{ width: Fill height: {row_h} flow: Overlay\nView {{ width: Fill height: Fill flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 14 right: 12}} spacing: 10\n{}{}{mark}}}\n{}}}\n",
                    Text::new(&format!("b1_prov_opt_t{i}"), tr(olabel)).px(14.0).fill().one_line().dsl(),
                    mono("", oid, 12.0, kit::MUTED),
                    kit::hit(&rid, true)
                )
            })
            .collect();
        // Five full rows and their four hairlines.
        let max_h = (rows.len() > 5).then_some((row_h * 5 + 4) as f64);
        v.push(kit::gap(6.0));
        v.push(kit::list_card_scroll("b1_prov_options", &rows, max_h));
    }
}

/// One Models row (board 1 #6): the chosen model checked, "(default)" when
/// it is (or becomes) the Profile's primary.
fn model_rows(v: &mut Ui, l: &Layout, ui: &ProviderUi, list: &[String], action: &str, id_base: &str, interactive: bool) -> Vec<(String, f64)> {
    let chosen = ui.model_id();
    list.iter()
        .enumerate()
        .map(|(i, m)| {
            let on = *m == chosen;
            let label = if on && ui.set_primary() { tr1("{value0} (default)", m) } else { m.clone() };
            let id = format!("{id_base}_{i}");
            let hit = if interactive {
                v.button(&id, &format!("{action}.{i}"));
                kit::hit(&id, true)
            } else {
                String::new()
            };
            // A model id that does not fit beside the check (a long catalog
            // id + "(default)" on a phone) wraps onto a second line in a
            // taller row instead of ending in an ellipsis. The label gets the
            // control width less the borders, insets, gap and the 23 px check;
            // the kit's estimate runs ~15% short of the board-1 label's
            // measured run (claude-3-5-haiku-20241022 (default): 237
            // estimated, ~282 drawn), hence the 1.2.
            let room = l.content_w - 2.0 - 14.0 - 12.0 - 10.0 - 23.0;
            let two = super::board3::ui::text_w(&label, 14.0, super::board3::ui::Face::Regular) * 1.2 > room;
            let text_id = format!("{id_base}_t{i}");
            let text = Text::new(&text_id, &label).px(14.0).fill();
            let h = match (l.phone, two) {
                (true, true) => 68.0,
                (true, false) => 46.0,
                (false, true) => 62.0,
                (false, false) => 40.0,
            };
            let row = format!(
                "View {{ width: Fill height: {h} flow: Overlay\nView {{ width: Fill height: Fill flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 14 right: 12}} spacing: 10\n{}{}}}\n{hit}}}\n",
                if two { text.dsl() } else { text.one_line().dsl() },
                kit::svg("", if on { "b1_check_on.svg" } else { "b1_check_off.svg" }, 23.0),
            );
            (row, h)
        })
        .collect()
}

/// A list of more than five rows scrolls inside a box five FULL rows tall
/// (their own heights, a wrapped row included, and the four hairlines).
fn five_rows_h(rows: &[(String, f64)]) -> Option<f64> {
    (rows.len() > 5).then(|| rows.iter().take(5).map(|(_, h)| h).sum::<f64>() + 4.0)
}

/// The Models list (board 1 #6), the endpoint's models after a fetch
/// ("Available from endpoint" with its count) and "Fetch available models".
fn models_section(v: &mut Ui, l: &Layout, ui: &ProviderUi, read_only: bool) {
    v.push(Text::new("", tr("Models")).px(15.0).fill().one_line().dsl());
    v.push(kit::gap(if l.phone { 8.0 } else { 6.0 }));
    let models: Vec<String> = ui.models.clone();
    let rows = model_rows(v, l, ui, &models, "provider.model", "b1_prov_model", !read_only);
    let max_h = five_rows_h(&rows);
    let rows: Vec<String> = rows.into_iter().map(|(r, _)| r).collect();
    if rows.is_empty() {
        v.push(Text::new("b1_prov_models_none", tr("No catalog models for this provider.")).px(14.0).color(kit::MUTED).fill().dsl());
    } else {
        v.push(kit::list_card_scroll("b1_prov_models", &rows, max_h));
    }
    if !ui.fetched.is_empty() {
        v.push(kit::gap(12.0));
        v.push(format!(
            "View {{ width: Fill height: Fit flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 8\n{}{}}}\n",
            Text::new("b1_prov_fetched_label", tr(copy::FROM_ENDPOINT)).px(15.0).one_line().dsl(),
            Text::new("b1_prov_fetched_count", &ui.fetched.len().to_string()).px(13.0).color(kit::MUTED).one_line().dsl()
        ));
        v.push(kit::gap(6.0));
        let fetched = ui.fetched.clone();
        let rows = model_rows(v, l, ui, &fetched, "provider.fetched", "b1_prov_fetched", !read_only);
        let max_h = five_rows_h(&rows);
        let rows: Vec<String> = rows.into_iter().map(|(r, _)| r).collect();
        v.push(kit::list_card_scroll("b1_prov_fetched_list", &rows, max_h));
    }
    if let Some(f) = &ui.fetch_feedback {
        v.push(kit::gap(8.0));
        v.push(kit::status_line("b1_prov_fetch_feedback", tr(&f.text), f.ok));
    }
    if !read_only && ui.can_fetch {
        v.push(kit::gap(10.0));
        let label = tr(if ui.busy && ui.last_op == Op::Fetch { copy::FETCHING } else { copy::FETCH });
        v.push(kit::pill_outline_fit("b1_prov_fetch", label, 40.0));
        v.button("b1_prov_fetch", "provider.fetch");
    }
}

/// The scroll budget of the editor's body: the window less the dialog's
/// chrome (header + footer + card padding), so the footer stays on screen.
fn body_max(l: &Layout) -> f64 {
    if l.phone {
        (l.h - 214.0).max(240.0).floor()
    } else {
        (l.h - 200.0).max(220.0).floor()
    }
}

/// The native view of the editor (p4-06, or p4-07 after a rejection).
pub fn view(ui: &ProviderUi, l: &Layout) -> Ui {
    let mut v = Ui::default();
    let title = tr(if ui.mode == Mode::Add { copy::ADD_PROVIDER } else { "Edit provider" });
    v.header(l, "b1_prov_back", "provider.back", title);
    // Mutation not advertised, or a row the editor cannot preserve: the same
    // editor, read-only, with the reason and a Close instead of Cancel/Save
    // (fail closed).
    let read_only = ui.read_only();
    let field_gap = if l.phone { 14.0 } else { 10.0 };
    v.push(kit::gap(if l.phone { 16.0 } else { 12.0 }));
    // The scroll bar's gutter (board 3 measured the bar drawing over a row's
    // right edge without one) sits inside the card's own side padding: the
    // view reaches 10 px into it and pads 10 px back, so the fields keep the
    // board's margins on both sides.
    v.push(format!(
        "b1_prov_scroll := ScrollYView {{ width: Fill height: Fit max_height: {} flow: Down margin: Inset{{right: -10}} padding: Inset{{right: 10}}\n",
        body_max(l)
    ));
    if ui.mode == Mode::Add {
        // The catalog-driven identity (`openAdd`): the family from the
        // catalog, the model id (typed or picked below), the route id.
        if ui.families.is_empty() {
            v.push(Field::new("b1_prov_family_text", &ui.family).label(tr("Provider / family ID")).placeholder("deepseek").dsl());
            v.input("b1_prov_family_text", "provider.family");
        } else {
            let label = ui.family_option().map(|f| f.label.clone()).unwrap_or_else(|| ui.family.clone());
            select_field(&mut v, l, ui, Select::Family, "Provider", &label, &ui.family);
        }
        issue_line(&mut v, ui, ms::Field::FamilyId, "b1_prov_family_issue");
        v.push(kit::gap(field_gap));
        v.push(Field::new("b1_prov_model_id", &ui.model_id()).label(tr("Model ID")).placeholder("deepseek-chat").dsl());
        v.input("b1_prov_model_id", "provider.model_id");
        issue_line(&mut v, ui, ms::Field::ModelId, "b1_prov_model_issue");
        // The catalog's (and the endpoint's) models, right under the id
        // they fill (`SuggestionGroup`).
        v.push(kit::gap(field_gap));
        models_section(&mut v, l, ui, read_only);
        v.push(kit::gap(if l.phone { 18.0 } else { 14.0 }));
    }
    v.push(Field::new("b1_prov_name", &ui.name).label(tr("Name")).placeholder(tr("Provider · Route")).read_only(read_only).dsl());
    v.input("b1_prov_name", "provider.name");
    issue_line(&mut v, ui, ms::Field::RouteLabel, "b1_prov_name_issue");
    v.push(kit::gap(field_gap));
    let url_placeholder = if ui.default_base_url.is_empty() { tr("Provider default") } else { ui.default_base_url.as_str() };
    v.push(Field::new("b1_prov_url", &ui.base_url).label(tr("Base URL")).placeholder(url_placeholder).read_only(read_only).dsl());
    v.input("b1_prov_url", "provider.url");
    issue_line(&mut v, ui, ms::Field::BaseUrl, "b1_prov_url_issue");
    v.push(kit::gap(field_gap));
    if ui.credential() == Credential::None {
        // `credentialRequirement === "none"`: no key field at all.
        v.push(Text::new("", tr("API key")).px(15.0).fill().one_line().dsl());
        v.push(kit::gap(8.0));
        v.push(kit::callout(false, false, tr(copy::KEY_NONE), None));
    } else {
        let eye = format!(
            "View {{ width: 36 height: 36 flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n{}{}}}\n",
            kit::svg("", if ui.key_revealed { "b1_eye_off.svg" } else { "b1_eye.svg" }, 22.0),
            kit::hit("b1_prov_eye", true)
        );
        // p4-07's red outline: the provider refused this key (on a Save or a
        // Test connection).
        let mut key = Field::new("b1_prov_key", &ui.key)
            .label(tr("API key"))
            .error(ui.key_rejected && (ui.screen == Screen::Rejected || ui.feedback.as_ref().is_some_and(|f| !f.ok)))
            .read_only(read_only)
            .trailing(eye);
        if !ui.key_revealed {
            key = key.password();
        }
        // A stored key is never sent back to the client: the field stays blank
        // and shows the board's mask as its placeholder (walk 87, "a blank
        // masked key"); typing replaces it, an empty field keeps the stored key.
        if ui.key.is_empty() && ui.key_stored {
            key = key.placeholder("••••••••••••••••••••••").placeholder_ink();
        } else if ui.key.is_empty() {
            key = key.placeholder(tr("Paste the provider's API key"));
        }
        v.push(key.dsl());
        v.input("b1_prov_key", "provider.key");
        v.returns("b1_prov_key", "provider.save");
        v.button("b1_prov_eye", "provider.key.reveal");
        issue_line(&mut v, ui, ms::Field::ApiKey, "b1_prov_key_issue");
    }
    // p4-07: a refused SAVE reads under the key it refused (the board).
    if ui.screen == Screen::Rejected {
        v.push(kit::gap(8.0));
        v.push(Text::new("b1_prov_error", &ui.failure_line()).px(15.0).color(kit::RED).fill().dsl());
        if !ui.save_refused {
            v.push(kit::gap(2.0));
            v.push(Text::new("b1_prov_kept", tr("Your draft is kept.")).px(15.0).color(kit::RED).fill().one_line().dsl());
        }
    }
    // The board's p4-06 order first (Name, Base URL, API key, Models), so
    // the editor opens on the approved composition; the web editor's other
    // controls follow it.
    if ui.mode == Mode::Edit {
        v.push(kit::gap(if l.phone { 18.0 } else { 14.0 }));
        models_section(&mut v, l, ui, read_only);
    }
    // The credential and its probe: the key's write-only note, the last
    // Test connection's line (where it was clicked), Test connection.
    if !read_only {
        v.push(kit::gap(if l.phone { 18.0 } else { 14.0 }));
        if ui.credential() != Credential::None {
            v.push(Text::new("b1_prov_key_hint", tr(if ui.credential_configured() { copy::KEY_CONFIGURED_HINT } else { copy::KEY_WRITE_ONLY_HINT }))
                .px(13.0)
                .color(kit::MUTED)
                .fill()
                .dsl());
        }
        if let Some(f) = &ui.feedback {
            v.push(kit::gap(8.0));
            v.push(kit::status_line("b1_prov_feedback", tr(&f.text), f.ok));
        }
        if ui.caps.test {
            v.push(kit::gap(10.0));
            let label = tr(if ui.busy && ui.last_op == Op::Test { copy::TESTING } else { copy::TEST });
            v.push(kit::pill_outline_fit("b1_prov_test", label, 40.0));
            v.button("b1_prov_test", "prov.test");
        }
    }
    // The route's protocol and credential reference (the web editor's
    // "API protocol" select and "Credential environment" field).
    v.push(kit::gap(if l.phone { 18.0 } else { 14.0 }));
    let protocol = ui.api_type.clone().unwrap_or_default();
    let plabel = options(ui, Select::Protocol)
        .into_iter()
        .find(|(id, _)| *id == protocol)
        .map(|(_, l)| l)
        .unwrap_or_else(|| tr("Choose a protocol").to_owned());
    select_field(&mut v, l, ui, Select::Protocol, "API protocol", &plabel, &protocol);
    issue_line(&mut v, ui, ms::Field::ApiProtocol, "b1_prov_protocol_issue");
    if ui.credential() != Credential::None {
        v.push(kit::gap(field_gap));
        let env = ui.api_key_env.clone().unwrap_or_default();
        v.push(Field::new("b1_prov_env", &env).label(tr("Credential environment")).placeholder("ZAI_API_KEY").read_only(read_only).dsl());
        v.input("b1_prov_env", "provider.env");
        if ui.issues.iter().any(|i| i.field == ms::Field::ApiKeyEnv) {
            issue_line(&mut v, ui, ms::Field::ApiKeyEnv, "b1_prov_env_issue");
        } else {
            hint(&mut v, "b1_prov_env_hint", tr(copy::ENV_HINT));
        }
    }
    v.push(kit::gap(field_gap));
    if ui.mode == Mode::Add {
        v.push(Field::new("b1_prov_route_id", &ui.route).label(tr("Route ID")).placeholder("deepseek").dsl());
        v.input("b1_prov_route_id", "provider.route_id");
        issue_line(&mut v, ui, ms::Field::RouteId, "b1_prov_route_issue");
    } else {
        // The fixed identity (board 1 #5's rows): an edit never renames it.
        let rows = vec![
            kit::kv_row(tr("Provider"), &format!("{} ({})", ui.family_label, ui.family)),
            kit::kv_row(tr("Route"), &ui.route),
        ];
        v.push(kit::list_card("b1_prov_identity", &rows));
        hint(&mut v, "b1_prov_identity_hint", &format!("{} {}", tr(copy::FIXED_PROVIDER), tr(copy::FIXED_ROUTE)));
    }
    v.push(kit::gap(if l.phone { 16.0 } else { 12.0 }));
    v.push(kit::callout(false, true, tr(copy::PARAMETERS_HEAD), Some(tr(copy::PARAMETERS_BODY))));
    if ui.model_id().trim().eq_ignore_ascii_case("glm-5.3-flash") {
        v.push(kit::gap(12.0));
        glm_guidance(&mut v);
    }
    v.push(kit::gap(4.0));
    v.push("}\n");
    if read_only {
        v.push(kit::gap(14.0));
        let why = tr(if ui.edit_blocked { copy::EDIT_BLOCKED } else { READ_ONLY });
        v.push(kit::callout(false, true, why, None));
        v.spacer(l, 16.0, 22.0);
        v.push(kit::pill_outline("b1_prov_cancel", tr("Close"), "Fill"));
        v.button("b1_prov_cancel", "provider.cancel");
        if l.phone {
            v.push(kit::gap(24.0));
        }
        return v;
    }
    if !ui.issues.is_empty() {
        v.push(kit::gap(10.0));
        v.push(Text::new("b1_prov_complete", tr(copy::COMPLETE_FIELDS)).px(14.0).color(kit::RED).fill().dsl());
    }
    v.spacer(l, 16.0, 22.0);
    let primary_label = tr(match (ui.busy, ui.screen) {
        (true, _) if ui.last_op == Op::Save => "Saving\u{2026}",
        (_, Screen::Rejected) => "Try again",
        _ => "Save",
    });
    let primary_action = if ui.screen == Screen::Rejected { "provider.retry" } else { "provider.save" };
    v.push(format!(
        "View {{ width: Fill height: Fit flow: Right spacing: 12\n{}{}}}\n",
        kit::pill_outline("b1_prov_cancel", tr("Cancel"), "Fill"),
        kit::pill_primary("b1_prov_save", primary_label, "Fill")
    ));
    v.button("b1_prov_cancel", "provider.cancel");
    v.button("b1_prov_save", primary_action);
    if l.phone {
        v.push(kit::gap(24.0));
    }
    v
}

/// `GlmFlashGuidance` (`ModelManagementSection.tsx:417-453`): read-only
/// provider guidance (board 1 #5's rows) with the official guide link.
fn glm_guidance(v: &mut Ui) {
    let mut rows = vec![format!(
        "View {{ width: Fill height: Fit flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 16 right: 8 top: 10 bottom: 8}} spacing: 8\nView {{ width: Fill height: Fit flow: Down spacing: 2\n{}{}}}\n{}}}\n",
        Text::new("b1_prov_glm_title", tr(copy::GLM_TITLE)).px(14.0).weight(500).fill().dsl(),
        // Wraps, never an ellipsis: on a phone the link leaves it ~150 px.
        Text::new("b1_prov_glm_sub", tr(copy::GLM_SUB)).px(13.0).color(kit::MUTED).fill().dsl(),
        kit::link("b1_prov_guide", tr(copy::GLM_LINK), kit::BLUE, 14.0, 500)
    )];
    v.button("b1_prov_guide", "provider.guide");
    for (k, val) in copy::GLM_GUIDANCE {
        rows.push(kit::kv_row(k, val));
    }
    v.push(kit::list_card("b1_prov_glm", &rows));
    hint(v, "b1_prov_glm_note", tr(copy::GLM_NOTE));
}

/// The bindings this screen projects. The KEY ITSELF is never a binding.
pub fn query(ui: &ProviderUi, id: &str) -> Option<Value> {
    match id {
        "prov.family" => Some(Value::String(ui.family.clone())),
        "provider.url" => Some(Value::String(ui.base_url.clone())),
        "prov.model" => Some(Value::String(ui.model.clone())),
        "prov.route" => Some(Value::String(ui.route.clone())),
        "prov.models" => Some(Value::Array(ui.models.iter().cloned().map(Value::String).collect())),
        "prov.key_masked" => Some(Value::String(ui.key_display().to_owned())),
        "prov.key_present" => Some(Value::Bool(!ui.key.is_empty() || ui.key_stored)),
        "prov.error" => ui.error.clone().map(Value::String),
        "prov.rejected" => Some(Value::Bool(ui.key_rejected)),
        "prov.default_model" => ui.default_model.clone().map(Value::String),
        _ => None,
    }
}

/// The live copy overrides for one Stage-B editor card (the design artifact).
/// The key is deliberately absent.
pub fn copies(screen: Screen, ui: &ProviderUi) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut push = |id: &str, v: &str| out.push((id.to_owned(), v.to_owned()));
    push("prov_name_text", &ui.name);
    push("prov_url_text", &ui.base_url);
    for (i, m) in ui.models.iter().enumerate().take(3) {
        let label = if ui.default_model.as_deref() == Some(m.as_str()) {
            tr1("{value0} (default)", m)
        } else {
            m.clone()
        };
        push(&format!("t_model_{i}_text"), &label);
    }
    if screen == Screen::Rejected {
        push("t_cal1_text", &ui.failure_line());
    }
    out
}

/// Lower one Stage-B editor card — the accepted design artifact; the app
/// mounts the native [`view`] (board1.rs explains why).
pub fn lower_screen(screen: Screen, ui: &ProviderUi) -> Result<String, String> {
    let dir = crate::design::dir("stage-b/phase4/cards").join(screen.card_dir());
    let card_src = std::fs::read_to_string(dir.join("page.card"))
        .map_err(|e| format!("read {}: {e}", dir.join("page.card").display()))?;
    let data: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("page.data.json")).map_err(|e| format!("read page.data.json: {e}"))?,
    )
    .map_err(|e| format!("parse page.data.json: {e}"))?;
    let card_src = crate::l0_host::apply_copies(&card_src, &copies(screen, ui));
    let prepared = octoscript_makepad::l0::prepare(&card_src, &data, &dir.join("kit"))
        .map_err(|e| format!("l0::prepare: {e}"))?;
    let mut tree = prepared.tree;
    octoscript_makepad::l0::inspectable(&mut tree);
    let dsl = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(&tree))
        .map_err(|e| format!("to_makepad_ui: {e}"))?;
    Ok(super::taps::wire_card_events_dir(&dsl, &dir))
}

// ------------------------------------------------------------- live state

/// The live editor state between taps (`workspace.rs:117-123` shape).
pub fn state() -> std::sync::MutexGuard<'static, ProviderUi> {
    static STATE: std::sync::OnceLock<std::sync::Mutex<ProviderUi>> = std::sync::OnceLock::new();
    STATE
        .get_or_init(|| std::sync::Mutex::new(ProviderUi::new("deepseek")))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Replace the live draft.
pub fn set(ui: ProviderUi) {
    *state() = ui;
}

/// Apply one action to the LIVE draft and return the transport effect.
pub fn perform(id: &str, value: Option<&str>) -> Option<Effect> {
    apply(&mut state(), resolve(id, value))
}

/// Seed the LIVE editor (Settings' entry) from the profile's configuration:
/// `profile/llm/list {profile_id}` for the primary route and
/// `profile/llm/catalog` for the family's models — through the controller's
/// read ([`model_settings::read`]), so the primary is parsed the web's way
/// (its inference values carried, an edit-blocked row refused).
pub async fn load(conv: &crate::flow::Conversation) -> Result<(), String> {
    let client = conv.client();
    let profile = Some(conv.profile()).filter(|p| !p.is_empty());
    let caps = state().caps;
    let read_caps = ms::Caps { read: caps.read, catalog: caps.catalog, ..Default::default() };
    let (config, catalog) = ms::read(client, profile.as_deref().unwrap_or(""), read_caps).await;
    let config = match config {
        Some(Ok(c)) => Some(c),
        Some(Err(e)) => return Err(e),
        None => None,
    };
    let catalog = catalog.and_then(Result::ok);
    let proj = ms::project(ms::Caps { read: true, ..read_caps }, false, config.as_ref(), catalog.as_ref(), None);
    let mut ui = state();
    if ui.edited {
        return Ok(());
    }
    ui.profile_id = config.as_ref().map(|c| c.profile_id.clone()).or(profile);
    ui.families = proj.families.clone();
    ui.protocols = proj.protocols.clone();
    ui.configured = proj.providers.clone();
    let seed = Seed {
        profile_id: ui.profile_id.clone(),
        caps: ui.caps,
        can_fetch: ui.can_fetch,
        families: proj.families.clone(),
        protocols: proj.protocols.clone(),
        configured: proj.providers.clone(),
        origin: ui.origin,
    };
    if config.as_ref().is_some_and(|c| c.is_empty()) && ui.caps.can_save() {
        // No provider yet: the catalog-driven Add form, whose first model
        // becomes the primary (`saveProvider`: an empty configuration).
        *ui = for_add(&seed, true);
    } else if let Some(p) = proj.providers.iter().find(|p| p.primary) {
        let mut fresh = for_row(p, &seed);
        // Settings' entry: the board's own fallbacks for an unlabelled
        // official route ("DeepSeek · Official API").
        if p.route.label == ms::route_label(&p.route.id) && p.route.base_url.is_empty() {
            fresh.route_label = "Official API".into();
            fresh.name = format!("{} \u{b7} Official API", fresh.family_label);
        }
        *ui = fresh;
    }
    Ok(())
}

/// The transport half: `prov.test` sends `profile/llm/test`; Save sends the
/// SAME params to `profile/llm/test` and, only when it passes, to
/// `profile/llm/upsert` with `set_primary` (`model-settings.ts:345-380`);
/// Fetch sends `profile/llm/fetch_models`. Every operation takes the
/// CONTROLLER draft (no key) and the key as its argument, read from the
/// editor only now. Every refusal goes through [`ProviderUi::reject`], which
/// redacts BEFORE any view can read it. `Ok(true)` = saved (the editor closes).
pub async fn perform_transport(conv: &crate::flow::Conversation, effect: Effect) -> Result<bool, String> {
    let op = match effect {
        Effect::Test => Op::Test,
        Effect::Save => Op::Save,
        Effect::Fetch => Op::Fetch,
        other => return Err(format!("screens/provider: {other:?} is not a transport effect")),
    };
    let (draft, key, profile, caps, can_fetch) = {
        let mut ui = state();
        if ui.profile_id.is_none() {
            ui.profile_id = Some(conv.profile()).filter(|p| !p.is_empty());
        }
        (ui.draft(), ui.key.clone(), ui.profile_id.clone().unwrap_or_default(), ui.caps, ui.can_fetch)
    };
    let allowed = match op {
        Op::Test => caps.test,
        Op::Save => caps.can_save(),
        Op::Fetch => can_fetch,
    };
    if !allowed {
        state().busy = false;
        return Err("profile/llm: not advertised by this server".into());
    }
    let client = conv.client();
    let key = Some(key.as_str()).filter(|k| !k.trim().is_empty());
    match op {
        Op::Test => match ms::test(client, &profile, &draft, key).await {
            Ok(t) if t.passed() => {
                let mut ui = state();
                ui.accept();
                ui.feedback = Some(Feedback { ok: true, text: copy::CONNECTION_OK.into() });
                Ok(false)
            }
            Ok(t) => {
                let reason = t.error.unwrap_or_else(|| if t.message.is_empty() { copy::TEST_DID_NOT_PASS.into() } else { t.message });
                state().test_failed(&reason);
                Err(format!("profile/llm/test: {}", redact(&reason, &state().key)))
            }
            Err(e) => {
                state().test_failed(&e);
                Err(format!("profile/llm/test: {}", redact(&e, &state().key)))
            }
        },
        Op::Fetch => match ms::fetch_models(client, &profile, &draft, key).await {
            Ok(f) => {
                let mut ui = state();
                ui.busy = false;
                let mut seen = Vec::new();
                for m in f.models {
                    if !seen.contains(&m) && seen.len() < 100 {
                        seen.push(m);
                    }
                }
                ui.fetch_feedback = Some(Feedback {
                    ok: true,
                    text: if seen.is_empty() { copy::NO_MODELS.into() } else { copy::models_found(seen.len()) },
                });
                ui.fetched = seen;
                Ok(false)
            }
            Err(e) => {
                let mut ui = state();
                ui.busy = false;
                ui.fetch_feedback = Some(Feedback { ok: false, text: copy::FETCH_FAILED.into() });
                Err(format!("profile/llm/fetch_models: {e}"))
            }
        },
        Op::Save => {
            let methods = ms::Caps { read: caps.read, test: caps.test, save: caps.save, ..Default::default() };
            match ms::save(client, &profile, &draft, key, methods).await {
                Ok(_) => {
                    let mut ui = state();
                    ui.accept();
                    ui.key.clear();
                    ui.key_stored = true;
                    ui.edited = false;
                    Ok(true)
                }
                Err(e) => {
                    // The web's save failure copy for anything but a key the
                    // provider refused (p4-07's own sentence).
                    perform_failed(&e);
                    let mut ui = state();
                    if !ui.key_rejected {
                        ui.save_refused = true;
                    }
                    Err(format!("profile/llm/test+upsert: {e}"))
                }
            }
        }
    }
}

/// A typed refusal from the transport, routed to the live draft so the copy
/// is redacted against the key BEFORE it can reach a view.
pub fn perform_failed(reason: &str) {
    apply(&mut state(), Effect::Failed { reason: reason.to_owned() });
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "sk-rejected-secret";

    #[test]
    fn each_operation_is_gated_on_its_own_advertised_method() {
        // model-settings.ts:156-170 and :349 (Save = test AND upsert).
        let m = |names: &[&str]| names.iter().map(|n| format!("profile/llm/{n}")).collect::<Vec<_>>();
        assert_eq!(Caps::from_methods(&m(&["list", "catalog", "test", "upsert"])), Caps::ALL);
        let read_only = Caps::from_methods(&m(&["list", "catalog", "test"]));
        assert!(read_only.read && read_only.test && !read_only.can_save());
        assert!(!Caps::from_methods(&m(&["list", "catalog", "upsert"])).can_save(), "Save needs Test too");
        assert_eq!(Caps::from_methods(&[]), Caps::default(), "fails closed");
        let mut ui = ProviderUi::new("deepseek");
        ui.caps = read_only;
        assert_eq!(apply(&mut ui, Effect::Save), None, "no upsert advertised -> no Save");
        // A23: without a writable editor there is no Test either (the web
        // renders no editor without onSave).
        assert_eq!(apply(&mut ui, Effect::Test), None);
        ui.caps = Caps::ALL;
        assert_eq!(apply(&mut ui, Effect::Test), Some(Effect::Test), "Test is its own operation");
        ui.busy = false;
        ui.caps.test = false;
        assert_eq!(apply(&mut ui, Effect::Test), None);
    }

    #[test]
    fn a_rejected_test_keeps_the_whole_draft() {
        // walk row 88 — family, model, route and the typed key all survive.
        let mut ui = ProviderUi::new("deepseek");
        ui.key = SECRET.to_owned();
        apply(&mut ui, Effect::Failed { reason: "401 unauthorized".into() });
        assert_eq!(ui.family, "deepseek");
        assert_eq!(ui.model, "deepseek-chat");
        assert_eq!(ui.route, "openrouter");
        assert_eq!(ui.key, SECRET);
        assert_eq!(ui.screen, Screen::Rejected);
    }

    #[test]
    fn the_recorded_401_reads_as_a_key_rejection_with_its_status() {
        // r29a line 10 (the real a6ea8505 reply).
        let raw = "API error (deepseek@api/deepseek-v4-flash, api_style=openai_chat_completions): authentication failed \u{2014} HTTP 401 - {\"error\":{\"message\":\"Authentication Fails\"}}";
        let mut ui = ProviderUi::new("deepseek");
        ui.reject(raw);
        assert!(ui.key_rejected);
        assert_eq!(ui.error_status, Some(401));
        assert_eq!(ui.failure_line(), "The provider rejected this key (401).");
    }

    #[test]
    fn the_key_never_reaches_the_copies_the_bindings_or_the_view_text() {
        let mut ui = ProviderUi::new("deepseek");
        ui.key = SECRET.to_owned();
        ui.reject(&format!("401 for {SECRET}"));
        for (_, v) in copies(Screen::Rejected, &ui) {
            assert!(!v.contains(SECRET), "the key leaked into a copy: {v:?}");
        }
        for id in ["prov.family", "provider.url", "prov.model", "prov.route", "prov.models", "prov.key_masked", "prov.key_present", "prov.error", "prov.rejected", "prov.default_model"] {
            if let Some(v) = query(&ui, id) {
                assert!(!v.to_string().contains(SECRET), "the key leaked into binding {id}: {v}");
            }
        }
        assert!(ui.error.as_deref().is_some_and(|e| !e.contains(SECRET)));
        // A23: the controller draft carries no key at all.
        let d = serde_json::to_string(&ui.draft()).unwrap();
        assert!(!d.contains(SECRET) && !d.contains("api_key\""), "{d}");
    }

    #[test]
    fn save_and_test_send_one_param_shape() {
        let mut ui = ProviderUi::new("deepseek");
        ui.route = "deepseek".into();
        ui.key = "k".into();
        let d = ui.draft();
        let t = ms::provision("p", &d, Some(&ui.key)).unwrap();
        assert_eq!(t["selection"]["model_id"], "deepseek-v4-flash");
        assert_eq!(t["api_key"], "k", "the key joins the request only");
        assert!(t.get("set_primary").is_none());
        assert!(d.set_primary, "an edit of the primary stays the primary");
        // The official endpoint is not sent back as an override.
        assert!(t["selection"]["route"].get("base_url").is_none(), "{t}");
    }

    #[test]
    fn redaction_removes_the_secret_raw_and_encoded_and_keyed_runs() {
        let r = redact(&format!("401 for {SECRET}"), SECRET);
        assert!(!r.contains(SECRET) && r.contains("[redacted]"));
        let enc = percent_encode(SECRET);
        assert!(!redact(&format!("401 for {enc}"), SECRET).contains(&enc));
        let r = redact("api_key=abcd1234 and Bearer tok_live_9", "");
        assert!(!r.contains("abcd1234") && !r.contains("tok_live_9"), "{r}");
        assert_eq!(redact(&"x".repeat(5_000), "").chars().count(), MAX_REDACTED);
    }

    #[test]
    fn a_failed_test_connection_reads_where_it_was_clicked_and_keeps_save() {
        let mut ui = ProviderUi::new("deepseek");
        ui.key = SECRET.to_owned();
        ui.test_failed(&format!("authentication failed \u{2014} HTTP 401 for {SECRET}"));
        assert_eq!(ui.screen, Screen::Editor, "Save stays Save");
        let f = ui.feedback.clone().unwrap();
        assert!(!f.ok && f.text == "The provider rejected this key (401). Your draft is kept.", "{f:?}");
        assert!(ui.key_rejected && !ui.error.as_deref().unwrap().contains(SECRET));
        ui.test_failed("upstream timeout");
        assert_eq!(ui.feedback.unwrap().text, format!("{TEST_FAILED} Your draft is kept."));
    }

    #[test]
    fn a_passed_test_clears_the_rejection() {
        let mut ui = ProviderUi::new("deepseek");
        ui.reject("401");
        ui.accept();
        assert!(ui.error.is_none() && !ui.key_rejected);
        assert_eq!(ui.screen, Screen::Editor);
    }

    #[test]
    fn a_second_save_while_one_is_in_flight_is_dropped() {
        let mut ui = ProviderUi::new("deepseek");
        assert_eq!(apply(&mut ui, Effect::Save), Some(Effect::Save));
        assert_eq!(apply(&mut ui, Effect::Save), None);
    }

    #[test]
    fn the_name_field_carries_the_route_label() {
        let mut ui = ProviderUi::new("deepseek");
        ui.name = "DeepSeek \u{b7} Work route".into();
        assert_eq!(ui.label_from_name(), "Work route");
    }

    #[test]
    fn every_declared_action_is_routed() {
        assert_eq!(unrouted(), Vec::<&str>::new());
        assert!(is_action("provider.model.7") && is_routed("provider.option.3") && is_action("provider.fetched.0"));
        assert!(!is_action("provider.model.x"));
    }

    #[test]
    fn a_long_model_row_wraps_in_a_taller_row_never_an_ellipsis() {
        let mut ui = ProviderUi::new("anthropic");
        ui.mode = Mode::Add;
        ui.config_empty = true;
        ui.model = "claude-3-5-haiku-20241022".to_owned();
        ui.default_model = Some(ui.model.clone());
        let list = vec!["claude-3-5-haiku-20241022".to_owned(), "claude-opus-4".to_owned()];
        let phone = Layout::of(super::super::board1::Surface::Provider, 360.0, 776.0);
        let rows = model_rows(&mut Ui::default(), &phone, &ui, &list, "provider.model", "b1_prov_model", true);
        assert!(rows[0].0.contains("claude-3-5-haiku-20241022 (default)") && rows[0].0.contains("height: 68") && rows[0].1 == 68.0, "{}", rows[0].0);
        assert!(rows[0].0.contains("Right{wrap: true}") && !rows[0].0.contains("Ellipsis"), "{}", rows[0].0);
        assert!(rows[1].0.contains("height: 46") && rows[1].0.contains("Ellipsis"), "{}", rows[1].0);
        let desk = Layout::of(super::super::board1::Surface::Provider, 990.0, 603.0);
        let rows = model_rows(&mut Ui::default(), &desk, &ui, &list, "provider.model", "b1_prov_model", true);
        assert!(rows.iter().all(|(r, h)| r.contains("height: 40") && *h == 40.0), "the desktop card fits both on one line");
        // Six rows: the scroll box shows five FULL rows (the wrapped one included) and their hairlines.
        let six: Vec<(String, f64)> = [68.0, 46.0, 46.0, 46.0, 46.0, 46.0].iter().map(|h| (String::new(), *h)).collect();
        assert_eq!(five_rows_h(&six), Some(68.0 + 4.0 * 46.0 + 4.0));
        assert_eq!(five_rows_h(&six[..5]), None);
    }

    #[test]
    fn an_invalid_draft_reaches_no_transport_and_names_its_fields() {
        let mut ui = ProviderUi::new("deepseek");
        ui.api_type = None;
        ui.api_key_env = Some(String::new());
        assert_eq!(apply(&mut ui, Effect::Save), None);
        let fields: Vec<ms::Field> = ui.issues.iter().map(|i| i.field).collect();
        assert_eq!(fields, [ms::Field::ApiProtocol, ms::Field::ApiKeyEnv]);
        assert!(!ui.busy);
    }
}
