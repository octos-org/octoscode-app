//! A4 — the board-3 native surface kit: the renderer's own vocabulary
//! (`DesignSurface`, `Label` with the kit's Inter / LXGW / mono text family,
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
/// kit face for latin, LXGW WenKai for CJK (without it every Chinese glyph is
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
    let cjk = if weight >= 600 { "LXGWWenKaiBold.ttf" } else { "LXGWWenKaiRegular.ttf" };
    let emoji = if cfg!(target_os = "macos") {
        "file_resource(\"/System/Library/Fonts/Apple Color Emoji.ttc\")".to_owned()
    } else {
        "crate_resource(\"makepad_widgets:resources/NotoColorEmoji.ttf\")".to_owned()
    };
    let (asc, desc) = if face == Face::Mono { (0.0, 0.0) } else { (0.04, 0.04) };
    format!(
        "TextStyle{{font_family: FontFamily{{latin := FontMember{{res: file_resource({latin:?}) asc: {asc} desc: {desc} weight: {weight}}} cjk := FontMember{{res: crate_resource(\"makepad_widgets:resources/{cjk}\") asc: 0.0 desc: 0.0 weight: {weight}}} symbols := FontMember{{res: crate_resource(\"makepad_widgets:resources/jetbrains_mono_variable.ttf\") asc: 0 desc: 0 weight: 400}} emoji := FontMember{{res: {emoji} asc: 0 desc: 0}}}} font_size: {} line_spacing: 1.25}}",
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
/// character is one em in every face: the family's LXGW WenKai member draws
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
        out.push_str(line);
        if line.contains("draw_svg.svg:") && !line.contains("draw_svg.color:") {
            out.push_str(" draw_svg.color: ");
            out.push_str(&ink);
        }
        out.push('\n');
    }
    if !dsl.ends_with('\n') {
        out.pop();
    }
    out
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
    d.text(id, lead, &Txt::new(13.0, Face::Medium, tok::RED_TEXT).w(W::Fill).wrap());
    if !cause.is_empty() && cause != lead {
        d.text(&format!("{id}_detail"), &cause, &Txt::new(11.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    }
    d.close();
}

/// A13 — an error line that may be either: developer wording
/// ([`is_protocol_error`]) gets the plain `lead` over it ([`failure`]); a
/// message already written for people ("History belongs to another
/// Session.") shows alone, red, as before.
pub fn error_line(d: &mut Dsl, id: &str, lead: &str, msg: &str) {
    if is_protocol_error(msg) {
        failure(d, id, lead, msg);
    } else {
        d.text(id, &clean_cause(msg), &Txt::new(12.5, Face::Regular, tok::RED_TEXT).w(W::Fill).wrap());
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
}

impl Txt {
    pub fn new(px: f64, face: Face, color: &'static str) -> Self {
        Self { px, face, color, width: W::Fit, wrap: false }
    }
    pub fn w(mut self, width: W) -> Self {
        self.width = width;
        self
    }
    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }
}

/// The type ramp (design px). Title/section/body/meta/micro per the board.
pub fn title() -> Txt {
    Txt::new(17.0, Face::Semibold, tok::TEXT)
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

/// The line-based DSL builder. Blocks open with `<id> := <Kind> {` on their
/// own line and close with a lone `}`, which is the shape
/// `taps::wired_taps` reads.
#[derive(Default)]
pub struct Dsl {
    out: String,
    seq: usize,
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
        self.out
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
        let border_line = match border {
            Some(c) => format!("draw_bg.border_width: 1 draw_bg.border_color: {c}"),
            None => "draw_bg.border_width: 0 draw_bg.border_color: #00000000".to_owned(),
        };
        let line = format!(
            "{props}\nshow_bg: true draw_bg.color: {fill} draw_bg.radius: {} draw_bg.ellipse: 0 draw_bg.border_position: 0 {border_line}",
            fmt_num(radius)
        );
        self.open(id, "DesignSurface", &line);
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
        let flow = if t.wrap { "flow: Right{wrap: true}" } else { "flow: Right" };
        let props = format!(
            "width: {} height: Fit padding: 0 text: {} {flow}\ndraw_text.text_style: {}\ndraw_text.color: {}",
            t.width.dsl(),
            lit(s),
            text_style(t.face, t.px),
            t.color
        );
        self.open(&id, "Label", &props);
        self.close();
    }

    /// A line icon from the module's `resources/icons/`. The stroke colour is
    /// baked into each file (the module's own icons do the same), so `_color`
    /// only documents the intent at the call site. On a surface that follows
    /// the theme, [`themed_icons`] tints them in dark.
    pub fn icon(&mut self, id: &str, file: &str, size: f64, _color: &str) {
        let id = if id.is_empty() { self.anon() } else { id.to_owned() };
        let path = crate::design::icon_resource(file);
        let props = format!(
            "width: {s} height: {s} animating: false draw_svg.svg: file_resource({path:?}) draw_svg.preserve_viewbox: true",
            s = fmt_num(size)
        );
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
        self.taps.push((id.to_owned(), event.to_owned()));
        let _ = writeln!(self.out, "{id} := DesignNativeButton {{");
        self.raw("width: Fill height: Fill");
        let _ = writeln!(self.out, "on_click: || {{ NAV(t: {event:?}) }}");
        self.raw("}");
    }

    /// A chip: tinted rounded label (`enabled`, `stdio`, `unverified`).
    pub fn chip(&mut self, id: &str, s: &str, fg: &'static str, bg: &str, border: Option<&str>, mono: bool) {
        let wrap_id = if id.is_empty() { self.anon() } else { format!("{id}_chip") };
        self.surface(
            &wrap_id,
            "width: Fit height: 20 flow: Right align: Align{x: 0.5 y: 0.5} padding: Inset{left: 7 right: 7 top: 0 bottom: 0}",
            bg,
            6.0,
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
        let (fill, fg, border) = match kind {
            Btn::Primary => (tok::BLACK, tok::WHITE, None),
            Btn::Outline => (tok::SURFACE, tok::TEXT, Some("#c7c7ccff")),
            Btn::OutlineOff => (tok::SURFACE, tok::DISABLED_INK, Some(tok::HAIRLINE)),
            Btn::Secondary => (tok::CHIP, tok::TEXT, None),
            Btn::Disabled => (tok::DISABLED_BG, tok::DISABLED_INK, None),
            Btn::Ghost => (tok::TRANSPARENT, tok::TEXT, None),
        };
        let radius = if matches!(kind, Btn::Outline | Btn::OutlineOff) && height > 34.0 {
            10.0
        } else {
            height / 2.0
        };
        // A Fit pill gets an explicit width (see `text_w`).
        let width = match width {
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
        self.text(label_id, label, &Txt::new(13.0, Face::Medium, fg));
        self.close();
        if !matches!(kind, Btn::Disabled | Btn::OutlineOff) {
            self.tap(tap_id, event);
        }
        self.close();
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
        // A 44x32 hit box around the 38x22 track (>= 28 px to tap).
        self.view(&format!("{id}_box"), "width: 44 height: 32 flow: Overlay align: Align{x: 0.5 y: 0.5}");
        self.surface(
            &format!("{id}_track"),
            &format!(
                "width: 38 height: 22 flow: Overlay align: Align{{x: {} y: 0.5}} padding: Inset{{left: 2 right: 2 top: 0 bottom: 0}}",
                if on { "1.0" } else { "0.0" }
            ),
            track,
            11.0,
            None,
        );
        let knob = self.anon();
        self.surface(&knob, "width: 18 height: 18", tok::WHITE, 9.0, None);
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
        self.surface(
            &format!("{id}_field"),
            &format!(
                "width: Fill height: {} flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 10 right: 10 top: 0 bottom: 0}}",
                fmt_num(height)
            ),
            tok::SURFACE,
            8.0,
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
        self.surface(
            &format!("{id}_field"),
            &format!(
                "width: Fill height: {} flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 10 right: 10 top: 0 bottom: 0}}",
                fmt_num(height)
            ),
            tok::SURFACE,
            8.0,
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
        self.surface(
            &format!("{id}_field"),
            &format!(
                "width: Fill height: {} flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 8 padding: Inset{{left: 10 right: 10 top: 0 bottom: 0}}",
                fmt_num(height)
            ),
            tok::SURFACE,
            8.0,
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
            8.0,
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
            8.0,
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
                label,
                &Txt::new(13.0, if on { Face::Medium } else { Face::Regular }, fg),
            );
            self.close();
            self.tap(&seg, event);
            self.close();
        }
        self.close();
    }
}

/// The kit's `TextInput` properties: 13 px text (mono or Inter), transparent
/// background (the field surface paints), faint placeholder; `multiline`
/// wraps and fills the field.
fn input_props(text: &str, placeholder: &str, mono: bool, multiline: bool) -> String {
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
        lit(placeholder),
        t = tok::TEXT,
        f = tok::FAINT,
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
        (self.avail_w - 32.0).min(max).max(240.0).floor()
    }

    /// The dialog's maximum height (`calc(100dvh - 32px)`).
    pub fn dialog_max_h(&self) -> f64 {
        (self.avail_h - 32.0).max(200.0).floor()
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
    // A8 — the root pans its content above an on-screen keyboard (makepad's
    // `KeyboardView`: the focused field stays visible while typing, the way a
    // phone browser scrolls the web dialog's focused input into view). The
    // kept gap (56 = 8 + a 38 button + 10) leaves a field's primary button,
    // set right under it, above the keyboard too. No keyboard (the desktop),
    // no shift.
    d.open(
        ids.root,
        "KeyboardView",
        "width: Fill height: Fill flow: Overlay align: Align{x: 0.5 y: 0.5} keyboard_min_shift: 56.",
    );
    d.rule(ids.backdrop, "width: Fill height: Fill", tok::MASK);
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
    d.surface(
        ids.dialog,
        &format!(
            "width: {} height: Fit flow: Down padding: Inset{{left: {p} right: {p} top: {p} bottom: {p}}}",
            fmt_num(width),
            p = fmt_num(pad)
        ),
        tok::SURFACE,
        16.0,
        Some(tok::HAIRLINE),
    );
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
    let max_body = (frame.dialog_max_h() - 2.0 * pad - chrome_h).max(120.0).floor();
    // The right inset is the scroll bar's gutter: measured on the phone
    // layout, the bar drew over the first rows' status chips without it.
    d.open(
        scroll_id,
        "ScrollYView",
        &format!(
            "width: Fill height: Fit max_height: {} flow: Down padding: Inset{{left: 0 top: 0 right: 10 bottom: 0}}",
            fmt_num(max_body)
        ),
    );
}

/// Close [`body_open`].
pub fn body_close(d: &mut Dsl) {
    d.close();
}

/// Close the card and the backdrop root.
pub fn shell_close(d: &mut Dsl) {
    d.close();
    d.close();
}

/// The standard header row: title on the left, an optional muted subtitle
/// under it, the close glyph on the right (the web's `.header` flex row,
/// `InventoryDialog.module.css:20-26`).
pub fn header(d: &mut Dsl, title_text: &str, close_event: &str) {
    let row = d.anon();
    d.view(&row, "width: Fill height: 32 flow: Right align: Align{x: 0.0 y: 0.5}");
    d.text("b3_title", title_text, &title().w(W::Fill));
    close_glyph(d, close_event);
    d.close();
}

/// The 28x28 close target with the module's own close icon.
pub fn close_glyph(d: &mut Dsl, event: &str) {
    close_glyph_id(d, B3_IDS.close, event);
}

/// [`close_glyph`] under another tap id (`<id>_box`, `<id>_icon`, `<id>`).
pub fn close_glyph_id(d: &mut Dsl, id: &str, event: &str) {
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
        12.0,
        Some(tok::HAIRLINE),
    );
}

/// A section heading inside a card (13px medium, the board's "Thread graph").
pub fn section_title(d: &mut Dsl, id: &str, text: &str) {
    d.text(id, text, &Txt::new(13.0, Face::Semibold, tok::TEXT).w(W::Fill));
}

/// A small grey field label above a control ("Workspace path").
pub fn field_label(d: &mut Dsl, id: &str, text: &str) {
    d.text(id, text, &Txt::new(12.0, Face::Medium, tok::MUTED).w(W::Fill));
}

/// The amber caution banner (screen 7): warning glyph, a bold line, a body.
pub fn banner(d: &mut Dsl, id: &str, head: &str, body: &str) {
    d.surface(
        id,
        "width: Fill height: Fit flow: Right spacing: 10 padding: Inset{left: 12 right: 12 top: 10 bottom: 10}",
        tok::AMBER_BG,
        10.0,
        Some(tok::AMBER_LINE),
    );
    d.icon(&format!("{id}_icon"), "b3_warning.svg", 16.0, tok::AMBER);
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 3");
    d.text(&format!("{id}_head"), head, &Txt::new(12.5, Face::Medium, tok::TEXT).w(W::Fill).wrap());
    if !body.is_empty() {
        d.text(&format!("{id}_body"), body, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    }
    d.close();
    d.close();
}

/// A read-only monospace value in a hairline box (the board's link field),
/// with an optional trailing icon button.
pub fn mono_box(d: &mut Dsl, id: &str, value: &str, trailing: Option<(&str, &str, &str)>) {
    d.surface(
        id,
        "width: Fill height: 36 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 6 padding: Inset{left: 10 right: 4 top: 0 bottom: 0}",
        tok::SURFACE,
        8.0,
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
    let secs = now_ms.saturating_sub(then_ms) / 1000;
    if secs < 60 {
        return "now".into();
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m");
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours}h");
    }
    let days = hours / 24;
    if days < 7 {
        return format!("{days}d");
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
