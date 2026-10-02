//! A10 — the header's "Review" entry: the web's `DiffReviewDialog`
//! (`features/review/DiffReviewDialog.tsx`) over `use-coding-safety.ts:268-345`
//! (`openDiffReview` / `closeDiffReview`), in the board-3 dialog kit.
//!
//! * the header: the eyebrow "Authoritative diff preview", the preview's
//!   title (else "Review changes"), the +N −N totals counted over the
//!   preview's lines once a result exists, Refresh (inert while loading) and
//!   the 28 px close;
//! * the status row: the result's status (else loading / error), its
//!   source, the preview id (right-aligned, `.review-status code
//!   { margin-left: auto }`; hidden on a phone);
//! * the body's states, each the web's own copy: "Loading the server
//!   snapshot…", "Preview unavailable" + the reason, "The preview is ready,
//!   but it contains no changed files.", or every file (status mark, path,
//!   "from" the old path, status) with its hunks (header + numbered lines,
//!   +/− marked and tinted) or "Line-level diff unavailable for this
//!   mutation.";
//! * ONE `diff/preview/get {session_id, preview_id}` per open / Refresh for
//!   the latest announced preview id (an approval's typed diff, a turn's
//!   diff); the reply must name THIS session and THIS preview, a reply that
//!   lands after a close / reopen (an older generation) is dropped.
//!
//! A28 — parity row 23 (design board 4 frames 1, 1b, 2; README "Row 23"):
//!
//! * every line is drawn as per-token RUNS (`<line>_c<k>_<class>`; inside a
//!   word mark `<line>_w<m>_c<k>_<class>`) in its
//!   file's syntax colours (`highlight::Tok::color`, by extension —
//!   [`super::diff_words`], the web's `diff-presentation.ts`), on context,
//!   removed and added lines; the row keeps its tint;
//! * a changed word sits in a WORD MARK (`<line>_w<m>`): a rounded fill behind
//!   its runs, the success colour at 22 % on added rows, the error colour at
//!   20 % on removed rows (`DiffReviewDialog.module.css` `.changedWord`),
//!   composited over the row tint; a token ink that would read below 4.5:1
//!   on a mark steps toward the text ink (the A18 rule, [`mark_ink`]);
//! * past the bound (`canDecorateDiff`: 400 lines / 40,000 characters / a
//!   2,000-character line) NOTHING is decorated: the note [`PLAIN_NOTE`]
//!   heads the body and each hunk is drawn as plain blocks
//!   (`<hunk>_b<n>`, one multi-line label per column, red / green text on
//!   the line tint) — every line, never truncated;
//! * the dialog is `min(1080, 100%) × min(780, 100%)` inside the web's 24 px
//!   backdrop inset (`.review-dialog`); at <= 760 px it is the web's compact
//!   FULL-SCREEN sheet (`@media (max-width: 760px)`: no inset, no radius, no
//!   border, the preview id hidden), every hunk scrolling sideways (A13).
//!
//! The web shows its "Review changes" entry only once a preview id is known;
//! the native header keeps its Review entry (board 2), so with no preview the
//! dialog says so and offers the server's native code review instead
//! (`/review`, the Code review dialog) when the server has it.
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use octos_core::ui_protocol::{DiffPreviewFile, DiffPreviewGetResult, DiffPreviewLine, DiffPreviewLineKind};
use octoscode_store::Store;
use serde_json::json;

use super::diff_words::{self as dw, DiffToken};
use super::host::{Job, Outcome};
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};
use crate::highlight::Tok;
use crate::i18n::tr;

pub const EYEBROW: &str = "Authoritative diff preview";
pub const TITLE: &str = "Review changes";
pub const LOADING: &str = "Loading the server snapshot…";
pub const UNAVAILABLE: &str = "Preview unavailable";
pub const NO_FILES: &str = "The preview is ready, but it contains no changed files.";
pub const NO_LINES: &str = "Line-level diff unavailable for this mutation.";
pub const MISMATCH: &str = "diff/preview/get returned a mismatched preview";
/// Native: the entry with no preview id yet (the web hides its entry then).
pub const NO_PREVIEW_HEAD: &str = "No diff preview yet";
pub const NO_PREVIEW_BODY: &str =
    "A preview appears here once this Session proposes file changes, such as an approval that edits files.";
pub const NO_METHOD: &str = "This server does not provide diff previews.";
pub const NATIVE_REVIEW: &str = "Code review…";
/// A28 — `DiffReviewDialog.tsx:107-109` `plainNotice`: past the bound.
pub const PLAIN_NOTE: &str = "Large preview shown as plain text. All lines are included.";

/// A28 — a hunk row (the web's `.dialog .diff-line { line-height: 24px }`).
pub const ROW_H: f64 = 24.0;
/// The code's size (the kit's mono) and the gutter numbers'.
pub const CODE_PX: f64 = 11.5;
pub const NUM_PX: f64 = 11.0;
/// LiberationMono's line box, hhea ascender − descender (1705 + 615 of 2048).
const MONO_LINE_EM: f64 = 2320.0 / 2048.0;
/// A block label's line spacing: its rows exactly [`ROW_H`] apart, like the
/// decorated rows.
pub const CODE_SPACING: f64 = ROW_H / (CODE_PX * MONO_LINE_EM);
pub const NUM_SPACING: f64 = ROW_H / (NUM_PX * MONO_LINE_EM);
/// The first row's top inset in a block (a lone label centred in a row).
const BLOCK_PAD: f64 = (ROW_H - CODE_PX * MONO_LINE_EM) / 2.0;
/// The web's dialog: `width: min(1080px, 100%)`, `height: min(780px, 100%)`
/// inside `.review-backdrop { padding: var(--dsw-space-5) }` (24 px); the
/// compact sheet below 760 px.
pub const MAX_W: f64 = 1080.0;
pub const MAX_H: f64 = 780.0;
pub const INSET: f64 = 24.0;
pub const SHEET_BELOW: f64 = 760.0;
/// A plain block's most lines (one label per column; a long hunk is several).
const BLOCK_LINES: usize = 100;

/// The web's `--dsw-alias-state-success-primary` / `-error-primary` (light,
/// `app/theme.css:86-89`): a mark is one of them at 22 % / 20 %.
const SUCCESS: &str = "#22c55e";
const ERROR: &str = "#ec1313";
/// A word mark's fill: the success colour at 22 % over the added row's tint
/// ([`mark_fill`] computes it; a test pins the literal).
pub const MARK_ADDED: &str = "#bbeacbff";
/// … the error colour at 20 % over the removed row's tint.
pub const MARK_REMOVED: &str = "#fac1c1ff";
/// The inks a run takes ON a mark, `(token, on an added mark, on a removed
/// mark)`: the token colour, stepped toward the text ink only where it read
/// below 4.5:1 on the mark ([`mark_ink`]; declared in
/// `screens::theme::CONTRAST_PAIRS`).
pub const MARK_INKS: &[(Option<Tok>, &str, &str)] = &[
    (None, "#1d1d1fff", "#1d1d1fff"),
    (Some(Tok::Plain), "#1d1d1fff", "#1d1d1fff"),
    (Some(Tok::Keyword), "#b4235aff", "#a52254ff"),
    (Some(Tok::String), "#2b6f3aff", "#296336ff"),
    (Some(Tok::Comment), "#5c6269ff", "#52575dff"),
    (Some(Tok::Constant), "#1864abff", "#195996ff"),
    (Some(Tok::Function), "#5f3dc4ff", "#5f3dc4ff"),
    (Some(Tok::Punctuation), "#495057ff", "#495057ff"),
];

#[derive(Debug, Clone, Default)]
pub struct DiffReviewState {
    /// The preview this open reads (`latestPreviewId`).
    pub preview_id: Option<String>,
    pub loading: bool,
    pub error: Option<String>,
    pub result: Option<DiffPreviewGetResult>,
    /// Bumped by every open / refresh / close: a reply for an older one is
    /// dropped (`diffRequestsRef.isCurrent`).
    pub generation: u64,
    /// A28 — the decorated body, built once per read, frame and language
    /// (the web's `useMemo` over `state.result`): the host lowers the
    /// dialog on every UI signal.
    pub cache: BodyCache,
}

/// One built body and what it was built for.
#[derive(Clone, Default)]
pub struct BodyCache(Arc<Mutex<Option<(BodyKey, Arc<String>)>>>);

impl std::fmt::Debug for BodyCache {
    /// A summary, never the DSL itself (a large preview's body is megabytes).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let slot = self.0.lock().unwrap_or_else(|p| p.into_inner());
        match slot.as_ref() {
            Some((k, body)) => write!(f, "BodyCache(gen {} {} lines, {} bytes)", k.generation, k.lines, body.len()),
            None => write!(f, "BodyCache(empty)"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct BodyKey {
    generation: u64,
    preview: String,
    lines: usize,
    chars: usize,
    width: i64,
    sheet: bool,
    lang: u64,
}

fn advertised(store: &Store) -> bool {
    crate::screens::dialog::advertises(store, "diff/preview/get")
}

fn native_review_available(store: &Store) -> bool {
    crate::screens::dialog::advertises(store, "review/start") && crate::screens::dialog::advertises(store, "review.start.v1")
}

/// `openDiffReview()`: the latest announced preview; nothing is read without
/// one (the job itself sends nothing when the server lacks the method).
pub fn on_open(st: &mut DiffReviewState) -> Outcome {
    st.generation += 1;
    st.result = None;
    st.error = None;
    st.loading = false;
    st.preview_id = crate::screens::review::ui().preview_id.clone();
    if st.preview_id.is_none() {
        return Outcome::Done;
    }
    st.loading = true;
    Outcome::Spawn(Job::DiffReviewLoad(st.generation))
}

/// `closeDiffReview()`: invalidate the in-flight read, drop the result.
pub fn on_close(st: &mut DiffReviewState) {
    st.generation += 1;
    st.loading = false;
    st.result = None;
    st.error = None;
}

pub fn perform(st: &mut DiffReviewState, action: &str, store: &Store) -> Outcome {
    match action {
        "b3.diff.refresh" => {
            if st.loading || st.preview_id.is_none() || !advertised(store) {
                return Outcome::Done;
            }
            st.generation += 1;
            st.loading = true;
            st.result = None;
            st.error = None;
            Outcome::Spawn(Job::DiffReviewLoad(st.generation))
        }
        "b3.diff.native" => {
            if !native_review_available(store) {
                return Outcome::Done;
            }
            // One modal at a time: the Code review dialog replaces this one.
            on_close(st);
            Outcome::Action("dialog.open.review".into())
        }
        _ => Outcome::Unrouted,
    }
}

/// ONE `diff/preview/get` for the open's preview id; the reply must name this
/// session and this preview (`use-coding-safety.ts:301-306`).
pub async fn load(conv: &crate::flow::Conversation, generation: u64) -> Result<String, String> {
    let session = conv.session_id();
    let Some(preview_id) = super::host::state().diff.preview_id.clone() else {
        return Ok("no preview".into());
    };
    if !advertised(&conv.store) {
        // `supportsMethod(…, DIFF_PREVIEW_GET)` fails: no frame.
        super::host::state().diff.loading = false;
        return Ok("diff/preview/get not advertised".into());
    }
    let reply = conv.client().request("diff/preview/get", json!({"session_id": session, "preview_id": preview_id})).await;
    let outcome: Result<DiffPreviewGetResult, String> = match reply {
        Ok(v) => match serde_json::from_value::<DiffPreviewGetResult>(v.clone()) {
            Ok(r) if r.preview.session_id.0 == session && serde_json::from_value::<octos_core::ui_protocol::PreviewId>(json!(preview_id)).ok().as_ref() == Some(&r.preview.preview_id) => {
                // The review screen's cache follows the same read.
                crate::screens::review::fold_preview(&v);
                Ok(r)
            }
            Ok(_) => Err(MISMATCH.to_owned()),
            Err(_) => Err("diff/preview/get returned a malformed preview".to_owned()),
        },
        Err(e) => Err(crate::screens::dialog::display_error(&e.to_string())),
    };
    let mut st = super::host::state();
    if st.diff.generation != generation {
        return Ok("a stale preview read dropped".into());
    }
    st.diff.loading = false;
    match outcome {
        Ok(r) => {
            let n = r.preview.files.len();
            let decorated = dw::can_decorate(&r.preview.files);
            st.diff.result = Some(r);
            Ok(format!("{n} file(s), {}", if decorated { "decorated" } else { "plain (past the decoration bound)" }))
        }
        Err(e) => {
            st.diff.error = Some(e.clone());
            Err(e)
        }
    }
}

/// `+additions` / `−deletions` over every line (`DiffReviewDialog.tsx:32-42`).
pub fn totals(r: &DiffPreviewGetResult) -> (usize, usize) {
    let (mut a, mut d) = (0, 0);
    for f in &r.preview.files {
        for h in &f.hunks {
            for l in &h.lines {
                match l.kind {
                    DiffPreviewLineKind::Added => a += 1,
                    DiffPreviewLineKind::Removed => d += 1,
                    DiffPreviewLineKind::Context => {}
                }
            }
        }
    }
    (a, d)
}

/// `fileStatusMark` (`DiffReviewDialog.tsx:215-224`).
pub fn status_mark(status: &str) -> &'static str {
    match status {
        "added" => "A",
        "deleted" => "D",
        "renamed" => "R",
        "modified" => "M",
        _ => "?",
    }
}

fn wire<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()
}

// ------------------------------------------------------------- the marks

/// `fg` at `alpha` over the opaque `bg` (`color-mix(in srgb, fg a%,
/// transparent)` drawn on `bg`), as `#rrggbbff`.
pub fn mix(fg: &str, alpha: f64, bg: &str) -> String {
    let rgb = |h: &str| {
        let v = u32::from_str_radix(h.trim_start_matches('#').get(0..6).unwrap_or("000000"), 16).unwrap_or(0);
        [(v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff]
    };
    let (f, b) = (rgb(fg), rgb(bg));
    let c: Vec<u32> = (0..3).map(|i| (alpha * f[i] as f64 + (1.0 - alpha) * b[i] as f64).round() as u32).collect();
    format!("#{:02x}{:02x}{:02x}ff", c[0], c[1], c[2])
}

/// A word mark's fill: the success colour at 22 % on an added row, the error
/// colour at 20 % on a removed row, over that row's tint.
pub fn mark_fill(added: bool) -> String {
    if added {
        mix(SUCCESS, 0.22, tok::GREEN_BG)
    } else {
        mix(ERROR, 0.20, tok::RED_BG)
    }
}

/// The ink a token takes on a mark: its colour, stepped 5 % at a time toward
/// the text ink until it reads >= 4.5:1 on the mark (the A18 contrast rule;
/// the web's own pair reads 4.0:1 for a keyword on a removed mark).
pub fn mark_ink(t: Option<Tok>, added: bool) -> String {
    let base = token_ink(t);
    let fill = mark_fill(added);
    let mut step = 0.0;
    loop {
        let ink = mix(tok::TEXT, step, base);
        if crate::screens::theme::wcag_ratio(&ink, &fill) + 1e-9 >= 4.5 || step >= 1.0 {
            return ink;
        }
        step += 0.05;
    }
}

/// A token's ink on a plain row (the web's `--shiki-token-*`, light).
pub fn token_ink(t: Option<Tok>) -> &'static str {
    match t {
        None => tok::TEXT,
        Some(t) => t.color(false),
    }
}

/// A run's class in its widget id: `tx` = no grammar, else the token class.
pub fn class_id(t: Option<Tok>) -> &'static str {
    match t {
        None => "tx",
        Some(Tok::Plain) => "pl",
        Some(Tok::Keyword) => "kw",
        Some(Tok::String) => "st",
        Some(Tok::Comment) => "cm",
        Some(Tok::Constant) => "cn",
        Some(Tok::Function) => "fn",
        Some(Tok::Punctuation) => "pu",
    }
}

// --------------------------------------------------------------- the kit

/// The kit's mono face (the latin member of `ui::text_style(Face::Mono)`).
pub fn kit_mono() -> String {
    crate::design::font_file("ux/LiberationMono-Regular.ttf").display().to_string()
}

/// A28 — the code labels as two templates (`mod.widgets.B3DiffCode`,
/// `mod.widgets.B3DiffNum`): the kit's mono family (Noto Sans SC for CJK,
/// WenKai as the lazy fallback, the symbols and emoji faces), so a run costs
/// one short DSL line instead of the whole family. Registered by lib.rs
/// before any dialog mounts.
#[cfg(target_os = "macos")]
mod kit {
    use makepad_widgets::*;

    script_mod! {
        use mod.prelude.widgets.*

        let B3DiffMono = FontFamily{
            latin := FontMember{res: file_resource(#(crate::screens::board3::diff_review::kit_mono())) asc: 0 desc: 0 weight: 400}
            cjk := FontMember{res: file_resource(#(crate::design::cjk_face_path(400))) asc: 0.0 desc: 0.0 weight: 400}
            cjk_rare := FontMember{res: crate_resource("makepad_widgets:resources/LXGWWenKaiRegular.ttf") asc: 0.0 desc: 0.0 lazy: 1}
            symbols := FontMember{res: crate_resource("makepad_widgets:resources/jetbrains_mono_variable.ttf") asc: 0 desc: 0 weight: 400}
            emoji := FontMember{res: file_resource("/System/Library/Fonts/Apple Color Emoji.ttc") asc: 0 desc: 0}
        }
        mod.widgets.B3DiffCode = Label{
            width: Fit height: Fit padding: 0 flow: Right
            draw_text +: {
                color: #1d1d1f
                text_style: TextStyle{font_family: B3DiffMono font_size: #(crate::screens::board3::diff_review::CODE_PX * 0.75) line_spacing: #(crate::screens::board3::diff_review::CODE_SPACING)}
            }
        }
        mod.widgets.B3DiffNum = Label{
            width: Fit height: Fit padding: 0 flow: Right
            draw_text +: {
                color: #5f646b
                text_style: TextStyle{font_family: B3DiffMono font_size: #(crate::screens::board3::diff_review::NUM_PX * 0.75) line_spacing: #(crate::screens::board3::diff_review::NUM_SPACING)}
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod kit {
    use makepad_widgets::*;

    script_mod! {
        use mod.prelude.widgets.*

        let B3DiffMono = FontFamily{
            latin := FontMember{res: file_resource(#(crate::screens::board3::diff_review::kit_mono())) asc: 0 desc: 0 weight: 400}
            cjk := FontMember{res: file_resource(#(crate::design::cjk_face_path(400))) asc: 0.0 desc: 0.0 weight: 400}
            cjk_rare := FontMember{res: crate_resource("makepad_widgets:resources/LXGWWenKaiRegular.ttf") asc: 0.0 desc: 0.0 lazy: 1}
            symbols := FontMember{res: crate_resource("makepad_widgets:resources/jetbrains_mono_variable.ttf") asc: 0 desc: 0 weight: 400}
            emoji := FontMember{res: crate_resource("makepad_widgets:resources/NotoColorEmoji.ttf") asc: 0 desc: 0}
        }
        mod.widgets.B3DiffCode = Label{
            width: Fit height: Fit padding: 0 flow: Right
            draw_text +: {
                color: #1d1d1f
                text_style: TextStyle{font_family: B3DiffMono font_size: #(crate::screens::board3::diff_review::CODE_PX * 0.75) line_spacing: #(crate::screens::board3::diff_review::CODE_SPACING)}
            }
        }
        mod.widgets.B3DiffNum = Label{
            width: Fit height: Fit padding: 0 flow: Right
            draw_text +: {
                color: #5f646b
                text_style: TextStyle{font_family: B3DiffMono font_size: #(crate::screens::board3::diff_review::NUM_PX * 0.75) line_spacing: #(crate::screens::board3::diff_review::NUM_SPACING)}
            }
        }
    }
}

pub use kit::script_mod;

// -------------------------------------------------------------- geometry

/// The dialog's box: `(sheet, width, height)` — desktop `min(1080, 100%) ×
/// min(780, 100%)` of the frame less the 24 px backdrop inset; at <= 760 px
/// the whole frame (the compact full-screen sheet).
pub fn geometry(frame: &Frame) -> (bool, f64, f64) {
    if frame.avail_w <= SHEET_BELOW {
        return (true, frame.avail_w.floor(), frame.avail_h.floor());
    }
    let w = (frame.avail_w - 2.0 * INSET).min(MAX_W).max(240.0).floor();
    let h = (frame.avail_h - 2.0 * INSET).min(MAX_H).max(200.0).floor();
    (false, w, h)
}

/// The side padding of the header / status row / body (the web's 15/18 and
/// 16 px; 12 px on the compact sheet).
fn side_pad(sheet: bool) -> f64 {
    if sheet {
        16.0
    } else {
        20.0
    }
}

/// A13 — one diff row's width: the gutters (old, new, prefix), the widest
/// line's code (the mono face: 0.6 em per character, exact) and the row's
/// insets — never narrower than the file card (`min_w`), so a short hunk's
/// tints still span the card.
pub fn line_row_w<'a>(lines: impl Iterator<Item = &'a str>, min_w: f64) -> f64 {
    row_w(lines.map(|l| ui::text_w(l, CODE_PX, Face::Mono)).fold(0.0_f64, f64::max), 30.0, min_w)
}

fn row_w(code_w: f64, num_w: f64, min_w: f64) -> f64 {
    (12.0 + num_w + 6.0 + num_w + 6.0 + 10.0 + 6.0 + code_w + 2.0 + 12.0).max(min_w).ceil()
}

/// The gutter column's width for the preview's largest line number.
fn num_w(files: &[DiffPreviewFile]) -> f64 {
    let most = files
        .iter()
        .flat_map(|f| f.hunks.iter().flat_map(|h| h.lines.iter()))
        .map(|l| l.old_line.unwrap_or(0).max(l.new_line.unwrap_or(0)))
        .max()
        .unwrap_or(0);
    let digits = most.to_string().len() as f64;
    (digits * NUM_PX * 0.6 + 2.0).ceil().max(30.0)
}

// -------------------------------------------------------------- the view

fn empty_box(d: &mut Dsl, id: &str, head: Option<&str>, body: &str, alert: bool) {
    let (fill, border) = if alert { (tok::RED_BG, Some("#f5c2c7ff")) } else { (tok::SURFACE2, Some(tok::HAIRLINE)) };
    d.surface(id, "width: Fill height: Fit flow: Down spacing: 6 padding: Inset{left: 14 right: 14 top: 14 bottom: 14}", fill, 10.0, border);
    if let Some(h) = head {
        d.text(&format!("{id}_head"), h, &Txt::new(13.0, Face::Semibold, if alert { tok::RED_TEXT } else { tok::TEXT }).w(W::Fill).wrap());
    }
    d.text(&format!("{id}_body"), body, &Txt::new(12.5, Face::Regular, if alert { tok::TEXT } else { tok::MUTED }).w(W::Fill).wrap());
    d.close();
}

/// Open the backdrop + the review's box (the web's `.review-backdrop` +
/// `.review-dialog`; on the compact sheet no radius, no border).
fn shell_open(d: &mut Dsl, sheet: bool, w: f64, h: f64) {
    let ids = &ui::B3_IDS;
    d.open(ids.root, "KeyboardView", "width: Fill height: Fill flow: Overlay align: Align{x: 0.5 y: 0.5} keyboard_min_shift: 56.");
    d.rule(ids.backdrop, "width: Fill height: Fill", tok::MASK);
    d.view(ids.backdrop_box, "width: Fill height: Fill flow: Overlay");
    d.tap(ids.backdrop_hit, ids.backdrop_event.unwrap_or("b3.noop"));
    d.close();
    let (radius, border) = if sheet { (0.0, None) } else { (16.0, Some(tok::HAIRLINE)) };
    d.surface(ids.dialog, &format!("width: {w} height: {h} flow: Down padding: 0"), tok::SURFACE, radius, border);
}

pub fn build(d: &mut Dsl, st: &DiffReviewState, frame: &Frame, store: &Store) {
    let (sheet, width, height) = geometry(frame);
    let pad = side_pad(sheet);
    shell_open(d, sheet, width, height);

    // Header: eyebrow + title | totals · Refresh · close (the compact sheet
    // gives the title its own row and puts the controls under it).
    let preview = st.result.as_ref().map(|r| &r.preview);
    let title = preview
        .and_then(|p| p.title.clone())
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| tr(TITLE).to_owned());
    let totals_text = st.result.as_ref().map(totals).map(|(a, r)| (format!("+{a}"), format!("−{r}")));
    let refresh = tr("Refresh");
    let refresh_w = ui::text_w(refresh, 13.0, Face::Medium) + 32.0;
    let totals_w = totals_text
        .as_ref()
        .map(|(a, r)| ui::text_w(a, 13.0, Face::Semibold) + ui::text_w(r, 13.0, Face::Semibold) + 6.0 + 8.0)
        .unwrap_or(0.0);
    let can_refresh = !st.loading && st.preview_id.is_some() && advertised(store);
    let totals = |d: &mut Dsl| {
        if let Some((a, r)) = &totals_text {
            d.view("b3_diff_totals", "width: Fit height: Fit flow: Right spacing: 6 align: Align{x: 0.0 y: 0.5}");
            d.text("b3_diff_add", a, &Txt::new(13.0, Face::Semibold, tok::GREEN_TEXT));
            d.text("b3_diff_del", r, &Txt::new(13.0, Face::Semibold, tok::RED_TEXT));
            d.close();
        }
    };
    let controls = |d: &mut Dsl| {
        d.button("b3_diff_refresh", refresh, "b3.diff.refresh", if can_refresh { Btn::Outline } else { Btn::OutlineOff }, W::Fit, 30.0);
        ui::close_glyph(d, "b3.close");
    };
    if sheet {
        d.view("b3_diff_head", &format!("width: Fill height: Fit flow: Down spacing: 2 padding: Inset{{left: {pad} right: {pad} top: 14 bottom: 10}}"));
        d.text("b3_diff_eyebrow", tr(EYEBROW), &Txt::new(11.0, Face::Medium, tok::MUTED));
        d.text("b3_title", &ui::fit_w(&title, width - 2.0 * pad, 17.0, Face::Semibold), &ui::title().w(W::Fill));
        d.view("b3_diff_head_row", "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{top: 6}");
        totals(d);
        d.view("b3_diff_head_gap", "width: Fill height: 1");
        d.close();
        controls(d);
        d.close();
        d.close();
    } else {
        d.view("b3_diff_head", &format!("width: Fill height: Fit flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 8 padding: Inset{{left: {pad} right: 16 top: 16 bottom: 14}}"));
        d.view("b3_diff_head_col", "width: Fill height: Fit flow: Down spacing: 2");
        d.text("b3_diff_eyebrow", tr(EYEBROW), &Txt::new(11.0, Face::Medium, tok::MUTED));
        let budget = (width - pad - 16.0 - refresh_w - 8.0 - 28.0 - 8.0 - totals_w - 8.0).max(80.0);
        d.text("b3_title", &ui::fit_w(&title, budget, 17.0, Face::Semibold), &ui::title().w(W::Fill));
        d.close();
        totals(d);
        controls(d);
        d.close();
    }
    d.rule("b3_diff_head_rule", "width: Fill height: 1", tok::HAIRLINE);

    // The status row (`.review-status`): status · source · … the preview id.
    if let Some(pid) = &st.preview_id {
        let status = match &st.result {
            Some(r) => wire(&r.status),
            None if st.loading => "loading".to_owned(),
            None => "error".to_owned(),
        };
        d.view(
            "b3_diff_status_row",
            &format!("width: Fill height: Fit flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 8 padding: Inset{{left: {pad} right: {pad} top: 10 bottom: 6}}"),
        );
        d.chip("b3_diff_status", &status, tok::TEXT, tok::CHIP, None, true);
        let mut used = ui::text_w(&status, 11.0, Face::Mono) + 14.0;
        if let Some(r) = &st.result {
            let source = wire(&r.source);
            used += ui::text_w(&source, 11.0, Face::Mono) + 14.0 + 8.0;
            d.chip("b3_diff_source", &source, tok::TEXT, tok::CHIP, None, true);
        }
        if !sheet {
            d.view("b3_diff_status_gap", "width: Fill height: 1");
            d.close();
            let budget = (width - 2.0 * pad - used - 16.0).max(60.0);
            d.text("b3_diff_preview_id", &ui::fit_middle(pid, budget, 11.5, Face::Mono), &Txt::new(11.5, Face::Mono, tok::MUTED));
        }
        d.close();
    }

    // The body (`.review-content`): it fills the rest of the box and scrolls.
    // The right inset is the scroll bar's gutter.
    let body_pad = if sheet { 12.0 } else { pad };
    d.open(
        ui::B3_IDS.scroll,
        "ScrollYView",
        &format!("width: Fill height: Fill flow: Down padding: Inset{{left: {body_pad} top: 6 right: {body_pad} bottom: 16}}"),
    );
    d.view("b3_diff_body", "width: Fill height: Fit flow: Down spacing: 12");
    // The content width (a file card's): the box less the body's equal
    // insets (the scroll bar overlays the right one). Measured: a 941 box
    // with 20 + 18 insets laid its cards out 903 wide.
    let inner_w = width - 2.0 * body_pad;
    if st.preview_id.is_none() {
        // Native: the entry exists before any preview does.
        let body_text = if advertised(store) { NO_PREVIEW_BODY } else { NO_METHOD };
        d.surface("b3_diff_empty", "width: Fill height: Fit flow: Down spacing: 10 padding: Inset{left: 14 right: 14 top: 14 bottom: 14}", tok::SURFACE2, 10.0, Some(tok::HAIRLINE));
        d.text("b3_diff_empty_head", tr(NO_PREVIEW_HEAD), &Txt::new(13.0, Face::Semibold, tok::TEXT).w(W::Fill).wrap());
        d.text("b3_diff_empty_body", tr(body_text), &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
        if native_review_available(store) {
            d.button("b3_diff_native", NATIVE_REVIEW, "b3.diff.native", Btn::Outline, W::Fit, 32.0);
        }
        d.close();
    } else if st.loading {
        empty_box(d, "b3_diff_loading", None, tr(LOADING), false);
    } else if let Some(e) = &st.error {
        empty_box(d, "b3_diff_error", Some(tr(UNAVAILABLE)), e, true);
    } else if let Some(r) = &st.result {
        if r.preview.files.is_empty() {
            empty_box(d, "b3_diff_nofiles", None, tr(NO_FILES), false);
        } else {
            let body = body_cached(st, r, sheet, inner_w);
            d.raw(body.trim_end_matches('\n'));
        }
    }
    d.close();
    d.close();
    ui::shell_close(d);
}

/// The files' DSL, built once per (read, width, sheet, language).
fn body_cached(st: &DiffReviewState, r: &DiffPreviewGetResult, sheet: bool, inner_w: f64) -> Arc<String> {
    let files = &r.preview.files;
    let key = BodyKey {
        generation: st.generation,
        preview: st.preview_id.clone().unwrap_or_default(),
        lines: files.iter().flat_map(|f| f.hunks.iter()).map(|h| h.lines.len()).sum(),
        chars: files.iter().flat_map(|f| f.hunks.iter().flat_map(|h| h.lines.iter())).map(|l| l.content.len()).sum(),
        width: inner_w.round() as i64,
        sheet,
        lang: crate::i18n::generation() * 2 + u64::from(crate::i18n::is_zh()),
    };
    let mut slot = st.cache.0.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((k, body)) = slot.as_ref() {
        if *k == key {
            return body.clone();
        }
    }
    let body = Arc::new(files_dsl(files, inner_w));
    *slot = Some((key, body.clone()));
    body
}

/// Every file of a non-empty preview: the plain-text note past the bound,
/// then each file card with its hunks — decorated rows, or plain blocks.
pub fn files_dsl(files: &[DiffPreviewFile], inner_w: f64) -> String {
    let decorations = dw::decorate_preview(files);
    let nw = num_w(files);
    let mut d = Dsl::new();
    if decorations.is_none() {
        d.text("b3_diff_plain_note", tr(PLAIN_NOTE), &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    }
    for (fi, f) in files.iter().enumerate() {
        let id = format!("b3_diff_file_{fi}");
        d.surface(&id, "width: Fill height: Fit flow: Down spacing: 0", tok::SURFACE, 10.0, Some(tok::HAIRLINE));
        // The file's summary row (`<summary>`): mark, path, from, status.
        let status = f.status.as_wire_str().to_owned();
        d.view(&format!("{id}_head"), "width: Fill height: Fit flow: Down spacing: 2 padding: Inset{left: 12 right: 12 top: 10 bottom: 10}");
        d.view(&format!("{id}_head_row"), "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
        d.text(&format!("{id}_mark"), status_mark(&status), &Txt::new(12.0, Face::Semibold, tok::MUTED));
        let path_budget = (inner_w - 24.0 - 20.0 - ui::text_w(&status, 11.5, Face::Regular) - 16.0).max(60.0);
        d.text(&format!("{id}_path"), &ui::fit_w(&f.path, path_budget, 12.5, Face::Mono), &Txt::new(12.5, Face::Mono, tok::TEXT).w(W::Fill));
        d.text(&format!("{id}_status"), &status, &Txt::new(11.5, Face::Regular, tok::MUTED));
        d.close();
        if let Some(old) = &f.old_path {
            let from = format!("{} {old}", tr("from"));
            d.text(&format!("{id}_from"), &ui::fit_w(&from, inner_w - 24.0, 11.5, Face::Mono), &Txt::new(11.5, Face::Mono, tok::MUTED).w(W::Fill));
        }
        d.close();
        if f.hunks.is_empty() {
            d.rule(&format!("{id}_rule"), "width: Fill height: 1", tok::HAIRLINE);
            d.view(&format!("{id}_nolines_row"), "width: Fill height: Fit flow: Down padding: Inset{left: 12 right: 12 top: 10 bottom: 10}");
            d.text(&format!("{id}_nolines"), tr(NO_LINES), &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
            d.close();
        }
        for (hi, h) in f.hunks.iter().enumerate() {
            let hid = format!("{id}_h{hi}");
            // The hunk header on its band (`.diff-hunk-header`).
            d.surface(&format!("{hid}_band"), "width: Fill height: Fit flow: Down padding: Inset{left: 12 right: 12 top: 6 bottom: 6}", tok::BLUE_BG, 0.0, None);
            d.text(&format!("{hid}_header"), &h.header, &Txt::new(11.5, Face::Mono, tok::BLUE_TEXT).w(W::Fill).wrap());
            d.close();
            // A13 — a hunk's lines stay WHOLE and scroll sideways together
            // (the web's `.diff-hunk { overflow: auto }`, `.diff-lines
            // { min-width: max-content }`): every row is as wide as the
            // widest line, so the tints line up when scrolled.
            let code_w = h.lines.iter().map(|l| ui::text_w(&l.content, CODE_PX, Face::Mono)).fold(0.0_f64, f64::max);
            let rw = row_w(code_w, nw, inner_w - 2.0);
            // An overflowing hunk keeps room under its last line for the bar.
            let bottom = if rw > inner_w - 2.0 + 0.5 { 14 } else { 6 };
            d.open(
                &format!("{hid}_scroll"),
                "ScrollXView",
                "width: Fill height: Fit flow: Down\nscroll_bars.scroll_bar_x.bar_side_margin: 4\nscroll_bars.scroll_bar_x.draw_bg.color: #d1d1d6ff\nscroll_bars.scroll_bar_x.draw_bg.color_hover: #aeaeb2ff\nscroll_bars.scroll_bar_x.draw_bg.color_drag: #aeaeb2ff",
            );
            d.view(&format!("{hid}_lines"), &format!("width: Fit height: Fit flow: Down padding: Inset{{top: 4 bottom: {bottom}}}"));
            let mut out = String::new();
            match &decorations {
                Some(dec) => {
                    for (li, (l, toks)) in h.lines.iter().zip(&dec[fi][hi]).enumerate() {
                        decorated_row(&mut out, &format!("{hid}_l{li}"), l, toks, rw, nw);
                    }
                }
                None => plain_blocks(&mut out, &hid, &h.lines, rw, nw),
            }
            d.raw(out.trim_end_matches('\n'));
            d.close();
            d.close();
        }
        d.close();
    }
    d.finish()
}

/// A line's row fill, prefix and prefix ink (`.diff-added` / `.diff-removed`).
fn row_look(kind: DiffPreviewLineKind) -> (&'static str, &'static str, &'static str) {
    match kind {
        DiffPreviewLineKind::Added => (tok::GREEN_BG, "+", tok::GREEN_TEXT),
        DiffPreviewLineKind::Removed => (tok::RED_BG, "−", tok::RED_TEXT),
        DiffPreviewLineKind::Context => (tok::TRANSPARENT, " ", tok::MUTED),
    }
}

fn surface_props(fill: &str, radius: f64) -> String {
    format!(
        "show_bg: true draw_bg.color: {fill} draw_bg.radius: {radius} draw_bg.ellipse: 0 draw_bg.border_width: 0 draw_bg.border_position: 0 draw_bg.border_color: #00000000"
    )
}

fn num(n: Option<u32>) -> String {
    n.map(|n| n.to_string()).unwrap_or_default()
}

/// One decorated line: the tinted row, its gutters, and the code as runs —
/// a changed range's runs inside one rounded mark.
fn decorated_row(out: &mut String, lid: &str, l: &DiffPreviewLine, toks: &[DiffToken], rw: f64, nw: f64) {
    let (fill, prefix, ink) = row_look(l.kind);
    let added = l.kind == DiffPreviewLineKind::Added;
    let _ = writeln!(out, "{lid} := DesignSurface {{");
    let _ = writeln!(
        out,
        "width: {rw} height: {ROW_H} flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 6 padding: Inset{{left: 12 right: 12 top: 0 bottom: 0}}"
    );
    let _ = writeln!(out, "{}", surface_props(fill, 0.0));
    let _ = writeln!(out, "{lid}_old := mod.widgets.B3DiffNum{{width: {nw} text: {}}}", ui::lit(&num(l.old_line)));
    let _ = writeln!(out, "{lid}_new := mod.widgets.B3DiffNum{{width: {nw} text: {}}}", ui::lit(&num(l.new_line)));
    let _ = writeln!(out, "{lid}_prefix := mod.widgets.B3DiffCode{{width: 10 text: {} draw_text.color: {ink}}}", ui::lit(prefix));
    let _ = writeln!(out, "{lid}_code := View {{");
    let _ = writeln!(out, "width: Fit height: Fit flow: Right align: Align{{x: 0.0 y: 0.5}}");
    let mut mark: Option<usize> = None;
    let mut marks = 0usize;
    for (k, t) in toks.iter().enumerate() {
        if t.text.is_empty() {
            continue;
        }
        match (t.changed, mark.is_some()) {
            (true, false) => {
                let _ = writeln!(out, "{lid}_w{marks} := DesignSurface {{");
                let _ = writeln!(out, "width: Fit height: Fit flow: Right padding: Inset{{left: 0 right: 0 top: 1 bottom: 1}}");
                let _ = writeln!(out, "{}", surface_props(&mark_fill(added), 3.0));
                mark = Some(marks);
                marks += 1;
            }
            (false, true) => {
                out.push_str("}\n");
                mark = None;
            }
            _ => {}
        }
        let color = if t.changed {
            format!(" draw_text.color: {}", mark_ink(t.tok, added))
        } else {
            match t.tok {
                None | Some(Tok::Plain) => String::new(),
                Some(tk) => format!(" draw_text.color: {}", tk.color(false)),
            }
        };
        // A run inside a mark names it (`<line>_w<m>_c<k>_<class>`), so a
        // walk reads which words a mark holds even while it is scrolled out.
        let owner = match mark {
            Some(m) => format!("{lid}_w{m}"),
            None => lid.to_owned(),
        };
        let _ = writeln!(out, "{owner}_c{k}_{} := mod.widgets.B3DiffCode{{text: {}{color}}}", class_id(t.tok), ui::lit(&t.text));
    }
    if mark.is_some() {
        out.push_str("}\n");
    }
    out.push_str("}\n}\n");
}

/// Past the bound: each run of same-kind lines is ONE block — a multi-line
/// label per column on the line tint, the code red / green / plain (no
/// syntax, no marks). A line holding a line break gets a block of its own,
/// so the rows after it keep their numbers.
fn plain_blocks(out: &mut String, hid: &str, lines: &[DiffPreviewLine], rw: f64, nw: f64) {
    let mut n = 0usize;
    let mut i = 0usize;
    while i < lines.len() {
        let kind = lines[i].kind;
        let breaks = |l: &DiffPreviewLine| l.content.contains(['\n', '\r']);
        let mut j = i + 1;
        if !breaks(&lines[i]) {
            while j < lines.len() && j - i < BLOCK_LINES && lines[j].kind == kind && !breaks(&lines[j]) {
                j += 1;
            }
        }
        let block = &lines[i..j];
        let (fill, prefix, ink) = row_look(kind);
        let code_ink = match kind {
            DiffPreviewLineKind::Added => tok::GREEN_TEXT,
            DiffPreviewLineKind::Removed => tok::RED_TEXT,
            DiffPreviewLineKind::Context => tok::TEXT,
        };
        let col = |f: &dyn Fn(&DiffPreviewLine) -> String| block.iter().map(f).collect::<Vec<_>>().join("\n");
        let bid = format!("{hid}_b{n}");
        let _ = writeln!(out, "{bid} := DesignSurface {{");
        let _ = writeln!(
            out,
            "width: {rw} height: Fit flow: Right align: Align{{x: 0.0 y: 0.0}} spacing: 6 padding: Inset{{left: 12 right: 12 top: {BLOCK_PAD:.3} bottom: {BLOCK_PAD:.3}}}"
        );
        let _ = writeln!(out, "{}", surface_props(fill, 0.0));
        let _ = writeln!(out, "{bid}_old := mod.widgets.B3DiffNum{{width: {nw} text: {}}}", ui::lit(&col(&|l| num(l.old_line))));
        let _ = writeln!(out, "{bid}_new := mod.widgets.B3DiffNum{{width: {nw} text: {}}}", ui::lit(&col(&|l| num(l.new_line))));
        let _ = writeln!(out, "{bid}_prefix := mod.widgets.B3DiffCode{{width: 10 text: {} draw_text.color: {ink}}}", ui::lit(&col(&|_| prefix.to_owned())));
        let _ = writeln!(out, "{bid}_code := mod.widgets.B3DiffCode{{text: {} draw_text.color: {code_ink}}}", ui::lit(&col(&|l| l.content.clone())));
        out.push_str("}\n");
        n += 1;
        i = j;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_and_totals_follow_the_web() {
        assert_eq!(status_mark("added"), "A");
        assert_eq!(status_mark("modified"), "M");
        assert_eq!(status_mark("deleted"), "D");
        assert_eq!(status_mark("renamed"), "R");
        assert_eq!(status_mark("copied"), "?");
        let r: DiffPreviewGetResult = serde_json::from_value(json!({
            "status": "ready", "source": "pending_store",
            "preview": {"session_id": "s", "preview_id": "01920000-0000-7000-8000-0000000000f1", "files": [
                {"path": "a.rs", "status": "modified", "hunks": [{"header": "@@", "lines": [
                    {"kind": "added", "content": "x", "new_line": 1},
                    {"kind": "removed", "content": "y", "old_line": 1},
                    {"kind": "context", "content": "z", "old_line": 2, "new_line": 2}
                ]}]}
            ]}
        }))
        .unwrap();
        assert_eq!(totals(&r), (1, 1));
        assert_eq!(wire(&r.status), "ready");
        assert_eq!(wire(&r.source), "pending_store");
    }

    /// A13 (judge, 360 px: "Args::\nparse()") — a hunk's lines never wrap:
    /// they sit whole in one `ScrollXView`, every row as wide as the widest
    /// line (so the tints line up), and never narrower than the card. A28:
    /// the code is a row of single-line runs (no wrapping flow anywhere).
    #[test]
    fn hunk_lines_stay_whole_and_scroll_sideways_together() {
        let long = "    let args = Args::parse(); // the whole call stays on its line";
        let r: DiffPreviewGetResult = serde_json::from_value(json!({
            "status": "ready", "source": "pending_store",
            "preview": {"session_id": "s", "preview_id": "01920000-0000-7000-8000-0000000000f1", "files": [
                {"path": "src/main.rs", "status": "modified", "hunks": [{"header": "@@ -1,2 +1,2 @@", "lines": [
                    {"kind": "removed", "content": "fn main() {", "old_line": 1},
                    {"kind": "added", "content": long, "new_line": 1},
                    {"kind": "context", "content": "}", "old_line": 2, "new_line": 2}
                ]}]}
            ]}
        }))
        .unwrap();
        let st = DiffReviewState { preview_id: Some("01920000-0000-7000-8000-0000000000f1".into()), result: Some(r), ..Default::default() };
        for frame in [Frame::DESKTOP, Frame { avail_w: 360.0, avail_h: 780.0 }] {
            let mut d = Dsl::new();
            build(&mut d, &st, &frame, &Store::new());
            let dsl = d.finish();
            let at = dsl.find("b3_diff_file_0_h0_scroll := ScrollXView {").expect("one sideways box per hunk");
            let rows: Vec<&str> = (0..3)
                .map(|i| {
                    let lid = format!("b3_diff_file_0_h0_l{i} := DesignSurface {{\nwidth: ");
                    let p = dsl.find(&lid).unwrap_or_else(|| panic!("row {i}"));
                    assert!(p > at, "row {i} is inside the box");
                    dsl[p + lid.len()..].split(' ').next().unwrap()
                })
                .collect();
            assert!(rows.iter().all(|w| *w == rows[0]), "equal row widths: {rows:?}");
            let w: f64 = rows[0].parse().unwrap();
            assert!(w >= ui::text_w(long, 11.5, Face::Mono) + 100.0, "the widest line fits its row: {w}");
            let code = dsl.find("b3_diff_file_0_h0_l1_code := View {\nwidth: Fit height: Fit flow: Right").expect("the code is one Fit row");
            let end = dsl[code..].find("b3_diff_file_0_h0_l2 :=").unwrap() + code;
            assert!(!dsl[code..end].contains("wrap: true"), "never wraps");
            let joined: String = dsl[code..end]
                .lines()
                .filter_map(|l| l.split_once("text: \"").map(|(_, t)| t.rsplit_once('"').map(|(t, _)| t).unwrap_or(t)))
                .collect();
            assert_eq!(joined, long, "the runs hold the whole line");
            assert_eq!(dsl.matches('{').count(), dsl.matches('}').count(), "balanced");
        }
        assert_eq!(line_row_w(["x"].into_iter(), 500.0), 500.0, "never narrower than the card");
    }

    /// The marks' fills are the web's colours composited over the row tints,
    /// and every ink a run takes on a mark reads >= 4.5:1 on it, stepped
    /// from the token colour only where needed (the table is that rule).
    #[test]
    fn mark_fills_and_inks_follow_the_web_and_the_contrast_rule() {
        assert_eq!(mark_fill(true), MARK_ADDED);
        assert_eq!(mark_fill(false), MARK_REMOVED);
        for (t, on_added, on_removed) in MARK_INKS {
            assert_eq!(mark_ink(*t, true), *on_added, "{t:?} on an added mark");
            assert_eq!(mark_ink(*t, false), *on_removed, "{t:?} on a removed mark");
            for (ink, fill) in [(on_added, MARK_ADDED), (on_removed, MARK_REMOVED)] {
                assert!(crate::screens::theme::wcag_ratio(ink, fill) >= 4.5, "{t:?}: {ink} on {fill}");
            }
            // Unchanged wherever the token colour already reads.
            for (ink, fill) in [(on_added, MARK_ADDED), (on_removed, MARK_REMOVED)] {
                if crate::screens::theme::wcag_ratio(token_ink(*t), fill) >= 4.5 {
                    assert_eq!(&ink[..7], &token_ink(*t)[..7], "{t:?}");
                }
                // …and declared to the A18 contrast guard.
                use crate::screens::theme::{Swatch, CONTRAST_PAIRS};
                let declared = CONTRAST_PAIRS.iter().any(|p| {
                    matches!((p.ink, p.fill), (Swatch::Fixed(i), Swatch::Fixed(f)) if i == &ink[..7] && f == fill)
                });
                assert!(declared, "{t:?}: {ink} on {fill} is not in CONTRAST_PAIRS");
            }
        }
        // The row inks are declared on every row fill too.
        use crate::screens::theme::{Swatch, CONTRAST_PAIRS};
        for t in [Tok::Keyword, Tok::String, Tok::Comment, Tok::Constant, Tok::Function, Tok::Punctuation] {
            for fill in [tok::SURFACE, tok::GREEN_BG, tok::RED_BG] {
                let declared = CONTRAST_PAIRS.iter().any(|p| {
                    matches!((p.ink, p.fill), (Swatch::Fixed(i), Swatch::Fixed(f)) if i == &t.color(false)[..7] && f == fill)
                });
                assert!(declared, "{t:?} on {fill} is not in CONTRAST_PAIRS");
            }
        }
    }

    /// The block labels' rows sit exactly one decorated row apart, and the
    /// geometry follows the web's dialog and compact sheet.
    #[test]
    fn block_pitch_and_dialog_geometry() {
        assert!((CODE_PX * MONO_LINE_EM * CODE_SPACING - ROW_H).abs() < 1e-9);
        assert!((NUM_PX * MONO_LINE_EM * NUM_SPACING - ROW_H).abs() < 1e-9);
        assert_eq!(geometry(&Frame::DESKTOP), (false, 942.0, 555.0));
        assert_eq!(geometry(&Frame { avail_w: 1400.0, avail_h: 900.0 }), (false, 1080.0, 780.0));
        assert_eq!(geometry(&Frame { avail_w: 360.0, avail_h: 780.0 }), (true, 360.0, 780.0));
        assert_eq!(geometry(&Frame { avail_w: 760.0, avail_h: 700.0 }), (true, 760.0, 700.0));
    }
}
