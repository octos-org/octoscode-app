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

/// #36e item 2 — the web's **+/− diff line tint** as LITERAL hexes, picked per
/// palette at LOWER time. `DiffReviewDialog.module.css:45-54`:
///
/// ```css
/// :global(.diff-added)   .changedWord { background: color-mix(in srgb, var(--dsw-alias-state-success-primary) 22%, transparent); }
/// :global(.diff-removed) .changedWord { background: color-mix(in srgb, var(--dsw-alias-state-error-primary)   20%, transparent); }
/// ```
///
/// with `success #22c55e` and `error light-dark(#ec1313, #ff6b6b)`
/// (`app/theme.css:86-89`). A `color-mix` over a surface resolves per palette, so
/// the two values are pre-mixed here against the panel's own background
/// (`color_bg_app`, `theme.rs:544` light `#ffffff` / `:557` dark `#1c1f22`).
///
/// **Why literals and not a role.** Assigning a NEW `theme.*` role in
/// `role_assignments` is not readable by a widget default on this host: the
/// module's `script_mod` then aborts with `property … not found in prototype
/// chain`, which unregisters `OctoscodeView` and leaves the whole app unmounted
/// (measured — see .peer/report-36e.md §2). Existing roles resolve; a new one
/// does not. So the tint is baked as literal hex at lower time, exactly the way
/// the design emitter writes `hex_rgba` literals
/// (`octoscript-makepad/src/design.rs:733-735`) — a `#(...)` splice
/// (`lib.rs:121` is the precedent), so the script never has to resolve a role.
pub fn diff_tint_hexes() -> (&'static str, &'static str) {
    match resolved() {
        // success #22c55e @22% over #ffffff; error #ec1313 @20% over #ffffff
        "light" => ("#cef2dc", "#fbd4d4"),
        // success #22c55e @22% over #1c1f22; error #ff6b6b @20% over #1c1f22
        _ => ("#1d442f", "#462425"),
    }
}

/// `theme.set` / `OCTOSCODE_THEME`: set the preference from its stored string.
/// Unknown values are rejected (`use-theme.ts:11-13` only accepts
/// light/dark, else system — here the caller keeps the current preference).
/// #37b — serialises tests that READ or FLIP the global theme. The resolved
/// mode is process-global, and the lib-test binary runs its tests on many
/// threads: a flipper (the #36g ink test flips light and restores dark)
/// running concurrently with a lower()-based assertion flips the token
/// bytes BETWEEN that test's two lowers — the_composer_dsl_is_draft_free_
/// and_stable failed 14/15 rounds at --test-threads=8 exactly this way
/// (idle lowered in light, typed in dark). Every test that lowers, retints
/// or sets the preference holds this lock for its whole body; poisoning is
/// ignored (a panicking holder must not cascade into unrelated failures).
#[cfg(test)]
pub fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

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
    // A18: the web's light text levels (`app/theme.css:79-85`), which the
    // kits' secondary / tertiary / placeholder / link / success inks now are
    // (CONTRAST_PAIRS): secondary keeps its dark twin; tertiary and the
    // placeholder read the old faint grey (#A1A1A6: 6.4:1 on #1C1F22);
    // the link blue and the success green take the web's dark values.
    ("#61666b", "#98989d"),
    ("#5f646b", "#a1a1a6"),
    ("#646970", "#a1a1a6"),
    ("#3564c6", "#679efe"),
    ("#166534", "#86efac"),
    ("#c50f0f", "#ff6b6b"),
    // the board-3 blue tint (a selected option, a running chip): its dark
    // twin under the link blue's dark value (5.42:1; the light tint left
    // in place read 2.66:1 under it).
    ("#eaf1fd", "#1d2a40"),
    // the board-3 disabled / terminal fill: a terminal peer's chip keeps its
    // secondary label readable in dark (#98989D read 2.37:1 on the light
    // fill left in place; 4.85:1 on the twin).
    ("#e9e9eb", "#2c2c2e"),
    // the send control's disc: black in light, white in dark (both atlases)
    ("#050505", "#f5f5f7"),
    ("#030202", "#f5f5f7"),
    // #36f: the conversation components' artboard INK literals — the
    // measured pill sat at 1.00:1 on these. The bubble's SURFACE is NOT a
    // table key on purpose: #f5f5f7 is ALSO the dark ink token, so a key
    // here eats the correct light text on every dark line (the double-
    // retint cut proved it — the text came out #2c2c2e-on-#2c2c2e). The
    // surface is rewritten role-scoped in bubble_dark_surface instead.
    ("#fafbfb", "#f5f5f7"),
    ("#434343", "#e8e8ea"),
    ("#252525", "#e8e8ea"),
    // NOTE: #1c1f22 is deliberately NOT a key — it is the SHELL's dark
    // surface token, and a key here breaks retint's dark fixed-point
    // (f31d_retint_dark_rewrites_tokens: the dark output must re-retint to
    // itself). The user bubble's #1c1f22 timestamp ink is handled by the
    // role-scoped pin in bubble_dark_surface instead.
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
    {
        set_os_reader(os_is_dark_macos);
        // Warm the appearance cache off the UI thread: the first synchronous
        // `defaults` read stalled startup (ui-hang: 1669 ms, Startup phase).
        std::thread::spawn(|| {
            let _ = os_is_dark_macos();
        });
    }
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
///
/// CACHED: `resolved()` runs on the UI thread from draw and lowering paths, and
/// spawning `defaults` there stalled frames (ui-hang reports 254-1889 ms with
/// `Command::output` on top, and a > 5 s instrument timeout in the integration
/// walk). Only the first call reads synchronously; afterwards the cached value
/// is returned at once and refreshed on a background thread at most every
/// `OS_DARK_TTL`, so an OS appearance flip shows within a few seconds.
#[cfg(target_os = "macos")]
pub fn os_is_dark_macos() -> bool {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};
    const OS_DARK_TTL: Duration = Duration::from_secs(3);
    static CACHE: Mutex<Option<(bool, Instant)>> = Mutex::new(None);
    static REFRESHING: AtomicBool = AtomicBool::new(false);
    fn read_defaults() -> bool {
        std::process::Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().eq("Dark"))
            .unwrap_or(false)
    }
    let cached = *CACHE.lock().unwrap();
    match cached {
        Some((dark, at)) => {
            if at.elapsed() > OS_DARK_TTL && !REFRESHING.swap(true, Ordering::SeqCst) {
                std::thread::spawn(|| {
                    let dark = read_defaults();
                    *CACHE.lock().unwrap() = Some((dark, Instant::now()));
                    REFRESHING.store(false, Ordering::SeqCst);
                });
            }
            dark
        }
        None => {
            let dark = read_defaults();
            *CACHE.lock().unwrap() = Some((dark, Instant::now()));
            dark
        }
    }
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
    role_assignments_for(resolved() == "dark")
}

/// [`role_assignments`] for an explicit palette (A18: the contrast guard reads
/// both without flipping the process-global preference).
pub fn role_assignments_for(dark: bool) -> String {
    // LIGHT pins the shell's CURRENT literals (byte-identical light mode); DARK
    // is the Stage B dark token set at the role level. Both modes assign — the
    // shell DSL references the roles, so the stock values must never leak in.
    // A18: light secondary is the web's `--dsw-alias-label-secondary` — the
    // board's #6E6E73 read 4.46:1 on `color_bg_even` (the sidebar's segment
    // track, Settings > Model's thinking segments).
    if !dark {
        r#"mod.theme.color_bg_app = #ffffff
mod.theme.color_bg_odd = #f7f7f8
mod.theme.color_bg_even = #f0f0f2
mod.theme.color_text_muted = #61666b
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

/// A18 — the shell's accent TEXT inks, per palette: `(name, light, dark)`.
/// chrome.rs splices them into its templates (`chrome::ink`, like its icons)
/// because a NEW `theme.*` role is not readable by a widget default on this
/// host (see [`diff_tint_hexes`]). Light keeps the board's values; dark takes
/// the web's dark link / error text (`app/theme.css:85,88`): the shared
/// #2F6FEB read 3.62:1 and #C4141B 2.73:1 on the dark window.
pub const SHELL_INKS: &[(&str, &str, &str)] = &[
    // "Change", "Advanced…", "Clear search"
    ("link", "#2f6feb", "#679efe"),
    // "Stop server…", the shutdown error
    ("danger", "#d1242f", "#ff6b6b"),
    // "Forget server"
    ("danger_strong", "#c4141b", "#ff6b6b"),
];

/// The startup palette's value of a [`SHELL_INKS`] entry (`#rrggbb`).
pub fn shell_ink(name: &str) -> &'static str {
    let dark = resolved() == "dark";
    SHELL_INKS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, light, dark_v)| if dark { *dark_v } else { *light })
        .unwrap_or("#ff00ff")
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

// ---- A18: the contrast guard ---------------------------------------------------
//
// The web's e2e `theme.spec.ts:72-112` runs axe's color-contrast rule on the
// conversation and on Settings, in manual light mode on a dark OS, and expects
// ZERO violations. Natively every TEXT ink is declared below against every fill
// it is drawn on; the tests compute the WCAG 2.x ratio of each pair in BOTH
// palettes, so a colour edit — a kit token, a TOKENS dark twin, a shell role,
// a shell ink — fails a test instead of silently regressing. Inks that are not
// informational text are listed, with the reason, in [`EXEMPT_INKS`]. The
// end-to-end proof is measured on the app's own pixels:
// tools/judge/contrast_walk.py.

/// WCAG 2.x 1.4.3: body text.
pub const BODY_TEXT: f64 = 4.5;
/// WCAG 2.x: large text (>= 24 px, >= 18.66 px bold) and UI glyphs (1.4.11).
pub const LARGE_OR_GLYPH: f64 = 3.0;

/// A colour as each palette draws it.
#[derive(Clone, Copy, Debug)]
pub enum Swatch {
    /// A kit literal on a surface that follows the theme ([`retint_dsl`]:
    /// fluid, the conversation surfaces, the board-3 transcript rows and the
    /// status strip, A14's dialog kit): dark is its [`TOKENS`] twin (itself
    /// when it has none).
    Themed(&'static str),
    /// A kit literal on a surface drawn with its light palette in BOTH themes
    /// (the board-3 dialogs, board 1's sheets): the same in dark.
    Fixed(&'static str),
    /// A shell theme role ([`role_assignments_for`]).
    Role(&'static str),
    /// A shell accent ink ([`SHELL_INKS`]).
    Shell(&'static str),
}

/// One text ink on one fill it is drawn on.
#[derive(Clone, Copy, Debug)]
pub struct ContrastPair {
    pub ink: Swatch,
    pub fill: Swatch,
    /// [`BODY_TEXT`] or [`LARGE_OR_GLYPH`].
    pub min: f64,
    /// Where the pair is drawn (named in a failure).
    pub at: &'static str,
}

const fn pair(ink: Swatch, fill: Swatch, min: f64, at: &'static str) -> ContrastPair {
    ContrastPair { ink, fill, min, at }
}

use super::board1_kit as b1;
use super::board3::ui::tok;
use crate::fluid as fl;
use Swatch::{Fixed, Role, Shell, Themed};

/// Every TEXT ink against every fill it is drawn on (see the section note).
pub const CONTRAST_PAIRS: &[ContrastPair] = &[
    // ---- the shell (chrome.rs): header, sidebar, new-chat defaults line,
    //      Settings, the sidebar's menus — theme roles + shell inks.
    pair(Role("color_fg_app"), Role("color_bg_app"), BODY_TEXT, "shell primary text on the window"),
    pair(Role("color_fg_app"), Role("color_bg_odd"), BODY_TEXT, "shell primary text on the raised grey"),
    pair(Role("color_fg_app"), Role("color_bg_even"), BODY_TEXT, "a selected sidebar row; the selected segment's label"),
    pair(Role("color_text_muted"), Role("color_bg_app"), BODY_TEXT, "shell secondary text; the Search chats / rename placeholders"),
    pair(Role("color_text_muted"), Role("color_bg_odd"), BODY_TEXT, "shell secondary text on the raised grey"),
    pair(Role("color_text_muted"), Role("color_bg_even"), BODY_TEXT, "a segment's inactive label on its track ('All', Settings > Model)"),
    pair(Shell("link"), Role("color_bg_app"), BODY_TEXT, "'Change', 'Advanced…', 'Clear search'"),
    pair(Shell("danger"), Role("color_bg_app"), BODY_TEXT, "'Stop server…', the shutdown error"),
    pair(Shell("danger_strong"), Role("color_bg_app"), BODY_TEXT, "'Forget server'"),
    pair(Fixed("#ffffff"), Fixed("#d1242f"), BODY_TEXT, "the Stop server confirm's label on its red fill"),
    pair(Fixed("#ffffff"), Fixed("#1d1d1f"), BODY_TEXT, "the rename form's Save"),
    pair(Fixed("#ffffff"), Fixed("#000000"), BODY_TEXT, "the held banner's Take over"),
    // ---- the conversation components (fluid.rs; themed by retint_dsl).
    pair(Themed(fl::INK), Themed(fl::SURFACE), BODY_TEXT, "answer prose, the empty state, the connect card"),
    pair(Themed(fl::INK), Themed(fl::TIP), BODY_TEXT, "a tool group, a code block"),
    pair(Themed(fl::INK), Themed(fl::RAISED), BODY_TEXT, "inline code chips, the tool output well"),
    pair(Themed(fl::MUTED), Themed(fl::SURFACE), BODY_TEXT, "the empty state's hint, the connect field's placeholder"),
    pair(Themed(fl::MUTED), Themed(fl::TIP), BODY_TEXT, "a code block's Copy label, tool meta"),
    pair(Themed(fl::MUTED), Themed(fl::RAISED), BODY_TEXT, "tool output meta"),
    pair(Themed(fl::GREEN), Themed(fl::SURFACE), LARGE_OR_GLYPH, "the success check glyph"),
    pair(Themed(fl::RED), Themed(fl::SURFACE), LARGE_OR_GLYPH, "the failure mark glyph"),
    // ---- board 3 on the theme's surfaces: the transcript rows (notice,
    //      thinking, file, receipt, fold bar), the status strip, the
    //      conversation surfaces (question / approval cards, plan, Trajectory).
    pair(Themed(tok::TEXT), Themed(tok::SURFACE), BODY_TEXT, "a notice's title, a file name, the strip's facts"),
    pair(Themed(tok::TEXT), Themed(tok::SURFACE2), BODY_TEXT, "a thinking block, a takeover card"),
    pair(Themed(tok::TEXT), Themed(tok::CHIP), BODY_TEXT, "a chip, an option row"),
    pair(Themed(tok::TEXT), Themed(tok::BLUE_BG), BODY_TEXT, "a selected question option"),
    pair(Themed(tok::MUTED), Themed(tok::SURFACE), BODY_TEXT, "a notice's body, file meta, the strip's caption"),
    pair(Themed(tok::MUTED), Themed(tok::SURFACE2), BODY_TEXT, "thinking meta, card help"),
    pair(Themed(tok::MUTED), Themed(tok::CHIP), BODY_TEXT, "a neutral chip"),
    pair(Themed(tok::MUTED), Themed(tok::BLUE_BG), BODY_TEXT, "a selected option's description"),
    pair(Themed(tok::MUTED), Themed(tok::DISABLED_BG), BODY_TEXT, "A14's dialogs: a terminal peer's status chip"),
    pair(Themed(tok::FAINT), Themed(tok::SURFACE), BODY_TEXT, "'Model not reported', question hints, placeholders"),
    pair(Themed(tok::FAINT), Themed(tok::SURFACE2), BODY_TEXT, "the approval card's tool line and key hint, plan statuses"),
    pair(Themed(tok::FAINT), Themed(tok::CHIP), BODY_TEXT, "a hint on a chip"),
    pair(Themed(tok::BLUE_TEXT), Themed(tok::SURFACE), BODY_TEXT, "links: Expand all / Collapse all, Stop turn, the strip's transition"),
    pair(Themed(tok::BLUE_TEXT), Themed(tok::SURFACE2), BODY_TEXT, "a link inside a card; the task eyebrow"),
    pair(Themed(tok::BLUE_TEXT), Themed(tok::BLUE_BG), BODY_TEXT, "a running task chip"),
    pair(Themed(tok::GREEN_TEXT), Themed(tok::GREEN_BG), BODY_TEXT, "a completed task chip"),
    pair(Themed(tok::RED_TEXT), Themed(tok::SURFACE), BODY_TEXT, "a file's download error, a card's error line"),
    pair(Themed(tok::RED_TEXT), Themed(tok::SURFACE2), BODY_TEXT, "an error inside a card"),
    pair(Themed(tok::RED_TEXT), Themed(tok::RED_BG), BODY_TEXT, "a failed task chip, a high-risk approval"),
    pair(Themed(tok::AMBER), Themed(tok::AMBER_BG), BODY_TEXT, "a medium-risk approval chip"),
    pair(Themed(tok::WHITE), Themed(tok::BLACK), BODY_TEXT, "a primary pill's label (inverted in dark)"),
    // ---- board-3 dialogs (light in both themes): the session pane, the
    //      composer's menus, Fleet, Routes, Inspector, Agents, History, …
    pair(Fixed(tok::TEXT), Fixed(tok::SURFACE), BODY_TEXT, "dialog text"),
    pair(Fixed(tok::TEXT), Fixed(tok::SURFACE2), BODY_TEXT, "a card in a dialog"),
    pair(Fixed(tok::TEXT), Fixed(tok::CHIP), BODY_TEXT, "a secondary pill, a chip"),
    pair(Fixed(tok::MUTED), Fixed(tok::SURFACE), BODY_TEXT, "dialog secondary text"),
    pair(Fixed(tok::MUTED), Fixed(tok::SURFACE2), BODY_TEXT, "a segment's label, card help"),
    pair(Fixed(tok::MUTED), Fixed(tok::CHIP), BODY_TEXT, "a neutral chip (complete, unknown)"),
    pair(Fixed(tok::MUTED), Fixed(tok::DISABLED_BG), BODY_TEXT, "a terminal peer's status chip"),
    pair(Fixed(tok::FAINT), Fixed(tok::SURFACE), BODY_TEXT, "a menu title, a model group header, a row's description"),
    pair(Fixed(tok::FAINT), Fixed(tok::SURFACE2), BODY_TEXT, "a key chip, a hint on a card"),
    pair(Fixed(tok::FAINT), Fixed(tok::CHIP), BODY_TEXT, "a hint on a chip"),
    pair(Fixed(tok::BLUE_TEXT), Fixed(tok::SURFACE), BODY_TEXT, "a dialog link, a notice"),
    pair(Fixed(tok::BLUE_TEXT), Fixed(tok::SURFACE2), BODY_TEXT, "a link inside a card"),
    pair(Fixed(tok::BLUE_TEXT), Fixed(tok::BLUE_BG), BODY_TEXT, "a starting peer chip, an allowed decision, a lane's initial"),
    pair(Fixed(tok::GREEN_TEXT), Fixed(tok::SURFACE), BODY_TEXT, "a success note ('Saved', 'Sent')"),
    pair(Fixed(tok::GREEN_TEXT), Fixed(tok::SURFACE2), BODY_TEXT, "a success note inside a card"),
    pair(Fixed(tok::GREEN_TEXT), Fixed(tok::GREEN_BG), BODY_TEXT, "a working / active chip, a diff's + marker"),
    pair(Fixed(tok::RED_TEXT), Fixed(tok::SURFACE), BODY_TEXT, "a dialog error, a destructive link"),
    pair(Fixed(tok::RED_TEXT), Fixed(tok::SURFACE2), BODY_TEXT, "an error inside a card"),
    pair(Fixed(tok::RED_TEXT), Fixed(tok::RED_BG), BODY_TEXT, "a failed chip, a diff's - marker, an error box"),
    pair(Fixed(tok::AMBER), Fixed(tok::SURFACE), BODY_TEXT, "a waiting note"),
    pair(Fixed(tok::AMBER), Fixed(tok::AMBER_BG), BODY_TEXT, "a waiting / paused chip"),
    pair(Fixed(tok::WHITE), Fixed(tok::BLACK), BODY_TEXT, "a primary pill's label"),
    pair(Fixed(tok::WHITE), Fixed(tok::RED), BODY_TEXT, "an armed danger action's label"),
    // ---- board 1's sheets (Connect, pairing, folder browser, provider).
    pair(Fixed(b1::INK), Fixed(b1::WHITE), BODY_TEXT, "sheet text"),
    pair(Fixed(b1::INK), Fixed(b1::SUBTLE), BODY_TEXT, "a subtle row"),
    pair(Fixed(b1::INK), Fixed(b1::SELECTED), BODY_TEXT, "a selected row"),
    pair(Fixed(b1::MUTED), Fixed(b1::WHITE), BODY_TEXT, "sheet secondary text"),
    pair(Fixed(b1::MUTED), Fixed(b1::SUBTLE), BODY_TEXT, "a subtle row's detail"),
    pair(Fixed(b1::MUTED), Fixed(b1::SELECTED), BODY_TEXT, "a selected row's detail"),
    pair(Fixed(b1::FAINT), Fixed(b1::WHITE), BODY_TEXT, "the pairing scan note"),
    pair(Fixed(b1::PLACEHOLDER), Fixed(b1::WHITE), BODY_TEXT, "a field's placeholder"),
    pair(Fixed(b1::BLUE), Fixed(b1::WHITE), BODY_TEXT, "a sheet link ('Pair with a link instead')"),
    pair(Fixed(b1::RED), Fixed(b1::WHITE), BODY_TEXT, "a field error"),
    pair(Fixed(b1::RED), Fixed(b1::RED_BG), BODY_TEXT, "an error box"),
    pair(Fixed(b1::WHITE), Fixed(b1::BLACK), BODY_TEXT, "a primary pill's label"),
];

/// Inks that are NOT informational text, each with why it may stay under
/// 4.5:1 (WCAG 1.4.3 exempts inactive UI components and decoration; axe's
/// color-contrast rule skips disabled controls).
pub const EXEMPT_INKS: &[(&str, &str)] = &[
    (
        tok::DISABLED_INK,
        "board 3: the label of a control that cannot be used right now — Btn::Disabled / OutlineOff, an unarmed \
         danger action, an option with no event, a Restore that is blocked — and a disabled field's text",
    ),
    ("#8e8e93", "the shell: a disabled field's text (`color_disabled`) and the idle server dot (a status glyph)"),
];

/// `#rrggbb` (lower case) of a swatch in one palette.
pub fn swatch_hex(s: Swatch, dark: bool) -> String {
    fn rgb(h: &str) -> String {
        h.get(0..7).unwrap_or(h).to_ascii_lowercase()
    }
    match s {
        Swatch::Fixed(h) => rgb(h),
        Swatch::Themed(h) if dark => {
            let key = rgb(h);
            TOKENS.iter().find(|(l, _)| *l == key).map(|(_, d)| (*d).to_owned()).unwrap_or(key)
        }
        Swatch::Themed(h) => rgb(h),
        Swatch::Role(name) => role_assignments_for(dark)
            .lines()
            .find_map(|l| l.strip_prefix(&format!("mod.theme.{name} = ")).map(rgb))
            .unwrap_or_else(|| format!("#role-{name}-unassigned")),
        Swatch::Shell(name) => SHELL_INKS
            .iter()
            .find(|(n, _, _)| *n == name)
            .map(|(_, l, d)| rgb(if dark { d } else { l }))
            .unwrap_or_else(|| format!("#shell-{name}-missing")),
    }
}

/// The WCAG 2.x contrast ratio of two `#rrggbb` colours: relative luminance
/// with the sRGB linearisation, `(L1 + 0.05) / (L2 + 0.05)`.
pub fn wcag_ratio(a: &str, b: &str) -> f64 {
    fn lum(h: &str) -> f64 {
        let v = u32::from_str_radix(h.trim_start_matches('#').get(0..6).unwrap_or("000000"), 16).unwrap_or(0);
        let lin = |c: u32| {
            let c = c as f64 / 255.0;
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin((v >> 16) & 0xff) + 0.7152 * lin((v >> 8) & 0xff) + 0.0722 * lin(v & 0xff)
    }
    let (la, lb) = (lum(a), lum(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[cfg(test)]
mod contrast_tests {
    use super::*;

    fn failures(dark: bool) -> Vec<String> {
        CONTRAST_PAIRS
            .iter()
            .filter_map(|p| {
                let (ink, fill) = (swatch_hex(p.ink, dark), swatch_hex(p.fill, dark));
                let r = wcag_ratio(&ink, &fill);
                (r + 1e-9 < p.min).then(|| {
                    format!(
                        "{} {ink} on {fill} = {r:.2}:1 < {}:1 — {} ({:?} on {:?})",
                        if dark { "dark" } else { "light" },
                        p.min,
                        p.at,
                        p.ink,
                        p.fill
                    )
                })
            })
            .collect()
    }

    /// The guard: every declared TEXT ink meets its WCAG minimum on every
    /// fill it is drawn on, in the light AND the dark palette.
    #[test]
    fn every_text_ink_meets_wcag_on_every_fill_in_both_palettes() {
        let mut bad = failures(false);
        bad.extend(failures(true));
        assert!(bad.is_empty(), "contrast below WCAG:\n{}", bad.join("\n"));
    }

    /// The guard has teeth: the board values A18 replaced fail it (the web's
    /// axe gate failed on exactly these), so a revert is caught.
    #[test]
    fn the_replaced_board_inks_fail_the_rule() {
        for (ink, fill, what) in [
            ("#a1a1a6", "#ffffff", "board 3's faint 'Model not reported' (2.57:1)"),
            ("#8e8e93", "#ffffff", "board 1's faint / the Search chats placeholder (3.26:1)"),
            ("#6e6e73", "#f0f0f2", "the secondary 'All' segment on its track (4.46:1)"),
            ("#2f6feb", "#eaf1fd", "blue chip text on the blue tint (4.03:1)"),
            ("#1f883d", "#e6f4ea", "green chip text on the green tint (3.98:1)"),
            ("#2f6feb", "#1c1f22", "a dark-theme link (3.62:1)"),
            ("#c4141b", "#1c1f22", "'Forget server' in dark (2.73:1)"),
            ("#1d1d1f", "#1c1f22", "an unmapped notice title on the dark transcript (1.02:1)"),
        ] {
            assert!(wcag_ratio(ink, fill) < BODY_TEXT, "{what}");
        }
    }

    /// The ratio is the WCAG formula (reference values).
    #[test]
    fn wcag_ratio_matches_the_reference_values() {
        assert!((wcag_ratio("#000000", "#ffffff") - 21.0).abs() < 1e-9);
        assert!((wcag_ratio("#ffffff", "#ffffff") - 1.0).abs() < 1e-9);
        assert!((wcag_ratio("#777777", "#ffffff") - 4.48).abs() < 0.01);
        assert_eq!(format!("{:.2}", wcag_ratio("#61666b", "#ffffff")), "5.80");
    }

    /// An exempt ink is never declared as informational text, and a declared
    /// ink never sits in the exemption list.
    #[test]
    fn exempt_inks_are_not_declared_text() {
        for (ink, why) in EXEMPT_INKS {
            assert!(!why.is_empty(), "every exemption says why");
            let key = ink.get(0..7).unwrap_or(ink).to_ascii_lowercase();
            for p in CONTRAST_PAIRS {
                let declared = match p.ink {
                    Swatch::Fixed(h) | Swatch::Themed(h) => h.get(0..7).unwrap_or(h).to_ascii_lowercase() == key,
                    _ => false,
                };
                assert!(!declared, "{ink} is exempt ({why}) but declared as text at {}", p.at);
            }
        }
    }

    /// Every swatch resolves in both palettes (a role or a shell ink that a
    /// palette does not assign would silently read as the stock colour).
    #[test]
    fn every_swatch_resolves_in_both_palettes() {
        for p in CONTRAST_PAIRS {
            for dark in [false, true] {
                for s in [p.ink, p.fill] {
                    let h = swatch_hex(s, dark);
                    assert!(
                        h.len() == 7 && u32::from_str_radix(&h[1..], 16).is_ok(),
                        "{s:?} does not resolve in {} ({h}) — {}",
                        if dark { "dark" } else { "light" },
                        p.at
                    );
                }
            }
        }
    }

    /// [`retint_dsl`]'s dark output is a fixed point: no dark twin is itself a
    /// light key (A18 added keys — the web's light text levels — and twins).
    #[test]
    fn dark_twins_are_never_light_keys() {
        for (light, dark) in TOKENS {
            assert!(
                !TOKENS.iter().any(|(l, _)| l == dark),
                "the dark twin {dark} of {light} is also a light key: a second retint would move it"
            );
        }
    }

    /// The source scan: board 3's text inks are TEXT tokens. A `Txt` (a text
    /// run) or a chip's `(fg, tint)` naming a FILL or ACCENT token directly —
    /// `tok::BLUE` (4.03:1 on its tint), `tok::GREEN`, `tok::RED`,
    /// `tok::HAIRLINE`, … — fails here with the text token to use.
    #[test]
    fn board3_text_runs_use_text_tokens() {
        const TEXT_TOKENS: &[&str] = &[
            "TEXT", "MUTED", "FAINT", "DISABLED_INK", "WHITE", "BLUE_TEXT", "GREEN_TEXT", "RED_TEXT", "AMBER",
        ];
        const TINTS: &[&str] = &["BLUE_BG", "GREEN_BG", "RED_BG", "AMBER_BG", "CHIP", "SURFACE2", "DISABLED_BG"];
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/screens");
        let mut files = Vec::new();
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("screens dir").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    files.push(p);
                }
            }
        }
        let tokens_in = |s: &str| -> Vec<String> {
            s.match_indices("tok::")
                .map(|(i, _)| s[i + 5..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect())
                .collect()
        };
        let mut bad = Vec::new();
        for f in &files {
            let src = std::fs::read_to_string(f).unwrap_or_default();
            let name = f.file_name().unwrap().to_string_lossy().to_string();
            // `Txt::new(<px>, <face>, <ink>)`: every token in the call names a text ink.
            for (at, _) in src.match_indices("Txt::new(") {
                let rest = &src[at + 9..];
                let mut depth = 1;
                let end = rest
                    .char_indices()
                    .find(|(_, c)| {
                        match c {
                            '(' => depth += 1,
                            ')' => depth -= 1,
                            _ => {}
                        }
                        depth == 0
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(rest.len());
                for t in tokens_in(&rest[..end]) {
                    if !TEXT_TOKENS.contains(&t.as_str()) {
                        let line = src[..at].lines().count();
                        bad.push(format!("{name}:{line}: Txt ink tok::{t}"));
                    }
                }
            }
            // A chip's `(tok::FG, tok::TINT` pair: the fg is a text ink.
            for (at, _) in src.match_indices("(tok::") {
                let seg: String = src[at + 1..].chars().take_while(|c| *c != ')' && *c != '\n').collect();
                let toks = tokens_in(&seg);
                if toks.len() >= 2 && TINTS.contains(&toks[1].as_str()) && !TEXT_TOKENS.contains(&toks[0].as_str()) {
                    let line = src[..at].lines().count();
                    bad.push(format!("{name}:{line}: chip ink tok::{} on tok::{}", toks[0], toks[1]));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "text drawn in a non-text token (use BLUE_TEXT / GREEN_TEXT / RED_TEXT, FAINT, or DISABLED_INK for a disabled control):\n{}",
            bad.join("\n")
        );
    }

    /// The shell's colour literals on text are declared or exempt: chrome.rs
    /// draws its text in roles and spliced shell inks; a raw `#rrggbb` on a
    /// `draw_text` must be a [`CONTRAST_PAIRS`] ink or an [`EXEMPT_INKS`] one.
    #[test]
    fn shell_text_literals_are_declared_or_exempt() {
        let src = include_str!("../chrome.rs");
        let declared: Vec<String> = CONTRAST_PAIRS
            .iter()
            .filter_map(|p| match p.ink {
                Swatch::Fixed(h) => Some(h.get(0..7).unwrap_or(h).to_ascii_lowercase()),
                _ => None,
            })
            .chain(EXEMPT_INKS.iter().map(|(h, _)| h.get(0..7).unwrap_or(h).to_ascii_lowercase()))
            .collect();
        let mut bad = Vec::new();
        for (n, line) in src.lines().enumerate() {
            let Some(i) = line.find("draw_text") else { continue };
            let tail = &line[i..];
            let mut rest = tail;
            while let Some(j) = rest.find("color") {
                let after = &rest[j..];
                let hex = after.find('#').filter(|k| *k < 24).map(|k| &after[k..]);
                if let Some(h) = hex {
                    let lit: String = h.chars().take(7).collect::<String>().to_ascii_lowercase();
                    if lit.len() == 7 && lit[1..].chars().all(|c| c.is_ascii_hexdigit()) && !declared.contains(&lit) {
                        bad.push(format!("chrome.rs:{}: {lit}", n + 1));
                    }
                }
                rest = &after[5..];
            }
        }
        assert!(bad.is_empty(), "undeclared text colours in chrome.rs:\n{}", bad.join("\n"));
    }
}
