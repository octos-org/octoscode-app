//! oc7 (round 8 E) — a Mac's New Chat that failed before the window moved to
//! it (no profile named yet, the link down, a workspace launch the server
//! refused): one alert with Try Again (the board-3 kit's alert: the app icon,
//! the title, the reason in a person's words, Cancel and the default Try
//! Again side by side), where a phone keeps the A26 error toast. A new chat
//! the server refused AFTER the window moved to it says so where the chat
//! would start instead (`fluid::start_state`).
//!
//! Return answers Try Again, Escape and a click outside answer Cancel; while
//! it is up the A9 dock holds the toasts (`a26_host::HOLDING_DOCKS`).
use std::sync::Mutex;

use super::board3::ui::{self, Dsl};
use crate::i18n::tr;

pub const ACTION_RETRY: &str = "start.retry";
pub const ACTION_CANCEL: &str = "start.cancel";
pub const ACTION_NOOP: &str = "start.noop";

/// What Try Again starts again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Retry {
    /// New Chat (`session.new`).
    NewChat,
    /// A workspace's New Chat Here (the sidebar's `NewChatInWorkspace`).
    InWorkspace { workspace: String, path: Option<String> },
}

/// The alert that is up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub cause: String,
    pub retry: Retry,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

fn lock() -> std::sync::MutexGuard<'static, Option<Pending>> {
    PENDING.lock().unwrap_or_else(|p| p.into_inner())
}

/// A new chat failed before the window moved to it: the alert on a Mac, the
/// error toast on a phone. Called from the runtime threads.
pub fn failed(cause: &str, retry: Retry) {
    if !crate::desk::active() {
        super::toasts::failed(super::toasts::Op::NewChat, cause);
        return;
    }
    raise(cause, retry);
}

/// Put the alert up (a Mac's half of [`failed`]).
fn raise(cause: &str, retry: Retry) {
    makepad_widgets::log!("[octoscode] new chat alert: {}", cause.trim());
    *lock() = Some(Pending { cause: cause.to_owned(), retry });
    makepad_widgets::SignalToUI::set_ui_signal();
}

/// The alert that is up, if any.
pub fn pending() -> Option<Pending> {
    lock().clone()
}

/// Close it, returning what it offered to start again.
pub fn take() -> Option<Pending> {
    lock().take()
}

/// The ids this alert owns.
pub fn routes(action: &str) -> bool {
    matches!(action, ACTION_RETRY | ACTION_CANCEL | ACTION_NOOP)
}

/// What the alert says under its title: the toast's words for the cause, or
/// a plain sentence when those are the log's to keep (a protocol error, or a
/// bare method name such as a refused `launch/resolve`).
pub fn words(cause: &str) -> String {
    let shown = super::toasts::cause_shown(cause);
    if shown.is_empty() || !shown.trim().contains(' ') {
        tr("The server didn’t open it.").to_owned()
    } else {
        shown
    }
}

/// Lower the alert (`None` when none is up).
pub fn lower() -> Option<super::activity::Lowered> {
    let p = pending()?;
    let words = words(&p.cause);
    let mut d = Dsl::new();
    d.view("st_alert_root", &format!("width: Fill height: Fill flow: Overlay align: {}", ui::sheet_align()));
    d.rule("st_alert_mask", "width: Fill height: Fill", ui::mac_mask());
    d.view("st_alert_backdrop_box", "width: Fill height: Fill flow: Overlay");
    d.tap("st_alert_backdrop", ACTION_CANCEL);
    d.close();
    d.sheet_surface("st_alert_dialog", &format!("width: {} height: Fit flow: Overlay", ui::ALERT_W), 10.0);
    d.view("st_alert_swallow_box", "width: Fill height: Fill flow: Overlay");
    d.tap("st_alert_swallow", ACTION_NOOP);
    d.close();
    d.view(
        "st_alert_col",
        "width: Fill height: Fit flow: Down align: Align{x: 0.5 y: 0.0} padding: Inset{left: 16 right: 16 top: 32 bottom: 16}",
    );
    ui::app_icon(&mut d, "st_alert_icon");
    ui::alert_words(&mut d, ("st_alert_title", tr("Couldn’t Start a New Chat")), &[("st_alert_words", words.as_str())]);
    ui::alert_buttons(
        &mut d,
        &[
            ui::AlertButton::ids("st_alert_cancel_box", "st_alert_cancel_label", "st_alert_cancel", "Cancel", ACTION_CANCEL, ui::Btn::Outline),
            ui::AlertButton::ids("st_alert_retry_box", "st_alert_retry_label", "st_alert_retry", "Try again", ACTION_RETRY, ui::Btn::Primary),
        ],
    );
    d.close(); // col
    d.close(); // dialog
    d.close(); // root
    let taps = d.taps.clone();
    Some(super::activity::Lowered { dsl: d.finish(), taps, inputs: Vec::new() })
}

/// Test seam: no alert.
pub fn reset() {
    *lock() = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A phone keeps its toast (a unit test draws no style, so it is one).
    #[test]
    fn a_phone_toasts_instead() {
        let _g = crate::screens::theme::test_lock();
        reset();
        crate::screens::toasts::reset();
        failed("not connected", Retry::NewChat);
        assert!(pending().is_none());
        let (items, _) = crate::screens::toasts::snapshot();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].lead, crate::screens::toasts::Op::NewChat.lead());
        crate::screens::toasts::reset();
    }

    /// The Mac's alert evaluates in the app VM in both appearances, every
    /// tap it routes is in the tree, Try Again comes last (the default, on
    /// the right), it says the server's words, and it hands back what to
    /// start again.
    #[test]
    fn the_alert_evaluates_with_both_answers() {
        use makepad_widgets::*;
        let _g = crate::screens::theme::test_lock();
        reset();
        let retry = Retry::InWorkspace { workspace: "demo".into(), path: Some("/w/demo".into()) };
        raise("The server is busy. Try again in a moment.", retry.clone());
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.with_vm(makepad_widgets::script_mod);
        cx.with_vm(octoscript_widgets::design::script_mod);
        cx.with_vm(octoscript_widgets::kit::script_mod);
        for dark in [false, true] {
            crate::screens::theme::set_preference(if dark { "dark" } else { "light" });
            let low = lower().expect("the alert");
            assert!(low.dsl.contains("The server is busy. Try again in a moment."), "{}", low.dsl);
            assert!(low.dsl.contains("AppIcon"), "the app's icon tops it");
            let events: Vec<&str> = low.taps.iter().map(|(_, e)| e.as_str()).collect();
            let cancel = low.taps.iter().position(|(id, _)| id == "st_alert_cancel").expect("Cancel");
            let again = low.taps.iter().position(|(id, _)| id == "st_alert_retry").expect("Try Again");
            assert!(again > cancel, "the default comes last: {events:?}");
            assert_eq!((low.taps[cancel].1.as_str(), low.taps[again].1.as_str()), (ACTION_CANCEL, ACTION_RETRY));
            let view = crate::mount::eval_component(&mut cx, MAIN_SPLASH_VM_ID, &low.dsl)
                .unwrap_or_else(|e| panic!("dark={dark}: {e}\n{}", low.dsl));
            for (id, ev) in &low.taps {
                assert!(!view.widget(&mut cx, &[LiveId::from_str(id)]).is_empty(), "{id} ({ev}) is not in the tree");
            }
        }
        assert_eq!(take().map(|p| p.retry), Some(retry));
        assert!(lower().is_none());
        crate::screens::theme::reset_state();
        reset();
    }

    #[test]
    fn its_words_are_the_toasts_or_a_plain_sentence() {
        assert_eq!(words("the replay fixture refused session/open #2 (--refuse)"), "the replay fixture refused session/open #2 (--refuse)");
        assert!(!words("").is_empty());
        assert_eq!(words("launch/resolve"), words(""), "a bare method is the log's");
    }
}
