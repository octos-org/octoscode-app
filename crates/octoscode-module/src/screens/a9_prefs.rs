//! A9 — display preferences (the web's "Browser preferences",
//! `features/preferences/model.ts` + `PreferencesDialog.tsx`), natively a
//! Settings section.
//!
//! The contract kept from the web (`model.ts:1-181`):
//! - ONLY the display whitelist is stored, as `{version: 1, theme, language,
//!   vimMode}` — exactly those four keys; a document with any other field
//!   (e.g. an accidentally supplied credential), another version, an unknown
//!   palette/language or a non-boolean vimMode is rejected and the defaults
//!   apply (`parseDisplayPreferences`, :35-62).
//! - Changes apply at once; only **Save** writes them; a store that cannot
//!   be read or written never blocks the app — the choice stays in effect and
//!   the section says "Preferences could not be saved." (`SAVE_ERROR`).
//! - Defaults: palette `terminal`, language from the device (`zh*` -> zh),
//!   vimMode off (:102-107).
//!
//! The file is the native counterpart of `octoscode.web.display.v1`:
//! `$HOME/.octoscode/display-v1.json` (`OCTOSCODE_DISPLAY_PREFS_PATH`
//! overrides). The System/Light/Dark appearance stays where it was
//! (`screens::theme`, the web's separate `dsw-theme` key).
//!
//! What the section offers natively: **Vim editing** (the composer's Vim
//! subset, `screens::board3::vim`), applied at once and restored at launch,
//! and (A24) the **Language** — English / 简体中文, the web dialog's first
//! field (`PreferencesDialog.tsx:42-55`): a choice re-renders every surface
//! at once through `crate::i18n` (the web's catalog), unsaved until Save, and
//! the saved choice is published before the first frame
//! ([`adopt_language`]). The palette is carried through the whitelist
//! unchanged (no native palette set — not offered, so nothing shown is a
//! dead switch).
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::Value;

/// The five named palettes (`DISPLAY_THEMES`, model.ts:2-8).
pub const DISPLAY_THEMES: [&str; 5] = ["terminal", "codex", "claude", "slate", "solarized"];

/// The saved / current display preferences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayPrefs {
    pub theme: String,
    pub language: String,
    pub vim_mode: bool,
}

/// `SAVE_ERROR` (model.ts:24), native wording.
pub const SAVE_ERROR: &str = "Preferences could not be saved.";

/// The defaults (`model.ts:102-107`): terminal, the device language, Vim off.
pub fn defaults() -> DisplayPrefs {
    defaults_for(&crate::i18n::device_locale())
}

/// The defaults for a device locale (`/^zh(?:-|_|$)/i` -> zh, model.ts:104).
pub fn defaults_for(locale: &str) -> DisplayPrefs {
    let language = crate::i18n::Lang::for_locale(locale).code().to_owned();
    DisplayPrefs { theme: "terminal".into(), language, vim_mode: false }
}

/// `parseDisplayPreferences` (model.ts:35-62): exactly the four keys,
/// version 1, a known palette and language, a boolean vimMode; else `None`.
pub fn parse(raw: &str) -> Option<DisplayPrefs> {
    let v: Value = serde_json::from_str(raw).ok()?;
    let o = v.as_object()?;
    if o.len() != 4 || o.get("version")? != &Value::from(1) {
        return None;
    }
    let theme = o.get("theme")?.as_str()?;
    let language = o.get("language")?.as_str()?;
    let vim = o.get("vimMode")?.as_bool()?;
    if !DISPLAY_THEMES.contains(&theme) || !matches!(language, "en" | "zh") {
        return None;
    }
    Some(DisplayPrefs { theme: theme.to_owned(), language: language.to_owned(), vim_mode: vim })
}

/// The document written on Save (only the whitelist).
pub fn document(p: &DisplayPrefs) -> String {
    serde_json::json!({
        "version": 1,
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

/// Load once (the launch path) and return the current preferences.
pub fn init() -> DisplayPrefs {
    let mut g = lock();
    if g.is_none() {
        let saved = load_from(&path());
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
    let lang = crate::i18n::Lang::parse(&init().language).unwrap_or(crate::i18n::Lang::En);
    crate::i18n::set_language(lang);
    lang
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

pub fn routes(action: &str) -> bool {
    matches!(action, ACTION_VIM | ACTION_SAVE | ACTION_LANG_EN | ACTION_LANG_ZH)
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
        assert_eq!(parse(r#"{"version":2,"theme":"slate","language":"en","vimMode":false}"#), None);
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
        assert_eq!(doc["version"], 1);
        assert_eq!(doc["vimMode"], true);
        assert_eq!(snapshot().status(), "Preferences saved.");
        assert!(load_from(&p).vim_mode, "the next launch adopts it");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
        reset();
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
