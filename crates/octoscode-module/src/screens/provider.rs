//! #D1/#A2 — the provider editor (atlas board 1 screens p4-06/07), one owner
//! per action id.
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
//! 3. **The key never reaches the page text** — it is held in the draft and in
//!    the password field's value only (the instrument shows `t: "•••"` and
//!    keeps the value in `val`, like an `<input>` value outside
//!    `textContent`); no label, copy id or binding ever carries it.
//! 4. **The failure is redacted, not echoed** — [`redact`] is the web's
//!    `redactModelSettingsError` (`model-settings.ts:523-543`); and the screen
//!    shows the board's own sentence, never the server's prose.
use octoscode_client::domains::profile::{
    LlmCatalog, LlmCatalogParams, LlmInferenceOverrides, LlmProvisionParams, LlmRouteSelection,
    LlmSelection, LlmTest, LlmUpsert, ProfileLlmList, ProfileLlmListParams,
};
use serde_json::Value;

use super::board1::{Layout, Ui};
use super::board1_kit::{self as kit, Field, Text};

/// The web's own alert copy (`model-management.spec.ts:181-183`), shown when a
/// failure is not a key rejection.
pub const TEST_FAILED: &str =
    "Connection failed. Check the endpoint, protocol, model, and credential.";

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
/// the save omits `base_url` unless the operator changed it.
pub fn family_meta(family: &str) -> (String, &'static str) {
    let (label, url) = match family {
        "deepseek" => ("DeepSeek", "https://api.deepseek.com/v1"),
        "openai" => ("OpenAI", "https://api.openai.com/v1"),
        "anthropic" => ("Anthropic", "https://api.anthropic.com"),
        "moonshot" | "moonshot-coding" => ("Moonshot", "https://api.moonshot.ai/v1"),
        "openrouter" => ("OpenRouter", "https://openrouter.ai/api/v1"),
        "zhipu" => ("Zhipu", "https://open.bigmodel.cn/api/paas/v4"),
        "zai" | "zai-coding" => ("Z.ai", "https://api.z.ai/api/paas/v4"),
        "gemini" => ("Gemini", "https://generativelanguage.googleapis.com/v1beta"),
        "groq" => ("Groq", "https://api.groq.com/openai/v1"),
        "dashscope" => ("DashScope", "https://dashscope.aliyuncs.com/compatible-mode/v1"),
        "minimax" | "minimax-cn" => ("MiniMax", "https://api.minimax.io/v1"),
        _ => ("", ""),
    };
    let label = if label.is_empty() {
        let mut c = family.chars();
        c.next()
            .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
            .unwrap_or_default()
    } else {
        label.to_owned()
    };
    (label, url)
}

/// The editor's live draft (`ModelSettingsDraft`, `model-settings.ts:55-64`):
/// the values are held so a failed test can keep them (row 88); the credential
/// is a separate field that no copy id and no binding ever returns.
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
    /// as the password field's masked value.
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
    /// Where we are.
    pub screen: Screen,
}

impl std::fmt::Debug for ProviderUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderUi")
            .field("family", &self.family)
            .field("route", &self.route)
            .field("model", &self.model)
            .field("models", &self.models)
            .field("key", &format_args!("<{} chars>", self.key.chars().count()))
            .field("screen", &self.screen)
            .finish()
    }
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
            models: vec![
                "deepseek-v4-flash".to_owned(),
                "deepseek-v4".to_owned(),
                "deepseek-chat".to_owned(),
            ],
            default_model: Some("deepseek-v4-flash".to_owned()),
            screen: Screen::Editor,
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
        self.screen = Screen::Rejected;
    }

    /// The test passed: drop the error, leave the draft alone.
    pub fn accept(&mut self) {
        self.error = None;
        self.error_status = None;
        self.key_rejected = false;
        self.busy = false;
        self.screen = Screen::Editor;
    }

    /// The board's p4-07 sentence for the current failure (never the raw text).
    pub fn failure_line(&self) -> String {
        if self.key_rejected {
            match self.error_status {
                Some(n) => format!("The provider rejected this key ({n})."),
                None => "The provider rejected this key.".to_owned(),
            }
        } else {
            TEST_FAILED.to_owned()
        }
    }

    /// Seed the editor from the profile's configured primary route and the
    /// catalog's models for its family (`profile/llm/list` + `catalog`).
    pub fn seed(
        &mut self,
        profile_id: Option<String>,
        primary: Option<&octoscode_client::domains::profile::ProfileLlmConfiguredModel>,
        catalog_models: &[String],
    ) {
        if self.edited {
            return;
        }
        self.profile_id = profile_id;
        if let Some(p) = primary {
            let (label, default_url) = family_meta(&p.family_id);
            self.family = p.family_id.clone();
            self.family_label = label;
            self.model = p.model_id.clone();
            self.default_model = Some(p.model_id.clone());
            self.route = p.route.route_id.clone().unwrap_or_else(|| p.family_id.clone());
            self.route_label = p.route.label.clone().unwrap_or_else(|| "Official API".to_owned());
            self.name = format!("{} \u{b7} {}", self.family_label, self.route_label);
            self.default_base_url = default_url.to_owned();
            self.base_url = p.route.base_url.clone().unwrap_or_else(|| default_url.to_owned());
            self.api_key_env = p.route.api_key_env.clone();
            self.api_type = p.route.api_type.clone();
            self.key_stored = p.has_api_key;
            let mut models: Vec<String> = catalog_models.to_vec();
            if !models.contains(&p.model_id) {
                models.insert(0, p.model_id.clone());
            }
            // The default first, the way the board lists it.
            models.sort_by_key(|m| m != &p.model_id);
            self.models = models;
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

    /// The `profile/llm/{test,upsert}` params, built ONCE per save
    /// (`model-settings.ts:345-353`). The default model row goes on the wire;
    /// `base_url` only when it differs from the official endpoint; the key
    /// only when one was typed (omitted = reuse the stored one,
    /// `LlmProvisionParams::api_key`).
    pub fn provision(&self, set_primary: bool) -> LlmProvisionParams {
        let model = self
            .default_model
            .clone()
            .or_else(|| self.models.first().cloned())
            .unwrap_or_else(|| self.model.clone());
        let base = self.base_url.trim();
        let base_url = (!base.is_empty() && base != self.default_base_url).then(|| base.to_owned());
        LlmProvisionParams {
            profile_id: self.profile_id.clone(),
            selection: LlmSelection {
                family_id: self.family.clone(),
                model_id: model,
                route: LlmRouteSelection {
                    route_id: Some(self.route.clone()),
                    label: Some(self.label_from_name()),
                    base_url,
                    api_key_env: self.api_key_env.clone(),
                    api_type: self.api_type.clone(),
                },
                inference: LlmInferenceOverrides::default(),
            },
            api_key: (!self.key.trim().is_empty()).then(|| self.key.trim().to_owned()),
            set_primary: set_primary.then_some(true),
        }
    }
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
    ("provider.retry", "Try again (p4-07): the same Save with the draft kept"),
    ("provider.cancel", "discard the editor"),
    ("provider.back", "the back chevron: discard the editor"),
];

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
];

pub fn is_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id)
}

pub fn is_routed(id: &str) -> bool {
    ROUTED.contains(&id)
}

pub fn unrouted() -> Vec<&'static str> {
    ACTIONS.iter().map(|(a, _)| *a).filter(|a| !ROUTED.contains(a)).collect()
}

/// What an action means.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Input { field: &'static str, value: String },
    /// Make the model row at this index the default.
    SelectModel(usize),
    ToggleKey,
    /// `profile/llm/test` alone.
    Test,
    /// `profile/llm/test` then `profile/llm/upsert`.
    Save,
    /// Leave the editor (no transport).
    Close,
    /// A redacted failure the caller just received.
    Failed { reason: String },
    Unhandled,
}

/// Route one action id to its effect. `value` carries an input payload.
pub fn resolve(id: &str, value: Option<&str>) -> Effect {
    let v = || value.unwrap_or_default().to_owned();
    match id {
        "provider.name" => Effect::Input { field: "provider.name", value: v() },
        "provider.url" => Effect::Input { field: "provider.url", value: v() },
        "provider.key" => Effect::Input { field: "provider.key", value: v() },
        "provider.key.reveal" => Effect::ToggleKey,
        "provider.model.0" => Effect::SelectModel(0),
        "provider.model.1" => Effect::SelectModel(1),
        "provider.model.2" => Effect::SelectModel(2),
        "provider.model.3" => Effect::SelectModel(3),
        "prov.test" => Effect::Test,
        "provider.save" | "provider.retry" => Effect::Save,
        "provider.cancel" | "provider.back" => Effect::Close,
        _ => Effect::Unhandled,
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
                _ => {}
            }
            ui.edited = true;
            None
        }
        Effect::SelectModel(i) => {
            if let Some(m) = ui.models.get(i).cloned() {
                ui.model = m.clone();
                ui.default_model = Some(m);
                ui.edited = true;
            }
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
        Effect::Test | Effect::Save => {
            if ui.busy {
                return None; // one request at a time (latest-request-wins)
            }
            ui.busy = true;
            Some(effect)
        }
        Effect::Close => Some(Effect::Close),
        Effect::Unhandled => None,
    }
}

// --------------------------------------------------------------------- views

/// The native view of the editor (p4-06, or p4-07 after a rejection).
pub fn view(ui: &ProviderUi, l: &Layout) -> Ui {
    let mut v = Ui::default();
    v.header(l, "b1_prov_back", "provider.back", "Edit provider");
    let field_gap = if l.phone { 14.0 } else { 10.0 };
    v.push(kit::gap(if l.phone { 16.0 } else { 12.0 }));
    v.push(Field::new("b1_prov_name", &ui.name).label("Name").placeholder("Provider · Route").dsl());
    v.input("b1_prov_name", "provider.name");
    v.push(kit::gap(field_gap));
    v.push(Field::new("b1_prov_url", &ui.base_url).label("Base URL").placeholder(&ui.default_base_url).dsl());
    v.input("b1_prov_url", "provider.url");
    v.push(kit::gap(field_gap));
    let eye = format!(
        "View {{ width: 36 height: 36 flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n{}{}}}\n",
        kit::svg("", if ui.key_revealed { "b1_eye_off.svg" } else { "b1_eye.svg" }, 22.0),
        kit::hit("b1_prov_eye", true)
    );
    let mut key = Field::new("b1_prov_key", &ui.key)
        .label("API key")
        .error(ui.screen == Screen::Rejected && ui.key_rejected)
        .trailing(eye);
    if !ui.key_revealed {
        key = key.password();
    }
    // A stored key is never sent back to the client: the field stays blank and
    // shows the board's mask as its placeholder (walk 87, "a blank masked
    // key"); typing replaces it, an empty field keeps the stored key.
    if ui.key.is_empty() && ui.key_stored {
        key = key.placeholder("••••••••••••••••••••••").placeholder_ink();
    } else if ui.key.is_empty() {
        key = key.placeholder("Paste the provider's API key");
    }
    v.push(key.dsl());
    v.input("b1_prov_key", "provider.key");
    v.returns("b1_prov_key", "provider.save");
    v.button("b1_prov_eye", "provider.key.reveal");
    if ui.screen == Screen::Rejected {
        v.push(kit::gap(8.0));
        v.push(Text::new("b1_prov_error", &ui.failure_line()).px(14.0).color(kit::RED).fill().dsl());
        v.push(kit::gap(2.0));
        v.push(Text::new("b1_prov_kept", "Your draft is kept.").px(14.0).color(kit::RED).fill().one_line().dsl());
    }
    v.push(kit::gap(if l.phone { 18.0 } else { 14.0 }));
    v.push(Text::new("", "Models").px(14.0).weight(500).fill().one_line().dsl());
    v.push(kit::gap(if l.phone { 8.0 } else { 6.0 }));
    let rows: Vec<String> = ui
        .models
        .iter()
        .take(4)
        .enumerate()
        .map(|(i, m)| {
            let is_default = ui.default_model.as_deref() == Some(m.as_str());
            let label = if is_default { format!("{m} (default)") } else { m.clone() };
            let id = format!("b1_prov_model_{i}");
            v.button(&id, &format!("provider.model.{i}"));
            format!(
                "View {{ width: Fill height: {} flow: Overlay\nView {{ width: Fill height: Fill flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 14 right: 12}} spacing: 10\n{}{}}}\n{}}}\n",
                if l.phone { 46 } else { 40 },
                Text::new(&format!("b1_prov_model_t{i}"), &label).px(14.0).fill().one_line().dsl(),
                kit::svg("", "b1_check_on.svg", 23.0),
                kit::hit(&id, true)
            )
        })
        .collect();
    v.push(kit::list_card("b1_prov_models", &rows));
    v.spacer(l, 16.0, 22.0);
    let primary_label = match (ui.busy, ui.screen) {
        (true, _) => "Saving\u{2026}",
        (false, Screen::Rejected) => "Try again",
        (false, Screen::Editor) => "Save",
    };
    let primary_action = if ui.screen == Screen::Rejected { "provider.retry" } else { "provider.save" };
    v.push(format!(
        "View {{ width: Fill height: Fit flow: Right spacing: 12\n{}{}}}\n",
        kit::pill_outline("b1_prov_cancel", "Cancel", "Fill"),
        kit::pill_primary("b1_prov_save", primary_label, "Fill")
    ));
    v.button("b1_prov_cancel", "provider.cancel");
    v.button("b1_prov_save", primary_action);
    if l.phone {
        v.push(kit::gap(24.0));
    }
    v
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
            format!("{m} (default)")
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

/// Seed the LIVE editor from the profile's configuration: `profile/llm/list`
/// (`ProfileLlmConfigReadParams { profile_id }`, `onboarding.ts:98`) for the
/// primary route and `profile/llm/catalog` for the family's models. Both
/// through the TYPED client.
pub async fn load(conv: &crate::flow::Conversation) -> Result<(), String> {
    let client = conv.client();
    let profile = Some(conv.profile()).filter(|p| !p.is_empty());
    let list = client
        .call::<ProfileLlmList>(ProfileLlmListParams { session_id: None, profile_id: profile.clone() })
        .await
        .map_err(|e| e.to_string())?;
    let catalog = client.call::<LlmCatalog>(LlmCatalogParams {}).await.ok();
    let family = list.primary.as_ref().map(|p| p.family_id.clone()).unwrap_or_default();
    let models: Vec<String> = catalog
        .as_ref()
        .and_then(|c| c.families.iter().find(|f| f.id == family))
        .map(|f| f.models.iter().map(|m| m.id.clone()).collect())
        .unwrap_or_default();
    state().seed(list.profile_id.clone().or(profile), list.primary.as_ref(), &models);
    Ok(())
}

/// The transport half: `prov.test` sends `profile/llm/test`; Save sends the
/// SAME params to `profile/llm/test` and, only when it passes, to
/// `profile/llm/upsert` with `set_primary` (`model-settings.ts:345-380`).
/// Every refusal goes through [`ProviderUi::reject`], which redacts BEFORE any
/// view can read it. `Ok(true)` = saved (the editor closes).
pub async fn perform_transport(conv: &crate::flow::Conversation, effect: Effect) -> Result<bool, String> {
    let save = match effect {
        Effect::Test => false,
        Effect::Save => true,
        other => return Err(format!("screens/provider: {other:?} is not a transport effect")),
    };
    let params = {
        let mut ui = state();
        if ui.profile_id.is_none() {
            ui.profile_id = Some(conv.profile()).filter(|p| !p.is_empty());
        }
        ui.provision(false)
    };
    let client = conv.client();
    let tested = match client.call::<LlmTest>(params.clone()).await {
        Ok(t) => t,
        Err(e) => {
            perform_failed(&e.to_string());
            return Err(format!("profile/llm/test: {}", redact(&e.to_string(), &state().key)));
        }
    };
    if let Some(want) = &params.profile_id {
        if &tested.profile_id != want {
            perform_failed("profile/llm/test answered for another profile");
            return Err("profile/llm/test: profile mismatch".into());
        }
    }
    let test_error = tested.error.clone().filter(|e| !e.is_empty());
    if !tested.applied || test_error.is_some() {
        let reason = test_error.unwrap_or_else(|| {
            if tested.message.is_empty() { "The provider test did not pass.".to_owned() } else { tested.message.clone() }
        });
        perform_failed(&reason);
        return Err(format!("profile/llm/test: {}", redact(&reason, &state().key)));
    }
    if !save {
        state().accept();
        return Ok(false);
    }
    let saved = match client
        .call::<LlmUpsert>(LlmProvisionParams { set_primary: Some(true), ..params })
        .await
    {
        Ok(s) => s,
        Err(e) => {
            perform_failed(&e.to_string());
            return Err(format!("profile/llm/upsert: {}", redact(&e.to_string(), &state().key)));
        }
    };
    if !saved.applied {
        perform_failed("The provider draft was not applied.");
        return Err("profile/llm/upsert: not applied".into());
    }
    let mut ui = state();
    ui.accept();
    ui.key.clear();
    ui.key_stored = true;
    ui.edited = false;
    Ok(true)
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
    }

    #[test]
    fn save_and_test_send_one_param_shape() {
        let mut ui = ProviderUi::new("deepseek");
        ui.route = "deepseek".into();
        ui.key = "k".into();
        let t = serde_json::to_value(ui.provision(false)).unwrap();
        let s = serde_json::to_value(ui.provision(true)).unwrap();
        assert_eq!(t["selection"], s["selection"], "Test and Save cannot drift");
        assert!(t.get("set_primary").is_none());
        assert_eq!(s["set_primary"], true);
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
    }
}
