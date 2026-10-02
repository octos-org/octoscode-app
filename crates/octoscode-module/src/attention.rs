//! A25 — attention: the web's `features/attention` natively (parity rows
//! 276, 322, 323).
//!
//! | web | native |
//! |---|---|
//! | `attention/model.ts:24-94` `AttentionTracker` | [`AttentionTracker`] |
//! | `attention/model.ts:101-131` `foregroundAttentionTurns` | [`foreground_turns`] |
//! | `attention/desktop-notifications.ts:27-141` `DesktopNotifications` | [`DesktopNotifications`] |
//! | `attention/desktop-notifications.ts:17-24` `DesktopEnvironment` | [`NotifyOs`] (production: makepad's notification API, `patches/makepad/macos-notifications.patch`) |
//! | `attention/use-attention.ts:27-99` `useAttention` | [`Controller`] + the host's event arms (lib.rs) |
//! | `product-settings/GeneralSettingsContent.tsx:213-247` the toggle row | `chrome.rs` General > Desktop notifications, fed by [`settings`] |
//!
//! Native mapping (decided here, cited in docs/decisions/D10b-makepad-macos-notifications.md):
//! - The web's "visible" (`document.visibilityState`, use-attention.ts:50/80)
//!   is the app window's FOCUS: a native window has no tab visibility, and an
//!   unfocused window means the person is in another app. The first value is
//!   the platform's `Cx::focused_window()`, so a module opened mid-run (the
//!   phone shell launches it after the window already has focus) starts right.
//! - A click: the platform brings the app forward (makepad
//!   `notification.rs`: macOS activates the app, Android resumes the
//!   activity) — the web's `environment.focus()` (desktop-notifications.ts:105-106) —
//!   and the host opens the notice's Session (operator decision 2026-10-02;
//!   the web only focuses). The notice id carries the Session.
//! - The notice title is the Session's label (the OS already names the app
//!   in the banner); the body is the web's (desktop-notifications.ts:98-103).
//! - Tab counts (the browser title "(n) …", model.ts:96-98) have no native
//!   surface (row 319 stays B): the unread count only decides when the
//!   notice is withdrawn (use-attention.ts:86), so the web copy's "Tab counts
//!   still work" clauses are dropped.
//! - Permission is asynchronous on every native OS: the first query's answer
//!   applies the web constructor's rule (desktop-notifications.ts:33-46), and a
//!   post that finds the permission gone answers with an authorization result
//!   that applies the web's show()-time revocation rule (:89-96).
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use makepad_widgets::{Actions, Cx};

// ------------------------------------------------------------------ model.ts

/// `BackgroundTurnSessionScope`: the Session a turn belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct SessionScope {
    pub workspace_root: String,
    pub profile_id: String,
    pub session_id: String,
}

impl SessionScope {
    pub fn session(session_id: &str) -> Self {
        SessionScope { session_id: session_id.to_owned(), ..Default::default() }
    }
}

/// `BackgroundTurnState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnState {
    Running,
    Waiting,
    Completed,
    Failed,
}

impl TurnState {
    pub fn as_str(self) -> &'static str {
        match self {
            TurnState::Running => "running",
            TurnState::Waiting => "waiting",
            TurnState::Completed => "completed",
            TurnState::Failed => "failed",
        }
    }
}

/// `BackgroundTurnSnapshot` (and, with a non-running state, an
/// `AttentionNotice`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnSnapshot {
    pub scope: SessionScope,
    pub turn_id: String,
    pub state: TurnState,
}

/// model.ts:12-18.
fn session_key(scope: &SessionScope) -> String {
    serde_json::to_string(&[&scope.workspace_root, &scope.profile_id, &scope.session_id]).unwrap_or_default()
}

/// model.ts:19-21.
fn turn_key(turn: &TurnSnapshot) -> String {
    serde_json::to_string(&[session_key(&turn.scope), turn.turn_id.clone()]).unwrap_or_default()
}

/// model.ts:86 — the bounded observation window.
const OBSERVATION_WINDOW: usize = 128;

/// An insertion-ordered map (the web's `Map`): `set` keeps an existing key's
/// position.
fn map_set<V>(map: &mut Vec<(String, V)>, key: &str, value: V) {
    match map.iter_mut().find(|(k, _)| k == key) {
        Some(row) => row.1 = value,
        None => map.push((key.to_owned(), value)),
    }
}

fn map_get<'a, V>(map: &'a [(String, V)], key: &str) -> Option<&'a V> {
    map.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn map_delete<V>(map: &mut Vec<(String, V)>, key: &str) {
    map.retain(|(k, _)| k != key);
}

/// model.ts:23-94 — presentation-only transition memory; it never derives
/// execution outcomes.
#[derive(Debug, Default)]
pub struct AttentionTracker {
    /// The connection identity (the web compares an object by reference;
    /// natively a counter that moves on every new connection).
    identity: Option<u64>,
    observed: Vec<(String, TurnState)>,
    unread: Vec<(String, TurnSnapshot)>,
}

impl AttentionTracker {
    /// model.ts:29-31.
    pub fn count(&self) -> usize {
        self.unread.len()
    }

    /// model.ts:33-35.
    pub fn acknowledge_all(&mut self) {
        self.unread.clear();
    }

    /// model.ts:37-93. Returns the turns that newly need attention.
    pub fn observe(
        &mut self,
        identity: Option<u64>,
        turns: &[TurnSnapshot],
        selected: Option<&SessionScope>,
        visible: bool,
    ) -> Vec<TurnSnapshot> {
        let reset = self.identity != identity;
        if reset || identity.is_none() {
            self.identity = identity;
            self.observed.clear();
            self.unread.clear();
        }
        if identity.is_none() {
            return Vec::new();
        }
        let selected_key = selected.map(session_key);
        let mut next = self.observed.clone();
        let mut notices = Vec::new();
        // `new Map(turns.map(t => [turnKey(t), t]))`: one row per key, the
        // last value wins, the first position stays.
        let mut current: Vec<(String, &TurnSnapshot)> = Vec::new();
        for turn in turns {
            map_set(&mut current, &turn_key(turn), turn);
        }
        for (key, turn) in &current {
            let previous = map_get(&self.observed, key).copied();
            let settled = matches!(previous, Some(TurnState::Completed | TurnState::Failed));
            map_set(&mut next, key, if settled { previous.unwrap() } else { turn.state });
            let reading = visible && Some(session_key(&turn.scope)) == selected_key;
            if reading {
                map_delete(&mut self.unread, key);
            }
            if !reset
                && !reading
                && matches!(previous, Some(TurnState::Running | TurnState::Waiting))
                && previous != Some(turn.state)
                && turn.state != TurnState::Running
            {
                let notice = (*turn).clone();
                map_set(&mut self.unread, key, notice.clone());
                notices.push(notice);
            }
        }
        // model.ts:78-83 — opening a previously background Session
        // acknowledges it even when its owner row is gone.
        if visible {
            self.unread.retain(|(_, notice)| Some(session_key(&notice.scope)) != selected_key);
        }
        // model.ts:84-90 — a bounded observation window.
        while next.len() > OBSERVATION_WINDOW {
            let (oldest, _) = next.remove(0);
            map_delete(&mut self.unread, &oldest);
        }
        self.observed = next;
        notices
    }
}

/// model.ts:100-131 — only explicit turn terminal records can finish a
/// hidden foreground response. `terminal` is the newest terminal of the
/// selected Session that completed (`true`) or failed (`false`); an
/// interrupted or rate-limited turn is not one (the web's `status: "info"`,
/// timeline/model.ts:779-786).
pub fn foreground_turns(
    selected: Option<&SessionScope>,
    active_turn: Option<&str>,
    waiting_turn: Option<&str>,
    terminal: Option<(&str, bool)>,
) -> Vec<TurnSnapshot> {
    let Some(selected) = selected else { return Vec::new() };
    let mut rows = Vec::new();
    if let Some(active) = active_turn.filter(|a| Some(*a) != terminal.map(|t| t.0)) {
        rows.push(TurnSnapshot {
            scope: selected.clone(),
            turn_id: active.to_owned(),
            state: if Some(active) == waiting_turn { TurnState::Waiting } else { TurnState::Running },
        });
    }
    if let Some((turn, completed)) = terminal {
        rows.push(TurnSnapshot {
            scope: selected.clone(),
            turn_id: turn.to_owned(),
            state: if completed { TurnState::Completed } else { TurnState::Failed },
        });
    }
    rows
}

// ------------------------------------------------- desktop-notifications.ts

/// The OS's answer about the permission to post (makepad
/// `NotificationAuthorization`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Authorization {
    Unavailable,
    NotDetermined,
    Granted,
    Denied,
}

impl Authorization {
    pub fn as_str(self) -> &'static str {
        match self {
            Authorization::Unavailable => "unavailable",
            Authorization::NotDetermined => "not-determined",
            Authorization::Granted => "granted",
            Authorization::Denied => "denied",
        }
    }

    fn parse(s: &str) -> Option<Authorization> {
        Some(match s {
            "unavailable" => Authorization::Unavailable,
            "not-determined" | "default" => Authorization::NotDetermined,
            "granted" => Authorization::Granted,
            "denied" => Authorization::Denied,
            _ => return None,
        })
    }
}

/// What the OS answers (makepad posts these as actions; [`os_events`]
/// reads them back).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OsEvent {
    Authorization { status: Authorization, requested: bool, error: Option<String> },
    Clicked { id: String },
    Failed { id: String, error: String },
}

/// desktop-notifications.ts:7-14 (`onToggle` is the host's
/// `notifications_toggle.toggle`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttentionSettings {
    pub enabled: bool,
    pub pending: bool,
    pub available: bool,
    pub message: String,
    pub error: bool,
}

/// desktop-notifications.ts:17-24 — the OS seam. Every call is
/// fire-and-forget; the answers come back as [`OsEvent`]s.
pub trait NotifyOs {
    /// Read the permission (never prompts).
    fn query(&mut self);
    /// Ask for it (the OS may prompt, once).
    fn request(&mut self);
    /// Show (or replace) the notice keyed by `id`. Never focuses the app.
    fn post(&mut self, id: &str, title: &str, body: &str);
    /// Withdraw the notice keyed by `id`.
    fn close(&mut self, id: &str);
    /// desktop-notifications.ts:163-169 (absent / unreadable = false).
    fn read_preference(&self) -> bool;
    /// desktop-notifications.ts:170-176 (blocked storage keeps it in memory).
    fn save_preference(&mut self, enabled: bool);
}

/// desktop-notifications.ts:4-5, natively the board's copy (board 2 #6):
/// the web's "Tab counts are always on." has no native surface.
pub const DEFAULT_MESSAGE: &str =
    "Notify when a turn needs you or finishes while OctosCode is in the background";
/// :41.
pub const UNAVAILABLE_MESSAGE: &str = "Desktop notifications are unavailable here.";
/// :70.
pub const ON_MESSAGE: &str = "Desktop notifications are on.";
/// :73.
pub const NOT_GRANTED_MESSAGE: &str = "Permission was not granted. You can enable notifications later.";
/// :80.
pub const REQUEST_FAILED_MESSAGE: &str =
    "Could not enable desktop notifications. Try again when permissions allow.";
/// :92.
pub const PERMISSION_CHANGED_MESSAGE: &str = "Notification permission changed.";
/// :112.
pub const SHOW_FAILED_MESSAGE: &str = "OctosCode could not show a desktop notification.";

/// :72 — where the person allows them natively (one line on the desktop
/// row: `every_desktop_message_fits_one_line_of_the_row`).
pub fn blocked_message() -> &'static str {
    if cfg!(target_os = "android") {
        "Notifications are blocked. Allow them in Settings › Apps › OctosCode › Notifications."
    } else {
        "Notifications are blocked. Allow OctosCode in System Settings › Notifications."
    }
}

/// The notice title when the Session has no label (:158 `"Octoscode"`).
pub const NOTICE_TITLE: &str = "OctosCode";

/// :98-103.
pub fn notice_body(state: TurnState) -> &'static str {
    match state {
        TurnState::Waiting => "A background response needs your input. Return to OctosCode to review it.",
        TurnState::Failed => "A background response needs attention. Return to OctosCode to review it.",
        _ => "A background response finished. Return to OctosCode to review it.",
    }
}

/// The notice id: the web's single tag (:160 `octoscode-attention`) plus the
/// Session, so a click can open it.
pub const NOTICE_PREFIX: &str = "octoscode-attention:";

pub fn notice_id(session_id: &str) -> String {
    format!("{NOTICE_PREFIX}{session_id}")
}

pub fn notice_session(id: &str) -> Option<&str> {
    id.strip_prefix(NOTICE_PREFIX).filter(|s| !s.is_empty())
}

/// desktop-notifications.ts:27-141.
#[derive(Debug)]
pub struct DesktopNotifications {
    settings: AttentionSettings,
    /// The notice on screen (:29), by id.
    notice: Option<String>,
    /// :30 — every toggle-off and dispose moves it, so a permission that
    /// answers afterwards is ignored (:66, :77).
    request: u64,
    /// The `request` generation a pending request belongs to.
    awaiting: Option<u64>,
    /// The first authorization answer arrived (the web's constructor reads
    /// the permission synchronously; natively the first query answers).
    known: bool,
}

impl Default for DesktopNotifications {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopNotifications {
    /// :33-46, before the OS answered: off, assumed available, the default
    /// copy. [`Self::start`] asks.
    pub fn new() -> Self {
        DesktopNotifications {
            settings: AttentionSettings {
                enabled: false,
                pending: false,
                available: true,
                message: DEFAULT_MESSAGE.to_owned(),
                error: false,
            },
            notice: None,
            request: 0,
            awaiting: None,
            known: false,
        }
    }

    pub fn start(&mut self, os: &mut dyn NotifyOs) {
        os.query();
    }

    /// :48.
    pub fn settings(&self) -> &AttentionSettings {
        &self.settings
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// :54-84 (the request half; the answer is [`Self::authorization`]).
    pub fn toggle(&mut self, os: &mut dyn NotifyOs) {
        if self.settings.pending || !self.settings.available {
            return;
        }
        if self.settings.enabled {
            self.request += 1;
            self.clear(os);
            self.save(os, false, DEFAULT_MESSAGE, false);
            return;
        }
        self.request += 1;
        self.awaiting = Some(self.request);
        self.settings.pending = true;
        self.settings.error = false;
        os.request();
    }

    /// The OS answered a query, a request, or a post's permission check.
    pub fn authorization(
        &mut self,
        os: &mut dyn NotifyOs,
        status: Authorization,
        requested: bool,
        error: Option<String>,
    ) {
        if requested {
            // :65-83 — only the request still awaited (not toggled off or
            // disposed since) settles the toggle.
            if !self.settings.pending || self.awaiting != Some(self.request) {
                return;
            }
            self.awaiting = None;
            self.known = true;
            // The state the request left behind decides; an error only
            // matters when nothing was decided (a genuine failure, :76-83).
            // macOS answers a refused app's request with `granted: NO` AND
            // an error ("Notifications are not allowed for this
            // application") — that is "blocked" (:72), not "could not".
            match status {
                Authorization::Unavailable => {
                    self.settings.available = false;
                    self.save(os, false, UNAVAILABLE_MESSAGE, false);
                }
                Authorization::Granted => self.save(os, true, ON_MESSAGE, false),
                Authorization::Denied => self.save(os, false, blocked_message(), true),
                Authorization::NotDetermined if error.is_some() => self.save(os, false, REQUEST_FAILED_MESSAGE, true),
                Authorization::NotDetermined => self.save(os, false, NOT_GRANTED_MESSAGE, true),
            }
            return;
        }
        if !self.known {
            // :33-46 — the constructor: the remembered opt-in counts only
            // while the permission is granted; an unavailable OS disables
            // the toggle.
            self.known = true;
            self.settings.available = status != Authorization::Unavailable;
            self.settings.enabled = self.settings.available
                && status == Authorization::Granted
                && os.read_preference();
            self.settings.message =
                if self.settings.available { DEFAULT_MESSAGE } else { UNAVAILABLE_MESSAGE }.to_owned();
            return;
        }
        // :89-96 — a post found the permission gone: never silently reuse
        // the saved consent.
        if self.settings.enabled && status != Authorization::Granted {
            self.notice = None;
            if status == Authorization::Unavailable {
                self.settings.available = false;
            }
            self.save(os, false, PERMISSION_CHANGED_MESSAGE, true);
        }
    }

    /// :86-116.
    pub fn show(&mut self, os: &mut dyn NotifyOs, state: TurnState, scope: &SessionScope, label: Option<&str>) {
        if !self.settings.enabled {
            return;
        }
        self.clear(os);
        let id = notice_id(&scope.session_id);
        let title = label.map(str::trim).filter(|l| !l.is_empty()).unwrap_or(NOTICE_TITLE);
        os.post(&id, title, notice_body(state));
        self.notice = Some(id);
    }

    /// :109-115 — the OS could not show it.
    pub fn failed(&mut self, os: &mut dyn NotifyOs, id: &str) {
        if self.notice.as_deref() == Some(id) {
            self.notice = None;
        }
        if self.settings.enabled {
            self.save(os, false, SHOW_FAILED_MESSAGE, true);
        }
    }

    /// :105-108 — focus (the platform did it) and withdraw; returns the
    /// Session the clicked notice names.
    pub fn click(&mut self, os: &mut dyn NotifyOs, id: &str) -> Option<String> {
        self.clear(os);
        notice_session(id).map(str::to_owned)
    }

    /// :118-125.
    pub fn clear(&mut self, os: &mut dyn NotifyOs) {
        if let Some(id) = self.notice.take() {
            os.close(&id);
        }
    }

    /// :127-130.
    pub fn dispose(&mut self, os: &mut dyn NotifyOs) {
        self.request += 1;
        self.clear(os);
    }

    /// :132-135.
    fn save(&mut self, os: &mut dyn NotifyOs, enabled: bool, message: &str, error: bool) {
        os.save_preference(enabled);
        self.settings.enabled = enabled;
        self.settings.pending = false;
        self.settings.message = message.to_owned();
        self.settings.error = error;
    }
}

// ----------------------------------------------------------- use-attention.ts

/// One look at the app for the tracker (use-attention.ts:68-81).
#[derive(Clone, Debug, Default)]
pub struct Observation {
    /// App.tsx:383-386 `connectionIdentity` ([`Controller::identity_for`]).
    pub identity: Option<u64>,
    /// Background turns (none natively, row 239) + [`foreground_turns`].
    pub turns: Vec<TurnSnapshot>,
    pub selected: Option<SessionScope>,
    /// Session id -> its label, for the notice title.
    pub labels: HashMap<String, String>,
}

/// use-attention.ts:27-99 — the tracker, the desktop notices and the
/// window's focus.
#[derive(Debug, Default)]
pub struct Controller {
    pub tracker: AttentionTracker,
    pub desktop: DesktopNotifications,
    focused: bool,
    started: bool,
    /// use-attention.ts:45 `previousIdentity`.
    identity: Option<u64>,
    /// App.tsx:383-386 — the identity moves on every new live connection.
    epoch: u64,
    was_live: bool,
}

impl Controller {
    /// Once: read the permission (and the window's focus, from the platform).
    pub fn start(&mut self, os: &mut dyn NotifyOs, focused: bool) {
        if self.started {
            return;
        }
        self.started = true;
        self.focused = focused;
        self.desktop.start(os);
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    /// App.tsx:383-386: `session.authenticated ? {} : null`, a new object for
    /// every connection.
    pub fn identity_for(&mut self, live: bool) -> Option<u64> {
        if live && !self.was_live {
            self.epoch += 1;
        }
        self.was_live = live;
        live.then_some(self.epoch)
    }

    /// use-attention.ts:65-96.
    pub fn observe(&mut self, os: &mut dyn NotifyOs, obs: &Observation) -> Vec<TurnSnapshot> {
        if obs.identity != self.identity {
            self.desktop.clear(os);
        }
        self.identity = obs.identity;
        let notices = self.tracker.observe(obs.identity, &obs.turns, obs.selected.as_ref(), self.focused);
        if self.tracker.count() == 0 {
            self.desktop.clear(os);
        }
        for notice in &notices {
            let label = obs.labels.get(&notice.scope.session_id).map(String::as_str);
            self.desktop.show(os, notice.state, &notice.scope, label);
        }
        notices
    }

    /// use-attention.ts:49-54 (focus / visibilitychange): a focused window
    /// acknowledges everything and withdraws the notice. Losing focus only
    /// records it.
    pub fn set_focused(&mut self, os: &mut dyn NotifyOs, focused: bool) {
        self.focused = focused;
        if focused {
            self.tracker.acknowledge_all();
            self.desktop.clear(os);
        }
    }

    /// An OS answer. Returns the Session a clicked notice names (the host
    /// opens it).
    pub fn os_event(&mut self, os: &mut dyn NotifyOs, event: OsEvent) -> Option<String> {
        match event {
            OsEvent::Authorization { status, requested, error } => {
                self.desktop.authorization(os, status, requested, error);
                None
            }
            OsEvent::Clicked { id } => self.desktop.click(os, &id),
            OsEvent::Failed { id, .. } => {
                self.desktop.failed(os, &id);
                None
            }
        }
    }

    /// use-attention.ts:57-62 (the effect's cleanup).
    pub fn dispose(&mut self, os: &mut dyn NotifyOs) {
        self.desktop.dispose(os);
    }
}

// ------------------------------------------------------------ the store view

/// The selected Session's attention facts from the store and the flow
/// (`App.tsx:2238-2250` hands the same to `AttentionBridge`): the live turn
/// (`activeTurnId`), the turn waiting on an approval or a question
/// (`waitingTurnId`), and the newest completed/failed terminal.
pub fn observation(
    store: &octoscode_store::Store,
    active_turn: Option<String>,
    identity: Option<u64>,
) -> Observation {
    let labels: HashMap<String, String> = store
        .sessions()
        .into_iter()
        .filter_map(|s| s.label_stem().map(|l| (s.id.clone(), l)))
        .collect();
    let Some(session) = store.active_session() else {
        return Observation { identity, labels, ..Default::default() };
    };
    let scope = SessionScope {
        workspace_root: String::new(),
        profile_id: store.domains.profile.current().unwrap_or_default(),
        session_id: session.clone(),
    };
    let waiting = store
        .domains
        .approval
        .question()
        .filter(|q| q.session_id == session)
        .map(|q| q.turn_id)
        .or_else(|| store.domains.approval.showing(&session).map(|(_, d)| d.turn_id));
    let terminal = newest_terminal(store, &session);
    let turns = foreground_turns(
        Some(&scope),
        active_turn.as_deref(),
        waiting.as_deref(),
        terminal.as_ref().map(|(t, c)| (t.as_str(), *c)),
    );
    Observation { identity, turns, selected: Some(scope), labels }
}

/// The newest turn of `session` whose terminal completed (`true`) or failed
/// (`false`); interrupted and rate-limited turns are skipped (the web's
/// `status: "info"` terminals, which `findLast` passes over).
pub fn newest_terminal(store: &octoscode_store::Store, session: &str) -> Option<(String, bool)> {
    let entries = store.domains.session.timeline.entries(session);
    let mut seen: Vec<String> = Vec::new();
    for entry in entries.iter().rev() {
        let Some(turn) = entry.turn_id.as_deref() else { continue };
        if seen.iter().any(|t| t == turn) {
            continue;
        }
        seen.push(turn.to_owned());
        match store.domains.turn.terminal(turn).as_deref() {
            Some("completed") => return Some((turn.to_owned(), true)),
            Some("interrupted") | Some("rate_limited") | None => {}
            Some(_) => return Some((turn.to_owned(), false)),
        }
    }
    None
}

// --------------------------------------------------------- the OS adapters

/// makepad's notification API, when this build's makepad carries it
/// (`cfg(makepad_notifications)`, build.rs: the octosense host, APK and fork
/// builds compile `$OCTOSENSE_WORKSPACE/makepad` with
/// patches/makepad/macos-notifications.patch).
#[cfg(makepad_notifications)]
mod platform {
    use super::{Authorization, OsEvent};
    use makepad_widgets::makepad_platform::notification::{
        NotificationAuthorization as A, NotificationAuthorizationResult, NotificationClicked, NotificationFailed,
    };
    use makepad_widgets::{ActionTrait, Cx};

    pub const AVAILABLE: bool = true;

    pub fn query(cx: &mut Cx) {
        cx.query_notification_authorization();
    }
    pub fn request(cx: &mut Cx) {
        cx.request_notification_authorization();
    }
    pub fn post(cx: &mut Cx, id: &str, title: &str, body: &str) {
        cx.post_notification(id, title, body);
    }
    pub fn close(cx: &mut Cx, id: &str) {
        cx.close_notification(id);
    }
    pub fn focused(cx: &Cx) -> bool {
        cx.focused_window().is_some()
    }

    fn to_os(status: Authorization) -> A {
        match status {
            Authorization::Unavailable => A::Unavailable,
            Authorization::NotDetermined => A::NotDetermined,
            Authorization::Granted => A::Granted,
            Authorization::Denied => A::Denied,
        }
    }

    /// The fake backend and the test hooks answer through the SAME actions
    /// the platform posts, so they take the same arms.
    pub fn post_event(event: OsEvent) {
        match event {
            OsEvent::Authorization { status, requested, error } => {
                Cx::post_action(NotificationAuthorizationResult { status: to_os(status), requested, error })
            }
            OsEvent::Clicked { id } => Cx::post_action(NotificationClicked { id }),
            OsEvent::Failed { id, error } => Cx::post_action(NotificationFailed { id, error }),
        }
    }

    pub fn read(action: &dyn ActionTrait) -> Option<OsEvent> {
        if let Some(r) = action.downcast_ref::<NotificationAuthorizationResult>() {
            let status = match r.status {
                A::Unavailable => Authorization::Unavailable,
                A::NotDetermined => Authorization::NotDetermined,
                A::Granted => Authorization::Granted,
                A::Denied => Authorization::Denied,
            };
            return Some(OsEvent::Authorization { status, requested: r.requested, error: r.error.clone() });
        }
        if let Some(c) = action.downcast_ref::<NotificationClicked>() {
            return Some(OsEvent::Clicked { id: c.id.clone() });
        }
        if let Some(f) = action.downcast_ref::<NotificationFailed>() {
            return Some(OsEvent::Failed { id: f.id.clone(), error: f.error.clone() });
        }
        None
    }
}

/// This workspace compiles the pinned makepad git rev, which has no
/// notification API: every query answers `Unavailable` (honestly — nothing
/// can be posted), through the same action round trip.
#[cfg(not(makepad_notifications))]
mod platform {
    use super::{Authorization, OsEvent};
    use makepad_widgets::{ActionTrait, Cx};

    pub const AVAILABLE: bool = false;

    /// The answers, posted as one action type of our own.
    #[derive(Clone, Debug)]
    struct Answer(OsEvent);

    pub fn query(_cx: &mut Cx) {
        post_event(OsEvent::Authorization { status: Authorization::Unavailable, requested: false, error: None });
    }
    pub fn request(_cx: &mut Cx) {
        post_event(OsEvent::Authorization { status: Authorization::Unavailable, requested: true, error: None });
    }
    pub fn post(_cx: &mut Cx, _id: &str, _title: &str, _body: &str) {
        query(_cx);
    }
    pub fn close(_cx: &mut Cx, _id: &str) {}
    pub fn focused(_cx: &Cx) -> bool {
        false
    }
    pub fn post_event(event: OsEvent) {
        Cx::post_action(Answer(event));
    }
    pub fn read(action: &dyn ActionTrait) -> Option<OsEvent> {
        action.downcast_ref::<Answer>().map(|a| a.0.clone())
    }
}

/// Whether this build's makepad has the notification API.
pub fn platform_api() -> bool {
    platform::AVAILABLE
}

/// Production: makepad's API, the opt-in kept in
/// `OCTOSCODE_NOTIFICATIONS_FILE` (A7, screens::settings).
pub struct CxOs<'a> {
    pub cx: &'a mut Cx,
}

impl NotifyOs for CxOs<'_> {
    fn query(&mut self) {
        platform::query(self.cx);
    }
    fn request(&mut self) {
        platform::request(self.cx);
    }
    fn post(&mut self, id: &str, title: &str, body: &str) {
        makepad_widgets::log!("[octoscode] attention: notice {id} — {title}: {body}");
        platform::post(self.cx, id, title, body);
    }
    fn close(&mut self, id: &str) {
        makepad_widgets::log!("[octoscode] attention: notice {id} withdrawn");
        platform::close(self.cx, id);
    }
    fn read_preference(&self) -> bool {
        crate::screens::settings::notification_consent()
    }
    fn save_preference(&mut self, enabled: bool) {
        crate::screens::settings::save_notification_consent(enabled);
    }
}

/// The click walks' OS (`OCTOSCODE_NOTIFY_FAKE=<mode>`, tools/walk/
/// a25_notifications.py): a hidden test app is a bare binary, which no OS
/// lets post notices. It answers through the same actions the platform
/// posts and logs every post/close; nothing reaches the OS.
///
/// Modes: `granted` / `denied` / `default` (dismissed) / `error` (the request
/// fails) / `pending` (never answers) / `unavailable`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeOs {
    pub mode: String,
    pub status: Authorization,
}

impl FakeOs {
    pub fn from_env() -> Option<FakeOs> {
        let mode = std::env::var("OCTOSCODE_NOTIFY_FAKE").ok().filter(|m| !m.is_empty())?;
        let status = if mode == "unavailable" { Authorization::Unavailable } else { Authorization::NotDetermined };
        Some(FakeOs { mode, status })
    }

    fn answer(&self, requested: bool, error: Option<String>) {
        makepad_widgets::log!(
            "[octoscode] attention(fake os): authorization {} requested={requested}",
            self.status.as_str()
        );
        platform::post_event(OsEvent::Authorization { status: self.status, requested, error });
    }
}

struct FakeCxOs<'a> {
    fake: &'a mut FakeOs,
}

impl NotifyOs for FakeCxOs<'_> {
    fn query(&mut self) {
        self.fake.answer(false, None);
    }
    fn request(&mut self) {
        match self.fake.mode.as_str() {
            "pending" => makepad_widgets::log!("[octoscode] attention(fake os): request left pending"),
            "error" => self.fake.answer(true, Some("the request failed".to_owned())),
            mode => {
                self.fake.status = match mode {
                    "granted" => Authorization::Granted,
                    "denied" => Authorization::Denied,
                    "unavailable" => Authorization::Unavailable,
                    _ => Authorization::NotDetermined,
                };
                self.fake.answer(true, None);
            }
        }
    }
    fn post(&mut self, id: &str, title: &str, body: &str) {
        if self.fake.status != Authorization::Granted {
            self.fake.answer(false, None);
            return;
        }
        makepad_widgets::log!("[octoscode] attention(fake os): posted {id} | {title} | {body}");
    }
    fn close(&mut self, id: &str) {
        makepad_widgets::log!("[octoscode] attention(fake os): closed {id}");
    }
    fn read_preference(&self) -> bool {
        crate::screens::settings::notification_consent()
    }
    fn save_preference(&mut self, enabled: bool) {
        crate::screens::settings::save_notification_consent(enabled);
    }
}

// ------------------------------------------------------- the module's state

#[derive(Debug, Default)]
struct State {
    controller: Controller,
    fake: Option<FakeOs>,
}

fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(State { controller: Controller::default(), fake: FakeOs::from_env() }))
}

fn with_os<R>(cx: &mut Cx, f: impl FnOnce(&mut Controller, &mut dyn NotifyOs) -> R) -> R {
    let mut st = state().lock().unwrap();
    let State { controller, fake } = &mut *st;
    match fake {
        Some(fake) => f(controller, &mut FakeCxOs { fake }),
        None => f(controller, &mut CxOs { cx }),
    }
}

/// The Settings row's state (`GeneralSettingsContent.tsx:213-247`).
pub fn settings() -> AttentionSettings {
    state().lock().unwrap().controller.desktop.settings().clone()
}

/// The host's start: read the permission and the window's focus.
pub fn start(cx: &mut Cx) {
    let focused = platform::focused(cx);
    with_os(cx, |c, os| {
        makepad_widgets::log!(
            "[octoscode] attention: start (platform api {}, focused {focused})",
            platform::AVAILABLE
        );
        c.start(os, focused)
    });
}

/// `notifications_toggle.toggle` (the Settings row).
pub fn toggle(cx: &mut Cx) {
    with_os(cx, |c, os| c.desktop.toggle(os));
}

/// One sync: the store's facts through the tracker (use-attention.ts:65-96).
pub fn observe(cx: &mut Cx, store: &octoscode_store::Store, active_turn: Option<String>) {
    with_os(cx, |c, os| {
        let identity = c.identity_for(store.is_live());
        let obs = observation(store, active_turn, identity);
        for n in c.observe(os, &obs) {
            makepad_widgets::log!(
                "[octoscode] attention: {} turn {} of {} needs attention (focused {})",
                n.state.as_str(),
                n.turn_id,
                n.scope.session_id,
                c.focused()
            );
        }
    });
}

/// `WindowGotFocus` / `WindowLostFocus`.
pub fn focus(cx: &mut Cx, focused: bool) {
    with_os(cx, |c, os| {
        if focused && !c.focused() {
            makepad_widgets::log!("[octoscode] attention: window focused — acknowledged");
        }
        c.set_focused(os, focused)
    });
}

/// The OS answers in this `Actions` batch. Returns the Session a clicked
/// notice names.
pub fn handle_actions(cx: &mut Cx, actions: &Actions) -> Option<String> {
    let events: Vec<OsEvent> = actions.iter().filter_map(|a| platform::read(a.as_ref())).collect();
    let mut open = None;
    for event in events {
        makepad_widgets::log!("[octoscode] attention: os {event:?}");
        if let Some(session) = with_os(cx, |c, os| c.os_event(os, event)) {
            open = Some(session);
        }
    }
    open
}

/// The walks' hooks (the instrument's `/event?data=`): `octoscode.attention.
/// click:<notice id>` injects the click the platform would post, through the
/// same action; `octoscode.attention.focus:1|0` calls the focus arms'
/// handler (a hidden test window never gains focus). Returns whether `data`
/// was ours.
pub fn test_hook(cx: &mut Cx, data: &str) -> bool {
    if let Some(id) = data.strip_prefix("octoscode.attention.click:") {
        makepad_widgets::log!("[octoscode] attention: test hook click {id}");
        platform::post_event(OsEvent::Clicked { id: id.to_owned() });
        return true;
    }
    if let Some(v) = data.strip_prefix("octoscode.attention.focus:") {
        focus(cx, v == "1");
        return true;
    }
    if let Some(v) = data.strip_prefix("octoscode.attention.authorization:") {
        // The fake OS's permission changed under the app (System Settings).
        if let Some(status) = Authorization::parse(v) {
            if let Some(fake) = state().lock().unwrap().fake.as_mut() {
                fake.status = status;
            }
            return true;
        }
    }
    false
}

/// `Event::Shutdown` (use-attention.ts:60).
pub fn dispose(cx: &mut Cx) {
    with_os(cx, |c, os| c.dispose(os));
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------ a recording fake OS
    #[derive(Debug, Default)]
    struct Fake {
        queries: usize,
        requests: usize,
        posts: Vec<(String, String, String)>,
        closes: Vec<String>,
        preference: bool,
        saves: Vec<bool>,
    }

    impl NotifyOs for Fake {
        fn query(&mut self) {
            self.queries += 1;
        }
        fn request(&mut self) {
            self.requests += 1;
        }
        fn post(&mut self, id: &str, title: &str, body: &str) {
            self.posts.push((id.into(), title.into(), body.into()));
        }
        fn close(&mut self, id: &str) {
            self.closes.push(id.into());
        }
        fn read_preference(&self) -> bool {
            self.preference
        }
        fn save_preference(&mut self, enabled: bool) {
            self.preference = enabled;
            self.saves.push(enabled);
        }
    }

    fn scope(s: &str) -> SessionScope {
        SessionScope { workspace_root: "/workspace/one".into(), profile_id: "coding".into(), session_id: s.into() }
    }

    fn turn(s: &str, t: &str, state: TurnState) -> TurnSnapshot {
        TurnSnapshot { scope: scope(s), turn_id: t.into(), state }
    }

    /// The OS answered its first query.
    fn known(os: &mut Fake, status: Authorization) -> DesktopNotifications {
        let mut d = DesktopNotifications::new();
        d.start(os);
        d.authorization(os, status, false, None);
        d
    }

    // ------------------------------------------------- model.test.ts:15-152
    #[test]
    fn seeds_hydrated_terminal_and_waiting_states_without_replaying_notifications() {
        let mut t = AttentionTracker::default();
        let hydrated = [turn("s1", "turn-1", TurnState::Completed), turn("s2", "turn-1", TurnState::Waiting)];
        assert!(t.observe(Some(1), &hydrated, None, false).is_empty());
        assert!(t.observe(Some(1), &hydrated, None, false).is_empty());
        assert_eq!(t.count(), 0);
    }

    #[test]
    fn signals_each_observed_state_once_and_acknowledges_after_reopening() {
        for state in [TurnState::Completed, TurnState::Failed, TurnState::Waiting] {
            let mut t = AttentionTracker::default();
            let first = turn("s1", "turn-1", TurnState::Running);
            t.observe(Some(1), &[first.clone()], Some(&scope("s2")), true);
            let next = TurnSnapshot { state, ..first.clone() };
            assert_eq!(t.observe(Some(1), &[next.clone()], Some(&scope("s2")), true), vec![next.clone()]);
            assert!(t.observe(Some(1), &[next.clone()], Some(&scope("s2")), true).is_empty());
            assert_eq!(t.count(), 1);
            t.observe(Some(1), &[], Some(&scope("s1")), true);
            assert_eq!(t.count(), 0, "{state:?}: opening the Session acknowledges it");
            assert!(t.observe(Some(1), &[next], Some(&scope("s2")), true).is_empty());
        }
    }

    #[test]
    fn waiting_then_completed_is_one_unread_turn_but_two_signals() {
        let mut t = AttentionTracker::default();
        let other = Some(scope("s2"));
        t.observe(Some(1), &[turn("s1", "turn-1", TurnState::Running)], other.as_ref(), true);
        t.observe(Some(1), &[turn("s1", "turn-1", TurnState::Waiting)], other.as_ref(), true);
        assert_eq!(t.observe(Some(1), &[turn("s1", "turn-1", TurnState::Completed)], other.as_ref(), true).len(), 1);
        assert_eq!(t.count(), 1);
        // Terminal-to-terminal contradictions are not new lifecycle evidence.
        assert!(t.observe(Some(1), &[turn("s1", "turn-1", TurnState::Failed)], other.as_ref(), true).is_empty());
    }

    #[test]
    fn separates_identical_turn_and_session_ids_by_workspace_and_profile() {
        let mut t = AttentionTracker::default();
        let a = turn("s1", "turn-1", TurnState::Running);
        let b = TurnSnapshot { scope: SessionScope { workspace_root: "/workspace/two".into(), ..scope("s1") }, ..a.clone() };
        let c = TurnSnapshot { scope: SessionScope { profile_id: "other".into(), ..scope("s1") }, ..a.clone() };
        let rows = [a, b, c];
        t.observe(Some(1), &rows, None, true);
        let failed: Vec<_> = rows.iter().map(|r| TurnSnapshot { state: TurnState::Failed, ..r.clone() }).collect();
        assert_eq!(t.observe(Some(1), &failed, None, true).len(), 3);
        assert_eq!(t.count(), 3);
    }

    #[test]
    fn resets_unread_and_baselines_when_identity_changes_or_disconnects() {
        let mut t = AttentionTracker::default();
        t.observe(Some(1), &[turn("s1", "turn-1", TurnState::Running)], None, true);
        t.observe(Some(1), &[turn("s1", "turn-1", TurnState::Failed)], None, true);
        assert_eq!(t.count(), 1);
        assert!(t.observe(Some(2), &[turn("s1", "turn-1", TurnState::Failed)], None, true).is_empty());
        assert_eq!(t.count(), 0);
        assert!(t.observe(None, &[turn("s1", "turn-1", TurnState::Running)], None, true).is_empty());
        assert!(t.observe(None, &[turn("s1", "turn-1", TurnState::Completed)], None, true).is_empty());
        assert_eq!(t.count(), 0);
    }

    #[test]
    fn retains_evidence_when_queue_removal_and_terminal_arrive_separately() {
        let mut t = AttentionTracker::default();
        let s1 = Some(scope("s1"));
        t.observe(Some(1), &[turn("s1", "turn-1", TurnState::Running)], s1.as_ref(), true);
        assert!(t.observe(Some(1), &[], s1.as_ref(), false).is_empty());
        assert_eq!(t.count(), 0);
        assert_eq!(t.observe(Some(1), &[turn("s1", "turn-1", TurnState::Completed)], s1.as_ref(), false).len(), 1);
    }

    #[test]
    fn deduplicates_overlapping_owner_and_foreground_snapshots() {
        let mut t = AttentionTracker::default();
        let running = turn("s1", "turn-1", TurnState::Running);
        let done = turn("s1", "turn-1", TurnState::Completed);
        t.observe(Some(1), &[running.clone()], None, false);
        assert_eq!(t.observe(Some(1), &[running.clone(), done.clone()], None, false).len(), 1);
        assert!(t.observe(Some(1), &[running], None, false).is_empty());
        assert!(t.observe(Some(1), &[done], None, false).is_empty());
        assert_eq!(t.count(), 1);
    }

    #[test]
    fn bounds_observation_and_unread_history() {
        let mut t = AttentionTracker::default();
        t.observe(Some(1), &[], None, false);
        for i in 0..200 {
            let row = turn("s1", &format!("turn-{i}"), TurnState::Running);
            t.observe(Some(1), &[row.clone()], None, false);
            t.observe(Some(1), &[TurnSnapshot { state: TurnState::Failed, ..row }], None, false);
        }
        assert_eq!(t.count(), 128);
    }

    // -------------------------------------------- model.test.ts:154-193
    #[test]
    fn foreground_ignores_a_missing_selection_and_associates_waiting_with_the_active_turn() {
        let s = scope("s1");
        assert!(foreground_turns(None, Some("turn-1"), Some("turn-1"), Some(("turn-1", true))).is_empty());
        assert!(foreground_turns(Some(&s), None, None, None).is_empty());
        assert_eq!(foreground_turns(Some(&s), Some("turn-1"), Some("turn-other"), None)[0].state, TurnState::Running);
        assert_eq!(foreground_turns(Some(&s), Some("turn-1"), Some("turn-1"), None)[0].state, TurnState::Waiting);
        let rows = foreground_turns(Some(&s), Some("turn-2"), None, Some(("turn-1", false)));
        assert_eq!(
            rows.iter().map(|r| (r.turn_id.as_str(), r.state)).collect::<Vec<_>>(),
            vec![("turn-2", TurnState::Running), ("turn-1", TurnState::Failed)]
        );
        // The active turn that already settled is reported once, settled.
        let rows = foreground_turns(Some(&s), Some("turn-1"), None, Some(("turn-1", true)));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].state, TurnState::Completed);
    }

    #[test]
    fn newest_terminal_skips_interrupted_and_rate_limited_turns() {
        use octoscode_store::{EntryKind, Store};
        let store = Store::new();
        for (turn, outcome) in [("t1", "completed"), ("t2", "errored"), ("t3", "interrupted")] {
            store.domains.session.timeline.append(
                "s1",
                Some(turn.to_owned()),
                EntryKind::USER_MESSAGE,
                "prompt".to_owned(),
            );
            store.domains.turn.set_terminal(turn, outcome);
        }
        assert_eq!(newest_terminal(&store, "s1"), Some(("t2".to_owned(), false)));
        store.domains.turn.set_terminal("t2", "rate_limited");
        assert_eq!(newest_terminal(&store, "s1"), Some(("t1".to_owned(), true)));
        assert_eq!(newest_terminal(&store, "other"), None);
    }

    // --------------------------- desktop-notifications.test.ts:22-151
    #[test]
    fn does_not_prompt_or_notify_without_explicit_opt_in_even_with_existing_permission() {
        let mut os = Fake::default();
        let mut d = known(&mut os, Authorization::Granted);
        d.show(&mut os, TurnState::Completed, &scope("s1"), None);
        assert!(!d.settings().enabled);
        assert_eq!(os.requests, 0);
        assert!(os.posts.is_empty());
        assert_eq!(os.queries, 1, "reading the permission never prompts");
    }

    #[test]
    fn keeps_notifications_disabled_when_the_permission_prompt_is_dismissed() {
        let mut os = Fake::default();
        let mut d = known(&mut os, Authorization::NotDetermined);
        d.toggle(&mut os);
        assert!(d.settings().pending, "Enabling… while the OS prompt waits");
        d.authorization(&mut os, Authorization::NotDetermined, true, None);
        let s = d.settings();
        assert!(!s.enabled && !s.pending && s.error);
        assert_eq!(s.message, NOT_GRANTED_MESSAGE);
        d.show(&mut os, TurnState::Failed, &scope("s1"), None);
        assert!(os.posts.is_empty());
    }

    #[test]
    fn supports_unavailable_systems_without_requesting_permission() {
        let mut os = Fake::default();
        let mut d = known(&mut os, Authorization::Unavailable);
        d.toggle(&mut os);
        let s = d.settings();
        assert!(!s.enabled && !s.available && !s.pending);
        assert_eq!(s.message, UNAVAILABLE_MESSAGE);
        assert_eq!(os.requests, 0);
    }

    #[test]
    fn a_denied_prompt_reads_blocked_and_a_failed_request_reads_could_not_enable() {
        let mut os = Fake::default();
        let mut d = known(&mut os, Authorization::NotDetermined);
        d.toggle(&mut os);
        d.authorization(&mut os, Authorization::Denied, true, None);
        assert_eq!(d.settings().message, blocked_message());
        assert!(d.settings().error && !d.settings().enabled);
        d.toggle(&mut os);
        d.authorization(&mut os, Authorization::NotDetermined, true, Some("blocked".into()));
        assert_eq!(d.settings().message, REQUEST_FAILED_MESSAGE);
        assert!(d.settings().error && !d.settings().pending);
        assert_eq!(os.saves, vec![false, false]);
    }

    /// Measured on macOS 26 (tools/walk/a25_live_macos.py, a refused bundle):
    /// the request completes with granted = NO and the error "Notifications
    /// are not allowed for this application" — the settled status (Denied)
    /// decides: blocked, not "could not enable".
    #[test]
    fn a_refused_request_that_also_reports_an_error_still_reads_blocked() {
        let mut os = Fake::default();
        let mut d = known(&mut os, Authorization::Denied);
        d.toggle(&mut os);
        d.authorization(
            &mut os,
            Authorization::Denied,
            true,
            Some("Notifications are not allowed for this application".into()),
        );
        assert_eq!(d.settings().message, blocked_message());
        assert!(d.settings().error && !d.settings().enabled && !d.settings().pending);
    }

    #[test]
    fn a_granted_request_turns_it_on_and_persists_and_off_persists_too() {
        let mut os = Fake::default();
        let mut d = known(&mut os, Authorization::NotDetermined);
        d.toggle(&mut os);
        d.authorization(&mut os, Authorization::Granted, true, None);
        assert!(d.settings().enabled && !d.settings().error);
        assert_eq!(d.settings().message, ON_MESSAGE);
        d.toggle(&mut os);
        assert!(!d.settings().enabled);
        assert_eq!(d.settings().message, DEFAULT_MESSAGE);
        assert_eq!(os.saves, vec![true, false]);
        assert_eq!(os.requests, 1, "turning it off asks nothing");
    }

    #[test]
    fn the_remembered_opt_in_counts_only_while_the_permission_is_granted() {
        let mut os = Fake { preference: true, ..Default::default() };
        assert!(known(&mut os, Authorization::Granted).settings().enabled);
        let mut os = Fake { preference: true, ..Default::default() };
        let mut d = known(&mut os, Authorization::Denied);
        // desktop-notifications.test.ts:83 — revoked: no silent reuse.
        d.show(&mut os, TurnState::Completed, &scope("s1"), None);
        assert!(!d.settings().enabled);
        assert_eq!(os.requests, 0);
        assert!(os.posts.is_empty());
    }

    #[test]
    fn a_post_that_finds_the_permission_gone_disables_with_the_changed_message() {
        let mut os = Fake { preference: true, ..Default::default() };
        let mut d = known(&mut os, Authorization::Granted);
        d.show(&mut os, TurnState::Completed, &scope("s1"), Some("Fix it"));
        assert_eq!(os.posts.len(), 1);
        // The platform checked before posting and answered instead.
        d.authorization(&mut os, Authorization::Denied, false, None);
        let s = d.settings();
        assert!(!s.enabled && s.error);
        assert_eq!(s.message, PERMISSION_CHANGED_MESSAGE);
        assert_eq!(os.saves, vec![false]);
    }

    #[test]
    fn a_notice_the_os_could_not_show_disables_with_an_alert() {
        let mut os = Fake { preference: true, ..Default::default() };
        let mut d = known(&mut os, Authorization::Granted);
        d.show(&mut os, TurnState::Failed, &scope("s1"), None);
        d.failed(&mut os, &notice_id("s1"));
        assert!(!d.settings().enabled && d.settings().error);
        assert_eq!(d.settings().message, SHOW_FAILED_MESSAGE);
        assert_eq!(os.saves, vec![false]);
    }

    #[test]
    fn closes_stale_notices_and_focuses_only_on_an_explicit_click() {
        let mut os = Fake { preference: true, ..Default::default() };
        let mut d = known(&mut os, Authorization::Granted);
        d.show(&mut os, TurnState::Completed, &scope("s1"), Some("Fix steer queue"));
        assert_eq!(
            os.posts,
            vec![(notice_id("s1"), "Fix steer queue".into(), notice_body(TurnState::Completed).into())]
        );
        // Posting is the only OS call a notice makes (the NotifyOs seam has
        // no focus: only the platform's click handling brings the app forward).
        assert_eq!((os.requests, os.closes.len()), (0, 0));
        // The click: the Session it names, the notice withdrawn.
        assert_eq!(d.click(&mut os, &notice_id("s1")), Some("s1".to_owned()));
        assert_eq!(os.closes, vec![notice_id("s1")]);
        // A newer notice replaces the shown one (the web's single tag).
        d.show(&mut os, TurnState::Failed, &scope("s2"), None);
        d.show(&mut os, TurnState::Waiting, &scope("s2"), None);
        assert_eq!(os.closes, vec![notice_id("s1"), notice_id("s2")]);
        assert_eq!(os.posts[2].1, NOTICE_TITLE);
        d.dispose(&mut os);
        assert_eq!(os.closes.last(), Some(&notice_id("s2")));
    }

    #[test]
    fn ignores_permission_that_resolves_after_disposal_and_rejects_duplicate_clicks() {
        let mut os = Fake::default();
        let mut d = known(&mut os, Authorization::NotDetermined);
        d.toggle(&mut os);
        d.toggle(&mut os);
        assert_eq!(os.requests, 1, "a second click while pending asks nothing");
        d.dispose(&mut os);
        d.authorization(&mut os, Authorization::Granted, true, None);
        assert!(os.saves.is_empty(), "an answer after disposal is ignored");
        assert!(!d.settings().enabled);
    }

    #[test]
    fn handles_blocked_storage_the_opt_in_stays_in_memory() {
        let _g = crate::screens::theme::test_lock();
        // A file under a FILE cannot be written (storage blocked).
        let dir = std::env::temp_dir().join(format!("a25-blocked-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let blocker = dir.join("not-a-dir");
        std::fs::write(&blocker, b"x").unwrap();
        std::env::set_var("OCTOSCODE_NOTIFICATIONS_FILE", blocker.join("notifications.json"));
        assert!(!crate::screens::settings::notification_consent(), "unreadable = off");
        crate::screens::settings::save_notification_consent(true); // must not panic
        assert!(!crate::screens::settings::notification_consent(), "nothing was written");
        std::env::remove_var("OCTOSCODE_NOTIFICATIONS_FILE");
        // The state machine keeps the choice for this run.
        struct Blocked;
        impl NotifyOs for Blocked {
            fn query(&mut self) {}
            fn request(&mut self) {}
            fn post(&mut self, _: &str, _: &str, _: &str) {}
            fn close(&mut self, _: &str) {}
            fn read_preference(&self) -> bool {
                false
            }
            fn save_preference(&mut self, _: bool) {}
        }
        let mut d = DesktopNotifications::new();
        d.authorization(&mut Blocked, Authorization::NotDetermined, false, None);
        d.toggle(&mut Blocked);
        d.authorization(&mut Blocked, Authorization::Granted, true, None);
        assert!(d.settings().enabled);
    }

    /// The production opt-in storage (A7's `OCTOSCODE_NOTIFICATIONS_FILE`,
    /// the same two calls `CxOs` makes) through the whole round trip:
    /// toggle -> granted -> saved; a restart reads it back while granted;
    /// off saves off.
    #[test]
    fn the_opt_in_round_trips_through_the_preference_file() {
        let _g = crate::screens::theme::test_lock();
        let dir = std::env::temp_dir().join(format!("a25-optin-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("notifications.json");
        let _ = std::fs::remove_file(&file);
        std::env::set_var("OCTOSCODE_NOTIFICATIONS_FILE", &file);
        struct Stored;
        impl NotifyOs for Stored {
            fn query(&mut self) {}
            fn request(&mut self) {}
            fn post(&mut self, _: &str, _: &str, _: &str) {}
            fn close(&mut self, _: &str) {}
            fn read_preference(&self) -> bool {
                crate::screens::settings::notification_consent()
            }
            fn save_preference(&mut self, enabled: bool) {
                crate::screens::settings::save_notification_consent(enabled);
            }
        }
        let mut d = DesktopNotifications::new();
        d.authorization(&mut Stored, Authorization::Granted, false, None);
        assert!(!d.settings().enabled, "granted by the OS, but never opted in");
        d.toggle(&mut Stored);
        d.authorization(&mut Stored, Authorization::Granted, true, None);
        assert!(crate::screens::settings::notification_consent());
        let mut restarted = DesktopNotifications::new();
        restarted.authorization(&mut Stored, Authorization::Granted, false, None);
        assert!(restarted.settings().enabled, "the opt-in survives a restart");
        restarted.toggle(&mut Stored);
        assert!(!crate::screens::settings::notification_consent(), "and so does the opt-out");
        std::env::remove_var("OCTOSCODE_NOTIFICATIONS_FILE");
    }

    // ------------------------------- use-attention.ts / attention.spec.ts
    fn controller(os: &mut Fake, focused: bool) -> Controller {
        let mut c = Controller::default();
        c.start(os, focused);
        c.desktop.authorization(os, Authorization::Granted, false, None);
        c
    }

    fn obs(identity: Option<u64>, turns: Vec<TurnSnapshot>, selected: &str) -> Observation {
        let mut labels = HashMap::new();
        labels.insert("s1".to_owned(), "Fix steer queue".to_owned());
        Observation { identity, turns, selected: Some(scope(selected)), labels }
    }

    /// attention.spec.ts:124 — while the person reads (focused), a finished
    /// turn never notifies.
    #[test]
    fn a_focused_window_never_notifies_for_its_own_session() {
        let mut os = Fake { preference: true, ..Default::default() };
        let mut c = controller(&mut os, true);
        c.observe(&mut os, &obs(Some(1), vec![turn("s1", "t1", TurnState::Running)], "s1"));
        c.observe(&mut os, &obs(Some(1), vec![turn("s1", "t1", TurnState::Completed)], "s1"));
        assert!(os.posts.is_empty());
        assert_eq!(c.tracker.count(), 0);
    }

    /// attention.spec.ts:162 — unfocused: the selected Session's finished
    /// turn notifies ONCE (later syncs cannot notify twice); returning
    /// acknowledges and withdraws it.
    #[test]
    fn an_unfocused_finished_turn_notifies_once_and_focus_acknowledges_it() {
        let mut os = Fake { preference: true, ..Default::default() };
        let mut c = controller(&mut os, false);
        c.observe(&mut os, &obs(Some(1), vec![turn("s1", "t1", TurnState::Running)], "s1"));
        for _ in 0..3 {
            c.observe(&mut os, &obs(Some(1), vec![turn("s1", "t1", TurnState::Completed)], "s1"));
        }
        assert_eq!(
            os.posts,
            vec![(notice_id("s1"), "Fix steer queue".into(), notice_body(TurnState::Completed).into())]
        );
        assert_eq!(c.tracker.count(), 1);
        c.set_focused(&mut os, true);
        assert_eq!(c.tracker.count(), 0);
        assert_eq!(os.closes, vec![notice_id("s1")]);
        assert!(c.desktop.notice().is_none());
    }

    /// A turn waiting on the person while unfocused notifies "needs your
    /// input"; a failed one "needs attention".
    #[test]
    fn waiting_and_failed_turns_notify_with_their_copy() {
        let mut os = Fake { preference: true, ..Default::default() };
        let mut c = controller(&mut os, false);
        c.observe(&mut os, &obs(Some(1), vec![turn("s1", "t1", TurnState::Running)], "s1"));
        c.observe(&mut os, &obs(Some(1), vec![turn("s1", "t1", TurnState::Waiting)], "s1"));
        c.observe(&mut os, &obs(Some(1), vec![turn("s1", "t1", TurnState::Failed)], "s1"));
        let bodies: Vec<&str> = os.posts.iter().map(|p| p.2.as_str()).collect();
        assert_eq!(bodies, vec![notice_body(TurnState::Waiting), notice_body(TurnState::Failed)]);
        assert_eq!(os.closes, vec![notice_id("s1")], "the second notice replaced the first");
    }

    /// The click routes to its Session and withdraws the notice; the host
    /// opens that Session (lib.rs `open_attention_session`).
    #[test]
    fn the_click_routes_to_its_session() {
        let mut os = Fake { preference: true, ..Default::default() };
        let mut c = controller(&mut os, false);
        c.observe(&mut os, &obs(Some(1), vec![turn("s1", "t1", TurnState::Running)], "s1"));
        c.observe(&mut os, &obs(Some(1), vec![turn("s1", "t1", TurnState::Completed)], "s1"));
        let session = c.os_event(&mut os, OsEvent::Clicked { id: notice_id("s1") });
        assert_eq!(session.as_deref(), Some("s1"));
        assert_eq!(os.closes, vec![notice_id("s1")]);
        assert_eq!(c.os_event(&mut os, OsEvent::Clicked { id: "someone-else".into() }), None);
    }

    /// attention.spec.ts:193 — Disconnect (identity null) and a new
    /// connection reset the baselines and withdraw the notice; nothing
    /// replays.
    #[test]
    fn disconnect_and_identity_change_reset_and_withdraw() {
        let mut os = Fake { preference: true, ..Default::default() };
        let mut c = controller(&mut os, false);
        let id1 = c.identity_for(true);
        c.observe(&mut os, &obs(id1, vec![turn("s1", "t1", TurnState::Running)], "s1"));
        c.observe(&mut os, &obs(id1, vec![turn("s1", "t1", TurnState::Completed)], "s1"));
        assert_eq!(os.posts.len(), 1);
        let gone = c.identity_for(false);
        assert_eq!(gone, None);
        c.observe(&mut os, &obs(gone, vec![turn("s1", "t1", TurnState::Completed)], "s1"));
        assert_eq!(os.closes, vec![notice_id("s1")]);
        assert_eq!(c.tracker.count(), 0);
        let id2 = c.identity_for(true);
        assert_ne!(id1, id2, "a new connection is a new identity");
        c.observe(&mut os, &obs(id2, vec![turn("s1", "t1", TurnState::Completed)], "s1"));
        assert_eq!(os.posts.len(), 1, "a reconnect seeds the baseline, never replays");
        assert_eq!(c.identity_for(true), id2, "still the same connection");
    }

    /// Losing focus only records it; notifications stay off until opted in
    /// even when unfocused.
    #[test]
    fn unfocused_without_the_opt_in_posts_nothing() {
        let mut os = Fake::default();
        let mut c = Controller::default();
        c.start(&mut os, false);
        c.desktop.authorization(&mut os, Authorization::Granted, false, None);
        c.observe(&mut os, &obs(Some(1), vec![turn("s1", "t1", TurnState::Running)], "s1"));
        c.observe(&mut os, &obs(Some(1), vec![turn("s1", "t1", TurnState::Completed)], "s1"));
        assert!(os.posts.is_empty());
        assert_eq!(c.tracker.count(), 1, "the unread count still moves");
    }

    /// Every desktop message is ONE line under the Desktop notifications row
    /// (the help label is 499 px wide at 990 px, Inter 9.75 pt = 13 px): a
    /// second line pushed the General section past the Settings frame when
    /// the Current workspace row also wraps (seen live, A25).
    #[test]
    fn every_desktop_message_fits_one_line_of_the_row() {
        use crate::screens::board3::ui::{text_w, Face};
        let messages = [
            DEFAULT_MESSAGE,
            UNAVAILABLE_MESSAGE,
            ON_MESSAGE,
            NOT_GRANTED_MESSAGE,
            REQUEST_FAILED_MESSAGE,
            PERMISSION_CHANGED_MESSAGE,
            SHOW_FAILED_MESSAGE,
            blocked_message(),
        ];
        for m in messages {
            let w = text_w(m, 13.0, Face::Regular);
            assert!(w <= 490.0, "{w:.0} px: {m}");
        }
    }

    #[test]
    fn notice_ids_carry_their_session() {
        assert_eq!(notice_session(&notice_id("dsflash:main")), Some("dsflash:main"));
        assert_eq!(notice_session("octoscode-attention:"), None);
        assert_eq!(notice_session("other"), None);
    }

    #[test]
    fn this_workspace_build_reports_the_platform_api_honestly() {
        // The workspace compiles the pinned makepad git rev (no notification
        // API); the octosense builds set cfg(makepad_notifications).
        assert_eq!(platform_api(), cfg!(makepad_notifications));
    }
}
