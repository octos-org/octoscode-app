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
/// `system` consults the injected OS-appearance reader (workflow 3); with no
/// reader set (tests, pre-wire) it falls back to DARK — the #30e deterministic
/// semantics, unchanged.
pub fn resolved() -> &'static str {
    let pref = theme().lock().unwrap().pref;
    match pref {
        Theme::Light => "light",
        Theme::Dark => "dark",
        Theme::System => match OS_READER.read().unwrap().as_ref() {
            Some(read) if read() => "dark",
            Some(_) => "light",
            None => "dark",
        },
    }
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
        crate::design::dir(&format!("stage-b/{stage}/cards"))
    }
    match which {
        "dark_conv" => Some((DARK_CONV_CARD, root("autonomy"))),
        "dark_settings" => Some((DARK_SETTINGS_CARD, root("autonomy"))),
        "light_conv" | "conversation" => Some((LIGHT_CONV_CARD, root("conversation"))),
        "light_settings" | "settings" => Some((LIGHT_SETTINGS_CARD, root("setup"))),
        _ => None,
    }
}

/// Lower one card verbatim through the SAME path `palette::lower_screen` uses
/// (L0 `prepare`, then the DESIGN branch — the artifact Gate B rendered; these
/// screens are fixed chrome, inside the RULES 8.10 carve-out). No live slots:
/// the cards carry no runtime prose.
fn lower_card(card: &str, root: std::path::PathBuf) -> Result<String, String> {
    let dir = root.join(card);
    let card_text = std::fs::read_to_string(dir.join("page.card"))
        .map_err(|e| format!("octoscode: {card}/page.card: {e}"))?;
    let data_text = std::fs::read_to_string(dir.join("page.data.json")).unwrap_or_else(|_| "{}".into());
    let data: Value = serde_json::from_str(&data_text)
        .map_err(|e| format!("octoscode: {card}/page.data.json: {e}"))?;
    let prepared = octoscript_makepad::l0::prepare(&card_text, &data, &dir.join("kit"))?;
    let dsl = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(&prepared.tree))?;
    // #35b item 1: the same card-dir-driven wiring the connect/palette paths
    // get, resolved against THIS card's own stage root (the theme cards live
    // in stage-b/{conversation,autonomy,setup}/cards, not only stage-b/setup).
    Ok(crate::screens::taps::wire_card_events_dir(&dsl, &dir))
}

/// Lower one theme-wired screen to the LIVE palette (#31d workflow 1: the
/// token path, NOT card swapping). The explicit `dark_*`/`light_*` names pin
/// their atlas card VERBATIM (Gate-B renders / captures / tests); the follow
/// names lower the LIGHT twin — the palette every kit ships — and
/// [`retint_dsl`] rewrites it to the resolved palette (light resolves to the
/// byte-identical passthrough, dark to the token set).
pub fn lower(which: &str, store: &std::sync::Arc<Store>) -> Result<String, String> {
    match which {
        "conversation" => Ok(retint_dsl(&lower_card(LIGHT_CONV_CARD, conv_root())?)),
        "settings" => Ok(retint_dsl(&lower_card(LIGHT_SETTINGS_CARD, setup_root())?)),
        other => {
            let (card, root) = card_for(other)
                .ok_or_else(|| format!("octoscode: unknown OCTOSCODE_SCREEN {other:?}"))?;
            lower_card(card, root)
        }
    }
}

fn conv_root() -> std::path::PathBuf {
    stage_root("conversation")
}
fn setup_root() -> std::path::PathBuf {
    stage_root("setup")
}
fn stage_root(stage: &str) -> std::path::PathBuf {
    crate::design::dir(&format!("stage-b/{stage}/cards"))
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

// ---- #31d workflow 1: the component-level token set ---------------------------

/// Light ↔ dark token pairs, read off the Stage B kits (values, not names):
/// light from setup-06/conversation-03 kit.json, dark from autonomy-12/11 —
/// surfaces #1C1F22/#1C1C1E, bubble #2C2C2E, text #F5F5F7/#E8E8EA, secondary
/// #98989D (autonomy-12's own muted token), hairline #38383A, send disc white.
/// Accents (blue #2F6FEB, green #1F883D, red #CF222E, amber) are NOT themed —
/// both palettes share them (the web's accent tokens are theme-invariant).
const TOKENS: &[(&str, &str)] = &[
    // surfaces: app bg, cards, rows
    ("#ffffff", "#1c1f22"),
    ("#fefefe", "#1c1f22"),
    ("#fdfdfd", "#1c1f22"),
    ("#f7f7f8", "#1c1c1e"),
    ("#f6f6f7", "#1c1c1e"),
    ("#e5e5e7", "#38383a"),
    ("#dededf", "#38383a"),
    ("#d2d2d5", "#38383a"),
    // the user bubble's raised fill
    ("#f4f4f5", "#2c2c2e"),
    ("#f0f0f2", "#2c2c2e"),
    // primary text
    ("#1d1d1f", "#f5f5f7"),
    ("#1b1b1b", "#f5f5f7"),
    ("#29292a", "#e8e8ea"),
    ("#313131", "#e8e8ea"),
    ("#353535", "#e8e8ea"),
    ("#373838", "#e8e8ea"),
    ("#000000", "#f5f5f7"),
    // secondary text
    ("#6e6e73", "#98989d"),
    ("#919295", "#98989d"),
    ("#9a9aa0", "#98989d"),
    // the send control's disc: black in light, white in dark (both atlases)
    ("#050505", "#f5f5f7"),
    ("#030202", "#f5f5f7"),
    // #31d re-capture round: the setup-08 (palette) and autonomy-01 (review)
    // kits carry NEAR-BLACK text and mid-grey hints outside the first table —
    // the dark captures' low-contrast command names came from exactly these.
    ("#0a0a0b", "#f5f5f7"),
    ("#0c0c0c", "#f5f5f7"),
    ("#0d0c0d", "#f5f5f7"),
    ("#101010", "#f5f5f7"),
    ("#141616", "#f5f5f7"),
    ("#313031", "#f5f5f7"),
    ("#2f2c2d", "#e8e8ea"),
    ("#383939", "#e8e8ea"),
    ("#3e4040", "#e8e8ea"),
    ("#434342", "#e8e8ea"),
    ("#454446", "#e8e8ea"),
    ("#464746", "#e8e8ea"),
    ("#484a49", "#e8e8ea"),
    ("#524746", "#e8e8ea"),
    ("#696b6f", "#98989d"),
    ("#707171", "#98989d"),
    ("#727374", "#98989d"),
    ("#747475", "#98989d"),
    ("#79797a", "#98989d"),
    // near-white surfaces + tinted fills
    ("#fcfcfc", "#1c1f22"),
    ("#e9edf4", "#2c2c2e"),
    ("#d4d4d7", "#38383a"),
    ("#d3d4d6", "#38383a"),
    ("#d5d6d8", "#38383a"),
    // #31d2: the review diff rows take their OWN dark fills (the outer's
    // prescription ~#12261A added / ~#2D1417 removed) and LIGHT text — the
    // first capture round left the near-black code inks unmapped, so the
    // added rows' text vanished against the dark green. Markers stay green/red
    // (#1f883d/#cf222e are accents, unmapped by design).
    ("#fdecec", "#2d1417"),
    ("#e6f4ea", "#12261a"),
    // the fold band ("⋮ N unmodified lines ⋮") surface — authored #f8f7f9,
    // unmapped in the first round, which kept the card WHITE in dark mode.
    ("#f8f7f9", "#232629"),
    // the diff rows' near-black code inks + gutter greys (kit values).
    ("#353636", "#e8e8ea"),
    ("#313834", "#e8e8ea"),
    ("#3b3c3a", "#e8e8ea"),
    ("#484848", "#e8e8ea"),
    ("#4c4c4b", "#e8e8ea"),
    ("#4e4e4d", "#e8e8ea"),
    ("#606060", "#98989d"),
    ("#78787b", "#98989d"),
];

/// Retint ONE lowered DSL string to the resolved palette. LIGHT is the byte
/// passthrough: every component/kit ships light, so an unchanged string keeps
/// the mount cache's diff-no-repaint behaviour (zero re-mounts). DARK rewrites
/// the light literals through [`TOKENS`] in BOTH forms the lowering emits: the
/// fork's `hex_rgba` emits `#rrggbbaa` (octoscript-makepad src/lib.rs:1025), so
/// 8-digit literals are rewritten RGB-plus-alpha — OPAQUE (alpha `ff`) only;
/// translucent fills (shadows, dimmers, `#00000000`) are left untouched so
/// they keep compositing over the themed surface. Mapped output is always 6
/// hex digits followed by the original alpha, never re-matched on a second
/// pass (the dark palette is not in the light key set).
pub fn retint_dsl(dsl: &str) -> String {
    if resolved() == "light" {
        return dsl.to_owned();
    }
    let bytes = dsl.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(dsl.len() + 256);
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' {
            // Count the hex run after '#': the lowering emits 6- AND 8-digit
            // forms; a 7-digit run (or any other length) is not a color.
            let mut n = 0usize;
            while n < 8
                && i + 1 + n < bytes.len()
                && bytes[i + 1 + n].is_ascii_hexdigit()
            {
                n += 1;
            }
            let terminated = i + 1 + n >= bytes.len() || !bytes[i + 1 + n].is_ascii_hexdigit();
            if (n == 6 || n == 8) && terminated {
                let hex: Vec<u8> = bytes[i..i + 1 + n].to_ascii_lowercase();
                if n == 6 {
                    match TOKENS.iter().find(|(l, _)| l.as_bytes() == &hex[..]) {
                        Some((_, d)) => out.extend_from_slice(d.as_bytes()),
                        None => out.extend_from_slice(&hex),
                    }
                    i += 7;
                    continue;
                }
                // 8-digit: rewrite OPAQUE literals RGB+alpha; translucent
                // fills keep their bytes (they composite over the theme).
                if &hex[7..9] == b"ff" {
                    match TOKENS.iter().find(|(l, _)| l.as_bytes() == &hex[..7]) {
                        Some((_, d)) => {
                            out.extend_from_slice(d.as_bytes());
                            out.extend_from_slice(b"ff");
                            i += 9;
                            continue;
                        }
                        None => {}
                    }
                }
                out.extend_from_slice(&hex);
                i += 9;
                continue;
            }
        }
        // Raw byte passthrough: non-ASCII bytes are copied untouched, so
        // the buffer stays valid UTF-8 (we never split a sequence — the
        // '#' branch only consumes ASCII).
        out.push(bytes[i]);
        i += 1;
    }
    // The rewrite only ever replaces ASCII runs with ASCII runs, so the
    // original multi-byte sequences are intact — from_utf8 cannot fail.
    String::from_utf8(out).unwrap_or_else(|_| dsl.to_owned())
}

// ---- #31d workflow 2: persistence across restarts ------------------------------
//
// The web stores the DISPLAY_PREFERENCES_KEY object (octoscode.web.display.v1,
// {version, theme, language, vimMode}) in localStorage; only light/dark are
// stored and system means the key is ABSENT (use-theme.ts:9-27). Natively the
// same JSON shape lives at $HOME/.octoscode/display.json (or OCTOSCODE_PREF_PATH)
// and is loaded ONLY by init_persistence() — the app's startup path calls it;
// tests never do, so the fresh-profile default (system) stays deterministic.

/// Read the persisted preference (None = absent/corrupt/system, web parity).
pub fn load_preference() -> Option<Theme> {
    let path = pref_path();
    let text = std::fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let stored = v.get("theme")?.as_str()?;
    Theme::parse(stored)
}

/// Persist the preference: light/dark write the object, system DELETES the file
/// (the absence IS system, use-theme.ts:20-22). Saving is best-effort — the
/// in-memory choice stays active when storage fails (use-theme.ts:24-26).
pub fn save_preference() {
    let stored = theme().lock().unwrap().pref.stored();
    let path = pref_path();
    match stored {
        Some(t) => {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let doc = serde_json::json!({
                "version": 1, "theme": t, "language": "en", "vimMode": false
            });
            let _ = std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap_or_default());
        }
        None => {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn pref_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("OCTOSCODE_PREF_PATH") {
        return p.into();
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::Path::new(&home).join(".octoscode").join("display.json")
}

/// The app's startup hook: adopt the persisted preference (if any) and install
/// the platform OS-appearance reader. The shell calls this once at mount; tests
/// never do.
pub fn init_persistence() {
    if let Some(t) = load_preference() {
        theme().lock().unwrap().pref = t;
    }
    #[cfg(target_os = "macos")]
    set_os_reader(os_is_dark_macos);
}


// ---- #31d workflow 3: the OS appearance ----------------------------------------

/// The injected OS-appearance reader (true = the OS is in dark mode). A
/// RwLock, not a OnceLock: tests inject per-scenario readers.
static OS_READER: std::sync::RwLock<Option<fn() -> bool>> = std::sync::RwLock::new(None);

/// Install a reader (the platform hook at startup; tests inject their own).
pub fn set_os_reader(read: fn() -> bool) {
    *OS_READER.write().unwrap() = Some(read);
}

/// Test seam: drop the injected reader (back to the dark fallback). Pub for
/// the integration tests (a separate crate); production never calls it.
pub fn clear_os_reader() {
    *OS_READER.write().unwrap() = None;
}

/// The macOS reader: AppleInterfaceStyle is set to "Dark" in dark mode and the
/// key is ABSENT in light mode (`defaults read -g AppleInterfaceStyle` exits
/// non-zero on light). Verified by the f31d tests through the injected seam;
/// the live `defaults` call itself is unverified-on-platform until the app
/// captures exercise system mode (disclosed in docs/upstream/ PR text too).
#[cfg(target_os = "macos")]
pub fn os_is_dark_macos() -> bool {
    std::process::Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().eq("Dark"))
        .unwrap_or(false)
}

// ---- #31d workflow 1b: the native shell containers ------------------------------
//
// The shell DSL (lib.rs script_mod) references makepad THEME ROLES
// (theme.color_bg_app, theme.color_bg_odd, …) instead of literals; this
// evaluator assigns the roles from the RESOLVED palette at script-mod time —
// the exact mechanism the makepad wm_theme bridge uses (`vm.eval` of
// `mod.theme.<role> = <value>` assignments; makepad/libs/wm_theme/src/lib.rs
// L262-301). Startup-correct by construction: init_persistence() runs before
// the first paint. A live `theme.cycle` re-assigns the roles for every widget
// created AFTER it and persists for the shell's next launch (disclosed).
pub fn role_assignments() -> String {
    // LIGHT pins the shell's CURRENT literals (byte-identical light mode); DARK
    // is the Stage B dark token set at the role level. Both modes assign — the
    // shell DSL references the roles, so the stock values must never leak in.
    if resolved() == "light" {
        r#"mod.theme.color_bg_app = #ffffff
mod.theme.color_bg_odd = #f7f7f8
mod.theme.color_bg_even = #f0f0f2
mod.theme.color_text_muted = #6e6e73
mod.theme.color_outset_1 = #e5e5e7
mod.theme.color_outset_2 = #e5e5e7
mod.theme.color_fg_app = #1d1d1f
mod.theme.color_bg_container = #ffffff
"#
        .to_owned()
    } else {
        r#"mod.theme.color_bg_app = #1c1f22
mod.theme.color_bg_odd = #1c1c1e
mod.theme.color_bg_even = #2c2c2e
mod.theme.color_text_muted = #98989d
mod.theme.color_outset_1 = #38383a
mod.theme.color_outset_2 = #38383a
mod.theme.color_fg_app = #f5f5f7
mod.theme.color_bg_container = #1c1f22
mod.theme.color_text = #f5f5f7
mod.theme.color_text_hover = #ffffff
mod.theme.color_bg_highlight = #2f6feb
mod.theme.color_bg_highlight_inline = #2c2c2e
mod.widgets.Window.pass.clear_color = #1c1f22
"#
        .to_owned()
    }
}

/// Assign the shell's theme roles in THIS VM (the `wm_theme::apply` pattern:
/// build a ScriptMod from the assignment lines and `vm.eval` it — unknown
/// roles are harmless, the assignment just creates them). Resolution order:
/// `OCTOSCODE_THEME` (the capture/probe seed; lib.rs's mount arm reads the
/// same variable) -> the persisted file ([`init_persistence`]) -> system via
/// the OS reader. Called from lib.rs's script_mod top (a `#(...)` splice) —
/// BEFORE the OctoscodeView class body dereferences any `theme.*` ref — and
/// from both capture probes' script_mod, so shell + card resolve identically.
pub fn eval_roles(vm: &mut makepad_widgets::ScriptVm) -> bool {
    use makepad_widgets::{ScriptMod, script_eval};
    if let Ok(pref) = std::env::var("OCTOSCODE_THEME") {
        set_preference(&pref);
    }
    init_persistence();
    let code = role_assignments();
    let script_mod_id = ScriptMod {
        cargo_manifest_path: crate::design::manifest_dir().to_string(),
        module_path: "octoscode_theme".to_string(),
        file: "octoscode_theme.splash".to_string(),
        line: 0,
        column: 0,
        code,
        values: vec![],
    };
    vm.eval(script_mod_id);
    for e in vm.take_errors() {
        ::log::warn!("octoscode_theme: {e}");
    }
    true
}
