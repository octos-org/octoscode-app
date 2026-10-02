//! A6 — the conversation's VIEW STATE (the web's
//! `features/timeline/use-conversation-scroll.ts` and `focus-restore.ts`).
//!
//! * **Reading positions per session** (`use-conversation-scroll.ts:19-44`):
//!   each session remembers whether the reader was following the tail and,
//!   if not, the anchor row and its offset; switching back restores it
//!   (`restorePosition`, `:120-145`), a session never seen starts following.
//! * **A disclosure is user intent** (`onDisclosureInteraction`, `:101-118`):
//!   opening a fold keeps its header in place instead of jumping to the tail
//!   — and (the native gap A4 left) the grown row is revealed: when its new
//!   body runs past the viewport bottom the list scrolls just enough to show
//!   it, never pushing its header above the top ([`reveal_delta`]); at the
//!   tail the list keeps following so the opened block stays in view.
//! * **Focus restore after a removed request** (`focus-restore.ts:20-46`):
//!   when the takeover card disappears and keyboard focus was inside it,
//!   focus moves to the composer; focus elsewhere is never moved.
use std::collections::HashMap;

/// One session's reading position (`ConversationViewState`,
/// `use-conversation-scroll.ts:11-16`): following the tail, or the anchor
/// row id and its offset from the viewport top.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub following: bool,
    pub first_id: usize,
    pub first_scroll: f64,
}

impl Default for Position {
    /// A session never seen starts following the tail (`:33-38`).
    fn default() -> Self {
        Position { following: true, first_id: 0, first_scroll: 0.0 }
    }
}

/// The per-session memory plus the pending reveal and the focus facts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ViewState {
    pub positions: HashMap<String, Position>,
    /// The session whose position the list currently shows.
    pub shown: Option<String>,
    /// A row the person just opened: (row key, frames left to try).
    pub reveal: Option<(String, u8)>,
    /// The takeover was showing on the last sync.
    pub takeover_was: bool,
    /// The takeover held the keyboard (one of its inputs had focus, or a
    /// takeover key acted) while it showed.
    pub focus_inside: bool,
}

impl ViewState {
    /// Remember `session`'s position before the list shows another one.
    pub fn remember(&mut self, session: &str, p: Position) {
        self.positions.insert(session.to_owned(), p);
    }

    /// The position to restore for `session` (following when never seen).
    pub fn recall(&self, session: &str) -> Position {
        self.positions.get(session).copied().unwrap_or_default()
    }
}

/// How far to scroll DOWN (px, ≥ 0) so a grown row's body is visible: its
/// bottom is brought to the viewport bottom (less `margin`), but never so far
/// that its top leaves the viewport top — the opened header stays in place
/// for a body taller than the viewport.
pub fn reveal_delta(top: f64, height: f64, viewport: f64, margin: f64) -> f64 {
    let bottom = top + height;
    let overflow = bottom - (viewport - margin);
    if overflow <= 0.5 {
        return 0.0;
    }
    let keep_header = (top - margin).max(0.0);
    overflow.min(keep_header).max(0.0)
}

/// `restoreFocusAfterRemoval` (`focus-restore.ts:20-46`) as a decision: move
/// focus to the composer only when the request surface was removed AND the
/// focus was inside it.
pub fn restore_focus(was_showing: bool, now_showing: bool, focus_inside: bool) -> bool {
    was_showing && !now_showing && focus_inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_never_seen_session_follows_and_a_seen_one_restores_its_anchor() {
        let mut v = ViewState::default();
        assert_eq!(v.recall("s1"), Position::default());
        assert!(v.recall("s1").following);
        v.remember("s1", Position { following: false, first_id: 7, first_scroll: -42.0 });
        assert_eq!(v.recall("s1"), Position { following: false, first_id: 7, first_scroll: -42.0 });
        assert!(v.recall("s2").following, "another session keeps its own memory");
    }

    #[test]
    fn a_grown_row_is_revealed_without_losing_its_header() {
        // Fits already: nothing to do.
        assert_eq!(reveal_delta(100.0, 80.0, 400.0, 12.0), 0.0);
        // Runs 60 px past the bottom (minus the margin): scroll exactly that.
        assert_eq!(reveal_delta(300.0, 148.0, 400.0, 12.0), 60.0);
        // Taller than the viewport: stop when the header reaches the top.
        assert_eq!(reveal_delta(300.0, 900.0, 400.0, 12.0), 288.0);
        // A header already at the top never moves up.
        assert_eq!(reveal_delta(0.0, 900.0, 400.0, 12.0), 0.0);
    }

    #[test]
    fn focus_moves_only_off_a_removed_surface_that_held_it() {
        assert!(restore_focus(true, false, true));
        assert!(!restore_focus(true, false, false), "focus elsewhere is never ours to move");
        assert!(!restore_focus(true, true, true), "still mounted: leave it");
        assert!(!restore_focus(false, false, true));
    }
}
