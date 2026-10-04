//! Display preferences: one theme choice, language and Vim editing.
//!
//! System/Light/Dark and named themes share one selector and persisted `theme`
//! value. Changes apply immediately; Save writes the four-key version-2 document.
//! Legacy version-1 palette documents and the separate appearance file are read
//! once at startup. Named palettes win; Terminal adopts the old appearance.
//! The storage path stays display-v1.json so existing installations migrate in place.
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::Value;

/// The mutually exclusive choices in the unified Theme selector.
pub const DISPLAY_THEMES: [&str; 7] = ["system", "light", "dark", "codex", "claude", "slate", "solarized"];

/// The saved / current display preferences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayPrefs {
    pub theme: String,
    pub language: String,
    pub vim_mode: bool,
}

/// `SAVE_ERROR` (model.ts:24), native wording.
pub const SAVE_ERROR: &str = "Preferences could not be saved.";

/// Defaults: follow the system appearance, device language, Vim off.
pub fn defaults() -> DisplayPrefs {
    defaults_for(&crate::i18n::device_locale())
}

/// The defaults for a device locale (`/^zh(?:-|_|$)/i` -> zh, model.ts:104).
pub fn defaults_for(locale: &str) -> DisplayPrefs {
    let language = crate::i18n::Lang::for_locale(locale).code().to_owned();
    DisplayPrefs { theme: "system".into(), language, vim_mode: false }
}

/// `parseDisplayPreferences` (model.ts:35-62): exactly the four keys,
/// versions 1/2, a known theme and language, a boolean vimMode; else `None`.
pub fn parse(raw: &str) -> Option<DisplayPrefs> {
    let v: Value = serde_json::from_str(raw).ok()?;
    let o = v.as_object()?;
    if o.len() != 4 || !matches!(o.get("version")?.as_u64(), Some(1 | 2)) {
        return None;
    }
    let theme = o.get("theme")?.as_str()?;
    let language = o.get("language")?.as_str()?;
    let vim = o.get("vimMode")?.as_bool()?;
    let legacy_terminal = o.get("version")? == &Value::from(1) && theme == "terminal";
    if (!DISPLAY_THEMES.contains(&theme) && !legacy_terminal) || !matches!(language, "en" | "zh") {
        return None;
    }
    Some(DisplayPrefs { theme: theme.to_owned(), language: language.to_owned(), vim_mode: vim })
}

/// The document written on Save (only the whitelist).
pub fn document(p: &DisplayPrefs) -> String {
    serde_json::json!({
        "version": 2,
        "theme": p.theme,
        "language": p.language,
        "vimMode": p.vim_mode,
    })
    .to_string()
}

pub fn path() -> PathBuf {
    if let Ok(p) = std::env::var("OCTOSCODE_DISPLAY_PREFS_PATH") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".octoscode").join("display-v1.json")
}

/// The section's state (`PreferencesSnapshot`: the values, dirty, error).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prefs {
    pub saved: DisplayPrefs,
    pub current: DisplayPrefs,
    pub error: Option<&'static str>,
    /// The last Save succeeded (the "saved" status line).
    pub just_saved: bool,
}

impl Prefs {
    pub fn dirty(&self) -> bool {
        self.current != self.saved
    }

    /// The status line (`PreferencesDialog.tsx:89-95`).
    pub fn status(&self) -> &'static str {
        if let Some(e) = self.error {
            e
        } else if self.dirty() {
            "Unsaved preferences."
        } else if self.just_saved {
            "Preferences saved."
        } else {
            ""
        }
    }
}

static PREFS: Mutex<Option<Prefs>> = Mutex::new(None);

fn lock() -> std::sync::MutexGuard<'static, Option<Prefs>> {
    PREFS.lock().unwrap_or_else(|p| p.into_inner())
}

/// Read the stored whitelist (tolerant: absent, unreadable or rejected
/// documents read as the defaults — denied storage never blocks the app).
pub fn load_from(path: &std::path::Path) -> DisplayPrefs {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| parse(&raw))
        .unwrap_or_else(defaults)
}

/// Merge the old palette + appearance without losing language or Vim settings.
pub fn migrate(loaded: Option<DisplayPrefs>, legacy: Option<crate::screens::theme::Theme>) -> DisplayPrefs {
    let needs_appearance = loaded.as_ref().is_none_or(|p| p.theme == "terminal");
    let mut prefs = loaded.unwrap_or_else(defaults);
    if needs_appearance {
        prefs.theme = legacy.and_then(|t| t.stored()).unwrap_or("system").to_owned();
    }
    prefs
}

/// Load once (the launch path) and return the current preferences.
pub fn init() -> DisplayPrefs {
    let mut g = lock();
    if g.is_none() {
        let loaded = std::fs::read_to_string(path()).ok().and_then(|raw| parse(&raw));
        let saved = migrate(loaded, crate::screens::theme::load_preference());
        *g = Some(Prefs { saved: saved.clone(), current: saved, error: None, just_saved: false });
    }
    g.as_ref().map(|p| p.current.clone()).unwrap_or_else(defaults)
}

pub fn snapshot() -> Prefs {
    init();
    lock().clone().expect("initialized")
}

/// Test seam.
pub fn reset() {
    *lock() = None;
}

/// Vim editing changed (the toggle, or `/vimmode`): applies now, unsaved.
pub fn set_vim(on: bool) {
    init();
    if let Some(p) = lock().as_mut() {
        if p.current.vim_mode != on {
            p.current.vim_mode = on;
            p.just_saved = false;
            p.error = None;
        }
    }
}

/// The unified theme changed (Settings > Preferences): applies now, unsaved.
pub fn set_palette(id: &str) -> bool {
    if !DISPLAY_THEMES.contains(&id) {
        return false;
    }
    init();
    if let Some(p) = lock().as_mut() {
        if p.current.theme != id {
            p.current.theme = id.to_owned();
            p.just_saved = false;
            p.error = None;
        }
    }
    true
}

/// A24 — the language changed (`setLanguage`, model.ts:128-130): applies at
/// once — `tr()` follows it and the host re-renders every surface — and
/// stays unsaved until Save. Returns whether the language in effect changed.
pub fn set_language(lang: crate::i18n::Lang) -> bool {
    init();
    if let Some(p) = lock().as_mut() {
        if p.current.language != lang.code() {
            p.current.language = lang.code().to_owned();
            p.just_saved = false;
            p.error = None;
        }
    }
    crate::i18n::set_language(lang)
}

/// A24 — the launch path: publish the stored (or device-default) language
/// before anything lowers, so a Chinese preference's first frame is Chinese.
/// Returns the language adopted.
pub fn adopt_language() -> crate::i18n::Lang {
    let lang = launch_language(&init());
    crate::i18n::set_language(lang);
    lang
}

/// The language a launch adopts from the loaded preferences (the stored
/// whitelist, else the device default).
pub fn launch_language(p: &DisplayPrefs) -> crate::i18n::Lang {
    crate::i18n::Lang::parse(&p.language).unwrap_or(crate::i18n::Lang::En)
}

/// Save the whitelist (`DisplayPreferencesStore.save`, model.ts:144-162).
/// `true` on success; a failed write keeps the choice and reports.
pub fn save_to(path: &std::path::Path) -> bool {
    init();
    let mut g = lock();
    let Some(p) = g.as_mut() else { return false };
    let doc = document(&p.current);
    let ok = (|| -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, doc)
    })()
    .is_ok();
    if ok {
        p.saved = p.current.clone();
        p.error = None;
        p.just_saved = true;
    } else {
        p.error = Some(SAVE_ERROR);
        p.just_saved = false;
    }
    ok
}

pub fn save() -> bool {
    save_to(&path())
}

pub const ACTION_VIM: &str = "a9.prefs.vim";
pub const ACTION_SAVE: &str = "a9.prefs.save";
/// A24 — the Language control's two segments.
pub const ACTION_LANG_EN: &str = "a9.prefs.lang.en";
pub const ACTION_LANG_ZH: &str = "a9.prefs.lang.zh";

/// A26 — `a9.prefs.palette.<id>`: one per palette row.
pub const ACTION_PALETTE: &str = "a9.prefs.palette.";
/// Theme row action ids, in `DISPLAY_THEMES` order.
pub const PALETTE_ACTIONS: [&str; 7] = [
    "a9.prefs.palette.system",
    "a9.prefs.palette.light",
    "a9.prefs.palette.dark",
    "a9.prefs.palette.codex",
    "a9.prefs.palette.claude",
    "a9.prefs.palette.slate",
    "a9.prefs.palette.solarized",
];

/// A26 — a palette row's click (`a9.prefs.palette.<id>`), the production
/// path the host's arm runs: the whitelist's `theme` changes (unsaved, as
/// the web's `setTheme`) and the palette applies. `Some(palette)` when the
/// look in effect changed (the host then re-themes the app), `None` for an
/// unknown id or the palette already in effect.
pub fn choose_palette(action: &str) -> Option<crate::screens::theme::Palette> {
    use crate::screens::theme;
    let id = palette_of(action)?;
    // Keep old recorded actions usable while the product offers one selector.
    let choice = if id == "terminal" { theme::preference() } else { id.to_owned() };
    let before = theme::selection();
    set_palette(&choice);
    theme::select(&choice);
    (before != choice).then(theme::palette)
}

/// The palette a palette action names (`a9.prefs.palette.codex` -> codex).
pub fn palette_of(action: &str) -> Option<&'static str> {
    let id = action.strip_prefix(ACTION_PALETTE)?;
    if id == "terminal" { return Some("terminal"); }
    DISPLAY_THEMES.into_iter().find(|t| *t == id)
}

pub fn routes(action: &str) -> bool {
    matches!(action, ACTION_VIM | ACTION_SAVE | ACTION_LANG_EN | ACTION_LANG_ZH) || palette_of(action).is_some()
}

/// The language a Language-control action selects.
pub fn language_of(action: &str) -> Option<crate::i18n::Lang> {
    match action {
        ACTION_LANG_EN => Some(crate::i18n::Lang::En),
        ACTION_LANG_ZH => Some(crate::i18n::Lang::Zh),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> std::sync::MutexGuard<'static, ()> {
        crate::screens::theme::test_lock()
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("a9-prefs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d.join("display-v1.json")
    }

    #[test]
    fn only_the_four_key_whitelist_parses() {
        assert_eq!(
            parse(r#"{"version":1,"theme":"slate","language":"zh","vimMode":true}"#),
            Some(DisplayPrefs { theme: "slate".into(), language: "zh".into(), vim_mode: true })
        );
        // An extra field (a credential that slipped in) rejects the document.
        assert_eq!(parse(r#"{"version":1,"theme":"slate","language":"en","vimMode":false,"token":"x"}"#), None);
        assert_eq!(parse(r#"{"version":3,"theme":"slate","language":"en","vimMode":false}"#), None);
        assert_eq!(parse(r#"{"version":1,"theme":"neon","language":"en","vimMode":false}"#), None);
        assert_eq!(parse(r#"{"version":1,"theme":"slate","language":"fr","vimMode":false}"#), None);
        assert_eq!(parse(r#"{"version":1,"theme":"slate","language":"en","vimMode":"yes"}"#), None);
        assert_eq!(parse("{not json"), None);
        assert_eq!(parse("[1,2]"), None);
    }

    #[test]
    fn save_writes_only_the_whitelist_and_a_fresh_load_adopts_it() {
        let _g = guard();
        reset();
        let p = tmp("save");
        set_vim(true);
        assert!(snapshot().dirty());
        assert_eq!(snapshot().status(), "Unsaved preferences.");
        assert!(save_to(&p));
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(doc.as_object().unwrap().len(), 4, "only the whitelist");
        assert_eq!(doc["version"], 2);
        assert_eq!(doc["vimMode"], true);
        assert_eq!(snapshot().status(), "Preferences saved.");
        assert!(load_from(&p).vim_mode, "the next launch adopts it");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
        reset();
    }

    /// A24 — the Language control: applies at once (`tr()` follows), stays
    /// unsaved until Save, Save writes it in the whitelist, the next launch
    /// adopts it.
    #[test]
    fn the_language_applies_at_once_saves_and_is_adopted_at_launch() {
        use crate::i18n::{self, Lang};
        let _g = guard();
        reset();
        i18n::set_language(Lang::En);
        let p = tmp("lang");
        let gen = i18n::generation();
        assert!(set_language(Lang::Zh));
        assert_eq!(snapshot().current.language, "zh");
        assert!(snapshot().dirty(), "unsaved until Save");
        assert_eq!(i18n::language(), Lang::Zh, "applied at once");
        assert!(i18n::generation() > gen, "the host re-renders on the bump");
        assert_eq!(i18n::tr("Language"), "语言");
        assert!(save_to(&p));
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(doc["language"], "zh");
        assert_eq!(doc.as_object().unwrap().len(), 4, "only the whitelist");
        // The next launch: the stored language, before anything lowers.
        assert_eq!(launch_language(&load_from(&p)), Lang::Zh);
        assert_eq!(language_of(ACTION_LANG_EN), Some(Lang::En));
        assert_eq!(language_of(ACTION_LANG_ZH), Some(Lang::Zh));
        assert!(routes(ACTION_LANG_ZH) && routes(ACTION_LANG_EN));
        assert!(set_language(Lang::En));
        assert_eq!(i18n::tr("Language"), "Language");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
        reset();
    }

    /// A24 — a fresh profile's language is the device's (model.ts:102-107):
    /// a `zh*` locale is Chinese, anything else English.
    #[test]
    fn the_default_language_follows_the_device_locale() {
        assert_eq!(defaults_for("zh-Hans-CN").language, "zh");
        assert_eq!(defaults_for("zh_TW.UTF-8").language, "zh");
        assert_eq!(defaults_for("en-US").language, "en");
        assert_eq!(defaults_for("").language, "en");
        assert_eq!(defaults(), defaults_for(&crate::i18n::device_locale()));
        assert_eq!(launch_language(&defaults_for("zh-CN")), crate::i18n::Lang::Zh);
        // An absent / rejected store falls back to the device default.
        let missing = tmp("absent");
        assert_eq!(load_from(&missing).language, defaults().language);
    }

    #[test]
    fn denied_storage_never_blocks_and_says_so() {
        let _g = guard();
        reset();
        // A path under a FILE cannot be created: the write fails.
        let blocker = std::env::temp_dir().join(format!("a9-prefs-blocker-{}", std::process::id()));
        std::fs::write(&blocker, "x").unwrap();
        let p = blocker.join("display-v1.json");
        set_vim(true);
        assert!(!save_to(&p));
        let s = snapshot();
        assert!(s.current.vim_mode, "the choice stays in effect");
        assert_eq!(s.status(), SAVE_ERROR);
        // An unreadable / rejected store reads as the defaults.
        assert_eq!(load_from(&p), defaults());
        let _ = std::fs::remove_file(&blocker);
        reset();
    }
}
