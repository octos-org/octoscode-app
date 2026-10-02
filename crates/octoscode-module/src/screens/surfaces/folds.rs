//! A6 — the transcript's per-block FOLD state (the web's
//! `features/timeline/folds.ts` + `App.tsx:2583-2603`).
//!
//! A block is a reasoning entry or a tool call; absent from the memory =
//! folded (the default for both, `folds.ts:3-12`). The fold bar heads the
//! transcript whenever a foldable block exists (`Timeline.tsx:72-75`):
//! * **Expand all** expands exactly the live reasoning AND tool ids, every
//!   other id folds back (`expandAll`, `folds.ts:22-27`, called with
//!   `kind === "reasoning" || kind === "tool"`, `App.tsx:2589-2600`);
//! * **Collapse all** returns to the all-folded default, discarding the
//!   per-block memory (`collapseAll`, `:30-33`);
//! * **pruning** drops memory for blocks that left the transcript
//!   (`pruneFolds`, `:35-44`) — natively the transcript is virtualized, not
//!   bounded at 40 rows, so a block "leaves" when its entry does (a rewind, a
//!   hydrate rebuild, another session's rows).
//!
//! The memory lives where the rows read it: the reasoning blocks' keys in the
//! session's `ThinkingPrefs.expanded` (A4's thinking rows), the tool rows'
//! keys in `FlowUi`'s disclosure list (A1's tool rows). Expand all also opens
//! every turn's tool group A1 folds under its "Worked for" header, so every
//! expanded tool is actually on screen.
use std::sync::{Arc, Mutex};

use octoscode_store::timeline::EntryKind;
use octoscode_store::Store;

use crate::flow::FlowUi;

/// The live foldable ids of `session`: (reasoning keys `r<entry id>`, tool
/// keys — the call id, else `<turn>:<ordinal>`, the SAME key A1's tool row
/// discloses under, `components::tool_view`).
pub fn live_ids(store: &Store, session: &str) -> (Vec<String>, Vec<String>) {
    let entries = store.domains.session.timeline.entries(session);
    let mut thinking = Vec::new();
    let mut tools = Vec::new();
    let mut ordinal: Vec<(String, usize)> = Vec::new();
    for e in &entries {
        if e.kind == EntryKind::REASONING && !e.text.trim().is_empty() {
            thinking.push(format!("r{}", e.id));
        } else if e.kind == EntryKind::TOOL_CALL {
            let turn = e.turn_id.clone().unwrap_or_default();
            let n = match ordinal.iter_mut().find(|(t, _)| t == &turn) {
                Some((_, n)) => {
                    *n += 1;
                    *n - 1
                }
                None => {
                    ordinal.push((turn.clone(), 1));
                    0
                }
            };
            let key = e
                .data
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{turn}:{n}"));
            tools.push(key);
        }
    }
    (thinking, tools)
}

/// Whether the fold bar shows (`hasFoldable`, `Timeline.tsx:72-75`): a
/// visible reasoning block or any tool call.
pub fn has_foldable(store: &Store, session: &str) -> bool {
    let show = store.domains.session.thinking(session).show_reasoning;
    let (thinking, tools) = live_ids(store, session);
    (show && !thinking.is_empty()) || !tools.is_empty()
}

/// `expandAll` over the live reasoning + tool ids.
pub fn expand_all(store: &Store, ui: &Arc<Mutex<FlowUi>>) {
    let Some(session) = store.active_session() else { return };
    let (thinking, tools) = live_ids(store, &session);
    store.domains.session.set_thinking_expanded(&session, thinking);
    if let Ok(mut u) = ui.lock() {
        u.set_expanded_keys(tools);
        u.set_folded_turns(Vec::new());
    }
}

/// `collapseAll`: the all-folded default; the memory is discarded.
pub fn collapse_all(store: &Store, ui: &Arc<Mutex<FlowUi>>) {
    if let Some(session) = store.active_session() {
        store.domains.session.set_thinking_expanded(&session, Vec::new());
    }
    if let Ok(mut u) = ui.lock() {
        u.set_expanded_keys(Vec::new());
    }
}

/// `pruneFolds`: drop the memory of blocks no longer in the transcript.
/// Returns whether anything was dropped.
pub fn prune(store: &Store, ui: &Arc<Mutex<FlowUi>>) -> bool {
    let Some(session) = store.active_session() else { return false };
    let (thinking, tools) = live_ids(store, &session);
    let mut changed = false;
    let prefs = store.domains.session.thinking(&session);
    let kept: Vec<String> = prefs.expanded.iter().filter(|k| thinking.contains(k)).cloned().collect();
    if kept.len() != prefs.expanded.len() {
        store.domains.session.set_thinking_expanded(&session, kept);
        changed = true;
    }
    if let Ok(mut u) = ui.lock() {
        let keys = u.expanded_keys();
        let kept: Vec<String> = keys.iter().filter(|k| tools.contains(k)).cloned().collect();
        if kept.len() != keys.len() {
            u.set_expanded_keys(kept);
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Arc<Store> {
        let s = Arc::new(Store::new());
        s.set_active(Some("s".into()));
        s
    }

    #[test]
    fn expand_all_opens_exactly_the_live_reasoning_and_tool_blocks() {
        let s = store();
        let tl = &s.domains.session.timeline;
        tl.upsert_user_message("s", "t1", "hi", serde_json::json!({}));
        let r = tl.append("s", Some("t1".into()), EntryKind::REASONING, "weighing".into());
        tl.append_data("s", Some("t1".into()), EntryKind::TOOL_CALL, "bash".into(), serde_json::json!({"tool_call_id": "call-1"}));
        tl.append("s", Some("t1".into()), EntryKind::TOOL_CALL, "read".into());
        let ui = Arc::new(Mutex::new(FlowUi::default()));
        ui.lock().unwrap().toggle_expanded("stale-key");
        ui.lock().unwrap().toggle_turn_fold("t1");
        assert!(has_foldable(&s, "s"));
        expand_all(&s, &ui);
        assert_eq!(s.domains.session.thinking("s").expanded, vec![format!("r{r}")]);
        assert_eq!(ui.lock().unwrap().expanded_keys(), vec!["call-1".to_owned(), "t1:1".to_owned()], "ids not listed fold back");
        assert!(ui.lock().unwrap().folded_turns().is_empty(), "every expanded tool is on screen");
        collapse_all(&s, &ui);
        assert!(s.domains.session.thinking("s").expanded.is_empty());
        assert!(ui.lock().unwrap().expanded_keys().is_empty());
    }

    #[test]
    fn pruning_drops_blocks_that_left_the_transcript() {
        let s = store();
        let tl = &s.domains.session.timeline;
        let r = tl.append("s", Some("t1".into()), EntryKind::REASONING, "weighing".into());
        s.domains.session.set_thinking_expanded("s", vec![format!("r{r}"), "r999".into()]);
        let ui = Arc::new(Mutex::new(FlowUi::default()));
        ui.lock().unwrap().toggle_expanded("gone-call");
        assert!(prune(&s, &ui));
        assert_eq!(s.domains.session.thinking("s").expanded, vec![format!("r{r}")]);
        assert!(ui.lock().unwrap().expanded_keys().is_empty());
        assert!(!prune(&s, &ui), "idempotent");
    }

    #[test]
    fn no_blocks_means_no_fold_bar() {
        let s = store();
        s.domains.session.timeline.upsert_user_message("s", "t1", "hi", serde_json::json!({}));
        assert!(!has_foldable(&s, "s"));
    }
}
