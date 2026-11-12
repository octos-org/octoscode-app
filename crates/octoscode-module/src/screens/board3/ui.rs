//! A4 — the board-3 native surface kit: the renderer's own vocabulary
//! (`DesignSurface`, `Label` with the kit's Inter / Noto Sans SC / mono text family,
//! `DesignNativeButton` tap targets, `TextInput`, `Svg`, `ScrollYView`), laid
//! out as FLOW regions sized from the live window instead of the atlas's
//! frozen 406x776 artboard.
//!
//! Why flow, not the measured card: every board-3 surface is a list the
//! server or the user fills at runtime (tools, servers, threads, checkpoints,
//! sessions, peers) — RULES "Dynamic content -> native flow regions" — and the
//! desktop window (~990x600) is SHORTER than the artboard (776), so a measured
//! card can neither fit the desktop nor narrow to a 360 px phone. The web
//! sizes these dialogs as `width: min(<max>px, 100%)` inside a 16 px backdrop
//! inset with `max-height: calc(100dvh - 32px)` (e.g.
//! `features/inventory/InventoryDialog.module.css:1-19`); [`Frame`] does the
//! same with the module's laid-out rect.
//!
//! Tokens are the board's (`design/stage-a/phase4-new3/atlas-prompt.md`):
//! white / #F7F7F8 surfaces, #E5E5E7 hairlines, #1D1D1F text, #6E6E73
//! secondary, solid black primary pills, blue only for toggles and links.
//!
//! Every tap target is a `DesignNativeButton` block in the exact shape
//! `taps::wired_taps` parses (`name := DesignNativeButton {` … `on_click: ||
//! { NAV(t: "…") }` … `}`), so the mounted dialog publishes its controls
//! through the SAME shared tap path every docked card uses; a per-row control
//! carries its row as the `#<row>` suffix `taps::split_row` decodes (#FX1).
use std::fmt::Write as _;

use crate::i18n::tr;

/// Colours (`#rrggbbaa`, the form the lowered cards use).
///
/// A18 — the TEXT inks meet WCAG 4.5:1 on every fill they are drawn on (the
/// web's e2e `theme.spec.ts:72-112` runs axe's color-contrast rule in light
/// mode and expects zero violations). The board's #6E6E73 secondary read
/// 4.46:1 on `CHIP` and its #A1A1A6 faint 2.57:1 on white, so the light text
/// levels are the web's (`app/theme.css:82-83`): the web keeps no text level
/// below ~5.8:1, its secondary and tertiary are one visual level, and the
/// hierarchy it keeps is primary vs the rest — as here. The pairs are
/// declared (and tested, light and dark) in `screens::theme::CONTRAST_PAIRS`.
pub mod tok {
    pub const TEXT: &str = "#1d1d1fff";
    /// Secondary text: the web's `--dsw-alias-label-secondary` (light).
    pub const MUTED: &str = "#61666bff";
    /// Tertiary text (hints, captions, "not reported", menu titles, group
    /// headers, placeholders): the web's `--dsw-alias-label-tertiary` (light).
    pub const FAINT: &str = "#5f646bff";
    /// The label of a control that cannot be used right now (a disabled or
    /// unarmed button, an unavailable option) — NEVER informational text.
    /// Exempt from 4.5:1 (axe skips disabled controls; WCAG 1.4.3 "inactive
    /// user interface component"); listed in `screens::theme::EXEMPT_INKS`.
    pub const DISABLED_INK: &str = "#a1a1a6ff";
    pub const HAIRLINE: &str = "#e5e5e7ff";
    pub const SURFACE: &str = "#ffffffff";
    pub const SURFACE2: &str = "#f7f7f8ff";
    pub const CHIP: &str = "#f0f0f2ff";
    pub const BLACK: &str = "#000000ff";
    pub const WHITE: &str = "#ffffffff";
    /// The board's blue for fills, toggles, focus rings and selection —
    /// text takes [`BLUE_TEXT`] (#2F6FEB read 4.03:1 on `BLUE_BG`).
    pub const BLUE: &str = "#2f6febff";
    /// Blue TEXT (links, blue chips, notes): the web's link/info blue
    /// `--dsw-alias-state-business-primary` (light), >= 4.57:1 on every fill.
    pub const BLUE_TEXT: &str = "#3564c6ff";
    pub const BLUE_BG: &str = "#eaf1fdff";
    /// The board's green for marks, dots and fills — text takes
    /// [`GREEN_TEXT`] (#1F883D read 3.98:1 on `GREEN_BG`).
    pub const GREEN: &str = "#1f883dff";
    /// Green TEXT: the web's `--dsw-alias-state-success-text` (light).
    pub const GREEN_TEXT: &str = "#166534ff";
    pub const GREEN_BG: &str = "#e6f4eaff";
    /// The board's red for fills, dots and icons — text takes [`RED_TEXT`].
    pub const RED: &str = "#cf222eff";
    /// Red TEXT (errors, destructive links, red chips): the web's
    /// `--dsw-alias-state-error-text` (light); its dark twin #FF6B6B keeps an
    /// error readable on a dark surface (#CF222E read 3.27:1 on the dark
    /// `RED_BG`).
    pub const RED_TEXT: &str = "#c50f0fff";
    pub const RED_BG: &str = "#fdececff";
    pub const AMBER: &str = "#a35a00ff";
    pub const AMBER_BG: &str = "#fff3e0ff";
    pub const AMBER_LINE: &str = "#f3d7a6ff";
    pub const DISABLED_BG: &str = "#e9e9ebff";
    pub const MASK: &str = "#0000004d";
    pub const TRANSPARENT: &str = "#00000000";
}

/// A text face of the kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Regular,
    Medium,
    Semibold,
    Mono,
}

/// The renderer's own text family (`octoscript-makepad design.rs:732`): the
/// kit face for latin, Noto Sans SC for CJK with LXGW WenKai as the lazy rare-glyph
/// fallback (without a CJK member every Chinese glyph is
/// a font miss), the symbols face, the platform emoji face. Sizes are design
/// pixels; the renderer emits `font_size = px * 0.75` and so do we, so a
/// board-3 label and a lowered card label of the same px match exactly.
pub fn text_style(face: Face, px: f64) -> String {
    let (file, weight) = match face {
        Face::Regular => ("ux/Inter-400.ttf", 400),
        Face::Medium => ("ux/Inter-500.ttf", 500),
        Face::Semibold => ("ux/Inter-600.ttf", 600),
        Face::Mono => ("ux/LiberationMono-Regular.ttf", 400),
    };
    let latin = crate::design::font_file(file);
    // Noto Sans SC first, LXGW WenKai as the lazy rare-glyph fallback (operator, board 4).
    let cjk_members = crate::design::cjk_members(weight as u32);
    let emoji = if cfg!(target_os = "macos") {
        "file_resource(\"/System/Library/Fonts/Apple Color Emoji.ttc\")".to_owned()
    } else {
        "crate_resource(\"makepad_widgets:resources/NotoColorEmoji.ttf\")".to_owned()
    };
    let (asc, desc) = if face == Face::Mono { (0.0, 0.0) } else { (0.04, 0.04) };
    format!(
        "TextStyle{{font_family: FontFamily{{latin := FontMember{{res: file_resource({latin:?}) asc: {asc} desc: {desc} weight: {weight}}} {cjk_members} symbols := FontMember{{res: crate_resource(\"makepad_widgets:resources/jetbrains_mono_variable.ttf\") asc: 0 desc: 0 weight: 400}} emoji := FontMember{{res: {emoji} asc: 0 desc: 0}}}} font_size: {} line_spacing: 1.25}}",
        fmt_num(px * 0.75)
    )
}

fn fmt_num(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

/// An estimate of a single-line run's width in design px (Inter's advance
/// widths by glyph class; mono is 0.6em). A `Fit` box whose only sizing
/// child is an Overlay `Fill` tap target measured 0 wide (the /snap of the
/// first build: `b3_insp_copy_btn_box [0,0,0,0]`), so pills and links carry
/// an explicit width from this estimate instead.
pub fn text_w(s: &str, px: f64, face: Face) -> f64 {
    let em: f64 = s.chars().map(|c| char_em(c, face)).sum();
    (em * px * weight_factor(face)).ceil()
}

/// One character's advance in em (see [`text_w`]). A CJK / full-width
/// character is one em in every face: the family's CJK members (Noto Sans SC, WenKai) draw
/// it, mono runs included.
pub fn char_em(c: char, face: Face) -> f64 {
    if (c as u32) > 0x2e80 {
        return 1.0;
    }
    if face == Face::Mono {
        return 0.6;
    }
    match c {
        'i' | 'l' | 'j' | '.' | ',' | '\'' | '|' | ':' | ';' | '!' | 'I' => 0.27,
        ' ' | 'f' | 't' | 'r' | '(' | ')' | '/' | '-' => 0.34,
        'm' | 'w' => 0.86,
        'M' | 'W' => 0.92,
        'A'..='Z' => 0.68,
        '0'..='9' => 0.58,
        '…' => 0.9,
        _ => 0.56,
    }
}

/// The heavier faces' advance factor.
pub fn weight_factor(face: Face) -> f64 {
    match face {
        Face::Semibold => 1.05,
        Face::Medium => 1.025,
        _ => 1.0,
    }
}

/// A18 — the kit's icons carry a light-theme stroke in their files (the
/// thinking block's `b3_chevron_*_dark.svg` is #1D1D1F-ish): on a surface that
/// follows the theme (the transcript rows), a dark palette would draw them
/// dark-on-dark, so every icon there takes the dark secondary ink
/// (`DrawSvg.color` replaces the geometry's colour — `components.rs`
/// `answer_actions_ink` does the same for the answer actions). Light is the
/// byte passthrough. The ink is declared in `screens::theme::CONTRAST_PAIRS`
/// (a UI glyph, 3:1).
pub const DARK_ICON_INK: &str = "#98989dff";

pub fn themed_icons(dsl: &str) -> String {
    if crate::screens::theme::resolved() != "dark" {
        return dsl.to_owned();
    }
    // A26: a named display palette tints them its own muted grey.
    let ink = crate::screens::theme::icon_ink();
    let mut out = String::with_capacity(dsl.len() + 64);
    for line in dsl.lines() {
        tint_icon_line(line, &ink, &mut out);
        out.push('\n');
    }
    if !dsl.ends_with('\n') {
        out.pop();
    }
    out
}

/// A dialog's scope line that names a Session, a profile or their ids:
/// developer text on a desktop, where the sheet's title and its note say the
/// same in words. Settings shows the profile where it matters.
fn hidden_scope(id: &str) -> bool {
    id.ends_with("_scope") && id != "b3_vim_scope"
}

/// One line of [`themed_icons`]: the ink goes inside the icon's own braces when
/// the line closes them (the board-1 kit writes a whole `Svg { … }` per line,
/// and after its `}` the property would land on the parent view), and at the
/// line's end when the icon goes on below.
fn tint_icon_line(line: &str, ink: &str, out: &mut String) {
    // A Mac's selected mark is two colours: one ink would flatten it.
    const OWN_COLOURS: &str = "icons/mac_";
    const KEY: &str = "draw_svg.svg:";
    let mut rest = line;
    while let Some(at) = rest.find(KEY) {
        let Some(close) = rest[at..].find('}').map(|n| at + n) else {
            break;
        };
        let open = rest[..at].rfind('{').map_or(0, |n| n + 1);
        out.push_str(&rest[..close]);
        if !rest[open..close].contains("draw_svg.color:") && !rest[open..close].contains(OWN_COLOURS) {
            out.push_str("draw_svg.color: ");
            out.push_str(ink);
            out.push(' ');
        }
        rest = &rest[close..];
    }
    out.push_str(rest);
    if crate::desk::active() && rest.contains("b3_shield_danger") && !rest.contains("draw_svg.color:") {
        // The dangerous permission stays red in dark.
        out.push_str(" draw_svg.color: ");
        out.push_str(&crate::desk::current().danger);
    } else if rest.contains(KEY) && !rest.contains("draw_svg.color:") && !rest.contains(OWN_COLOURS) {
        out.push_str(" draw_svg.color: ");
        out.push_str(ink);
    }
}

/// Truncate `s` with an ellipsis so its estimated run fits `px_budget`
/// (per-character advances, so a CJK line is cut where it really ends).
pub fn fit_w(s: &str, px_budget: f64, px: f64, face: Face) -> String {
    // A small margin for the estimate's error (measured: LXGW's full-width
    // advance is ~1.02 em at 13 px).
    let budget = px_budget * 0.97;
    if text_w(s, px, face) <= budget {
        return s.to_owned();
    }
    let k = px * weight_factor(face);
    let ell = char_em('…', face) * k;
    let mut out = String::new();
    let mut w = 0.0;
    for c in s.chars() {
        let cw = char_em(c, face) * k;
        if w + cw + ell > budget {
            break;
        }
        out.push(c);
        w += cw;
    }
    let mut out = out.trim_end().to_owned();
    out.push('…');
    out
}

/// A13 — shorten `s` IN THE MIDDLE so its estimated run fits `px_budget`:
/// the head and the tail stay (a link keeps its scheme and the session id at
/// its end), joined by one `…`. Unchanged when it already fits.
pub fn fit_middle(s: &str, px_budget: f64, px: f64, face: Face) -> String {
    let budget = px_budget * 0.97;
    if text_w(s, px, face) <= budget {
        return s.to_owned();
    }
    let k = px * weight_factor(face);
    let room = (budget - char_em('…', face) * k).max(0.0);
    let chars: Vec<char> = s.chars().collect();
    // The head takes up to two thirds of the room (a link's scheme and path
    // name what it is); the tail takes what is left.
    let mut head = 0usize;
    let mut used = 0.0;
    while head < chars.len() {
        let cw = char_em(chars[head], face) * k;
        if used + cw > room * 2.0 / 3.0 {
            break;
        }
        used += cw;
        head += 1;
    }
    let mut tail = chars.len();
    while tail > head {
        let cw = char_em(chars[tail - 1], face) * k;
        if used + cw > room {
            break;
        }
        used += cw;
        tail -= 1;
    }
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(chars[tail..].iter());
    out
}

/// A13 (judge: raw protocol errors in dialogs) — is this error text
/// developer wording rather than a sentence written for people? The
/// client's own errors carry the JSON-RPC method and a transport wrapper
/// (`octoscode_client::ClientError`: `{method}: rpc error …`, `{method}: bad
/// result: …`, `{method}: transport: …`); the web's scope checks throw
/// "Invalid or wrong-scope …" (`packages/client/src/inventory.ts:179-190`).
pub fn is_protocol_error(s: &str) -> bool {
    let t = s.trim();
    let method_prefix = t.split_once(": ").is_some_and(|(head, _)| {
        head.contains('/')
            && !head.starts_with('/')
            && head.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '/' | '_' | '.' | '-'))
    });
    method_prefix
        || ["rpc error", "bad result", ": transport: ", "wrong-scope", "missing field", "invalid type", "unknown variant"]
            .iter()
            .any(|k| t.contains(k))
}

/// A13 — a failure as people read it: a plain-language lead naming what
/// failed (`lead`), then the cause the web would print (`cause.message`,
/// e.g. `InventoryDialog.tsx:56-61`, capped at 512 characters like the web)
/// on a smaller muted line under it, so the information stays but no longer
/// leads. Ids: `{id}` is the lead, `{id}_detail` the cause.
pub fn failure(d: &mut Dsl, id: &str, lead: &str, cause: &str) {
    let cause = clean_cause(cause);
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 2");
    d.text(id, tr(lead), &Txt::new(13.0, Face::Medium, tok::RED_TEXT).w(W::Fill).wrap());
    if !cause.is_empty() && cause != lead {
        if crate::desk::active() {
            d.unseen(&format!("{id}_detail"), &cause);
        } else {
            d.text(&format!("{id}_detail"), &cause, &Txt::new(11.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
        }
    }
    d.close();
}

/// A Mac's empty, loading or failed state: centred in its pane, a glyph, a
/// title, what to do next, and at most one action `(id, label, event)`.
pub fn mac_state(d: &mut Dsl, id: &str, glyph: &str, title: &str, note: Option<(&str, &str)>, action: Option<(&str, &str, &str)>) {
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 6 align: Align{x: 0.5 y: 0.0} padding: Inset{left: 16 right: 16 top: 28 bottom: 28}");
    let mark = d.anon();
    d.view(&mark, "width: Fill height: Fit flow: Right align: Align{x: 0.5 y: 0.5}");
    d.icon(&format!("{id}_mark"), glyph, 36.0, tok::MUTED);
    d.close();
    d.text(id, title, &Txt::new(15.0, Face::Semibold, tok::TEXT).w(W::Fill).wrap().centre());
    if let Some((note_id, words)) = note {
        d.text(note_id, words, &Txt::new(13.0, Face::Regular, tok::MUTED).w(W::Fill).wrap().centre());
    }
    if let Some((action_id, label, event)) = action {
        let row = d.anon();
        d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.5 y: 0.5} padding: Inset{top: 4}");
        d.button(action_id, label, event, Btn::Outline, W::Fit, 28.0);
        d.close();
    }
    d.close();
}

/// A failed read with its way back: [`failure`] on a phone, and on a Mac the
/// centred state with Try Again (the cause stays in the tree for the tests).
pub fn failure_state(d: &mut Dsl, id: &str, lead: &str, cause: &str, retry: (&str, &str)) {
    if !crate::desk::active() {
        failure(d, id, lead, cause);
        return;
    }
    let shown = tr(lead);
    mac_state(d, id, "b3_warning.svg", shown.strip_suffix('.').unwrap_or(shown), None, Some((retry.0, tr("Try again"), retry.1)));
    let cause = clean_cause(cause);
    if !cause.is_empty() && cause != lead {
        d.unseen(&format!("{id}_detail"), &cause);
    }
}

/// A13 — an error line that may be either: developer wording
/// ([`is_protocol_error`]) gets the plain `lead` over it ([`failure`]); a
/// message already written for people ("History belongs to another
/// Session.") shows alone, red, as before.
pub fn error_line(d: &mut Dsl, id: &str, lead: &str, msg: &str) {
    if is_protocol_error(msg) {
        failure(d, id, lead, msg);
    } else {
        // A24: a message written for people is product copy (its key);
        // server text is no key and reads as written.
        d.text(id, &clean_cause(tr(msg)), &Txt::new(12.5, Face::Regular, tok::RED_TEXT).w(W::Fill).wrap());
    }
}

/// A13 — a dialog's error: when the dialog recorded what failed for THIS
/// error text (`failed` = the plain lead and the cause it was recorded
/// with), the lead with the cause muted under it ([`failure`]); otherwise
/// [`error_line`] (an error set elsewhere never inherits a stale lead).
pub fn dialog_error(d: &mut Dsl, id: &str, e: &str, failed: Option<&(&'static str, String)>, fallback: &str) {
    match failed.filter(|(_, cause)| cause == e) {
        Some((lead, _)) => failure(d, id, lead, e),
        None => error_line(d, id, fallback, e),
    }
}

/// Whitespace collapsed (the web renders the cause in a `<p>`), capped at
/// 512 characters (`.slice(0, 512)`).
fn clean_cause(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(512).collect()
}

/// Escape a runtime string for a DSL string literal (`text: "…"`). Debug
/// formatting escapes quotes, backslashes and control characters.
pub fn lit(s: &str) -> String {
    format!("{s:?}")
}

/// Sanitise a free-form key into a widget-id fragment (`[a-z0-9_]`).
pub fn idfrag(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect();
    out.truncate(24);
    out
}

/// A text run's look.
#[derive(Debug, Clone)]
pub struct Txt {
    pub px: f64,
    pub face: Face,
    pub color: &'static str,
    /// `Fit` (single line, natural width), `Fill` or a fixed width.
    pub width: W,
    /// Wrap onto several lines (needs a non-Fit width).
    pub wrap: bool,
    /// Lines centred in their width (an empty state's words).
    pub centre: bool,
    /// One line, cut with an ellipsis when its width runs out.
    pub elide: bool,
}

impl Txt {
    pub fn new(px: f64, face: Face, color: &'static str) -> Self {
        Self { px, face, color, width: W::Fit, wrap: false, centre: false, elide: false }
    }
    pub fn w(mut self, width: W) -> Self {
        self.width = width;
        self
    }
    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }
    pub fn centre(mut self) -> Self {
        self.centre = true;
        self
    }
    pub fn elide(mut self) -> Self {
        self.elide = true;
        self
    }
}

/// The type ramp (design px). Title/section/body/meta/micro per the board.
pub fn title() -> Txt {
    Txt::new(crate::desk::by(17.0, 15.0), Face::Semibold, tok::TEXT)
}
pub fn heading() -> Txt {
    Txt::new(13.0, Face::Semibold, tok::TEXT)
}
pub fn body() -> Txt {
    Txt::new(13.0, Face::Regular, tok::TEXT)
}
pub fn body_medium() -> Txt {
    Txt::new(13.0, Face::Medium, tok::TEXT)
}
pub fn meta() -> Txt {
    Txt::new(12.0, Face::Regular, tok::MUTED)
}
pub fn micro() -> Txt {
    Txt::new(11.0, Face::Regular, tok::MUTED)
}
pub fn mono() -> Txt {
    Txt::new(12.0, Face::Mono, tok::TEXT)
}

/// A walk dimension.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum W {
    Fit,
    Fill,
    Px(f64),
}

impl W {
    fn dsl(self) -> String {
        match self {
            W::Fit => "Fit".into(),
            W::Fill => "Fill".into(),
            W::Px(v) => fmt_num(v),
        }
    }
}

/// Where a control sits on a Mac sheet's foot ([`Dsl::foot_begin`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Foot {
    /// The leading edge: a secondary action (Reload, Copy as Markdown).
    Lead,
    /// The trailing edge: the sheet's own Cancel or Close, before the default.
    Trail,
    /// The one default action, last; the close control then says Cancel.
    Default,
}

/// A Mac sheet's foot, gathered while its body is built and drawn by
/// [`shell_close`]: a sheet closes with a push button at its bottom right,
/// never a glyph in its title row.
#[derive(Default)]
struct SheetFoot {
    close: Option<(String, String)>,
    lead: String,
    trail: String,
    has_default: bool,
    own_close: bool,
    held: Option<(Foot, String)>,
    /// A sheet with no padding of its own: the foot brings its own, under a
    /// hairline (its body fills the sheet and scrolls).
    inset: Option<f64>,
    /// The body may scroll: a hairline ends it above the foot.
    scrolls: bool,
    /// What Return does on this sheet when its default is an action of its own
    /// (a list's Open), not its Done.
    default_event: Option<String>,
    /// A manager's sheet keeps "Done" beside its default: nothing to cancel.
    keep_done: bool,
    /// n11o (E9 5): what floats over the sheet (a pop-up's menu), drawn after
    /// it in the root's overlay ([`Dsl::over_begin`]).
    over: String,
    over_held: Option<String>,
}

/// What Return answers on the open Mac sheet when its blue Done is its only
/// action: the close event, and the sheet's fields, which keep Return for
/// themselves while one holds the keyboard.
static SHEET_DONE: std::sync::Mutex<Option<(String, Vec<String>)>> = std::sync::Mutex::new(None);

/// A Mac sheet's list whose scroll area ends on a row, not across one: the
/// lowering names the rows ([`end_scroll_on_rows`]), the host measures a layout
/// and caps the area at the top of the row its edge cut ([`set_row_edge`]).
#[derive(Clone, Debug, PartialEq)]
pub struct RowEdge {
    pub scroll: String,
    /// The rows' ids: `{prefix}0`, `{prefix}1`, …
    pub prefix: String,
    pub rows: usize,
    /// The area's room before any cap.
    pub room: f64,
}

static ROWS_NEXT: std::sync::Mutex<Option<(String, usize)>> = std::sync::Mutex::new(None);
static ROW_EDGES: std::sync::Mutex<Vec<(RowEdge, Option<f64>)>> = std::sync::Mutex::new(Vec::new());

/// The rows of the list the next [`body_open_id`] scrolls.
pub fn end_scroll_on_rows(prefix: &str, rows: usize) {
    if crate::desk::active() {
        *ROWS_NEXT.lock().unwrap() = Some((prefix.to_owned(), rows));
    }
}

/// The lists not measured yet.
pub fn row_edges_pending() -> Vec<RowEdge> {
    ROW_EDGES.lock().unwrap().iter().filter(|(_, c)| c.is_none()).map(|(e, _)| e.clone()).collect()
}

/// The host measured `edge`: its area ends `cap` points below its top.
pub fn set_row_edge(edge: &RowEdge, cap: f64) {
    for (e, c) in ROW_EDGES.lock().unwrap().iter_mut() {
        if e == edge {
            *c = Some(cap);
        }
    }
}

/// A sheet that went: its list is measured afresh when it opens again.
pub fn forget_row_edge(scroll: &str) {
    ROW_EDGES.lock().unwrap().retain(|(e, _)| e.scroll != scroll);
}

/// [`body_open_id`]'s room for `scroll`, capped where a measure said.
fn row_room(scroll: &str, room: f64) -> f64 {
    let rows = ROWS_NEXT.lock().unwrap().take();
    let mut edges = ROW_EDGES.lock().unwrap();
    let Some((prefix, rows)) = rows.filter(|(_, n)| *n > 1) else {
        edges.retain(|(e, _)| e.scroll != scroll);
        return room;
    };
    let edge = RowEdge { scroll: scroll.to_owned(), prefix, rows, room };
    if let Some((_, cap)) = edges.iter().find(|(e, _)| *e == edge) {
        return cap.map_or(room, |c| room.min(c));
    }
    edges.retain(|(e, _)| e.scroll != scroll);
    edges.push((edge, None));
    room
}

/// See [`SHEET_DONE`].
pub fn sheet_done() -> Option<(String, Vec<String>)> {
    SHEET_DONE.lock().unwrap().clone()
}

/// The surface now drawn in the sheet's family has no blue Done (a menu).
pub fn no_sheet_done() {
    *SHEET_DONE.lock().unwrap() = None;
}

/// The last press on a Mac list's row: (list, row, when).
static LAST_ROW_PRESS: std::sync::Mutex<Option<(String, usize, std::time::Instant)>> = std::sync::Mutex::new(None);

/// A second press on the same row of `list` soon after the first: a double click,
/// which does the row's own action.
pub fn second_click(list: &str, row: usize) -> bool {
    let mut last = LAST_ROW_PRESS.lock().unwrap_or_else(|p| p.into_inner());
    let now = std::time::Instant::now();
    let double = matches!(&*last, Some((l, r, at)) if l == list && *r == row && now.duration_since(*at).as_millis() <= 450);
    *last = if double { None } else { Some((list.to_owned(), row, now)) };
    double
}

/// The selected row's band in a Mac list. A sheet is the key window while it
/// is up, so its list's selection is the accent (the system's selected
/// content colour), with white words ([`Dsl::accent_since`]).
pub fn selection_fill() -> String {
    if crate::desk::current().dark { "#0058d0ff" } else { "#0063e1ff" }.to_owned()
}

/// The words a selected row says on the accent: white, its secondary words a
/// shade under (4.5:1 on either appearance's accent).
pub const ON_ACCENT: &str = "#fffffeff";
pub const ON_ACCENT_MUTED: &str = "#fffffee6";

/// A Mac key list's selected row (what [`Dsl::accent_since`] rewrites): the
/// words and marks the row itself carries turn white; what sits on its own
/// fill inside it (a push button, a dot) keeps its colours.
pub fn on_accent(dsl: &str) -> String {
    let tk = crate::desk::current();
    let muted = [tok::MUTED, tok::FAINT, tk.muted.as_str()];
    let mut out = String::with_capacity(dsl.len() + 64);
    let mut depth = 0usize;
    // The open widgets' kinds, innermost last, and the depth of a filled
    // surface nested in the row (its inks are its own).
    let mut kinds: Vec<String> = Vec::new();
    let mut band_seen = false;
    let mut own_fill: Option<usize> = None;
    for line in dsl.lines() {
        let t = line.trim();
        if t.ends_with('{') && t.contains(" := ") {
            depth += 1;
            kinds.push(t.split(" := ").nth(1).unwrap_or("").trim_end_matches('{').trim().to_owned());
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if t == "}" {
            if own_fill == Some(depth) {
                own_fill = None;
            }
            depth = depth.saturating_sub(1);
            kinds.pop();
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let kind = kinds.last().map(String::as_str).unwrap_or("");
        if kind == "DesignSurface" && t.contains("show_bg: true draw_bg.color: ") && !t.contains("draw_bg.color: #00000000") {
            if !band_seen {
                band_seen = true;
            } else if own_fill.is_none() {
                own_fill = Some(depth);
            }
        }
        if own_fill.is_some() {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let mut l = line.to_owned();
        if let Some(at) = l.find("draw_text.color: ") {
            let from = at + "draw_text.color: ".len();
            let to = l[from..].find(|c: char| c.is_whitespace()).map_or(l.len(), |n| from + n);
            let ink = if muted.contains(&&l[from..to]) { ON_ACCENT_MUTED } else { ON_ACCENT };
            l.replace_range(from..to, ink);
        }
        // A Mac's own two-colour marks (a box, a radio) keep their colours.
        if kind == "Svg" && l.contains("draw_svg.svg:") && !l.contains("icons/mac_") {
            if let Some(at) = l.find("draw_svg.color: ") {
                let from = at + "draw_svg.color: ".len();
                let to = l[from..].find(|c: char| c.is_whitespace()).map_or(l.len(), |n| from + n);
                l.replace_range(from..to, ON_ACCENT);
            } else {
                l.push_str(" draw_svg.color: ");
                l.push_str(ON_ACCENT);
            }
        }
        out.push_str(&l);
        out.push('\n');
    }
    if !dsl.ends_with('\n') {
        out.pop();
    }
    out
}

/// A menu item's words: a button's words on a Mac (Title Case, the language's own).
pub fn menu_word(source: &str) -> String {
    crate::desk::title(tr(source)).into_owned()
}

/// The selection a Mac list shows: the one chosen, else `fallback`, else the first row.
pub fn selection_of(chosen: Option<usize>, fallback: Option<usize>, rows: usize) -> Option<usize> {
    chosen.filter(|i| *i < rows).or(fallback.filter(|i| *i < rows)).or((rows > 0).then_some(0))
}

/// The selection one step up (`-1`) or down (`1`), held at the list's ends.
pub fn step_selection(current: Option<usize>, step: i32, rows: usize) -> Option<usize> {
    if rows == 0 {
        return None;
    }
    Some(match current {
        None => 0,
        Some(i) if step < 0 => i.saturating_sub(1),
        Some(i) => (i + 1).min(rows - 1),
    })
}

/// The line-based DSL builder. Blocks open with `<id> := <Kind> {` on their
/// own line and close with a lone `}`, which is the shape
/// `taps::wired_taps` reads.
#[derive(Default)]
pub struct Dsl {
    out: String,
    seq: usize,
    /// `Some` while a Mac sheet is open (always `None` on a phone).
    sheet: Option<SheetFoot>,
    /// The next button's label is data (a model's name), not a command's words.
    verbatim: bool,
    /// The next button's word, when it isn't the kind's (a red word).
    word_ink: Option<&'static str>,
    /// A Mac sheet has drawn its one default in the accent.
    primary_drawn: bool,
    /// (widget id, routed event) for every tap target emitted — the same pairs
    /// `taps::wired_taps` recovers from the text (a test pins the equality).
    pub taps: Vec<(String, String)>,
    /// Text inputs emitted: (widget id, input key).
    pub inputs: Vec<(String, String)>,
}

impl Dsl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn finish(self) -> String {
        crate::desk::in_words(self.out)
    }

    /// Where what is built next starts ([`Dsl::accent_since`]).
    pub fn mark(&self) -> usize {
        self.out.len()
    }

    /// A Mac key list's selected row, built since `at`, is drawn on the
    /// accent: its own words and marks turn white ([`on_accent`]).
    pub fn accent_since(&mut self, at: usize) {
        if at <= self.out.len() && self.out.is_char_boundary(at) {
            let row = self.out.split_off(at);
            self.out.push_str(&on_accent(&row));
        }
    }

    /// On a Mac sheet, what is built until [`Dsl::foot_end`] goes to the
    /// sheet's foot. Anywhere else, it stays where it is.
    pub fn foot_begin(&mut self, at: Foot) {
        if let Some(sheet) = &mut self.sheet {
            sheet.held = Some((at, std::mem::take(&mut self.out)));
        }
    }

    /// n11o (E9 5): on a Mac sheet, what is built until [`Dsl::over_end`]
    /// floats over the whole sheet, as a pop-up's menu does: [`shell_close`]
    /// draws it after the sheet. Anywhere else, it stays where it is.
    pub fn over_begin(&mut self) {
        if let Some(sheet) = &mut self.sheet {
            sheet.over_held = Some(std::mem::take(&mut self.out));
        }
    }

    /// See [`Dsl::over_begin`].
    pub fn over_end(&mut self) {
        if let Some(sheet) = &mut self.sheet {
            if let Some(out) = sheet.over_held.take() {
                let built = std::mem::replace(&mut self.out, out);
                sheet.over.push_str(&built);
            }
        }
    }

    /// See [`Dsl::foot_begin`].
    pub fn foot_end(&mut self) {
        if let Some(sheet) = &mut self.sheet {
            if let Some((at, out)) = sheet.held.take() {
                let built = std::mem::replace(&mut self.out, out);
                match at {
                    Foot::Lead => sheet.lead.push_str(&built),
                    Foot::Trail => sheet.trail.push_str(&built),
                    Foot::Default => {
                        sheet.trail.push_str(&built);
                        sheet.has_default = true;
                    }
                }
            }
        }
    }

    /// Return on this Mac sheet does `event`, its blue default in the foot.
    pub fn foot_default_event(&mut self, event: &str) {
        if let Some(sheet) = &mut self.sheet {
            sheet.default_event = Some(event.to_owned());
        }
    }

    /// The sheet's close still says Done beside a default (a list it manages).
    pub fn foot_keep_done(&mut self) {
        if let Some(sheet) = &mut self.sheet {
            sheet.keep_done = true;
        }
    }

    /// A Mac list row's right-click menu (GestureView's): `items` in order, each a
    /// title and the event it routes, `("-", "")` a rule between groups. The rows'
    /// own press stays with what is inside. Close it with [`Dsl::close`]. Standalone,
    /// on the plain makepad fork, GestureView has no menu: the row is only its contents.
    pub fn menu_open(&mut self, id: &str, items: &[(String, String)]) {
        let titles: Vec<String> = items.iter().map(|(t, _)| format!("{t:?}")).collect();
        // `if a { … } else { if b { … } }`: each choice routes its own event.
        let mut arms = String::new();
        for (t, ev) in items.iter().rev().filter(|(t, _)| t != "-") {
            let route = format!("NAV(t: {:?})", format!("menu:{ev}"));
            arms = if arms.is_empty() {
                format!("if title == {t:?} {{ {route} }}")
            } else {
                format!("if title == {t:?} {{ {route} }} else {{ {arms} }}")
            };
        }
        let _ = writeln!(self.out, "{id} := GestureView {{");
        self.raw("width: Fill height: Fit flow: Down");
        if crate::host_makepad::ROW_MENUS {
            let _ = writeln!(self.out, "menu: [{}]", titles.join(", "));
            let _ = writeln!(self.out, "on_menu: |title| {{ {arms} }}");
        }
    }

    /// This Mac sheet draws its own Cancel or Close in its foot.
    pub fn foot_own_close(&mut self) {
        if let Some(sheet) = &mut self.sheet {
            sheet.own_close = true;
        }
    }

    /// A fresh anonymous id (`b3n_<n>`), so every node is inspectable in /snap.
    pub fn anon(&mut self) -> String {
        self.seq += 1;
        format!("b3n_{}", self.seq)
    }

    pub fn raw(&mut self, line: &str) {
        self.out.push_str(line);
        self.out.push('\n');
    }

    /// Open a block: `<id> := <kind> {` then the property line. An empty id
    /// mints an anonymous one (A7: ` := DesignSurface {` — the notice row's
    /// unnamed rule — failed the WHOLE row in the app VM, so the slot kept
    /// its previous content on screen).
    pub fn open(&mut self, id: &str, kind: &str, props: &str) {
        let id = if id.is_empty() { self.anon() } else { id.to_owned() };
        let _ = writeln!(self.out, "{id} := {kind} {{");
        if !props.is_empty() {
            self.raw(props);
        }
    }

    pub fn close(&mut self) {
        self.raw("}");
    }

    /// A plain flow container (no paint).
    pub fn view(&mut self, id: &str, props: &str) {
        self.open(id, "View", props);
    }

    /// A painted rounded surface (`DesignSurface`): fill, radius, border.
    pub fn surface(&mut self, id: &str, props: &str, fill: &str, radius: f64, border: Option<&str>) {
        // No card rounder than a Mac's group box; a pill's half-height corner
        // comes down to a control's. A circle (an avatar, a radio) stays one.
        let radius = if crate::desk::active() { mac_radius(props, radius) } else { radius };
        let border_line = match border {
            Some(c) => format!("draw_bg.border_width: 1 draw_bg.border_color: {c}"),
            None => "draw_bg.border_width: 0 draw_bg.border_color: #00000000".to_owned(),
        };
        // On the rect's edge half the stroke is clipped; inside, it is one clean point.
        let position = if crate::desk::active() { 1 } else { 0 };
        let line = format!(
            "{props}\nshow_bg: true draw_bg.color: {fill} draw_bg.radius: {} draw_bg.ellipse: 0 draw_bg.border_position: {position} {border_line}",
            fmt_num(radius)
        );
        self.open(id, "DesignSurface", &line);
    }

    /// A dialog's own surface. On a desktop, a sheet that hangs from the foot of
    /// the toolbar ([`crate::desk::sheet_room`] says how far it may go).
    pub fn sheet_surface(&mut self, id: &str, props: &str, radius: f64) {
        if crate::desk::active() {
            let line = format!(
                "{props} margin: Inset{{top: {}}}\ndraw_bg.color: {} draw_bg.border_color: {}",
                fmt_num(crate::desk::bar_h()),
                tok::SURFACE,
                tok::HAIRLINE
            );
            self.open(id, "OcMacSheet", &line);
        } else {
            self.surface(id, props, tok::SURFACE, radius, Some(tok::HAIRLINE));
        }
    }

    /// A menu's own surface (E7 2). On a Mac, a menu's material (`OcMacMenu`,
    /// the framework's menus' grey, edge and shadow); elsewhere the board's card.
    pub fn menu_surface(&mut self, id: &str, props: &str, radius: f64) {
        if crate::desk::active() {
            let line = format!(
                "{props}\ndraw_bg.color: {} draw_bg.border_color: {}",
                crate::desk::token("menu"),
                crate::desk::token("menu_edge")
            );
            self.open(id, "OcMacMenu", &line);
        } else {
            self.surface(id, props, tok::SURFACE, radius, Some(tok::HAIRLINE));
        }
    }

    /// A 1 px hairline (horizontal, Fill width).
    pub fn hairline(&mut self) {
        let id = self.anon();
        self.rule(&id, "width: Fill height: 1", tok::HAIRLINE);
    }

    /// A filled rectangle (no radius, no border) — the hairline/backdrop
    /// primitive. A plain `View { show_bg }` paints nothing in the mounted
    /// tree (measured: every divider and the mask were invisible), so this is
    /// a `DesignSurface`, the same widget every lowered card fills with.
    pub fn rule(&mut self, id: &str, walk: &str, color: &str) {
        let line = format!(
            "{walk}\nshow_bg: true draw_bg.color: {color} draw_bg.radius: 0 draw_bg.ellipse: 0 draw_bg.border_width: 0 draw_bg.border_position: 0 draw_bg.border_color: #00000000"
        );
        self.open(id, "DesignSurface", &line);
        self.close();
    }

    /// A vertical 1 px hairline of `h` px.
    pub fn vrule(&mut self, h: f64) {
        let id = self.anon();
        self.rule(&id, &format!("width: 1 height: {}", fmt_num(h)), tok::HAIRLINE);
    }

    /// Empty space.
    pub fn gap(&mut self, w: W, h: f64) {
        let id = self.anon();
        self.open(&id, "View", &format!("width: {} height: {}", w.dsl(), fmt_num(h)));
        self.close();
    }

    /// A text run. `id` names it in /snap (pass `""` for an anonymous id).
    pub fn text(&mut self, id: &str, s: &str, t: &Txt) {
        let id = if id.is_empty() { self.anon() } else { id.to_owned() };
        let mac;
        let t = if crate::desk::active() && t.px > 13.0 {
            // A Mac's body is 13 points; only titles go larger, and not far.
            let px = match (t.face, t.px) {
                (Face::Semibold, p) if p >= 17.0 => 15.0,
                (Face::Mono, _) => 12.0,
                (Face::Semibold, p) if p >= 15.0 => p,
                _ => 13.0,
            };
            mac = Txt { px, ..t.clone() };
            &mac
        } else {
            t
        };
        // A32 — a run cut by [`fit_w`] ends in "…" measured from estimates;
        // one the estimate under-measures (capitals, digits, dashes) is wider
        // than its label, whose clip would cut that "…" to two dots. On a
        // bounded single line, makepad's own ellipsis cuts such a run again
        // inside the label (decision D10e); a run that fits draws as before.
        let flow = if t.wrap {
            "flow: Right{wrap: true}"
        } else if t.elide || (t.width != W::Fit && s.ends_with('\u{2026}') && !s.contains('\n')) {
            "flow: Right max_lines: 1 text_overflow: TextOverflow.Ellipsis"
        } else {
            "flow: Right"
        };
        let mut props = format!(
            "width: {} height: Fit padding: 0 text: {} {flow}\ndraw_text.text_style: {}\ndraw_text.color: {}",
            t.width.dsl(),
            lit(s),
            text_style(t.face, t.px),
            t.color
        );
        if t.centre {
            props.push_str("\nalign: Align{x: 0.5}");
        }
        // No padding: without this g, y and p lose their tails.
        if crate::desk::active() {
            props.push_str("\nclip_y: false");
        }
        // A sheet's title says what it is about; the session's and the
        // profile's keys under it are for the protocol, not for reading.
        if crate::desk::active() && (hidden_scope(&id) || id == "b3_insp_slash") {
            props.push_str("\nvisible: false");
        }
        self.open(&id, "Label", &props);
        self.close();
    }

    /// A word the walks and the protocol read and a Mac reader never needs: it
    /// stays in the tree, takes no space and draws nothing.
    pub fn unseen(&mut self, id: &str, s: &str) {
        let props = format!("width: Fit height: Fit padding: 0 text: {} visible: false", lit(s));
        self.open(id, "Label", &props);
        self.close();
    }

    /// A line icon from the module's `resources/icons/`. The stroke colour is
    /// baked into each file (the module's own icons do the same), so `_color`
    /// only documents the intent at the call site. On a surface that follows
    /// the theme, [`themed_icons`] tints them in dark.
    pub fn icon(&mut self, id: &str, file: &str, size: f64, color: &str) {
        let id = if id.is_empty() { self.anon() } else { id.to_owned() };
        if file == "b1_spinner.svg" && crate::desk::active() {
            // A Mac's activity indicator: the framework's, the same in every app.
            self.open(&id, "LoadingSpinner", &format!("width: {s} height: {s}", s = fmt_num(size)));
            self.close();
            return;
        }
        let path = crate::design::icon_resource(file);
        let mut props = format!(
            "width: {s} height: {s} animating: false draw_svg.svg: file_resource({path:?}) draw_svg.preserve_viewbox: true",
            s = fmt_num(size)
        );
        if crate::desk::active() {
            // A warning stays amber and a failure red in dark too, and a tick
            // on a blue badge stays white.
            let ink = match color {
                tok::WHITE => Some("#fffffeff"),
                tok::RED | tok::AMBER | tok::GREEN => Some(color),
                _ if size >= 30.0 => Some(color),
                _ => None,
            };
            if let Some(ink) = ink {
                props.push_str(&format!(" draw_svg.color: {ink}"));
            }
        }
        self.open(&id, "Svg", &props);
        self.close();
    }

    /// A round dot (status light).
    pub fn dot(&mut self, color: &str, size: f64) {
        let id = self.anon();
        let line = format!(
            "width: {s} height: {s}\nshow_bg: true draw_bg.color: {color} draw_bg.radius: {r} draw_bg.ellipse: 1 draw_bg.border_width: 0 draw_bg.border_position: 0 draw_bg.border_color: #00000000",
            s = fmt_num(size),
            r = fmt_num(size / 2.0)
        );
        self.open(&id, "DesignSurface", &line);
        self.close();
    }

    /// A transparent tap target over its parent's box (the parent must be an
    /// `Overlay` flow). `event` is the routed action id (with an optional
    /// `#<row>` suffix).
    pub fn tap(&mut self, id: &str, event: &str) {
        self.tap_on(id, event, false);
    }

    /// `on_blue`: over the default button its wash darkens in either appearance,
    /// so the white word keeps its contrast.
    fn tap_on(&mut self, id: &str, event: &str, on_blue: bool) {
        self.taps.push((id.to_owned(), event.to_owned()));
        let _ = writeln!(self.out, "{id} := DesignNativeButton {{");
        self.raw("width: Fill height: Fill");
        // A Mac's control answers the pointer: a wash, gone at rest. A
        // backdrop only takes the press.
        if crate::desk::active() && !id.contains("backdrop") {
            let t = crate::desk::current();
            let (hover, down) = if on_blue { ("#00000012", "#00000020") } else { (t.wash.as_str(), t.wash_down.as_str()) };
            let _ = writeln!(self.out, "draw_bg +: {{color_hover: {hover} color_down: {down}}}");
        }
        let _ = writeln!(self.out, "on_click: || {{ NAV(t: {event:?}) }}");
        self.raw("}");
    }

    /// A chip: tinted rounded label (`enabled`, `stdio`, `unverified`).
    pub fn chip(&mut self, id: &str, s: &str, fg: &'static str, bg: &str, border: Option<&str>, mono: bool) {
        let wrap_id = if id.is_empty() { self.anon() } else { format!("{id}_chip") };
        if crate::desk::active() {
            // A Mac's status is a word in its colour, never a pill, and a blue word reads as a link.
            let fg = if fg == tok::BLUE_TEXT { tok::MUTED } else { fg };
            self.view(&wrap_id, "width: Fit height: 20 flow: Right align: Align{x: 0.5 y: 0.5}");
            let t = Txt::new(12.0, if mono { Face::Mono } else { Face::Regular }, fg);
            if !mono && s.starts_with(|c: char| c.is_ascii_lowercase()) {
                self.text(id, &crate::desk::sentence(s), &t);
            } else {
                self.text(id, s, &t);
            }
            self.close();
            return;
        }
        self.surface(
            &wrap_id,
            "width: Fit height: 20 flow: Right align: Align{x: 0.5 y: 0.5} padding: Inset{left: 7 right: 7 top: 0 bottom: 0}",
            bg,
            crate::desk::by(6.0, 4.0),
            border,
        );
        let t = if mono {
            Txt::new(11.0, Face::Mono, fg)
        } else {
            Txt::new(11.0, Face::Medium, fg)
        };
        self.text(id, s, &t);
        self.close();
    }

    /// A pill button: `Primary` = solid black + white text; `Outline` = white
    /// with a hairline; `Disabled` = grey, no tap target (fail closed — a
    /// disabled control must not route anything).
    pub fn button(&mut self, id: &str, label: &str, event: &str, kind: Btn, width: W, height: f64) {
        self.button_ids(&format!("{id}_box"), &format!("{id}_label"), id, label, event, kind, width, height);
    }

    /// [`Dsl::button`] whose label is data (a model's name): its case stands.
    pub fn data_button(&mut self, id: &str, label: &str, event: &str, kind: Btn, width: W, height: f64) {
        self.verbatim = true;
        self.button(id, label, event, kind, width, height);
    }

    /// [`Dsl::button`] with explicit widget ids for its surface, its label
    /// and its tap target (A14: the A5 dialog host keeps the
    /// `<base>_surface` / `<base>_label` / `<base>_control` names its walks
    /// and tests address).
    #[allow(clippy::too_many_arguments)]
    pub fn button_ids(
        &mut self,
        box_id: &str,
        label_id: &str,
        tap_id: &str,
        label: &str,
        event: &str,
        kind: Btn,
        width: W,
        height: f64,
    ) {
        let mac = crate::desk::active().then(crate::desk::current);
        let (fill, fg, border) = match (&mac, kind) {
            // A Mac's default action in the system blue; every other push
            // button a raised neutral face under a hairline.
            (Some(t), Btn::Primary) => (t.primary.as_str(), "#fffffeff", None),
            (Some(t), Btn::Outline | Btn::Secondary) => (t.button.as_str(), tok::TEXT, Some(t.border.as_str())),
            (Some(t), Btn::OutlineOff | Btn::Disabled) => (t.button.as_str(), crate::desk::disabled_on_face(), Some(t.border.as_str())),
            (_, Btn::Primary) => (tok::BLACK, tok::WHITE, None),
            (_, Btn::Outline) => (tok::SURFACE, tok::TEXT, Some("#c7c7ccff")),
            (_, Btn::OutlineOff) => (tok::SURFACE, tok::DISABLED_INK, Some(tok::HAIRLINE)),
            (_, Btn::Secondary) => (tok::CHIP, tok::TEXT, None),
            (_, Btn::Disabled) => (tok::DISABLED_BG, tok::DISABLED_INK, None),
            (_, Btn::Ghost) => (tok::TRANSPARENT, tok::TEXT, None),
        };
        let fg = self.word_ink.take().unwrap_or(fg);
        self.primary_drawn |= mac.is_some() && matches!(kind, Btn::Primary);
        let height = if mac.is_some() { height.min(28.0) } else { height };
        let radius = if mac.is_some() {
            6.0
        } else if matches!(kind, Btn::Outline | Btn::OutlineOff) && height > 34.0 {
            10.0
        } else {
            height / 2.0
        };
        // A24: the kit's buttons take a product-copy key: shown in the
        // current language (measured in it too).
        let label = tr(label);
        let label = if std::mem::take(&mut self.verbatim) {
            std::borrow::Cow::Borrowed(label)
        } else {
            crate::desk::title(label)
        };
        let label: &str = &label;
        // A Fit pill gets an explicit width (see `text_w`).
        let width = match width {
            W::Fit if mac.is_some() => W::Px((text_w(label, 13.0, Face::Medium) + 24.0).max(72.0)),
            W::Fit => W::Px(text_w(label, 13.0, Face::Medium) + 32.0),
            w => w,
        };
        self.surface(
            box_id,
            &format!(
                "width: {} height: {} flow: Overlay align: Align{{x: 0.5 y: 0.5}}",
                width.dsl(),
                fmt_num(height)
            ),
            fill,
            radius,
            border,
        );
        let inner = self.anon();
        self.view(
            &inner,
            "width: Fill height: Fill flow: Right align: Align{x: 0.5 y: 0.5} padding: Inset{left: 12 right: 12 top: 0 bottom: 0}",
        );
        self.text(label_id, label, &Txt::new(13.0, if mac.is_some() { Face::Regular } else { Face::Medium }, fg));
        self.close();
        if !matches!(kind, Btn::Disabled | Btn::OutlineOff) {
            self.tap_on(tap_id, event, matches!(kind, Btn::Primary));
        }
        self.close();
    }

    /// The next button's word says it can't be undone: red, on a neutral face.
    pub fn red_word(&mut self) {
        self.word_ink = Some(crate::desk::danger_on_face());
    }

    /// A text link (blue), optionally tappable.
    pub fn link(&mut self, id: &str, label: &str, event: Option<&str>, px: f64) {
        self.link_ids(&format!("{id}_box"), &format!("{id}_label"), id, label, event, px, tok::BLUE_TEXT);
    }

    /// [`Dsl::link`] with explicit ids and ink (A14: the dialog host's
    /// `+ New loop` keeps `…_control`, a skill's Remove keeps `t_removeN` /
    /// `t_removeN_hit`; a destructive link is red, a paused one faint).
    #[allow(clippy::too_many_arguments)]
    pub fn link_ids(
        &mut self,
        box_id: &str,
        label_id: &str,
        tap_id: &str,
        label: &str,
        event: Option<&str>,
        px: f64,
        color: &'static str,
    ) {
        if crate::desk::active() {
            // A Mac's action is a push button, never a blue word; a red word says
            // it can't be undone, and a faint one that it is paused.
            self.word_ink = match color {
                tok::BLUE_TEXT => None,
                tok::RED_TEXT => Some(crate::desk::danger_on_face()),
                other => Some(other),
            };
            let kind = if event.is_some() { Btn::Outline } else { Btn::OutlineOff };
            self.button_ids(box_id, label_id, tap_id, label, event.unwrap_or(""), kind, W::Fit, 28.0);
            return;
        }
        // A24: a link's label is a product-copy key (current language).
        let label = crate::desk::title(tr(label));
        let label: &str = &label;
        // Explicit box (see `text_w`): the tap target must not measure 0.
        let w = text_w(label, px, Face::Regular) + 4.0;
        // >= 28 px high: the brief's minimum hit size.
        let h = (px * 1.6).ceil().max(28.0);
        self.view(
            box_id,
            &format!("width: {} height: {} flow: Overlay align: Align{{x: 0.0 y: 0.5}}", fmt_num(w), fmt_num(h)),
        );
        self.text(label_id, label, &Txt::new(px, Face::Regular, color));
        if let Some(ev) = event {
            self.tap(tap_id, ev);
        }
        self.close();
    }

    /// A14 — a line icon in a square hit box: the glyph is `<base>`
    /// (`size` px), its box `<base>_box` (`hit` px, centred) and, when
    /// `event` is given, the tap `<base>_hit` over the whole box (>= 28 px).
    pub fn icon_hit(&mut self, base: &str, file: &str, size: f64, hit: f64, event: Option<&str>) {
        self.view(
            &format!("{base}_box"),
            &format!("width: {h} height: {h} flow: Overlay align: Align{{x: 0.5 y: 0.5}}", h = fmt_num(hit)),
        );
        self.icon(base, file, size, tok::MUTED);
        if let Some(ev) = event {
            self.tap(&format!("{base}_hit"), ev);
        }
        self.close();
    }

    /// The board's switch: a blue (on) / grey (off) track with a white knob.
    pub fn toggle(&mut self, id: &str, on: bool, event: &str) {
        let track = if on { tok::BLUE } else { "#d1d1d6ff" };
        // A Mac's switch: 28 by 16, its knob a hair inside.
        let mac = crate::desk::active();
        let ((box_w, box_h), (track_w, track_h), knob_d, knob_fill) = if mac {
            ((32, 28), (28, 16), 14, "#fffffeff")
        } else {
            ((44, 32), (38, 22), 18, tok::WHITE)
        };
        let inset = if mac { 1 } else { 2 };
        // A 44x32 hit box around the 38x22 track (>= 28 px to tap).
        self.view(&format!("{id}_box"), &format!("width: {box_w} height: {box_h} flow: Overlay align: Align{{x: 0.5 y: 0.5}}"));
        self.surface(
            &format!("{id}_track"),
            &format!(
                "width: {track_w} height: {track_h} flow: Overlay align: Align{{x: {} y: 0.5}} padding: Inset{{left: {inset} right: {inset} top: 0 bottom: 0}}",
                if on { "1.0" } else { "0.0" }
            ),
            track,
            track_h as f64 / 2.0,
            None,
        );
        let knob = self.anon();
        self.surface(&knob, &format!("width: {knob_d} height: {knob_d}"), knob_fill, knob_d as f64 / 2.0, None);
        self.close();
        self.close();
        self.tap(id, event);
        self.close();
    }

    /// A text input inside a hairline field. The DSL never embeds the LIVE
    /// text while typing (the mount would rebuild the widget and drop the
    /// focus): `text` is the snapshot the dialog state carries.
    pub fn input(&mut self, id: &str, key: &str, text: &str, placeholder: &str, mono: bool, height: f64) {
        self.inputs.push((id.to_owned(), key.to_owned()));
        let height = field_height(height);
        self.surface(
            &format!("{id}_field"),
            &format!(
                "width: Fill height: {} flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 10 right: 10 top: 0 bottom: 0}}",
                fmt_num(height)
            ),
            tok::SURFACE,
            crate::desk::by(8.0, 6.0),
            Some("#d9d9dcff"),
        );
        self.open(id, "TextInput", &input_props(text, placeholder, mono, false));
        self.close();
        self.close();
    }

    /// A17 — a MASKED input (`<input type="password">`, the onboarding API
    /// key): the field draws dots, and the typed text is never embedded in
    /// the DSL — the host puts the live value back after a remount
    /// (`onboarding::post_mount_texts`), so a secret never rides a mount.
    pub fn input_secret(&mut self, id: &str, key: &str, placeholder: &str, height: f64) {
        self.inputs.push((id.to_owned(), key.to_owned()));
        let height = field_height(height);
        self.surface(
            &format!("{id}_field"),
            &format!(
                "width: Fill height: {} flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 10 right: 10 top: 0 bottom: 0}}",
                fmt_num(height)
            ),
            tok::SURFACE,
            crate::desk::by(8.0, 6.0),
            Some("#d9d9dcff"),
        );
        let props = input_props("", placeholder, false, false)
            .replace("is_read_only: false", "is_read_only: false is_password: true");
        self.open(id, "TextInput", &props);
        self.close();
        self.close();
    }

    /// A14 — [`Dsl::input`] with a leading line icon (the registry search's
    /// magnifier): the glyph and the text share the field's centre line
    /// (both centred by the field's `align y: 0.5`; a judge capture had the
    /// placeholder 9 px under the glyph).
    #[allow(clippy::too_many_arguments)]
    pub fn input_icon(&mut self, id: &str, key: &str, text: &str, placeholder: &str, icon: &str, height: f64) {
        self.inputs.push((id.to_owned(), key.to_owned()));
        let height = field_height(height);
        self.surface(
            &format!("{id}_field"),
            &format!(
                "width: Fill height: {} flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 8 padding: Inset{{left: 10 right: 10 top: 0 bottom: 0}}",
                fmt_num(height)
            ),
            tok::SURFACE,
            crate::desk::by(8.0, 6.0),
            Some("#d9d9dcff"),
        );
        self.icon(&format!("{id}_icon"), icon, 16.0, tok::MUTED);
        self.open(id, "TextInput", &input_props(text, placeholder, false, false));
        self.close();
        self.close();
    }

    /// A14 — a multi-line text field (the review instructions' `<textarea>`):
    /// the text wraps from the field's top-left corner.
    pub fn input_multiline(&mut self, id: &str, key: &str, text: &str, placeholder: &str, height: f64) {
        self.inputs.push((id.to_owned(), key.to_owned()));
        self.surface(
            &format!("{id}_field"),
            &format!(
                "width: Fill height: {} flow: Down align: Align{{x: 0.0 y: 0.0}} padding: Inset{{left: 10 right: 10 top: 6 bottom: 6}}",
                fmt_num(height)
            ),
            tok::SURFACE,
            crate::desk::by(8.0, 6.0),
            Some("#d9d9dcff"),
        );
        self.open(id, "TextInput", &input_props(text, placeholder, false, true));
        self.close();
        self.close();
    }

    /// A8 — a READ-ONLY, selectable, wrapping text field (the web's
    /// clipboard-denied fallback `<textarea readOnly>`,
    /// `CopySessionLink.tsx:71-85`): the value is always visible in full and
    /// the person can select and copy it by hand. No input event is routed.
    pub fn readonly_text(&mut self, id: &str, text: &str, mono: bool) {
        self.surface(
            &format!("{id}_field"),
            "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} padding: Inset{left: 10 right: 10 top: 6 bottom: 6}",
            tok::SURFACE2,
            crate::desk::by(8.0, 6.0),
            Some("#d9d9dcff"),
        );
        let face = if mono { Face::Mono } else { Face::Regular };
        let style = text_style(face, 12.0);
        let props = format!(
            "width: Fill height: Fit padding: 0 margin: 0\ntext: {}\nflow: Right{{wrap: true}} is_read_only: true is_multiline: true\ndraw_bg +: {{pixel: fn() {{return vec4(0.0, 0.0, 0.0, 0.0)}}}}\ndraw_text +: {{color: {t} color_hover: {t} color_focus: {t} color_down: {t} color_disabled: {t} color_empty: {t} color_empty_hover: {t} color_empty_focus: {t}}}\ndraw_text.text_style: {style}\ndraw_cursor +: {{color: #00000000}}\ndraw_selection +: {{color: #2f6feb33 color_hover: #2f6feb33 color_focus: #2f6feb40 color_down: #2f6feb40 color_empty: #00000000 color_disabled: #00000000}}",
            lit(text),
            t = tok::TEXT,
        );
        self.open(id, "TextInput", &props);
        self.close();
        self.close();
    }

    /// A segmented control: `options` = (label, event); `selected` index.
    /// `Seg::Pill` selects with the board's solid black pill (screen 6's
    /// effort control); `Seg::Tab` with a raised white segment on the grey
    /// track (screen 1's "Tools | MCP servers").
    pub fn segmented(&mut self, id: &str, options: &[(&str, String)], selected: usize, width: W, style: Seg) {
        if crate::desk::active() {
            return self.mac_segmented(id, options, selected, width);
        }
        let (track, height) = match style {
            Seg::Tab => (tok::SURFACE2, 34.0),
            Seg::Pill => (tok::SURFACE, 38.0),
        };
        self.surface(
            &format!("{id}_track"),
            &format!(
                "width: {} height: {} flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 2 padding: Inset{{left: 3 right: 3 top: 3 bottom: 3}}",
                width.dsl(),
                fmt_num(height)
            ),
            track,
            9.0,
            Some(tok::HAIRLINE),
        );
        for (i, (label, event)) in options.iter().enumerate() {
            let on = i == selected;
            // The board's Pill control separates unselected neighbours with
            // a hairline (screen 6: Low | Medium | High | Max).
            if style == Seg::Pill && i > 0 && !on && i - 1 != selected {
                self.vrule(18.0);
            }
            let seg = format!("{id}_{i}");
            let (fill, border, fg) = match (style, on) {
                (Seg::Pill, true) => (tok::BLACK, None, tok::WHITE),
                (Seg::Tab, true) => (tok::SURFACE, Some(tok::HAIRLINE), tok::TEXT),
                (_, false) => (tok::TRANSPARENT, None, tok::MUTED),
            };
            self.surface(
                &format!("{seg}_box"),
                "width: Fill height: Fill flow: Overlay align: Align{x: 0.5 y: 0.5}",
                fill,
                7.0,
                border,
            );
            let inner = self.anon();
            self.view(&inner, "width: Fill height: Fill flow: Right align: Align{x: 0.5 y: 0.5}");
            self.text(
                &format!("{seg}_label"),
                tr(label),
                &Txt::new(13.0, if on { Face::Medium } else { Face::Regular }, fg),
            );
            self.close();
            self.tap(&seg, event);
            self.close();
        }
        self.close();
    }
}

impl Dsl {
    /// A Mac's segmented control: a gray track, the chosen segment a raised
    /// neutral face. Both of the board's styles read the same on a desktop.
    fn mac_segmented(&mut self, id: &str, options: &[(&str, String)], selected: usize, width: W) {
        let t = crate::desk::current();
        let width = match width {
            W::Fill => {
                let widest = options.iter().map(|(label, _)| text_w(tr(label), 12.0, Face::Regular)).fold(0.0, f64::max);
                let n = options.len() as f64;
                W::Px((widest + 24.0).max(64.0).ceil() * n + 2.0 * (n - 1.0) + 4.0)
            }
            other => other,
        };
        self.surface(
            &format!("{id}_track"),
            &format!(
                "width: {} height: 28 flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 2 padding: Inset{{left: 2 right: 2 top: 2 bottom: 2}}",
                width.dsl(),
            ),
            &t.selected,
            7.0,
            None,
        );
        for (i, (label, event)) in options.iter().enumerate() {
            let on = i == selected;
            let seg = format!("{id}_{i}");
            let (fill, border) = if on {
                (t.segment_on.as_str(), Some(t.border.as_str()))
            } else {
                (tok::TRANSPARENT, None)
            };
            self.surface(
                &format!("{seg}_box"),
                "width: Fill height: Fill flow: Overlay align: Align{x: 0.5 y: 0.5}",
                fill,
                5.0,
                border,
            );
            let inner = self.anon();
            self.view(&inner, "width: Fill height: Fill flow: Right align: Align{x: 0.5 y: 0.5}");
            self.text(&format!("{seg}_label"), tr(label), &Txt::new(12.0, Face::Regular, tok::TEXT));
            self.close();
            self.tap(&seg, event);
            self.close();
        }
        self.close();
    }
}

/// A field's height: a Mac's single-line field is 28 points.
fn field_height(height: f64) -> f64 {
    if crate::desk::active() {
        height.min(28.0)
    } else {
        height
    }
}

/// The kit's `TextInput` properties: 13 px text (mono or Inter), transparent
/// background (the field surface paints), faint placeholder; `multiline`
/// wraps and fills the field.
pub(crate) fn input_props(text: &str, placeholder: &str, mono: bool, multiline: bool) -> String {
    let face = if mono { Face::Mono } else { Face::Regular };
    let style = text_style(face, 13.0);
    let (walk, flow) = if multiline {
        ("width: Fill height: Fill", "flow: Right{wrap: true} is_multiline: true")
    } else {
        ("width: Fill height: Fit", "flow: Right")
    };
    format!(
        "{walk} padding: Inset{{left: 0 right: 0 top: 4 bottom: 4}} margin: 0\ntext: {} empty_text: {}\n{flow} is_read_only: false\ndraw_bg +: {{pixel: fn() {{return vec4(0.0, 0.0, 0.0, 0.0)}}}}\ndraw_text +: {{color: {t} color_hover: {t} color_focus: {t} color_down: {t} color_disabled: {off} color_empty: {f} color_empty_hover: {f} color_empty_focus: {f}}}\ndraw_text.text_style: {style}\ndraw_cursor +: {{color: {t}}}\ndraw_selection +: {{color: #2f6feb33 color_hover: #2f6feb33 color_focus: #2f6feb40 color_down: #2f6feb40 color_empty: #00000000 color_disabled: #00000000}}",
        lit(text),
        lit(tr(placeholder)),
        t = tok::TEXT,
        f = crate::desk::by(tok::FAINT, crate::desk::placeholder()),
        off = tok::DISABLED_INK,
    )
}

/// Segmented-control look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seg {
    Pill,
    Tab,
}

/// Button look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Btn {
    Primary,
    Outline,
    /// An outline control that is not available right now (no tap).
    OutlineOff,
    /// The board's grey filled pill (screen 5's Refresh).
    Secondary,
    Disabled,
    Ghost,
}

/// The dialog frame: the web's backdrop + `min(<max>px, 100%)` dialog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// The module's laid-out width/height (logical px).
    pub avail_w: f64,
    pub avail_h: f64,
}

impl Frame {
    /// The desktop default the shell gives the module (`/snap`:
    /// OctoscodeView [54,76,990,603]) — used before the first layout.
    pub const DESKTOP: Frame = Frame { avail_w: 990.0, avail_h: 603.0 };

    /// The dialog width: `min(max, avail - 2*16)` (the web's backdrop inset).
    pub fn dialog_w(&self, max: f64) -> f64 {
        let max = if crate::desk::active() { crate::desk::sheet_w(max) } else { max };
        (self.avail_w - 32.0).min(max).max(240.0).floor()
    }

    /// The dialog's maximum height (`calc(100dvh - 32px)`).
    pub fn dialog_max_h(&self) -> f64 {
        // On a desktop a sheet hangs from the foot of the toolbar.
        crate::desk::by(self.avail_h - 32.0, crate::desk::sheet_room(self.avail_h)).max(200.0).floor()
    }

    /// Phone-narrow: stack table columns into two-line rows.
    pub fn compact(&self, dialog_w: f64) -> bool {
        dialog_w < 520.0
    }
}

/// The dialog card's inner padding (the web's `.dialog { padding: 20px }`,
/// 16 on a phone-narrow frame).
pub fn dialog_pad(frame: &Frame, width: f64) -> f64 {
    if frame.compact(width) {
        16.0
    } else {
        20.0
    }
}

/// The widget ids one dialog family's frame carries. Board 3's dialogs use
/// [`B3_IDS`]; A14: the A5 dialog host (`screens::dialog`) draws the SAME
/// frame under the ids its walks and the judge tour address
/// (`dialog_frame`, `dialog_scroll`, `dialog_close`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellIds {
    pub root: &'static str,
    pub backdrop: &'static str,
    pub backdrop_box: &'static str,
    pub backdrop_hit: &'static str,
    /// The backdrop's routed event; `None` = a press that routes nothing.
    pub backdrop_event: Option<&'static str>,
    pub dialog: &'static str,
    pub scroll: &'static str,
    /// The close glyph's tap (its box is `<close>_box`, its icon
    /// `<close>_icon`).
    pub close: &'static str,
}

/// Board 3's own frame ids.
pub const B3_IDS: ShellIds = ShellIds {
    root: "b3_root",
    backdrop: "b3_backdrop",
    backdrop_box: "b3_backdrop_box",
    backdrop_hit: "b3_backdrop_hit",
    backdrop_event: Some("b3.noop"),
    dialog: "b3_dialog",
    scroll: "b3_scroll",
    close: "b3_close",
};

/// What a Mac dims a window with behind its sheet.
pub fn mac_mask() -> &'static str {
    crate::desk::sheet_dim()
}

/// How an overlay places its sheet: centred on a phone, hanging on a desktop.
pub fn sheet_align() -> &'static str {
    crate::desk::by("Align{x: 0.5 y: 0.5}", "Align{x: 0.5 y: 0.0}")
}

/// A Mac's corner for a `DesignSurface` of these props: a square whose radius
/// is half its side is a circle and stays one; a stadium comes down to a
/// control's six points; anything else to a group box's ten.
fn mac_radius(props: &str, radius: f64) -> f64 {
    let px = |key: &str| -> Option<f64> {
        let at = props.find(key)? + key.len();
        let rest = props[at..].trim_start();
        let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
        rest[..end].parse().ok()
    };
    match (px("width:"), px("height:")) {
        (Some(w), Some(h)) if w == h && radius >= w / 2.0 => radius,
        (_, Some(h)) if radius >= h / 2.0 && radius > 6.0 => 6.0,
        _ => radius.min(10.0),
    }
}

/// Open the backdrop + the centred card (the web's `.backdrop` grid +
/// `.dialog`). The card hugs its content; [`body_open`] caps the body so the
/// whole card never exceeds the frame's max height (the web's `max-height:
/// calc(100dvh - 32px)` + `overflow: auto`).
pub fn shell_open(d: &mut Dsl, frame: &Frame, width: f64) {
    shell_open_ids(d, frame, width, &B3_IDS);
}

/// [`shell_open`] under another family's ids (the same frame, backdrop and
/// keyboard behaviour).
pub fn shell_open_ids(d: &mut Dsl, frame: &Frame, width: f64, ids: &ShellIds) {
    let pad = dialog_pad(frame, width);
    if crate::desk::active() {
        d.sheet = Some(SheetFoot::default());
    }
    // A8 — the root pans its content above an on-screen keyboard (makepad's
    // `KeyboardView`: the focused field stays visible while typing, the way a
    // phone browser scrolls the web dialog's focused input into view). The
    // kept gap (56 = 8 + a 38 button + 10) leaves a field's primary button,
    // set right under it, above the keyboard too. No keyboard (the desktop),
    // no shift.
    d.open(
        ids.root,
        "KeyboardView",
        &format!("width: Fill height: Fill flow: Overlay align: {} keyboard_min_shift: 56.", sheet_align()),
    );
    d.rule(ids.backdrop, "width: Fill height: Fill", crate::desk::by(tok::MASK, mac_mask()));
    // The backdrop is modal: it swallows presses so nothing under it (the
    // sidebar, the composer) reacts, and it does not close the dialog
    // (`ui/ModalSurface.tsx`: Escape or the close control only).
    d.view(ids.backdrop_box, "width: Fill height: Fill flow: Overlay");
    match ids.backdrop_event {
        Some(ev) => d.tap(ids.backdrop_hit, ev),
        None => {
            // A transparent button that takes the press and routes nothing.
            let _ = writeln!(
                d.out,
                "{} := Button {{ width: Fill height: Fill text: \"\" draw_bg.color: #00000000 draw_bg.color_hover: #00000000 draw_bg.color_down: #00000000 draw_bg.border_size: 0.0 draw_bg.color_2: #00000000 draw_bg.border_color: #00000000 draw_bg.border_color_2: #00000000 }}",
                ids.backdrop_hit
            );
        }
    }
    d.close();
    d.sheet_surface(
        ids.dialog,
        &format!(
            "width: {} height: Fit flow: Down padding: Inset{{left: {p} right: {p} top: {p} bottom: {p}}}",
            fmt_num(width),
            p = fmt_num(pad)
        ),
        16.0,
    );
}

/// A family's own frame is a Mac sheet too: its close goes to the foot
/// ([`shell_close`]). `inset`: the foot's side padding when the frame has
/// none of its own.
pub fn sheet_begin(d: &mut Dsl, inset: Option<f64>) {
    if crate::desk::active() {
        d.sheet = Some(SheetFoot { inset, ..SheetFoot::default() });
    }
}

/// The body region: a `Fit` scroll view capped at what the frame leaves after
/// the card padding and the dialog's fixed chrome (`chrome_h`: header +
/// footer). It hugs short content and scrolls long content —
/// `ScrollYView` resolves a `Fit` height against `max_height`
/// (makepad `scroll_bars.rs:372-378`).
pub fn body_open(d: &mut Dsl, frame: &Frame, width: f64, chrome_h: f64) {
    body_open_id(d, frame, width, chrome_h, B3_IDS.scroll);
}

/// [`body_open`] with the scroll view's id.
pub fn body_open_id(d: &mut Dsl, frame: &Frame, width: f64, chrome_h: f64, scroll_id: &str) {
    let pad = dialog_pad(frame, width);
    // A Mac sheet's foot sits under the body (16 of gap, a 28 button).
    let foot = if d.sheet.is_some() { 44.0 } else { 0.0 };
    if let Some(sheet) = &mut d.sheet {
        sheet.scrolls = true;
    }
    let max_body = row_room(scroll_id, (frame.dialog_max_h() - 2.0 * pad - chrome_h - foot).max(120.0).floor());
    // The right inset is the scroll bar's gutter: measured on the phone
    // layout, the bar drew over the first rows' status chips without it. On a
    // Mac the gutter hangs in the sheet's own padding, so the body's edge
    // meets the title's and the foot's.
    let hang = if d.sheet.is_some() { " margin: Inset{right: -10}" } else { "" };
    let end = if d.sheet.is_some() { 8 } else { 0 };
    d.open(
        scroll_id,
        "ScrollYView",
        &format!(
            "width: Fill height: Fit max_height: {}{hang} flow: Down padding: Inset{{left: 0 top: 0 right: 10 bottom: {end}}}",
            fmt_num(max_body)
        ),
    );
}

/// n11o (E9 7): a Mac sheet whose body is a scroll view of its own (not
/// [`body_open`]'s): a hairline ends it above the foot, as [`body_open_id`]'s does.
pub fn sheet_scrolls(d: &mut Dsl) {
    if let Some(sheet) = &mut d.sheet {
        sheet.scrolls = true;
    }
}

/// Close [`body_open`].
pub fn body_close(d: &mut Dsl) {
    d.close();
}

/// Close the card and the backdrop root.
pub fn shell_close(d: &mut Dsl) {
    // n11o (E9 5): what floats over the sheet goes after it, in the root.
    let mut over = String::new();
    if let Some(mut foot) = d.sheet.take() {
        over = std::mem::take(&mut foot.over);
        no_sheet_done();
        let close = foot.close.filter(|_| !foot.own_close);
        // A default of the sheet's own: Return does it.
        if let Some(event) = &foot.default_event {
            *SHEET_DONE.lock().unwrap() = Some((event.clone(), d.inputs.iter().map(|(n, _)| n.clone()).collect()));
        }
        if close.is_some() || !foot.lead.is_empty() || !foot.trail.is_empty() {
            let props = match foot.inset {
                Some(pad) => {
                    d.hairline();
                    format!(
                        "width: Fill height: Fit flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 8 padding: Inset{{left: {p} right: {p} top: 12 bottom: 12}}",
                        p = fmt_num(pad)
                    )
                }
                None if foot.scrolls => {
                    d.hairline();
                    "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 margin: Inset{top: 12}".to_owned()
                }
                None => "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 margin: Inset{top: 16}".to_owned(),
            };
            d.view("sheet_foot", &props);
            d.out.push_str(&foot.lead);
            d.gap(W::Fill, 1.0);
            if let Some((id, event)) = close {
                let word = if foot.has_default && !foot.keep_done { tr("Cancel") } else { crate::i18n::tr_ctx("button", "Done") };
                // With nothing else to do, Done is the sheet's default.
                let default = !foot.has_default && !d.primary_drawn;
                let kind = if default { Btn::Primary } else { Btn::Outline };
                d.button_ids(&format!("{id}_box"), &format!("{id}_label"), &id, word, &event, kind, W::Fit, 28.0);
                if default {
                    *SHEET_DONE.lock().unwrap() = Some((event, d.inputs.iter().map(|(n, _)| n.clone()).collect()));
                }
            }
            d.out.push_str(&foot.trail);
            d.close();
        }
    }
    d.close();
    d.out.push_str(&over);
    d.close();
}

/// A Mac's answers to a question before something goes for good: the red word
/// first, the blue Cancel last. Return and Escape answer Cancel.
pub fn destructive_row(d: &mut Dsl, cancel: (&str, &str), go: (&str, &str, &str), armed: bool) {
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right spacing: 8 align: Align{x: 1.0 y: 0.5}");
    if armed {
        d.red_word();
    }
    d.button(go.0, go.1, go.2, if armed { Btn::Outline } else { Btn::Disabled }, W::Fit, 28.0);
    d.button(cancel.0, tr("Cancel"), cancel.1, Btn::Primary, W::Fit, 28.0);
    d.close();
}

/// A Mac alert's width: the compact alert of macOS 11 and on, as the native
/// apps draw theirs (Files' Empty Trash, Notes' questions).
pub const ALERT_W: f64 = 260.0;
/// What its words and buttons have: the width less its 16-point sides.
pub const ALERT_INNER: f64 = ALERT_W - 32.0;

/// One of a Mac alert's buttons ([`alert_buttons`]).
pub struct AlertButton<'a> {
    /// Its surface, word and press ids, as [`Dsl::button_ids`] takes them.
    pub ids: (String, String, String),
    pub word: &'a str,
    pub event: &'a str,
    pub kind: Btn,
    /// What it does can't be undone: its word is red.
    pub red: bool,
}

impl<'a> AlertButton<'a> {
    /// A button under [`Dsl::button`]'s ids: `<id>_box`, `<id>_label`, `<id>`.
    pub fn new(id: &str, word: &'a str, event: &'a str, kind: Btn) -> Self {
        Self::ids(&format!("{id}_box"), &format!("{id}_label"), id, word, event, kind)
    }

    /// A button under its own three ids.
    pub fn ids(surface: &str, label: &str, tap: &str, word: &'a str, event: &'a str, kind: Btn) -> Self {
        Self { ids: (surface.to_owned(), label.to_owned(), tap.to_owned()), word, event, kind, red: false }
    }

    /// Its word in red: what it does can't be undone.
    pub fn red(mut self) -> Self {
        self.red = true;
        self
    }
}

/// Open a Mac alert in `ids`' frame: the window's dim, and a [`ALERT_W`]-point
/// panel hanging from the toolbar with the app's icon at its top, as the
/// native apps' alerts (16 points at its sides and foot, 32 over the icon).
/// Its words go in with [`alert_words`], its buttons with [`alert_buttons`];
/// [`shell_close`] closes it. (Desk only: a phone keeps its own dialogs.)
pub fn mac_alert_open_ids(d: &mut Dsl, ids: &ShellIds, icon_id: &str) {
    if crate::desk::active() {
        d.sheet = Some(SheetFoot::default());
    }
    d.open(
        ids.root,
        "KeyboardView",
        &format!("width: Fill height: Fill flow: Overlay align: {} keyboard_min_shift: 56.", sheet_align()),
    );
    d.rule(ids.backdrop, "width: Fill height: Fill", crate::desk::by(tok::MASK, mac_mask()));
    d.view(ids.backdrop_box, "width: Fill height: Fill flow: Overlay");
    match ids.backdrop_event {
        Some(ev) => d.tap(ids.backdrop_hit, ev),
        None => {
            let _ = writeln!(
                d.out,
                "{} := Button {{ width: Fill height: Fill text: \"\" draw_bg.color: #00000000 draw_bg.color_hover: #00000000 draw_bg.color_down: #00000000 draw_bg.border_size: 0.0 draw_bg.color_2: #00000000 draw_bg.border_color: #00000000 draw_bg.border_color_2: #00000000 }}",
                ids.backdrop_hit
            );
        }
    }
    d.close();
    d.sheet_surface(
        ids.dialog,
        &format!(
            "width: {} height: Fit flow: Down align: Align{{x: 0.5 y: 0.0}} padding: Inset{{left: 16 right: 16 top: 32 bottom: 16}}",
            fmt_num(ALERT_W)
        ),
        10.0,
    );
    app_icon(d, icon_id);
}

/// [`mac_alert_open_ids`] in board 3's own frame.
pub fn mac_alert_open(d: &mut Dsl, icon_id: &str) {
    mac_alert_open_ids(d, &B3_IDS, icon_id);
}

/// The app's own icon at 64 points, as a Mac alert carries it: the art the
/// shell draws in the Dock (`AppIcon`, the same in every app).
pub fn app_icon(d: &mut Dsl, id: &str) {
    d.open(id, "AppIcon", "name: \"octoscode\" width: 64 height: 64");
    d.close();
}

/// A Mac alert's title (bold, 13 points) and its words under it (11 points),
/// centred under the icon. Each of `words` is `(id, text)`.
pub fn alert_words(d: &mut Dsl, title: (&str, &str), words: &[(&str, &str)]) {
    d.gap(W::Fill, 10.0);
    d.text(title.0, title.1, &Txt::new(13.0, Face::Semibold, tok::TEXT).w(W::Fill).wrap().centre());
    for (id, text) in words.iter().filter(|(_, t)| !t.is_empty()) {
        d.gap(W::Fill, 5.0);
        d.text(id, text, &Txt::new(11.0, Face::Regular, tok::TEXT).w(W::Fill).wrap().centre());
    }
}

/// Whether two buttons fit side by side, each as wide as the other.
fn alert_pair_fits(buttons: &[AlertButton]) -> bool {
    let half = (ALERT_INNER - 8.0) / 2.0;
    buttons.iter().all(|b| text_w(&crate::desk::title(tr(b.word)), 13.0, Face::Medium) + 24.0 <= half)
}

/// A Mac alert's buttons, given in reading order with the default last: one
/// alone takes the whole width; two sit side by side, as wide as each other,
/// the default on the right, when their words fit; otherwise, and with three or
/// more, they stack, the default on top. 16 points under the words.
pub fn alert_buttons(d: &mut Dsl, buttons: &[AlertButton]) {
    let beside = buttons.len() <= 2 && alert_pair_fits(buttons);
    let row = d.anon();
    let flow = if beside { "Right" } else { "Down" };
    d.view(&row, &format!("width: Fill height: Fit flow: {flow} spacing: 8 margin: Inset{{top: 16}}"));
    let order: Vec<&AlertButton> = if beside { buttons.iter().collect() } else { buttons.iter().rev().collect() };
    for b in order {
        if b.red && !matches!(b.kind, Btn::Disabled | Btn::OutlineOff) {
            d.red_word();
        }
        d.button_ids(&b.ids.0, &b.ids.1, &b.ids.2, b.word, b.event, b.kind, W::Fill, 28.0);
    }
    d.close();
}

/// The standard header row: title on the left, an optional muted subtitle
/// under it, the close glyph on the right (the web's `.header` flex row,
/// `InventoryDialog.module.css:20-26`).
pub fn header(d: &mut Dsl, title_text: &str, close_event: &str) {
    let row = d.anon();
    d.view(&row, "width: Fill height: 32 flow: Right align: Align{x: 0.0 y: 0.5}");
    d.text("b3_title", tr(title_text), &title().w(W::Fill));
    close_glyph(d, close_event);
    d.close();
}

/// The 28x28 close target with the module's own close icon.
pub fn close_glyph(d: &mut Dsl, event: &str) {
    close_glyph_id(d, B3_IDS.close, event);
}

/// [`close_glyph`] under another tap id (`<id>_box`, `<id>_icon`, `<id>`).
pub fn close_glyph_id(d: &mut Dsl, id: &str, event: &str) {
    // A Mac sheet closes from its foot ([`shell_close`]).
    if let Some(sheet) = &mut d.sheet {
        sheet.close = Some((id.to_owned(), event.to_owned()));
        return;
    }
    d.view(&format!("{id}_box"), "width: 28 height: 28 flow: Overlay align: Align{x: 0.5 y: 0.5}");
    d.icon(&format!("{id}_icon"), "b3_close.svg", 15.0, tok::TEXT);
    d.tap(id, event);
    d.close();
}

/// The session / Profile scope line under a dialog's title (the board's
/// `dsflash:main` subtitle, the web's `.scope`).
pub fn scope() -> Txt {
    Txt::new(11.5, Face::Mono, tok::MUTED)
}

/// A 28x28 icon button (refresh, copy).
pub fn icon_button(d: &mut Dsl, id: &str, file: &str, size: f64, event: &str) {
    d.view(&format!("{id}_box"), "width: 28 height: 28 flow: Overlay align: Align{x: 0.5 y: 0.5}");
    d.icon(&format!("{id}_icon"), file, size, tok::MUTED);
    d.tap(id, event);
    d.close();
}

/// Open a bordered section card (the board's grouped boxes, radius 12).
pub fn card_open(d: &mut Dsl, id: &str, spacing: f64) {
    d.surface(
        id,
        &format!(
            "width: Fill height: Fit flow: Down spacing: {} padding: Inset{{left: 14 right: 14 top: 12 bottom: 12}}",
            fmt_num(spacing)
        ),
        tok::SURFACE,
        crate::desk::by(12.0, 8.0),
        Some(tok::HAIRLINE),
    );
}

/// A section heading inside a card (13px medium, the board's "Thread graph").
pub fn section_title(d: &mut Dsl, id: &str, text: &str) {
    d.text(id, tr(text), &Txt::new(13.0, Face::Semibold, tok::TEXT).w(W::Fill));
}

/// A small grey field label above a control ("Workspace path").
pub fn field_label(d: &mut Dsl, id: &str, text: &str) {
    d.text(id, tr(text), &Txt::new(12.0, Face::Medium, tok::MUTED).w(W::Fill));
}

/// The width of a Mac form's label column that holds these labels.
pub fn form_key_w(labels: &[&str]) -> f64 {
    labels
        .iter()
        .map(|l| (text_w(tr(l), 13.0, Face::Regular) * 1.08).ceil())
        .fold(60.0, f64::max)
        .min(240.0)
}

/// Open one line of a form. On a Mac the label sits to the left of its control,
/// right-aligned in a column `key_w` wide, and what follows up to [`form_close`]
/// is the control's column. A phone's label stands above its control.
pub fn form_open(d: &mut Dsl, id: &str, text: &str, key_w: f64) {
    if !crate::desk::active() {
        field_label(d, id, text);
        return;
    }
    let line = d.anon();
    d.view(&line, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.0} spacing: 8");
    let key = d.anon();
    d.view(&key, &format!("width: {} height: 28 flow: Right align: Align{{x: 1.0 y: 0.5}}", fmt_num(key_w)));
    d.text(id, tr(text), &Txt::new(13.0, Face::Regular, tok::TEXT));
    d.close();
    let control = d.anon();
    d.view(&control, "width: Fill height: Fit flow: Down spacing: 4");
}

/// Close what [`form_open`] opened.
pub fn form_close(d: &mut Dsl) {
    if crate::desk::active() {
        d.close();
        d.close();
    }
}

/// A form's note under its control column (a phone's runs the full width).
pub fn form_note(d: &mut Dsl, id: &str, text: &str, key_w: f64) {
    let ink = Txt::new(crate::desk::by(12.0, 11.0), Face::Regular, tok::MUTED).w(W::Fill).wrap();
    if !crate::desk::active() {
        d.text(id, text, &ink);
        return;
    }
    let line = d.anon();
    d.view(&line, &format!("width: Fill height: Fit flow: Down padding: Inset{{left: {}}}", fmt_num(key_w + 8.0)));
    d.text(id, text, &ink);
    d.close();
}

/// An alert's box: on a Mac, plain words in their place beside their mark, not
/// a tinted card; elsewhere the board's box. Closes with [`Dsl::close`].
pub fn alert_open(d: &mut Dsl, id: &str, props: &str, fill: &str, radius: f64, edge: Option<&str>) {
    if crate::desk::active() {
        d.view(id, "width: Fill height: Fit flow: Right spacing: 8 align: Align{x: 0.0 y: 0.0}");
    } else {
        d.surface(id, props, fill, radius, edge);
    }
}

/// The amber caution banner (screen 7): warning glyph, a bold line, a body.
pub fn banner(d: &mut Dsl, id: &str, head: &str, body: &str) {
    alert_open(
        d,
        id,
        "width: Fill height: Fit flow: Right spacing: 10 padding: Inset{left: 12 right: 12 top: 10 bottom: 10}",
        tok::AMBER_BG,
        crate::desk::by(10.0, 6.0),
        Some(tok::AMBER_LINE),
    );
    d.icon(&format!("{id}_icon"), "b3_warning.svg", 16.0, tok::AMBER);
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 3");
    d.text(&format!("{id}_head"), tr(head), &Txt::new(12.5, Face::Medium, tok::TEXT).w(W::Fill).wrap());
    if !body.is_empty() {
        d.text(&format!("{id}_body"), tr(body), &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    }
    d.close();
    d.close();
}

/// A read-only monospace value in a hairline box (the board's link field),
/// with an optional trailing icon button.
pub fn mono_box(d: &mut Dsl, id: &str, value: &str, trailing: Option<(&str, &str, &str)>) {
    d.surface(
        id,
        &format!(
            "width: Fill height: {} flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 6 padding: Inset{{left: {} right: {} top: 0 bottom: 0}}",
            crate::desk::by(36, 28),
            crate::desk::by(10, 8),
            crate::desk::by(4, 2),
        ),
        tok::SURFACE,
        crate::desk::by(8.0, 6.0),
        Some("#d9d9dcff"),
    );
    d.text(&format!("{id}_value"), value, &Txt::new(12.0, Face::Mono, tok::TEXT).w(W::Fill));
    if let Some((tid, file, event)) = trailing {
        icon_button(d, tid, file, 14.0, event);
    }
    d.close();
}

/// The web's relative-time label (`features/shell/relative-time.ts:1-17`):
/// `now` under a minute, then `Nm`, `Nh`, `Nd` under a week, then a short
/// date (`Sep 24`). `then_ms`/`now_ms` are Unix milliseconds.
pub fn rel_time(now_ms: u64, then_ms: u64) -> String {
    // A24: in the interface language (刚刚 / N 分钟前 / N 小时前 / N 天前), the
    // sidebar's own buckets (`sidebar::relative_label`) — the web prints them
    // without t().
    if crate::desk::active() && !crate::i18n::is_zh() {
        return crate::screens::sidebar::mac_list_date(then_ms, now_ms);
    }
    let secs = now_ms.saturating_sub(then_ms) / 1000;
    if secs < 60 {
        return tr("now").into();
    }
    let mins = secs / 60;
    if mins < 60 {
        return crate::i18n::tr1("{value0}m", &mins.to_string());
    }
    let hours = mins / 60;
    if hours < 24 {
        return crate::i18n::tr1("{value0}h", &hours.to_string());
    }
    let days = hours / 24;
    if days < 7 {
        return crate::i18n::tr1("{value0}d", &days.to_string());
    }
    short_date(then_ms)
}

/// A24 — a past moment as people read it, in the current language: "just
/// now", "5m ago", "3h ago", "2d ago" (刚刚 / 5 分钟前 / 3 小时前 / 2 天前),
/// then a short date past a week (no "ago" after a date).
pub fn rel_ago(now_ms: u64, then_ms: u64) -> String {
    let secs = now_ms.saturating_sub(then_ms) / 1000;
    if secs < 60 {
        return tr("just now").to_owned();
    }
    let mins = secs / 60;
    if mins < 60 {
        return crate::i18n::tr1("{value0}m ago", &mins.to_string());
    }
    let hours = mins / 60;
    if hours < 24 {
        return crate::i18n::tr1("{value0}h ago", &hours.to_string());
    }
    let days = hours / 24;
    if days < 7 {
        return crate::i18n::tr1("{value0}d ago", &days.to_string());
    }
    short_date(then_ms)
}

/// `Intl.DateTimeFormat(undefined, {month: "short", day: "numeric"})` in the
/// en-US shape (`Sep 24`), computed in UTC (no tz database in the module).
pub fn short_date(ms: u64) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let days = (ms / 86_400_000) as i64;
    // Civil-from-days (Howard Hinnant), UTC.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    // The web's Intl short month + day in the interface language ("10月1日").
    if crate::i18n::is_zh() {
        return format!("{month}月{day}日");
    }
    format!("{} {}", MONTHS[(month - 1) as usize], day)
}

/// Parse an RFC 3339 / ISO-8601 UTC timestamp (`2026-10-01T15:04:05Z`, with
/// optional fraction or `+00:00`) to Unix ms. Server `updated_at` values use
/// this shape; anything else is `None` (the row then shows no time).
pub fn parse_iso_ms(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }
    let num = |a: usize, b: usize| s.get(a..b)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, se) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    // days-from-civil
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let mut ms = ((days * 86_400 + h * 3600 + mi * 60 + se) * 1000) as i64;
    // An explicit numeric offset (`+08:00`) shifts to UTC.
    let tail = &s[19..];
    let tail = tail.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    if let Some(off) = tail.strip_prefix('+').or_else(|| tail.strip_prefix('-')) {
        if off.len() >= 5 {
            let oh: i64 = off[0..2].parse().ok()?;
            let om: i64 = off[3..5].parse().ok()?;
            let sign = if tail.starts_with('+') { 1 } else { -1 };
            ms -= sign * (oh * 3600 + om * 60) * 1000;
        }
    }
    (ms >= 0).then_some(ms as u64)
}

/// Unix ms now.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The last path segment (`/home/user/octos` → `octos`), the web's
/// `workspaceName` rule.
pub fn leaf(path: &str) -> String {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(path)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A13 — developer wording is recognised (the client's method-prefixed
    /// rpc / decode / transport errors, the web's scope refusals); sentences
    /// for people are not.
    #[test]
    fn protocol_errors_are_told_from_sentences_for_people() {
        for raw in [
            "session/hydrate: bad result: missing field `session_id`",
            "tool/status/list: rpc error -32601 (method not found)",
            "session/open: transport: channel closed",
            "Invalid or wrong-scope tool status",
            "agent/status/read: returned agent \"a2\"",
        ] {
            assert!(is_protocol_error(raw), "{raw}");
        }
        for plain in [
            "History changed. Reload the checkpoint picker.",
            "The Session became active. Wait before rewinding.",
            "Couldn't delete the session: session is busy",
            "A confirmed session and profile are required",
            "Upload was not confirmed (500).",
        ] {
            assert!(!is_protocol_error(plain), "{plain}");
        }
        // failure(): the lead first (red, medium), the cause muted under it,
        // whitespace collapsed and capped at 512 characters like the web.
        let mut d = Dsl::new();
        failure(&mut d, "x_error", "Couldn't read the tools for this session.", &format!("a/b: rpc error 1 ({})", "z ".repeat(400)));
        let dsl = d.finish();
        let lead = dsl.find("x_error := Label").unwrap();
        let detail = dsl.find("x_error_detail := Label").unwrap();
        assert!(lead < detail && dsl[lead..detail].contains(tok::RED_TEXT) && dsl[detail..].contains(tok::MUTED));
        let cause = dsl[detail..].split("text: \"").nth(1).unwrap().split('"').next().unwrap();
        assert_eq!(cause.chars().count(), 512);
        // error_line(): a plain message shows alone.
        let mut d = Dsl::new();
        error_line(&mut d, "y_error", "Lead", "Couldn't delete the session: session is busy");
        let dsl = d.finish();
        assert!(!dsl.contains("Lead") && !dsl.contains("y_error_detail"), "{dsl}");
    }

    /// A13 — the middle ellipsis keeps both ends and fits the budget.
    #[test]
    fn fit_middle_keeps_the_head_and_the_tail() {
        let s = "abcdefghijklmnopqrstuvwxyz0123456789";
        let out = fit_middle(s, 120.0, 12.0, Face::Mono);
        assert!(text_w(&out, 12.0, Face::Mono) <= 120.0, "{out}");
        let (head, tail) = out.split_once('…').unwrap();
        assert!(s.starts_with(head) && s.ends_with(tail) && !head.is_empty() && !tail.is_empty(), "{out}");
        assert_eq!(fit_middle("short", 120.0, 12.0, Face::Mono), "short");
    }

    #[test]
    fn taps_are_in_the_shape_the_shared_tap_path_reads() {
        let mut d = Dsl::new();
        d.view("b3_root", "width: Fill height: Fit flow: Overlay");
        d.button("b3_x", "Close", "b3.close", Btn::Primary, W::Fit, 32.0);
        d.toggle("b3_t", true, "b3.toggle#2");
        d.close();
        let taps = d.taps.clone();
        let dsl = d.finish();
        let parsed = crate::screens::taps::wired_taps(&dsl);
        assert_eq!(parsed, taps, "wired_taps recovers exactly the emitted taps");
        assert_eq!(
            crate::screens::taps::split_row("b3.toggle#2"),
            ("b3.toggle", Some(2))
        );
    }

    #[test]
    fn a_disabled_button_routes_nothing() {
        let mut d = Dsl::new();
        d.button("b3_go", "Resume chat", "b3.resume.confirm", Btn::Disabled, W::Fill, 36.0);
        assert!(d.taps.is_empty());
        assert!(!d.finish().contains("NAV("));
    }

    #[test]
    fn the_dialog_width_is_the_web_min_rule() {
        let desk = Frame::DESKTOP;
        assert_eq!(desk.dialog_w(760.0), 760.0);
        let phone = Frame { avail_w: 360.0, avail_h: 780.0 };
        assert_eq!(phone.dialog_w(760.0), 328.0);
        assert!(phone.compact(328.0));
        assert!(!desk.compact(760.0));
        assert_eq!(desk.dialog_max_h(), 571.0);
    }

    #[test]
    fn the_body_is_capped_by_the_frame_minus_chrome() {
        let desk = Frame::DESKTOP;
        let mut d = Dsl::new();
        shell_open(&mut d, &desk, 760.0);
        body_open(&mut d, &desk, 760.0, 150.0);
        body_close(&mut d);
        shell_close(&mut d);
        let dsl = d.finish();
        // 571 max - 2*20 padding - 150 chrome = 381
        assert!(dsl.contains("height: Fit max_height: 381"), "{dsl}");
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
    }

    #[test]
    fn relative_time_is_the_web_formatter() {
        let now = 1_759_331_045_000; // 2025-10-01T15:04:05Z
        assert_eq!(rel_time(now, now - 30_000), "now");
        assert_eq!(rel_time(now, now - 2 * 60_000), "2m");
        assert_eq!(rel_time(now, now - 3 * 3_600_000), "3h");
        assert_eq!(rel_time(now, now - 2 * 86_400_000), "2d");
        assert_eq!(rel_time(now, now - 8 * 86_400_000), "Sep 23");
        assert_eq!(parse_iso_ms("2025-10-01T15:04:05Z"), Some(now));
        assert_eq!(parse_iso_ms("2025-10-01T15:04:05.123+00:00"), Some(now));
        assert_eq!(parse_iso_ms("2025-10-01T23:04:05+08:00"), Some(now));
        assert_eq!(parse_iso_ms("yesterday"), None);
        assert_eq!(leaf("/home/user/octos/"), "octos");
    }

    #[test]
    fn runtime_strings_are_escaped() {
        assert_eq!(lit("a \"b\" \\ c"), "\"a \\\"b\\\" \\\\ c\"");
        assert_eq!(idfrag("fs.read/x"), "fs_read_x");
    }
}
