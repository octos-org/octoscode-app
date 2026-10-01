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
pub mod tok {
    pub const TEXT: &str = "#1d1d1fff";
    pub const MUTED: &str = "#6e6e73ff";
    pub const FAINT: &str = "#a1a1a6ff";
    pub const HAIRLINE: &str = "#e5e5e7ff";
    pub const SURFACE: &str = "#ffffffff";
    pub const SURFACE2: &str = "#f7f7f8ff";
    pub const CHIP: &str = "#f0f0f2ff";
    pub const BLACK: &str = "#000000ff";
    pub const WHITE: &str = "#ffffffff";
    pub const BLUE: &str = "#2f6febff";
    pub const BLUE_BG: &str = "#eaf1fdff";
    pub const GREEN: &str = "#1f883dff";
    pub const GREEN_BG: &str = "#e6f4eaff";
    pub const RED: &str = "#cf222eff";
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

    /// Open a block: `<id> := <kind> {` then the property line.
    pub fn open(&mut self, id: &str, kind: &str, props: &str) {
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
    /// only documents the intent at the call site.
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
        let (fill, fg, border) = match kind {
            Btn::Primary => (tok::BLACK, tok::WHITE, None),
            Btn::Outline => (tok::SURFACE, tok::TEXT, Some(tok::HAIRLINE)),
            Btn::Disabled => (tok::DISABLED_BG, tok::FAINT, None),
            Btn::Ghost => (tok::TRANSPARENT, tok::TEXT, None),
        };
        let radius = if matches!(kind, Btn::Outline) && height > 34.0 { 10.0 } else { height / 2.0 };
        self.surface(
            &format!("{id}_box"),
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
            "width: Fill height: Fill flow: Right align: Align{x: 0.5 y: 0.5} padding: Inset{left: 16 right: 16 top: 0 bottom: 0}",
        );
        self.text(&format!("{id}_label"), label, &Txt::new(13.0, Face::Medium, fg));
        self.close();
        if kind != Btn::Disabled {
            self.tap(id, event);
        }
        self.close();
    }

    /// A text link (blue), optionally tappable.
    pub fn link(&mut self, id: &str, label: &str, event: Option<&str>, px: f64) {
        let wrap = format!("{id}_box");
        self.view(&wrap, "width: Fit height: Fit flow: Overlay");
        self.text(&format!("{id}_label"), label, &Txt::new(px, Face::Regular, tok::BLUE));
        if let Some(ev) = event {
            self.tap(id, ev);
        }
        self.close();
    }

    /// The board's switch: a blue (on) / grey (off) track with a white knob.
    pub fn toggle(&mut self, id: &str, on: bool, event: &str) {
        let track = if on { tok::BLUE } else { "#d1d1d6ff" };
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
        let face = if mono { Face::Mono } else { Face::Regular };
        let style = text_style(face, 13.0);
        let props = format!(
            "width: Fill height: Fit padding: Inset{{left: 0 right: 0 top: 4 bottom: 4}} margin: 0\ntext: {} empty_text: {}\nflow: Right is_read_only: false\ndraw_bg +: {{pixel: fn() {{return vec4(0.0, 0.0, 0.0, 0.0)}}}}\ndraw_text +: {{color: {t} color_hover: {t} color_focus: {t} color_down: {t} color_disabled: {f} color_empty: {f} color_empty_hover: {f} color_empty_focus: {f}}}\ndraw_text.text_style: {style}\ndraw_cursor +: {{color: {t}}}\ndraw_selection +: {{color: #2f6feb33 color_hover: #2f6feb33 color_focus: #2f6feb40 color_down: #2f6feb40 color_empty: #00000000 color_disabled: #00000000}}",
            lit(text),
            lit(placeholder),
            t = tok::TEXT,
            f = tok::FAINT,
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
        self.surface(
            &format!("{id}_track"),
            &format!(
                "width: {} height: 34 flow: Right spacing: 2 padding: Inset{{left: 3 right: 3 top: 3 bottom: 3}}",
                width.dsl()
            ),
            tok::SURFACE2,
            9.0,
            Some(tok::HAIRLINE),
        );
        for (i, (label, event)) in options.iter().enumerate() {
            let on = i == selected;
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

/// Open the backdrop + the centred card. `natural_h` is the content's
/// estimated natural height (header + body + footer, without the card
/// padding). When it does not fit the frame the card pins to the frame's
/// max height and the caller wraps its body in [`scroll_open`] (the web's
/// `max-height` + `overflow: auto`). Returns whether the body must scroll.
pub fn shell_open(d: &mut Dsl, frame: &Frame, width: f64, natural_h: f64) -> bool {
    let max_h = frame.dialog_max_h();
    let pad = dialog_pad(frame, width);
    let card_h = if natural_h + 2.0 * pad > max_h { Some(max_h) } else { None };
    d.view(
        "b3_root",
        "width: Fill height: Fill flow: Overlay align: Align{x: 0.5 y: 0.5}",
    );
    d.rule("b3_backdrop", "width: Fill height: Fill", tok::MASK);
    d.surface(
        "b3_dialog",
        &format!(
            "width: {} height: {} flow: Down padding: Inset{{left: {p} right: {p} top: {p} bottom: {p}}}",
            fmt_num(width),
            card_h.map(fmt_num).unwrap_or_else(|| "Fit".into()),
            p = fmt_num(pad)
        ),
        tok::SURFACE,
        16.0,
        Some(tok::HAIRLINE),
    );
    card_h.is_some()
}

/// The scrolling body region (only when [`shell_open`] said so).
pub fn scroll_open(d: &mut Dsl) {
    // The right inset is the scroll bar's gutter: measured on the phone
    // layout, the bar drew over the first rows' status chips without it.
    d.open(
        "b3_scroll",
        "ScrollYView",
        "width: Fill height: Fill flow: Down padding: Inset{left: 0 top: 0 right: 10 bottom: 0}",
    );
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
    d.view("b3_close_box", "width: 28 height: 28 flow: Overlay align: Align{x: 0.5 y: 0.5}");
    d.icon("b3_close_icon", "icon_close.svg", 12.0, tok::MUTED);
    d.tap("b3_close", event);
    d.close();
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn a_tall_body_pins_the_card_and_scrolls() {
        let desk = Frame::DESKTOP;
        let mut d = Dsl::new();
        assert!(shell_open(&mut d, &desk, 760.0, 900.0));
        let mut d2 = Dsl::new();
        assert!(!shell_open(&mut d2, &desk, 760.0, 300.0));
    }

    #[test]
    fn runtime_strings_are_escaped() {
        assert_eq!(lit("a \"b\" \\ c"), "\"a \\\"b\\\" \\\\ c\"");
        assert_eq!(idfrag("fs.read/x"), "fs_read_x");
    }
}
