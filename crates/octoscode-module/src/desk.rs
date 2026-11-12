//! The Mac look on a desktop: the one gate and the one token table.
//!
//! OctoSense's `apps/AGENTS.md` ("The Mac look on a desktop") is the contract
//! every app follows: 13-point text, 28-point controls, a sidebar and a toolbar
//! instead of pages and pills, neutral surfaces a shade apart, hairlines, one
//! default action per surface in the system blue. A phone's tree and metrics
//! stay what they were.
//!
//! [`active`] says which of the two this process draws. It is false until
//! [`note_style`] has read the host's style (so a lowering in a unit test, and
//! a phone, keep their strings byte for byte), and true for every non-mobile
//! style, the standalone binary included.
//!
//! The kits ([`crate::fluid`], `screens::board1_kit`, `screens::board3::ui`, the
//! lowered components) write the light board's literal colours and name the
//! Inter faces. On a desktop [`for_mount`], at the one chokepoint every mounted
//! surface passes (`mount::MountCache::mount`), turns those into the tokens
//! below and the theme's text faces, so every sheet follows the appearance too.
//!
//! The tokens are `apps/interface.splash`'s formulas over the host's page and
//! ink, so a hosted OctosCode draws the same surfaces as every other app.
use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use makepad_widgets::*;

static DESK: AtomicBool = AtomicBool::new(false);
/// `theme.ui_unified`: a Mac window with no title bar (below).
static UNIFIED: AtomicBool = AtomicBool::new(false);
/// A window of OctoSense's own: the shell's kernel is its connection (below).
static HOSTED: AtomicBool = AtomicBool::new(false);
/// Bumped when the host's style or appearance changed: the view re-lowers.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The page and ink a macOS sheet sets (`widgets/themes/macos*/theme.splash`).
const LIGHT_PAGE: u32 = 0xececec;
const LIGHT_INK: u32 = 0x242426;
const DARK_PAGE: u32 = 0x28282a;
const DARK_INK: u32 = 0xf2f2f4;

/// The shell's page and ink, read when it (re)registers the module.
static HOST: RwLock<Option<(u32, u32)>> = RwLock::new(None);
static TABLE: RwLock<Option<Arc<[Tokens; 2]>>> = RwLock::new(None);

script_mod! {
    use mod.prelude.widgets.*

    // A kit tap target on a Mac: nothing at rest, the colours its DSL names under
    // the pointer (`board3::ui::Dsl::tap`), so a row or a button answers hover.
    // Here, not in each DSL: a mounted surface must not carry its own shader.
    mod.widgets.DesignNativeButton = mod.widgets.DesignNativeButton {
        draw_bg +: {
            color_hover: #0000
            color_down: #0000
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.0 0.0 self.rect_size.x self.rect_size.y 3.0)
                sdf.fill(vec4(0.0).mix(self.color_hover, self.hover).mix(self.color_down, self.down))
                return sdf.result
            }
        }
    }
    mod.prelude.widgets.DesignNativeButton = mod.widgets.DesignNativeButton

    // A Mac sheet's surface, as in Clock, Notes and Reminders: a 10-point corner,
    // a hairline, a soft shadow. A batch of its own, so what it holds draws over
    // the window's dim.
    mod.widgets.OcMacSheet = RoundedShadowView{
        new_batch: true
        draw_bg +: {
            border_radius: 5.0 border_size: 1.0
            shadow_color: #x00000040 shadow_radius: 24.0 shadow_offset: vec2(0.0, 8.0)
        }
    }
    mod.prelude.widgets.OcMacSheet = mod.widgets.OcMacSheet

    // A Mac menu's surface, as the framework's menus draw theirs (`PopupMenu`
    // in the Mac theme): a 12-point corner, a 0.65-point edge, the 14-point
    // shadow all round. Its fill and edge are the `menu` and `menu_edge` tokens.
    mod.widgets.OcMacMenu = RoundedShadowView{
        new_batch: true
        draw_bg +: {
            border_radius: 6.0 border_size: 0.65
            shadow_color: #x00000052 shadow_radius: 14.0 shadow_offset: vec2(0.0, 0.0)
        }
    }
    mod.prelude.widgets.OcMacMenu = mod.widgets.OcMacMenu
}

/// Install what the design vocabulary needs on a desktop, after it registered.
pub fn register_vocabulary(vm: &mut ScriptVm) {
    if active() {
        script_mod(vm);
    }
}

/// Whether this process draws the Mac look (any non-mobile style).
pub fn active() -> bool {
    DESK.load(Ordering::Relaxed)
}

/// Whether the window has no title bar. The shell's Mac style says so
/// (`theme.ui_unified`, `apps/AGENTS.md`): the app draws to the window's top edge,
/// the window's lights sit over the leading 84 points of its toolbar or over the
/// top of its sidebar, and a press nothing claims in the top 52 points moves the
/// window. Every other desktop keeps a title bar and 40-point toolbars.
pub fn unified() -> bool {
    active() && UNIFIED.load(Ordering::Relaxed)
}

pub fn note_hosted(on: bool) {
    HOSTED.store(on, Ordering::Relaxed);
}

/// A desktop window on the shell's own kernel. There is no server to name, no
/// profile to choose and no peers to start, so what speaks of them goes away.
pub fn hosted() -> bool {
    active() && HOSTED.load(Ordering::Relaxed)
}

/// A toolbar's height: the window's top band.
pub fn bar_h() -> f64 {
    if unified() {
        52.0
    } else {
        40.0
    }
}

/// The room a toolbar with nothing to its left leaves for the window's lights.
pub fn lights_room() -> f64 {
    if unified() {
        84.0
    } else {
        0.0
    }
}

/// A toolbar's leading inset, `left` points from the window's edge: its usual 12,
/// or what clears the lights.
pub fn bar_lead(left: f64) -> f64 {
    (lights_room() - left).max(12.0)
}

/// What a sheet leaves under itself, above the window's foot.
const SHEET_FOOT: f64 = 16.0;

/// The dim behind a sheet, the same in every app: 14% black in the light,
/// 32% in the dark, as the shell's and Files' sheets.
pub fn sheet_dim() -> &'static str {
    if current().dark {
        "#00000052"
    } else {
        "#00000024"
    }
}

/// The height a sheet may take in a window `h` points tall: it hangs from the
/// foot of the toolbar, as in Clock, Notes and Reminders.
pub fn sheet_room(h: f64) -> f64 {
    h - bar_h() - SHEET_FOOT
}

/// A sheet's width on a desktop: a question's, a form's or a list's, whatever
/// the phone's or the web's width was. A table of two panes takes the largest.
pub fn sheet_w(max: f64) -> f64 {
    if max <= 480.0 {
        400.0
    } else if max <= 600.0 {
        520.0
    } else if max <= 800.0 {
        640.0
    } else {
        760.0
    }
}

/// A change counter of what the tokens depend on: the view compares it to
/// re-lower its surfaces after a host restyle.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// Read the style a host installed in this VM (or the bare desktop's default).
pub fn note_style(vm: &mut ScriptVm) {
    use makepad_widgets::{script_eval, ScriptMod};
    let desk = !desktop_style::current_style(vm).mobile();
    if DESK.swap(desk, Ordering::Relaxed) != desk {
        GENERATION.fetch_add(1, Ordering::Relaxed);
    }
    // Only a shell's sheet states it, so a bare standalone window has its title
    // bar. `OCTOSCODE_UNIFIED` previews the layout there.
    let unified = script_eval!(vm, { try { mod.theme.ui_unified == true } catch { false } }).as_bool().unwrap_or(false)
        || std::env::var_os("OCTOSCODE_UNIFIED").is_some();
    let _ = vm.take_errors();
    if UNIFIED.swap(unified, Ordering::Relaxed) != unified {
        GENERATION.fetch_add(1, Ordering::Relaxed);
    }
}

/// Read the embedding shell's page and ink. Runs when the shell (re)registers
/// the module, before this module's own role assignments replace them in its
/// isolate.
pub fn note_host(vm: &mut ScriptVm) {
    use makepad_widgets::{script_eval, ScriptMod};
    let page = script_eval!(vm, { mod.theme.color_bg_app }).as_color();
    let ink = script_eval!(vm, { mod.theme.color_text }).as_color();
    let _ = vm.take_errors();
    let next = match (page, ink) {
        (Some(p), Some(i)) => Some((p >> 8, i >> 8)),
        _ => None,
    };
    let mut host = HOST.write().unwrap();
    if *host != next {
        *host = next;
        *TABLE.write().unwrap() = None;
        GENERATION.fetch_add(1, Ordering::Relaxed);
    }
}

/// The file a mark draws from on a desktop: a selected check, radio or plan
/// step is blue with a white mark in both appearances, in the system's blue.
pub fn mac_icon(name: &str) -> &str {
    if !active() {
        return name;
    }
    match name {
        "cv_check_on.svg" | "b3_box_on.svg" => "mac_check_on.svg",
        "cv_radio_on.svg" => "mac_radio_on.svg",
        "cv_step_done.svg" => "mac_step_done.svg",
        "cv_step_active.svg" => "mac_step_active.svg",
        "b1_check_on.svg" => "mac_circle_check.svg",
        other => other,
    }
}

/// What the shell's appearance is, when a shell hosts this app, on any tree
/// (its System appearance is the shell's, not the OS's: the page comes from the
/// shell, so ink chosen by the OS could land dark on dark).
pub fn host_dark() -> Option<bool> {
    let (_, ink) = (*HOST.read().unwrap())?;
    Some(Rgb::of(ink).lum() >= 0.5)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rgb(f64, f64, f64);

impl Rgb {
    fn of(v: u32) -> Rgb {
        Rgb(
            ((v >> 16) & 0xff) as f64 / 255.0,
            ((v >> 8) & 0xff) as f64 / 255.0,
            (v & 0xff) as f64 / 255.0,
        )
    }
    /// `dot(c, #x36b61200)`, the interface's luminance.
    fn lum(self) -> f64 {
        self.0 * (0x36 as f64 / 255.0) + self.1 * (0xb6 as f64 / 255.0) + self.2 * (0x12 as f64 / 255.0)
    }
    fn mix(self, to: Rgb, t: f64) -> Rgb {
        Rgb(
            self.0 + (to.0 - self.0) * t,
            self.1 + (to.1 - self.1) * t,
            self.2 + (to.2 - self.2) * t,
        )
    }
    fn hex(self) -> String {
        let b = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u32;
        format!("#{:02x}{:02x}{:02x}", b(self.0), b(self.1), b(self.2))
    }
}

/// The Mac surfaces and inks of one appearance (`#rrggbb`).
#[derive(Debug, Clone, PartialEq)]
pub struct Tokens {
    pub dark: bool,
    pub page: String,
    pub ink: String,
    /// The content pane and a list: white, or the dark list colour.
    pub content: String,
    /// What sits on a pane or a sheet: a group, a card; white, or a lifted gray.
    pub surface: String,
    pub field: String,
    pub hover: String,
    pub border: String,
    pub muted: String,
    pub link: String,
    pub primary: String,
    pub primary_hover: String,
    pub primary_down: String,
    /// The accent as a glyph or a track, with a white mark on it. `primary` is the
    /// fill under a white word, too dark to read on a dark sheet.
    pub accent: String,
    pub success: String,
    pub danger: String,
    /// A red word on `button`: a dark face needs a lighter red than a sheet does.
    pub danger_button: String,
    pub sidebar: String,
    pub bar: String,
    pub selected: String,
    pub button: String,
    pub button_hover: String,
    pub button_down: String,
    pub segment_on: String,
    pub switch_off: String,
    pub ring: String,
    /// A recessed well on the content pane: a code block, a tool's output.
    pub well: String,
    /// A raised fill: a user's message, an inline code chip.
    pub raised: String,
    /// A band one step off the pane: a fold, a table head.
    pub band: String,
    pub tint_accent: String,
    pub tint_danger: String,
    pub tint_success: String,
    pub edge_danger: String,
    pub amber: String,
    pub tint_amber: String,
    pub edge_amber: String,
    /// A hover or press wash over any surface (`#rrggbbaa`).
    pub wash: String,
    pub wash_down: String,
}

impl Tokens {
    /// `apps/interface.splash`'s formulas over a page and an ink.
    pub fn of(page: u32, ink: u32) -> Tokens {
        let (page_c, ink_c) = (Rgb::of(page), Rgb::of(ink));
        let light = ink_c.lum() < 0.5;
        let lum = page_c.lum();
        let mac = !(0.25..=0.86).contains(&lum);
        let white = Rgb::of(0xffffff);
        let surface = if light { white } else { page_c.mix(ink_c, 0.07) };
        let button = if light { white } else { page_c.mix(ink_c, 0.16) };
        let link = if mac {
            Rgb::of(if light { 0x0060c0 } else { 0x6cb8ff })
        } else {
            ink_c.mix(Rgb::of(0x356bce), 0.25)
        };
        let success = if mac {
            Rgb::of(if light { 0x1a7030 } else { 0x30d158 })
        } else {
            ink_c.mix(Rgb::of(0x28a55b), 0.20)
        };
        let danger = if mac {
            Rgb::of(if light { 0xc70012 } else { 0xff8d85 })
        } else {
            ink_c.mix(Rgb::of(0xe13b53), 0.20)
        };
        let amber = Rgb::of(if light { 0x8f5400 } else { 0xffb340 });
        let primary = Rgb::of(0x0071e3);
        // Tints keep their ink at 4.5:1 on either appearance (`tests::`).
        let tint = |c: Rgb, l: f64, d: f64| surface.mix(c, if light { l } else { d });
        Tokens {
            dark: !light,
            page: page_c.hex(),
            ink: ink_c.hex(),
            content: if light { "#ffffff".to_owned() } else { "#1e1e1e".to_owned() },
            surface: surface.hex(),
            field: surface.hex(),
            hover: page_c.mix(ink_c, 0.07).hex(),
            border: page_c.mix(ink_c, 0.14).hex(),
            muted: ink_c.mix(page_c, 0.26).hex(),
            link: link.hex(),
            primary: primary.hex(),
            primary_hover: "#006edb".to_owned(),
            primary_down: "#0060c0".to_owned(),
            // The dark Mac sheet's selected control is #0a84ff; this is the nearest
            // that reads 3:1 on the sheet and on its own tint too.
            accent: if light { primary.hex() } else { "#0a8aff".to_owned() },
            success: success.hex(),
            danger: danger.hex(),
            danger_button: if light { danger.hex() } else { danger.mix(white, 0.15).hex() },
            sidebar: page_c.mix(ink_c, 0.03).hex(),
            bar: Rgb::of(if light { 0xededf0 } else { 0x303033 }).hex(),
            selected: page_c.mix(ink_c, 0.11).hex(),
            button: button.hex(),
            button_hover: button.mix(ink_c, 0.04).hex(),
            button_down: button.mix(ink_c, 0.10).hex(),
            // What sits on a track, where a dark Mac's control is lighter than
            // `button`: on a dark sheet it has to read from the track.
            segment_on: if light { button.hex() } else { page_c.mix(ink_c, 0.27).hex() },
            switch_off: if light { page_c.mix(ink_c, 0.11).hex() } else { page_c.mix(ink_c, 0.27).hex() },
            ring: if light { page_c.mix(ink_c, 0.26).hex() } else { page_c.mix(ink_c, 0.42).hex() },
            well: if light { Rgb::of(0xf6f6f8).hex() } else { page_c.mix(ink_c, 0.03).hex() },
            // In dark, as light as the red word on it allows (`danger`, 4.5:1).
            raised: if light { Rgb::of(0xf0f0f2).hex() } else { page_c.mix(ink_c, 0.125).hex() },
            band: if light { Rgb::of(0xf4f4f6).hex() } else { page_c.mix(ink_c, 0.04).hex() },
            tint_accent: tint(primary, 0.10, 0.18).hex(),
            tint_danger: tint(Rgb::of(0xd70015), 0.08, 0.16).hex(),
            tint_success: tint(Rgb::of(0x1c7833), 0.09, 0.14).hex(),
            edge_danger: tint(danger, 0.30, 0.45).hex(),
            amber: amber.hex(),
            tint_amber: tint(Rgb::of(0xff9f0a), 0.12, 0.14).hex(),
            edge_amber: tint(Rgb::of(0xff9f0a), 0.40, 0.40).hex(),
            // `#fffffe`: never a retint key (`#ffffff` is the dark surface's).
            wash: if light { "#00000012".to_owned() } else { "#fffffe14".to_owned() },
            wash_down: if light { "#00000020".to_owned() } else { "#fffffe24".to_owned() },
        }
    }
}

impl Tokens {
    /// A named palette's surfaces and inks, by the same formulas: a person's own
    /// theme colours the whole window, its sidebar and toolbar too.
    fn named(p: &crate::screens::theme::NamedPalette) -> Tokens {
        let hex = |s: &str| u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0);
        let mut t = Tokens::of(hex(p.surface), hex(p.text));
        let (page, ink) = (Rgb::of(hex(p.surface)), Rgb::of(hex(p.text)));
        t.content = p.surface.to_owned();
        t.bar = page.mix(ink, 0.07).hex();
        t.border = p.hairline.to_owned();
        t.muted = p.muted.to_owned();
        t.link = p.accent_text.to_owned();
        t.success = p.success_text.to_owned();
        t.danger = p.danger_text.to_owned();
        t.danger_button = p.danger_text.to_owned();
        // The accent as a fill under a white word: as dark as 4.5:1 asks.
        let (accent, black) = (Rgb::of(hex(p.accent)), Rgb::of(0));
        let mut fill = accent;
        let mut k = 0.0;
        while crate::screens::theme::wcag_ratio("#ffffff", &fill.hex()) < 4.5 && k < 0.6 {
            k += 0.02;
            fill = accent.mix(black, k);
        }
        t.primary = fill.hex();
        t.accent = p.accent.to_owned();
        t.primary_hover = fill.mix(black, 0.06).hex();
        t.primary_down = fill.mix(black, 0.14).hex();
        t
    }
}

fn table() -> Arc<[Tokens; 2]> {
    if let Some(t) = TABLE.read().unwrap().as_ref() {
        return t.clone();
    }
    let host = *HOST.read().unwrap();
    let of = |dark: bool| {
        let (page, ink) = if dark { (DARK_PAGE, DARK_INK) } else { (LIGHT_PAGE, LIGHT_INK) };
        // The shell's own page and ink for its appearance; an explicit
        // Light / Dark preference that differs from it takes the Mac's.
        match host {
            Some((p, i)) if (Rgb::of(i).lum() >= 0.5) == dark => Tokens::of(p, i),
            _ => Tokens::of(page, ink),
        }
    };
    let t = Arc::new([of(false), of(true)]);
    *TABLE.write().unwrap() = Some(t.clone());
    t
}

/// A red word on a raised gray face, for a kit whose colours are constants.
pub fn danger_on_face() -> &'static str {
    use crate::screens::theme::{current_look, Look};
    match current_look() {
        Look::Mac(true) => "#ff9e97ff",
        _ => crate::screens::board3::ui::tok::RED_TEXT,
    }
}

/// A sheet is up: what lies under it offers no default of its own.
static SHEET_UP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// See [`SHEET_UP`].
pub fn note_sheet_up(up: bool) {
    SHEET_UP.store(up, std::sync::atomic::Ordering::Relaxed);
}

/// See [`SHEET_UP`].
pub fn sheet_up() -> bool {
    SHEET_UP.load(std::sync::atomic::Ordering::Relaxed)
}

/// The open chat's history is still loading.
static HISTORY_LOADING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// See [`HISTORY_LOADING`].
pub fn note_history_loading(loading: bool) {
    HISTORY_LOADING.store(loading, std::sync::atomic::Ordering::Relaxed);
}

/// See [`HISTORY_LOADING`].
pub fn history_loading() -> bool {
    HISTORY_LOADING.load(std::sync::atomic::Ordering::Relaxed)
}

/// The open chat's history couldn't be read.
static HISTORY_FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// See [`HISTORY_FAILED`].
pub fn note_history_failed(failed: bool) {
    HISTORY_FAILED.store(failed, std::sync::atomic::Ordering::Relaxed);
}

/// See [`HISTORY_FAILED`].
pub fn history_failed() -> bool {
    HISTORY_FAILED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Status of the selected chat's own open: 0 ready/none, 1 opening, 2 failed.
static CHAT_OPEN: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub(crate) fn note_chat_open(phase: Option<crate::flow::OpenPhase>) {
    let value = match phase {
        Some(crate::flow::OpenPhase::Opening) => 1,
        Some(crate::flow::OpenPhase::Failed) => 2,
        _ => 0,
    };
    CHAT_OPEN.store(value, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn chat_open() -> u8 {
    CHAT_OPEN.load(std::sync::atomic::Ordering::Relaxed)
}

/// A file's size as a Mac says it (Finder's: decimal units, "98 KB",
/// "1.3 MB"), where the web says KiB and MiB.
pub fn file_size(bytes: u64) -> String {
    let trim = |s: String| {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s
        }
    };
    if bytes < 1000 {
        return if bytes == 1 {
            crate::i18n::tr("1 byte").to_owned()
        } else {
            crate::i18n::tr1("{value0} bytes", &bytes.to_string())
        };
    }
    let kb = (bytes as f64 / 1e3).round();
    if kb < 1000.0 {
        return format!("{kb:.0} KB");
    }
    let mb = bytes as f64 / 1e6;
    if (mb * 10.0).round() < 10_000.0 {
        return format!("{} MB", trim(format!("{mb:.1}")));
    }
    format!("{} GB", trim(format!("{:.2}", bytes as f64 / 1e9)))
}

/// A push button's word while it can't be pressed: about 3.5:1 on either
/// appearance's face, no literal the retint tables move.
pub fn disabled_on_face() -> &'static str {
    use crate::screens::theme::{current_look, Look};
    match current_look() {
        Look::Mac(true) => "#a5a5aaff",
        _ => "#8a8a8fff",
    }
}

/// An option's name while it can't be chosen (a menu row, a sheet's radio
/// row): about 3.5:1 on a menu or a sheet in either appearance, as a disabled
/// button's word is; no literal the retint tables move.
pub fn disabled_option_ink() -> &'static str {
    "#8a8a8fff"
}

/// The same on a Mac menu's material (the `menu` token: #ececec, #28282a):
/// about 3.5:1 on either appearance's menu.
pub fn disabled_menu_ink() -> &'static str {
    "#7c7c81ff"
}

/// The tokens of one appearance.
pub fn tokens(dark: bool) -> Tokens {
    table()[usize::from(dark)].clone()
}

/// The tokens the current look draws with.
pub fn current() -> Tokens {
    use crate::screens::theme::{current_look, Look};
    match current_look() {
        Look::Named(palette) => match palette.named() {
            Some(colors) => Tokens::named(colors),
            None => tokens(true),
        },
        _ => tokens(crate::screens::theme::resolved() == "dark"),
    }
}

/// Words a Title Case phrase keeps in lower case inside it.
const SMALL_WORDS: &[&str] = &[
    "a", "an", "and", "as", "at", "but", "by", "for", "from", "if", "in", "into", "nor", "of", "on", "or", "per", "the",
    "to", "via", "vs", "with",
];

/// The shell's static words that are commands: its sidebar rows, toolbar tools,
/// menu items and push buttons. Labels and explanations stay in sentence case.
const COMMANDS: &[&str] = &[
    "New chat",
    "Clear search",
    "Add workspace",
    "Settings",
    "Cancel",
    "Save",
    "Copy as Markdown",
    "Review",
    "Take over",
    "Change",
    "Save preferences",
    "Done",
    "Stop server…",
    "Stop server",
    "Advanced…",
    "Open",
    "Edit",
    "Disconnect",
    "Forget server",
    "Details",
    "Session history",
    "Fleet",
    "New chat here",
    "Rename",
    "Remove from sidebar",
];

/// What a desktop says in place of the words the phone and the web share: a Mac's
/// Title Case on a command or a window's title, an ellipsis on a command that
/// asks for more, and plain words for the protocol's. Sources are English, whole
/// and distinctive: a translated string or text a person wrote matches nothing,
/// and the phone never reads this. The phone keeps its words, and so do the tests
/// that lower with the gate off.
const WORDS: &[(&str, &str)] = &[
    ("Saved for this profile:", "Saved for this profile"),
    // A Mac's app reloads its window, not a web page's "app".
    ("Reload app", "Reload"),
    ("Client view unavailable", "OctosCode Stopped Working"),
    ("The client could not recover this view. Closing its connection may have stopped running work. Octos keeps persisted history; unsent drafts and queued messages may be lost when you reload.", "This window couldn\u{2019}t recover. Work in progress may have stopped. Your chats are saved, but unsent drafts and queued messages may be lost when you reload."),
    ("Open saved conversation", "Open Saved Conversation"),
    ("Aside \u{2014} /btw", "Aside"),
    ("Octoscode setup", "OctosCode Setup"),
    ("This conversation link is invalid", "This Conversation Link Is Invalid"),
    (
        "The command is a JSON array of arguments; it is not run through a shell. The monitor polls it on the server.",
        "The command runs directly, not through a shell, and Octos keeps checking it.",
    ),
    ("Filter regex (optional)", "Filter (optional)"),
    (
        "Probe command must be a JSON array of arguments, e.g. [\\\"./scripts/watch.sh\\\", \\\"--verbose\\\"].",
        "Type the command to run, such as ./scripts/watch.sh --verbose.",
    ),
    ("Repository or server-side path", "Repository or folder path"),
    ("New loop", "New Loop"),
    ("New monitor", "New Monitor"),
    ("Self-paced", "Self-Paced"),
    ("Fixed interval", "Fixed Interval"),
    ("Use the server's maintenance prompt", "The usual maintenance prompt"),
    ("Native /loop default. Leave the prompt empty to use the server's maintenance prompt and cadence.", "Leave the prompt empty to use the usual maintenance prompt and schedule."),
    ("The server decides the next run from the loop's result.", "Each run’s result decides when the next one starts."),
    ("Use the native interval syntax, with a whole number and unit: 60s, 5m, 2h, or 1d.", "A whole number and a unit: 60s, 5m, 2h or 1d."),
    ("Creating a loop schedules server-owned work. It does not run in this app, and closing this panel does not stop it.", "A loop keeps running after you close this sheet."),
    ("Reading task snapshots from your confirmed sessions.", "Reading background work from your chats."),
    ("Create your local coding profile", "Create a Coding Profile"),
    ("This follows Octoscode\u{2019}s solo onboarding: create a named profile, test a server-advertised model route, save it, then open the canonical coding session.", "Name a profile and choose a model. OctosCode tests the model, saves the profile and opens a chat."),
    ("Run the canonical setup on the server, then reconnect this workspace.", "Finish the setup on the server with this command, then connect again."),
    ("This server cannot onboard from this app", "This Server Can\u{2019}t Be Set Up Here"),
    ("This server does not support starting peers", "This server doesn't support starting peers."),
    ("Provider catalog unavailable", "Couldn\u{2019}t Load the Providers"),
    ("The server returned an invalid catalog.", "The server\u{2019}s answer couldn\u{2019}t be read."),
    ("Use as the default local profile", "Use as the default profile"),
    ("Manage the active profile\u{2019}s provider routes and credentials. Model selection and the current Session runtime remain separate.", "The providers and API keys this profile can use. Choosing a model for a chat is separate."),
    ("Provider changes update the saved Profile configuration. Restart Octos before relying on route or credential changes in an already bootstrapped runtime.", "A change takes effect the next time Octos starts."),
    ("No model providers configured", "No Model Providers"),
    ("+ New loop", "New Loop…"),
    ("+ New monitor", "New Monitor…"),
    ("Select a candidate first", "Choose a chat first"),
    ("No task snapshots are available yet.", "Background work shows up here."),
    ("LLM", "By Model"),
    ("Heuristic", "By Rules"),
    ("Run native code review", "Run a code review"),
    ("Manage server research provider lanes", "Manage research providers"),
    ("Search activity across confirmed sessions", "Search activity across sessions"),
    ("No MCP servers reported by this runtime.", "No MCP servers are set up."),
    ("Session runtime", "In use by this session"),
    // What the server's configuration can't do.
    ("Core did not report a complete route identity. This entry is read-only.", "This entry can't be edited."),
    ("This entry contains settings the editor cannot preserve. Edit it through Core configuration instead.", "This entry has settings the editor can't keep, so it can't be edited here."),
    ("This configured model contains settings this editor cannot preserve. Edit it through Core configuration instead.", "This model has settings the editor can't keep, so it can't be chosen here."),
    // A row's actions: the agent is the row.
    ("Read status", "Status"),
    ("Read output", "Output"),
    ("List artifacts", "Artifacts"),
    ("Interrupt agent", "Interrupt"),
    // A window's or sheet's title.
    ("Session settings", "Session Settings"),
    ("Review changes", "Review Changes"),
    ("Code review", "Code Review"),
    ("Undo workspace changes", "Undo Workspace Changes"),
    ("Conversation history", "Conversation History"),
    ("Fork conversation", "Fork Conversation"),
    ("Thread graph", "Threads"),
    ("Remembered approvals", "Remembered Approvals"),
    ("Thinking effort", "Thinking Effort"),
    ("Turn images", "Attach Images"),
    (
        "Choose up to four PNG, JPEG, GIF or WebP images. Selecting a file does not upload it; upload explicitly, then send it with this Session's next prompt.",
        "Choose up to four PNG, JPEG, GIF or WebP images, up to 20 MB each. Upload them, and they go with your next message.",
    ),
    ("exists. A retry only repeats provider test and save.", "is saved. Try Again tests the provider again."),
    ("Open a different session", "Open a Different Session"),
    ("Research provider lanes", "Research Models"),
    ("Resume chat", "Resume Chat"),
    ("Choose workspace folder", "Choose Workspace Folder"),
    ("Edit provider", "Edit Provider"),
    ("Add provider", "Add Provider"),
    ("Open a workspace", "Open a Workspace"),
    ("MCP servers", "MCP Servers"),
    ("Stop the Octos server?", "Stop the Octos Server?"),
    ("Forget this server?", "Forget This Server?"),
    (
        "Use a whole-number native interval such as 60s, 5m, or 2h (60 seconds to 24 hours).",
        "Use a whole number and a unit, such as 60s, 5m or 2h (60 seconds to 24 hours).",
    ),
    ("Restoring session state", "Restoring this session"),
    ("Add model provider", "Add Model Provider"),
    ("Delete model provider?", "Delete Model Provider?"),
    ("Session recovery required", "Session Recovery Required"),
    ("Rename workspace", "Rename Workspace"),
    ("Peer controller", "Peer Controller"),
    ("Peer control", "Peer Control"),
    ("Start a peer", "Start a Peer"),
    ("No peers yet", "No Peers"),
    ("Open a project first", "No Project Open"),
    // A command.
    ("Add workspace", "Add Workspace\u{2026}"),
    ("Open a workspace\u{2026}", "Open a Workspace\u{2026}"),
    ("Browse folders\u{2026}", "Browse Folders\u{2026}"),
    ("Pair with a link instead", "Pair with a Link Instead"),
    ("Use local solo server", "Use Local Solo Server"),
    ("Enter server and token instead", "Enter Server and Token Instead"),
    ("Forget this device", "Forget This Device"),
    ("Use this folder", "Use This Folder"),
    ("Create folder", "Create Folder"),
    ("New folder", "New Folder"),
    ("Close images", "Done"),
    ("Cancel uploads and close", "Cancel Uploads"),
    ("Choose image files", "Choose Images\u{2026}"),
    ("Create conversation fork", "Create Fork"),
    ("Start native review", "Start Review"),
    ("Approve once", "Approve Once"),
    ("Approve for session", "Approve for Session"),
    ("Review diff", "Review Diff"),
    ("Submit answer", "Submit Answer"),
    ("Copy diagnostics", "Copy Diagnostics"),
    ("Load more output", "Load More Output"),
    ("Report issue", "Report Issue"),
    ("Hide peers", "Hide Peers"),
    ("Show peers", "Show Peers"),
    ("Release seat", "Release Seat"),
    ("Answer question", "Answer Question"),
    ("Respond to approval", "Respond to Approval"),
    ("Peer gather", "Gather Results"),
    ("Close agent", "Close Agent"),
    ("Manage providers", "Manage Providers\u{2026}"),
    ("Refresh history", "Refresh History"),
    ("Reload history", "Reload History"),
    ("Copy conversation link", "Copy Conversation Link"),
    ("Test route", "Test Connection"),
    ("Discover models", "Find Models"),
    ("Clear lane draft", "Clear"),
    ("Review lane save", "Review\u{2026}"),
    ("Review installation", "Review Installation\u{2026}"),
    // A protocol's or an architecture's words.
    ("Read-only server snapshot. Reading does not change the conversation, queued prompts, or active Session.", "What the server knows about this chat."),
    ("The server could not return a valid inspection for this owner. Retry the read.", "Couldn't read this from the server. Click Refresh to try again."),
    ("This view lists server-owned decisions only. It does not clear or change permissions.", "These are the decisions the server remembers. This doesn't change them."),
    ("No thread graph read yet.", "Nothing read yet."),
    ("Approval scopes were not read.", "Nothing read yet."),
    ("Up to 50 UTF-8 bytes. No #, :, /, control characters, or the reserved name \u{201c}default\u{201d}.", "A short name, without #, : or /."),
    ("Restore server-owned files to a saved snapshot. This can replace workspace changes; conversation messages are not rewound.", "Restore the workspace to a saved snapshot. This can replace changes you made; the conversation stays as it is."),
    ("Run the server's native review specialists on the current project changes. This starts a Session turn; it is not a diff preview.", "Review the current project changes. This starts a turn in this chat."),
    ("Results and any errors appear in this Session. You can queue ordinary prompts while review runs, or stop it using the Session's Stop control.", "Results and any errors appear in this chat. You can queue prompts while it runs, or stop it."),
    ("Named provider routes for the server research pipeline. This is not a separate browser agent loop. Provider identity and API style are separate fields.", "The models the research tools can use, each under its own name."),
    ("Lane key", "Name"),
    ("Add or replace a lane", "Add or Replace a Model"),
    ("Provider identity", "Provider"),
    ("API key environment name", "API Key Variable"),
    ("New credential (optional; otherwise reuse server configuration)", "New API Key"),
    ("Context window (optional)", "Context Window"),
    ("Saving an existing key replaces that lane. Empty optional fields use the server/provider defaults.", "Saving an existing name replaces it. Fields left empty use the provider's defaults."),
    ("No research lanes configured in this Profile.", "No research models are set up yet."),
    ("Couldn't load the research lanes.", "Couldn't load the research models."),
    ("Catalog rows are unverified candidates, not confirmed workspace sessions.", "These chats may not belong to this workspace."),
    ("Refresh the catalog before selecting a row. Bare IDs cannot safely identify a historical conversation.", "Refresh the list, then choose one to confirm it."),
    ("Refresh the catalog before selecting this row.", "Refresh the list before choosing this one."),
    ("A confirmed source Session is required to browse history. Opening does not submit a prompt.", "Opening a chat sends nothing."),
    ("Confirm exact title to resume:", "Confirm title:"),
    ("No matching historical candidates.", "No matching chats."),
    // n11o (E9 9): Resume Chat's waits and failures, as a person would say them.
    ("Reading unverified history candidates…", "Loading chats…"),
    ("Historical identity was not resolved. The listed conversation was not resumed.", "This chat\u{2019}s history couldn\u{2019}t be checked, so it wasn\u{2019}t resumed."),
    ("A history opening is already pending.", "A chat is already opening."),
    ("History listing failed.", "The list of chats couldn\u{2019}t be read."),
    ("Skills provide reusable instructions from SKILL.md, and some add tools. MCP servers and the full tool list have their own inventory.", "Skills give the agent reusable instructions, and some add tools."),
    ("Skills are shared by this Profile, not installed on this device. Installation may download executable tools and dependencies. Review and trust the source first.", "Skills belong to this profile, not this device. Installing one may download programs, so trust its source first."),
    ("Loading, or this server does not offer a session skill catalog.", "Nothing to show for this chat."),
    ("Sends the native agent request through this session\u{2019}s ordinary prompt queue. Available only while idle.", "Sends the request as a prompt in this chat. Available only while idle."),
    ("The status this server reports. Servers are configured on the server.", "What the server reports. They are set up on the server."),
    ("Executable tools available to this session.", "The tools this chat can use."),
    ("The Octos profile backing this session.", "The profile this chat uses."),
    ("Choose System, Light, Dark, or a named dark theme. Save remembers your choice.", "System follows OctoSense; a named theme stays dark."),
    ("Changes apply immediately. Save remembers them on this device; no server configuration, credentials or conversations are stored.", "Changes apply at once. Save Preferences keeps them on this device."),
    ("Choose a folder, and OctosCode starts a session in it.", "Choose a folder, and OctosCode starts a chat in it."),
    ("Stored for this server only", "Kept for this server only"),
    ("This server lists the current workspace only; update the server for history across projects.", "Only this workspace's chats are listed here."),
    ("Items", "Messages"),
    // A question asks; its button says what it does.
    ("Confirm removal", "Remove This Skill?"),
    ("Confirm remove", "Remove"),
    ("Confirm server installation", "Install This Skill?"),
    ("Confirm install", "Install"),
    ("Compact context", "Compact This Chat?"),
    ("Confirm compaction", "Compact"),
    (
        "Compact this session now? Older context may be summarized. The durable transcript remains on the server.",
        "Older context may be summarized. The full transcript stays on the server.",
    ),
    ("Confirm lane removal", "Remove This Research Model?"),
    ("Confirm lane save", "Save This Research Model?"),
    ("Confirm save", "Save"),
    ("Cancel lane change", "Cancel"),
    ("Branch (server default: main)", "Branch (default: main)"),
    // n11o (E9 1): Settings › Connection names the server; its address is here.
    ("Pairing and the saved access token", "The server\u{2019}s address, pairing and the saved access token"),
    // n11o (E9 2): what the server remembers is approvals, in this chat.
    ("No remembered approval scopes for this Session.", "No remembered approvals in this chat."),
];

/// Part of a longer sentence the code writes around a value.
/// A session and a profile are ordinary words inside a sentence.
const IN_A_SENTENCE: &[(&str, &str)] = &[
    (" this Session", " this session"),
    (" the Session", " the session"),
    (" a Session", " a session"),
    (" new Session", " new session"),
    (" another Session", " another session"),
    (" any Session", " any session"),
    (" each Session", " each session"),
    (" peer Session", " peer session"),
    (" this Profile", " this profile"),
    (" the Profile", " the profile"),
];

const PHRASES: &[(&str, &str)] = &[
    // n11o (E9 2): "2 remembered approval scopes for this Session."
    (" remembered approval scopes for this Session.", " remembered approvals in this chat."),
    (" remembered approval scope for this Session.", " remembered approval in this chat."),
    ("Restoring authoritative session state", "Getting the latest from Octos"),
    (" is marked keyless by the Core catalog.", " needs no key."),
    (" is not available in this native build \u{2014} nothing was sent to the model.", " isn\u{2019}t available here. Nothing was sent to the model."),
    (" is not available in this native build", " isn\u{2019}t available here"),
    (" are not supported in this native build.", " aren\u{2019}t supported here."),
    (" hidden by the server", " hidden"),
    (" Session task snapshots unavailable", " sessions couldn\u{2019}t be read"),
];

fn word_table() -> &'static std::collections::HashMap<&'static str, &'static str> {
    static TABLE: std::sync::OnceLock<std::collections::HashMap<&'static str, &'static str>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| WORDS.iter().copied().collect())
}

/// A desktop's word for this English source, if it has its own.
pub fn word(source: &str) -> Option<&'static str> {
    let word = word_table().get(source).copied();
    if hosted() {
        let said = word.unwrap_or(source);
        if let Some((_, there)) = ON_HOST.iter().find(|(from, _)| *from == said) {
            return Some(there);
        }
    }
    word
}

/// What a window on the shell's own kernel says instead, where the sentence
/// speaks of a server. Whole sentences only: a chat's own title is never touched.
const ON_HOST: &[(&str, &str)] = &[
    ("MCP servers are configured on the server. This app shows the status the server reports and can't add or remove servers.", "MCP servers are configured for Octos. This app shows their status and can't add or remove them."),
    ("What the server reports. They are set up on the server.", "What Octos reports. They are set up for Octos."),
    ("What the server knows about this chat.", "What Octos knows about this chat."),
    ("These are the decisions the server remembers. This doesn't change them.", "These are the decisions Octos remembers. This doesn't change them."),
    ("Couldn't read this from the server. Click Refresh to try again.", "Couldn't read this. Click Refresh to try again."),
    ("Profile defaults are read-only on this server.", "Profile defaults are read-only here."),
    ("Not supported by this server", "Not available here"),
    ("Not reported by this server", "Not reported"),
    ("Open new chats in the server's session sandbox", "Open new chats in a session sandbox"),
    ("This server offers no skills, MCP status, tools or memory.", "Octos offers no skills, MCP status, tools or memory here."),
];

/// The permission readback on the shell's kernel, which is no server to a person.
pub fn on_host(text: String) -> String {
    if hosted() {
        text.replacen("Server: ", "In use: ", 1)
    } else {
        text
    }
}

fn reworded(text: &str) -> Option<Cow<'static, str>> {
    if let Some(new) = word(text) {
        return Some(Cow::Borrowed(new));
    }
    let phrased = PHRASES.iter().find(|(from, _)| text.contains(from)).map(|(from, to)| Cow::Owned(text.replacen(from, to, 1)));
    let base: &str = phrased.as_deref().unwrap_or(text);
    if IN_A_SENTENCE.iter().any(|(from, _)| base.contains(from)) {
        let plain = IN_A_SENTENCE.iter().fold(base.to_owned(), |s, (from, to)| s.replace(from, to));
        return Some(Cow::Owned(plain));
    }
    phrased
}

/// A kit's finished surface in a desktop's words: every `text:` literal
/// (`empty_text:` too) that [`WORDS`] names. A phone's passes untouched.
pub fn in_words(dsl: String) -> String {
    if !active() {
        return dsl;
    }
    rewrite_literals(dsl)
}

fn rewrite_literals(dsl: String) -> String {
    const OPEN: &str = "text: \"";
    let bytes = dsl.as_bytes();
    let mut out: Option<String> = None;
    let (mut copied, mut at) = (0, 0);
    while let Some(found) = dsl[at..].find(OPEN) {
        let start = at + found + OPEN.len();
        let mut end = start;
        while end < bytes.len() && bytes[end] != b'"' {
            end += if bytes[end] == b'\\' { 2 } else { 1 };
        }
        if end >= bytes.len() {
            break;
        }
        if let Some(new) = reworded(&dsl[start..end]) {
            let o = out.get_or_insert_with(|| String::with_capacity(dsl.len() + 32));
            o.push_str(&dsl[copied..start]);
            o.push_str(&new);
            copied = end;
        }
        at = end + 1;
    }
    match out {
        Some(mut o) => {
            o.push_str(&dsl[copied..]);
            o
        }
        None => dsl,
    }
}

/// `label` in Title Case. A word of plain lower-case letters takes a capital; a
/// word with a capital, a digit or punctuation inside it (a path, a model's name,
/// any other script) is left as it stands, so this never touches Chinese.
pub fn title_case(label: &str) -> String {
    fn lettered(word: &str) -> bool {
        word.chars().any(|c| c.is_ascii_alphabetic())
    }
    let words: Vec<&str> = label.split(' ').collect();
    let first = words.iter().position(|w| lettered(w));
    let last = words.iter().rposition(|w| lettered(w));
    let mut out = String::with_capacity(label.len());
    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let core = word.trim_end_matches(['…', ',', ':', '?', '.', '!']);
        let plain = !core.is_empty() && core.bytes().all(|b| b.is_ascii_lowercase());
        if plain && (Some(i) == first || Some(i) == last || !SMALL_WORDS.contains(&core)) {
            out.push_str(&word[..1].to_ascii_uppercase());
            out.push_str(&word[1..]);
        } else {
            out.push_str(word);
        }
    }
    out
}

/// A status line a server wrote in lower case, as a desktop's label.
pub fn capital(line: &str) -> std::borrow::Cow<'_, str> {
    if active() && line.starts_with(|c: char| c.is_ascii_lowercase()) {
        std::borrow::Cow::Owned(sentence(line))
    } else {
        std::borrow::Cow::Borrowed(line)
    }
}

/// A wire word as a label: its first letter a capital (`read only` -> `Read only`).
pub fn sentence(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A word the server sent (`approval_gated`, `background_task`) as a desktop
/// shows it, in sentence case with spaces; as it stands on a phone.
pub fn wire_word(word: &str) -> String {
    if active() {
        sentence(&word.replace(['_', '-'], " "))
    } else {
        word.to_owned()
    }
}

/// A tool's wire name as words on a desktop (`apply_patch` -> `Apply patch`), as
/// it stands on a phone. A title with spaces already reads as words.
pub fn tool_word(name: &str) -> String {
    if !active() || name.is_empty() || name.contains(' ') {
        return name.to_owned();
    }
    let words: Vec<&str> = name.split(['_', '-']).filter(|w| !w.is_empty()).collect();
    brand_case(sentence(&words.join(" ")))
}

/// A maker's name as the maker writes it, in a label built from a lower-case id.
pub fn brand_case(label: String) -> String {
    if !active() {
        return label;
    }
    label
        .split(' ')
        .map(|word| match word {
            "Deepseek" => "DeepSeek",
            "Openai" => "OpenAI",
            "Gpt" => "GPT",
            "Glm" => "GLM",
            "Zai" => "Z.AI",
            "Minimax" => "MiniMax",
            "Mcp" => "MCP",
            other => other,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A model as people read it: its id in words. "deepseek-v4-flash" is DeepSeek V4
/// Flash, "claude-3-5-haiku-20241022" Claude 3.5 Haiku, "qwen3-coder" Qwen3 Coder: a
/// route's prefix and a trailing date stamp go, a run of single digits is a version.
pub fn model_word(id: &str) -> String {
    let id = id.rsplit('/').next().unwrap_or(id).trim();
    let parts: Vec<&str> = id.split(['-', '_', ' ']).filter(|p| !p.is_empty()).collect();
    let digit = |p: &str| p.len() == 1 && p.chars().all(|c| c.is_ascii_digit());
    let mut words: Vec<String> = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        let p = parts[i];
        if i > 0 && i + 1 == parts.len() && p.len() == 8 && p.chars().all(|c| c.is_ascii_digit()) {
            break;
        }
        if digit(p) {
            let mut v = p.to_owned();
            while i + 1 < parts.len() && digit(parts[i + 1]) {
                i += 1;
                v.push('.');
                v.push_str(parts[i]);
            }
            words.push(v);
        } else {
            let mut c = p.chars();
            words.push(c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default());
        }
        i += 1;
    }
    if words.is_empty() {
        return id.to_owned();
    }
    let joined = words.join(" ");
    if !active() {
        return joined;
    }
    // "GPT 5.4" is GPT-5.4 and "GLM 4.6" GLM-4.6: the makers' own spelling.
    let mut named = brand_case(joined);
    for brand in ["GPT", "GLM"] {
        for digit in '0'..='9' {
            named = named.replace(&format!("{brand} {digit}"), &format!("{brand}-{digit}"));
        }
    }
    named
}

/// A model's name on a Mac: the server's title when it is a name, else the id in
/// words ([`model_word`]). A title that is only the id, or the route and the id
/// ("local / qwen3-coder"), is no name.
pub fn model_title(title: &str, id: &str) -> String {
    let t = title.trim();
    let named = !t.is_empty() && t != id && !t.contains('/') && t.chars().any(|c| c.is_uppercase() || c.is_whitespace());
    if named {
        t.to_owned()
    } else {
        model_word(if id.trim().is_empty() { t } else { id })
    }
}

/// A button's, menu item's or toolbar word's text: Title Case on a desktop, as it
/// stands on a phone.
pub fn title(label: &str) -> Cow<'_, str> {
    if active() {
        match label {
            // A failure's one action is the same words everywhere.
            "Retry" | "Retry now" => Cow::Borrowed("Try Again"),
            _ => match word(label) {
                Some(new) => Cow::Borrowed(new),
                None => Cow::Owned(title_case(label)),
            },
        }
    } else {
        Cow::Borrowed(label)
    }
}

/// A static label's text for its English `source`: `shown`, in Title Case if a
/// desktop takes that source for a command.
pub fn command<'a>(source: &str, shown: &'a str) -> Cow<'a, str> {
    if !active() {
        return Cow::Borrowed(shown);
    }
    // A translation stands: only the English source has a desktop's own word.
    if let Some(new) = word(source).filter(|_| source == shown) {
        return Cow::Borrowed(new);
    }
    // Asks for the new name first. ("Rename" alone is too common for the table.)
    if source == "Rename" && source == shown {
        return Cow::Borrowed("Rename\u{2026}");
    }
    if COMMANDS.contains(&source) {
        Cow::Owned(title_case(shown))
    } else {
        Cow::Borrowed(shown)
    }
}

/// On a desktop, Fleet is what the sidebar selects while its pane is up.
pub fn fleet_is_selected() -> bool {
    use crate::screens::board3::host;
    active() && host::open_dialog() == Some(host::Dialog::Fleet)
}

/// A workspace as a Mac's toolbar names it: its folder's name, as Finder's and
/// Xcode's windows do (Settings › General has the rest of the path).
pub fn folder_name(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rsplit('/').next() {
        Some(name) if !name.is_empty() => name.to_owned(),
        _ => path.to_owned(),
    }
}

/// A path as a Mac writes it under the person's home: `~/Projects/lumen`.
pub fn home_path(path: &str) -> String {
    let Some(home) = std::env::var("HOME").ok().map(|h| h.trim_end_matches('/').to_owned()).filter(|h| !h.is_empty()) else {
        return path.to_owned();
    };
    if path == home {
        return "~".to_owned();
    }
    match path.strip_prefix(&format!("{home}/")) {
        Some(rest) => format!("~/{rest}"),
        None => path.to_owned(),
    }
}

/// `value` when a desktop draws, else `phone`.
pub fn by<T>(phone: T, value: T) -> T {
    if active() {
        value
    } else {
        phone
    }
}

/// A token of the current look as the colour value a template splices:
/// `#(crate::desk::color("sidebar"))`.
pub fn color(name: &str) -> ScriptValue {
    ScriptValue::from_color(rgba(&token(name)))
}

/// A phone follows an explicit app theme when selected, otherwise the host's
/// actual page and ink. The unhosted preview falls back to the current look.
pub fn phone_color(name: &str) -> ScriptValue {
    let t = match crate::screens::theme::selection().as_str() {
        "light" => Tokens::of(0xffffff, 0x1d1d1f),
        "dark" => Tokens::of(0x1c1f22, 0xf5f5f7),
        "system" => match host_dark() {
            Some(dark) => tokens(dark),
            None => current(),
        },
        _ => current(),
    };
    let value = match name {
        "ink" => t.ink,
        "muted" => {
            let mut muted = t.muted.clone();
            if crate::screens::theme::palette().named().is_some() {
                // The selected row is slightly lighter than a named palette's
                // page. Give its small date label a margin above 4.5:1.
                let base = Rgb::of(rgba(&t.muted) >> 8);
                let ink = Rgb::of(rgba(&t.ink) >> 8);
                let mut mix = 0.0_f64;
                while crate::screens::theme::wcag_ratio(&muted, &t.selected) < 5.0 && mix < 1.0 {
                    mix = (mix + 0.05).min(1.0);
                    muted = base.mix(ink, mix).hex();
                }
            }
            muted
        }
        "selected" => t.selected,
        _ => return color(name),
    };
    ScriptValue::from_color(rgba(&value))
}

/// A token of the current look (`#rrggbb`, a wash `#rrggbbaa`).
pub fn token(name: &str) -> String {
    let t = current();
    match name {
        "page" => t.page,
        "ink" => t.ink,
        "content" => t.content,
        "surface" => t.surface,
        "field" => t.field,
        "hover" => t.hover,
        "border" => t.border,
        "muted" => t.muted,
        "link" => t.link,
        "primary" => t.primary,
        "primary_hover" => t.primary_hover,
        "primary_down" => t.primary_down,
        "accent" => t.accent,
        "success" => t.success,
        "danger" => t.danger,
        "danger_button" => t.danger_button,
        "sidebar" => t.sidebar,
        "bar" => t.bar,
        "selected" => t.selected,
        "button" => t.button,
        "button_hover" => t.button_hover,
        "segment_on" => t.segment_on,
        "switch_off" => t.switch_off,
        "ring" => t.ring,
        "button_down" => t.button_down,
        "well" => t.well,
        "raised" => t.raised,
        "band" => t.band,
        "tint_accent" => t.tint_accent,
        "tint_danger" => t.tint_danger,
        "tint_success" => t.tint_success,
        "edge_danger" => t.edge_danger,
        "amber" => t.amber,
        "tint_amber" => t.tint_amber,
        "edge_amber" => t.edge_amber,
        "wash" => t.wash,
        "wash_down" => t.wash_down,
        "dim" => sheet_dim().to_owned(),
        // Over a toolbar control that can't act: its word and glyph read as off.
        "bar_veil" => format!("{}a6", t.bar.get(..7).unwrap_or("#ffffff")),
        // A menu's material and edge, the framework's (`theme.color_fg_app`,
        // `color_bevel_outset_1` of the Mac themes): the same in every palette.
        "menu" => if t.dark { "#28282a" } else { "#ececec" }.to_owned(),
        "menu_edge" => if t.dark { "#5b5b60" } else { "#bebebe" }.to_owned(),
        "placeholder" => placeholder().to_owned(),
        _ => "#ff00ff".to_owned(),
    }
}

/// A Mac field's placeholder (E7 5): the system's placeholder text colour,
/// black at a quarter in light and white at a quarter in dark, so it reads as
/// a hint and never as typed text. Translucent, it reads the same on any
/// field, and the retint leaves it alone.
pub fn placeholder() -> &'static str {
    if current().dark { "#ffffff40" } else { "#00000040" }
}

/// `#rrggbb[aa]` -> `0xRRGGBBAA`.
fn rgba(hex: &str) -> u32 {
    let h = hex.trim_start_matches('#');
    match h.len() {
        8 => u32::from_str_radix(h, 16).unwrap_or(0xff00ffff),
        _ => (u32::from_str_radix(h.get(0..6).unwrap_or("ff00ff"), 16).unwrap_or(0xff00ff) << 8) | 0xff,
    }
}

/// A `#rrggbb[aa]` token as the colour `script_apply_eval!` takes.
pub fn rgba_value(hex: &str) -> Vec4f {
    Vec4f::from_u32(rgba(hex))
}

/// A dark role (`screens::theme::TOKENS`' right column, the pinned dark
/// literals) as the Mac token of that appearance.
pub fn role_hex(role: &str, dark: bool) -> Option<String> {
    let t = &table()[usize::from(dark)];
    Some(
        match role {
            "#1c1f22" => &t.surface,
            "#1c1c1e" => &t.well,
            "#2c2c2e" => &t.raised,
            "#232629" => &t.band,
            "#38383a" => &t.border,
            "#f5f5f7" | "#e8e8ea" => &t.ink,
            "#98989d" | "#a1a1a6" => &t.muted,
            "#679efe" => &t.link,
            "#86efac" => &t.success,
            "#ff6b6b" => &t.danger,
            "#1d2a40" => &t.tint_accent,
            "#2d1417" => &t.tint_danger,
            "#12261a" => &t.tint_success,
            "#2f6feb" => &t.primary,
            "#1f883d" => &t.success,
            "#cf222e" => &t.danger,
            _ => return None,
        }
        .clone(),
    )
}

/// A kit literal the light-to-dark table has no twin for: its token on a Mac.
/// In dark, a literal a lowering pinned for dark (a dark role) too.
pub fn literal_hex(hex: &str, dark: bool) -> Option<String> {
    let t = &table()[usize::from(dark)];
    let out = match hex {
        // field and control edges
        "#d9d9dc" | "#c7c7cc" | "#d2d2d7" | "#d1d1d6" | "#e5e5ea" | "#dedee0" => &t.border,
        // the board's selected row
        "#efeff1" => &t.selected,
        // accents
        "#2f6feb" | "#1651f3" | "#1f4fb8" => &t.primary,
        "#cf222e" | "#d1242f" | "#c4141b" | "#d93025" | "#ec1313" => &t.danger,
        "#1f883d" | "#22c55e" => &t.success,
        "#eef3fe" | "#eef2fd" => &t.tint_accent,
        "#f4c7c9" | "#f5c2c7" | "#f3c4c7" | "#f2c4c4" => &t.edge_danger,
        "#a35a00" => &t.amber,
        "#fff3e0" => &t.tint_amber,
        "#f3d7a6" => &t.edge_amber,
        _ if dark => return role_hex(hex, dark),
        _ => return None,
    };
    Some(out.clone())
}

/// A lowered surface as a desktop draws it (the kits' literals as tokens, their
/// faces as the theme's). A phone's passes untouched.
pub fn for_mount(ui: &str) -> Cow<'_, str> {
    if !active() {
        return Cow::Borrowed(ui);
    }
    // A fill the board writes in the colour the dark table reads as ink.
    let fills = ui.replace("draw_bg.color: #f5f5f7ff", "draw_bg.color: #f7f7f8ff");
    let fills = if crate::screens::theme::resolved() == "dark" { dark_marks(fills) } else { fills };
    let icons = crate::screens::board3::ui::themed_icons(&fills);
    let tinted = crate::screens::theme::retint_dsl(&icons);
    Cow::Owned(theme_faces(&tinted))
}

/// A selected mark as a dark look draws it: each file's twin in the dark accent.
/// Here, not where a surface is lowered, so a live switch reaches a kept surface.
fn dark_marks(ui: String) -> String {
    const MARKS: [(&str, &str); 4] = [
        ("icons/mac_check_on.svg", "icons/mac_check_on_dark.svg"),
        ("icons/mac_circle_check.svg", "icons/mac_circle_check_dark.svg"),
        ("icons/mac_radio_on.svg", "icons/mac_radio_on_dark.svg"),
        ("icons/mac_step_done.svg", "icons/mac_step_done_dark.svg"),
    ];
    if !ui.contains("icons/mac_") {
        return ui;
    }
    MARKS.iter().fold(ui, |ui, (light, dark)| ui.replace(light, dark))
}

/// Every `FontFamily{…}` the kits write (the Inter and mono files, with their
/// CJK, symbol and emoji members) as the theme's own text family, the one the
/// shell and every other app reads in (PingFang SC on a Mac).
///
/// Only the kits' exact opening matches: a string in the DSL has its quotes
/// escaped, so text a person typed never looks like `file_resource("`.
pub fn theme_faces(dsl: &str) -> String {
    const OPEN: &str = "FontFamily{latin := FontMember{res: file_resource(\"";
    let mut out = String::with_capacity(dsl.len());
    let mut rest = dsl;
    while let Some(at) = rest.find(OPEN) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        // The family ends at the brace that closes its opening one.
        let mut depth = 0i32;
        let mut end = None;
        for (i, b) in tail.bytes().enumerate() {
            match b {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else {
            out.push_str(tail);
            return out;
        };
        let family = &tail[..end];
        out.push_str(theme_family(family));
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// The theme's family for a kit family, by its latin file.
fn theme_family(family: &str) -> &'static str {
    if family.contains("LiberationMono") {
        "theme.font_code.font_family"
    } else if family.contains("Inter-600") || family.contains("Inter-700") {
        "theme.font_bold.font_family"
    } else {
        "theme.font_regular.font_family"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dimmed_option_reads_about_three_and_a_half_to_one_on_a_menu() {
        use crate::screens::theme::wcag_ratio;
        for (menu, dark) in [("#ececec", false), ("#28282a", true)] {
            let r = wcag_ratio(disabled_menu_ink(), menu);
            assert!((3.3..=3.8).contains(&r), "dark={dark}: {r:.2}");
        }
    }

    #[test]
    fn a_file_size_reads_as_finder_says_it() {
        assert_eq!(file_size(1), "1 byte");
        assert_eq!(file_size(67), "67 bytes");
        assert_eq!(file_size(2_048), "2 KB");
        assert_eq!(file_size(98_304), "98 KB");
        assert_eq!(file_size(503_316), "503 KB");
        assert_eq!(file_size(999_600), "1 MB");
        assert_eq!(file_size(1_258_291), "1.3 MB");
        assert_eq!(file_size(2_000_000), "2 MB");
        assert_eq!(file_size(20_971_520), "21 MB");
        assert_eq!(file_size(3_456_000_000), "3.46 GB");
    }

    #[test]
    fn a_model_reads_by_name_never_by_route_and_id() {
        // (Brand case is the Mac's alone; a unit test draws no style.)
        assert_eq!(model_word("claude-3-5-haiku-20241022"), "Claude 3.5 Haiku");
        assert_eq!(model_word("local/qwen3-coder"), "Qwen3 Coder");
        assert_eq!(model_word("kimi-k3"), "Kimi K3");
        assert_eq!(model_word("claude-opus-4-1"), "Claude Opus 4.1");
        assert_eq!(model_title("local / qwen3-coder", "qwen3-coder"), "Qwen3 Coder");
        assert_eq!(model_title("kimi-k3", "kimi-k3"), "Kimi K3");
        assert_eq!(model_title("Kimi K3", "kimi-k3"), "Kimi K3");
        assert_eq!(model_title("", "glm-4.6"), "Glm 4.6");
    }

    #[test]
    fn the_tokens_are_the_interfaces_formulas_on_a_mac_sheet() {
        let light = Tokens::of(LIGHT_PAGE, LIGHT_INK);
        assert_eq!(light.surface, "#ffffff");
        assert_eq!(light.bar, "#ededf0");
        assert_eq!(light.primary, "#0071e3");
        assert_eq!(light.link, "#0060c0");
        assert_eq!(light.content, "#ffffff");
        assert!(!light.dark);
        let dark = Tokens::of(DARK_PAGE, DARK_INK);
        assert_eq!(dark.surface, "#363638");
        assert_eq!(dark.bar, "#303033");
        assert_eq!(dark.link, "#6cb8ff");
        assert_eq!(dark.content, "#1e1e1e");
        assert!(dark.dark);
    }

    #[test]
    fn text_on_the_mac_surfaces_reads_at_four_and_a_half() {
        use crate::screens::theme::wcag_ratio as ratio;
        for t in [Tokens::of(LIGHT_PAGE, LIGHT_INK), Tokens::of(DARK_PAGE, DARK_INK)] {
            for fill in [&t.content, &t.surface, &t.sidebar, &t.bar, &t.selected, &t.well, &t.raised, &t.band] {
                assert!(ratio(&t.ink, fill) >= 4.5, "ink on {fill}");
                assert!(ratio(&t.muted, fill) >= 4.5, "{} on {fill} ({})", t.muted, ratio(&t.muted, fill));
            }
            assert!(ratio(&t.link, &t.surface) >= 4.5);
            assert!(ratio(&t.link, &t.tint_accent) >= 4.5, "link on its tint");
            assert!(ratio(&t.danger, &t.surface) >= 4.5);
            assert!(ratio(&t.danger_button, &t.button) >= 4.5, "danger on a push button");
            assert!(ratio(&t.danger, &t.tint_danger) >= 4.5, "danger on its tint");
            assert!(ratio(&t.success, &t.surface) >= 4.5);
            assert!(ratio(&t.success, &t.tint_success) >= 4.5, "success on its tint");
            assert!(ratio(&t.amber, &t.tint_amber) >= 4.5, "amber on its tint");
            assert!(ratio("#ffffff", &t.primary) >= 4.5, "the default button");
        }
    }

    #[test]
    fn the_accent_reads_as_a_glyph_and_its_white_mark_reads_on_it() {
        use crate::screens::theme::wcag_ratio as ratio;
        for t in [Tokens::of(LIGHT_PAGE, LIGHT_INK), Tokens::of(DARK_PAGE, DARK_INK)] {
            for fill in [&t.content, &t.surface, &t.tint_accent] {
                assert!(ratio(&t.accent, fill) >= 3.0, "{} on {fill}", t.accent);
            }
            assert!(ratio("#ffffff", &t.accent) >= 3.0, "the white mark on {}", t.accent);
        }
        // Each selected mark's dark twin is drawn in that accent.
        let accent = Tokens::of(DARK_PAGE, DARK_INK).accent.to_uppercase();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/icons");
        let ui = "mac_check_on mac_circle_check mac_radio_on mac_step_done"
            .split(' ')
            .map(|m| format!("icons/{m}.svg "))
            .collect::<String>();
        let dark = dark_marks(ui);
        assert_eq!(dark.matches("_dark.svg").count(), 4, "{dark}");
        for rel in dark.split_whitespace() {
            let svg = std::fs::read_to_string(dir.join(rel.trim_start_matches("icons/"))).expect(rel);
            assert!(svg.contains(&format!("fill=\"{accent}\"")), "{rel}: {svg}");
        }
        assert_eq!(dark_marks("icons/mac_step_active.svg".to_owned()), "icons/mac_step_active.svg", "a ring has no white mark");
    }

    #[test]
    fn title_case_takes_commands_and_leaves_names_alone() {
        assert_eq!(title_case("Add workspace"), "Add Workspace");
        assert_eq!(title_case("Pair with a link instead"), "Pair with a Link Instead");
        assert_eq!(title_case("Copy as Markdown"), "Copy as Markdown");
        assert_eq!(title_case("Stop server…"), "Stop Server…");
        assert_eq!(title_case("Browse folders…"), "Browse Folders…");
        assert_eq!(title_case("Back to /home/user/code"), "Back to /home/user/code");
        assert_eq!(title_case("+ New loop"), "+ New Loop");
        assert_eq!(title_case("in progress"), "In Progress");
        assert_eq!(title_case("what to do with"), "What to Do With");
        assert_eq!(title_case("deepseek-v4-flash"), "deepseek-v4-flash");
        assert_eq!(title_case("Stop turn · Esc"), "Stop Turn · Esc");
        assert_eq!(title_case("新建会话"), "新建会话");
        assert_eq!(title_case(""), "");
    }

    #[test]
    fn a_surface_takes_the_desktops_words_and_keeps_everything_else() {
        // A phone's lowering passes through untouched: the gate is false in a unit test.
        assert_eq!(in_words("text: \"Close images\"".to_owned()), "text: \"Close images\"");
        let dsl = "a := Label{text: \"Close images\"} b := TextInput{empty_text: \"Close images\"} c := Label{text: \"A \\\"quoted\\\" word\"}";
        assert_eq!(rewrite_literals(dsl.to_owned()), "a := Label{text: \"Done\"} b := TextInput{empty_text: \"Done\"} c := Label{text: \"A \\\"quoted\\\" word\"}");
        // Only a whole label is a word; a phrase may sit inside a longer one.
        assert_eq!(word("Close images now"), None);
        assert_eq!(rewrite_literals("t := Label{text: \"Sent with this Session's next prompt in the Session.\"}".to_owned()), "t := Label{text: \"Sent with this session's next prompt in the session.\"}");
        assert_eq!(rewrite_literals("t := Label{text: \"The key is marked keyless by the Core catalog.\"}".to_owned()), "t := Label{text: \"The key needs no key.\"}");
    }

    #[test]
    fn a_kit_family_becomes_the_theme_family_and_typed_text_stays() {
        let kit = "draw_text.text_style: TextStyle{font_family: FontFamily{latin := FontMember{res: file_resource(\"/x/ux/Inter-600.ttf\") asc: 0.04 desc: 0.04 weight: 600} cjk := FontMember{res: file_resource(\"/x/c.ttf\") asc: 0.0 desc: 0.0}} font_size: 9.75}";
        let out = theme_faces(kit);
        assert_eq!(out, "draw_text.text_style: TextStyle{font_family: theme.font_bold.font_family font_size: 9.75}");
        let mono = "FontFamily{latin := FontMember{res: file_resource(\"/x/ux/LiberationMono-Regular.ttf\") weight: 400}}";
        assert_eq!(theme_faces(mono), "theme.font_code.font_family");
        // What a person typed is a Debug-quoted string: its quotes are escaped.
        let typed = "text: \"FontFamily{latin := FontMember{res: file_resource(\\\"x\\\")}}\"";
        assert_eq!(theme_faces(typed), typed);
    }
}
