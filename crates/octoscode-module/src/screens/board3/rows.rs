//! Board-3 screen 9 (+ screen 6's lower half) — the TRANSCRIPT ROWS the
//! conversation timeline gains (rows: timeline × 2, reasoning "folded
//! thinking blocks").
//!
//! The base row model is A1's `screen::timeline_rows` (user bubble, prose,
//! tools, tail) and stays byte-identical; [`timeline`] composes the board-3
//! rows around it, per turn:
//! * folded THINKING blocks after the turn's user bubble when the Session
//!   shows reasoning (`Timeline.tsx:262-299` ReasoningBlock: one-line
//!   summary, `{s} s · {words} words`, folded by default; the fold bar
//!   "Expand all / Collapse all" heads the transcript, `:142-163`);
//! * DELIVERED FILES after the answer — the turn's `file_attached` and the
//!   persisted answer's `meta.media` (`AttachmentList.tsx:12-26`: the file
//!   name, never inline in the body; Download; images preview);
//! * SYSTEM NOTICES at the turn's end — the store's `system.notice` entries
//!   (turn errors, non-clean terminals, `warning`, background completions,
//!   hydrated system rows: `entry-model.ts:70-78` `addSystemMessage`).
use std::sync::Arc;

use octoscode_store::timeline::{EntryKind, TimelineEntry};
use octoscode_store::Store;

use crate::components::ItemKind;
use crate::screen::Row;

use super::ui::{self, tok, Btn, Dsl, Face, Txt, W};

/// One transcript row: A1's base row, or a board-3 row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TRow {
    Base(Row),
    /// "Expand all / Collapse all".
    FoldBar,
    /// A folded reasoning block (timeline entry id).
    Thinking(u64),
    /// A system notice (timeline entry id).
    Notice(u64),
    /// A delivered file (timeline entry id).
    File(u64),
}

impl TRow {
    /// The kind A1's row spacing (`screen::lead_gap`) sees: a base row's own
    /// kind; the board-3 rows (fold bar, thinking, notice, file) space like
    /// answer prose, i.e. the web's timeline gap after a user bubble and none
    /// otherwise.
    pub fn layout_kind(&self) -> ItemKind {
        match self {
            TRow::Base(r) => r.kind,
            _ => ItemKind::AssistantProse,
        }
    }
}

#[derive(Default)]
struct Extras {
    thinking: Vec<u64>,
    files: Vec<u64>,
    notices: Vec<u64>,
}

/// The composed rows, in display order.
pub fn timeline(store: &Arc<Store>, live: bool) -> Vec<TRow> {
    timeline_folded(store, live, &[])
}

/// [`timeline`] over A1's folded base rows: a settled turn named in `folded`
/// keeps its "Worked for" header but hides its tool rows; the board-3 rows
/// (thinking, files, notices) compose onto whatever base rows remain, keyed by
/// turn id exactly as before (integration of A1 + A4).
pub fn timeline_folded(store: &Arc<Store>, live: bool, folded: &[String]) -> Vec<TRow> {
    let base = crate::screen::timeline_rows_folded(store, live, folded);
    let Some(session) = store.active_session() else {
        return base.into_iter().map(TRow::Base).collect();
    };
    let show = store.domains.session.thinking(&session).show_reasoning;
    let entries = store.domains.session.timeline.entries(&session);
    // Per turn (""= no turn): the board-3 entries in arrival order.
    let mut per_turn: Vec<(String, Extras)> = Vec::new();
    fn slot(per_turn: &mut Vec<(String, Extras)>, turn: &str) -> usize {
        match per_turn.iter().position(|(t, _)| t == turn) {
            Some(i) => i,
            None => {
                per_turn.push((turn.to_owned(), Extras::default()));
                per_turn.len() - 1
            }
        }
    }
    let mut any_thinking = false;
    for e in &entries {
        let t = e.turn_id.clone().unwrap_or_default();
        if e.kind == EntryKind::REASONING && show && !e.text.trim().is_empty() {
            let i = slot(&mut per_turn, &t);
            per_turn[i].1.thinking.push(e.id);
            any_thinking = true;
        } else if e.kind == EntryKind::ATTACHMENT {
            let i = slot(&mut per_turn, &t);
            per_turn[i].1.files.push(e.id);
        } else if e.kind == EntryKind::SYSTEM_NOTICE {
            let i = slot(&mut per_turn, &t);
            per_turn[i].1.notices.push(e.id);
        }
    }
    let mut out: Vec<TRow> = Vec::with_capacity(base.len() + 8);
    if any_thinking {
        out.push(TRow::FoldBar);
    }
    let take = |per_turn: &mut Vec<(String, Extras)>, turn: &str| -> Option<Extras> {
        let i = per_turn.iter().position(|(t, _)| t == turn)?;
        Some(std::mem::take(&mut per_turn[i].1))
    };
    let mut current: Option<String> = None;
    let mut pending_files: Vec<u64> = Vec::new();
    let mut pending_notices: Vec<u64> = Vec::new();
    let flush = |out: &mut Vec<TRow>, files: &mut Vec<u64>, notices: &mut Vec<u64>| {
        out.extend(files.drain(..).map(TRow::File));
        out.extend(notices.drain(..).map(TRow::Notice));
    };
    for row in base {
        let t = row.turn.clone().unwrap_or_default();
        if current.as_deref() != Some(t.as_str()) {
            // A new turn group: settle the previous turn's leftovers first.
            flush(&mut out, &mut pending_files, &mut pending_notices);
            current = Some(t.clone());
            if let Some(x) = take(&mut per_turn, &t) {
                pending_files = x.files;
                pending_notices = x.notices;
                if row.kind == ItemKind::UserBubble {
                    out.push(TRow::Base(row));
                    out.extend(x.thinking.into_iter().map(TRow::Thinking));
                    continue;
                }
                out.extend(x.thinking.into_iter().map(TRow::Thinking));
            }
        }
        // A1 orders a turn "the work, then its result" (worked-for + tool rows,
        // then the answer, then its actions), so a turn's delivered files and
        // notices follow the ANSWER, and settle before the actions row (or the
        // live working row) when a turn has no answer yet.
        match row.kind {
            ItemKind::AssistantProse => {
                out.push(TRow::Base(row));
                out.extend(pending_files.drain(..).map(TRow::File));
                out.extend(pending_notices.drain(..).map(TRow::Notice));
            }
            ItemKind::AnswerActions | ItemKind::WorkingRow => {
                out.extend(pending_files.drain(..).map(TRow::File));
                out.extend(pending_notices.drain(..).map(TRow::Notice));
                out.push(TRow::Base(row));
            }
            _ => out.push(TRow::Base(row)),
        }
    }
    flush(&mut out, &mut pending_files, &mut pending_notices);
    // Entries of turns with no base row at all (a notice-only turn).
    for (_, x) in per_turn {
        out.extend(x.thinking.into_iter().map(TRow::Thinking));
        out.extend(x.files.into_iter().map(TRow::File));
        out.extend(x.notices.into_iter().map(TRow::Notice));
    }
    out
}

fn entry(store: &Store, id: u64) -> Option<TimelineEntry> {
    let session = store.active_session()?;
    store.domains.session.timeline.entries(&session).into_iter().find(|e| e.id == id)
}

/// The notice's title and body (`entry-model.ts`): a `code: message` text
/// splits at the first ": "; terminal outcomes read as the web's titles.
pub fn notice_parts(e: &TimelineEntry) -> (String, String) {
    // A7: a client-authored notice (`Timeline::upsert_notice`, the web's
    // `addSystemMessage(…, title, body)`) names its own title.
    if let Some(title) = e.data.get("title").and_then(|t| t.as_str()) {
        let body = e.data.get("body").and_then(|b| b.as_str()).unwrap_or(&e.text);
        return (title.to_owned(), body.to_owned());
    }
    if let Some(outcome) = e.data.get("outcome").and_then(|o| o.as_str()) {
        let title = match outcome {
            "interrupted" => "Turn stopped",
            "rate_limited" => "Turn rate limited",
            "completed" => "Turn complete",
            _ => "Turn failed",
        };
        return (title.to_owned(), String::new());
    }
    if let (Some(code), Some(msg)) = (
        e.data.get("code").and_then(|c| c.as_str()),
        e.data.get("message").and_then(|m| m.as_str()),
    ) {
        let title = if code.is_empty() { "Warning".to_owned() } else { code.to_owned() };
        return (title, if msg.is_empty() { "The server reported a warning.".to_owned() } else { msg.to_owned() });
    }
    if e.data.get("kind").and_then(|k| k.as_str()) == Some("background_spawn_complete") {
        return ("Background task finished".to_owned(), e.text.clone());
    }
    ("System".to_owned(), e.text.clone())
}

/// The file name a reference shows (`attachments.ts:7-11`): the last path
/// segment, `file` when none.
pub fn file_name(reference: &str) -> String {
    reference
        .rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("file")
        .to_owned()
}

/// `attachments.ts:17-19`: previewable images (SVG deliberately excluded).
pub fn is_previewable(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp"].iter().any(|x| n.ends_with(x))
}

/// The size label (MiB with one decimal for ≥ 1 MiB, else KiB).
pub fn size_label(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MiB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{} KiB", (bytes as f64 / 1024.0).ceil() as u64)
    }
}

/// What a click anywhere on a board-3 row does (the host `row_hit` overlay
/// covers the row): a thinking block toggles its fold, a file downloads.
pub fn primary_action(row: &TRow) -> Option<String> {
    match row {
        TRow::Thinking(id) => Some(format!("b3.think.block.r{id}")),
        TRow::File(id) => Some(format!("b3.file.download#{id}")),
        _ => None,
    }
}

/// Download/preview state per file entry (UI-local).
pub fn file_state() -> std::sync::MutexGuard<'static, std::collections::HashMap<u64, FileState>> {
    use std::sync::{Mutex, OnceLock};
    static S: OnceLock<Mutex<std::collections::HashMap<u64, FileState>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Default::default())).lock().unwrap_or_else(|p| p.into_inner())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileState {
    pub busy: bool,
    pub saved_to: Option<String>,
    pub preview_path: Option<String>,
    pub error: Option<String>,
}

/// Lower one board-3 transcript row (the timeline item's DSL body).
pub fn lower(row: &TRow, store: &Store) -> String {
    let mut d = Dsl::new();
    match row {
        TRow::Base(_) => return String::new(),
        TRow::FoldBar => {
            d.view("b3_tl_fold", "width: Fill height: Fit flow: Down padding: Inset{left: 4 right: 21 top: 2 bottom: 6}");
            super::thinking::fold_bar(&mut d, "b3_tl_fold");
            d.close();
        }
        TRow::Thinking(id) => {
            let Some(e) = entry(store, *id) else { return String::new() };
            let session = store.active_session().unwrap_or_default();
            let prefs = store.domains.session.thinking(&session);
            let block = super::thinking::blocks(store, &session, None)
                .into_iter()
                .find(|b| b.key == format!("r{id}"));
            let Some(block) = block else { return String::new() };
            let open = prefs.expanded.contains(&block.key);
            d.view("b3_tl_think", "width: Fill height: Fit flow: Down padding: Inset{left: 4 right: 21 top: 4 bottom: 8}");
            super::thinking::block_view(&mut d, &format!("b3_tl_think_{id}"), &block, open, &format!("b3.think.block.r{id}"));
            d.close();
            let _ = e;
        }
        TRow::Notice(id) => {
            let Some(e) = entry(store, *id) else { return String::new() };
            let (title, body) = notice_parts(&e);
            d.view("b3_tl_notice", "width: Fill height: Fit flow: Down padding: Inset{left: 4 right: 21 top: 6 bottom: 10}");
            d.rule("", "width: Fill height: 1", tok::HAIRLINE);
            let row = d.anon();
            d.view(&row, "width: Fill height: Fit flow: Right spacing: 10 padding: Inset{top: 10}");
            d.surface("b3_tl_notice_icon_box", "width: 26 height: 26 flow: Overlay align: Align{x: 0.5 y: 0.5}", tok::SURFACE2, 13.0, Some(tok::HAIRLINE));
            d.icon("", "b3_info.svg", 14.0, tok::MUTED);
            d.close();
            let col = d.anon();
            d.view(&col, "width: Fill height: Fit flow: Down spacing: 4");
            d.text(&format!("b3_tl_notice_title_{id}"), &title, &Txt::new(13.0, Face::Medium, tok::TEXT).w(W::Fill));
            if !body.is_empty() {
                d.text(&format!("b3_tl_notice_body_{id}"), &body, &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
            }
            d.close();
            d.close();
            d.close();
        }
        TRow::File(id) => {
            let Some(e) = entry(store, *id) else { return String::new() };
            let path = e.data.get("path").and_then(|p| p.as_str()).unwrap_or(&e.text).to_owned();
            let name = file_name(&path);
            let size = e.data.get("size_bytes").and_then(|s| s.as_u64());
            let st = file_state().get(id).cloned().unwrap_or_default();
            d.view("b3_tl_file", "width: Fill height: Fit flow: Down padding: Inset{left: 4 right: 21 top: 4 bottom: 10}");
            d.surface(
                &format!("b3_tl_file_{id}"),
                "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 12 padding: Inset{left: 14 right: 14 top: 12 bottom: 12}",
                tok::SURFACE,
                12.0,
                Some(tok::HAIRLINE),
            );
            d.icon("", "b3_file.svg", 28.0, tok::TEXT);
            let col = d.anon();
            d.view(&col, "width: Fill height: Fit flow: Down spacing: 4");
            d.text(&format!("b3_tl_file_name_{id}"), &super::inventory::fit(&name, 300.0, 14.0, false), &Txt::new(14.0, Face::Regular, tok::TEXT).w(W::Fill));
            let meta = match (&st.saved_to, &st.error, size) {
                (Some(p), _, _) => format!("Saved to {}", ui::leaf(p)),
                (_, Some(err), _) => err.clone(),
                (_, _, Some(b)) => size_label(b),
                _ => e.data.get("mime").and_then(|m| m.as_str()).unwrap_or("").to_owned(),
            };
            d.text(&format!("b3_tl_file_meta_{id}"), &meta, &Txt::new(12.0, Face::Regular, if st.error.is_some() { tok::RED } else { tok::MUTED }).w(W::Fill));
            d.close();
            let btns = d.anon();
            d.view(&btns, "width: Fit height: Fit flow: Down spacing: 8");
            if is_previewable(&name) {
                d.button(&format!("b3_tl_file_preview_{id}"), "Preview", &format!("b3.file.preview#{id}"), if st.busy { Btn::Disabled } else { Btn::Outline }, W::Px(104.0), 32.0);
            }
            d.button(
                &format!("b3_tl_file_download_{id}"),
                if st.busy { "Downloading…" } else { "Download" },
                &format!("b3.file.download#{id}"),
                if st.busy { Btn::Disabled } else { Btn::Outline },
                W::Px(104.0),
                32.0,
            );
            d.close();
            d.close();
            if let Some(p) = &st.preview_path {
                d.gap(W::Fill, 8.0);
                d.open(
                    &format!("b3_tl_file_img_{id}"),
                    "Image",
                    &format!("width: Fill height: 220 fit: ImageFit.Smallest src: file_resource({p:?})"),
                );
                d.close();
            }
            d.close();
        }
    }
    d.finish()
}

/// Download (or preview) one delivered file through `GET /api/files`, saving
/// it under the user's Downloads folder (preview: a temp file the row shows).
pub async fn fetch(conv: &crate::flow::Conversation, id: u64, preview: bool) -> Result<String, String> {
    let Some(e) = entry(&conv.store, id) else { return Err("Invalid file reference".into()) };
    let path = e.data.get("path").and_then(|p| p.as_str()).unwrap_or(&e.text).to_owned();
    let name = file_name(&path);
    file_state().entry(id).or_default().busy = true;
    super::host::wake();
    let r = conv.download_file(&path).await;
    let mut st = file_state();
    let fs = st.entry(id).or_default();
    fs.busy = false;
    match r {
        Ok(bytes) => {
            let dir = if preview {
                std::env::temp_dir().join("octoscode-previews")
            } else {
                std::env::var("HOME")
                    .map(|h| std::path::PathBuf::from(h).join("Downloads"))
                    .unwrap_or_else(|_| std::env::temp_dir())
            };
            let _ = std::fs::create_dir_all(&dir);
            let target = dir.join(&name);
            std::fs::write(&target, &bytes).map_err(|e| e.to_string())?;
            let shown = target.to_string_lossy().into_owned();
            if preview {
                fs.preview_path = Some(shown.clone());
            } else {
                fs.saved_to = Some(shown.clone());
            }
            fs.error = None;
            Ok(format!("{} bytes", bytes.len()))
        }
        Err(err) => {
            fs.error = Some(err.clone());
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Arc<Store> {
        let s = Arc::new(Store::new());
        s.set_active(Some("s".into()));
        s.set_sessions(vec![octoscode_store::Session {
            id: "s".into(),
            title: None,
            message_count: 0,
            updated_at: None,
            last_prompt: None,
            active_turn: false,
        }]);
        s
    }

    #[test]
    fn rows_compose_around_the_base_model_per_turn() {
        let s = store();
        let tl = &s.domains.session.timeline;
        tl.upsert_user_message("s", "t1", "analyze the report", serde_json::json!({}));
        tl.append("s", Some("t1".into()), EntryKind::REASONING, "Weighed two approaches".into());
        tl.append("s", Some("t1".into()), EntryKind::ASSISTANT_TEXT, "Here it is.".into());
        let file = tl.append_data("s", Some("t1".into()), EntryKind::ATTACHMENT, "out/report.pdf".into(),
            serde_json::json!({"path": "out/report.pdf", "size_bytes": 1_258_291}));
        let notice = tl.append_data("s", Some("t1".into()), EntryKind::SYSTEM_NOTICE, "interrupted".into(),
            serde_json::json!({"outcome": "interrupted"}));
        s.domains.turn.set_terminal("t1", "interrupted");
        let rows = timeline(&s, false);
        let shape: Vec<String> = rows
            .iter()
            .map(|r| match r {
                TRow::Base(b) => b.kind.id().to_owned(),
                TRow::FoldBar => "fold".into(),
                TRow::Thinking(_) => "thinking".into(),
                TRow::Notice(_) => "notice".into(),
                TRow::File(_) => "file".into(),
            })
            .collect();
        assert_eq!(
            shape,
            // A1's base order: the work (worked-for), then the result (prose),
            // then the actions row; files and notices follow the answer.
            ["fold", "user-bubble", "thinking", "worked-for", "assistant-prose", "file", "notice", "answer-actions"]
        );
        assert!(rows.contains(&TRow::File(file)) && rows.contains(&TRow::Notice(notice)));
        // The base model is untouched underneath.
        assert_eq!(crate::screen::timeline_rows(&s, false).len(), 4);
    }

    #[test]
    fn hidden_reasoning_drops_the_blocks_and_the_fold_bar() {
        let s = store();
        let tl = &s.domains.session.timeline;
        tl.upsert_user_message("s", "t1", "hi", serde_json::json!({}));
        tl.append("s", Some("t1".into()), EntryKind::REASONING, "thinking".into());
        s.domains.session.set_show_reasoning("s", false);
        let rows = timeline(&s, false);
        assert!(!rows.iter().any(|r| matches!(r, TRow::Thinking(_) | TRow::FoldBar)));
    }

    #[test]
    fn notices_and_files_read_like_the_web() {
        let e = TimelineEntry {
            id: 1, turn_id: None, kind: EntryKind::SYSTEM_NOTICE, text: "x".into(),
            data: serde_json::json!({"code": "provider_overloaded", "message": "Retrying in 5s"}),
            closed: false, finalized: false,
        };
        assert_eq!(notice_parts(&e), ("provider_overloaded".into(), "Retrying in 5s".into()));
        assert_eq!(file_name("artifacts/plan.md"), "plan.md");
        assert_eq!(file_name(""), "file");
        assert!(is_previewable("a.PNG") && !is_previewable("a.svg"));
        assert_eq!(size_label(1_258_291), "1.2 MiB");
    }

    #[test]
    fn each_row_lowers_balanced() {
        let s = store();
        let tl = &s.domains.session.timeline;
        tl.upsert_user_message("s", "t1", "hi", serde_json::json!({}));
        let r = tl.append("s", Some("t1".into()), EntryKind::REASONING, "Checked the retry path".into());
        let f = tl.append_data("s", Some("t1".into()), EntryKind::ATTACHMENT, "a.png".into(), serde_json::json!({"path": "a.png"}));
        let n = tl.append("s", Some("t1".into()), EntryKind::SYSTEM_NOTICE, "Server restarted; reconnecting the session.".into());
        for row in [TRow::FoldBar, TRow::Thinking(r), TRow::File(f), TRow::Notice(n)] {
            let dsl = lower(&row, &s);
            assert!(!dsl.is_empty(), "{row:?}");
            assert_eq!(dsl.matches('{').count(), dsl.matches('}').count(), "{row:?}");
        }
        let taps = crate::screens::taps::wired_taps(&lower(&TRow::File(f), &s));
        assert!(taps.iter().any(|(_, e)| e == &format!("b3.file.download#{f}")));
        assert!(taps.iter().any(|(_, e)| e == &format!("b3.file.preview#{f}")));
    }
}
