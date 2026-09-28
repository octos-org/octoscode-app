//! octoscode-store — the session store every later lane writes into.
//!
//! **The public API here is ours.** It wraps `octos-app-store`'s session
//! concepts behind octoscode's own types so a fan-out lane can add state
//! without reaching into a dependency's internals. The store is a plain
//! struct with interior mutability ([`Store`]), shared as an `Arc` between
//! the UI thread (which renders from it) and the notification handlers (which
//! mutate it). Notifications reach it **only through the registry**
//! ([`crate::domains`]): no other code path writes to it.
//!
//! ## What it holds
//! - **connection state** — the transport's `ConnectionState`, as a display
//!   string (`"Live"`, `"Reconnecting{attempt:2}"`) plus a boolean "live".
//! - **sessions** — the `session/list` rows, and which one is active.
//! - **per-session timeline** — append-only entries (a `message/delta`
//!   appends text; a turn boundary appends a marker).
//! - **capabilities** — the accepted capability ids from the handshake.
use std::collections::HashMap;
use std::sync::Mutex;

/// One session row from `session/list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    pub title: Option<String>,
    pub message_count: usize,
    pub updated_at: Option<String>,
    pub last_prompt: Option<String>,
    /// Whether a turn is live in this session (from `SessionInfo.active_turn`).
    pub active_turn: bool,
}

/// One entry in a session's timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimelineEntry {
    /// Streamed assistant/reasoning text (`message/delta`).
    TextDelta { turn_id: String, text: String },
    /// A turn began.
    TurnStarted { turn_id: String },
    /// A turn ended (`completed` / `error`).
    TurnEnded { turn_id: String, error: Option<String> },
}

/// The store's mutable state. Private: callers go through [`Store`]'s methods.
#[derive(Debug, Default)]
struct Inner {
    connection: String,
    live: bool,
    capabilities: Vec<String>,
    sessions: Vec<Session>,
    active: Option<String>,
    timelines: HashMap<String, Vec<TimelineEntry>>,
    /// Notifications seen, by method — a cheap activity/diagnostic counter.
    seen: HashMap<String, usize>,
}

/// The session store. Cheap to clone-share: wrap in an `Arc`.
#[derive(Debug, Default)]
pub struct Store {
    inner: Mutex<Inner>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- connection state -------------------------------------------------

    /// Set from the transport's `ConnectionState`. Stores a display string
    /// (the exact `format!("{state:?}")` the caller passes) plus liveness.
    pub fn set_connection(&self, display: String, live: bool) {
        let mut i = self.inner.lock().unwrap();
        i.connection = display;
        i.live = live;
    }

    /// The connection state as display text (e.g. `"Live"`).
    pub fn connection(&self) -> String {
        self.inner.lock().unwrap().connection.clone()
    }

    /// Whether the connection is `Live`.
    pub fn is_live(&self) -> bool {
        self.inner.lock().unwrap().live
    }

    // ---- capabilities -----------------------------------------------------

    pub fn set_capabilities(&self, accepted: Vec<String>) {
        self.inner.lock().unwrap().capabilities = accepted;
    }

    pub fn capabilities(&self) -> Vec<String> {
        self.inner.lock().unwrap().capabilities.clone()
    }

    // ---- sessions ---------------------------------------------------------

    /// Replace the session list (from `session/list`). Keeps the active id if
    /// it still exists, else clears it.
    pub fn set_sessions(&self, sessions: Vec<Session>) {
        let mut i = self.inner.lock().unwrap();
        if let Some(active) = &i.active {
            if !sessions.iter().any(|s| &s.id == active) {
                i.active = None;
            }
        }
        i.sessions = sessions;
    }

    pub fn sessions(&self) -> Vec<Session> {
        self.inner.lock().unwrap().sessions.clone()
    }

    /// The session count — what the module tile shows.
    pub fn session_count(&self) -> usize {
        self.inner.lock().unwrap().sessions.len()
    }

    pub fn set_active(&self, id: Option<String>) {
        self.inner.lock().unwrap().active = id;
    }

    pub fn active_session(&self) -> Option<String> {
        self.inner.lock().unwrap().active.clone()
    }

    // ---- per-session timeline --------------------------------------------

    /// Append an entry to `session`'s timeline.
    pub fn push_timeline(&self, session: &str, entry: TimelineEntry) {
        self.inner
            .lock()
            .unwrap()
            .timelines
            .entry(session.to_owned())
            .or_default()
            .push(entry);
    }

    pub fn timeline(&self, session: &str) -> Vec<TimelineEntry> {
        self.inner
            .lock()
            .unwrap()
            .timelines
            .get(session)
            .cloned()
            .unwrap_or_default()
    }

    /// The concatenated `message/delta` text for a session — the live reply.
    pub fn live_text(&self, session: &str) -> String {
        self.timeline(session)
            .into_iter()
            .filter_map(|e| match e {
                TimelineEntry::TextDelta { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    }

    // ---- diagnostics ------------------------------------------------------

    /// Record that a notification method was seen (any handler).
    pub fn note_seen(&self, method: &str) {
        *self
            .inner
            .lock()
            .unwrap()
            .seen
            .entry(method.to_owned())
            .or_insert(0) += 1;
    }

    pub fn seen_count(&self, method: &str) -> usize {
        self.inner
            .lock()
            .unwrap()
            .seen
            .get(method)
            .copied()
            .unwrap_or(0)
    }

    /// A one-line summary for a tile: connection + session count.
    pub fn summary(&self) -> String {
        let i = self.inner.lock().unwrap();
        format!("conn: {}   sessions: {}", i.connection, i.sessions.len())
    }
}
