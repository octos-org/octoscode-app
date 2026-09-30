//! Stage C wiring — board 3.11 (Dark conversation) & 3.12 (Dark settings): the
//! app-wide theme preference (#30e). One submodule, one owner of the `theme.*`
//! ids (the one-owner rule, LESSONS).
//!
//! # The preference (the web's contract, cited)
//!
//! The sidebar footer's theme button cycles **system → dark → light → system**
//! (`ProductSidebar.tsx:984-1010` renders the Dark/Light/System label and calls
//! `onThemeToggle`; `App.tsx:2343` wires it to `cycleTheme`; the cycle order is
//! `use-theme.ts:44-49`). Only `light`/`dark` persist — `system` means the
//! stored key is ABSENT (`use-theme.ts:9-27`: `removeItem` for system, parse
//! falls back to system). The native preference lives in this module's static
//! (the web's key `octoscode.web.display.v1` is browser-localStorage per the
//! parity matrix row `preferences,g-timeline` `model.ts:1`; the app-side
//! `user_store.rs` belongs to the appcard layer, outside this card's Files).
//!
//! # Applying it app-wide
//!
//! The two board-3.11/3.12 Stage B cards ARE the dark token set: their kit
//! declares `"theme": "light"` (the generator's only value) but the token
//! VALUES are the dark palette — autonomy-11 bg `#1C1C1E`/`#2C2C2E`, text
//! `#FCFCFC`; autonomy-12 bg `#191C1F`/`#1D1F23`. Their light twins are the
//! same screens at Stage B light: setup-06 (General settings — texts identical
//! to autonomy-12's) and conversation-03 (Streaming turn — the conversation
//! structure s11 mirrors). So "the same components with the dark token set"
//! resolves to CARD SELECTION: dark mounts 11/12, light mounts the twins.
//! Components that ship only a light kit keep rendering light (nothing breaks);
//! their dark kits land per-component later — disclosed in docs/cards/30e.md.
//!
//! `system` resolves DARK: the makepad fork exposes no OS-appearance API
//! (searched `octoscript-makepad-fork/crates/` for dark_mode/OsTheme/appearance
//! — no hits), so until the platform surfaces an appearance event the shell's
//! drawn default is dark. The resolution is deterministic and tested; the OS
//! read itself is **unverified on platform** and disclosed in docs/cards/30e.md.

use std::sync::{Mutex, OnceLock};

use octoscode_store::Store;
use serde_json::Value;

/// The dark cards (board 3.11/3.12 Stage B).
pub const DARK_CONV_CARD: &str = "autonomy-11";
pub const DARK_SETTINGS_CARD: &str = "autonomy-12";
/// The light twins (the same screens with the light token set).
pub const LIGHT_CONV_CARD: &str = "conversation-03";
pub const LIGHT_SETTINGS_CARD: &str = "setup-06";

/// The stored preference (`ThemePreference`, `use-theme.ts:4`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    System,
    Dark,
    Light,
}

impl Theme {
    /// The sidebar label (`ProductSidebar.tsx:998-1005`: "Dark" | "Light" |
    /// "System").
    pub fn label(self) -> &'static str {
        match self {
            Theme::System => "System",
            Theme::Dark => "Dark",
            Theme::Light => "Light",
        }
    }

    /// What persistence holds: only light/dark; system = the key is absent
    /// (`use-theme.ts:20-22`).
    pub fn stored(self) -> Option<&'static str> {
        match self {
            Theme::Light => Some("light"),
            Theme::Dark => Some("dark"),
            Theme::System => None,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Theme::System),
            "dark" => Some(Theme::Dark),
            "light" => Some(Theme::Light),
            _ => None,
        }
    }

    /// The resolved palette for rendering. System falls back to dark (doc
    /// comment above — no OS-appearance API in the fork; disclosed).
    pub fn resolved(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark | Theme::System => "dark",
        }
    }
}

/// Screen-local theme state (UI state the protocol never carries — the same
/// category as `FlowUi`; a static behind one lock keeps the lib.rs edit to the
/// registration lines, which the entry asks for).
pub struct ThemeUi {
    pub pref: Theme,
}

impl Default for ThemeUi {
    fn default() -> Self {
        // The web's fresh profile reads system (`use-theme.ts:12`).
        Self { pref: Theme::System }
    }
}

static THEME: OnceLock<Mutex<ThemeUi>> = OnceLock::new();

fn theme() -> &'static Mutex<ThemeUi> {
    THEME.get_or_init(|| Mutex::new(ThemeUi::default()))
}

/// The current preference string ("system" | "dark" | "light").
pub fn preference() -> String {
    let t = theme().lock().unwrap();
    t.pref.stored().unwrap_or("system").to_owned()
}

/// The resolved palette the mounted screens use ("dark" | "light").
pub fn resolved() -> &'static str {
    theme().lock().unwrap().pref.resolved()
}

/// `theme.set` / `OCTOSCODE_THEME`: set the preference from its stored string.
/// Unknown values are rejected (`use-theme.ts:11-13` only accepts
/// light/dark, else system — here the caller keeps the current preference).
pub fn set_preference(value: &str) -> bool {
    match Theme::parse(value) {
        Some(pref) => {
            theme().lock().unwrap().pref = pref;
            true
        }
        None => false,
    }
}

/// `theme.cycle` — system → dark → light → system (`use-theme.ts:44-49`).
/// Returns (preference, resolved) AFTER the cycle.
pub fn cycle() -> (String, &'static str) {
    let mut t = theme().lock().unwrap();
    t.pref = match t.pref {
        Theme::System => Theme::Dark,
        Theme::Dark => Theme::Light,
        Theme::Light => Theme::System,
    };
    (t.pref.stored().unwrap_or("system").to_owned(), t.pref.resolved())
}

/// Test seam: reset to the fresh-profile default.
pub fn reset_state() {
    *theme().lock().unwrap() = ThemeUi::default();
}

// ---- id ownership (one owner per id, LESSONS) --------------------------------

/// The binding ids this screen owns (`theme.preference`, `theme.resolved`).
pub fn owns_binding(id: &str) -> bool {
    id.starts_with("theme.")
}

/// The action ids this screen owns. `theme.cycle` is the sidebar's single
/// control (`ProductSidebar.tsx:992` one button, no arguments).
pub fn is_action(id: &str) -> bool {
    matches!(id, "theme.cycle")
}

/// Resolve one of this module's action ids.
#[derive(Debug)]
pub enum Effect {
    /// `theme.cycle` — the preference moved; carries the state AFTER the cycle.
    Cycle { preference: String, resolved: &'static str },
    /// A declared id with no resolvable target — logged by name, never fatal.
    Unhandled(String),
}

pub fn resolve(action: &str, _index: usize, _ctx: &crate::bindings::Ctx<'_>) -> Effect {
    match action {
        "theme.cycle" => {
            let (preference, resolved) = cycle();
            Effect::Cycle { preference, resolved }
        }
        other => Effect::Unhandled(other.to_owned()),
    }
}

/// Resolve one of this module's binding ids (JSON only).
pub fn query(_ctx: &crate::bindings::Ctx<'_>, id: &str) -> Option<Value> {
    match id {
        "theme.preference" => Some(Value::String(preference())),
        "theme.resolved" => Some(Value::String(resolved().to_owned())),
        _ => None,
    }
}

// ---- card selection + lowering ------------------------------------------------

/// The screen names this module mounts, and the card the CURRENT resolved
/// theme selects. `dark_conv`/`dark_settings` pin the dark card explicitly
/// (tests/captures); `conversation`/`settings` follow the preference.
pub fn card_for(which: &str) -> Option<(&'static str, std::path::PathBuf)> {
    fn root(stage: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../design/stage-b/{stage}/cards"))
    }
    let dark = resolved() == "dark";
    match which {
        "dark_conv" => Some((DARK_CONV_CARD, root("autonomy"))),
        "dark_settings" => Some((DARK_SETTINGS_CARD, root("autonomy"))),
        "light_conv" => Some((LIGHT_CONV_CARD, root("conversation"))),
        "light_settings" => Some((LIGHT_SETTINGS_CARD, root("setup"))),
        "conversation" if dark => Some((DARK_CONV_CARD, root("autonomy"))),
        "conversation" => Some((LIGHT_CONV_CARD, root("conversation"))),
        "settings" if dark => Some((DARK_SETTINGS_CARD, root("autonomy"))),
        "settings" => Some((LIGHT_SETTINGS_CARD, root("setup"))),
        _ => None,
    }
}

/// Lower one theme-wired screen through the SAME path `palette::lower_screen`
/// uses (L0 `prepare`, then the DESIGN branch — the artifact Gate B rendered;
/// these screens are fixed chrome, inside the RULES 8.10 carve-out). No live
/// slots: the cards carry no runtime prose.
pub fn lower(which: &str, _store: &std::sync::Arc<Store>) -> Result<String, String> {
    let (card, root) = card_for(which)
        .ok_or_else(|| format!("octoscode: unknown OCTOSCODE_SCREEN {which:?}"))?;
    let dir = root.join(card);
    let card_text = std::fs::read_to_string(dir.join("page.card"))
        .map_err(|e| format!("octoscode: {card}/page.card: {e}"))?;
    let data_text = std::fs::read_to_string(dir.join("page.data.json")).unwrap_or_else(|_| "{}".into());
    let data: Value = serde_json::from_str(&data_text)
        .map_err(|e| format!("octoscode: {card}/page.data.json: {e}"))?;
    let prepared = octoscript_makepad::l0::prepare(&card_text, &data, &dir.join("kit"))?;
    octoscript_makepad::design::to_makepad_ui(&prepared.tree)
}

/// Mount one theme-wired card into a splash slot (the `palette::mount_screen`
/// shape): the LIVE theme preference selects the card via [`card_for`], so a
/// `theme.cycle` flip changes what the NEXT mount lowers.
pub fn mount(
    cache: &mut crate::mount::MountCache,
    cx: &mut crate::makepad_widgets::Cx,
    splash: crate::makepad_widgets::SplashRef,
    which: &str,
    store: &std::sync::Arc<Store>,
) -> Result<bool, String> {
    let dsl = lower(which, store)?;
    cache.mount(cx, &splash, &dsl)
}
