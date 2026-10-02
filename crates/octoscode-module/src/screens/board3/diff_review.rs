//! A10 — the header's "Review" entry: the web's `DiffReviewDialog`
//! (`features/review/DiffReviewDialog.tsx`) over `use-coding-safety.ts:268-345`
//! (`openDiffReview` / `closeDiffReview`). No Stage-A board draws it: built
//! in the board-3 dialog kit (A5 style) — a centred modal, so its title never
//! meets the session header's.
//!
//! * the header: the eyebrow "Authoritative diff preview", the preview's
//!   title (else "Review changes"), the +N −N totals counted over the
//!   preview's lines once a result exists, Refresh (inert while loading) and
//!   the 28 px close;
//! * the status row: the result's status (else loading / error), its
//!   source, the preview id;
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
//! The web shows its "Review changes" entry only once a preview id is known;
//! the native header keeps its Review entry (board 2), so with no preview the
//! dialog says so and offers the server's native code review instead
//! (`/review`, the Code review dialog) when the server has it.
use octos_core::ui_protocol::{DiffPreviewGetResult, DiffPreviewLineKind};
use octoscode_store::Store;
use serde_json::json;

use super::host::{Job, Outcome};
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};

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
/// The rendering bound (the web renders every line; a native label per line
/// is bounded so a huge preview cannot stall the frame — the remainder is
/// counted, never silently dropped).
pub const MAX_LINES: usize = 1500;

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
            st.diff.result = Some(r);
            Ok(format!("{n} file(s)"))
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

// -------------------------------------------------------------------- view

fn empty_box(d: &mut Dsl, id: &str, head: Option<&str>, body: &str, alert: bool) {
    let (fill, border) = if alert { (tok::RED_BG, Some("#f5c2c7ff")) } else { (tok::SURFACE2, Some(tok::HAIRLINE)) };
    d.surface(id, "width: Fill height: Fit flow: Down spacing: 6 padding: Inset{left: 14 right: 14 top: 14 bottom: 14}", fill, 10.0, border);
    if let Some(h) = head {
        d.text(&format!("{id}_head"), h, &Txt::new(13.0, Face::Semibold, if alert { tok::RED } else { tok::TEXT }).w(W::Fill).wrap());
    }
    d.text(&format!("{id}_body"), body, &Txt::new(12.5, Face::Regular, if alert { tok::TEXT } else { tok::MUTED }).w(W::Fill).wrap());
    d.close();
}

pub fn build(d: &mut Dsl, st: &DiffReviewState, frame: &Frame, store: &Store) {
    let width = frame.dialog_w(760.0);
    let compact = frame.compact(width);
    let pad = ui::dialog_pad(frame, width);
    // The content width: the card less its padding and the scroll gutter.
    let inner_w = width - 2.0 * pad - 10.0;
    ui::shell_open(d, frame, width);

    // Header: eyebrow + title | totals · Refresh · close.
    let preview = st.result.as_ref().map(|r| &r.preview);
    let title = preview.and_then(|p| p.title.clone()).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| TITLE.to_owned());
    let totals = st.result.as_ref().map(totals);
    let refresh_w = ui::text_w("Refresh", 13.0, Face::Medium) + 32.0;
    let totals_text = totals.map(|(a, r)| (format!("+{a}"), format!("−{r}")));
    let totals_w = totals_text
        .as_ref()
        .map(|(a, r)| ui::text_w(a, 12.5, Face::Semibold) + ui::text_w(r, 12.5, Face::Semibold) + 6.0 + 10.0)
        .unwrap_or(0.0);
    let inline_totals = !compact;
    let right_w = refresh_w + 8.0 + 28.0 + if inline_totals { totals_w } else { 0.0 };
    let head = d.anon();
    d.view(&head, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 2");
    d.text("b3_diff_eyebrow", EYEBROW, &Txt::new(11.0, Face::Medium, tok::MUTED));
    let title_budget = (width - 2.0 * pad - right_w - 8.0).max(80.0);
    d.text("b3_title", &ui::fit_w(&title, title_budget, 16.0, Face::Semibold), &ui::title().w(W::Fill));
    d.close();
    if let (true, Some((a, r))) = (inline_totals, &totals_text) {
        d.text("b3_diff_add", a, &Txt::new(12.5, Face::Semibold, tok::GREEN));
        d.text("b3_diff_del", r, &Txt::new(12.5, Face::Semibold, tok::RED));
    }
    let can_refresh = !st.loading && st.preview_id.is_some() && advertised(store);
    d.button("b3_diff_refresh", "Refresh", "b3.diff.refresh", if can_refresh { Btn::Outline } else { Btn::OutlineOff }, W::Fit, 30.0);
    ui::close_glyph(d, "b3.close");
    d.close();
    if let (false, Some((a, r))) = (inline_totals, &totals_text) {
        let row = d.anon();
        d.view(&row, "width: Fill height: Fit flow: Right spacing: 6 padding: Inset{top: 4}");
        d.text("b3_diff_add", a, &Txt::new(12.5, Face::Semibold, tok::GREEN));
        d.text("b3_diff_del", r, &Txt::new(12.5, Face::Semibold, tok::RED));
        d.close();
    }

    // The status row (`.review-status`): status · source · the preview id.
    if let Some(pid) = &st.preview_id {
        let status = match &st.result {
            Some(r) => wire(&r.status),
            None if st.loading => "loading".to_owned(),
            None => "error".to_owned(),
        };
        let row = d.anon();
        d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{top: 8 bottom: 4}");
        d.chip("b3_diff_status", &status, tok::TEXT, tok::CHIP, None, false);
        let mut used = ui::text_w(&status, 12.0, Face::Medium) + 24.0;
        if let Some(r) = &st.result {
            let source = wire(&r.source);
            used += ui::text_w(&source, 12.0, Face::Medium) + 24.0 + 8.0;
            d.chip("b3_diff_source", &source, tok::MUTED, tok::SURFACE2, Some(tok::HAIRLINE), false);
        }
        let budget = (inner_w - used - 8.0).max(60.0);
        d.text("b3_diff_preview_id", &ui::fit_w(pid, budget, 11.5, Face::Mono), &Txt::new(11.5, Face::Mono, tok::MUTED).w(W::Fill));
        d.close();
    }
    d.gap(W::Fill, 8.0);

    ui::body_open(d, frame, width, if compact { 120.0 } else { 104.0 });
    let body = d.anon();
    d.view(&body, "width: Fill height: Fit flow: Down spacing: 12");
    if st.preview_id.is_none() {
        // Native: the entry exists before any preview does.
        let body_text = if advertised(store) { NO_PREVIEW_BODY } else { NO_METHOD };
        d.surface("b3_diff_empty", "width: Fill height: Fit flow: Down spacing: 10 padding: Inset{left: 14 right: 14 top: 14 bottom: 14}", tok::SURFACE2, 10.0, Some(tok::HAIRLINE));
        d.text("b3_diff_empty_head", NO_PREVIEW_HEAD, &Txt::new(13.0, Face::Semibold, tok::TEXT).w(W::Fill).wrap());
        d.text("b3_diff_empty_body", body_text, &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
        if native_review_available(store) {
            d.button("b3_diff_native", NATIVE_REVIEW, "b3.diff.native", Btn::Outline, W::Fit, 32.0);
        }
        d.close();
    } else if st.loading {
        empty_box(d, "b3_diff_loading", None, LOADING, false);
    } else if let Some(e) = &st.error {
        empty_box(d, "b3_diff_error", Some(UNAVAILABLE), e, true);
    } else if let Some(p) = preview {
        if p.files.is_empty() {
            empty_box(d, "b3_diff_nofiles", None, NO_FILES, false);
        }
        let mut drawn = 0usize;
        let total: usize = p.files.iter().map(|f| f.hunks.iter().map(|h| h.lines.len()).sum::<usize>()).sum();
        for (fi, f) in p.files.iter().enumerate() {
            let id = format!("b3_diff_file_{fi}");
            d.surface(&id, "width: Fill height: Fit flow: Down spacing: 0", tok::SURFACE, 10.0, Some(tok::HAIRLINE));
            // The file's summary row (`<summary>`): mark, path, from, status.
            let status = f.status.as_wire_str().to_owned();
            d.surface(&format!("{id}_head"), "width: Fill height: Fit flow: Down spacing: 2 padding: Inset{left: 12 right: 12 top: 9 bottom: 9}", tok::SURFACE2, 10.0, None);
            let row = d.anon();
            d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
            d.text(&format!("{id}_mark"), status_mark(&status), &Txt::new(12.0, Face::Semibold, tok::MUTED));
            let path_budget = (inner_w - 24.0 - 20.0 - ui::text_w(&status, 11.5, Face::Regular) - 16.0).max(60.0);
            d.text(&format!("{id}_path"), &ui::fit_w(&f.path, path_budget, 12.5, Face::Mono), &Txt::new(12.5, Face::Mono, tok::TEXT).w(W::Fill));
            d.text(&format!("{id}_status"), &status, &Txt::new(11.5, Face::Regular, tok::MUTED));
            d.close();
            if let Some(old) = &f.old_path {
                d.text(&format!("{id}_from"), &ui::fit_w(&format!("from {old}"), inner_w - 24.0, 11.5, Face::Mono), &Txt::new(11.5, Face::Mono, tok::MUTED).w(W::Fill));
            }
            d.close();
            if f.hunks.is_empty() {
                let pad_row = d.anon();
                d.view(&pad_row, "width: Fill height: Fit flow: Down padding: Inset{left: 12 right: 12 top: 10 bottom: 10}");
                d.text(&format!("{id}_nolines"), NO_LINES, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
                d.close();
            }
            for (hi, h) in f.hunks.iter().enumerate() {
                let hid = format!("{id}_h{hi}");
                let hrow = d.anon();
                d.view(&hrow, "width: Fill height: Fit flow: Down padding: Inset{left: 12 right: 12 top: 8 bottom: 4}");
                d.text(&format!("{hid}_header"), &h.header, &Txt::new(11.5, Face::Mono, tok::BLUE).w(W::Fill).wrap());
                d.close();
                let lines = d.anon();
                d.view(&lines, "width: Fill height: Fit flow: Down padding: Inset{bottom: 6}");
                for (li, l) in h.lines.iter().enumerate() {
                    if drawn >= MAX_LINES {
                        break;
                    }
                    drawn += 1;
                    let (fill, prefix, ink) = match l.kind {
                        DiffPreviewLineKind::Added => (tok::GREEN_BG, "+", tok::GREEN),
                        DiffPreviewLineKind::Removed => (tok::RED_BG, "−", tok::RED),
                        DiffPreviewLineKind::Context => (tok::TRANSPARENT, " ", tok::MUTED),
                    };
                    let lid = format!("{hid}_l{li}");
                    d.surface(&lid, "width: Fill height: Fit flow: Right spacing: 6 padding: Inset{left: 12 right: 12 top: 2 bottom: 2}", fill, 0.0, None);
                    let num = |n: Option<u32>| n.map(|n| n.to_string()).unwrap_or_default();
                    d.text(&format!("{lid}_old"), &num(l.old_line), &Txt::new(11.0, Face::Mono, tok::FAINT).w(W::Px(30.0)));
                    d.text(&format!("{lid}_new"), &num(l.new_line), &Txt::new(11.0, Face::Mono, tok::FAINT).w(W::Px(30.0)));
                    d.text(&format!("{lid}_prefix"), prefix, &Txt::new(11.5, Face::Mono, ink).w(W::Px(10.0)));
                    d.text(&format!("{lid}_code"), &l.content, &Txt::new(11.5, Face::Mono, tok::TEXT).w(W::Fill).wrap());
                    d.close();
                }
                d.close();
            }
            d.close();
        }
        if drawn < total {
            d.text(
                "b3_diff_more",
                &format!("{} more lines are not drawn here.", total - drawn),
                &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
            );
        }
    }
    d.close();
    ui::body_close(d);
    ui::shell_close(d);
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
}
