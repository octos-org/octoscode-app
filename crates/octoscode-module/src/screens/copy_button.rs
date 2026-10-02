//! A8 — the header's "Copy as Markdown" control, the native form of the web's
//! `features/transcript-export/CopyConversationButton.tsx` (mounted in the
//! product header, `App.tsx:2409-2425`): copy the open Session's conversation
//! as Markdown, read from the server's canonical history
//! (`screens::transcript::copy_conversation`, `session/hydrate`).
//!
//! The phases and copy are the web's (`CopyConversationButton.tsx:11-22`):
//! idle "Copy as Markdown" -> "Copying…" (disabled) -> "Copied" /
//! "Nothing to copy" / "Copy failed", each result reset to idle after
//! 2.5 s; the result is KEYED BY SESSION (the web mounts the button with
//! `key={session_id}`, so another Session never shows this one's result); the
//! control exists only while a Session is open and the server advertises
//! `session/hydrate`, and the phone header has no room for it (the web hides
//! it there, `copy-conversation.spec.ts:68`).
use std::sync::Mutex;
use std::time::{Duration, Instant};

use octoscode_store::Store;

/// The routed action id (the header pill's tap).
pub const ACTION: &str = "transcript.copy";
/// `RESULT_MS` (`:20`).
pub const RESULT: Duration = Duration::from_millis(2500);

/// `CopyPhase` (`:9`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Copying,
    Copied,
    Empty,
    Error,
}

impl Phase {
    /// `LABELS` (`:11-17`).
    pub fn label(self) -> &'static str {
        match self {
            Phase::Idle => "Copy as Markdown",
            Phase::Copying => "Copying…",
            Phase::Copied => "Copied",
            Phase::Empty => "Nothing to copy",
            Phase::Error => "Copy failed",
        }
    }
}

#[derive(Debug, Clone)]
struct State {
    phase: Phase,
    /// The Session the phase belongs to.
    session: String,
    /// When a result phase began (the 2.5 s reset clock).
    at: Option<Instant>,
    /// A request id: a result for an older request is dropped.
    request: u64,
    /// The failure detail (the web's `title`).
    detail: Option<String>,
}

static STATE: Mutex<State> = Mutex::new(State { phase: Phase::Idle, session: String::new(), at: None, request: 0, detail: None });

fn lock() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|p| p.into_inner())
}

pub fn owns(action: &str) -> bool {
    action == ACTION
}

/// The phase the header shows for `session` at `now`: another Session's
/// result reads idle, and a result older than 2.5 s has reset.
pub fn phase_at(session: &str, now: Instant) -> Phase {
    let st = lock();
    if st.session != session {
        return Phase::Idle;
    }
    match (st.phase, st.at) {
        (Phase::Copied | Phase::Empty | Phase::Error, Some(at)) if now.duration_since(at) >= RESULT => Phase::Idle,
        (p, _) => p,
    }
}

pub fn phase(session: &str) -> Phase {
    phase_at(session, Instant::now())
}

/// Whether the control is offered: an open Session on a server advertising
/// `session/hydrate`, outside the compact (phone) header.
pub fn offered(store: &Store, compact: bool) -> bool {
    !compact
        && store.active_session().is_some()
        && store.domains.config.supported_methods().iter().any(|m| m == "session/hydrate")
}

/// One copy request (`copy`, `:47-64`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub id: u64,
    pub session: String,
}

/// Start a copy for `session`: `None` while one is already copying (the
/// button is disabled in that phase).
pub fn begin(session: &str) -> Option<Request> {
    let mut st = lock();
    if st.session == session && st.phase == Phase::Copying {
        return None;
    }
    st.request += 1;
    st.phase = Phase::Copying;
    st.session = session.to_owned();
    st.at = None;
    st.detail = None;
    Some(Request { id: st.request, session: session.to_owned() })
}

/// Finish request `req` (a stale request is dropped): `Ok(Some(md))` copied,
/// `Ok(None)` nothing to copy, `Err(detail)` failed.
pub fn finish(req: &Request, outcome: Result<Option<String>, String>) -> Option<String> {
    let mut st = lock();
    if st.request != req.id || st.session != req.session {
        return None;
    }
    st.at = Some(Instant::now());
    match outcome {
        Ok(Some(md)) => {
            st.phase = Phase::Copied;
            Some(md)
        }
        Ok(None) => {
            st.phase = Phase::Empty;
            None
        }
        Err(e) => {
            st.phase = Phase::Error;
            st.detail = Some(e);
            None
        }
    }
}

/// Run the read through the production client; the markdown goes to the
/// board-3 host's pending clipboard (the UI thread writes it, `lib.rs`
/// sync_board3), and the UI is woken again when the result label resets.
pub async fn run(req: Request, conv: &crate::flow::Conversation) -> Phase {
    let store = &conv.store;
    let root = store.domains.session.workspace_root(&req.session);
    let title = store
        .sessions()
        .into_iter()
        .find(|s| s.id == req.session)
        .and_then(|s| s.title.filter(|t| !t.trim().is_empty()));
    let outcome = match crate::screens::transcript::copy_conversation(conv.client(), &req.session, root.as_deref(), title.as_deref()).await {
        Ok(crate::screens::transcript::CopyOutcome::Copied(md)) => Ok(Some(md)),
        Ok(crate::screens::transcript::CopyOutcome::Empty) => Ok(None),
        Ok(crate::screens::transcript::CopyOutcome::Foreign) => Err("History belongs to another Session.".to_owned()),
        Err(e) => Err(e),
    };
    if let Some(md) = finish(&req, outcome) {
        crate::screens::board3::host::state().pending_clipboard = Some(md);
    }
    makepad_widgets::SignalToUI::set_ui_signal();
    phase(&req.session)
}

/// Wake the header once more when the result label resets (the host spawns
/// this after [`run`]).
pub async fn wake_after_result() {
    tokio::time::sleep(RESULT + Duration::from_millis(50)).await;
    makepad_widgets::SignalToUI::set_ui_signal();
}

/// Test seam.
pub fn reset() {
    *lock() = State { phase: Phase::Idle, session: String::new(), at: None, request: 0, detail: None };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_follow_the_web_and_results_are_keyed_by_session_and_reset() {
        let _g = crate::screens::theme::test_lock();
        reset();
        assert_eq!(phase("s1").label(), "Copy as Markdown");
        let req = begin("s1").unwrap();
        assert_eq!(phase("s1").label(), "Copying…");
        assert!(begin("s1").is_none(), "disabled while copying");
        assert_eq!(phase("s2"), Phase::Idle, "another Session never shows this result");
        assert_eq!(finish(&req, Ok(Some("# md".into()))).as_deref(), Some("# md"));
        assert_eq!(phase("s1").label(), "Copied");
        assert_eq!(phase_at("s1", Instant::now() + RESULT), Phase::Idle, "reset after 2.5 s");
        let req = begin("s1").unwrap();
        finish(&req, Ok(None));
        assert_eq!(phase("s1").label(), "Nothing to copy");
        let req = begin("s1").unwrap();
        let stale = Request { id: req.id - 1, session: "s1".into() };
        assert_eq!(finish(&stale, Ok(Some("old".into()))), None, "a stale request is dropped");
        finish(&req, Err("boom".into()));
        assert_eq!(phase("s1").label(), "Copy failed");
    }
}
