//! A7 — `A7CodeLines`: a code block's body with syntax colours.
//!
//! The renderer's text flow cannot carry per-run colours without side effects
//! (its inline link items underline every run in the run's own colour, and a
//! run that starts a line loses its leading whitespace — `TextFlow::draw_text`
//! trims both ends), so the highlighted body is this small widget instead: the
//! code is lexed in Rust ([`crate::highlight`], memoised per text/language/
//! theme) and each line is one wrapping row of coloured runs, drawn with the
//! code face. Indentation is kept exactly; a run longer than the row wraps at
//! its spaces; an empty line keeps its height. Copying uses the block's own
//! Copy control (the trimmed source), so the widget is display only.
//!
//! `A7MathBlock` is the display-math line: the renderer's `MathView` centred
//! the way KaTeX centres a display formula.
use makepad_widgets::*;

use crate::highlight::{self, Tok};

script_mod! {
    use mod.prelude.widgets.*

    mod.widgets.A7CodeLines = set_type_default() do #(A7CodeLines::register_widget(vm)) {
        width: Fill
        height: Fit
        draw_text +: {
            color: #1d1d1f
            text_style: theme.font_code{
                font_size: theme.font_size_p
            }
        }
    }

    mod.widgets.A7MathBlockBase = #(A7MathBlock::register_widget(vm))
    mod.widgets.A7MathBlock = set_type_default() do mod.widgets.A7MathBlockBase{
        width: Fill
        height: Fit
        align: Align{x: 0.5}
    }
}

/// A display-math block (`$$…$$`) centred on its own line, as KaTeX's
/// `.katex-display` is (`text-align: center`). The renderer's Markdown hands
/// the expression to its `display_math` item with `set_text`; a `View` drops
/// that, so this wrapper forwards it to its `math` child (a `MathView`).
#[derive(Script, ScriptHook, Widget)]
pub struct A7MathBlock {
    #[deref]
    view: View,
}

impl Widget for A7MathBlock {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }

    fn text(&self) -> String {
        self.view.text()
    }

    fn set_text(&mut self, cx: &mut Cx, v: &str) {
        self.view.widget(cx, ids!(math)).set_text(cx, v);
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct A7CodeLines {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[redraw]
    #[live]
    draw_text: DrawText,
    #[walk]
    walk: Walk,
    #[area]
    #[rust]
    area: Area,
    /// The code (no trailing newline).
    #[live]
    text: ArcStringMut,
    /// The fence's language (the web's alias table, `highlight::grammar`).
    #[live]
    lang: ArcStringMut,
    /// Dark theme colours (`theme.css` `light-dark(...)`).
    #[live(false)]
    dark: bool,
    /// One code line's height in logical px (the web's `12px/20px`); a row
    /// is padded to it, since a single laid-out run is only its glyph box.
    #[live(20.0)]
    line_height: f64,
    /// The lexed lines, keyed by (text, lang, dark).
    #[rust]
    lines: Vec<Vec<(Vec4f, String)>>,
    #[rust]
    key: Option<(String, String, bool)>,
}

/// `#rrggbbaa` → linear Vec4 (the DSL's own colour parse).
fn hex(c: &str) -> Vec4f {
    let h = c.trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("ff"), 16).unwrap_or(255) as f32 / 255.0;
    vec4(p(0), p(2), p(4), if h.len() >= 8 { p(6) } else { 1.0 })
}

/// Split a run at its spaces, keeping each space with the word before it,
/// so a long comment or string can wrap inside the row.
fn word_chunks(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        cur.push(ch);
        if ch == ' ' && cur.trim() != "" {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

impl A7CodeLines {
    fn ensure_lines(&mut self) {
        let key = (self.text.as_ref().to_owned(), self.lang.as_ref().to_owned(), self.dark);
        if self.key.as_ref() == Some(&key) {
            return;
        }
        let grammar = highlight::grammar(Some(key.1.as_str()).filter(|l| !l.is_empty()));
        let dark = self.dark;
        self.lines = highlight::block(grammar, &key.0)
            .into_iter()
            .map(|spans| {
                let mut row: Vec<(Vec4f, String)> = Vec::new();
                let mut at_start = true;
                for (tok, text) in spans {
                    let color = hex(tok.color(dark));
                    // The leading indentation stays one run (never split);
                    // the rest wraps at spaces.
                    if at_start {
                        let lead = text.len() - text.trim_start().len();
                        if lead > 0 {
                            row.push((hex(Tok::Plain.color(dark)), text[..lead].to_owned()));
                        }
                        if lead < text.len() {
                            at_start = false;
                            for c in word_chunks(&text[lead..]) {
                                row.push((color, c));
                            }
                        }
                    } else {
                        for c in word_chunks(&text) {
                            row.push((color, c));
                        }
                    }
                }
                row
            })
            .collect();
        self.key = Some(key);
    }
}

impl Widget for A7CodeLines {
    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        self.ensure_lines();
        cx.begin_turtle(walk, Layout { flow: Flow::Down, ..Layout::default() });
        let plain = hex(Tok::Plain.color(self.dark));
        let glyph_box = self
            .draw_text
            .layout(cx, 0.0, 0.0, None, false, Align::default(), "Mg")
            .size_in_lpxs
            .height as f64
            * self.draw_text.font_scale as f64;
        let pad = ((self.line_height - glyph_box) / 2.0).max(0.0);
        let row = Layout {
            flow: Flow::right_wrap(),
            padding: Inset { top: pad, bottom: pad, left: 0.0, right: 0.0 },
            ..Layout::default()
        };
        for line in &self.lines {
            cx.begin_turtle(Walk::fill_fit(), row);
            if line.is_empty() {
                self.draw_text.color = plain;
                self.draw_text.draw_walk(cx, Walk::fit(), Align::default(), " ");
            }
            for (color, run) in line {
                self.draw_text.color = *color;
                self.draw_text.draw_walk(cx, Walk::fit(), Align::default(), run);
            }
            cx.end_turtle();
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }

    fn text(&self) -> String {
        self.text.as_ref().to_string()
    }

    fn set_text(&mut self, cx: &mut Cx, v: &str) {
        if self.text.as_ref() == v {
            return;
        }
        self.text.as_mut_empty().push_str(v);
        self.redraw(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_runs_wrap_at_their_spaces_and_colours_parse() {
        assert_eq!(word_chunks("// drain in order"), ["// ", "drain ", "in ", "order"]);
        assert_eq!(word_chunks("x"), ["x"]);
        let c = hex("#1864abff");
        assert!((c.x - 0x18 as f32 / 255.0).abs() < 1e-6 && (c.w - 1.0).abs() < 1e-6);
    }
}
