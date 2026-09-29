//! Card #21 §3 — the per-item **action router**.
//!
//! The #16 components emit binding **action ids** (8.8 condition 2): a
//! `thread-row` click, the composer's send/steer/stop, a tool cell's disclosure.
//! The screen routes each one **with its item index**; this module turns
//! `(action id, item index, store+flow state)` into the concrete [`Effect`] the
//! module performs.
//!
//! Keeping the mapping pure — no window, no transport — is what makes the
//! routing testable: a test asserts the table (an action id + the item it
//! names maps to the right effect) without a UI, and a replay test asserts the
//! effects the flow then sends are the recorded protocol methods.
use crate::bindings::{self, Ctx};

/// The concrete thing the module does for a routed action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// `session.refresh` — re-ask for the session list.
    Refresh,
    /// `session.new` — mint a fresh session id and open it.
    NewChat,
    /// `composer.submit` — `turn/start` with the current draft.
    Submit,
    /// `turn.steer` — send `text` into the live turn's input buffer.
    Steer(String),
    /// `turn.interrupt` — stop the named live turn.
    Interrupt(String),
    /// `thread.open` — open the session the clicked row names.
    Open(String),
    /// `tool.toggle` — flip the disclosure of the tool whose call id this is
    /// (UI-local; the store never sees it).
    ToggleTool(String),
    /// `answer.copy` — copy the last answer (UI-local; the host owns the
    /// clipboard).
    CopyAnswer,
    /// Card #28e — a board-4 chrome toggle (review panel, settings drawer,
    /// command palette). UI-local, like `ToggleTool`.
    UiChrome(UiChrome),
    /// A declared id with no resolvable target (a missing row), or an id this
    /// router does not own. Logged by name, never fatal (LESSONS 6).
    Unhandled(String),
}

/// The board-4 chrome surfaces a view can toggle (card #28e).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiChrome {
    /// The 560 px right Review panel.
    ReviewToggle,
    /// The 420 px Session-settings drawer.
    SettingsToggle,
    /// The 560 px floating "/" command palette.
    PaletteToggle,
}

/// Route one action. `index` is the item the control belonged to (a session
/// row's index, a tool row's index, …); it is ignored by the whole-column
/// actions. Never panics: an id with no target is [`Effect::Unhandled`].
pub fn resolve(action: &str, index: usize, ctx: &Ctx<'_>) -> Effect {
    match action {
        "session.refresh" => Effect::Refresh,
        "session.new" => Effect::NewChat,
        "composer.submit" => Effect::Submit,
        "turn.steer" => Effect::Steer(ctx.ui.lock().unwrap().draft()),
        "turn.interrupt" => match ctx.ui.lock().unwrap().active_turn() {
            Some(turn) => Effect::Interrupt(turn),
            None => Effect::Unhandled(action.to_owned()),
        },
        "thread.open" => match ctx.store.sessions().get(index) {
            Some(s) => Effect::Open(s.id.clone()),
            None => Effect::Unhandled(format!("{action}[{index}]")),
        },
        "tool.toggle" => match ctx.ui.lock().unwrap().tools().get(index) {
            Some(t) => Effect::ToggleTool(t.tool_call_id.clone()),
            None => Effect::Unhandled(format!("{action}[{index}]")),
        },
        "answer.copy" => Effect::CopyAnswer,
        // Card #28e — board-4 chrome toggles. UI-local (no protocol method):
        // the view flips its own FlowUi flags and redraws.
        "review.toggle" => Effect::UiChrome(UiChrome::ReviewToggle),
        "settings.toggle" => Effect::UiChrome(UiChrome::SettingsToggle),
        "palette.toggle" => Effect::UiChrome(UiChrome::PaletteToggle),
        // `answer.expand` / any other declared id the router does not own.
        other => Effect::Unhandled(other.to_owned()),
    }
}

/// The action ids this router performs (the §3 set) — the coverage contract a
/// test checks against [`bindings::ACTIONS`].
pub const ROUTED: &[&str] = &[
    "session.refresh",
    "session.new",
    "composer.submit",
    "turn.steer",
    "turn.interrupt",
    "thread.open",
    "tool.toggle",
    "answer.copy",
    "review.toggle",
    "settings.toggle",
    "palette.toggle",
];

/// Whether `id` is a routed action id.
pub fn is_routed(id: &str) -> bool {
    ROUTED.contains(&id)
}

/// The action ids declared in [`bindings::ACTIONS`] but NOT owned by this
/// router (e.g. `answer.expand`, a UI-local disclosure the worked-for row owns).
pub fn unrouted() -> Vec<&'static str> {
    bindings::ACTIONS
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| !is_routed(id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use octoscode_store::{Session, Store};

    use crate::flow::FlowUi;

    fn ctx_with() -> (Arc<Store>, Arc<Mutex<FlowUi>>) {
        let store = Arc::new(Store::new());
        store.set_sessions(vec![
            Session {
                id: "s1".into(),
                title: Some("One".into()),
                message_count: 0,
                updated_at: None,
                last_prompt: None,
                active_turn: false,
            },
            Session {
                id: "s2".into(),
                title: None,
                message_count: 1,
                updated_at: None,
                last_prompt: None,
                active_turn: false,
            },
        ]);
        store.set_active(Some("s1".into()));
        let ui = Arc::new(Mutex::new(FlowUi::default()));
        (store, ui)
    }

    #[test]
    fn every_routed_id_is_a_declared_action() {
        for id in ROUTED {
            assert!(bindings::is_action(id), "{id} must be a declared action");
        }
        // Only `answer.expand` (the worked-for disclosure) is left to its own row.
        assert_eq!(unrouted(), vec!["answer.expand"]);
    }

    #[test]
    fn thread_open_routes_with_the_clicked_rows_own_id() {
        let (store, ui) = ctx_with();
        let ctx = Ctx::new(&store, &ui);
        assert_eq!(resolve("thread.open", 0, &ctx), Effect::Open("s1".into()));
        assert_eq!(resolve("thread.open", 1, &ctx), Effect::Open("s2".into()));
        // A stale index (the row scrolled away) is Unhandled, never a panic.
        assert_eq!(
            resolve("thread.open", 9, &ctx),
            Effect::Unhandled("thread.open[9]".into())
        );
    }

    #[test]
    fn tool_toggle_routes_with_the_tool_call_id() {
        let (store, ui) = ctx_with();
        ui.lock().unwrap().note_tool_started_for_test("call_a", "read_file");
        let ctx = Ctx::new(&store, &ui);
        assert_eq!(
            resolve("tool.toggle", 0, &ctx),
            Effect::ToggleTool("call_a".into())
        );
        assert_eq!(
            resolve("tool.toggle", 3, &ctx),
            Effect::Unhandled("tool.toggle[3]".into())
        );
    }

    #[test]
    fn the_composer_and_answer_controls_route_to_their_effects() {
        let (store, ui) = ctx_with();
        ui.lock().unwrap().set_draft_inner("steer me");
        ui.lock().unwrap().begin_turn_now("t7");
        let ctx = Ctx::new(&store, &ui);
        assert_eq!(resolve("composer.submit", 0, &ctx), Effect::Submit);
        assert_eq!(resolve("turn.steer", 0, &ctx), Effect::Steer("steer me".into()));
        assert_eq!(resolve("turn.interrupt", 0, &ctx), Effect::Interrupt("t7".into()));
        assert_eq!(resolve("session.new", 0, &ctx), Effect::NewChat);
        assert_eq!(resolve("session.refresh", 0, &ctx), Effect::Refresh);
        assert_eq!(resolve("answer.copy", 0, &ctx), Effect::CopyAnswer);
        // With no live turn, interrupt has no target.
        ui.lock().unwrap().end_turn_now(true);
        let ctx = Ctx::new(&store, &ui);
        assert_eq!(
            resolve("turn.interrupt", 0, &ctx),
            Effect::Unhandled("turn.interrupt".into())
        );
    }
}
