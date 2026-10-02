//! Board-3 screen 11 — HISTORY CHECKPOINTS (row: history × 1; the mutation
//! paths are `screens/history.rs`, P4f1).
//!
//! Web: `features/history/HistoryDialog.tsx` in its rewind mode, opened by
//! `/rewind` (alias `/backtrack`, `registry.ts:147-160`): checkpoints from the
//! canonical `session/hydrate` (`checkpoints.ts:15-57` — one per user-rooted
//! `thread_id`, oldest = #1, listed newest first, 180-char preview), a
//! confirmation box before the change ("Rewind to checkpoint #n?" + "Confirm
//! conversation rewind" / "Cancel"), the blocked-reason gate
//! (`history-binding.ts:97-129`), and after `session/rollback` the original
//! prompt returns to an EMPTY composer for editing — nothing is resent.
//!
//! The approved board adds the relative time per row and marks the live turn
//! (greyed, a check, no Restore): while a turn runs the newest checkpoint IS
//! that turn, and the web refuses to rewind an active Session anyway ("The
//! Session became active. Wait before rewinding."). "Copy as Markdown" is the
//! web's header export (`transcript-export/CopyConversationButton.tsx`),
//! reached here through the same `screens/transcript.rs` read.
use serde_json::Value;

use octoscode_store::Store;

use crate::screens::history::{self, ConversationCheckpoint, HistoryMode};

use super::host::Outcome;
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};

#[derive(Debug, Clone, Default)]
pub struct CkState {
    pub loading: bool,
    pub applying: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub rows: Vec<(ConversationCheckpoint, Option<u64>)>,
    pub live: bool,
    pub blocked: Option<String>,
    /// The row awaiting confirmation (index into `rows`).
    pub confirm: Option<usize>,
    pub copy_label: Option<String>,
    pub ticket: u64,
}

/// The first user message time of each checkpoint, from the hydrate rows
/// (`timestamp`/`created_at`, RFC 3339 or Unix seconds/ms).
fn checkpoint_times(messages: &[Value], cps: &[ConversationCheckpoint]) -> Vec<Option<u64>> {
    let time_of = |m: &Value| -> Option<u64> {
        for k in ["timestamp", "created_at", "at", "ts"] {
            match m.get(k) {
                Some(Value::String(s)) => {
                    if let Some(ms) = ui::parse_iso_ms(s) {
                        return Some(ms);
                    }
                }
                Some(Value::Number(n)) => {
                    let v = n.as_u64()?;
                    return Some(if v < 10_000_000_000 { v * 1000 } else { v });
                }
                _ => {}
            }
        }
        None
    };
    cps.iter()
        .map(|cp| {
            messages
                .iter()
                .filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
                .find(|m| {
                    let content = m.get("content").and_then(|c| c.as_str()).unwrap_or("");
                    content == cp.prefill
                })
                .and_then(time_of)
        })
        .collect()
}

pub async fn load(conv: &crate::flow::Conversation) -> Result<String, String> {
    let session = conv.session_id();
    let ticket = {
        let mut st = super::host::state();
        st.ck.ticket += 1;
        st.ck.loading = true;
        st.ck.error = None;
        st.ck.ticket
    };
    let blocked = history::blocked_reason(&conv.store, &session, HistoryMode::Rewind);
    let thread = history::read_history(conv.client(), &session).await;
    let mut st = super::host::state();
    if st.ck.ticket != ticket {
        return Ok("stale history read dropped".into());
    }
    st.ck.loading = false;
    st.ck.blocked = blocked;
    match thread {
        Ok(thread) => {
            let messages: Vec<Value> = thread
                .get("messages")
                .and_then(|m| m.as_array())
                .cloned()
                .unwrap_or_default();
            let cps = history::conversation_checkpoints(&messages);
            let times = checkpoint_times(&messages, &cps);
            st.ck.live = history::has_active_turn(&thread);
            st.ck.rows = cps.into_iter().zip(times).collect();
            st.ck.confirm = None;
            Ok(format!("{} checkpoints", st.ck.rows.len()))
        }
        Err(e) => {
            st.ck.error = Some(e.clone());
            Err(e)
        }
    }
}

/// Apply the confirmed rewind (`history::rewind_conversation`: fresh
/// hydrate, active-turn refusal, identity re-resolution, `session/rollback`,
/// canonical rehydration), then hand the prompt to an EMPTY composer.
pub async fn rewind(conv: &crate::flow::Conversation, key: String) -> Result<String, String> {
    let session = conv.session_id();
    let selected = {
        let st = super::host::state();
        st.ck.rows.iter().find(|(c, _)| c.key == key).map(|(c, _)| c.clone())
    };
    let Some(selected) = selected else {
        let mut st = super::host::state();
        st.ck.applying = false;
        st.ck.error = Some("History changed. Reload the checkpoint picker.".into());
        return Err("stale checkpoint".into());
    };
    let out = history::rewind_conversation(conv.client(), &conv.store, &session, &selected).await;
    {
        let mut st = super::host::state();
        st.ck.applying = false;
        st.ck.confirm = None;
        match &out {
            Ok(o) => {
                st.ck.notice = Some(o.notice.clone());
                st.ck.error = None;
            }
            Err(e) => st.ck.error = Some(e.clone()),
        }
    }
    let out = out?;
    // `history-coordinator`: the prefill goes to the composer ONLY when it is
    // empty; an existing draft is preserved.
    if let Some(prefill) = &out.prefill {
        let ui = conv.ui();
        let mut u = ui.lock().unwrap();
        if u.draft().trim().is_empty() {
            u.set_draft_inner(prefill.clone());
        }
    }
    let _ = load(conv).await;
    Ok(out.notice)
}

/// "Copy as Markdown" — the export read (`screens/transcript.rs`); the host
/// writes the text to the clipboard on the UI thread.
pub async fn copy_markdown(conv: &crate::flow::Conversation) -> Result<String, String> {
    let r = crate::screens::transcript::perform(conv, &conv.store).await;
    let mut st = super::host::state();
    match r {
        Ok(md) if md.is_empty() => {
            st.ck.copy_label = Some("Nothing to copy".into());
            Ok("nothing to copy".into())
        }
        Ok(md) => {
            st.ck.copy_label = Some("Copied".into());
            st.pending_clipboard = Some(md.clone());
            Ok(format!("{} bytes", md.len()))
        }
        Err(e) => {
            st.ck.copy_label = Some("Copy failed".into());
            Err(e)
        }
    }
}

pub fn perform(st: &mut CkState, action: &str, index: usize) -> Outcome {
    match action {
        "b3.ck.restore" => {
            if st.blocked.is_some() || st.applying || index >= st.rows.len() {
                return Outcome::Done;
            }
            if st.live && index == 0 {
                return Outcome::Done; // the live turn is never a rewind target
            }
            st.confirm = Some(index);
            st.notice = None;
            Outcome::Done
        }
        "b3.ck.cancel" => {
            st.confirm = None;
            Outcome::Done
        }
        "b3.ck.confirm" => match st.confirm.and_then(|i| st.rows.get(i)) {
            Some((cp, _)) => {
                st.applying = true;
                Outcome::Spawn(super::host::Job::Rewind(cp.key.clone()))
            }
            None => Outcome::Done,
        },
        "b3.ck.reload" => Outcome::Spawn(super::host::Job::CheckpointsLoad),
        "b3.ck.copy_md" => {
            st.copy_label = Some("Copying…".into());
            Outcome::Spawn(super::host::Job::CopyMarkdown)
        }
        _ => Outcome::Unrouted,
    }
}

// -------------------------------------------------------------------- view


pub fn build(d: &mut Dsl, st: &CkState, frame: &Frame, store: &Store) {
    let width = frame.dialog_w(720.0);
    let pad = ui::dialog_pad(frame, width);
    let inner_w = width - 2.0 * pad;
    ui::shell_open(d, frame, width);
    ui::header(d, "Conversation history", "b3.close");
    let session = store.domains.session.active().unwrap_or_default();
    d.text("b3_ck_scope", &session, &Txt::new(11.5, Face::Mono, tok::MUTED).w(W::Fill));
    d.gap(W::Fill, 12.0);
    ui::body_open(d, frame, width, 64.0);
    if st.loading {
        d.text("b3_ck_loading", "Loading server history…", &ui::meta());
    }
    if st.applying {
        d.text("b3_ck_applying", "Applying and refreshing the owning Session…", &ui::meta());
    }
    if let Some(why) = &st.blocked {
        d.text("b3_ck_blocked", why, &Txt::new(12.0, Face::Regular, tok::AMBER).w(W::Fill).wrap());
    }
    if let Some(e) = &st.error {
        d.text("b3_ck_error", e, &Txt::new(12.0, Face::Regular, tok::RED).w(W::Fill).wrap());
    }
    if let Some(n) = &st.notice {
        d.text("b3_ck_notice", n, &Txt::new(12.0, Face::Regular, tok::GREEN).w(W::Fill).wrap());
    }
    if st.rows.is_empty() && !st.loading {
        d.text("b3_ck_empty", "No user turns to rewind.", &ui::meta());
    }
    // One bordered list, hairlines between rows (the board's grouped box).
    if !st.rows.is_empty() {
        d.surface("b3_ck_list", "width: Fill height: Fit flow: Down", tok::SURFACE, 12.0, Some(tok::HAIRLINE));
        let now = ui::now_ms();
        for (i, (cp, at)) in st.rows.iter().enumerate() {
            if i > 0 {
                d.hairline();
            }
            let live = st.live && i == 0;
            let rid = format!("b3_ck_row_{i}");
            d.view(&rid, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 12 padding: Inset{left: 14 right: 14 top: 11 bottom: 11}");
            let num_color = if live { tok::FAINT } else { tok::TEXT };
            d.text(&format!("{rid}_num"), &format!("#{}", cp.checkpoint), &Txt::new(13.0, Face::Mono, num_color).w(W::Px(44.0)));
            let col = d.anon();
            d.view(&col, "width: Fill height: Fit flow: Down spacing: 3");
            let when = match at {
                Some(ms) => {
                    let r = ui::rel_time(now, *ms);
                    if r == "now" { "just now".to_owned() } else { format!("{r} ago") }
                }
                None => format!(
                    "{} message{}",
                    cp.user_message_count,
                    if cp.user_message_count == 1 { "" } else { "s" }
                ),
            };
            d.text(&format!("{rid}_when"), &when, &Txt::new(11.5, Face::Regular, tok::MUTED));
            let preview = if cp.preview.is_empty() { "(attachment prompt)".to_owned() } else { cp.preview.clone() };
            let label = if live { "Current live turn".to_owned() } else { preview };
            d.text(
                &format!("{rid}_preview"),
                &super::inventory::fit(&label, inner_w - 160.0, 13.0, false),
                &Txt::new(13.0, Face::Regular, if live { tok::FAINT } else { tok::TEXT }).w(W::Fill),
            );
            d.close();
            if live {
                d.icon(&format!("{rid}_check"), "b3_check.svg", 18.0, tok::TEXT);
            } else if st.blocked.is_none() {
                d.link(&format!("{rid}_restore"), "Restore", Some(&format!("b3.ck.restore#{i}")), 13.0);
            } else {
                d.text("", "Restore", &Txt::new(13.0, Face::Regular, tok::FAINT));
            }
            d.close();
        }
        d.close();
    }
    if let Some(i) = st.confirm {
        if let Some((cp, _)) = st.rows.get(i) {
            d.gap(W::Fill, 12.0);
            d.surface("b3_ck_confirm", "width: Fill height: Fit flow: Down spacing: 8 padding: Inset{left: 14 right: 14 top: 12 bottom: 12}", tok::SURFACE2, 12.0, Some(tok::HAIRLINE));
            d.text("b3_ck_confirm_q", &format!("Rewind to checkpoint #{}?", cp.checkpoint), &Txt::new(13.0, Face::Medium, tok::TEXT).w(W::Fill));
            if cp.user_message_count > 1 {
                d.text(
                    "",
                    &format!("This turn contains {} user messages. They belong to one thread and will be removed together.", cp.user_message_count),
                    &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
                );
            }
            if cp.media_count > 0 {
                d.text("", "The prompt contained attachments; reattach them before resending.", &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
            }
            let row = d.anon();
            d.view(&row, "width: Fill height: Fit flow: Right spacing: 8 align: Align{x: 1.0 y: 0.5}");
            d.button("b3_ck_cancel", "Cancel", "b3.ck.cancel", Btn::Outline, W::Fit, 32.0);
            let kind = if st.applying { Btn::Disabled } else { Btn::Primary };
            d.button("b3_ck_confirm_btn", "Confirm conversation rewind", "b3.ck.confirm", kind, W::Fit, 32.0);
            d.close();
            d.close();
        }
    }
    d.gap(W::Fill, 12.0);
    d.text(
        "b3_ck_note",
        "Restoring removes that turn and every later one; its prompt returns to the composer to edit and resend. Workspace files are not restored.",
        &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
    );
    d.gap(W::Fill, 10.0);
    let foot = d.anon();
    d.view(&foot, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 14");
    d.link("b3_ck_copy_md", "Copy as Markdown", Some("b3.ck.copy_md"), 13.0);
    if let Some(l) = &st.copy_label {
        d.text("b3_ck_copy_state", l, &Txt::new(12.0, Face::Regular, tok::MUTED));
    }
    d.close();
    ui::body_close(d);
    ui::shell_close(d);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cp(n: usize, key: &str, prefill: &str) -> ConversationCheckpoint {
        ConversationCheckpoint {
            key: key.into(),
            checkpoint: n,
            preview: prefill.into(),
            prefill: prefill.into(),
            media_count: 0,
            user_message_count: 1,
        }
    }

    #[test]
    fn restore_asks_for_confirmation_and_the_live_turn_never_arms() {
        let mut st = CkState {
            rows: vec![(cp(2, "k2", "Refactored queue"), None), (cp(1, "k1", "Initial draft"), None)],
            live: true,
            ..Default::default()
        };
        assert_eq!(perform(&mut st, "b3.ck.restore", 0), Outcome::Done);
        assert_eq!(st.confirm, None, "the live turn is not a target");
        perform(&mut st, "b3.ck.restore", 1);
        assert_eq!(st.confirm, Some(1));
        assert_eq!(
            perform(&mut st, "b3.ck.confirm", 0),
            Outcome::Spawn(super::super::host::Job::Rewind("k1".into()))
        );
        assert!(st.applying);
        perform(&mut st, "b3.ck.cancel", 0);
        assert_eq!(st.confirm, None);
    }

    #[test]
    fn a_blocked_history_arms_nothing() {
        let mut st = CkState { rows: vec![(cp(1, "k1", "x"), None)], blocked: Some("Wait".into()), ..Default::default() };
        perform(&mut st, "b3.ck.restore", 0);
        assert_eq!(st.confirm, None);
    }

    #[test]
    fn times_come_from_the_hydrated_user_rows() {
        let msgs = vec![
            json!({"role": "user", "content": "Initial draft", "thread_id": "t1", "seq": 0, "timestamp": "2026-10-01T10:00:00Z"}),
            json!({"role": "user", "content": "Refactored queue", "thread_id": "t2", "seq": 2, "timestamp": 1_759_316_400}),
        ];
        let cps = history::conversation_checkpoints(&msgs);
        let times = checkpoint_times(&msgs, &cps);
        assert_eq!(cps[0].checkpoint, 2, "newest first");
        assert_eq!(times[0], Some(1_759_316_400_000));
        assert_eq!(times[1], ui::parse_iso_ms("2026-10-01T10:00:00Z"));
    }

    #[test]
    fn the_dialog_lowers_balanced_with_row_taps() {
        let st = CkState {
            rows: vec![(cp(2, "k2", "Refactored queue"), None), (cp(1, "k1", "Initial"), None)],
            confirm: Some(1),
            ..Default::default()
        };
        let mut d = Dsl::new();
        build(&mut d, &st, &Frame::DESKTOP, &Store::new());
        let dsl = d.finish();
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
        let taps = crate::screens::taps::wired_taps(&dsl);
        for ev in ["b3.ck.restore#0", "b3.ck.restore#1", "b3.ck.cancel", "b3.ck.confirm", "b3.ck.copy_md", "b3.close"] {
            assert!(taps.iter().any(|(_, e)| e == ev), "{ev}");
        }
    }
}
