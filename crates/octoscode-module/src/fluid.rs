//! A1 — the conversation rows as FLUID layouts (desktop + phone).
//!
//! The Stage-B components (`design/components/*`) were cut from 406 px phone
//! artboards, and their lowered DSL kept every measured width: the answer
//! prose wrapped at 356 px, the composer card was 374 px and each tool call a
//! 371x105 card with its status on its own line — inside a 681 px desktop
//! column. These builders keep the kit's TOKENS (the Inter faces, the
//! LiberationMono code face, the palette, the component icons) and drop the
//! artboard GEOMETRY: every box is `Fill`, bounded by the live column width
//! from [`crate::conv_layout`], the way the web lays out the same rows
//! (`src-web/apps/web/src/app/styles.css:678-760`, `Timeline.module.css`,
//! `markdown.css`).
//!
//! Dynamic text (the answer, the bubble, tool titles) stays a native flow
//! region (`Markdown` / wrapping `Label`), per RULES 8.10.
//!
//! The light palette is the shell's role set (`screens::theme::role_assign-
//! ments`); [`crate::screens::theme::retint_dsl`] maps it for dark at the
//! tail of `components::lower`, so every literal here is one of its keys.
use std::path::PathBuf;

use crate::conv_layout::{Density, Metrics};
use crate::i18n::{tr, tr1};

// ---- palette (light; dark comes from retint_dsl) ---------------------------

/// Primary text (`color_fg_app`).
pub const INK: &str = "#1d1d1fff";
/// Secondary text (`color_text_muted`): A18 — the web's
/// `--dsw-alias-label-secondary` (light), >= 4.5:1 on every fill here
/// (`screens::theme::CONTRAST_PAIRS`); dark is #98989D via `retint_dsl`.
pub const MUTED: &str = "#61666bff";
/// Hairlines and card borders (`color_outset_1`).
pub const BORDER: &str = "#e5e5e7ff";
/// The page / card surface (`color_bg_app`).
pub const SURFACE: &str = "#ffffffff";
/// A raised fill: inline code chips, the tool output well.
pub const RAISED: &str = "#f4f4f5ff";
/// The tool group's quiet fill (`color_bg_odd`, the sidebar grey).
pub const TIP: &str = "#f7f7f8ff";
/// The success check (board: diff green #1F883D).
pub const GREEN: &str = "#1f883dff";
/// The failure mark (board: diff red #CF222E).
pub const RED: &str = "#cf222eff";

/// A text face of the kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Regular,
    Medium,
    SemiBold,
    Mono,
}

impl Face {
    fn file(self) -> &'static str {
        match self {
            Face::Regular => "ux/Inter-400.ttf",
            Face::Medium => "ux/Inter-500.ttf",
            Face::SemiBold => "ux/Inter-600.ttf",
            Face::Mono => "ux/LiberationMono-Regular.ttf",
        }
    }
    fn weight(self) -> u32 {
        match self {
            Face::Regular | Face::Mono => 400,
            Face::Medium => 500,
            Face::SemiBold => 600,
        }
    }
}

/// The Inter/mono family with the sans CJK members ([`crate::design::cjk_members`])
/// and the renderer's symbol + emoji members — the same member order the kit
/// emitter writes, so a mixed line keeps Inter's metrics.
pub fn family(face: Face) -> String {
    let latin = crate::design::font_file(face.file());
    let emoji = if cfg!(target_os = "macos") {
        "file_resource(\"/System/Library/Fonts/Apple Color Emoji.ttc\")".to_owned()
    } else {
        "crate_resource(\"makepad_widgets:resources/NotoColorEmoji.ttf\")".to_owned()
    };
    format!(
        "FontFamily{{latin := FontMember{{res: file_resource({:?}) asc: 0.04 desc: 0.04 weight: {w}}} \
         {cjk} \
         symbols := FontMember{{res: crate_resource(\"makepad_widgets:resources/jetbrains_mono_variable.ttf\") asc: 0 desc: 0 weight: 400}} \
         emoji := FontMember{{res: {emoji} asc: 0 desc: 0}}}}",
        latin.display().to_string(),
        w = face.weight(),
        cjk = crate::design::cjk_members(face.weight()),
    )
}

/// Inter's line box (hhea ascender − descender, the 0.04 fudges cancel):
/// a `Label` line advances `LINE_BOX × px × line_spacing`.
const LINE_BOX: f64 = 2478.0 / 2048.0;
/// A `TextFlow` (Markdown) line advances `1.1303 × px × line_spacing`
/// (measured by the renderer, octoscript-makepad `design.rs:563-584`).
const FLOW_BOX: f64 = 1.1303;

/// A `TextStyle` literal: `px` is the CSS pixel size (font_size is points,
/// px × 0.75), `line_px` the CSS line height.
pub fn style(face: Face, px: f64, line_px: f64) -> String {
    format!(
        "TextStyle{{font_family: {} font_size: {} line_spacing: {:.4}}}",
        family(face),
        px * 0.75,
        line_px / (LINE_BOX * px)
    )
}

fn flow_style(face: Face, px: f64, line_px: f64) -> String {
    format!(
        "TextStyle{{font_family: {} font_size: {} line_spacing: {:.4}}}",
        family(face),
        px * 0.75,
        line_px / (FLOW_BOX * px)
    )
}

/// The type scale per density: the web's numbers on the desktop
/// (`markdown.css:8-14` 15/25, `Timeline.module.css:212-226` 13/20 tool
/// headers, 11-12 px status), one step up on the phone (the approved phone
/// boards set a larger body; the web's ≤760 composer is 16 px).
#[derive(Debug, Clone, Copy)]
pub struct Scale {
    pub body: f64,
    pub body_line: f64,
    pub small: f64,
    pub small_line: f64,
    pub tiny: f64,
    pub row_h: f64,
    pub turn_gap: f64,
}

pub fn scale(d: Density) -> Scale {
    match d {
        // The Mac's 13-point body, 11 to 12 secondary, 28-point rows.
        Density::Desktop if crate::desk::active() => Scale {
            body: 13.0,
            body_line: 20.0,
            small: 12.0,
            small_line: 18.0,
            tiny: 11.0,
            row_h: 28.0,
            turn_gap: 16.0,
        },
        Density::Desktop => Scale {
            body: 15.0,
            body_line: 24.0,
            small: 13.0,
            small_line: 20.0,
            tiny: 12.0,
            row_h: 36.0,
            turn_gap: 24.0,
        },
        Density::Phone => Scale {
            body: 16.0,
            body_line: 25.0,
            small: 14.0,
            small_line: 20.0,
            tiny: 12.5,
            row_h: 40.0,
            turn_gap: 20.0,
        },
    }
}

/// An icon file: the module's own (`resources/icons/<name>`) or a component
/// asset (`components/<id>/assets/<file>`), as an absolute materialized path.
pub fn icon(rel: &str) -> PathBuf {
    if rel.contains('/') {
        crate::design::font_file(rel)
    } else {
        crate::design::font_file(&format!("icons/{rel}"))
    }
}

/// A Mac's activity indicator: the framework's, the same in every app.
pub(crate) fn spinner(id: &str, size: f64) -> String {
    format!("{id} := LoadingSpinner{{width: {size} height: {size}}}\n")
}

pub(crate) fn svg(id: &str, rel: &str, size: f64, color: &str) -> String {
    if rel == "components/working-row/assets/icon_spinner.svg" && crate::desk::active() {
        return spinner(id, size);
    }
    format!(
        "{id} := Svg{{width: {size} height: {size} animating: false \
         draw_svg.svg: file_resource({:?}) draw_svg.preserve_viewbox: true \
         draw_svg.color: {color}}}\n",
        icon(rel).display().to_string()
    )
}

/// A `Fit`-width label WRAPS at its `max_width` only when `max_lines > 0`:
/// `DrawText::draw_walk` resolves a Fit box's max bound as the wrap width
/// only for a line-limited or ellipsized run (makepad draw_text.rs:2242-
/// 2255); without it the run lays out on one line and the box clips it
/// (measured: a long prompt cut at the bubble's right edge). A bound far
/// above any real message keeps it unlimited in practice.
pub const FIT_WRAP_LINES: u32 = 10_000;

pub(crate) fn label(id: &str, text: &str, style: &str, color: &str, walk: &str) -> String {
    let head = if id.is_empty() { String::new() } else { format!("{id} := ") };
    format!(
        "{head}Label{{{walk} padding: 0 {tails}text: {text:?}\n\
         draw_text.text_style: {style}\n\
         draw_text.color: {color}}}\n",
        tails = if crate::desk::active() { "clip_y: false " } else { "" },
    )
}

/// A transparent hit target over its parent overlay (the shell's own
/// `review_close` pattern, `lib.rs`): no fill, a faint press tint.
pub fn hit(id: &str, radius: f64) -> String {
    // `color_2` at (-1,-1,-1,-1) DISABLES the face gradient: at #00000000 the
    // gradient stays on (button.rs tests `color_2.x > -0.5`) and mixes toward
    // the theme's `color_2_focus` once clicked — measured as a white wash
    // over a disclosed tool row. Focus stays transparent for the same reason.
    // A6: NO hover tint (the chrome's `OcHit` rule): a synthesized click — a
    // phone tap, the instrument — never sends hover-out, so the tint stuck on
    // the row just disclosed (the "shared row hover tint" left on an opened
    // block). The press tint is the feedback.
    // A desktop has a pointer: a real hover (a mouse leaves again), and a
    // Mac's corner on every control the hit lies over.
    let (hover, down, radius) = if crate::desk::active() {
        let t = crate::desk::current();
        (t.wash, t.wash_down, radius.min(3.0))
    } else {
        ("#00000000".to_owned(), "#00000014".to_owned(), radius)
    };
    format!(
        "{id} := Button{{width: Fill height: Fill text: \"\" margin: 0 padding: 0 \
         label_walk: Walk{{width: 0 height: 0}} icon_walk: Walk{{width: 0 height: 0}} \
         draw_bg.color: #00000000 draw_bg.color_hover: {hover} draw_bg.color_down: {down} \
         draw_bg.color_focus: #00000000 draw_bg.color_disabled: #00000000 \
         draw_bg.color_2: vec4(-1.0, -1.0, -1.0, -1.0) draw_bg.border_size: 0.0 draw_bg.border_radius: {radius} \
         draw_bg.border_color: #00000000 draw_bg.border_color_2: vec4(-1.0, -1.0, -1.0, -1.0)}}\n"
    )
}

/// A hit over the blue default button: under the pointer it darkens in either
/// appearance, so the white word keeps its contrast.
fn hit_on_blue(id: &str, radius: f64) -> String {
    let t = crate::desk::current();
    hit(id, radius)
        .replace(&format!("color_hover: {}", t.wash), "color_hover: #00000012")
        .replace(&format!("color_down: {}", t.wash_down), "color_down: #00000020")
}

// ---- the rows ---------------------------------------------------------------

/// The web's gap between timeline entries (`Timeline.module.css:1-3`).
pub const TIMELINE_GAP: f64 = 20.0;
/// The space the person's bubble keeps under itself.
pub const BUBBLE_BOTTOM: f64 = 6.0;
/// The "Worked for" row's own top padding.
pub const WORKED_TOP: f64 = 4.0;
/// The live "Working" row's own top padding.
pub const WORKING_TOP: f64 = 4.0;

/// The person's message: right-aligned, hugging its text up to
/// `min(680px, 82%)` of the column, wrapping inside (`styles.css:700-707`).
/// The board's black bubble with white text (light); dark keeps the shell's
/// raised surface (#36f).
pub fn user_bubble(tok: &str, text: &str, m: &Metrics, dark: bool) -> String {
    let s = scale(m.density);
    let (fill, ink) = if crate::desk::active() {
        // A Mac's message: a raised neutral, never a black pill.
        let t = crate::desk::current();
        (format!("{}ff", t.raised), format!("{}ff", t.ink))
    } else if dark {
        ("#2c2c2eff".to_owned(), "#f5f5f7ff".to_owned())
    } else {
        ("#000000ff".to_owned(), "#ffffffff".to_owned())
    };
    // The bubble's text box: the cap minus the bubble's own side padding.
    let (pad_x, pad_y, corner) = crate::desk::by((16.0, 10.0, 9.0), (12.0, 7.0, 4.0));
    let text_max = (m.bubble_max_w - 2.0 * pad_x).max(40.0);
    format!(
        "user_align := View{{width:Fill height:Fit flow:Down align: Align{{x: 1.0}} \
         padding: Inset{{top: {top} bottom: {BUBBLE_BOTTOM}}}\n\
         i{tok}_userbubble := RoundedView{{width: Fit height: Fit max_width: {max} flow: Down \
         padding: Inset{{left: {pad_x} right: {pad_x} top: {pad_y} bottom: {pad_y}}}\n\
         draw_bg +: {{color: {fill} border_radius: {corner}}}\n\
         i{tok}_userbubble_0 := Label{{width: Fit height: Fit max_width: {text_max} padding: 0 {tails}\
         flow: Right{{wrap: true}} max_lines: {FIT_WRAP_LINES} text: {text:?}\n\
         draw_text.text_style: {style}\n\
         draw_text.color: {ink}}}\n\
         }}\n}}\n",
        tails = if crate::desk::active() { "clip_y: false " } else { "" },
        top = s.turn_gap,
        max = m.bubble_max_w,
        style = style(Face::Regular, s.body, s.body_line),
    )
}

/// The answer: one native `Markdown` flow region at the web's prose rhythm
/// (15/25, 14 px between blocks, `markdown.css:8-30`), the column's width up
/// to the 75ch measure. Fenced code wraps (the board's accepted alternative
/// to horizontal scroll, #21g item 2). Tables follow `markdown.css:137-155`:
/// 9/14 px cells, hairline rules, no header fill (makepad's default painted
/// the header row in the highlight blue).
pub fn assistant_prose(tok: &str, body: &str, m: &Metrics) -> String {
    assistant_answer(tok, &crate::markdown::display(body, false), None, m)
}

/// A7 — the answer from its DISPLAY form (`crate::markdown::display`): each
/// prose segment is one native `Markdown` region at A1's prose rhythm, each
/// top-level fenced block the web's code block (`CodeBlock.tsx:94-107`: a
/// banner with the language and a Copy control over the code;
/// `markdown.css:182-210`). `copied` = the block whose Copy was pressed in
/// the last second ("Copied", `CodeBlock.tsx:72-80`).
pub fn assistant_answer(tok: &str, d: &crate::markdown::Display, copied: Option<usize>, m: &Metrics) -> String {
    use crate::markdown::Segment;
    let mut inner = String::new();
    let last = d.segments.len().saturating_sub(1);
    let mut prose_n = 0usize;
    for (k, seg) in d.segments.iter().enumerate() {
        match seg {
            Segment::Prose(text) => {
                let id = if prose_n == 0 {
                    format!("i{tok}_assistantprose")
                } else {
                    format!("i{tok}_assistantprose_{prose_n}")
                };
                prose_n += 1;
                inner.push_str(&markdown_region(&id, text, d.math, m));
            }
            Segment::Code { lang, code } => {
                // The web's `.md-code-block` margin (14 px), collapsed at the
                // answer's own edges (`markdown.css:16-25`).
                let top = if k == 0 { 0.0 } else { 14.0 };
                let bottom = if k == last { 0.0 } else { 14.0 };
                inner.push_str(&code_block(tok, k, lang.as_deref(), code, copied == Some(k), !d.streaming, top, bottom, m));
            }
        }
    }
    if inner.is_empty() {
        inner.push_str(&markdown_region(&format!("i{tok}_assistantprose"), "", false, m));
    }
    format!("View{{width: Fill height: Fit flow: Down padding: Inset{{top: 4 bottom: 8}}\n{inner}}}\n")
}

/// One prose `Markdown` region (A1's look). With `math`, the renderer's math
/// extension typesets `$…$` / `$$…$$` through makepad's MathView (the web's
/// KaTeX): `inline_math` sits in the line at the body's height, `display_math`
/// is its own centred block (`code_view::A7MathBlock`, KaTeX's
/// `.katex-display`). Without it every `$` arrives escaped (`markdown::sanitize`).
pub(crate) fn markdown_region(id: &str, body: &str, math: bool, m: &Metrics) -> String {
    let s = scale(m.density);
    let line = s.body_line + 1.0;
    // MathView lays out at `font_size * 1.75` (makepad math_view.rs) and its
    // font_size is in points like the body's: the body is `s.body * 0.75` pt,
    // so 0.57 of it keeps inline math at the text's height.
    let math_dsl = if math {
        format!(
            "use_math_widget: true\n\
             inline_math := MathView{{font_size: {inline:.2} baseline_offset: 3.0 color: {INK}}}\n\
             display_math := mod.widgets.A7MathBlock{{math := MathView{{font_size: {display:.2} baseline_offset: 0.0 color: {INK}}}}}\n",
            inline = s.body * 0.75 * 0.78,
            display = s.body * 0.75 * 0.92,
        )
    } else {
        String::new()
    };
    // A7: the renderer's `MarkdownLink` draws an EMPTY link widget at the
    // link's start (its text is never set — the link's words follow as plain
    // text), which showed the widget's default "Button" and a tall margin.
    // The template gives it the web's outbound-link affordance instead: a
    // blue "↗" in the line, the control that opens the (already vetted)
    // absolute http/https/mailto destination (`lib.rs` LinkNavigated).
    let link_dsl = format!(
        "link := MarkdownLink{{text: \"↗\" margin: 0 padding: Inset{{left: 1 right: 3}} \
         draw_text +: {{color: {ink} color_hover: {down} color_pressed: {down} text_style: {link_style}}}}}\n",
        ink = crate::desk::by("#2f6febff", "#3564c6ff"),
        down = crate::desk::by("#1f4fb8ff", "#3564c6ff"),
        link_style = flow_style(Face::Regular, s.body, line),
    );
    format!(
        "{id} := Markdown{{width: Fill max_width: {max} height: Fit padding: 0 margin: 0\n\
         body: {body:?}\n\
         font_size: {fs}\n\
         font_color: {INK}\n\
         paragraph_spacing: {para}\n\
         pre_code_spacing: 10\n\
         heading_base_scale: {heading}\n\
         fixed_font_size_scale: 0.87\n\
         inline_code_padding: Inset{{left: 5 right: 5 top: 1 bottom: 1}}\n\
         inline_code_margin: Inset{{left: 2 right: 2 top: 0 bottom: 0}}\n\
         text_style_normal: {regular}\n\
         text_style_italic: {regular}\n\
         text_style_bold: {bold}\n\
         text_style_bold_italic: {bold}\n\
         text_style_fixed: {mono}\n\
         code_layout: Layout{{flow: Right{{wrap: true}} padding: Inset{{left: 14 right: 14 top: 10 bottom: 10}}}}\n\
         table_cell_layout: Layout{{flow: Right{{wrap: true}} padding: Inset{{left: {cell_x} right: {cell_x} top: {cell_y} bottom: {cell_y}}}}}\n\
         draw_block +: {{code_color: {RAISED} line_color: {INK} sep_color: {BORDER} \
         quote_bg_color: {BORDER} quote_fg_color: {MUTED} \
         table_header_bg_color: #00000000 table_border_color: {BORDER}}}\n\
         {math_dsl}\
         {link_dsl}\
         }}\n",
        max = m.prose_max_w,
        fs = s.body * 0.75,
        para = crate::desk::by(14.0, 10.0),
        heading = crate::desk::by(1.6, 1.4),
        cell_x = crate::desk::by(14.0, 10.0),
        cell_y = crate::desk::by(9.0, 5.0),
        regular = flow_style(Face::Regular, s.body, line),
        bold = flow_style(Face::SemiBold, s.body, line),
        mono = flow_style(Face::Mono, s.body, line),
    )
}

/// The code block's code size (`markdown.css:200-207`: 400 12px/20px).
pub const CODE_PX: f64 = 12.0;
pub const CODE_LINE: f64 = 20.0;

/// The code's line height by the running look (a Mac's tighter 12/17).
fn code_line() -> f64 {
    crate::desk::by(CODE_LINE, 17.0)
}

/// One fenced code block (`CodeBlock.tsx:94-107`, `markdown.css:182-207`): a
/// rounded tip-grey card; its banner names the language (`text` without one)
/// and carries the Copy control (`code_copy_<k>`, a >= 28 px hit the host
/// routes to the clipboard); the code below keeps its whitespace.
///
/// A13 (judge, 360 px: "Args::\nparse()") — a settled block lays every line
/// out WHOLE in a `ScrollXView` (`i<tok>_code_<k>_scroll`): a long line
/// scrolls sideways (touch drag, a trackpad's horizontal swipe, the bar)
/// like the web's `pre { white-space: pre; overflow-x: auto }`. A block
/// with no known grammar renders the same way, uncoloured. While a reply
/// streams (and past the size bound) the renderer's wrapping code flow stays
/// (A1's accepted alternative; the stream re-lowers every delta).
#[allow(clippy::too_many_arguments)]
pub fn code_block(
    tok: &str,
    k: usize,
    lang: Option<&str>,
    code: &str,
    copied: bool,
    highlight: bool,
    top: f64,
    bottom: f64,
    m: &Metrics,
) -> String {
    let banner_style = style(Face::Mono, 11.0, 18.0);
    let copy_style = style(Face::Medium, 11.0, 18.0);
    let label_text = lang.filter(|l| !l.is_empty()).unwrap_or("text");
    let shown = crate::markdown::copy_text(code);
    format!(
        "i{tok}_code_{k} := RoundedView{{width: Fill max_width: {max} height: Fit flow: Down \
         margin: Inset{{top: {top} bottom: {bottom}}}\n\
         draw_bg +: {{color: {RAISED} border_radius: {corner}{edge}}}\n\
         i{tok}_code_{k}_banner := View{{width: Fill height: {banner_h} flow: Right align: Align{{y: 0.5}} \
         padding: Inset{{left: {banner_l} right: 4}}\n\
         {lang_label}\
         View{{width: Fill height: 1}}\n\
         View{{width: {copy_w} height: {copy_h} flow: Overlay align: Align{{x: 1.0 y: 0.5}}\n\
         View{{width: Fill height: Fill flow: Right align: Align{{x: 1.0 y: 0.5}} padding: Inset{{right: 9}}\n\
         {copy_label}}}\n\
         {copy_hit}}}\n\
         }}\n\
         {body}\
         }}\n",
        max = m.prose_max_w,
        corner = crate::desk::by(12.0, 3.0),
        edge = crate::desk::by(String::new(), format!(" border_size: 1.0 border_color: {BORDER}")),
        banner_h = crate::desk::by(34.0, 28.0),
        banner_l = crate::desk::by(13.0, 10.0),
        copy_w = crate::desk::by(72.0, 56.0),
        copy_h = crate::desk::by(30.0, 28.0),
        lang_label = label(&format!("i{tok}_code_{k}_lang"), label_text, &banner_style, MUTED, "width: Fit height: Fit"),
        copy_label = label(
            &format!("i{tok}_code_{k}_copy_label"),
            tr(if copied { "Copied" } else { "Copy" }),
            &copy_style,
            if copied { INK } else { MUTED },
            "width: Fit height: Fit",
        ),
        copy_hit = hit(&format!("code_copy_{k}"), 6.0),
        body = if highlight && highlightable(&shown) {
            // A13: whole lines in a sideways-scrolling box; uncoloured
            // (`lang: ""`) when no grammar knows the fence's language.
            let lang_id = crate::highlight::grammar(lang).map(|g| g.id).unwrap_or("");
            // The bar: a quiet grey handle in the block's bottom padding,
            // inset past the card's 12 px corners (the theme's default
            // handle is near-white: a bright stripe on the tip-grey card).
            let (bar, bar_on) = if crate::screens::theme::resolved() == "dark" {
                ("#5a5a5eff", "#6e6e73ff")
            } else {
                ("#d1d1d6ff", "#aeaeb2ff")
            };
            format!(
                "i{tok}_code_{k}_scroll := ScrollXView{{width: Fill height: Fit flow: Down\n\
                 scroll_bars.scroll_bar_x.bar_side_margin: 12\n\
                 {bars}\
                 View{{width: Fit height: Fit flow: Down padding: Inset{{left: {gutter} right: {gutter} top: 0 bottom: {foot}}}\n\
                 {lines}}}\n\
                 }}\n",
                gutter = crate::desk::by(16.0, 10.0),
                foot = crate::desk::by(14.0, 10.0),
                lines = highlighted_body(&format!("i{tok}_code_{k}_body"), lang_id, &shown),
                // A Mac's overlay scroller is the system's: a colour set here
                // would show a handle at rest.
                bars = if crate::desk::active() {
                    String::new()
                } else {
                    format!(
                        "scroll_bars.scroll_bar_x.draw_bg.color: {bar}\n\
                         scroll_bars.scroll_bar_x.draw_bg.color_hover: {bar_on}\n\
                         scroll_bars.scroll_bar_x.draw_bg.color_drag: {bar_on}\n"
                    )
                },
            )
        } else {
            format!(
                "View{{width: Fill height: Fit flow: Down padding: Inset{{left: {gutter} right: {gutter} top: 0 bottom: {foot}}}\n\
                 {body}}}\n",
                gutter = crate::desk::by(16.0, 10.0),
                foot = crate::desk::by(14.0, 10.0),
                body = code_body(&format!("i{tok}_code_{k}_body"), &shown),
            )
        },
    )
}

/// The size past which a block stays plain (the lexer runs at lowering time;
/// the web lazy-loads per visible block, natively the row lowers only when
/// the virtualized list shows it — this bounds the one-off cost).
fn highlightable(code: &str) -> bool {
    code.len() <= 24_000 && code.lines().count() <= 600
}

/// The highlighted code: the module's own `A7CodeLines` widget
/// (`code_view.rs`) — the code lexed in Rust (`crate::highlight`), each line
/// one row of runs in the web's token colours (`theme.css:101-112`,
/// `--shiki-token-*`), indentation kept, on the code face. A13: the rows
/// never wrap (`wrap: false`) and the widget is as wide as its longest line,
/// for the `ScrollXView` around it. `lang` is a grammar id, or `""` (plain).
fn highlighted_body(id: &str, lang: &str, code: &str) -> String {
    let dark = crate::screens::theme::resolved() == "dark";
    // A26: a named display palette colours the tokens its own way.
    let palette = crate::screens::theme::palette().id();
    format!(
        "{id} := mod.widgets.A7CodeLines{{width: Fit height: Fit\n\
         wrap: false\n\
         text: {code:?}\n\
         lang: {lang:?}\n\
         dark: {dark}\n\
         palette: {palette:?}\n\
         line_height: {line}\n\
         draw_text +: {{text_style: {mono}}}\n\
         }}\n",
        line = code_line(),
        mono = style(Face::Mono, CODE_PX, code_line()),
    )
}


/// The code itself: ONE `Markdown` region holding just this block, re-fenced
/// with a backtick run longer than any inside the code, so its whitespace and
/// line breaks render exactly (the renderer's own code path, as A1's answer
/// code), on the card's colour with the web's 12/20 mono.
fn code_body(id: &str, code: &str) -> String {
    let longest = code
        .split(|c| c != '`')
        .map(|r| r.len())
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    let body = format!("{fence}\n{code}\n{fence}");
    format!(
        "{id} := Markdown{{width: Fill height: Fit padding: 0 margin: 0\n\
         body: {body:?}\n\
         font_size: {fs}\n\
         font_color: {INK}\n\
         paragraph_spacing: 0\n\
         pre_code_spacing: 0\n\
         fixed_font_size_scale: 1.0\n\
         text_style_normal: {mono}\n\
         text_style_fixed: {mono}\n\
         code_layout: Layout{{flow: Right{{wrap: true}} padding: 0}}\n\
         draw_block +: {{code_color: #00000000 line_color: {INK} sep_color: {BORDER} \
         quote_bg_color: {BORDER} quote_fg_color: {MUTED} \
         table_header_bg_color: #00000000 table_border_color: {BORDER}}}\n\
         }}\n",
        fs = CODE_PX * 0.75,
        mono = flow_style(Face::Mono, CODE_PX, code_line()),
    )
}

/// The tool family glyph (`src-web/.../timeline/tool-kind.ts:9-34`): matched
/// on whole words of the server-authored name, `generic` when nothing fits.
pub fn tool_kind(name: &str) -> &'static str {
    // Split camelCase and separators so `readFile` and `web_fetch` both match.
    let mut words = String::with_capacity(name.len() + 8);
    let mut prev_lower = false;
    for ch in name.chars() {
        if ch.is_ascii_uppercase() && prev_lower {
            words.push(' ');
        }
        prev_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        if matches!(ch, '.' | '_' | '-' | '/' | ':') {
            words.push(' ');
        } else {
            words.push(ch.to_ascii_lowercase());
        }
    }
    const PATTERNS: &[(&str, &[&str])] = &[
        ("shell", &["shell", "bash", "sh", "zsh", "exec", "run", "command", "terminal"]),
        ("edit", &["edit", "write", "patch", "apply", "create", "delete", "move", "rename", "format"]),
        ("read", &["read", "cat", "open", "view", "show", "list", "ls", "stat", "tree"]),
        ("search", &["search", "grep", "find", "glob", "ripgrep", "rg", "lookup", "query"]),
        ("web", &["web", "http", "fetch", "curl", "browse", "browser", "url", "download"]),
    ];
    for (kind, keys) in PATTERNS {
        if words.split_whitespace().any(|w| keys.contains(&w)) {
            return kind;
        }
    }
    "generic"
}

/// Best-effort one-line target from a tool call's arguments JSON
/// (`folds.ts:121-138` `toolTarget`): the first of the web's keys that holds
/// a non-empty string, first line, truncated to 24 chars + `…`.
pub fn tool_target(arguments: &str) -> String {
    let Ok(serde_json::Value::Object(row)) = serde_json::from_str::<serde_json::Value>(arguments) else {
        return String::new();
    };
    target_of(&row)
}

/// The target of a tool call's `arguments_preview` as the server sends it:
/// JSON ([`tool_target`]) or the live server's `key: "value"` text form
/// (measured on the a6ea8505 serve: `path: "README.md"`) — the first of the
/// web's keys wins, its value unquoted, truncated like the web's.
pub fn preview_target(preview: &str) -> String {
    let t = tool_target(preview);
    if !t.is_empty() || preview.trim_start().starts_with('{') {
        return t;
    }
    const KEYS: &[&str] = &[
        "cmd", "command", "path", "file_path", "filepath", "file", "url", "query", "pattern", "name",
    ];
    for key in KEYS {
        let needle = format!("{key}: ");
        let mut search = preview;
        while let Some(at) = search.find(&needle) {
            // Whole key only: `file_path: …` must not answer for `path`.
            let before = search[..at].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                search = &search[at + needle.len()..];
                continue;
            }
            let rest = &search[at + needle.len()..];
            let value = if let Some(q) = rest.strip_prefix('"') {
                let mut out = String::new();
                let mut esc = false;
                for ch in q.chars() {
                    if esc {
                        out.push(ch);
                        esc = false;
                    } else if ch == '\\' {
                        esc = true;
                    } else if ch == '"' {
                        break;
                    } else {
                        out.push(ch);
                    }
                }
                out
            } else {
                rest.split(|c| c == ',' || c == '\n').next().unwrap_or("").trim().to_owned()
            };
            let first = value.trim().lines().next().unwrap_or("").trim().to_owned();
            if first.is_empty() {
                break;
            }
            return if first.chars().count() > 24 {
                format!("{}…", first.chars().take(24).collect::<String>())
            } else {
                first
            };
        }
    }
    String::new()
}

/// [`tool_target`] over an already-parsed argument object.
pub fn target_of(row: &serde_json::Map<String, serde_json::Value>) -> String {
    const KEYS: &[&str] = &[
        "cmd", "command", "path", "file_path", "filepath", "file", "url", "query", "pattern", "name",
    ];
    for key in KEYS {
        if let Some(serde_json::Value::String(v)) = row.get(*key) {
            let first = v.trim().lines().next().unwrap_or("").trim();
            if first.is_empty() {
                continue;
            }
            let n = first.chars().count();
            return if n > 24 {
                format!("{}…", first.chars().take(24).collect::<String>())
            } else {
                first.to_owned()
            };
        }
    }
    String::new()
}

/// Where a tool row sits in its turn's group (one card per turn).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupPos {
    Single,
    First,
    Middle,
    Last,
}

impl GroupPos {
    pub fn of(ordinal: usize, count: usize) -> GroupPos {
        match (ordinal, count) {
            (_, 0 | 1) => GroupPos::Single,
            (0, _) => GroupPos::First,
            (i, n) if i + 1 == n => GroupPos::Last,
            _ => GroupPos::Middle,
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            GroupPos::Single => "single",
            GroupPos::First => "first",
            GroupPos::Middle => "middle",
            GroupPos::Last => "last",
        }
    }
    pub fn from_id(id: &str) -> GroupPos {
        match id {
            "first" => GroupPos::First,
            "middle" => GroupPos::Middle,
            "last" => GroupPos::Last,
            _ => GroupPos::Single,
        }
    }
}

/// One tool call as the row draws it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolView {
    /// The server-authored tool name (`read_file`).
    pub title: String,
    /// `title · target`'s target (`README.md`), may be empty.
    pub target: String,
    /// `running` | `done` | `failed` | `skipped` | `aborted`.
    pub state: String,
    /// Whole seconds, when the call reported a duration.
    pub secs: Option<u64>,
    /// The output preview, shown when the row is disclosed.
    pub output: String,
}

impl ToolView {
    /// The web's status label (`Timeline.tsx:261-277`): `Running`, else
    /// `Done`/`Failed`/`Finished` with ` · N s` when timed.
    pub fn status_label(&self) -> String {
        let word = match self.state.as_str() {
            "running" => return "Running".to_owned(),
            "done" | "complete" | "completed" => "Done",
            "failed" | "error" => "Failed",
            "skipped" => "Skipped",
            "aborted" => "Stopped",
            _ => "Finished",
        };
        match self.secs {
            Some(s) => format!("{word} · {s} s"),
            None => word.to_owned(),
        }
    }
}

/// One tool call as a compact single-line row (the web's 42 px `toolHeader`,
/// `Timeline.module.css:205-226`): kind glyph · `title · target` · status ·
/// check. Consecutive calls of one turn share ONE bordered card — this row
/// draws its own segment of it (`pos`): rounded top on the first, hairline
/// between rows, rounded bottom on the last. Disclosed (`open`), the output
/// preview hangs under the header in a mono well (the web's `toolBody`).
pub fn tool_row(tok: &str, t: &ToolView, pos: GroupPos, open: bool, m: &Metrics) -> String {
    let s = scale(m.density);
    // Per-corner radii (tl, tr, br, bl) — makepad's SDF box draws TWICE the
    // given radius (sdf.rs:407 "the effective visual radius is 2*r"), so 5.0
    // is the web's 10 px card corner — and the box inset that hides the
    // shared edge: a middle segment's top/bottom strokes fall OUTSIDE its
    // rect (border_inset −2), so only one hairline separates two rows.
    // A "square" corner is never 0: `box_all` with a zero radius has no
    // interior distance (the `sdf.box` fix at sdf.rs:412-414 never reached
    // box_all), so its stroke painted the WHOLE face in the border colour
    // (measured: the middle row's fill read #e5e5e7, not #f7f7f8). The shared
    // edges sit outside the rect anyway, so 1.0 there is invisible.
    let r = crate::desk::by(5.0, 3.0);
    let q = 1.0;
    let (radius, inset) = match pos {
        GroupPos::Single => (format!("vec4({r:.1}, {r:.1}, {r:.1}, {r:.1})"), "vec4(0.0, 0.0, 0.0, 0.0)"),
        GroupPos::First => (format!("vec4({r:.1}, {r:.1}, {q:.1}, {q:.1})"), "vec4(0.0, 0.0, 0.0, -2.0)"),
        GroupPos::Middle => (format!("vec4({q:.1}, {q:.1}, {q:.1}, {q:.1})"), "vec4(0.0, -2.0, 0.0, -2.0)"),
        GroupPos::Last => (format!("vec4({q:.1}, {q:.1}, {r:.1}, {r:.1})"), "vec4(0.0, -2.0, 0.0, 0.0)"),
    };
    let top_rule = matches!(pos, GroupPos::Middle | GroupPos::Last);
    let bottom_gap = if matches!(pos, GroupPos::Single | GroupPos::Last) { crate::desk::by(10.0, 8.0) } else { 0.0 };
    let kind = tool_kind(&t.title);
    let failed = matches!(t.state.as_str(), "failed" | "error");
    let running = t.state == "running";
    let glyph_color = if failed { RED } else { MUTED };
    let mark = if failed {
        svg(&format!("i{tok}_toolcell_mark"), "tool_failed.svg", 14.0, RED)
    } else if running {
        svg(
            &format!("i{tok}_toolcell_mark"),
            "components/working-row/assets/icon_spinner.svg",
            14.0,
            MUTED,
        )
    } else {
        svg(&format!("i{tok}_toolcell_mark"), "tool_check.svg", 14.0, GREEN)
    };
    let chevron = svg(
        &format!("i{tok}_toolcell_chev"),
        if open { "chevron_down.svg" } else { "chevron_right.svg" },
        12.0,
        MUTED,
    );
    let title_style = style(Face::Medium, s.small, s.small_line);
    let target_style = style(Face::Regular, s.small, s.small_line);
    let status_style = style(Face::Regular, s.tiny, s.small_line);
    let mut body = String::new();
    if top_rule {
        body.push_str(&format!(
            "View{{width: Fill height: 1 margin: Inset{{left: 12 right: 12}} show_bg: true draw_bg.color: {BORDER}}}\n"
        ));
    }
    // The header's click target rides the header itself (a fixed-height
    // overlay): a hit in the list item's Fit template sized the row on the
    // phone shell (48 px hits over 40 px rows split the card).
    body.push_str(&format!(
        "View{{width: Fill height: {h} flow: Overlay\n\
         i{tok}_toolcell_head := View{{width: Fill height: {h} flow: Right align: Align{{y: 0.5}} spacing: 8 \
         padding: Inset{{left: 12 right: 12}}\n\
         {glyph}\
         i{tok}_toolcell_text := View{{width: Fill height: Fit flow: Right align: Align{{y: 0.5}} spacing: 6\n\
         {title}{sep}{target}\
         }}\n\
         {status}{mark}{chevron}\
         }}\n\
         {head_hit}\
         }}\n",
        h = s.row_h,
        head_hit = hit("tool_hit", 4.0),
        glyph = svg(&format!("i{tok}_toolcell_kind"), &format!("tool_{kind}.svg"), 14.0, glyph_color),
        title = label(
            &format!("i{tok}_toolcell_title"),
            &crate::desk::tool_word(&t.title),
            &title_style,
            INK,
            "width: Fit height: Fit max_lines: 1",
        ),
        sep = if t.target.is_empty() {
            String::new()
        } else {
            label(&format!("i{tok}_toolcell_dot"), "·", &target_style, MUTED, "width: Fit height: Fit")
        },
        target = if t.target.is_empty() {
            String::new()
        } else {
            label(
                &format!("i{tok}_toolcell_target"),
                &t.target,
                &target_style,
                MUTED,
                "width: Fill height: Fit max_lines: 1 text_overflow: TextOverflow.Ellipsis",
            )
        },
        status = label(
            &format!("i{tok}_toolcell_status"),
            &t.status_label(),
            &status_style,
            if failed { RED } else { MUTED },
            "width: Fit height: Fit",
        ),
    ));
    if open {
        let preview = if t.output.trim().is_empty() {
            if running { "Waiting for tool output…".to_owned() } else { "Finished without text output.".to_owned() }
        } else {
            clip_lines(&t.output, 12)
        };
        body.push_str(&format!(
            "View{{width: Fill height: Fit padding: Inset{{left: 12 right: 12 bottom: 10}}\n\
             RoundedView{{width: Fill height: Fit padding: Inset{{left: 10 right: 10 top: 8 bottom: 8}} \
             draw_bg +: {{color: {SURFACE} border_radius: 3.0 border_size: 1.0 border_color: {BORDER}}}\n\
             {out}}}\n}}\n",
            out = label(
                &format!("i{tok}_toolcell_output"),
                &preview,
                &style(Face::Mono, 12.0, crate::desk::by(19.0, 17.0)),
                MUTED,
                "width: Fill height: Fit flow: Right{wrap: true}",
            ),
        ));
    }
    format!(
        "View{{width: Fill height: Fit flow: Down padding: Inset{{bottom: {bottom_gap}}}\n\
         i{tok}_toolcell := RoundedAllView{{width: Fill height: Fit flow: Down \
         draw_bg +: {{color: {TIP} border_size: {outline} border_color: {BORDER} border_radius: {radius} border_inset: {inset}}}\n\
         {body}}}\n}}\n",
        // This shader's stroke reads heavier than a hairline; a Mac's row sits
        // on its gray alone.
        outline = crate::desk::by("1.0", "0.0"),
    )
}

/// Keep at most `n` lines of a preview, marking the cut.
fn clip_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= n {
        return text.trim_end().to_owned();
    }
    let mut out = lines[..n].join("\n");
    out.push_str(&format!("\n… {} more lines", lines.len() - n));
    out
}

/// The settled turn's disclosure header ("Worked for 12s"), ABOVE its tool
/// group and answer (board 4 frame 1, conversation-09; Codex's yardstick):
/// small secondary text, a chevron when there are calls to fold, and the
/// call count so a folded turn still says what it hides.
pub fn worked_for(tok: &str, label_text: &str, tools: usize, open: bool, m: &Metrics) -> String {
    let s = scale(m.density);
    let text = label_text.trim_end_matches('›').trim_end().to_owned();
    let count = match tools {
        0 => String::new(),
        1 => "1 tool call".to_owned(),
        n => format!("{n} tool calls"),
    };
    let st = style(Face::Regular, s.small, s.small_line);
    // Nothing to say (no duration and no calls: a history turn without flow
    // timing): no row at all, rather than an empty 36 px band.
    if text.is_empty() && count.is_empty() {
        return "View{width: Fill height: 0}\n".to_owned();
    }
    // A turn with no measured duration (no flow timing: a history row, a
    // capture seed) shows only its count — never a dangling "· 3 tool calls".
    let mut row = if text.is_empty() {
        String::new()
    } else {
        label(&format!("i{tok}_workedfor_label"), &text, &st, MUTED, "width: Fit height: Fit")
    };
    if !count.is_empty() {
        if !text.is_empty() {
            row.push_str(&label(&format!("i{tok}_workedfor_dot"), "·", &st, MUTED, "width: Fit height: Fit"));
        }
        row.push_str(&label(&format!("i{tok}_workedfor_count"), &count, &st, MUTED, "width: Fit height: Fit"));
        row.push_str(&svg(
            &format!("i{tok}_workedfor_chev"),
            if open { "chevron_down.svg" } else { "chevron_right.svg" },
            12.0,
            MUTED,
        ));
    }
    // 28 px: the row is its own (>= 28 px) click target.
    format!(
        "View{{width: Fill height: Fit flow: Down padding: Inset{{top: {WORKED_TOP} bottom: 4}}\n\
         View{{width: Fill height: {h} flow: Overlay\n\
         i{tok}_workedfor := View{{width: Fill height: {h} flow: Right align: Align{{y: 0.5}} spacing: 6\n\
         {row}}}\n\
         {hit}\
         }}\n}}\n",
        h = crate::desk::by(28.0, 24.0),
        hit = hit("worked_hit", 4.0),
    )
}

/// The live activity line at the turn's tail (`TurnActivityIndicator`,
/// the web's "Working · 12s"): spinner + small secondary text.
pub fn working_row(tok: &str, text: &str, m: &Metrics) -> String {
    let s = scale(m.density);
    format!(
        "View{{width: Fill height: Fit flow: Down padding: Inset{{top: {WORKING_TOP} bottom: 8}}\n\
         i{tok}_workingrow := View{{width: Fill height: 24 flow: Right align: Align{{y: 0.5}} spacing: 8\n\
         {spin}{text}}}\n}}\n",
        spin = svg(
            &format!("i{tok}_workingrow_spin"),
            "components/working-row/assets/icon_spinner.svg",
            14.0,
            MUTED,
        ),
        text = label(
            &format!("i{tok}_workingrow_label"),
            text,
            &style(Face::Regular, s.small, s.small_line),
            MUTED,
            "width: Fit height: Fit",
        ),
    )
}

/// The answer's action row: copy / feedback / share, then the completion
/// time RIGHT after them — attached to its turn, not floated to the far
/// right edge of a wide column (conversation-09 keeps both on one line at
/// phone width; on a 680 px column the right edge detached it).
pub fn answer_actions(tok: &str, timestamp: &str, m: &Metrics) -> String {
    let s = scale(m.density);
    // Each icon sits at the LEFT of a full 28 px hit box, so the first one is
    // flush with the answer's text edge without a negative margin (that
    // margin clipped the copy target to 22 px — the A1 /snap check).
    let desk = crate::desk::active();
    let btn = |id: &str, file: &str, hit_id: Option<&str>| {
        // A desktop has no control that does nothing: the feedback and share
        // glyphs, which no action backs, stay off it.
        if desk && hit_id.is_none() {
            return String::new();
        }
        format!(
            "View{{width: {box_w} height: {box_w} flow: Overlay align: Align{{x: {box_x} y: 0.5}}\n{}{}}}\n",
            svg(id, &format!("components/answer-actions/assets/{file}"), crate::desk::by(16.0, 14.0), MUTED),
            hit_id.map(|h| hit(h, 6.0)).unwrap_or_default(),
            box_w = crate::desk::by(28.0, 24.0),
            box_x = crate::desk::by(0.0, 0.5),
        )
    };
    format!(
        "View{{width: Fill height: Fit flow: Down padding: Inset{{top: 2 bottom: {foot}}}\n\
         i{tok}_answeractions := View{{width: Fill height: {h} flow: Right align: Align{{y: 0.5}} spacing: {gap}\n\
         {copy}{thumbs}{share}\
         {time}}}\n}}\n",
        foot = crate::desk::by(12.0, 8.0),
        h = crate::desk::by(28.0, 24.0),
        gap = crate::desk::by(2.0, 6.0),
        // The copy control routes `answer.copy` (lib.rs, by this hit id in
        // the row's own scope).
        copy = btn(&format!("i{tok}_answeractions_0"), "icon_copy.svg", Some("answer_copy_hit")),
        thumbs = btn(&format!("i{tok}_answeractions_1"), "icon_thumbs.svg", None),
        share = btn(&format!("i{tok}_answeractions_2"), "icon_share.svg", None),
        time = label(
            &format!("i{tok}_answeractions_3"),
            timestamp,
            &style(Face::Regular, s.tiny, s.small_line),
            MUTED,
            "width: Fit height: Fit",
        ),
    )
}

/// The composer's live state the DSL depends on (never the draft: #32h).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerView {
    pub placeholder: String,
    /// The approval pill's label (the live permission mode, else the
    /// authored "Ask for approval").
    pub approval: String,
    /// The model picker's label.
    pub model: String,
}

/// The plus, mic and send controls of the composer row (px, square).
pub const COMPOSER_CONTROL: f64 = 32.0;

/// The composer's square controls by the running look (a Mac's 26-point tool).
fn composer_control() -> f64 {
    crate::desk::by(COMPOSER_CONTROL, 26.0)
}

/// The composer row's text size by the running look.
fn composer_row_px() -> f64 {
    crate::desk::by(COMPOSER_ROW_PX, 12.0)
}
/// The model picker's chevron and its gap to the label.
const COMPOSER_CHEVRON: f64 = 12.0;
const COMPOSER_CHEVRON_GAP: f64 = 4.0;
/// The control row's two labels: the web's session strip type
/// (`SessionConfig.module.css:3-19`, 500 13px/20px) at every density.
const COMPOSER_ROW_PX: f64 = 13.0;
const COMPOSER_ROW_LINE: f64 = 20.0;

/// The composer control row's spacing and the width each label may take.
///
/// The web keeps its actions whole (`.composer-actions { flex: none }`,
/// `styles.css:928-933`) and lets the strip text ellipsize
/// (`SessionConfig.module.css:37-42`). Makepad's row does not shrink a `Fit`
/// child, so the labels carry an explicit max: the card width minus every
/// fixed part, two thirds for the approval pill. Measured at 360x780 before
/// this budget: the row needed 362 px of a 336 px card and the send control
/// was cut to 15 px.
///
/// The maxima are NOT in the composer's DSL: the host applies them to the
/// live labels (`lib.rs` `apply_composer_fit`). A width in the DSL changed
/// the mount string on every resize step, and each remount replaced the
/// TextInput — measured on a maximize: nine remounts and the typed draft
/// gone. Only the density-level spacing below rides the DSL.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComposerRowFit {
    /// The row's side padding.
    pub pad_x: f64,
    /// The gap between two neighbouring controls (the spacer carries none).
    pub gap: f64,
    /// The approval pill's side padding.
    pub pill_pad: f64,
    /// The model picker's side padding.
    pub model_pad: f64,
    /// The approval label's max width (it ellipsizes past it).
    pub approval_max: f64,
    /// The model label's max width (it ellipsizes past it).
    pub model_max: f64,
}

impl ComposerRowFit {
    /// Every part of the row that is not one of the two labels.
    pub fn fixed_w(&self) -> f64 {
        // plus | pill | spacer | model | mic | send: four gaps. n12g (E10 1): a
        // desktop draws no mic, so its row is two square controls and three
        // gaps; counting the mic held 32 points back from the labels, and the
        // permission pop-up cut its words beside an empty stretch of row.
        let (controls, gaps) = crate::desk::by((3.0, 4.0), (2.0, 3.0));
        2.0 * self.pad_x
            + controls * composer_control()
            + gaps * self.gap
            + 2.0 * self.pill_pad
            + 2.0 * self.model_pad
            + COMPOSER_CHEVRON_GAP
            + COMPOSER_CHEVRON
            // A Mac's two pop-ups each carry a chevron.
            + crate::desk::by(0.0, COMPOSER_CHEVRON_GAP + COMPOSER_CHEVRON)
    }
}

pub fn composer_row_fit(m: &Metrics) -> ComposerRowFit {
    let (pad_x, gap, pill_pad, model_pad) = match m.density {
        Density::Desktop if crate::desk::active() => (10.0, 6.0, 8.0, 8.0),
        Density::Desktop => (10.0, 6.0, 12.0, 6.0),
        Density::Phone => (8.0, 4.0, 10.0, 2.0),
    };
    let mut fit = ComposerRowFit { pad_x, gap, pill_pad, model_pad, approval_max: 0.0, model_max: 0.0 };
    let labels = (m.composer_w - fit.fixed_w()).max(0.0);
    fit.model_max = (labels * 0.36).clamp(40.0, 160.0).floor();
    fit.approval_max = (labels - fit.model_max).max(40.0).floor();
    fit
}

/// A13 — the label maxima by NEED (judge, 360 px: the seats read "Write ·
/// Network..." and "deepse..."): each label asks for its natural width (the
/// kit's advance estimate, `board3::ui::text_w`, with a margin — measured on
/// desktop the estimate ran 4 px short of "deepseek-v4-flash"); when both
/// fit, each gets it and the model takes the spare; when not, a label that
/// needs at most half the room keeps its own width and the other takes the
/// rest, else they split it. The web lets the strip ellipsize the same way
/// (`.trigger { max-width: min(360px, 45cqw) }`, `.triggerLabel` ellipsis).
pub fn composer_row_fit_for(m: &Metrics, approval: &str, model: &str) -> ComposerRowFit {
    use crate::screens::board3::ui::{text_w, Face as Ui};
    let mut fit = composer_row_fit(m);
    let room = (m.composer_w - fit.fixed_w()).max(0.0).floor();
    let need = |s: &str, face: Ui| (text_w(s, composer_row_px(), face) * 1.06 + 4.0).ceil();
    // A desktop's model pop-up draws its name in the regular face, as its permission one does.
    let model_face = if crate::desk::active() { Ui::Regular } else { Ui::Medium };
    let (a, mo) = (need(approval, Ui::Regular), need(model, model_face));
    let half = (room / 2.0).floor();
    let approval_max = if a + mo <= room || a <= half {
        a
    } else if mo <= half {
        room - mo
    } else {
        half
    };
    fit.approval_max = approval_max.min(room);
    fit.model_max = (room - fit.approval_max).max(0.0);
    fit
}

/// A13 — the approval pill's label on a phone: the mode alone ("Write ·
/// Network allowed" -> "Write"). The web hides the permission trigger's
/// label at a narrow container (`SessionControlBar.module.css` `@container
/// (max-width: 460px)`); the mode word keeps the seat legible and leaves the
/// model its name. The menu still names both (`board3::seats`).
pub fn compact_permission_label(label: &str) -> &str {
    label.split(" · ").next().map(str::trim).filter(|s| !s.is_empty()).unwrap_or(label)
}

/// The composer card (`styles.css:785-850` `.composer`): one rounded card,
/// the input on top, ONE control row under it — `+` and the approval pill on
/// the left, the model picker, mic and the round send control on the right.
/// Every control is laid out by flow (no artboard margins), so the card
/// spans the column at any width. The input WRAPS (multiline, Enter still
/// submits), so a long prompt grows the card instead of scrolling sideways —
/// the horizontal scroll is what hid the first typed character.
///
/// The host's hit ids (`plus_hit`, `approval_pill_hit`, `mic_hit`,
/// `send_hit`) are real buttons INSIDE this tree, each over its own control,
/// so a click lands on exactly the control it shows.
pub fn composer(c: &ComposerView, m: &Metrics) -> String {
    let s = scale(m.density);
    let input_px = s.body;
    let fit = composer_row_fit(m);
    let gap = fit.gap;
    let desk = crate::desk::active();
    let control = composer_control();
    // A Mac's composer: 8-point corners, a 24-point popup, the blue default.
    let (card_corner, input_x, input_top, row_h, seat_h, seat_corner) = if desk {
        (4.0, 12.0, 10.0, 34.0, 24.0, 3.0)
    } else {
        (9.0, 16.0, 14.0, 48.0, 30.0, 15.0)
    };
    let (send_fill, send_corner) = if desk { ("#0071e3ff", 3.0) } else { ("#000000ff", 16.0) };
    // What a control that can't act right now is filled with.
    let busy_fill = if desk { crate::desk::current().border } else { "#c7c7ccff".to_owned() };
    let busy_fill_veil = busy_fill.clone();
    let icon_btn = |hit_id: &str, file: &str, size: f64, left: f64| {
        format!(
            "View{{width: {control} height: {control} margin: Inset{{left: {left}}} \
             flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n\
             {svg}{hit}}}\n",
            svg = svg(&format!("{hit_id}_icon"), &format!("components/composer/assets/{file}"), size, MUTED),
            hit = hit(hit_id, 16.0),
        )
    };
    // The labels' max widths are applied by the host (`apply_composer_fit`),
    // never written here: the DSL must not change with the width.
    // A Mac's placeholder is the system's (E7 5).
    let ph = if desk { crate::desk::placeholder() } else { MUTED };
    let one_line = "width: Fit height: Fit max_lines: 1 text_overflow: TextOverflow.Ellipsis";
    let seat_veil = |id: &str| {
        if desk {
            let face = crate::desk::current().surface;
            format!("{id} := RoundedView{{width: Fill height: Fill visible: false draw_bg +: {{color: {}a6 border_radius: {seat_corner}}}}}\n", &face[..7])
        } else {
            String::new()
        }
    };
    format!(
        "i0_composer := RoundedView{{width: Fill height: Fit flow: Down padding: 0\n\
         draw_bg +: {{color: {SURFACE} border_radius: {card_corner} border_size: 1.0 border_color: {BORDER}}}\n\
         View{{width: Fill height: Fit padding: Inset{{left: {input_x} right: {input_x} top: {input_top} bottom: 2}}\n\
         i0_composer_0 := DesignInput{{width: Fill height: Fit min_height: {min_h} max_height: 200 \
         padding: Inset{{top: 2 bottom: 2}} margin: 0\n\
         is_multiline: true submit_on_enter: true flow: Right{{wrap: true}}\n\
         text: \"\" empty_text: {placeholder:?}\n\
         draw_text +: {{color: {INK} color_empty: {ph} text_style: {input_style} \
         get_color: fn() {{return mix(self.color, self.color_empty, self.empty)}}}}\n\
         draw_cursor +: {{color: {INK}}}\n\
         draw_selection +: {{color: #2f6feb33 color_hover: #2f6feb33 color_focus: #2f6feb40 \
         color_down: #2f6feb40 color_empty: #2f6feb33 color_disabled: #2f6feb33}}\n\
         }}\n}}\n\
         i0_composer_row := View{{width: Fill height: {row_h} flow: Right align: Align{{y: 0.5}} spacing: 0 \
         padding: Inset{{left: {pad_x} right: {pad_x} bottom: 4}}\n\
         {plus}\
         i0_composer_2 := View{{width: Fit height: {seat_h} margin: Inset{{left: {gap}}} flow: Overlay\n\
         RoundedView{{width: Fit height: {seat_h} flow: Right align: Align{{y: 0.5}}{approval_spacing} \
         padding: Inset{{left: {pill_pad} right: {pill_pad}}} \
         draw_bg +: {{color: {SURFACE} border_radius: {seat_corner} border_size: 1.0 border_color: {BORDER}}}\n\
         {approval}{approval_chev}}}\n\
         {approval_hit}{approval_veil}}}\n\
         View{{width: Fill height: 1}}\n\
         i0_composer_model := View{{width: Fit height: {seat_h} margin: Inset{{left: {gap}}} flow: Overlay\n\
         {model_seat}{{width: Fit height: {seat_h} flow: Right \
         align: Align{{y: 0.5}} spacing: {COMPOSER_CHEVRON_GAP} \
         padding: Inset{{left: {model_pad} right: {model_pad}}}{model_face}\n\
         {model}{model_chev}}}\n\
         {model_hit}{model_veil}}}\n\
         {mic}\
         i0_composer_5 := View{{width: {control} height: {control} margin: Inset{{left: {gap}}} \
         flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n\
         RoundedView{{width: {control} height: {control} draw_bg +: {{color: {send_fill} border_radius: {send_corner}}}}}\n\
         composer_stop_busy := RoundedView{{width: {control} height: {control} visible: false draw_bg +: {{color: {busy_fill} border_radius: {send_corner}}}}}\n\
         {send_off}composer_send_icon := View{{width: 16 height: 16 flow: Overlay\n{send_icon}}}\n\
         composer_stop_icon := View{{width: 16 height: 16 flow: Overlay visible: false\n{stop_icon}}}\n\
         {send_hit}{send_veil}}}\n\
         }}\n}}\n",
        min_h = (s.body_line + 4.0).round(),
        placeholder = c.placeholder,
        input_style = style(Face::Regular, input_px, s.body_line),
        pad_x = fit.pad_x,
        pill_pad = fit.pill_pad,
        model_pad = fit.model_pad,
        plus = icon_btn("plus_hit", "icon_plus1.svg", crate::desk::by(18.0, 16.0), 0.0),
        approval = label(
            "i0_composer_2_0",
            match m.density {
                Density::Phone => compact_permission_label(&c.approval),
                Density::Desktop => &c.approval,
            },
            &style(Face::Regular, composer_row_px(), COMPOSER_ROW_LINE),
            INK,
            one_line,
        ),
        approval_hit = hit("approval_pill_hit", 15.0),
        // A Mac's two pop-ups are one control: a bordered face, a regular word, a chevron.
        approval_spacing = if desk { format!(" spacing: {COMPOSER_CHEVRON_GAP}") } else { String::new() },
        approval_chev = if desk { svg("i0_composer_2_chev", "chevron_down.svg", COMPOSER_CHEVRON, MUTED) } else { String::new() },
        model_seat = if desk { "RoundedView" } else { "View" },
        model_face = if desk {
            format!(" draw_bg +: {{color: {SURFACE} border_radius: {seat_corner} border_size: 1.0 border_color: {BORDER}}}")
        } else {
            String::new()
        },
        model = label(
            "i0_composer_4",
            &c.model,
            &style(if desk { Face::Regular } else { Face::Medium }, composer_row_px(), COMPOSER_ROW_LINE),
            INK,
            one_line,
        ),
        model_chev = svg("i0_composer_4_chev", "chevron_down.svg", COMPOSER_CHEVRON, MUTED),
        // A10 — the model seat opens the model menu (web `ModelControl`'s
        // trigger); the host hides the seat when the server lacks the menu.
        model_hit = hit("model_seat_hit", 15.0),
        // While a lost connection re-dials a Mac's seats stay, under a veil of
        // the card's face (the host shows it: `OctoscodeView::sync_seats`).
        approval_veil = seat_veil("i0_composer_2_veil"),
        model_veil = seat_veil("i0_composer_model_veil"),
        // Reserved for a later card, unwired: a desktop draws no dead control.
        mic = if desk { String::new() } else { icon_btn("mic_hit", "icon_mic1.svg", 18.0, gap) },
        busy_fill = busy_fill,
        send_off = if desk {
            format!("composer_send_off := RoundedView{{width: {control} height: {control} visible: false draw_bg +: {{color: {busy_fill} border_radius: {send_corner}}}}}\n")
        } else {
            String::new()
        },
        // The off control's glyph reads as off in both appearances: a veil of
        // the off face over it (and over the hit's hover).
        send_veil = if desk {
            let veil = format!("{}a6", &busy_fill_veil[..7]);
            format!("composer_send_veil := RoundedView{{width: {control} height: {control} visible: false draw_bg +: {{color: {veil} border_radius: {send_corner}}}}}\n")
        } else {
            String::new()
        },
        // A desktop's control stays blue in dark, so its glyph must not follow
        // the retint of white to the surface colour.
        send_icon = svg("i0_composer_5_0", "components/composer/assets/icon_send.svg", 16.0, if desk { "#fffffeff" } else { "#ffffffff" }),
        // The running turn's STOP glyph (conversation-08 `stop2`): shown by
        // the host while `turn.active`, so the DSL never changes per turn.
        stop_icon = svg("i0_composer_5_1", "components/composer/assets/icon_stop.svg", 16.0, if desk { "#fffffeff" } else { "#ffffffff" }),
        send_hit = hit("send_hit", 16.0),
    )
}

/// A7 — what the composer dock shows above the composer card, from the turn
/// controller's state (never the draft).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComposerExtras {
    /// Prompts waiting behind the active turn (FIFO count).
    pub queued: usize,
    /// "Steer now" is offered (an accepted turn runs and the server steers).
    pub steer: bool,
    pub send_now: bool,
    pub queue_draft: bool,
    /// The held turn's recovery phase: `checking` / `unknown` /
    /// `unavailable` / `error` (`TurnRecoveryState.phase`).
    pub recovery: Option<String>,
    /// A focused peer Session's slug: the composer is read-only.
    pub peer_readonly: Option<String>,
    /// The driver-seat handover line (`seat.rs`): the in-flight status
    /// ("Handing back control…" / "Resuming chat…") or the last failure
    /// (`true` = error tone).
    pub seat_status: Option<(String, bool)>,
    /// n12g (E10 3): on a desktop, the images + attached to this chat's next
    /// message, in order (each its own chip with a remove control).
    pub attachments: Vec<AttachmentChip>,
}

/// n12g (E10 3) — one attached image as a desktop's composer shows it, as Mail
/// and Messages show theirs: a thumbnail, the name, a line under it (the size,
/// "Uploading…" while it goes up with the message, or why it didn't) and ×.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AttachmentChip {
    pub name: String,
    pub detail: String,
    /// Its upload failed: the line is in the alert ink.
    pub failed: bool,
    /// The local file it was read from, for its thumbnail.
    pub path: Option<String>,
}

/// A7 — the queued chip (conversation-08 `queued_row`: `1 queued · Steer
/// now · ✕` on the tip-grey pill above the composer; behaviour
/// `QueuedPrompts.tsx`: the count, steering, removal without interrupting
/// the server), the turn recovery notice (`TurnRecoveryNotice.tsx` + its
/// CSS) and the read-only peer row (`ComposerInput.tsx:255-265`). The hit
/// ids (`queue_steer_hit`, `queue_remove_hit`, `recovery_check_hit`,
/// `recovery_continue_hit`) are routed by the host.
pub fn composer_extras(x: &ComposerExtras, m: &Metrics) -> String {
    let mut out = String::new();
    if let Some((text, error)) = &x.seat_status {
        out.push_str(&seat_status_row(text, *error, m));
    }
    if let Some(phase) = &x.recovery {
        out.push_str(&recovery_notice(phase, x.queued > 0, m));
    }
    if x.queue_draft {
        out.push_str(&format!("queue_draft := View{{width: Fit height: 32 flow: Overlay align: Align{{y: 0.5}}\n{}{} }}\n",
            label("queue_draft_label", tr("Queue (Tab)"), &style(Face::Medium, 13.0, 20.0), INK, "width: Fit height: Fit"),
            hit("queue_draft_hit", 6.0)));
    }
    if x.send_now {
        out.push_str(&format!("queue_send_now := View{{width: Fit height: 32 flow: Overlay align: Align{{y: 0.5}}\n{}{} }}\n",
            label("queue_send_now_label", tr("Send now (Ctrl+X)"), &style(Face::Medium, 13.0, 20.0), INK, "width: Fit height: Fit"),
            hit("queue_send_now_hit", 6.0)));
    }
    if x.queued > 0 {
        out.push_str(&queue_chip(x.queued, x.steer, m));
    }
    if let Some(slug) = &x.peer_readonly {
        out.push_str(&peer_readonly_row(slug, m));
    }
    if !x.attachments.is_empty() && x.peer_readonly.is_none() {
        out.push_str(&attachment_strip(&x.attachments));
    }
    if out.is_empty() {
        return String::new();
    }
    format!("composer_extras := View{{width: Fill height: Fit flow: Down spacing: 8 padding: Inset{{bottom: 8}}\n{out}}}\n")
}

/// n12g (E10 3) — the images attached to the next message, a row of chips
/// over the composer (`composer_att_<i>`, its × `composer_att_remove_<i>`,
/// routed by the host to the draft's remove).
fn attachment_strip(chips: &[AttachmentChip]) -> String {
    let name_style = style(Face::Regular, 12.0, 16.0);
    let detail_style = style(Face::Regular, 11.0, 14.0);
    let mut out = String::from("composer_atts := View{width: Fill height: Fit flow: Right{wrap: true} spacing: 6\n");
    for (i, c) in chips.iter().enumerate() {
        let thumb = match &c.path {
            Some(p) => format!(
                "RoundedView{{width: 28 height: 28 flow: Overlay draw_bg +: {{color: {TIP} border_radius: 3}}\n\
                 composer_att_{i}_img := Image{{width: Fill height: Fill fit: ImageFit.Biggest src: file_resource({p:?})}}}}\n"
            ),
            None => format!(
                "View{{width: 28 height: 28 align: Align{{x: 0.5 y: 0.5}}\n{}}}\n",
                svg(&format!("composer_att_{i}_glyph"), "b3_image.svg", 16.0, MUTED)
            ),
        };
        let ink = if c.failed { crate::desk::danger_on_face() } else { MUTED };
        out.push_str(&format!(
            "composer_att_{i} := RoundedView{{width: Fit height: 40 flow: Right align: Align{{y: 0.5}} spacing: 8 \
             padding: Inset{{left: 6 right: 2}}\n\
             draw_bg +: {{color: {SURFACE} border_radius: 4 border_size: 1.0 border_color: {BORDER}}}\n\
             {thumb}\
             View{{width: Fit height: Fit flow: Down spacing: 1\n{name}{detail}}}\n\
             View{{width: 22 height: 22 flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n{x}{remove}}}\n\
             }}\n",
            name = label(
                &format!("composer_att_{i}_name"),
                &c.name,
                &name_style,
                INK,
                "width: Fit height: Fit max_width: 180 max_lines: 1 text_overflow: TextOverflow.Ellipsis",
            ),
            detail = label(&format!("composer_att_{i}_detail"), &c.detail, &detail_style, ink, "width: Fit height: Fit"),
            x = svg(&format!("composer_att_{i}_x"), "b3_x_small.svg", 10.0, MUTED),
            remove = hit(&format!("composer_att_remove_{i}"), 11.0),
        ));
    }
    out.push_str("}\n");
    out
}

fn queue_chip(queued: usize, steer: bool, m: &Metrics) -> String {
    let (px, h) = match m.density {
        Density::Desktop => crate::desk::by((13.0, 32.0), (12.0, 28.0)),
        Density::Phone => (14.0, 34.0),
    };
    let st = style(Face::Medium, px, 20.0);
    let dot = || label("", "·", &st, MUTED, "width: Fit height: Fit");
    let steer_part = if steer {
        format!(
            "{dot}View{{width: Fit height: {h} flow: Overlay align: Align{{y: 0.5}}\n\
             View{{width: Fit height: Fill flow: Right align: Align{{y: 0.5}} padding: Inset{{left: 2 right: 2}}\n{l}}}\n\
             {hit}}}\n",
            dot = dot(),
            l = label("queue_steer_label", tr("Steer now"), &st, INK, "width: Fit height: Fit"),
            hit = hit("queue_steer_hit", 6.0),
        )
    } else {
        String::new()
    };
    format!(
        "queue_chip := RoundedView{{width: Fit height: {h} flow: Right align: Align{{y: 0.5}} spacing: 5 \
         margin: Inset{{top: 2}} padding: Inset{{left: 12 right: 2}}\n\
         draw_bg +: {{color: {TIP} border_radius: {corner} border_size: 1.0 border_color: {BORDER}}}\n\
         {count}{steer_part}{dot}\
         View{{width: {x_w} height: {x_w} flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n{x}{remove}}}\n\
         }}\n",
        corner = crate::desk::by(10.0, 3.0),
        x_w = 28.0,
        count = label("queue_count", &crate::i18n::tr_with("{count} queued", &[("count", &queued.to_string())]), &st, INK, "width: Fit height: Fit"),
        dot = dot(),
        x = svg("queue_remove_icon", "b3_close.svg", 12.0, MUTED),
        remove = hit("queue_remove_hit", 14.0),
    )
}

fn recovery_notice(phase: &str, queued: bool, m: &Metrics) -> String {
    let checking = phase == "checking";
    let title = if checking { "Checking the last response" } else { "Response status is uncertain" };
    let mut body = vec![if checking {
        "Checking whether Octos is still working or has finished. Your message will not be sent again."
    } else {
        "Octos has not confirmed whether the last response is still running or has finished. Sending is paused so the same work is not started twice."
    }];
    if phase == "unavailable" {
        body.push("This server does not support checking response status. You can copy your text and manage the connection in Settings.");
    }
    if phase == "error" {
        body.push("The status check failed. You can try again.");
    }
    if !checking {
        body.push("If the response was lost, for example because the server restarted, continue without confirming its outcome. This does not stop or resend it; queued messages send next.");
    }
    let s = scale(m.density);
    let title_st = style(Face::SemiBold, s.body, s.body_line);
    let body_st = style(Face::Regular, s.small, s.small_line + 1.0);
    let wrap = format!("width: Fill height: Fit flow: Right{{wrap: true}} max_lines: {FIT_WRAP_LINES}");
    let mut paras = String::new();
    for (i, p) in body.iter().enumerate() {
        paras.push_str(&label(&format!("recovery_body_{i}"), tr(p), &body_st, INK, &wrap));
    }
    let btn = |id: &str, text: &str, live: bool| {
        let fg = if live { INK } else { MUTED };
        let hit_part = if live { hit(id, 8.0) } else { String::new() };
        format!(
            "View{{width: Fit height: {h} flow: Overlay\n\
             RoundedView{{width: Fit height: {h} flow: Right align: Align{{y: 0.5}} padding: Inset{{left: {pad} right: {pad}}} \
             draw_bg +: {{color: {SURFACE} border_radius: {corner} border_size: 1.0 border_color: {BORDER}}}\n{l}}}\n\
             {hit_part}}}\n",
            h = crate::desk::by(36.0, 28.0),
            pad = crate::desk::by(14.0, 12.0),
            corner = crate::desk::by(8.0, 3.0),
            l = label(&format!("{id}_label"), &crate::desk::title(tr(text)), &style(Face::Medium, s.small, s.small_line), fg, "width: Fit height: Fit"),
        )
    };
    let mut buttons = String::new();
    if phase != "unavailable" {
        buttons.push_str(&btn("recovery_check_hit", tr(if checking { "Checking status…" } else { "Check status" }), !checking));
    }
    if !checking {
        buttons.push_str(&btn("recovery_continue_hit", tr("Continue without it"), true));
    }
    let footer = if queued {
        label(
            "recovery_footer",
            tr("Queued messages remain here. You can remove them below."),
            &style(Face::Regular, s.tiny, s.small_line),
            MUTED,
            &wrap,
        )
    } else {
        String::new()
    };
    format!(
        "recovery_notice := RoundedView{{width: Fill height: Fit flow: Down spacing: 8 \
         padding: Inset{{left: 16 right: 16 top: {pad_y} bottom: {pad_y}}}\n\
         draw_bg +: {{color: {SURFACE} border_radius: {corner} border_size: 1.0 border_color: #f3d7a6ff}}\n\
         {t}\
         View{{width: Fill height: Fit flow: Down spacing: 4\n{paras}}}\n\
         View{{width: Fill height: Fit flow: Right{{wrap: true}} spacing: 8\n{buttons}}}\n\
         {footer}\
         }}\n",
        pad_y = crate::desk::by(14.0, 12.0),
        corner = crate::desk::by(12.0, 5.0),
        t = label("recovery_title", tr(title), &title_st, INK, &wrap),
    )
}

/// `PEER_READONLY_HINT` (`peer-readonly.ts:17-18`): "↳ read-only peer ·
/// {slug} · steer from the master" — the dim status row that replaces the
/// editable composer (`ComposerInput.tsx:255-265`).
/// A7 — the seat handover line above the composer (the web's session strip
/// `handing-back` / `resuming-chat` states and the Resume chat notice,
/// App.tsx:2051-2056/:3152): the peer row's quiet pill; a failure reads in
/// the error ink.
fn seat_status_row(text: &str, error: bool, m: &Metrics) -> String {
    let s = scale(m.density);
    format!(
        "seat_status := RoundedView{{width: Fill height: Fit flow: Right align: Align{{y: 0.5}} \
         padding: Inset{{left: {pad_x} right: {pad_x} top: {pad_y} bottom: {pad_y}}}\n\
         draw_bg +: {{color: {TIP} border_radius: {corner} border_size: 1.0 border_color: {edge}}}\n\
         {l}}}\n",
        pad_x = crate::desk::by(16.0, 12.0),
        pad_y = crate::desk::by(10.0, 6.0),
        corner = crate::desk::by(9.0, 3.0),
        edge = if error { "#f4c7c9ff" } else { BORDER },
        l = label(
            "seat_status_label",
            // A24: the seat words (seat.rs) in the current language; a
            // server's own refusal text is not a catalog key and stays.
            tr(text),
            &style(Face::Regular, s.small, s.small_line),
            if error { "#cf222eff" } else { MUTED },
            "width: Fill height: Fit",
        ),
    )
}

fn peer_readonly_row(slug: &str, m: &Metrics) -> String {
    let s = scale(m.density);
    format!(
        "peer_readonly := RoundedView{{width: Fill height: {h} flow: Right align: Align{{y: 0.5}} \
         padding: Inset{{left: {pad_x} right: {pad_x}}}\n\
         draw_bg +: {{color: {TIP} border_radius: {corner} border_size: 1.0 border_color: {BORDER}}}\n\
         {l}}}\n",
        h = crate::desk::by(48.0, 32.0),
        pad_x = crate::desk::by(16.0, 12.0),
        corner = crate::desk::by(9.0, 3.0),
        l = label(
            "peer_readonly_label",
            &tr1("↳ read-only peer · {value0} · steer from the master", slug),
            &style(Face::Regular, s.small, s.small_line),
            MUTED,
            "width: Fill height: Fit max_lines: 1 text_overflow: TextOverflow.Ellipsis",
        ),
    )
}

/// The empty conversation (`Timeline.tsx:111-134` + conversation-02): a
/// quiet mark, the question, the web's one-line hint, and the workspace chip
/// when the session reported its root.
pub fn empty_state(workspace: Option<&str>, m: &Metrics) -> String {
    let (title_px, title_line) = match m.density {
        Density::Desktop => crate::desk::by((26.0, 34.0), (17.0, 24.0)),
        Density::Phone => (23.0, 30.0),
    };
    let title = match workspace {
        Some(ws) if !ws.is_empty() => tr1("What should we build in {value0}?", ws),
        _ => tr("What should we build?").to_owned(),
    };
    let chip = match workspace {
        Some(ws) if !ws.is_empty() => format!(
            "View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: 16}}\n\
             RoundedView{{width: Fit height: {h} flow: Right align: Align{{y: 0.5}} spacing: 6 \
             padding: Inset{{left: {l} right: {r}}} draw_bg +: {{color: {SURFACE} border_radius: {corner} \
             border_size: 1.0 border_color: {BORDER}}}\n\
             {icon}{text}}}\n}}\n",
            h = crate::desk::by(28.0, 24.0),
            l = crate::desk::by(10.0, 8.0),
            r = crate::desk::by(12.0, 8.0),
            corner = crate::desk::by(14.0, 3.0),
            icon = svg("empty_chip_icon", "folder.svg", 14.0, MUTED),
            text = label(
                "empty_chip",
                ws,
                &style(Face::Regular, crate::desk::by(13.0, 12.0), 18.0),
                INK,
                "width: Fit height: Fit",
            ),
        ),
        _ => String::new(),
    };
    format!(
        "empty_col := View{{width: Fill height: Fit flow: Down align: Align{{x: 0.5}} \
         padding: Inset{{left: 24 right: 24}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}}\n\
         RoundedView{{width: {well} height: {well} flow: Overlay align: Align{{x: 0.5 y: 0.5}} \
         draw_bg +: {{color: {well_fill} border_radius: {well_corner}}}\n\
         {mark}}}\n}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: {gap_t}}}\n\
         {title}}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: {gap_h}}}\n\
         {hint}}}\n\
         {chip}}}\n",
        well = crate::desk::by(48.0, 36.0),
        well_fill = crate::desk::by(RAISED, "#00000000"),
        well_corner = crate::desk::by(7.0, 0.5),
        gap_t = crate::desk::by(18.0, 6.0),
        gap_h = crate::desk::by(8.0, 6.0),
        mark = svg("empty_mark", "chat_terminal.svg", crate::desk::by(26.0, 36.0), crate::desk::by(INK, MUTED)),
        title = label(
            "empty_title",
            &title,
            &style(Face::SemiBold, title_px, title_line),
            INK,
            // Fill + max_width (centred by the parent's align): an aligned
            // row is placed against the max bound, so a Fit box that shrank
            // to the text clipped it (measured: "…build in o|").
            "width: Fill height: Fit max_width: 520 align: Align{x: 0.5} flow: Right{wrap: true}",
        ),
        hint = label(
            "empty_hint",
            tr("Describe a change, investigate a bug, or ask how the code works."),
            &style(Face::Regular, crate::desk::by(14.0, 13.0), crate::desk::by(22.0, 18.0)),
            MUTED,
            "width: Fill height: Fit max_width: 480 align: Align{x: 0.5} flow: Right{wrap: true}",
        ),
    )
}

/// A19b — the conversation of a Session whose history is not on screen yet:
/// `None` while it loads (the web's "Loading conversation…", `App.tsx:2564`,
/// and its "Restoring session state", `App.tsx:2741`), `Some(reason)` when it
/// could not be read (the web's "Session recovery required" with the
/// server's detail, `App.tsx:2742-2750`). Drawn in the empty welcome's place
/// (`empty_state`), so a Session that has history never shows the welcome.
pub fn history_state(failed: Option<&str>, m: &Metrics) -> String {
    let (title_px, title_line) = match m.density {
        Density::Desktop => crate::desk::by((22.0, 30.0), (15.0, 22.0)),
        Density::Phone => (20.0, 27.0),
    };
    let retry = if failed.is_some() && crate::desk::active() {
        let word = crate::desk::title(tr("Try again")).into_owned();
        format!(
            "View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: 10}}\n\
             history_retry_wrap := View{{width: {w} height: 28 flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n\
             RoundedView{{width: Fill height: Fill draw_bg +: {{color: #0071e3ff border_radius: 3.0}}}}\n\
             {l}{h}}}\n}}\n",
            w = (word.chars().count() as f64 * 7.0 + 28.0).max(96.0).ceil(),
            l = label("history_retry_label", &word, &style(Face::Regular, 13.0, 18.0), "#fffffeff", "width: Fit height: Fit"),
            h = hit_on_blue("history_retry", 3.0),
        )
    } else {
        String::new()
    };
    let (title, hint, ink) = match failed {
        None => (
            tr("Loading conversation…").to_owned(),
            crate::desk::by(tr("Restoring session state"), "").to_owned(),
            INK,
        ),
        // A desktop says what to do in plain words, in the ink of its other
        // states; the server's raw reason is the log's.
        Some(reason) if crate::desk::active() => {
            makepad_widgets::log!("[octoscode] history could not be loaded: {reason}");
            (tr("Couldn’t Load This Chat").to_owned(), tr("Its history couldn’t be read.").to_owned(), INK)
        }
        Some(reason) => (
            tr("Session recovery required").to_owned(),
            tr1(
                "The conversation history could not be loaded: {value0}. Reopen it from the sidebar to try again.",
                reason.trim().trim_end_matches('.'),
            ),
            "#cf222eff",
        ),
    };
    format!(
        "history_col := View{{width: Fill height: Fit flow: Down align: Align{{x: 0.5}} \
         padding: Inset{{left: 24 right: 24}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}}\n\
         RoundedView{{width: {well} height: {well} flow: Overlay align: Align{{x: 0.5 y: 0.5}} \
         draw_bg +: {{color: {well_fill} border_radius: {well_corner}}}\n\
         {mark}}}\n}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: {gap_t}}}\n\
         {title}}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: {gap_h}}}\n\
         {hint}}}\n{retry}}}\n",
        well = crate::desk::by(48.0, 36.0),
        well_fill = crate::desk::by(RAISED, "#00000000"),
        well_corner = crate::desk::by(7.0, 0.5),
        gap_t = crate::desk::by(18.0, 6.0),
        gap_h = crate::desk::by(8.0, 6.0),
        mark = if failed.is_none() && crate::desk::active() {
            spinner("history_mark", 20.0)
        } else {
            svg("history_mark", "chat_terminal.svg", crate::desk::by(26.0, 36.0), MUTED)
        },
        title = label(
            "history_title",
            &title,
            &style(Face::SemiBold, title_px, title_line),
            ink,
            "width: Fill height: Fit max_width: 520 align: Align{x: 0.5} flow: Right{wrap: true}",
        ),
        hint = label(
            "history_hint",
            &hint,
            &style(Face::Regular, crate::desk::by(14.0, 13.0), crate::desk::by(22.0, 18.0)),
            MUTED,
            "width: Fill height: Fit max_width: 480 align: Align{x: 0.5} flow: Right{wrap: true}",
        ),
    )
}

/// A Mac's new chat the server refused to open (`flow::Conversation::
/// start_refused`): said where the chat would start, in the empty welcome's
/// place, as [`history_state`] says a chat that couldn't load (its Mac
/// geometry: a 36 pt mark, 6 pt gaps): the mark, the
/// title, the server's reason in a person's words (the toast's, `toasts::
/// cause_shown`) and Try Again, the default, which opens this chat again.
pub fn start_state(reason: &str) -> String {
    let words = crate::screens::toasts::cause_shown(reason);
    let words = if words.is_empty() { tr("The server didn’t open it.").to_owned() } else { words };
    let word = crate::desk::title(tr("Try again")).into_owned();
    format!(
        "start_col := View{{width: Fill height: Fit flow: Down align: Align{{x: 0.5}} \
         padding: Inset{{left: 24 right: 24}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}}\n\
         View{{width: 36 height: 36 flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n\
         {mark}}}\n}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: 6}}\n\
         {title}}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: 6}}\n\
         {hint}}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: 10}}\n\
         new_chat_retry_wrap := View{{width: {w} height: 28 flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n\
         RoundedView{{width: Fill height: Fill draw_bg +: {{color: #0071e3ff border_radius: 3.0}}}}\n\
         {l}{h}}}\n}}\n}}\n",
        mark = svg("start_mark", "chat_terminal.svg", 36.0, MUTED),
        title = label(
            "start_title",
            tr("Couldn’t Start a New Chat"),
            &style(Face::SemiBold, 15.0, 22.0),
            INK,
            "width: Fill height: Fit max_width: 520 align: Align{x: 0.5} flow: Right{wrap: true}",
        ),
        hint = label(
            "start_hint",
            &words,
            &style(Face::Regular, 13.0, 18.0),
            MUTED,
            "width: Fill height: Fit max_width: 480 align: Align{x: 0.5} flow: Right{wrap: true}",
        ),
        w = (word.chars().count() as f64 * 7.0 + 28.0).max(96.0).ceil(),
        l = label("new_chat_retry_label", &word, &style(Face::Regular, 13.0, 18.0), "#fffffeff", "width: Fit height: Fit"),
        h = hit_on_blue("new_chat_retry", 3.0),
    )
}

/// What the first-run Connect card draws (A1). Built from
/// `screens::connect::ConnectUi`; the field TEXTS are only the initial
/// values — the inputs own their text afterwards (typing never remounts).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnectView {
    pub server: String,
    /// The classified failure message or the raw error of the last attempt.
    pub error: String,
    /// The failure's next steps ("Re-enter the token · Retry").
    pub error_actions: String,
    /// "Last tried 9:41 PM ·" when an attempt failed.
    pub last_tried: String,
    pub connecting: bool,
    /// A9: the handshake ambiguity's honest second half, when the failure
    /// probe could not tell (`ConnectionPanel.tsx:284-292`); else empty.
    pub hint: String,
}

/// Board 4 frame 4 — the first-run "Connect to Octos" card, centered in the
/// pane to the right of the sidebar (`left` = the sidebar's width, 0 on the
/// phone): 480 px max, Server and Access token fields, the storage note, a
/// full-width black Connect pill and the blue "Use local solo server" link
/// (a real control — the lowered setup-01 card drew it as a plain label).
/// The web's `ConnectionPanel.module.css` shape: one bordered card, labels
/// above 40 px fields, the error as a tinted callout under the title.
///
/// Ids the host routes: `connect_server` / `connect_token` (inputs →
/// `input.server` / `input.token`), `connect_btn` (→ `connect`),
/// `connect_solo` (→ `connect.use_local_solo`), `connect_server_error`
/// (the live validation line, set by the host without a remount), and A2's
/// `b1_connect_pair` (→ `b1.open.pairing`, routed by `board1::collect`).
pub fn connect_card(c: &ConnectView, m: &Metrics, left: f64) -> String {
    connect_card_with_offer(c, m, left, None)
}

/// [`connect_card`] carrying A11's discovery offer (`screens::discovery`):
/// under the title (and any error), the web's place for it
/// (`ConnectionPanel.tsx:172-194`): board 1's neutral callout (the info icon
/// of p4-04/p4-09) saying "Found Octos on <host>." with ONE outline pill
/// "Connect to <host>" (board 1's secondary pill, p4-02/p4-09). A status,
/// never an error. Ids: `connect_offer_box` (the callout the host hides in
/// place once the offer is dismissed), `connect_offer_text`,
/// `connect_offer_label` and the hit `connect_offer` (→ `b1.open.discovered`,
/// routed by `board1::collect`).
pub fn connect_card_with_offer(
    c: &ConnectView,
    m: &Metrics,
    left: f64,
    offer: Option<&crate::screens::discovery::Offer>,
) -> String {
    if crate::desk::active() {
        return connect_form(c, left, offer);
    }
    let phone = m.density == Density::Phone;
    let (pad_x, pad_top) = if phone { (20.0, 24.0) } else { (32.0, 28.0) };
    let field = |id: &str, text: &str, placeholder: &str, password: bool| {
        // The token field carries the board's eye control (board 4 frame 4;
        // the web's Show/Hide, ConnectionPanel.tsx:262-270): two icons, the
        // host shows one and flips the input's masking on a click.
        let eye = if password {
            // `Svg` has no `visible` property (the DSL refused to evaluate
            // and the whole card stayed unmounted): each icon rides a View
            // the host shows or hides.
            format!(
                "connect_eye_wrap := View{{width: 28 height: 28 flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n\
                 connect_eye_show := View{{width: 16 height: 16 flow: Overlay\n{show}}}\n\
                 connect_eye_hide := View{{width: 16 height: 16 flow: Overlay visible: false\n{hide}}}\n\
                 {hit}}}\n",
                show = svg("connect_eye_show_icon", "eye.svg", 16.0, MUTED),
                hide = svg("connect_eye_hide_icon", "eye_off.svg", 16.0, MUTED),
                hit = hit("connect_eye", 6.0),
            )
        } else {
            String::new()
        };
        format!(
            "RoundedView{{width: Fill height: 42 flow: Right align: Align{{y: 0.5}} spacing: 6 \
             padding: Inset{{left: 12 right: {pr}}} \
             draw_bg +: {{color: {SURFACE} border_radius: 4.0 border_size: 1.0 border_color: #d2d2d5ff}}\n\
             {id} := DesignInput{{width: Fill height: Fit padding: 0 margin: 0 text: {text:?} \
             empty_text: {placeholder:?} is_password: {password}\n\
             draw_text +: {{color: {INK} color_empty: {MUTED} text_style: {st} \
             get_color: fn() {{return mix(self.color, self.color_empty, self.empty)}}}}\n\
             draw_cursor +: {{color: {INK}}}\n\
             draw_selection +: {{color: #2f6feb33 color_hover: #2f6feb33 color_focus: #2f6feb40 \
             color_down: #2f6feb40 color_empty: #2f6feb33 color_disabled: #2f6feb33}}\n\
             }}\n{eye}}}\n",
            pr = if password { 7.0 } else { 12.0 },
            st = style(Face::Regular, 14.0, 20.0),
        )
    };
    let caption = |text: &str| {
        label(
            "",
            text,
            &style(Face::Medium, 13.0, 18.0),
            INK,
            "width: Fit height: Fit margin: Inset{bottom: 6}",
        )
    };
    let error = if c.error.is_empty() {
        String::new()
    } else {
        // A9: "Last tried 8:54 PM · check the address · Retry" (the time
        // ran into the actions without a separator).
        let detail = [c.last_tried.as_str(), c.error_actions.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" · ");
        format!(
            "connect_error := RoundedView{{width: Fill height: Fit flow: Down spacing: 2 margin: Inset{{top: 14}} \
             padding: Inset{{left: 12 right: 12 top: 10 bottom: 10}} \
             draw_bg +: {{color: #fdececff border_radius: 4.0}}\n\
             {msg}{hint}{detail}}}\n",
            hint = if c.hint.is_empty() {
                String::new()
            } else {
                label(
                    "connect_error_hint",
                    &c.hint,
                    &style(Face::Regular, 13.0, 19.0),
                    INK,
                    "width: Fill height: Fit flow: Right{wrap: true}",
                )
            },
            msg = label(
                "connect_error_text",
                &c.error,
                &style(Face::Medium, 13.0, 19.0),
                RED,
                "width: Fill height: Fit flow: Right{wrap: true}",
            ),
            detail = if detail.is_empty() {
                String::new()
            } else {
                label(
                    "connect_error_detail",
                    &detail,
                    &style(Face::Regular, 12.0, 18.0),
                    MUTED,
                    "width: Fill height: Fit flow: Right{wrap: true}",
                )
            },
        )
    };
    // A11 — the discovery offer (board 1's neutral callout + outline pill).
    let offer = match offer {
        None => String::new(),
        Some(o) => format!(
            "connect_offer_box := RoundedView{{width: Fill height: Fit flow: Right spacing: 10 margin: Inset{{top: 14}} \
             padding: Inset{{left: 12 right: 14 top: 12 bottom: 12}} \
             draw_bg +: {{color: {TIP} border_radius: 6.0 border_size: 1.0 border_color: {BORDER}}}\n\
             View{{width: 20 height: 20 align: Align{{x: 0.5 y: 0.5}}\n{icon}}}\n\
             View{{width: Fill height: Fit flow: Down spacing: 10\n\
             {msg}\
             connect_offer_wrap := View{{width: Fit height: 32 flow: Overlay\n\
             RoundedView{{width: Fit height: 32 flow: Right align: Align{{x: 0.5 y: 0.5}} \
             padding: Inset{{left: 14 right: 14}} \
             draw_bg +: {{color: {SURFACE} border_radius: 16.0 border_size: 1.0 border_color: {INK}}}\n\
             {btn}}}\n\
             {hit}}}\n}}\n}}\n",
            icon = svg("connect_offer_icon", "b1_info.svg", 18.0, INK),
            msg = label(
                "connect_offer_text",
                &o.message(),
                &style(Face::Regular, 14.0, 20.0),
                INK,
                "width: Fill height: Fit flow: Right{wrap: true}",
            ),
            btn = label(
                "connect_offer_label",
                &o.action(),
                &style(Face::Medium, 13.0, 18.0),
                INK,
                "width: Fit height: Fit",
            ),
            hit = hit("connect_offer", 16.0),
        ),
    };
    format!(
        "connect_center := View{{width: Fill height: Fill flow: Down align: Align{{x: 0.5 y: 0.42}} \
         padding: Inset{{left: {lp} right: {rp} top: 16 bottom: 16}}\n\
         connect_card := RoundedView{{width: Fill max_width: 480 height: Fit flow: Down \
         padding: Inset{{left: {pad_x} right: {pad_x} top: {pad_top} bottom: 22}}\n\
         draw_bg +: {{color: {SURFACE} border_radius: 6.0 border_size: 1.0 border_color: {BORDER}}}\n\
         {title}\
         {error}\
         {offer}\
         View{{width: Fill height: 18}}\n\
         {server_cap}{server_field}\
         {server_err}\
         View{{width: Fill height: 14}}\n\
         {token_cap}{token_field}\
         View{{width: Fill height: Fit padding: Inset{{top: 8}}\n{note}}}\n\
         connect_btn_wrap := View{{width: Fill height: 44 flow: Overlay align: Align{{x: 0.5 y: 0.5}} margin: Inset{{top: 22}}\n\
         RoundedView{{width: Fill height: Fill draw_bg +: {{color: #000000ff border_radius: 22.0}}}}\n\
         {btn_label}\
         {btn_hit}}}\n\
         View{{width: Fill height: Fit flow: Down align: Align{{x: 0.5}} padding: Inset{{top: 12}} spacing: 2\n\
         connect_pair_wrap := View{{width: Fit height: 32 flow: Overlay align: Align{{x: 0.5 y: 0.5}} \
         padding: Inset{{left: 8 right: 8}}\n\
         {pair_label}{pair_hit}}}\n\
         connect_solo_wrap := View{{width: Fit height: 32 flow: Overlay align: Align{{x: 0.5 y: 0.5}} \
         padding: Inset{{left: 8 right: 8}}\n\
         {solo_label}{solo_hit}}}\n}}\n\
         }}\n}}\n",
        lp = left + 16.0,
        rp = 16.0,
        title = label(
            "connect_title",
            tr("Connect to Octos"),
            &style(Face::SemiBold, if phone { 20.0 } else { 19.0 }, 26.0),
            INK,
            "width: Fit height: Fit",
        ),
        server_cap = caption(tr("Server")),
        server_field = field("connect_server", &c.server, "http://127.0.0.1:50190", false),
        server_err = label(
            "connect_server_error",
            "",
            &style(Face::Regular, 12.0, 18.0),
            RED,
            "width: Fill height: Fit margin: Inset{top: 4} flow: Right{wrap: true}",
        ),
        token_cap = caption(tr("Access token")),
        token_field = field("connect_token", "", tr("Paste your server token"), true),
        note = label(
            "connect_note",
            tr("Stored for this server only"),
            &style(Face::Regular, 12.0, 18.0),
            MUTED,
            "width: Fit height: Fit",
        ),
        btn_label = label(
            "connect_btn_label",
            tr(if c.connecting { "Connecting…" } else { "Connect" }),
            &style(Face::Medium, 15.0, 20.0),
            "#ffffffff",
            "width: Fit height: Fit",
        ),
        btn_hit = hit("connect_btn", 22.0),
        solo_label = label(
            "connect_solo_label",
            tr("Use local solo server"),
            &style(Face::Regular, 14.0, 20.0),
            "#2f6febff",
            "width: Fit height: Fit",
        ),
        solo_hit = hit("connect_solo", 6.0),
        // #A2 board 1: the Connect screen's way into pairing (p4-01). The id is
        // routed by `screens::board1::entry_controls` -> `b1.open.pairing`.
        pair_label = label(
            "connect_pair_label",
            tr("Pair with a link instead"),
            &style(Face::Regular, 14.0, 20.0),
            "#2f6febff",
            "width: Fit height: Fit",
        ),
        pair_hit = hit("b1_connect_pair", 6.0),
    )
}

/// A Mac window before it has a chat: the toolbar's band (the app's name on it
/// when the window has no title bar) with `pane` under it, and `over` (a sheet
/// and its dim) above both. The host and the walks find it by `connect_center`.
fn first_run_window(pane: &str, over: &str) -> String {
    let t = crate::desk::current();
    let name = if crate::desk::unified() {
        label("first_run_title", "OctosCode", &style(Face::SemiBold, 15.0, 20.0), INK, "width: Fit height: Fit")
    } else {
        String::new()
    };
    format!(
        "connect_center := View{{width: Fill height: Fill flow: Overlay\n\
         View{{width: Fill height: Fill flow: Down\n\
         RoundedView{{width: Fill height: {bar} flow: Right align: Align{{y: 0.5}} \
         padding: Inset{{left: {lead} right: 16}} draw_bg +: {{color: {band} border_radius: 0.0}}\n{name}}}\n\
         RoundedView{{width: Fill height: 1 draw_bg +: {{color: {edge} border_radius: 0.0}}}}\n\
         {pane}}}\n\
         {over}}}\n",
        bar = crate::desk::bar_h(),
        lead = crate::desk::bar_lead(0.0),
        band = t.bar,
        edge = t.border,
    )
}

/// What lies over [`first_run_window`] while it alerts: the shared dim, and
/// `body` on a Mac alert's 260-point panel (16 points at its sides and foot,
/// 32 over the app's icon at its top), hanging from the toolbar's foot, centred.
fn first_run_alert(id: &str, body: &str) -> String {
    format!(
        "RoundedView{{width: Fill height: Fill draw_bg +: {{color: {dim} border_radius: 0.0}}}}\n\
         View{{width: Fill height: Fill flow: Down align: Align{{x: 0.5 y: 0.0}} padding: Inset{{left: 16 right: 16 top: {top} bottom: 16}}\n\
         {id} := OcMacSheet{{width: {w} height: Fit flow: Down align: Align{{x: 0.5 y: 0.0}} padding: Inset{{left: 16 right: 16 top: 32 bottom: 16}}\n draw_bg.color: {SURFACE} draw_bg.border_color: {BORDER}\n\
         {body}}}\n}}\n",
        dim = crate::desk::sheet_dim(),
        top = crate::desk::bar_h(),
        w = crate::screens::board3::ui::ALERT_W,
    )
}

/// Quiet context behind a first-run sheet. It has no controls or secret fields;
/// the sheet's dim and input shield remain responsible for modal behavior.
pub fn first_run_backdrop(ask_workspace: bool) -> String {
    let title = if ask_workspace { tr("No Workspace Open") } else { tr("Connect to Octos") };
    let pane = format!(
        "View{{width: Fill height: Fill flow: Down spacing: 8 align: Align{{x: 0.5 y: 0.55}} \
         padding: Inset{{left: 24 right: 24 top: 16 bottom: 16}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}}\n{mark}}}\n\
         {title}}}\n",
        mark = svg("first_run_context_mark", "chat_terminal.svg", 36.0, MUTED),
        title = label(
            "first_run_context_title", title, &style(Face::SemiBold, 15.0, 20.0), INK,
            "width: Fill height: Fit align: Align{x: 0.5} flow: Right{wrap: true}",
        ),
    );
    crate::desk::in_words(first_run_window(&pane, ""))
}

/// n11o (E9 3): what lies behind a first-run alert: the window's own quiet
/// state (the app's mark and a title, nothing to press), as behind a sheet.
fn quiet_pane(title: &str) -> String {
    format!(
        "View{{width: Fill height: Fill flow: Down spacing: 8 align: Align{{x: 0.5 y: 0.55}} \
         padding: Inset{{left: 24 right: 24 top: 16 bottom: 16}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}}\n{mark}}}\n\
         {title}}}\n",
        mark = svg("first_run_context_mark", "chat_terminal.svg", 36.0, MUTED),
        title = label(
            "first_run_context_title", title, &style(Face::SemiBold, 15.0, 20.0), INK,
            "width: Fill height: Fit align: Align{x: 0.5} flow: Right{wrap: true}",
        ),
    )
}

/// A desktop's Connect screen: a form on the content pane, the shape of the
/// Mac's own "Connect to Server" (a title, labels beside 28-point fields, the
/// default action at the trailing edge), with every id the card's host routes.
fn connect_form(c: &ConnectView, _left: f64, offer: Option<&crate::screens::discovery::Offer>) -> String {
    // A Mac's placeholder is the system's (E7 5).
    let ph = crate::desk::placeholder();
    const LABEL_W: f64 = 92.0;
    let inset = LABEL_W + 10.0;
    let field = |id: &str, text: &str, placeholder: &str, password: bool| {
        let eye = if password {
            format!(
                "connect_eye_wrap := View{{width: 28 height: 28 flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n\
                 connect_eye_show := View{{width: 14 height: 14 flow: Overlay\n{show}}}\n\
                 connect_eye_hide := View{{width: 14 height: 14 flow: Overlay visible: false\n{hide}}}\n\
                 {hit}}}\n",
                show = svg("connect_eye_show_icon", "eye.svg", 14.0, MUTED),
                hide = svg("connect_eye_hide_icon", "eye_off.svg", 14.0, MUTED),
                hit = hit("connect_eye", 3.0),
            )
        } else {
            String::new()
        };
        format!(
            "RoundedView{{width: Fill height: 28 flow: Right align: Align{{y: 0.5}} spacing: 4 \
             padding: Inset{{left: 8 right: {pr}}} \
             draw_bg +: {{color: {SURFACE} border_radius: 3.0 border_size: 1.0 border_color: #d2d2d5ff}}\n\
             {id} := DesignInput{{width: Fill height: Fit padding: 0 margin: 0 text: {text:?} \
             empty_text: {placeholder:?} is_password: {password}\n\
             draw_text +: {{color: {INK} color_empty: {ph} text_style: {st} \
             get_color: fn() {{return mix(self.color, self.color_empty, self.empty)}}}}\n\
             draw_cursor +: {{color: {INK}}}\n\
             draw_selection +: {{color: #2f6feb33 color_hover: #2f6feb33 color_focus: #2f6feb40 \
             color_down: #2f6feb40 color_empty: #2f6feb33 color_disabled: #2f6feb33}}\n\
             }}\n{eye}}}\n",
            pr = if password { 2.0 } else { 8.0 },
            st = style(Face::Regular, 13.0, 18.0),
        )
    };
    let row = |caption: &str, control: String| {
        format!(
            "View{{width: Fill height: 28 flow: Right align: Align{{y: 0.5}} spacing: 10\n{l}{control}}}\n",
            l = label(
                "",
                caption,
                &style(Face::Regular, 13.0, 18.0),
                INK,
                &format!("width: {LABEL_W} height: Fit align: Align{{x: 1.0}}"),
            ),
        )
    };
    // Under the centred title, its words centred too (E7 5).
    let error = if c.error.is_empty() {
        String::new()
    } else {
        let detail = [c.last_tried.as_str(), c.error_actions.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" · ")
            .replace(
                " · check the address · Retry",
                &if c.server.trim_end_matches('/') == SOLO_SERVER {
                    // Use Local Solo Server's own address: there's no address to check.
                    format!(". {}", tr("Make sure Octos is running on this Mac."))
                } else {
                    ". Check the address, then click Connect.".to_owned()
                },
            )
            .replace(" · Re-enter the token · Retry", ". Enter the token again, then click Connect.");
        format!(
            "connect_error := View{{width: Fill height: Fit flow: Down spacing: 2 margin: Inset{{top: 12}}\n\
             {msg}{hint}{detail}}}\n",
            msg = label(
                "connect_error_text",
                &c.error,
                &style(Face::Medium, 13.0, 18.0),
                RED,
                "width: Fill height: Fit flow: Right{wrap: true} align: Align{x: 0.5}",
            ),
            hint = if c.hint.is_empty() {
                String::new()
            } else {
                label(
                    "connect_error_hint",
                    &c.hint,
                    &style(Face::Regular, 12.0, 18.0),
                    INK,
                    "width: Fill height: Fit flow: Right{wrap: true} align: Align{x: 0.5}",
                )
            },
            detail = if detail.is_empty() {
                String::new()
            } else {
                label(
                    "connect_error_detail",
                    &detail,
                    &style(Face::Regular, 12.0, 18.0),
                    MUTED,
                    "width: Fill height: Fit flow: Right{wrap: true} align: Align{x: 0.5}",
                )
            },
        )
    };
    let offer = match offer {
        None => String::new(),
        Some(o) => format!(
            "connect_offer_box := RoundedView{{width: Fill height: Fit flow: Right spacing: 8 align: Align{{y: 0.5}} margin: Inset{{top: 12}} \
             padding: Inset{{left: 10 right: 8 top: 7 bottom: 7}} \
             draw_bg +: {{color: {TIP} border_radius: 3.0 border_size: 1.0 border_color: {BORDER}}}\n\
             {icon}\
             View{{width: Fill height: Fit flow: Down\n{msg}}}\n\
             connect_offer_wrap := View{{width: Fit height: 28 flow: Overlay\n\
             RoundedView{{width: Fit height: 28 flow: Right align: Align{{x: 0.5 y: 0.5}} \
             padding: Inset{{left: 12 right: 12}} \
             draw_bg +: {{color: {SURFACE} border_radius: 3.0 border_size: 1.0 border_color: {BORDER}}}\n\
             {btn}}}\n\
             {hit}}}\n}}\n",
            icon = svg("connect_offer_icon", "b1_info.svg", 16.0, INK),
            msg = label(
                "connect_offer_text",
                &o.message(),
                &style(Face::Regular, 13.0, 18.0),
                INK,
                "width: Fill height: Fit flow: Right{wrap: true}",
            ),
            btn = label(
                "connect_offer_label",
                &o.action(),
                &style(Face::Regular, 13.0, 18.0),
                INK,
                "width: Fit height: Fit",
            ),
            hit = hit("connect_offer", 3.0),
        ),
    };
    // The other ways in are push buttons beside the default.
    let link = |wrap: &str, id: &str, text: &str, hit_id: &str| {
        use crate::screens::board3::ui as kit;
        let t = crate::desk::current();
        format!(
            "{wrap} := View{{width: {w} height: 28 flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n\
             RoundedView{{width: Fill height: Fill draw_bg +: {{color: {face} border_radius: 3.0 border_size: 1.0 border_color: {edge}}}}}\n\
             {l}{h}}}\n",
            w = (kit::text_w(text, 13.0, kit::Face::Regular) + 20.0).ceil(),
            face = t.button,
            edge = t.border,
            l = label(id, text, &style(Face::Regular, 13.0, 18.0), INK, "width: Fit height: Fit"),
            h = hit(hit_id, 3.0),
        )
    };
    let body = format!(
        "{title}\
         {error}\
         {offer}\
         View{{width: Fill height: 16}}\n\
         {server}\
         View{{width: Fill height: Fit padding: Inset{{left: {inset}}}\n{server_err}}}\n\
         View{{width: Fill height: 8}}\n\
         {token}\
         View{{width: Fill height: Fit padding: Inset{{left: {inset} top: 4}}\n{note}}}\n\
         View{{width: Fill height: 16}}\n\
         View{{width: Fill height: 28 flow: Right align: Align{{y: 0.5}} spacing: 6\n\
         {pair}{solo}\
         View{{width: Fill height: 1}}\n\
         connect_btn_wrap := View{{width: 96 height: 28 flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n\
         connect_btn_on := RoundedView{{width: Fill height: Fill draw_bg +: {{color: #0071e3ff border_radius: 3.0}}}}\n\
         connect_btn_off := RoundedView{{width: Fill height: Fill visible: false draw_bg +: {{color: {off_face} border_radius: 3.0 border_size: 1.0 border_color: {off_edge}}}}}\n\
         {btn_label}\
         {btn_label_off}\
         {btn_hit}}}\n\
         }}\n",
        off_face = crate::desk::current().button,
        off_edge = crate::desk::current().border,
        btn_label_off = label(
            "connect_btn_label_off",
            tr("Connect"),
            &style(Face::Regular, 13.0, 18.0),
            crate::screens::board3::ui::tok::DISABLED_INK,
            "width: Fit height: Fit visible: false",
        ),
        title = label(
            "connect_title",
            tr("Connect to Octos"),
            &style(Face::SemiBold, 15.0, 22.0),
            INK,
            "width: Fill height: Fit align: Align{x: 0.5}",
        ),
        server = row(tr("Server"), field("connect_server", &c.server, "http://127.0.0.1:50190", false)),
        server_err = label(
            "connect_server_error",
            "",
            &style(Face::Regular, 11.0, 16.0),
            RED,
            "width: Fill height: Fit margin: Inset{top: 2} flow: Right{wrap: true}",
        ),
        token = row(tr("Access token"), field("connect_token", "", tr("Paste your server token"), true)),
        note = label(
            "connect_note",
            tr("Stored for this server only"),
            &style(Face::Regular, 11.0, 16.0),
            MUTED,
            "width: Fit height: Fit",
        ),
        pair = link("connect_pair_wrap", "connect_pair_label", &crate::desk::title(tr("Pair with a link instead")), "b1_connect_pair"),
        solo = link("connect_solo_wrap", "connect_solo_label", &crate::desk::title(tr("Use local solo server")), "connect_solo"),
        btn_label = label(
            "connect_btn_label",
            tr(if c.connecting { "Connecting…" } else { "Connect" }),
            &style(Face::Regular, 13.0, 18.0),
            "#fffffeff",
            "width: Fit height: Fit",
        ),
        btn_hit = hit_on_blue("connect_btn", 3.0),
    );
    // The first connection is the welcome window's content: it has no parent
    // document to dim and its actions remain in the same place as the fields.
    let pane = format!(
        "View{{width: Fill height: Fill flow: Down align: Align{{x: 0.5 y: 0.42}} \
         padding: Inset{{left: 24 right: 24 top: 16 bottom: 16}}\n\
         connect_card := View{{width: Fill max_width: 480 height: Fit flow: Down\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{bottom: 14}}\n{mark}}}\n\
         {body}}}\n}}\n",
        mark = svg("first_run_mark", "chat_terminal.svg", 36.0, MUTED),
    );
    first_run_window(&pane, "")
}

/// The local solo server's address (`connect.use_local_solo`).
const SOLO_SERVER: &str = "http://127.0.0.1:50190";

/// What a desktop's window says under its toolbar when the app is connected (or
/// hosted by a shell, which owns the connection) with no session open: what to
/// do, and the action. `busy`: the connection is still coming up. A failure, or
/// a kernel with no model provider, is an alert on a sheet over the window.
pub fn open_workspace_state(left: f64, busy: bool, failed: bool, no_provider: bool, error: &str) -> String {
    let title_words = if failed {
        tr("Couldn't Connect").to_owned()
    } else if busy {
        tr("Connecting…").to_owned()
    } else {
        if no_provider { tr("No Model Provider") } else { tr("No Workspace Open") }.to_owned()
    };
    let hint_words = if failed && crate::desk::hosted() {
        // n11o (E9 3): what failed, and the way on (Try Again, the default):
        // a shell's window talks only to the Octos on this Mac, never to an address.
        tr("Octos on this Mac isn\u{2019}t answering, so OctosCode can\u{2019}t connect to it. Try again in a moment.")
    } else if failed {
        // One line in the alert's 228 points: "now." ran on alone.
        tr("OctosCode can't reach its assistant.")
    } else if busy {
        ""
    } else if no_provider {
        // A Mac's title already said it.
        tr(crate::desk::by(crate::screens::board1::NO_PROVIDER_NOTE, "Add one in AI Providers, then try again."))
    } else {
        tr("Choose a folder, and OctosCode starts a session in it.")
    };
    // A push button, `w` wide (Fill or points); the default is blue.
    let button = |wrap: &str, w: &str, word: &str, label_id: &str, hit_id: &str, default: bool| {
        let t = crate::desk::current();
        let face = if default {
            "RoundedView{width: Fill height: Fill draw_bg +: {color: #0071e3ff border_radius: 3.0}}\n".to_owned()
        } else {
            format!(
                "RoundedView{{width: Fill height: Fill draw_bg +: {{color: {} border_radius: 3.0 border_size: 1.0 border_color: {}}}}}\n",
                t.button, t.border
            )
        };
        let head = if wrap.is_empty() { String::new() } else { format!("{wrap} := ") };
        format!(
            "{head}View{{width: {w} height: 28 flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n{face}{l}{h}}}\n",
            l = label(
                label_id,
                word,
                &style(Face::Regular, 13.0, 18.0),
                if default { "#fffffeff" } else { INK },
                "width: Fit height: Fit",
            ),
            h = if default { hit_on_blue(hit_id, 3.0) } else { hit(hit_id, 3.0) },
        )
    };
    if !busy && (failed || no_provider) {
        // A Mac's alert (E7 1), as the native apps draw theirs: the app's icon on
        // top, what went wrong in bold and the words centred under it, the way on
        // as its default.
        let again = crate::desk::title(tr("Try again")).into_owned();
        // (wrap id, word, label id, press id, default), in reading order, the default last.
        let answers: Vec<(&str, String, &str, &str, bool)> = if no_provider && !failed && crate::desk::hosted() {
            // A shell's window keeps its providers in AI Providers: the way there,
            // and a way to look again on coming back.
            vec![
                ("", again.clone(), "open_ws_again_label", "b1_open_workspace", false),
                ("", crate::desk::title(tr("Open AI Providers\u{2026}")).into_owned(), "open_ws_label", "open_ws_providers", true),
            ]
        } else if failed && crate::desk::hosted() {
            // A shell's window talks only to the shell's kernel: no other server to choose,
            // so the way out is closing the window, beside the default Try Again.
            vec![
                ("", crate::desk::title(tr("Close")).into_owned(), "open_ws_close_label", "open_ws_close", false),
                ("open_ws_wrap", again.clone(), "open_ws_label", "open_ws_retry", true),
            ]
        } else {
            vec![("open_ws_wrap", again.clone(), "open_ws_label", if failed { "open_ws_retry" } else { "b1_open_workspace" }, true)]
        };
        // Two side by side and as wide as each other when their words fit, the
        // default on the right; otherwise stacked, the default on top.
        let half = (crate::screens::board3::ui::ALERT_INNER - 8.0) / 2.0;
        let beside = answers.len() <= 2
            && answers.iter().all(|(_, word, ..)| {
                crate::screens::board3::ui::text_w(word, 13.0, crate::screens::board3::ui::Face::Medium) + 24.0 <= half
            });
        let mut row = String::new();
        let order: Vec<&(&str, String, &str, &str, bool)> =
            if beside { answers.iter().collect() } else { answers.iter().rev().collect() };
        for (wrap, word, label_id, hit_id, default) in order {
            row.push_str(&button(wrap, "Fill", word, label_id, hit_id, *default));
        }
        let buttons = format!(
            "View{{width: Fill height: Fit flow: {} spacing: 8 margin: Inset{{top: 16}}\n{row}}}\n",
            if beside { "Right" } else { "Down" }
        );
        // What the system said, under the words.
        let error = if error.is_empty() {
            String::new()
        } else {
            format!(
                "View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: 6}}\n{}}}\n",
                label(
                    "open_ws_error",
                    error,
                    &style(Face::Regular, 11.0, 14.0),
                    RED,
                    "width: Fill height: Fit align: Align{x: 0.5} flow: Right{wrap: true}",
                ),
            )
        };
        let body = format!(
            "open_ws_icon := AppIcon{{name: \"octoscode\" width: 64 height: 64}}\n\
             View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: 10}}\n{title}}}\n\
             View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: 5}}\n{hint}}}\n\
             {error}{buttons}",
            title = label(
                "open_ws_title",
                &title_words,
                &style(Face::SemiBold, 13.0, 17.0),
                INK,
                "width: Fill height: Fit align: Align{x: 0.5} flow: Right{wrap: true}",
            ),
            hint = label(
                "open_ws_hint",
                hint_words,
                &style(Face::Regular, 11.0, 14.0),
                INK,
                "width: Fill height: Fit align: Align{x: 0.5} flow: Right{wrap: true}",
            ),
        );
        let sheet = first_run_alert("open_ws_alert", &body);
        // n11o (E9 3): the window keeps its own quiet state behind the alert,
        // never an empty grey pane.
        let behind = if failed {
            crate::desk::title(tr("Not connected to Octos")).into_owned()
        } else {
            tr("No Workspace Open").to_owned()
        };
        return crate::desk::in_words(first_run_window(&quiet_pane(&behind), &sheet));
    }
    let error = if error.is_empty() {
        String::new()
    } else {
        format!(
            "View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: 10}}\n{}}}\n",
            label(
                "open_ws_error",
                error,
                &style(Face::Regular, 12.0, 18.0),
                RED,
                "width: Fill height: Fit max_width: 420 align: Align{x: 0.5} flow: Right{wrap: true}",
            ),
        )
    };
    // One default action; while the connection is on its way, Cancel (the
    // shell's window has no other server: it closes, as the failure's Close).
    let action = if busy {
        let word = crate::desk::title(tr("Cancel")).into_owned();
        let w = (word.chars().count() as f64 * 7.0 + 28.0).max(96.0).ceil();
        format!(
            "View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: 10}}\n{}}}\n",
            button("", &format!("{w}"), &word, "open_ws_cancel_label", "open_ws_cancel", false),
        )
    } else {
        let word = crate::desk::title(tr("Open a workspace\u{2026}")).into_owned();
        let w = (word.chars().count() as f64 * 7.0 + 28.0).max(96.0).ceil();
        format!(
            "View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: 10}}\n{}}}\n",
            button("open_ws_wrap", &format!("{w}"), &word, "open_ws_label", "b1_open_workspace", true),
        )
    };
    let pane = format!(
        "View{{width: Fill height: Fill flow: Down spacing: 6 align: Align{{x: 0.5 y: 0.42}} \
         padding: Inset{{left: {lp} right: 24 top: 24 bottom: 24}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}}\n{mark}}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}}\n{title}}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}}\n{hint}}}\n\
         {error}{action}\
         }}\n",
        lp = left + 24.0,
        mark = if busy {
            spinner("open_ws_mark", 20.0)
        } else {
            svg("open_ws_mark", "chat_terminal.svg", 36.0, MUTED)
        },
        title = label(
            "open_ws_title",
            &title_words,
            &style(Face::SemiBold, 15.0, 20.0),
            INK,
            "width: Fill height: Fit max_width: 420 align: Align{x: 0.5} flow: Right{wrap: true}",
        ),
        hint = label(
            "open_ws_hint",
            hint_words,
            &style(Face::Regular, 13.0, 18.0),
            MUTED,
            "width: Fill height: Fit max_width: 420 align: Align{x: 0.5} flow: Right{wrap: true}",
        ),
    );
    crate::desk::in_words(first_run_window(&pane, ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Integration of A1 (native connect card) with A2 (board 1): the Connect
    /// screen keeps its way into pairing, as a Button routed by id through
    /// `screens::board1::entry_controls` (`b1_connect_pair` -> `b1.open.pairing`).
    #[test]
    fn the_connect_card_carries_the_pairing_link() {
        let card = connect_card(&ConnectView::default(), &crate::conv_layout::Metrics::for_window(990.0, true), 0.0);
        assert!(card.contains("b1_connect_pair := Button{"), "pairing hit is a Button");
        assert!(card.contains("Pair with a link instead"), "pairing link label");
        let entries = crate::screens::board1::entry_controls();
        assert!(entries.iter().any(|(id, a)| id == "b1_connect_pair" && a == "b1.open.pairing"));
    }

    /// A11 — the discovery offer rides the same card only while offered, in
    /// the web's place (under the title, above the fields), with ONE button
    /// routed by id to `b1.open.discovered`.
    #[test]
    fn the_connect_card_carries_the_discovery_offer_only_when_offered() {
        let m = crate::conv_layout::Metrics::for_window(990.0, true);
        let plain = connect_card(&ConnectView::default(), &m, 0.0);
        assert!(!plain.contains("connect_offer"), "no offer without discovery");
        let o = crate::screens::discovery::Offer {
            origin: "http://127.0.0.1:8434".into(),
            label: "127.0.0.1:8434".into(),
            pairing_required: true,
        };
        let card = connect_card_with_offer(&ConnectView::default(), &m, 0.0, Some(&o));
        assert!(card.contains("connect_offer := Button{"), "the offer's hit is a Button");
        assert_eq!(card.matches("Found Octos on 127.0.0.1:8434.").count(), 1, "offered once");
        assert_eq!(card.matches("Connect to 127.0.0.1:8434").count(), 1);
        let at = |s: &str| card.find(s).unwrap_or_else(|| panic!("{s} missing"));
        assert!(at("Connect to Octos") < at("Found Octos on") && at("Found Octos on") < at("connect_server :="));
        let entries = crate::screens::board1::entry_controls();
        assert!(entries.iter().any(|(id, a)| id == "connect_offer" && a == "b1.open.discovered"));
    }
    use crate::conv_layout::Metrics;

    fn desk() -> Metrics {
        Metrics::for_window(990.0, true)
    }

    #[test]
    fn tool_kinds_follow_the_web_table() {
        // tool-kind.test.ts:4-33
        assert_eq!(tool_kind("shell"), "shell");
        assert_eq!(tool_kind("run_command"), "shell");
        assert_eq!(tool_kind("read_file"), "read");
        assert_eq!(tool_kind("fs.readFile"), "read");
        assert_eq!(tool_kind("list_dir"), "read");
        assert_eq!(tool_kind("apply_patch"), "edit");
        assert_eq!(tool_kind("grep"), "search");
        assert_eq!(tool_kind("web_fetch"), "web");
        assert_eq!(tool_kind("thread"), "generic", "whole words only");
        assert_eq!(tool_kind(""), "generic");
    }

    #[test]
    fn the_target_is_the_web_keys_first_line_truncated() {
        assert_eq!(tool_target(r#"{"path":"README.md"}"#), "README.md");
        assert_eq!(tool_target(r#"{"command":"cargo test\n--all"}"#), "cargo test");
        assert_eq!(
            tool_target(r#"{"path":"crates/octos-cli/src/api/ui_protocol_transport.rs"}"#),
            "crates/octos-cli/src/api…"
        );
        assert_eq!(tool_target("not json"), "");
        assert_eq!(tool_target(r#"{"other":1}"#), "");
        // The live server's text preview (a6ea8505): `key: "value"`.
        assert_eq!(preview_target(r#"path: "README.md""#), "README.md");
        assert_eq!(preview_target(r#"path: ".""#), ".");
        assert_eq!(preview_target(r#"command: "ls -la", cwd: "/x""#), "ls -la");
        assert_eq!(preview_target(r#"file_path: "a.rs""#), "a.rs", "file_path is its own key");
        assert_eq!(preview_target(r#"{"path":"x.rs"}"#), "x.rs");
        assert_eq!(preview_target("nothing here"), "");
    }

    #[test]
    fn the_status_label_carries_the_word_and_the_seconds() {
        let mut t = ToolView { state: "done".into(), secs: Some(2), ..Default::default() };
        assert_eq!(t.status_label(), "Done · 2 s");
        t.state = "running".into();
        assert_eq!(t.status_label(), "Running");
        t.state = "failed".into();
        t.secs = None;
        assert_eq!(t.status_label(), "Failed");
    }

    /// The operator's measurement: prose wrapped at ~300 px in a 693 px row.
    /// The fluid prose is `Fill` up to the column's measure — no artboard
    /// width survives.
    #[test]
    fn the_prose_fills_the_column_not_the_phone_artboard() {
        let m = desk();
        let dsl = assistant_prose("0", "hello `code`", &m);
        assert!(dsl.contains("Markdown{width: Fill max_width: 661"), "{dsl}");
        assert!(!dsl.contains("width: 356"), "the 356 px artboard width must not survive");
        assert!(dsl.contains("body: \"hello `code`\""));
        assert!(dsl.contains("NotoSansSC-Regular.ttf"), "CJK prose gets the sans face");
    }

    #[test]
    fn the_composer_spans_the_column_and_wraps_its_input() {
        let m = desk();
        let c = ComposerView {
            placeholder: "Ask Octos anything".into(),
            approval: "Ask for approval".into(),
            model: "v4-flash".into(),
        };
        let dsl = composer(&c, &m);
        assert!(dsl.starts_with("i0_composer := RoundedView{width: Fill"), "{dsl}");
        assert!(dsl.contains("i0_composer_0 := DesignInput{width: Fill"), "the input fills the card");
        assert!(dsl.contains("is_multiline: true submit_on_enter: true flow: Right{wrap: true}"),
            "a long prompt wraps instead of scrolling its first characters out of view");
        for id in ["plus_hit", "approval_pill_hit", "mic_hit", "send_hit"] {
            assert!(dsl.contains(&format!("{id} := Button{{")), "{id} is a real control in the card");
        }
        assert!(!dsl.contains("margin: Inset{left: 287"), "no artboard-anchored controls");
        assert!(dsl.contains("assets/icon_send"), "the kit's own send glyph");
        // Both glyphs ride the DSL (the host shows one): a turn starting or
        // ending must not change the mount string and remount the input.
        assert!(dsl.contains("composer_send_icon := View") && dsl.contains("composer_stop_icon := View"));
        assert!(dsl.contains("assets/icon_stop"), "the running turn's stop glyph");
    }

    /// A13 (judge, 360x780: "Write · Network..." and "deepse...") — on a
    /// phone the approval seat shows the mode alone and the room splits by
    /// need, so both seats read whole; the row still fits its card; the
    /// desktop keeps the web's full label.
    #[test]
    fn the_seat_labels_read_whole_on_a_phone() {
        use crate::screens::board3::ui::{text_w, Face as Ui};
        assert_eq!(compact_permission_label("Write · Network allowed"), "Write");
        assert_eq!(compact_permission_label("Full access · Network blocked"), "Full access");
        assert_eq!(compact_permission_label("Ask for approval"), "Ask for approval", "the art text stays");
        // The rendered widths at 13 px (/snap): "deepseek-v4-flash" (Medium)
        // 119 px, "Write · Network allowed" 146 px.
        const MODEL_W: f64 = 119.0;
        const FULL_APPROVAL_W: f64 = 146.0;
        for w in [360.0, 412.0] {
            let m = Metrics::for_window(w, false);
            let fit = composer_row_fit_for(&m, "Write", "deepseek-v4-flash");
            assert!(fit.fixed_w() + fit.approval_max + fit.model_max <= m.composer_w + 0.5, "{w}: the row fits its card");
            assert!(fit.model_max >= MODEL_W, "{w}: the model reads whole ({})", fit.model_max);
            assert!(fit.approval_max >= text_w("Write", 13.0, Ui::Regular), "{w}: the mode reads whole");
        }
        let desk = Metrics::for_window(990.0, true);
        let fit = composer_row_fit_for(&desk, "Write · Network allowed", "deepseek-v4-flash");
        assert!(fit.approval_max >= FULL_APPROVAL_W && fit.model_max >= MODEL_W, "desktop: both whole");
        assert!(fit.fixed_w() + fit.approval_max + fit.model_max <= desk.composer_w + 0.5);
        // Too long for both: the shorter keeps its width, the other gives.
        let phone = Metrics::for_window(360.0, false);
        let fit = composer_row_fit_for(&phone, "Full access", "deepseek-v4-flash-extended-context");
        assert!(fit.approval_max >= text_w("Full access", 13.0, Ui::Regular), "the mode stays whole");
        assert!(fit.fixed_w() + fit.approval_max + fit.model_max <= phone.composer_w + 0.5);
        // The phone composer carries the mode alone; the desktop the full label.
        let c = ComposerView {
            placeholder: "Ask Octos anything".into(),
            approval: "Write · Network allowed".into(),
            model: "deepseek-v4-flash".into(),
        };
        assert!(composer(&c, &phone).contains("text: \"Write\""));
        assert!(composer(&c, &desk).contains("text: \"Write · Network allowed\""));
    }

    /// Measured at 360x780 (OCTOSENSE_WINDOW_SIZE): the row asked 362 px of
    /// a 336 px card and `send_hit` was cut to 15x32. The budget keeps every
    /// control whole and lets the two labels ellipsize instead.
    #[test]
    fn the_composer_row_fits_its_card_at_every_width() {
        // The rendered widths at 13 px (desktop /snap): "Ask for approval"
        // 101 px, "v4-flash" 52 px.
        const APPROVAL_W: f64 = 101.0;
        const MODEL_W: f64 = 52.0;
        for (w, sidebar) in [(320.0, false), (360.0, false), (412.0, false), (990.0, true), (1376.0, true)] {
            let m = Metrics::for_window(w, sidebar);
            let fit = composer_row_fit(&m);
            assert!(
                fit.fixed_w() + fit.approval_max + fit.model_max <= m.composer_w + 0.5,
                "{w}: {} + {} + {} > card {}",
                fit.fixed_w(),
                fit.approval_max,
                fit.model_max,
                m.composer_w
            );
            if w >= 360.0 {
                assert!(fit.approval_max >= APPROVAL_W, "{w}: the default pill label must not ellipsize");
                assert!(fit.model_max >= MODEL_W, "{w}: the default model label must not ellipsize");
            }
        }
        let phone = Metrics::for_window(360.0, false);
        let c = ComposerView {
            placeholder: "Ask Octos anything".into(),
            approval: "Ask for approval".into(),
            model: "v4-flash".into(),
        };
        let dsl = composer(&c, &phone);
        assert!(dsl.contains("spacing: 0 padding: Inset{left: 8 right: 8"), "{dsl}");
        for id in ["i0_composer_2_0", "i0_composer_4"] {
            let at = dsl.find(&format!("{id} := Label{{")).unwrap();
            let head = &dsl[at..at + dsl[at..].find('\n').unwrap()];
            assert!(head.contains("max_lines: 1 text_overflow: TextOverflow.Ellipsis"), "{head}");
            assert!(!head.contains("max_width"), "the host applies the max (no width in the DSL): {head}");
            assert!(dsl[at..].contains("font_size: 9.75"), "13 px: the web's strip type");
        }
        // The mount string is the same at every width of one density: a
        // resize never remounts the composer (and never drops the draft).
        for (a, b) in [(361.0, 759.0), (990.0, 1376.0)] {
            assert_eq!(
                composer(&c, &Metrics::for_window(a, a >= 760.0)),
                composer(&c, &Metrics::for_window(b, b >= 760.0)),
                "{a} vs {b}"
            );
        }
        // The send disc keeps its 32 px box (it is never the one that gives).
        assert!(dsl.contains("i0_composer_5 := View{width: 32 height: 32 margin: Inset{left: 4}"), "{dsl}");
    }

    #[test]
    fn tool_rows_share_one_card_per_turn() {
        let m = desk();
        let t = ToolView { title: "read_file".into(), target: "README.md".into(), state: "done".into(), secs: Some(1), output: String::new() };
        let first = tool_row("0", &t, GroupPos::First, false, &m);
        let middle = tool_row("1", &t, GroupPos::Middle, false, &m);
        let last = tool_row("2", &t, GroupPos::Last, false, &m);
        assert!(first.contains("border_radius: vec4(5.0, 5.0, 1.0, 1.0)"), "{first}");
        assert!(middle.contains("border_inset: vec4(0.0, -2.0, 0.0, -2.0)"), "{middle}");
        assert!(last.contains("border_radius: vec4(1.0, 1.0, 5.0, 5.0)"), "{last}");
        // No zero corner anywhere: a zero-radius box_all strokes its whole face.
        for dsl in [&first, &middle, &last] {
            let at = dsl.find("border_radius: vec4(").unwrap();
            let radii = &dsl[at..at + dsl[at..].find(')').unwrap()];
            assert!(!radii.contains("0.0"), "{radii}");
        }
        assert!(!first.contains("height: 1 margin"), "no hairline above the first row");
        assert!(middle.contains("height: 1 margin"), "one hairline between rows");
        // One line: title, target and status share the header row.
        assert!(first.contains("text: \"read_file\""));
        assert!(first.contains("text: \"README.md\""));
        assert!(first.contains("text: \"Done · 1 s\""));
        assert!(first.contains("tool_read.svg"), "the kind glyph");
        assert!(first.contains(&format!("height: {}", scale(m.density).row_h)));
    }

    #[test]
    fn the_bubble_hugs_and_is_capped_at_the_web_bubble_width() {
        let m = desk();
        let dsl = user_bubble("0", "hi", &m, false);
        assert!(dsl.starts_with("user_align := View{width:Fill height:Fit flow:Down align: Align{x: 1.0}"));
        assert!(dsl.contains(&format!("RoundedView{{width: Fit height: Fit max_width: {}", m.bubble_max_w)));
        assert!(dsl.contains("flow: Right{wrap: true}"));
        assert!(dsl.contains("#000000ff"), "the board's black bubble");
        let dark = user_bubble("0", "hi", &m, true);
        assert!(dark.contains("#2c2c2eff") && dark.contains("draw_text.color: #f5f5f7ff"));
    }

    /// `Svg` has no `visible` property: a `visible:` inside an Svg block
    /// made the whole Connect card's DSL fail to evaluate (the card never
    /// mounted). Hideable icons ride a View.
    #[test]
    fn no_builder_sets_visible_on_an_svg() {
        let m = desk();
        let c = ConnectView { server: "http://127.0.0.1:50190".into(), ..Default::default() };
        let t = ToolView { title: "read_file".into(), state: "done".into(), ..Default::default() };
        let all = [
            connect_card(&c, &m, 261.0),
            tool_row("0", &t, GroupPos::Single, true, &m),
            worked_for("0", "Worked for 2s", 2, true, &m),
            answer_actions("0", "now", &m),
            empty_state(Some("octos"), &m),
            composer(
                &ComposerView { placeholder: "x".into(), approval: "y".into(), model: "z".into() },
                &m,
            ),
        ];
        for dsl in all {
            for (i, _) in dsl.match_indices(":= Svg{") {
                let head = &dsl[i..i + dsl[i..].find('}').unwrap_or(dsl.len() - i)];
                assert!(!head.contains("visible"), "an Svg cannot take `visible`: {head}");
            }
        }
    }

    /// Measured on the Chinese capture seed: a settled turn with no
    /// duration drew "· 3 tool calls" — a dot with nothing before it.
    #[test]
    fn a_worked_row_without_a_duration_shows_only_its_count() {
        let m = desk();
        let dsl = worked_for("0", "", 3, true, &m);
        assert!(dsl.contains("text: \"3 tool calls\""), "{dsl}");
        assert!(!dsl.contains("text: \"·\""), "no dangling separator: {dsl}");
        assert_eq!(worked_for("0", "", 0, true, &m), "View{width: Fill height: 0}\n", "nothing to say: no band");
        let timed = worked_for("0", "Worked for 2s ›", 3, true, &m);
        assert!(timed.contains("text: \"Worked for 2s\"") && timed.contains("text: \"·\""));
    }

    #[test]
    fn the_timestamp_sits_with_its_actions_not_at_the_far_edge() {
        let m = desk();
        let dsl = answer_actions("0", "now", &m);
        assert!(dsl.contains("text: \"now\""));
        assert!(!dsl.contains("align: Align{x: 1.0"), "no right-flush timestamp");
        let icons = dsl.find("icon_share").unwrap();
        let time = dsl.find("text: \"now\"").unwrap();
        assert!(icons < time, "the time follows the icons in the same row");
    }
}
