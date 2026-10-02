//! #A2 — the native view vocabulary for board 1 (`design/stage-a/phase4-new`).
//!
//! The nine board-1 screens used to mount as Stage-B L0 cards: measured
//! 406x776 phone artboards with every node at an `abs_pos`. That frozen phone
//! width cannot fit a 990x603 desktop window (the artboard is taller than the
//! window) nor a 360-wide phone (it is wider), and a server folder list or a
//! provider's model list is runtime data, which RULES 8.10 says must flow, not
//! sit in measured boxes. So the screens are emitted here as flowing makepad
//! DSL — the board's visual language (atlas-prompt.md: white surfaces,
//! #E5E5E7 hairlines, #1D1D1F ink, #6E6E73 secondary, black pills, #2F6FEB
//! links, #CF222E/#FDECEC errors, Inter, radius 12 cards, 999 pills) with the
//! web's adaptation rule: a centred max-width dialog on a desktop window, a
//! full-width sheet below 560 px (`NewSessionWorkspacePicker.module.css:23`
//! `min(540px, 100vw - 48px)` and its `@media (max-width: 560px)` block).
//!
//! Every helper returns a DSL fragment; ids carry the `b1_` prefix so the host
//! can find a control anywhere in the tree (`board1::collect`).

/// Board palette (atlas-prompt.md "Visual language"). A18: the grey TEXT
/// levels are the web's (`app/theme.css:79-83`) — the board's #6E6E73 read
/// 4.46:1 on `SELECTED`-grey fills and its #8E8E93 3.26:1 on white, below the
/// web's axe gate (WCAG 4.5:1); the pairs are tested in
/// `screens::theme::CONTRAST_PAIRS`.
pub const INK: &str = "#1d1d1fff";
/// `--dsw-alias-label-secondary` (light).
pub const MUTED: &str = "#61666bff";
/// `--dsw-alias-label-tertiary` (light).
pub const FAINT: &str = "#5f646bff";
/// `--dsw-alias-label-caption` (light), the web's placeholder ink
/// (`styles.css` `.composer textarea::placeholder`).
pub const PLACEHOLDER: &str = "#646970ff";
pub const HAIR: &str = "#e5e5e7ff";
pub const FIELD_EDGE: &str = "#d2d2d7ff";
pub const WHITE: &str = "#ffffffff";
pub const SUBTLE: &str = "#f5f5f7ff";
pub const SELECTED: &str = "#efeff1ff";
pub const BLUE: &str = "#2f6febff";
pub const RED: &str = "#cf222eff";
pub const RED_BG: &str = "#fdececff";
pub const RED_EDGE: &str = "#f4c7c9ff";
pub const BLACK: &str = "#000000ff";
pub const CLEAR: &str = "#00000000";

/// Escape a runtime string into a DSL string literal (the renderer does the
/// same with `{:?}`, `octoscript-makepad/src/design.rs:655`).
pub fn lit(s: &str) -> String {
    format!("{s:?}")
}

thread_local! {
    /// The type scale of the view being built: 1.0 in a desktop dialog, a
    /// little larger on a phone sheet (the board's phone artboards set body
    /// text ~16-17 px on a 406 px width). Set by `board1::compose` per view;
    /// thread-local so parallel builds (tests) never see each other's scale.
    static TYPE_SCALE: std::cell::Cell<f64> = const { std::cell::Cell::new(1.0) };
}

/// Build the next views at this type scale (see [`TYPE_SCALE`]).
pub fn set_type_scale(s: f64) {
    TYPE_SCALE.with(|c| c.set(s));
}

fn scaled(px: f64) -> f64 {
    (px * TYPE_SCALE.with(|c| c.get()) * 4.0).round() / 4.0
}

/// One Inter face at `px` logical pixels (the renderer's `size * 0.75` point
/// rule, `design.rs:732`), with the CJK/symbol/emoji fallbacks it uses so a
/// folder or provider name in Chinese still draws.
pub fn font(weight: u16, px: f64) -> String {
    font_spaced(weight, px, 1.0)
}

/// [`font`] with an explicit line spacing (wrapping prose uses ~1.15).
pub fn font_spaced(weight: u16, px: f64, line_spacing: f64) -> String {
    let face = match weight {
        w if w >= 700 => "ux/Inter-700.ttf",
        w if w >= 600 => "ux/Inter-600.ttf",
        w if w >= 500 => "ux/Inter-500.ttf",
        _ => "ux/Inter-400.ttf",
    };
    let latin = crate::design::font_file(face).display().to_string();
    let cjk = if weight >= 600 {
        "LXGWWenKaiBold.ttf"
    } else {
        "LXGWWenKaiRegular.ttf"
    };
    let emoji = if cfg!(target_os = "macos") {
        "file_resource(\"/System/Library/Fonts/Apple Color Emoji.ttc\")"
    } else {
        "crate_resource(\"makepad_widgets:resources/NotoColorEmoji.ttf\")"
    };
    format!(
        "TextStyle{{font_family: FontFamily{{latin := FontMember{{res: file_resource({latin:?}) asc: 0.04 desc: 0.04 weight: {weight}}} cjk := FontMember{{res: crate_resource(\"makepad_widgets:resources/{cjk}\") asc: 0.0 desc: 0.0 weight: {weight}}} symbols := FontMember{{res: crate_resource(\"makepad_widgets:resources/jetbrains_mono_variable.ttf\") asc: 0 desc: 0 weight: 400}} emoji := FontMember{{res: {emoji} asc: 0 desc: 0}}}} font_size: {:.2} line_spacing: {line_spacing}}}",
        scaled(px) * 0.75
    )
}

/// The absolute resource string of one of the module's own icons.
pub fn icon(name: &str) -> String {
    crate::design::icon_resource(name)
}

/// A text run. `width` is a DSL size (`Fit`, `Fill`, a number).
pub struct Text<'a> {
    pub id: &'a str,
    pub text: &'a str,
    pub px: f64,
    pub weight: u16,
    pub color: &'a str,
    pub width: &'a str,
    /// `Align{x}` of the run inside its box (0 left, 0.5 centre, 1 right).
    pub align_x: f64,
    /// One line with an ellipsis (a path, a folder name) vs. wrapping prose.
    pub single_line: bool,
}

impl<'a> Text<'a> {
    pub fn new(id: &'a str, text: &'a str) -> Self {
        Self {
            id,
            text,
            px: 15.0,
            weight: 400,
            color: INK,
            width: "Fit",
            align_x: 0.0,
            single_line: false,
        }
    }
    pub fn px(mut self, px: f64) -> Self {
        self.px = px;
        self
    }
    pub fn weight(mut self, w: u16) -> Self {
        self.weight = w;
        self
    }
    pub fn color(mut self, c: &'a str) -> Self {
        self.color = c;
        self
    }
    pub fn fill(mut self) -> Self {
        self.width = "Fill";
        self
    }
    pub fn centered(mut self) -> Self {
        self.align_x = 0.5;
        self
    }
    pub fn right(mut self) -> Self {
        self.align_x = 1.0;
        self
    }
    pub fn one_line(mut self) -> Self {
        self.single_line = true;
        self
    }
    pub fn dsl(&self) -> String {
        let flow = if self.single_line {
            "flow: Right max_lines: 1 text_overflow: TextOverflow.Ellipsis"
        } else {
            "flow: Right{wrap: true}"
        };
        let spacing = if self.single_line { 1.0 } else { 1.15 };
        let name = if self.id.is_empty() {
            String::new()
        } else {
            format!("{} := ", self.id)
        };
        format!(
            "{name}Label {{\nwidth: {} height: Fit padding: 0 margin: 0 {flow}\nalign: Align{{x: {} y: 0.5}}\ntext: {}\ndraw_text.color: {}\ndraw_text.text_style: {}\n}}\n",
            self.width,
            self.align_x,
            lit(self.text),
            self.color,
            font_spaced(self.weight, self.px, spacing)
        )
    }
}

/// A vertical gap.
pub fn gap(h: f64) -> String {
    format!("View {{ width: Fill height: {h} }}\n")
}

/// A horizontal hairline (the board's 1 px #E5E5E7 divider).
pub fn hairline() -> String {
    format!("SolidView {{ width: Fill height: 1 draw_bg.color: {HAIR} }}\n")
}

/// The black primary pill (`atlas-prompt.md`: solid #000, white text, 999 radius).
pub fn pill_primary(id: &str, label: &str, width: &str) -> String {
    pill(id, label, width, BLACK, "#2a2a2cff", "#3a3a3cff", WHITE, None)
}

/// The secondary outline pill (white, 1.5 px ink edge).
pub fn pill_outline(id: &str, label: &str, width: &str) -> String {
    pill(id, label, width, WHITE, "#f5f5f7ff", "#ececeeff", INK, Some(INK))
}

#[allow(clippy::too_many_arguments)]
fn pill(
    id: &str,
    label: &str,
    width: &str,
    bg: &str,
    hover: &str,
    down: &str,
    ink: &str,
    edge: Option<&str>,
) -> String {
    let (border, edge) = match edge {
        Some(e) => (1.5, e),
        None => (0.0, CLEAR),
    };
    // `Sdf2d.box`'s radius argument is half the drawn corner (the GaussRounded
    // note in octoscript-widgets design.rs:186), so a 44 px pill uses 11.
    format!(
        "{id} := ButtonFlat {{\nwidth: {width} height: 44 padding: 0 margin: 0 align: Align{{x: 0.5 y: 0.5}}\ntext: {}\ndraw_bg +: {{color: {bg} color_hover: {hover} color_down: {down} color_focus: {bg} color_disabled: {bg} border_size: {border} border_radius: 11.0 border_color: {edge} border_color_hover: {edge} border_color_down: {edge} border_color_focus: {edge} border_color_disabled: {edge}}}\ndraw_text +: {{color: {ink} color_hover: {ink} color_down: {ink} color_focus: {ink} color_disabled: {ink} text_style: {}}}\n}}\n",
        lit(label),
        font(600, 15.0)
    )
}

/// A23 — the outline pill at its label's width (the editor's probe actions,
/// "Test connection" / "Fetch available models"): the board's outline pill
/// (1.5 px ink edge, 999 radius) with side padding instead of a Fill width.
pub fn pill_outline_fit(id: &str, label: &str, height: f64) -> String {
    let edge = INK;
    format!(
        "{id} := ButtonFlat {{\nwidth: Fit height: {height} padding: Inset{{left: 18 right: 18}} margin: 0 align: Align{{x: 0.5 y: 0.5}}\ntext: {}\ndraw_bg +: {{color: {WHITE} color_hover: #f5f5f7ff color_down: #ececeeff color_focus: {WHITE} color_disabled: {WHITE} border_size: 1.5 border_radius: {} border_color: {edge} border_color_hover: {edge} border_color_down: {edge} border_color_focus: {edge} border_color_disabled: {edge}}}\ndraw_text +: {{color: {INK} color_hover: {INK} color_down: {INK} color_focus: {INK} color_disabled: {INK} text_style: {}}}\n}}\n",
        lit(label),
        height / 4.0,
        font(600, 14.0)
    )
}

/// A23 — a status line: a round dot and its sentence (board 3's status
/// light, "● connected"): green for a passed probe, red for a failed one.
pub fn status_line(id: &str, text: &str, ok: bool) -> String {
    let (dot, ink) = if ok { ("#1f883dff", "#166534ff") } else { (RED, RED) };
    format!(
        "View {{ width: Fill height: Fit flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 8\nDesignSurface {{ width: 8 height: 8 show_bg: true draw_bg.color: {dot} draw_bg.radius: 4 draw_bg.ellipse: 1 draw_bg.border_width: 0 draw_bg.border_position: 0 draw_bg.border_color: {CLEAR} }}\n{}}}\n",
        Text::new(id, text).px(14.0).color(ink).fill().dsl()
    )
}

/// A text-only control (the board's blue link, or the red "Forget this device").
pub fn link(id: &str, label: &str, color: &str, px: f64, weight: u16) -> String {
    format!(
        "{id} := ButtonFlat {{\nwidth: Fit height: 32 padding: Inset{{left: 6 right: 6}} margin: 0 align: Align{{x: 0.5 y: 0.5}}\ntext: {}\ndraw_bg +: {{color: {CLEAR} color_hover: #00000008 color_down: #00000012 color_focus: {CLEAR} color_disabled: {CLEAR} border_size: 0.0 border_radius: 6.0 border_color: {CLEAR} border_color_hover: {CLEAR} border_color_down: {CLEAR} border_color_focus: {CLEAR} border_color_disabled: {CLEAR}}}\ndraw_text +: {{color: {color} color_hover: {color} color_down: {color} color_focus: {color} color_disabled: {color} text_style: {}}}\n}}\n",
        lit(label),
        font(weight, px)
    )
}

/// A transparent hit target laid over a composite (a row, an icon). Fill/Fill
/// inside an Overlay parent.
pub fn hit(id: &str, hover: bool) -> String {
    let (h, d) = if hover {
        ("#0000000a", "#00000014")
    } else {
        (CLEAR, CLEAR)
    };
    format!(
        "{id} := ButtonFlat {{\nwidth: Fill height: Fill padding: 0 margin: 0 text: \"\"\ndraw_bg +: {{color: {CLEAR} color_hover: {h} color_down: {d} color_focus: {CLEAR} color_disabled: {CLEAR} border_size: 0.0 border_radius: 6.0 border_color: {CLEAR} border_color_hover: {CLEAR} border_color_down: {CLEAR} border_color_focus: {CLEAR} border_color_disabled: {CLEAR}}}\n}}\n"
    )
}

/// An SVG icon of `size` px.
pub fn svg(id: &str, file: &str, size: f64) -> String {
    let name = if id.is_empty() {
        String::new()
    } else {
        format!("{id} := ")
    };
    format!(
        "{name}Svg {{ width: {size} height: {size} animating: false draw_svg.svg: file_resource({:?}) draw_svg.preserve_viewbox: true }}\n",
        icon(file)
    )
}

/// The back chevron (top-left of every board-1 screen): a 32 px square hit
/// target with the 20 px chevron centred in it.
pub fn back_button(id: &str) -> String {
    format!(
        "View {{ width: 32 height: 32 flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n{}{}}}\n",
        // The board draws the back chevron ~17 px tall (26 px glyph box).
        svg("", "b1_chevron_left.svg", 26.0),
        hit(id, true)
    )
}

/// A labelled field: the label above, then a 44 px field. `error` draws the
/// board's red outline (p4-07's rejected key). `trailing` is an optional
/// control on the right inside the field (the key's eye).
pub struct Field<'a> {
    pub id: &'a str,
    pub label: Option<&'a str>,
    pub text: &'a str,
    pub placeholder: &'a str,
    pub password: bool,
    pub error: bool,
    pub read_only: bool,
    pub trailing: Option<String>,
    /// The placeholder draws in ink, not grey: a stored secret's mask (the
    /// board's p4-06 key field shows black dots for a key the profile keeps).
    pub placeholder_ink: bool,
}

impl<'a> Field<'a> {
    pub fn new(id: &'a str, text: &'a str) -> Self {
        Self {
            id,
            label: None,
            text,
            placeholder: "",
            password: false,
            error: false,
            read_only: false,
            trailing: None,
            placeholder_ink: false,
        }
    }
    pub fn placeholder_ink(mut self) -> Self {
        self.placeholder_ink = true;
        self
    }
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }
    pub fn label(mut self, l: &'a str) -> Self {
        self.label = Some(l);
        self
    }
    pub fn placeholder(mut self, p: &'a str) -> Self {
        self.placeholder = p;
        self
    }
    pub fn password(mut self) -> Self {
        self.password = true;
        self
    }
    pub fn error(mut self, e: bool) -> Self {
        self.error = e;
        self
    }
    pub fn trailing(mut self, t: String) -> Self {
        self.trailing = Some(t);
        self
    }
    pub fn dsl(&self) -> String {
        let mut out = String::new();
        if let Some(l) = self.label {
            // The board's field labels are regular weight ("Server", "Name").
            out.push_str(&Text::new("", l).px(15.0).fill().one_line().dsl());
            out.push_str(&gap(8.0));
        }
        let (edge, width) = if self.error { (RED, 1.5) } else { (FIELD_EDGE, 1.0) };
        let trailing_w = if self.trailing.is_some() { 40.0 } else { 0.0 };
        out.push_str(&format!(
            "DesignSurface {{\nwidth: Fill height: 44 flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 14 right: {}}}\ndraw_bg.color: {WHITE} draw_bg.radius: 10 draw_bg.border_width: {width} draw_bg.border_position: 1 draw_bg.border_color: {edge}\n",
            if trailing_w > 0.0 { 4.0 } else { 14.0 }
        ));
        let ph = if self.placeholder_ink { INK } else { PLACEHOLDER };
        // An empty field STARTS in the input's `empty` state. Left to its
        // creation-time 0.2 s transition, the placeholder drew in the typed
        // ink until some event advanced the animator (measured on the phone
        // shell's first frame: #1d1d1f, grey only after a hover).
        let empty_default = if self.text.is_empty() {
            "animator +: {empty +: {default: @on}}\n"
        } else {
            ""
        };
        out.push_str(&format!(
            "{} := TextInputFlat {{\n{empty_default}width: Fill height: Fill padding: Inset{{top: 12 bottom: 12}} margin: 0 flow: Right\ntext: {}\nempty_text: {}\nis_password: {} is_read_only: {}\ndraw_bg +: {{color: {CLEAR} color_hover: {CLEAR} color_focus: {CLEAR} color_down: {CLEAR} color_empty: {CLEAR} color_disabled: {CLEAR} border_size: 0.0 border_color: {CLEAR} border_color_hover: {CLEAR} border_color_focus: {CLEAR} border_color_down: {CLEAR} border_color_empty: {CLEAR} border_color_disabled: {CLEAR}}}\ndraw_text +: {{color: {INK} color_hover: {INK} color_focus: {INK} color_down: {INK} color_disabled: {MUTED} color_empty: {ph} color_empty_hover: {ph} color_empty_focus: {PLACEHOLDER} text_style: {}}}\ndraw_cursor.color: {INK}\n}}\n",
            self.id,
            lit(self.text),
            lit(self.placeholder),
            self.password,
            self.read_only,
            font(400, 15.0)
        ));
        if let Some(t) = &self.trailing {
            out.push_str(t);
        }
        out.push_str("}\n");
        out
    }
}

/// A callout box (radius 12): the board's light-red refusal (p4-03) or the
/// neutral grey note with an info icon (p4-04, p4-09).
pub fn callout(red: bool, info_icon: bool, head: &str, next: Option<&str>) -> String {
    let (bg, edge, head_ink) = if red {
        (RED_BG, RED_EDGE, RED)
    } else {
        (SUBTLE, HAIR, INK)
    };
    let mut body = String::new();
    body.push_str(&Text::new("", head).px(15.0).color(head_ink).fill().dsl());
    if let Some(n) = next {
        body.push_str(&gap(6.0));
        body.push_str(&Text::new("", n).px(14.0).color(MUTED).fill().dsl());
    }
    let icon_dsl = if info_icon {
        format!(
            "View {{ width: 24 height: Fit padding: Inset{{top: 0}} {}}}\n",
            svg("", "b1_info.svg", 22.0)
        )
    } else {
        String::new()
    };
    format!(
        "DesignSurface {{\nwidth: Fill height: Fit flow: Right spacing: 12 padding: Inset{{left: 16 right: 16 top: 14 bottom: 14}}\ndraw_bg.color: {bg} draw_bg.radius: 12 draw_bg.border_width: 1 draw_bg.border_position: 1 draw_bg.border_color: {edge}\n{icon_dsl}View {{ width: Fill height: Fit flow: Down\n{body}}}\n}}\n"
    )
}

/// A rounded list container (radius 12, hairline edge) whose `rows` are
/// separated by hairlines. Each row is already a full DSL fragment.
pub fn list_card(id: &str, rows: &[String]) -> String {
    let mut inner = String::new();
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            inner.push_str(&hairline());
        }
        inner.push_str(r);
    }
    let name = if id.is_empty() {
        String::new()
    } else {
        format!("{id} := ")
    };
    format!(
        "{name}DesignSurface {{\nwidth: Fill height: Fit flow: Down padding: 1\ndraw_bg.color: {WHITE} draw_bg.radius: 12 draw_bg.border_width: 1 draw_bg.border_position: 1 draw_bg.border_color: {HAIR}\n{inner}}}\n"
    )
}

/// [`list_card`] whose rows scroll inside a fixed height when `max_h` is set.
pub fn list_card_scroll(id: &str, rows: &[String], max_h: Option<f64>) -> String {
    let Some(h) = max_h else {
        return list_card(id, rows);
    };
    let mut inner = String::new();
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            inner.push_str(&hairline());
        }
        inner.push_str(r);
    }
    format!(
        "{id} := DesignSurface {{\nwidth: Fill height: Fit flow: Down padding: 1\ndraw_bg.color: {WHITE} draw_bg.radius: 12 draw_bg.border_width: 1 draw_bg.border_position: 1 draw_bg.border_color: {HAIR}\nScrollYView {{ width: Fill height: {h} flow: Down\n{inner}}}\n}}\n"
    )
}

/// A key/value settings row (p4-05: "Server  192.168.1.20:50190").
pub fn kv_row(key: &str, value: &str) -> String {
    format!(
        "View {{ width: Fill height: 52 flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 16 right: 16}} spacing: 12\n{}{}}}\n",
        Text::new("", key).px(15.0).one_line().dsl(),
        Text::new("", value).px(15.0).color(MUTED).fill().right().one_line().dsl()
    )
}

/// A single-text row (p4-05's "Stays on this device only").
pub fn note_row(text: &str, color: &str) -> String {
    format!(
        "View {{ width: Fill height: 52 flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 16 right: 16}}\n{}}}\n",
        Text::new("", text).px(15.0).color(color).fill().one_line().dsl()
    )
}

/// The "or" divider of p4-01: hairline, "or", hairline.
pub fn or_divider() -> String {
    format!(
        "View {{ width: Fill height: 18 flow: Right align: Align{{x: 0.5 y: 0.5}} spacing: 12\nSolidView {{ width: Fill height: 1 draw_bg.color: {HAIR} }}\n{}SolidView {{ width: Fill height: 1 draw_bg.color: {HAIR} }}\n}}\n",
        Text::new("", "or").px(14.0).color(MUTED).dsl()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_are_escaped() {
        assert_eq!(lit("a\"b"), "\"a\\\"b\"");
        assert_eq!(lit("Pairing with 127.0.0.1…"), "\"Pairing with 127.0.0.1…\"");
    }

    #[test]
    fn a_pill_carries_its_label_and_never_a_gradient_stop() {
        let p = pill_primary("b1_x", "Pair", "Fill");
        assert!(p.contains("text: \"Pair\""));
        assert!(p.contains("color: #000000ff"));
        // `color_2` left at its default (-1) keeps the face flat; setting it
        // turns the fill into a gradient (button.rs:182).
        assert!(!p.contains("color_2"));
    }
}
