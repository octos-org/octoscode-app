//! Board-3 screen 6 — REASONING EFFORT + folded thinking (rows: reasoning × 4).
//!
//! Web: `features/reasoning/ReasoningDialog.tsx` (`/thinking`, alias
//! `/think`, `registry.ts:245-253`; an argument `low|medium|high|max|default|
//! reset` sets the effort without UI, `composer/intent.ts:147-152`), the
//! effort model `reasoning/model.ts:13-19` ("" = Profile default), the
//! show-thinking preference `reasoning/show-thinking.ts:6-14` (default ON,
//! malformed -> ON: fails closed), and the transcript's `ReasoningBlock`
//! (`timeline/Timeline.tsx:262-299`: "Thought process", `{s} s · {words}
//! words`, folded by default, "Expand all" / "Collapse all").
//!
//! The effort is captured per Session and rides EVERY `turn/start` as
//! `reasoning_effort` (omitted for Profile default) — `flow.rs`
//! `start_turn_with_id` reads [`effort_param`]. The Session's initial effort
//! is the `session/open` reply's `reasoning_effort`
//! (`session-composer-drafts.ts:40`), seeded by the flow.
use octoscode_store::domains::session::ThinkingPrefs;
use octoscode_store::timeline::EntryKind;
use octoscode_store::Store;

use super::host::Outcome;
use super::ui::{self, tok, Dsl, Face, Frame, Seg, Txt, W};
use crate::i18n::{tr, tr_with};

pub const EFFORTS: [&str; 4] = ["low", "medium", "high", "max"];

/// The `reasoning_effort` a new prompt carries, or `None` for the Profile
/// default (`use-turn-controller.ts:398-410`: omitted unless set).
pub fn effort_param(store: &Store, session: &str) -> Option<String> {
    let e = store.domains.session.thinking(session).effort;
    EFFORTS.contains(&e.as_str()).then_some(e)
}

/// `/thinking <arg>` (`composer/intent.ts:147-152`): a level sets the
/// override, `default`/`reset` clears it, anything else is refused with the
/// web's report text.
pub fn apply_arg(store: &Store, session: &str, arg: &str) -> Result<String, String> {
    let a = arg.trim().to_ascii_lowercase();
    if EFFORTS.contains(&a.as_str()) {
        store.domains.session.set_thinking_effort(session, &a);
        Ok(a)
    } else if a == "default" || a == "reset" {
        store.domains.session.set_thinking_effort(session, "");
        Ok("Profile default".into())
    } else {
        Err("Arguments for /thinking are not supported in this Web build. Open the command without arguments to use its controls. Nothing was sent to the model.".into())
    }
}

/// One folded reasoning block of the Session's transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// `r<entry id>` — the fold key the transcript and this panel share.
    pub key: String,
    pub summary: String,
    pub body: String,
    pub words: usize,
    pub seconds: Option<u64>,
    pub live: bool,
}

/// `folds.ts:47-51`: words = the body split on whitespace.
pub fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

/// The one-line summary: the first non-empty line, trimmed of markdown
/// emphasis, cut at ~72 chars.
pub fn summary_of(s: &str) -> String {
    let line = s
        .lines()
        .map(|l| l.trim().trim_start_matches(['#', '*', '-', ' ']).trim_end_matches('*'))
        .find(|l| !l.is_empty())
        .unwrap_or(tr("Thought process"));
    super::inventory::fit(line, 72.0 * 7.0, 13.0, false)
}

/// The Session's reasoning blocks, oldest first.
pub fn blocks(store: &Store, session: &str, live_turn: Option<&str>) -> Vec<Block> {
    store
        .domains
        .session
        .timeline
        .entries(session)
        .into_iter()
        .filter(|e| e.kind == EntryKind::REASONING && !e.text.trim().is_empty())
        .map(|e| {
            let started = e.data.get("started_ms").and_then(|v| v.as_u64());
            let ended = e.data.get("ended_ms").and_then(|v| v.as_u64());
            Block {
                key: format!("r{}", e.id),
                summary: summary_of(&e.text),
                words: word_count(&e.text),
                seconds: match (started, ended) {
                    (Some(a), Some(b)) if b >= a => Some(((b - a) + 500) / 1000),
                    _ => None,
                },
                live: !e.closed && live_turn.is_some() && e.turn_id.as_deref() == live_turn,
                body: e.text,
            }
        })
        .collect()
}

/// The meta line (`Timeline.tsx:281-287`).
pub fn meta(b: &Block) -> String {
    match b.seconds {
        Some(s) => tr_with("{seconds} s · {words} words", &[("seconds", &s.to_string()), ("words", &b.words.to_string())]),
        None => tr_with("{words} words", &[("words", &b.words.to_string())]),
    }
}

// ------------------------------------------------------- show-thinking pref

/// The browser preference's native home: `$HOME/.octoscode/show-thinking.json`
/// holding `true`/`false` (the web's `octoscode.web.show-thinking.v1`).
fn pref_path() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("OCTOSCODE_SHOW_THINKING_FILE") {
        return Some(p.into());
    }
    std::env::var("HOME").ok().map(|h| std::path::Path::new(&h).join(".octoscode/show-thinking.json"))
}

/// Read the preference: only the exact text `false` turns it off — anything
/// missing or malformed is ON (`show-thinking.ts:6-14`, fails closed).
pub fn parse_pref(raw: Option<&str>) -> bool {
    !matches!(raw.map(str::trim), Some("false"))
}

pub fn load_pref() -> bool {
    parse_pref(pref_path().and_then(|p| std::fs::read_to_string(p).ok()).as_deref())
}

/// Apply the stored preference to the FIRST opened Session only; every later
/// Session starts from the default (`App.tsx:524-537`,
/// `session-composer-drafts.ts:41`). The pane's "Default on" toggle mirrors
/// the preference.
pub fn apply_pref_once(store: &Store, session: &str) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static APPLIED: AtomicBool = AtomicBool::new(false);
    let pref = load_pref();
    store.domains.session.set_thinking_default_on(session, pref);
    if !APPLIED.swap(true, Ordering::SeqCst) {
        store.domains.session.set_show_reasoning(session, pref);
    }
}

pub fn save_pref(on: bool) -> bool {
    let Some(p) = pref_path() else { return false };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(p, if on { "true" } else { "false" }).is_ok()
}

// ------------------------------------------------------------------ actions

/// Every `b3.think.*` action against the ACTIVE Session's prefs.
pub fn perform(store: &Store, session: &str, action: &str) -> Outcome {
    let prefs = store.domains.session.thinking(session);
    match action {
        a if a.starts_with("b3.think.effort.") => {
            let level = a.trim_start_matches("b3.think.effort.");
            if level == "default" {
                store.domains.session.set_thinking_effort(session, "");
            } else if EFFORTS.contains(&level) {
                store.domains.session.set_thinking_effort(session, level);
            }
            Outcome::Done
        }
        "b3.think.show" => {
            store.domains.session.set_show_reasoning(session, !prefs.show_reasoning);
            Outcome::Done
        }
        "b3.think.default" => {
            let next = !prefs.default_on;
            store.domains.session.set_thinking_default_on(session, next);
            // The pane toggle writes the browser preference (App.tsx:3301-3306).
            save_pref(next);
            Outcome::Done
        }
        "b3.think.expand_all" => {
            let keys: Vec<String> = blocks(store, session, None).into_iter().map(|b| b.key).collect();
            store.domains.session.set_thinking_expanded(session, keys);
            Outcome::Done
        }
        "b3.think.collapse_all" => {
            store.domains.session.set_thinking_expanded(session, Vec::new());
            Outcome::Done
        }
        a if a.starts_with("b3.think.block.") => {
            let key = a.trim_start_matches("b3.think.block.").to_owned();
            toggle_block(store, session, &key);
            Outcome::Done
        }
        _ => Outcome::Unrouted,
    }
}

/// Toggle one fold (shared with the transcript's thinking rows).
pub fn toggle_block(store: &Store, session: &str, key: &str) {
    let mut expanded = store.domains.session.thinking(session).expanded;
    if expanded.iter().any(|k| k == key) {
        expanded.retain(|k| k != key);
    } else {
        expanded.push(key.to_owned());
    }
    store.domains.session.set_thinking_expanded(session, expanded);
}

// -------------------------------------------------------------------- view

fn effort_index(p: &ThinkingPrefs) -> Option<usize> {
    EFFORTS.iter().position(|e| *e == p.effort)
}

fn toggle_row(d: &mut Dsl, id: &str, label: &str, help: &str, on: bool, event: &str) {
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 12 padding: Inset{top: 12 bottom: 12}");
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 3");
    d.text(&format!("{id}_label"), label, &Txt::new(13.5, Face::Regular, tok::TEXT).w(W::Fill));
    if !help.is_empty() {
        d.text(&format!("{id}_help"), help, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    }
    d.close();
    d.toggle(id, on, event);
    d.close();
}

/// One folded thinking block (the panel's preview and the transcript row
/// share this look). `event` toggles it.
pub fn block_view(d: &mut Dsl, id: &str, b: &Block, open: bool, event: &str) {
    d.surface(id, "width: Fill height: Fit flow: Down", tok::SURFACE, 10.0, Some(tok::HAIRLINE));
    d.view(&format!("{id}_head"), "width: Fill height: 42 flow: Overlay");
    let row = d.anon();
    d.view(&row, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{left: 12 right: 12 top: 0 bottom: 0}");
    d.icon(
        &format!("{id}_chev"),
        if open { "b3_chevron_down_dark.svg" } else { "b3_chevron_right_dark.svg" },
        14.0,
        tok::TEXT,
    );
    let label = if b.live { tr("Thinking…").to_owned() } else { b.summary.clone() };
    d.text(&format!("{id}_summary"), &label, &Txt::new(13.0, Face::Medium, tok::TEXT).w(W::Fill));
    d.text(&format!("{id}_meta"), &meta(b), &Txt::new(11.5, Face::Regular, tok::MUTED));
    d.close();
    d.tap(&format!("{id}_tap"), event);
    d.close();
    if open {
        d.rule(&format!("{id}_rule"), "width: Fill height: 1", tok::HAIRLINE);
        let body = d.anon();
        d.view(&body, "width: Fill height: Fit flow: Down spacing: 6 padding: Inset{left: 34 right: 14 top: 10 bottom: 12}");
        for (i, line) in b.body.lines().filter(|l| !l.trim().is_empty()).take(12).enumerate() {
            d.text(
                &format!("{id}_line_{i}"),
                line.trim(),
                &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
            );
        }
        d.close();
    }
    d.close();
}

/// The fold bar (`TimelineFolds`): "Expand all" / "Collapse all".
pub fn fold_bar(d: &mut Dsl, id: &str) {
    let row = d.anon();
    d.view(&row, "width: Fill height: 28 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 18");
    d.link(&format!("{id}_expand"), tr("Expand all"), Some("b3.think.expand_all"), 12.5);
    d.link(&format!("{id}_collapse"), tr("Collapse all"), Some("b3.think.collapse_all"), 12.5);
    d.close();
}


pub fn build(d: &mut Dsl, frame: &Frame, store: &Store) {
    let session = store.domains.session.active().unwrap_or_default();
    let prefs = store.domains.session.thinking(&session);
    let list = blocks(store, &session, None);
    let shown: Vec<&Block> = list.iter().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect();
    let width = frame.dialog_w(600.0);
    ui::shell_open(d, frame, width);
    ui::header(d, tr("Thinking effort"), "b3.close");
    d.text("b3_think_scope", &session, &Txt::new(11.5, Face::Mono, tok::MUTED).w(W::Fill));
    d.gap(W::Fill, 14.0);
    ui::body_open(d, frame, width, 64.0);
    let opts: Vec<(&str, String)> = [("Low", "low"), ("Medium", "medium"), ("High", "high"), ("Max", "max")]
        .iter()
        .map(|(l, v)| (tr(l), format!("b3.think.effort.{v}")))
        .collect();
    d.segmented("b3_think_effort", &opts, effort_index(&prefs).unwrap_or(usize::MAX), W::Fill, Seg::Pill);
    let help = d.anon();
    d.view(&help, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{top: 8}");
    d.text(
        "b3_think_help",
        tr("Sets how much the model thinks before answering"),
        &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
    );
    if effort_index(&prefs).is_some() {
        d.link("b3_think_default", tr("Use profile default"), Some("b3.think.effort.default"), 12.0);
    } else {
        d.text("b3_think_default_on", tr("Profile default"), &Txt::new(12.0, Face::Medium, tok::TEXT));
    }
    d.close();
    d.gap(W::Fill, 10.0);
    d.hairline();
    toggle_row(
        d,
        "b3_think_show",
        tr("Show reasoning"),
        tr("Shows the model's reasoning while it works"),
        prefs.show_reasoning,
        "b3.think.show",
    );
    d.hairline();
    toggle_row(
        d,
        "b3_think_default_new",
        tr("Default on for new chats"),
        "",
        prefs.default_on,
        "b3.think.default",
    );
    d.hairline();
    d.gap(W::Fill, 8.0);
    if !prefs.show_reasoning {
        d.text(
            "b3_think_hidden",
            tr("Reasoning is hidden in this Session's transcript."),
            &ui::meta().w(W::Fill),
        );
    } else if shown.is_empty() {
        d.text(
            "b3_think_none",
            tr("No reasoning in this Session yet. New thinking appears folded in the transcript."),
            &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
        );
    } else {
        fold_bar(d, "b3_think_fold");
        d.gap(W::Fill, 6.0);
        let col = d.anon();
        d.view(&col, "width: Fill height: Fit flow: Down spacing: 8");
        for (i, b) in shown.iter().enumerate() {
            let open = prefs.expanded.contains(&b.key);
            block_view(d, &format!("b3_think_block_{i}"), b, open, &format!("b3.think.block.{}", b.key));
        }
        d.close();
    }
    ui::body_close(d);
    ui::shell_close(d);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_effort_rides_turn_start_only_when_overridden() {
        let store = Store::new();
        assert_eq!(effort_param(&store, "s"), None, "Profile default omits reasoning_effort");
        perform(&store, "s", "b3.think.effort.max");
        assert_eq!(effort_param(&store, "s").as_deref(), Some("max"));
        perform(&store, "s", "b3.think.effort.default");
        assert_eq!(effort_param(&store, "s"), None);
        assert_eq!(apply_arg(&store, "s", "HIGH").unwrap(), "high");
        assert_eq!(effort_param(&store, "s").as_deref(), Some("high"));
        assert!(apply_arg(&store, "s", "turbo").is_err());
        apply_arg(&store, "s", "reset").unwrap();
        assert_eq!(effort_param(&store, "s"), None);
    }

    #[test]
    fn the_preference_fails_closed_to_on() {
        assert!(parse_pref(None));
        assert!(parse_pref(Some("garbage")));
        assert!(parse_pref(Some("true")));
        assert!(!parse_pref(Some("false")));
        assert!(!parse_pref(Some(" false\n")));
    }

    #[test]
    fn reasoning_entries_become_folded_blocks_with_word_counts() {
        let store = Store::new();
        store.set_active(Some("s".into()));
        let tl = &store.domains.session.timeline;
        tl.append("s", Some("t1".into()), EntryKind::USER_MESSAGE, "hi".into());
        tl.append_data(
            "s",
            Some("t1".into()),
            EntryKind::REASONING,
            "**Weighed two approaches**\nCompared cache-first vs queue-based.".into(),
            serde_json::json!({"started_ms": 1000, "ended_ms": 13_200}),
        );
        let b = blocks(&store, "s", None);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].summary, "Weighed two approaches");
        assert_eq!(b[0].words, 7);
        assert_eq!(meta(&b[0]), "12 s · 7 words");
        // folded by default; toggling opens; collapse-all folds everything
        assert!(store.domains.session.thinking("s").expanded.is_empty());
        perform(&store, "s", &format!("b3.think.block.{}", b[0].key));
        assert_eq!(store.domains.session.thinking("s").expanded, vec![b[0].key.clone()]);
        perform(&store, "s", "b3.think.collapse_all");
        assert!(store.domains.session.thinking("s").expanded.is_empty());
        perform(&store, "s", "b3.think.expand_all");
        assert_eq!(store.domains.session.thinking("s").expanded.len(), 1);
    }

    #[test]
    fn the_panel_lowers_balanced_with_every_control_wired() {
        let store = Store::new();
        store.set_active(Some("s".into()));
        store.domains.session.timeline.append("s", Some("t".into()), EntryKind::REASONING, "Checked the retry path".into());
        let mut d = Dsl::new();
        build(&mut d, &Frame::DESKTOP, &store);
        let dsl = d.finish();
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
        let taps = crate::screens::taps::wired_taps(&dsl);
        for ev in [
            "b3.think.effort.low", "b3.think.effort.max", "b3.think.show", "b3.think.default",
            "b3.think.expand_all", "b3.think.collapse_all", "b3.close",
        ] {
            assert!(taps.iter().any(|(_, e)| e == ev), "{ev}");
        }
        assert!(taps.iter().any(|(_, e)| e.starts_with("b3.think.block.r")));
    }
}
