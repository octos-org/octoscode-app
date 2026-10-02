//! A9 — the Settings additions that belong to the connection: the General
//! "Octos server" status row (the web's five connection states), the
//! "Current workspace" / "Profile" rows, and the Connection section's
//! **Disconnect** / **Forget server** with the web's leave confirmation.
//!
//! Web oracle:
//! - `product-settings/GeneralSettingsContent.tsx:43-60` (`STATUS_COPY`,
//!   `statusClass`), `:163-200` (the workspace / preset / profile rows, shown
//!   only when known; `App.tsx:3431-3438` passes the workspace path and the
//!   opened Profile, never an agent preset), `:286-313` (Disconnect keeps the
//!   server remembered; Forget removes the saved server and its credential).
//! - `connection/LeaveConnectionDialog.tsx:1-63` + `App.tsx:3444-3452,
//!   3598-3617`: Disconnect / Forget ask first only while work is unfinished
//!   (`App.tsx:1568-1574`: an active turn, queued input, a running
//!   background turn) or an unsaved draft would be lost; otherwise they act
//!   at once.
//!
//! The dialog is drawn with the native dialog kit (`board3::ui`) and mounted
//! in A9's dock (`a9_host`), over Settings.
use std::sync::Mutex;

use octoscode_store::Store;

use super::board3::ui::{self, tok, Dsl, Face, Frame, Txt, W};

// ---------------------------------------------------------- status projection

/// `ProductConnectionStatus` (`GeneralSettingsContent.tsx:15-16`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnStatus {
    Idle,
    Connecting,
    Connected,
    Disconnected,
    Error,
}

impl ConnStatus {
    /// `STATUS_COPY` (`GeneralSettingsContent.tsx:43-49`).
    pub fn copy(self) -> &'static str {
        match self {
            ConnStatus::Idle => "Not connected",
            ConnStatus::Connecting => "Connecting…",
            ConnStatus::Connected => "Connected",
            ConnStatus::Disconnected => "Disconnected",
            ConnStatus::Error => "Connection error",
        }
    }

    /// The dot (`statusClass`, `:51-60`; `ProductSettings.module.css:65-84`):
    /// success / warn / error, else the caption grey.
    pub fn dot(self) -> Dot {
        match self {
            ConnStatus::Connected => Dot::Ok,
            ConnStatus::Connecting => Dot::Busy,
            ConnStatus::Error => Dot::Err,
            ConnStatus::Idle | ConnStatus::Disconnected => Dot::Idle,
        }
    }
}

/// The four dot fills the row carries (one is shown).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dot {
    Ok,
    Busy,
    Err,
    Idle,
}

/// The store's connection row (the transport's `ConnectionState` Debug text,
/// or the shell's `Offline`) → the web's five states. `connecting` = a
/// Connect the person pressed is in flight.
pub fn status_of(conn: &str, connecting: bool) -> ConnStatus {
    if connecting {
        return ConnStatus::Connecting;
    }
    match conn {
        "Live" => ConnStatus::Connected,
        "Dialing" | "Handshaking" => ConnStatus::Connecting,
        c if c.starts_with("Reconnecting") => ConnStatus::Connecting,
        "Failed" => ConnStatus::Error,
        "Offline" => ConnStatus::Disconnected,
        _ => ConnStatus::Idle,
    }
}

/// One read-only settings row (`SettingRow`, `:62-90`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoRow {
    pub title: &'static str,
    pub description: String,
    pub value: String,
    /// The value is a path (mono, `pathValue`).
    pub path: bool,
}

/// The rows shown only when known (`:163-200`): the current workspace (its
/// name, the path as the description) and the opened Profile.
pub fn info_rows(store: &Store, profile: &str) -> Vec<InfoRow> {
    let mut out = Vec::new();
    let path = store
        .active_session()
        .and_then(|s| store.domains.session.workspace_root(&s))
        .filter(|p| !p.trim().is_empty());
    if let Some(path) = path {
        out.push(InfoRow {
            title: "Current workspace",
            value: ui::leaf(&path),
            description: path,
            path: false,
        });
    }
    if !profile.trim().is_empty() {
        out.push(InfoRow {
            title: "Profile",
            description: "The Octos profile backing this session.".to_owned(),
            value: profile.to_owned(),
            path: false,
        });
    }
    out
}

// ------------------------------------------------------------ the leave flow

/// Which way out (`LeaveConnectionDialogProps.action`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveKind {
    Disconnect,
    Forget,
}

/// The pending confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leave {
    pub kind: LeaveKind,
    /// The composer holds text that would be lost (`unsavedDraft`).
    pub unsaved: bool,
}

static PENDING: Mutex<Option<Leave>> = Mutex::new(None);

pub fn pending() -> Option<Leave> {
    *PENDING.lock().unwrap_or_else(|p| p.into_inner())
}

fn set_pending(l: Option<Leave>) {
    *PENDING.lock().unwrap_or_else(|p| p.into_inner()) = l;
}

/// `hasUnfinishedWork` (`App.tsx:1568-1574`) natively: this session's turn is
/// live, or another session reports a live turn (`session/list`
/// `active_turn`). The native composer keeps no send queue.
pub fn unfinished_work(turn_active: bool, store: &Store) -> bool {
    let active = store.active_session();
    turn_active
        || store
            .sessions()
            .iter()
            .any(|s| s.active_turn && Some(&s.id) != active.as_ref())
}

/// A Disconnect / Forget click: ask first when work is unfinished or a draft
/// would be lost (the web's gate), else act now. `Some(kind)` = perform it.
pub fn request(kind: LeaveKind, unfinished: bool, unsaved_draft: bool) -> Option<LeaveKind> {
    if unfinished || unsaved_draft {
        set_pending(Some(Leave { kind, unsaved: unsaved_draft }));
        None
    } else {
        set_pending(None);
        Some(kind)
    }
}

/// Cancel / Escape / the backdrop: nothing happens.
pub fn cancel() {
    set_pending(None);
}

/// The dialog's confirm: the asked action, once.
pub fn confirm() -> Option<LeaveKind> {
    let kind = pending().map(|l| l.kind);
    set_pending(None);
    kind
}

/// The voluntary disconnect on the production transport: the socket closes
/// and the transport's task drains and exits (no reconnect), and the store
/// reads Offline. Disconnect and Forget both run this.
pub async fn disconnect(conv: &crate::flow::Conversation) {
    let _ = conv
        .command_sender()
        .send(octos_app_transport::OutboundCommand::Disconnect)
        .await;
    conv.store.set_connection("Offline".to_owned(), false);
}

/// Forget server's extra half: the remembered address and this origin's
/// saved token go (`credentials.rs`); other servers' tokens stay.
pub fn forget_saved(server: &str) {
    crate::credentials::forget_token(server);
    crate::credentials::forget_server();
    // A19 — and the remembered Session/profile/workspace (the web's
    // forgetConnection clears its tab connection, `ConnectionGate.tsx:319-356`).
    crate::screens::remembered::forget(server);
}

pub const ACTION_DISCONNECT: &str = "a9.leave.disconnect";
pub const ACTION_FORGET: &str = "a9.leave.forget";
pub const ACTION_CANCEL: &str = "a9.leave.cancel";
pub const ACTION_CONFIRM: &str = "a9.leave.confirm";
pub const ACTION_NOOP: &str = "a9.leave.noop";

pub fn routes(action: &str) -> bool {
    matches!(action, ACTION_DISCONNECT | ACTION_FORGET | ACTION_CANCEL | ACTION_CONFIRM | ACTION_NOOP)
}

/// The dialog copy (`LeaveConnectionDialog.tsx:34-56`).
pub fn copy(l: &Leave) -> (&'static str, Vec<&'static str>, &'static str) {
    let forget = l.kind == LeaveKind::Forget;
    let mut body = vec![
        "Current and background work may stop when this connection closes. Queued messages will be discarded.",
    ];
    if l.unsaved {
        body.push("This input has not been saved. Copy it before leaving this conversation.");
    }
    body.push(if forget {
        // Natively the sign-in detail is the token saved for this server
        // (credentials.rs), not a browser tab's.
        "This also removes the saved server address and its saved access token."
    } else {
        "The server stays remembered so you can reconnect later."
    });
    (
        if forget { "Forget this server?" } else { "Disconnect from Octos?" },
        body,
        if forget { "Forget server" } else { "Disconnect" },
    )
}

/// Lower the pending confirmation (`None` when nothing is pending).
pub fn lower(frame: &Frame) -> Option<super::activity::Lowered> {
    let l = pending()?;
    let (title, body, confirm_label) = copy(&l);
    // `width: min(440px, 100%)` inside the 20 px backdrop inset; 24 px
    // padding, radius 20 (`LeaveConnectionDialog.module.css:1-30`).
    let w = (frame.avail_w - 40.0).min(440.0).max(240.0).floor();
    let mut d = Dsl::new();
    d.view("a9_lv_root", "width: Fill height: Fill flow: Overlay align: Align{x: 0.5 y: 0.5}");
    d.rule("a9_lv_mask", "width: Fill height: Fill", tok::MASK);
    d.view("a9_lv_backdrop_box", "width: Fill height: Fill flow: Overlay");
    d.tap("a9_lv_backdrop", ACTION_CANCEL);
    d.close();
    d.surface(
        "a9_lv_dialog",
        &format!("width: {w} height: Fit flow: Overlay"),
        tok::SURFACE,
        20.0,
        Some(tok::HAIRLINE),
    );
    d.view("a9_lv_swallow_box", "width: Fill height: Fill flow: Overlay");
    d.tap("a9_lv_swallow", ACTION_NOOP);
    d.close();
    d.view(
        "a9_lv_col",
        "width: Fill height: Fit flow: Down spacing: 12 padding: Inset{left: 24 right: 24 top: 24 bottom: 24}",
    );
    d.text("a9_lv_title", title, &Txt::new(20.0, Face::Semibold, tok::TEXT).w(W::Fill).wrap());
    for (i, p) in body.iter().enumerate() {
        d.text(
            &format!("a9_lv_p{i}"),
            p,
            &Txt::new(14.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
        );
    }
    // The actions, right-aligned (`.actions`: flex-end, gap 10, top 22).
    d.view(
        "a9_lv_actions",
        "width: Fill height: Fit flow: Right spacing: 10 align: Align{x: 1.0 y: 0.5} margin: Inset{top: 10}",
    );
    action_button(&mut d, "a9_lv_cancel", "Cancel", ACTION_CANCEL, tok::TEXT);
    action_button(&mut d, "a9_lv_confirm", confirm_label, ACTION_CONFIRM, tok::RED);
    d.close();
    d.close(); // col
    d.close(); // dialog
    d.close(); // root
    let taps = d.taps.clone();
    Some(super::activity::Lowered { dsl: d.finish(), taps, inputs: Vec::new() })
}

/// The web's action button (`.actions button`: min-height 44, 10x16 padding,
/// a hairline border, radius 10, 14 px medium; the confirm in error text).
fn action_button(d: &mut Dsl, id: &str, label: &str, event: &str, fg: &'static str) {
    let w = ui::text_w(label, 14.0, Face::Medium) + 34.0;
    d.surface(
        &format!("{id}_box"),
        &format!("width: {w} height: 44 flow: Overlay align: Align{{x: 0.5 y: 0.5}}"),
        tok::SURFACE,
        10.0,
        Some("#d1d1d6ff"),
    );
    d.text(&format!("{id}_label"), label, &Txt::new(14.0, Face::Medium, fg));
    d.tap(id, event);
    d.close();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        crate::screens::theme::test_lock()
    }

    #[test]
    fn the_five_states_follow_the_transport_and_the_web_copy() {
        assert_eq!(status_of("Live", false), ConnStatus::Connected);
        assert_eq!(status_of("Dialing", false), ConnStatus::Connecting);
        assert_eq!(status_of("Handshaking", false), ConnStatus::Connecting);
        assert_eq!(status_of("Reconnecting { attempt: 2 }", false), ConnStatus::Connecting);
        assert_eq!(status_of("Failed", false), ConnStatus::Error);
        assert_eq!(status_of("Offline", false), ConnStatus::Disconnected);
        assert_eq!(status_of("Idle", false), ConnStatus::Idle);
        assert_eq!(status_of("", false), ConnStatus::Idle);
        assert_eq!(status_of("Offline", true), ConnStatus::Connecting, "a pressed Connect");
        let copy: Vec<_> = [ConnStatus::Idle, ConnStatus::Connecting, ConnStatus::Connected, ConnStatus::Disconnected, ConnStatus::Error]
            .iter()
            .map(|s| s.copy())
            .collect();
        assert_eq!(copy, ["Not connected", "Connecting…", "Connected", "Disconnected", "Connection error"]);
        assert_eq!(ConnStatus::Connected.dot(), Dot::Ok);
        assert_eq!(ConnStatus::Connecting.dot(), Dot::Busy);
        assert_eq!(ConnStatus::Error.dot(), Dot::Err);
        assert_eq!(ConnStatus::Disconnected.dot(), Dot::Idle);
    }

    #[test]
    fn info_rows_show_only_what_is_known() {
        let store = Store::new();
        assert!(info_rows(&store, "").is_empty(), "nothing known, nothing shown");
        store.set_active(Some("p:main".into()));
        store.domains.session.set_workspace_root("p:main", "/home/user/src/octos");
        let rows = info_rows(&store, "octoscode");
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].title, rows[0].value.as_str(), rows[0].description.as_str()), ("Current workspace", "octos", "/home/user/src/octos"));
        assert_eq!((rows[1].title, rows[1].value.as_str()), ("Profile", "octoscode"));
    }

    #[test]
    fn leaving_asks_only_when_work_or_a_draft_would_be_lost() {
        let _g = lock();
        cancel();
        assert_eq!(request(LeaveKind::Disconnect, false, false), Some(LeaveKind::Disconnect), "nothing to lose: act now");
        assert_eq!(pending(), None);
        assert_eq!(request(LeaveKind::Forget, true, false), None, "a live turn: ask");
        assert_eq!(pending(), Some(Leave { kind: LeaveKind::Forget, unsaved: false }));
        cancel();
        assert_eq!(confirm(), None, "a cancelled confirmation does nothing");
        assert_eq!(request(LeaveKind::Disconnect, false, true), None, "an unsaved draft: ask");
        let (title, body, label) = copy(&pending().unwrap());
        assert_eq!(title, "Disconnect from Octos?");
        assert_eq!(label, "Disconnect");
        assert!(body.contains(&"This input has not been saved. Copy it before leaving this conversation."));
        assert!(body.last().unwrap().contains("stays remembered"));
        assert_eq!(confirm(), Some(LeaveKind::Disconnect));
        assert_eq!(confirm(), None, "confirmed once");
    }

    #[test]
    fn the_forget_copy_names_what_is_removed_and_the_dialog_is_wired() {
        let _g = lock();
        request(LeaveKind::Forget, true, false);
        let (title, body, label) = copy(&pending().unwrap());
        assert_eq!((title, label), ("Forget this server?", "Forget server"));
        assert_eq!(body.len(), 2, "no draft paragraph without a draft");
        let low = lower(&Frame::DESKTOP).expect("pending");
        assert_eq!(crate::screens::taps::wired_taps(&low.dsl), low.taps);
        let events: Vec<&str> = low.taps.iter().map(|(_, e)| e.as_str()).collect();
        assert!(events.contains(&ACTION_CONFIRM) && events.contains(&ACTION_CANCEL));
        assert!(low.dsl.contains("width: 440"), "min(440, 100%)");
        let phone = lower(&Frame { avail_w: 360.0, avail_h: 780.0 }).unwrap();
        assert!(phone.dsl.contains("width: 320"), "the 20 px inset on a phone");
        cancel();
    }

    #[test]
    fn unfinished_work_counts_this_turn_and_other_live_sessions() {
        use octoscode_store::domains::session::Session;
        let store = Store::new();
        let s = |id: &str, live: bool| Session { id: id.into(), title: None, message_count: 1, updated_at: None, last_prompt: None, active_turn: live };
        store.set_sessions(vec![s("p:a", false), s("p:b", false)]);
        store.set_active(Some("p:a".into()));
        assert!(!unfinished_work(false, &store));
        assert!(unfinished_work(true, &store), "this session's turn");
        store.set_sessions(vec![s("p:a", false), s("p:b", true)]);
        assert!(unfinished_work(false, &store), "a background turn");
    }
}
