//! A12 — the connection recovery banner: what the window shows while a
//! connection that was live is re-dialing.
//!
//! Web reference (`apps/web/src/app/App.tsx:2724-2754`, CSS
//! `styles.css:779-858`, copy `durable-session.ts:65-78`): while the opened
//! Session's recovery is not healthy the conversation stays and a status
//! banner sits where the composer form is — a 30 px rounded warn-tinted mark
//! with the refresh glyph, then "Reconnecting to Octos" (13 px, 500) over
//! "Connection lost · retry N" (11 px, tertiary, one line), or "Restoring
//! session state" / "Restoring authoritative session state" once the socket is
//! back and the Session re-opens. The banner spans the chat column
//! (`max-width: var(--dsw-layout-chat-wide)`, the composer's own width),
//! 1 px border, radius 16, 12x14 padding, 12 px gap.
//!
//! Native additions, each for an honest state the web has no need for:
//! - the detail names the server being re-dialed (`127.0.0.1:50290`), so a
//!   server that moved is visible instead of silently retried;
//! - a send or command refused while disconnected says so on the banner
//!   ("Not sent — your text stays in the composer…"): the native composer
//!   stays editable through the outage (the web hides it);
//! - "Retry now" (the approved setup-12 card's `t_retry`, "Reconnecting…
//!   attempt 2 · Retry now") re-dials at once instead of waiting out the
//!   transport's backoff, and "Disconnect" is the explicit give-up (A9's
//!   confirmed leave; the Connect card then shows the server in use);
//! - with NO conversation at all (a transport-less shell), a refused action
//!   reads "Not connected to Octos" instead of doing nothing.
use std::sync::Mutex;

use octoscode_store::Store;

use super::board3::ui::{tok, text_w, Btn, Dsl, Face, Txt, W};
use crate::i18n::{tr, tr1};

/// Re-dial at once.
pub const ACTION_RETRY: &str = "a12.link.retry";
/// The explicit give-up: A9's Disconnect (with its confirmation when work or
/// unsaved input would be lost).
pub const ACTION_DISCONNECT: &str = "a12.link.disconnect";
/// Dismiss the "Not connected" notice (no conversation).
pub const ACTION_DISMISS: &str = "a12.link.dismiss";

pub fn routes(action: &str) -> bool {
    matches!(action, ACTION_RETRY | ACTION_DISCONNECT | ACTION_DISMISS)
}

/// The banner's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// The socket is gone; the transport re-dials.
    Reconnecting,
    /// A new socket is up; the Session re-opens and re-hydrates.
    Restoring,
    /// The re-open on the new socket was refused (the reason is the
    /// detail); Retry now tries again.
    RecoveryRequired(String),
    /// There is no conversation at all and an action needed one.
    NotConnected,
}

/// What the banner draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub kind: Kind,
    /// `host:port` of the server being re-dialed (no scheme).
    pub server: String,
    /// Cumulative re-dial attempts.
    pub attempt: u32,
    /// The refused action, if one was refused during this outage.
    pub held: Option<String>,
}

/// The refused action's line (process-wide, like the other screens' notices).
static HELD: Mutex<Option<String>> = Mutex::new(None);

fn lock() -> std::sync::MutexGuard<'static, Option<String>> {
    HELD.lock().unwrap_or_else(|p| p.into_inner())
}

/// The line for a refused send: a prompt (`None`) or a typed command
/// (`/review`). The text is never consumed, so the line says where it is.
pub fn held_line(command: Option<&str>) -> String {
    match command {
        Some(cmd) if !cmd.trim().is_empty() => {
            tr1("{value0} was not run — your text stays in the composer.", cmd.trim())
        }
        _ => tr("Not sent — your text stays in the composer.").to_owned(),
    }
}

/// Record an action refused while disconnected (the banner's amber line).
pub fn note_held(line: impl Into<String>) {
    *lock() = Some(line.into());
    makepad_widgets::SignalToUI::set_ui_signal();
}

pub fn held() -> Option<String> {
    lock().clone()
}

pub fn clear_held() {
    *lock() = None;
}

/// `http://127.0.0.1:50290/` -> `127.0.0.1:50290` (the address a person
/// recognises; the scheme and a trailing slash are noise here).
pub fn server_label(endpoint: &str) -> String {
    let e = endpoint.trim();
    let e = e.split_once("://").map(|(_, rest)| rest).unwrap_or(e);
    let e = e.split(['?', '#']).next().unwrap_or(e);
    e.trim_end_matches('/').to_owned()
}

/// The banner for this store, if one shows. `has_conversation` = a
/// transport exists (the bridge holds a conversation).
pub fn view(store: &Store, has_conversation: bool) -> Option<View> {
    if let Some(o) = store.outage() {
        return Some(View {
            kind: match (&o.error, o.restoring) {
                (Some(e), _) => Kind::RecoveryRequired(e.clone()),
                (None, true) => Kind::Restoring,
                (None, false) => Kind::Reconnecting,
            },
            server: server_label(&o.endpoint),
            attempt: o.attempt,
            held: held(),
        });
    }
    if store.is_live() && has_conversation {
        // Back online: a refusal from the outage is history.
        clear_held();
        return None;
    }
    if !has_conversation {
        if let Some(h) = held() {
            return Some(View { kind: Kind::NotConnected, server: String::new(), attempt: 0, held: Some(h) });
        }
    }
    None
}

/// The title / detail copy.
pub fn copy(v: &View) -> (String, String) {
    match &v.kind {
        Kind::Reconnecting => {
            let mut detail = tr("Connection lost").to_owned();
            if v.attempt > 0 {
                detail.push_str(&format!(" · {}", tr1("retry {value0}", &v.attempt.to_string())));
            }
            if !v.server.is_empty() {
                detail.push_str(&format!(" · {}", v.server));
            }
            (tr("Reconnecting to Octos").to_owned(), detail)
        }
        Kind::Restoring => {
            let mut detail = tr("Restoring authoritative session state").to_owned();
            if !v.server.is_empty() {
                detail.push_str(&format!(" · {}", v.server));
            }
            (tr("Restoring session state").to_owned(), detail)
        }
        Kind::RecoveryRequired(reason) => {
            let mut detail = reason.clone();
            if !v.server.is_empty() {
                detail.push_str(&format!(" · {}", v.server));
            }
            (tr("Session recovery required").to_owned(), detail)
        }
        Kind::NotConnected => (
            tr("Not connected to Octos").to_owned(),
            tr("Nothing was sent. Connect to a server first.").to_owned(),
        ),
    }
}

/// The failed-recovery border (the web's error at 35 % over the surface).
const ERROR_LINE: &str = "#f0b9bdff";
/// The failed-recovery mark (`--dsw-alias-interactive-bg-hover-danger`).
const ERROR_MARK: &str = "#fbd5d8ff";

/// The lowered banner.
pub struct Lowered {
    pub dsl: String,
    pub taps: Vec<(String, String)>,
}

/// Lower the banner at `width` (the composer column's width). `phone` puts
/// the actions under the text (the composer is ~330 px there).
pub fn lower(v: &View, width: f64, phone: bool) -> Lowered {
    let (title, detail) = copy(v);
    let failed = matches!(v.kind, Kind::RecoveryRequired(_));
    let mut d = Dsl::new();
    // The banner box: the chat column's width, 1 px border, radius 16. The
    // in-progress states keep a neutral surface; a failed recovery reads as
    // a blocking state (the web's error tint: `recovery-error`,
    // `styles.css:826-843`). The bottom margin seats it over the strip like
    // the composer's other rows.
    d.surface(
        "a12_link_banner",
        "width: Fill height: Fit flow: Down margin: Inset{bottom: 8}",
        if failed { tok::RED_BG } else { tok::SURFACE },
        16.0,
        Some(if failed { ERROR_LINE } else { tok::HAIRLINE }),
    );
    // The in-progress states carry the web's accent edge (`inset 3px 0 0`
    // business blue): a 3 px bar on the banner's left edge, so the mark
    // still starts near the web's 14 px (3 + the 12 px gap).
    let accent = matches!(v.kind, Kind::Reconnecting | Kind::Restoring);
    d.view(
        "a12_link_row",
        &format!(
            "width: Fill height: Fit flow: Right spacing: 12 align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: {} right: 14 top: 12 bottom: 12}}",
            // 1 px: the bar sits inside the 1 px border, like the web's inset.
            if accent { 1 } else { 14 }
        ),
    );
    if accent {
        d.rule("a12_link_accent", "width: 3 height: 30", tok::BLUE);
    }
    // The 30 px warn-tinted mark with the refresh glyph (a warning glyph on
    // the blocking states).
    d.surface(
        "a12_link_mark",
        "width: 30 height: 30 flow: Overlay align: Align{x: 0.5 y: 0.5}",
        if failed { ERROR_MARK } else { tok::AMBER_BG },
        10.0,
        None,
    );
    let glyph = if accent { "a12_refresh_warn.svg" } else { "b3_warning.svg" };
    d.icon("a12_link_icon", glyph, 16.0, tok::AMBER);
    d.close();
    // Title over detail (+ the refused-action line).
    d.view("a12_link_text", "width: Fill height: Fit flow: Down spacing: 3");
    d.text("a12_link_title", &title, &Txt::new(13.0, Face::Medium, tok::TEXT).w(W::Fill));
    d.text("a12_link_detail", &detail, &Txt::new(11.0, Face::Regular, tok::MUTED).w(W::Fill));
    if let Some(h) = &v.held {
        d.text("a12_link_held", h, &Txt::new(12.0, Face::Regular, tok::AMBER).w(W::Fill).wrap());
    }
    d.close();
    if !phone {
        actions(&mut d, v);
    }
    d.close(); // row
    if phone {
        // Under the text: indented to the text column (1 + 3 accent + 12 gap
        // + 30 mark + 12 gap; 14 pad + 30 + 12 without the accent).
        let indent = if accent { 58.0 } else { 56.0 };
        d.view(
            "a12_link_actions_row",
            &format!("width: Fill height: Fit flow: Right spacing: 8 align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: {indent} right: 14 top: 0 bottom: 12}}"),
        );
        actions(&mut d, v);
        d.close();
    }
    d.close(); // banner
    let _ = width;
    let taps = d.taps.clone();
    Lowered { dsl: d.finish(), taps }
}

/// The banner's actions: Retry now + Disconnect while re-dialing, Disconnect
/// while restoring, a dismiss for the no-conversation notice.
fn actions(d: &mut Dsl, v: &View) {
    match &v.kind {
        Kind::Reconnecting | Kind::RecoveryRequired(_) => {
            d.button("a12_link_retry", tr("Retry now"), ACTION_RETRY, Btn::Outline, W::Px(text_w(tr("Retry now"), 13.0, Face::Medium) + 28.0), 32.0);
            d.button("a12_link_leave", tr("Disconnect"), ACTION_DISCONNECT, Btn::Ghost, W::Px(text_w(tr("Disconnect"), 13.0, Face::Medium) + 24.0), 32.0);
        }
        Kind::Restoring => {
            d.button("a12_link_leave", tr("Disconnect"), ACTION_DISCONNECT, Btn::Ghost, W::Px(text_w(tr("Disconnect"), 13.0, Face::Medium) + 24.0), 32.0);
        }
        Kind::NotConnected => {
            d.button("a12_link_dismiss", tr("Dismiss"), ACTION_DISMISS, Btn::Ghost, W::Px(text_w(tr("Dismiss"), 13.0, Face::Medium) + 24.0), 32.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The refused-action line is process-wide: the tests that read it run
    /// one at a time.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn reset() -> std::sync::MutexGuard<'static, ()> {
        let g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        clear_held();
        g
    }

    #[test]
    fn the_banner_follows_the_retained_outage_and_names_the_server() {
        let _serial = reset();
        let s = Store::new();
        assert!(view(&s, true).is_none(), "nothing before a connection");
        s.set_connection("Live".into(), true);
        assert!(view(&s, true).is_none(), "nothing while live");
        s.set_connection("Reconnecting { attempt: 1 }".into(), false);
        s.connection.note_outage("http://127.0.0.1:50290", Some(3), false);
        let v = view(&s, true).expect("the outage shows");
        assert_eq!(v.kind, Kind::Reconnecting);
        assert_eq!(copy(&v), ("Reconnecting to Octos".into(), "Connection lost · retry 3 · 127.0.0.1:50290".into()));
        s.connection.note_outage("http://127.0.0.1:50290", None, true);
        let v = view(&s, true).unwrap();
        assert_eq!(copy(&v).0, "Restoring session state");
        assert_eq!(copy(&v).1, "Restoring authoritative session state · 127.0.0.1:50290");
        s.connection.note_outage_error("session is being restored");
        let v = view(&s, true).unwrap();
        assert_eq!(v.kind, Kind::RecoveryRequired("session is being restored".into()));
        assert_eq!(copy(&v), ("Session recovery required".into(), "session is being restored · 127.0.0.1:50290".into()));
        let low = lower(&v, 640.0, false);
        assert!(low.taps.iter().any(|(_, e)| e == ACTION_RETRY), "Retry now re-opens");
        assert!(!low.dsl.contains("a12_link_accent"), "a blocking state, not in progress");
        s.connection.note_outage("http://127.0.0.1:50290", Some(9), false);
        assert_eq!(view(&s, true).unwrap().kind, Kind::Reconnecting, "a new re-dial clears the failure");
        s.set_connection("Live".into(), true);
        assert!(view(&s, true).is_none(), "gone once live");
    }

    #[test]
    fn a_refusal_is_said_on_the_banner_and_cleared_when_back() {
        let _serial = reset();
        let s = Store::new();
        s.set_connection("Live".into(), true);
        s.set_connection("Reconnecting { attempt: 1 }".into(), false);
        s.connection.note_outage("http://h:1", Some(1), false);
        note_held(held_line(None));
        assert_eq!(view(&s, true).unwrap().held.as_deref(), Some("Not sent — your text stays in the composer."));
        note_held(held_line(Some("/review")));
        let v = view(&s, true).unwrap();
        assert_eq!(v.held.as_deref(), Some("/review was not run — your text stays in the composer."));
        let low = lower(&v, 640.0, false);
        assert!(low.dsl.contains("/review was not run"));
        s.set_connection("Live".into(), true);
        assert!(view(&s, true).is_none());
        assert!(held().is_none(), "back online clears the refusal");
    }

    #[test]
    fn without_a_conversation_a_refusal_reads_not_connected() {
        let _serial = reset();
        let s = Store::new();
        assert!(view(&s, false).is_none());
        note_held(held_line(Some("/undo")));
        let v = view(&s, false).expect("shown");
        assert_eq!(v.kind, Kind::NotConnected);
        assert_eq!(copy(&v).0, "Not connected to Octos");
        let low = lower(&v, 640.0, false);
        assert_eq!(low.taps, vec![("a12_link_dismiss".to_owned(), ACTION_DISMISS.to_owned())]);
        clear_held();
    }

    #[test]
    fn the_actions_are_routed_taps_and_the_phone_puts_them_under_the_text() {
        let _serial = reset();
        let v = View { kind: Kind::Reconnecting, server: "h:1".into(), attempt: 2, held: None };
        let desk = lower(&v, 640.0, false);
        let ids: Vec<&str> = desk.taps.iter().map(|(i, _)| i.as_str()).collect();
        assert_eq!(ids, vec!["a12_link_retry", "a12_link_leave"]);
        assert!(desk.taps.iter().all(|(_, e)| routes(e)));
        assert_eq!(desk.taps, crate::screens::taps::wired_taps(&desk.dsl), "the shared tap path parses them");
        let phone = lower(&v, 330.0, true);
        assert!(phone.dsl.contains("a12_link_actions_row"));
        assert!(!desk.dsl.contains("a12_link_actions_row"));
        let restoring = lower(&View { kind: Kind::Restoring, ..v }, 640.0, false);
        assert_eq!(restoring.taps.len(), 1, "no Retry while the socket is already back");
    }

    #[test]
    fn server_labels_drop_the_scheme_and_slash() {
        assert_eq!(server_label("http://127.0.0.1:50290/"), "127.0.0.1:50290");
        assert_eq!(server_label("https://octos.example/api?x=1"), "octos.example/api");
        assert_eq!(server_label("h:1"), "h:1");
    }
}
