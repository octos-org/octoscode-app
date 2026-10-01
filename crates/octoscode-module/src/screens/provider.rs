//! #D1 — the provider editor (atlas screens p4-06/07), one owner per action id.
//!
//! The web's contract is `features/models/model-settings.ts` plus the spec
//! `e2e/model-management.spec.ts`. Three properties the editor must keep, each
//! with its citation:
//!
//! 1. **The draft is kept when the provider rejects the key** — walk row 88
//!    pins that after a failed Test connection the family id, model id, route
//!    and the typed key all still hold their values
//!    (`model-management.spec.ts:182-196`), and the app does the same at
//!    `App.tsx:634` ("the failure is surfaced, never swallowed, and the draft is
//!    kept").
//! 2. **The key never reaches the page text** — the spec asserts the rejected
//!    credential appears nowhere in `document.documentElement.textContent`
//!    (`:190-201`). The draft therefore holds no raw credential field at all
//!    (`model-settings.ts:55`: "A provider/model draft deliberately contains no
//!    raw credential field"); ours does the same.
//! 3. **The failure is redacted, not echoed** — `redactModelSettingsError`
//!    (`model-settings.ts:523-543`) replaces the secret itself (raw *and*
//!    URL-encoded), then any `api_key: …` / `Bearer …` run, then truncates to
//!    1000 chars. [`redact`] is that function, the same three passes.
//!
//! The draft's credential is read only when a request is built and never stored
//! in a copy id, so the editor's visible state is the redaction's guarantee.

use serde_json::Value;

/// The alert copy the spec pins verbatim
/// (`model-management.spec.ts:181-183`).
pub const TEST_FAILED: &str =
    "Connection failed. Check the endpoint, protocol, model, and credential.";

/// The web truncates a redacted message at this length (`model-settings.ts:542`).
const MAX_REDACTED: usize = 1_000;

/// The placeholder for a stored-but-not-shown credential.
const DOTS: &str = "••••••••••••••••••";

/// The two editor cards (design/stage-b/phase4/cards).
pub const CARDS: &[(&str, &str)] = &[("provider", "p4-06"), ("provider_rejected", "p4-07")];

/// Which provider-editor card is mounted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    /// `p4-06` — the plain editor.
    #[default]
    Editor,
    /// `p4-07` — the same editor after the provider rejected the key.
    Rejected,
}

impl Screen {
    pub const ALL: [(Screen, &'static str); 2] =
        [(Screen::Editor, "p4-06"), (Screen::Rejected, "p4-07")];

    /// The card directory under `design/stage-b/phase4/cards`.
    pub fn card_dir(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(s, _)| *s == self)
            .map(|(_, dir)| *dir)
            .expect("every screen has a card")
    }
}

/// Redact a provider failure the way the web's `redactModelSettingsError` does
/// (`model-settings.ts:523-543`): the secret itself — raw and URL-encoded — then
/// any `api_key:`/`api-key`/`apikey` run, then any `Bearer …` run, then a
/// 1000-char cut.
pub fn redact(reason: &str, secret: &str) -> String {
    let mut message = reason.to_owned();
    let trimmed = secret.trim();
    for candidate in [Some(secret), Some(trimmed)]
        .into_iter()
        .flatten()
        .filter(|c| !c.is_empty())
    {
        // A `HashSet`-like dedup so a secret that equals its own trimmed form is
        // not replaced twice (the web's `new Set(candidates)`, :531).
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

/// The `api_key:`/`api-key=`/`apikey =` pass (`model-settings.ts:540`): the key
/// name and its separator survive, the following non-space run becomes
/// `[redacted]`. No regex crate is available here, so the three spellings the
/// web's `/api[_-]?key\s*[:=]\s*[^\s,;]+/gi` matches are scanned by hand.
fn redact_keyed_runs(s: &str) -> String {
    const NAMES: [&str; 3] = ["apikey", "api_key", "api-key"];
    let bytes = s.as_bytes();
    let lower = s.to_ascii_lowercase();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    'outer: while i < s.len() {
        for name in NAMES {
            if lower[i..].starts_with(name) {
                // The name must not be the tail of a longer word.
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
        // Advance one whole char (the strings here are ASCII-delimited by the
        // byte scan above, but multi-byte copy must not be split).
        let ch = s[i..].chars().next().expect("i is a char boundary");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// The `Bearer <token>` pass (`model-settings.ts:541`).
fn redact_bearer(s: &str) -> String {
    // The scan and the slice must run over the SAME string: lowercasing can
    // change byte lengths for non-ASCII, so a position found in a lowercased
    // copy is not a position in the original.
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < s.len() {
        let tail = &s[i..];
        if !tail[..tail.len().min(7)].eq_ignore_ascii_case("bearer ") {
            let ch = tail.chars().next().expect("i is a char boundary");
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        let after = 7usize; // "bearer ".len() == 7
        out.push_str(&s[i..i + after]);
        let token = &s[i + after..];
        let end = token
            .find(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .unwrap_or(token.len());
        out.push_str("[redacted]");
        i += after + end;
    }
    out
}

/// Minimal `encodeURIComponent` for the redaction pass — the web also tries the
/// encoded form of the secret (`:534`).
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*'
            | b'\'' | b'(' | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The editor's live draft.
///
/// Mirrors `ModelSettingsDraft` (`model-settings.ts:55-64`): the *values* are
/// held here so a failed test can keep them (row 88), but the credential is a
/// separate field that is never projected into a copy id and never returned by
/// [`Screen`]'s bindings.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProviderUi {
    /// "Provider / family ID" — `deepseek`.
    pub family: String,
    /// "Model ID" — the default the spec expects the form to pre-fill.
    pub model: String,
    /// "Route ID" — the default the spec expects the form to pre-fill.
    pub route: String,
    /// The Base URL, shown on the card as a plain field.
    pub base_url: String,
    /// The typed key. Held so a rejected draft keeps it (row 88), never shown.
    pub key: String,
    /// The three model rows, top to bottom, with the default first
    /// (`atlas-prompt.md:40-41`).
    pub models: Vec<String>,
    /// Which model row is the default.
    pub default_model: Option<String>,
    /// The redacted failure, if the last test failed. Never the raw prose.
    pub error: Option<String>,
    /// The key field is outlined red (p4-07).
    pub key_rejected: bool,
    /// Where we are.
    pub screen: Screen,
}

impl ProviderUi {
    /// A new draft with the defaults the spec pins (`:184-187`:
    /// model `deepseek-chat`, route `openrouter`).
    pub fn new(family: &str) -> Self {
        Self {
            family: family.to_owned(),
            model: "deepseek-chat".to_owned(),
            route: "openrouter".to_owned(),
            base_url: "https://api.deepseek.com/v1".to_owned(),
            models: vec![
                "deepseek-v4-flash (default)".to_owned(),
                "deepseek-v4".to_owned(),
                "deepseek-chat".to_owned(),
            ],
            default_model: Some("deepseek-v4-flash".to_owned()),
            screen: Screen::Editor,
            ..Default::default()
        }
    }

    /// The key as the UI may show it. A rejected draft keeps the key *value*
    /// (row 88) but shows dots — the raw key is never a copy id
    /// (`model-management.spec.ts:190-201`).
    pub fn key_display(&self) -> &'static str {
        if self.key.is_empty() {
            ""
        } else {
            DOTS
        }
    }

    /// A failure came back: redact it against the live key, keep the draft, and
    /// move to p4-07. This is the single place a refusal enters the editor.
    pub fn reject(&mut self, raw_reason: &str) {
        self.error = Some(redact(raw_reason, &self.key));
        self.key_rejected = true;
        self.screen = Screen::Rejected;
        // Property 1: the draft survives untouched. The fields are only ever
        // written by `Input`, so this arm deliberately clears nothing but the
        // screen's error state.
    }

    /// The test passed: drop the error, leave the draft alone.
    pub fn accept(&mut self) {
        self.error = None;
        self.key_rejected = false;
        self.screen = Screen::Editor;
    }
}

/// The action ids these two cards emit, with what each one means.
pub const ACTIONS: &[(&str, &str)] = &[
    ("prov.name", "the Name field's live text (p4-06/p4-07)"),
    ("prov.url", "the Base URL field's live text (p4-06/p4-07)"),
    ("prov.key", "the API key field's live text — read at dispatch, never shown"),
    ("key.eye", "reveal/hide the masked key (the key stays masked on a refusal)"),
    ("model.row.0", "make the first listed model the default"),
    ("model.row.1", "make the second listed model the default"),
    ("model.row.2", "make the third listed model the default"),
    ("prov.test", "test the connection with the draft's values"),
    ("prov.save", "save the draft (profile/llm/upsert)"),
    ("prov.cancel", "discard the editor and return to Settings"),
    ("prov.retry", "re-test after a rejection, with the draft kept"),
    ("prov.back", "step back from the rejected editor"),
];

/// The ids [`resolve`] routes.
pub const ROUTED: &[&str] = &[
    "prov.name",
    "prov.url",
    "prov.key",
    "key.eye",
    "model.row.0",
    "model.row.1",
    "model.row.2",
    "prov.test",
    "prov.save",
    "prov.cancel",
    "prov.retry",
    "prov.back",
];

pub fn is_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id)
}

pub fn is_routed(id: &str) -> bool {
    ROUTED.contains(&id)
}

/// Declared but unrouted (the coverage contract every screen set carries).
pub fn unrouted() -> Vec<&'static str> {
    ACTIONS
        .iter()
        .map(|(a, _)| *a)
        .filter(|a| !ROUTED.contains(a))
        .collect()
}

/// What an action means.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Live text for a field.
    Input { field: &'static str, value: String },
    /// Make the model row at this index the default.
    SelectModel(usize),
    /// Show/hide the masked key.
    ToggleKey,
    /// Test the connection: `llm/test` with the draft's values AND its key.
    Test,
    /// Save the draft: `llm/upsert`.
    Save,
    /// Leave the editor (no transport).
    Close,
    /// A redacted failure the caller just received.
    Failed { reason: String },
    /// Unhandled id.
    Unhandled,
}

/// Route one action id to its effect. `value` carries an `input` payload.
pub fn resolve(id: &str, value: Option<&str>) -> Effect {
    match id {
        "prov.name" => Effect::Input {
            field: "prov.family",
            value: value.unwrap_or_default().to_owned(),
        },
        "prov.url" => Effect::Input {
            field: "prov.url",
            value: value.unwrap_or_default().to_owned(),
        },
        "prov.key" => Effect::Input {
            field: "prov.key",
            value: value.unwrap_or_default().to_owned(),
        },
        "key.eye" => Effect::ToggleKey,
        "model.row.0" => Effect::SelectModel(0),
        "model.row.1" => Effect::SelectModel(1),
        "model.row.2" => Effect::SelectModel(2),
        "prov.test" => Effect::Test,
        "prov.save" => Effect::Save,
        "prov.cancel" | "prov.retry" | "prov.back" => Effect::Close,
        _ => Effect::Unhandled,
    }
}

/// Apply one effect to the draft. Returns the transport effect to perform.
pub fn apply(ui: &mut ProviderUi, effect: Effect) -> Option<Effect> {
    match effect {
        Effect::Input { field, value } => {
            match field {
                "prov.family" => ui.family = value,
                "prov.url" => ui.base_url = value,
                "prov.key" => ui.key = value,
                _ => {}
            }
            None
        }
        Effect::SelectModel(i) => {
            if let Some(m) = ui.models.get(i).cloned() {
                // The row's label carries " (default)"; the value is the id.
                let id = m.split(" (").next().unwrap_or(&m).to_owned();
                ui.default_model = Some(id.clone());
                // Exactly one row carries the marker: drop it everywhere, then
                // put it on the picked row.
                for row in ui.models.iter_mut() {
                    if let Some(base) = row.strip_suffix(" (default)") {
                        row.truncate(base.len());
                    }
                }
                if let Some(cur) = ui.models.get_mut(i) {
                    cur.push_str(" (default)");
                }
            }
            None
        }
        Effect::ToggleKey => None,
        Effect::Failed { reason } => {
            ui.reject(&reason);
            None
        }
        transport @ (Effect::Test | Effect::Save) => Some(transport),
        Effect::Close => Some(Effect::Close),
        Effect::Unhandled => None,
    }
}

/// The live copy overrides for one editor card.
///
/// The key is deliberately absent: `prov_key` is an *input* whose text the
/// renderer draws as dots, and a rejected draft's key never becomes a copy id.
pub fn copies(screen: Screen, ui: &ProviderUi) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut push = |id: &str, v: &str| out.push((id.to_owned(), v.to_owned()));
    push("prov_name", &ui.family);
    push("prov_url", &ui.base_url);
    for (i, m) in ui.models.iter().enumerate().take(3) {
        push(&format!("t_model_{i}"), m);
    }
    if screen == Screen::Rejected {
        let err = ui
            .error
            .as_deref()
            .map(|_| TEST_FAILED)
            .unwrap_or(TEST_FAILED);
        push("t_cal1", err);
    }
    out
}

/// Lower one editor card to the module's DSL (the `connect::lower_screen`
/// chain, pointed at the phase4 board).
pub fn lower_screen(screen: Screen, ui: &ProviderUi) -> Result<String, String> {
    let dir = crate::design::dir("stage-b/phase4/cards").join(screen.card_dir());
    let card_src = std::fs::read_to_string(dir.join("page.card"))
        .map_err(|e| format!("read {}: {e}", dir.join("page.card").display()))?;
    let data: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("page.data.json"))
            .map_err(|e| format!("read page.data.json: {e}"))?,
    )
    .map_err(|e| format!("parse page.data.json: {e}"))?;
    let card_src = crate::l0_host::apply_copies(&card_src, &copies(screen, ui));
    let prepared = octoscript_makepad::l0::prepare(&card_src, &data, &dir.join("kit"))
        .map_err(|e| format!("l0::prepare: {e}"))?;
    let mut tree = prepared.tree;
    octoscript_makepad::l0::inspectable(&mut tree);
    let dsl = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(&tree))
        .map_err(|e| format!("to_makepad_ui: {e}"))?;
    // #35b item 1: the ONE card-tap wiring, keyed by the card DIRECTORY.
    Ok(super::taps::wire_card_events_dir(&dsl, &dir))
}

/// The bindings this screen projects.
pub fn query(ui: &ProviderUi, id: &str) -> Option<Value> {
    match id {
        "prov.family" => Some(Value::String(ui.family.clone())),
        "prov.url" => Some(Value::String(ui.base_url.clone())),
        "prov.model" => Some(Value::String(ui.model.clone())),
        "prov.route" => Some(Value::String(ui.route.clone())),
        "prov.models" => Some(Value::Array(
            ui.models.iter().cloned().map(Value::String).collect(),
        )),
        // The KEY ITSELF is never a binding: a binding is readable by anything
        // holding the UI state, and the spec asserts the credential is nowhere
        // in the page. Only its presence/mask is projected.
        "prov.key_masked" => Some(Value::String(ui.key_display().to_owned())),
        "prov.key_present" => Some(Value::Bool(!ui.key.is_empty())),
        "prov.error" => ui.error.clone().map(Value::String),
        "prov.rejected" => Some(Value::Bool(ui.key_rejected)),
        "prov.default_model" => ui.default_model.clone().map(Value::String),
        _ => None,
    }
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
        apply(&mut ui, Effect::Failed {
            reason: "401 unauthorized".into(),
        });
        assert_eq!(ui.family, "deepseek");
        assert_eq!(ui.model, "deepseek-chat");
        assert_eq!(ui.route, "openrouter");
        assert_eq!(ui.key, SECRET);
        assert_eq!(ui.screen, Screen::Rejected);
    }

    #[test]
    fn the_key_never_reaches_the_copies() {
        let mut ui = ProviderUi::new("deepseek");
        ui.key = SECRET.to_owned();
        ui.reject("boom");
        for (_, v) in copies(Screen::Rejected, &ui) {
            assert!(!v.contains(SECRET), "the key leaked into a copy: {v:?}");
        }
        assert_eq!(ui.key_display(), DOTS);
    }

    #[test]
    fn the_key_never_reaches_the_bindings() {
        let mut ui = ProviderUi::new("deepseek");
        ui.key = SECRET.to_owned();
        ui.reject("boom");
        for id in [
            "prov.family",
            "prov.url",
            "prov.model",
            "prov.route",
            "prov.models",
            "prov.key_masked",
            "prov.key_present",
            "prov.error",
            "prov.rejected",
            "prov.default_model",
        ] {
            if let Some(v) = query(&ui, id) {
                assert!(
                    !v.to_string().contains(SECRET),
                    "the key leaked into binding {id}: {v}"
                );
            }
        }
    }

    #[test]
    fn redaction_removes_the_secret_raw_and_encoded() {
        // model-settings.ts:531-538
        let r = redact(&format!("401 for {SECRET}"), SECRET);
        assert!(!r.contains(SECRET), "{r}");
        assert!(r.contains("[redacted]"));
        let enc = percent_encode(SECRET);
        let r2 = redact(&format!("401 for {enc}"), SECRET);
        assert!(!r2.contains(&enc), "{r2}");
    }

    #[test]
    fn redaction_removes_key_and_bearer_runs() {
        // model-settings.ts:540-541
        let r = redact("api_key=abcd1234 and Bearer tok_live_9", "");
        assert!(!r.contains("abcd1234"), "{r}");
        assert!(!r.contains("tok_live_9"), "{r}");
    }

    #[test]
    fn the_rejected_message_is_bounded() {
        // model-settings.ts:542
        let r = redact(&"x".repeat(5_000), "");
        assert_eq!(r.chars().count(), MAX_REDACTED);
    }

    #[test]
    fn a_passed_test_clears_the_rejection() {
        let mut ui = ProviderUi::new("deepseek");
        ui.reject("401");
        ui.accept();
        assert!(ui.error.is_none());
        assert!(!ui.key_rejected);
        assert_eq!(ui.screen, Screen::Editor);
    }

    #[test]
    fn every_declared_action_is_routed() {
        assert_eq!(unrouted(), Vec::<&str>::new());
    }
}
