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

// ---- palette (light; dark comes from retint_dsl) ---------------------------

/// Primary text (`color_fg_app`).
pub const INK: &str = "#1d1d1fff";
/// Secondary text (`color_text_muted`).
pub const MUTED: &str = "#6e6e73ff";
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

fn svg(id: &str, rel: &str, size: f64, color: &str) -> String {
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

fn label(id: &str, text: &str, style: &str, color: &str, walk: &str) -> String {
    let head = if id.is_empty() { String::new() } else { format!("{id} := ") };
    format!(
        "{head}Label{{{walk} padding: 0 text: {text:?}\n\
         draw_text.text_style: {style}\n\
         draw_text.color: {color}}}\n"
    )
}

/// A transparent hit target over its parent overlay (the shell's own
/// `review_close` pattern, `lib.rs`): no fill, a faint hover/press tint.
pub fn hit(id: &str, radius: f64) -> String {
    // `color_2` at (-1,-1,-1,-1) DISABLES the face gradient: at #00000000 the
    // gradient stays on (button.rs tests `color_2.x > -0.5`) and mixes toward
    // the theme's `color_2_focus` once clicked — measured as a white wash
    // over a disclosed tool row. Focus stays transparent for the same reason.
    format!(
        "{id} := Button{{width: Fill height: Fill text: \"\" margin: 0 padding: 0 \
         label_walk: Walk{{width: 0 height: 0}} icon_walk: Walk{{width: 0 height: 0}} \
         draw_bg.color: #00000000 draw_bg.color_hover: #0000000a draw_bg.color_down: #00000014 \
         draw_bg.color_focus: #00000000 draw_bg.color_disabled: #00000000 \
         draw_bg.color_2: vec4(-1.0, -1.0, -1.0, -1.0) draw_bg.border_size: 0.0 draw_bg.border_radius: {radius} \
         draw_bg.border_color: #00000000 draw_bg.border_color_2: vec4(-1.0, -1.0, -1.0, -1.0)}}\n"
    )
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
    let (fill, ink) = if dark { ("#2c2c2eff", "#f5f5f7ff") } else { ("#000000ff", "#ffffffff") };
    // The bubble's text box: the cap minus the bubble's own side padding.
    let pad_x = 16.0;
    let text_max = (m.bubble_max_w - 2.0 * pad_x).max(40.0);
    format!(
        "user_align := View{{width:Fill height:Fit flow:Down align: Align{{x: 1.0}} \
         padding: Inset{{top: {top} bottom: {BUBBLE_BOTTOM}}}\n\
         i{tok}_userbubble := RoundedView{{width: Fit height: Fit max_width: {max} flow: Down \
         padding: Inset{{left: {pad_x} right: {pad_x} top: 10 bottom: 10}}\n\
         draw_bg +: {{color: {fill} border_radius: 9.0}}\n\
         i{tok}_userbubble_0 := Label{{width: Fit height: Fit max_width: {text_max} padding: 0 \
         flow: Right{{wrap: true}} max_lines: {FIT_WRAP_LINES} text: {text:?}\n\
         draw_text.text_style: {style}\n\
         draw_text.color: {ink}}}\n\
         }}\n}}\n",
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
    let s = scale(m.density);
    let line = s.body_line + 1.0;
    format!(
        "View{{width: Fill height: Fit flow: Down padding: Inset{{top: 4 bottom: 8}}\n\
         i{tok}_assistantprose := Markdown{{width: Fill max_width: {max} height: Fit padding: 0 margin: 0\n\
         body: {body:?}\n\
         font_size: {fs}\n\
         font_color: {INK}\n\
         paragraph_spacing: 14\n\
         pre_code_spacing: 10\n\
         heading_base_scale: 1.6\n\
         fixed_font_size_scale: 0.87\n\
         inline_code_padding: Inset{{left: 5 right: 5 top: 1 bottom: 1}}\n\
         inline_code_margin: Inset{{left: 2 right: 2 top: 0 bottom: 0}}\n\
         text_style_normal: {regular}\n\
         text_style_italic: {regular}\n\
         text_style_bold: {bold}\n\
         text_style_bold_italic: {bold}\n\
         text_style_fixed: {mono}\n\
         code_layout: Layout{{flow: Right{{wrap: true}} padding: Inset{{left: 14 right: 14 top: 10 bottom: 10}}}}\n\
         table_cell_layout: Layout{{flow: Right{{wrap: true}} padding: Inset{{left: 14 right: 14 top: 9 bottom: 9}}}}\n\
         draw_block +: {{code_color: {RAISED} line_color: {INK} sep_color: {BORDER} \
         quote_bg_color: {BORDER} quote_fg_color: {MUTED} \
         table_header_bg_color: #00000000 table_border_color: {BORDER}}}\n\
         }}\n}}\n",
        max = m.prose_max_w,
        fs = s.body * 0.75,
        regular = flow_style(Face::Regular, s.body, line),
        bold = flow_style(Face::SemiBold, s.body, line),
        mono = flow_style(Face::Mono, s.body, line),
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
    let r = 5.0;
    let q = 1.0;
    let (radius, inset) = match pos {
        GroupPos::Single => (format!("vec4({r:.1}, {r:.1}, {r:.1}, {r:.1})"), "vec4(0.0, 0.0, 0.0, 0.0)"),
        GroupPos::First => (format!("vec4({r:.1}, {r:.1}, {q:.1}, {q:.1})"), "vec4(0.0, 0.0, 0.0, -2.0)"),
        GroupPos::Middle => (format!("vec4({q:.1}, {q:.1}, {q:.1}, {q:.1})"), "vec4(0.0, -2.0, 0.0, -2.0)"),
        GroupPos::Last => (format!("vec4({q:.1}, {q:.1}, {r:.1}, {r:.1})"), "vec4(0.0, -2.0, 0.0, 0.0)"),
    };
    let top_rule = matches!(pos, GroupPos::Middle | GroupPos::Last);
    let bottom_gap = if matches!(pos, GroupPos::Single | GroupPos::Last) { 10.0 } else { 0.0 };
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
            &t.title,
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
                &style(Face::Mono, 12.0, 19.0),
                MUTED,
                "width: Fill height: Fit flow: Right{wrap: true}",
            ),
        ));
    }
    format!(
        "View{{width: Fill height: Fit flow: Down padding: Inset{{bottom: {bottom_gap}}}\n\
         i{tok}_toolcell := RoundedAllView{{width: Fill height: Fit flow: Down \
         draw_bg +: {{color: {TIP} border_size: 1.0 border_color: {BORDER} border_radius: {radius} border_inset: {inset}}}\n\
         {body}}}\n}}\n"
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
         View{{width: Fill height: 28 flow: Overlay\n\
         i{tok}_workedfor := View{{width: Fill height: 28 flow: Right align: Align{{y: 0.5}} spacing: 6\n\
         {row}}}\n\
         {hit}\
         }}\n}}\n",
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
    let btn = |id: &str, file: &str, hit_id: Option<&str>| {
        format!(
            "View{{width: 28 height: 28 flow: Overlay align: Align{{x: 0.0 y: 0.5}}\n{}{}}}\n",
            svg(id, &format!("components/answer-actions/assets/{file}"), 16.0, MUTED),
            hit_id.map(|h| hit(h, 6.0)).unwrap_or_default(),
        )
    };
    format!(
        "View{{width: Fill height: Fit flow: Down padding: Inset{{top: 2 bottom: 12}}\n\
         i{tok}_answeractions := View{{width: Fill height: 28 flow: Right align: Align{{y: 0.5}} spacing: 2\n\
         {copy}{thumbs}{share}\
         {time}}}\n}}\n",
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
        // plus | pill | spacer | model | mic | send: four gaps.
        2.0 * self.pad_x
            + 3.0 * COMPOSER_CONTROL
            + 4.0 * self.gap
            + 2.0 * self.pill_pad
            + 2.0 * self.model_pad
            + COMPOSER_CHEVRON_GAP
            + COMPOSER_CHEVRON
    }
}

pub fn composer_row_fit(m: &Metrics) -> ComposerRowFit {
    let (pad_x, gap, pill_pad, model_pad) = match m.density {
        Density::Desktop => (10.0, 6.0, 12.0, 6.0),
        Density::Phone => (8.0, 4.0, 10.0, 2.0),
    };
    let mut fit = ComposerRowFit { pad_x, gap, pill_pad, model_pad, approval_max: 0.0, model_max: 0.0 };
    let labels = (m.composer_w - fit.fixed_w()).max(0.0);
    fit.model_max = (labels * 0.36).clamp(40.0, 160.0).floor();
    fit.approval_max = (labels - fit.model_max).max(40.0).floor();
    fit
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
    let icon_btn = |hit_id: &str, file: &str, size: f64, left: f64| {
        format!(
            "View{{width: {COMPOSER_CONTROL} height: {COMPOSER_CONTROL} margin: Inset{{left: {left}}} \
             flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n\
             {svg}{hit}}}\n",
            svg = svg(&format!("{hit_id}_icon"), &format!("components/composer/assets/{file}"), size, MUTED),
            hit = hit(hit_id, 16.0),
        )
    };
    // The labels' max widths are applied by the host (`apply_composer_fit`),
    // never written here: the DSL must not change with the width.
    let one_line = "width: Fit height: Fit max_lines: 1 text_overflow: TextOverflow.Ellipsis";
    format!(
        "i0_composer := RoundedView{{width: Fill height: Fit flow: Down padding: 0\n\
         draw_bg +: {{color: {SURFACE} border_radius: 9.0 border_size: 1.0 border_color: {BORDER}}}\n\
         View{{width: Fill height: Fit padding: Inset{{left: 16 right: 16 top: 14 bottom: 2}}\n\
         i0_composer_0 := DesignInput{{width: Fill height: Fit min_height: {min_h} max_height: 200 \
         padding: Inset{{top: 2 bottom: 2}} margin: 0\n\
         is_multiline: true submit_on_enter: true flow: Right{{wrap: true}}\n\
         text: \"\" empty_text: {placeholder:?}\n\
         draw_text +: {{color: {INK} color_empty: {MUTED} text_style: {input_style} \
         get_color: fn() {{return mix(self.color, self.color_empty, self.empty)}}}}\n\
         draw_cursor +: {{color: {INK}}}\n\
         draw_selection +: {{color: #2f6feb33 color_hover: #2f6feb33 color_focus: #2f6feb40 \
         color_down: #2f6feb40 color_empty: #2f6feb33 color_disabled: #2f6feb33}}\n\
         }}\n}}\n\
         i0_composer_row := View{{width: Fill height: 48 flow: Right align: Align{{y: 0.5}} spacing: 0 \
         padding: Inset{{left: {pad_x} right: {pad_x} bottom: 4}}\n\
         {plus}\
         i0_composer_2 := View{{width: Fit height: 30 margin: Inset{{left: {gap}}} flow: Overlay\n\
         RoundedView{{width: Fit height: 30 flow: Right align: Align{{y: 0.5}} \
         padding: Inset{{left: {pill_pad} right: {pill_pad}}} \
         draw_bg +: {{color: {SURFACE} border_radius: 15.0 border_size: 1.0 border_color: {BORDER}}}\n\
         {approval}}}\n\
         {approval_hit}}}\n\
         View{{width: Fill height: 1}}\n\
         i0_composer_model := View{{width: Fit height: 30 margin: Inset{{left: {gap}}} flow: Right \
         align: Align{{y: 0.5}} spacing: {COMPOSER_CHEVRON_GAP} \
         padding: Inset{{left: {model_pad} right: {model_pad}}}\n\
         {model}{model_chev}}}\n\
         {mic}\
         i0_composer_5 := View{{width: {COMPOSER_CONTROL} height: {COMPOSER_CONTROL} margin: Inset{{left: {gap}}} \
         flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n\
         RoundedView{{width: 32 height: 32 draw_bg +: {{color: #000000ff border_radius: 16.0}}}}\n\
         composer_send_icon := View{{width: 16 height: 16 flow: Overlay\n{send_icon}}}\n\
         composer_stop_icon := View{{width: 16 height: 16 flow: Overlay visible: false\n{stop_icon}}}\n\
         {send_hit}}}\n\
         }}\n}}\n",
        min_h = (s.body_line + 4.0).round(),
        placeholder = c.placeholder,
        input_style = style(Face::Regular, input_px, s.body_line),
        pad_x = fit.pad_x,
        pill_pad = fit.pill_pad,
        model_pad = fit.model_pad,
        plus = icon_btn("plus_hit", "icon_plus1.svg", 18.0, 0.0),
        approval = label(
            "i0_composer_2_0",
            &c.approval,
            &style(Face::Regular, COMPOSER_ROW_PX, COMPOSER_ROW_LINE),
            INK,
            one_line,
        ),
        approval_hit = hit("approval_pill_hit", 15.0),
        model = label(
            "i0_composer_4",
            &c.model,
            &style(Face::Medium, COMPOSER_ROW_PX, COMPOSER_ROW_LINE),
            INK,
            one_line,
        ),
        model_chev = svg("i0_composer_4_chev", "chevron_down.svg", COMPOSER_CHEVRON, MUTED),
        mic = icon_btn("mic_hit", "icon_mic1.svg", 18.0, gap),
        send_icon = svg("i0_composer_5_0", "components/composer/assets/icon_send.svg", 16.0, "#ffffffff"),
        // The running turn's STOP glyph (conversation-08 `stop2`): shown by
        // the host while `turn.active`, so the DSL never changes per turn.
        stop_icon = svg("i0_composer_5_1", "components/composer/assets/icon_stop.svg", 16.0, "#ffffffff"),
        send_hit = hit("send_hit", 16.0),
    )
}

/// The empty conversation (`Timeline.tsx:111-134` + conversation-02): a
/// quiet mark, the question, the web's one-line hint, and the workspace chip
/// when the session reported its root.
pub fn empty_state(workspace: Option<&str>, m: &Metrics) -> String {
    let (title_px, title_line) = match m.density {
        Density::Desktop => (26.0, 34.0),
        Density::Phone => (23.0, 30.0),
    };
    let title = match workspace {
        Some(ws) if !ws.is_empty() => format!("What should we build in {ws}?"),
        _ => "What should we build?".to_owned(),
    };
    let chip = match workspace {
        Some(ws) if !ws.is_empty() => format!(
            "View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: 16}}\n\
             RoundedView{{width: Fit height: 28 flow: Right align: Align{{y: 0.5}} spacing: 6 \
             padding: Inset{{left: 10 right: 12}} draw_bg +: {{color: {SURFACE} border_radius: 14.0 \
             border_size: 1.0 border_color: {BORDER}}}\n\
             {icon}{text}}}\n}}\n",
            icon = svg("empty_chip_icon", "folder.svg", 14.0, MUTED),
            text = label(
                "empty_chip",
                ws,
                &style(Face::Regular, 13.0, 18.0),
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
         RoundedView{{width: 48 height: 48 flow: Overlay align: Align{{x: 0.5 y: 0.5}} \
         draw_bg +: {{color: {RAISED} border_radius: 7.0}}\n\
         {mark}}}\n}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: 18}}\n\
         {title}}}\n\
         View{{width: Fill height: Fit align: Align{{x: 0.5}} padding: Inset{{top: 8}}\n\
         {hint}}}\n\
         {chip}}}\n",
        mark = svg("empty_mark", "chat_terminal.svg", 26.0, INK),
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
            "Describe a change, investigate a bug, or ask how the code works.",
            &style(Face::Regular, 14.0, 22.0),
            MUTED,
            "width: Fill height: Fit max_width: 480 align: Align{x: 0.5} flow: Right{wrap: true}",
        ),
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
             draw_text +: {{color: {INK} color_empty: #9a9aa0ff text_style: {st} \
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
        let detail = [c.last_tried.as_str(), c.error_actions.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "connect_error := RoundedView{{width: Fill height: Fit flow: Down spacing: 2 margin: Inset{{top: 14}} \
             padding: Inset{{left: 12 right: 12 top: 10 bottom: 10}} \
             draw_bg +: {{color: #fdececff border_radius: 4.0}}\n\
             {msg}{detail}}}\n",
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
    format!(
        "connect_center := View{{width: Fill height: Fill flow: Down align: Align{{x: 0.5 y: 0.42}} \
         padding: Inset{{left: {lp} right: {rp} top: 16 bottom: 16}}\n\
         connect_card := RoundedView{{width: Fill max_width: 480 height: Fit flow: Down \
         padding: Inset{{left: {pad_x} right: {pad_x} top: {pad_top} bottom: 22}}\n\
         draw_bg +: {{color: {SURFACE} border_radius: 6.0 border_size: 1.0 border_color: {BORDER}}}\n\
         {title}\
         {error}\
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
         b1_connect_pair_wrap := View{{width: Fit height: 32 flow: Overlay align: Align{{x: 0.5 y: 0.5}} \
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
            "Connect to Octos",
            &style(Face::SemiBold, if phone { 20.0 } else { 19.0 }, 26.0),
            INK,
            "width: Fit height: Fit",
        ),
        server_cap = caption("Server"),
        server_field = field("connect_server", &c.server, "http://127.0.0.1:50190", false),
        server_err = label(
            "connect_server_error",
            "",
            &style(Face::Regular, 12.0, 18.0),
            RED,
            "width: Fill height: Fit margin: Inset{top: 4} flow: Right{wrap: true}",
        ),
        token_cap = caption("Access token"),
        token_field = field("connect_token", "", "Paste your server token", true),
        note = label(
            "connect_note",
            "Stored for this server only",
            &style(Face::Regular, 12.0, 18.0),
            MUTED,
            "width: Fit height: Fit",
        ),
        btn_label = label(
            "connect_btn_label",
            if c.connecting { "Connecting…" } else { "Connect" },
            &style(Face::Medium, 15.0, 20.0),
            "#ffffffff",
            "width: Fit height: Fit",
        ),
        btn_hit = hit("connect_btn", 22.0),
        solo_label = label(
            "connect_solo_label",
            "Use local solo server",
            &style(Face::Regular, 14.0, 20.0),
            "#2f6febff",
            "width: Fit height: Fit",
        ),
        solo_hit = hit("connect_solo", 6.0),
        // #A2 board 1: the Connect screen's way into pairing (p4-01). The id is
        // routed by `screens::board1::entry_controls` -> `b1.open.pairing`.
        pair_label = label(
            "b1_connect_pair_label",
            "Pair with a link instead",
            &style(Face::Regular, 14.0, 20.0),
            "#2f6febff",
            "width: Fit height: Fit",
        ),
        pair_hit = hit("b1_connect_pair", 6.0),
    )
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
