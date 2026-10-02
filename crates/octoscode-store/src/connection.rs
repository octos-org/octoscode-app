//! Connection state — the transport's `ConnectionState`, for the UI.
//!
//! Not a protocol domain: every domain reads it, none owns it. Kept at the
//! store root beside [`crate::diagnostics`] for that reason.
use std::sync::Mutex;

/// The connection, as display text plus a liveness flag.
///
/// The display string is whatever the caller passes (the module formats
/// `ConnectionState` with `{:?}`, so `"Live"`, `"Reconnecting { attempt: 2 }"`).
/// Keeping it a string means this crate never depends on the transport's enum.
#[derive(Debug, Default)]
pub struct ConnectionState {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    display: String,
    live: bool,
    /// A12 — this connection has been `Live` at least once.
    ever_live: bool,
    /// A12 — the retained outage, if the live connection dropped.
    outage: Option<Outage>,
}

/// A12 — an outage of a connection that WAS live: the web keeps the opened
/// Session (sidebar + conversation) and shows "Reconnecting to Octos"
/// (`App.tsx:2724-2754`, `durable-session.ts:71-78`) until the transport is
/// back, instead of returning to the first-run Connect card. Only an explicit
/// leave (Disconnect / Forget, display `"Offline"`) ends it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outage {
    /// The server the transport keeps re-dialing (never a default).
    pub endpoint: String,
    /// Re-dial attempts so far, cumulative across transport restarts (the
    /// web's `Connection lost · retry N`).
    pub attempt: u32,
    /// The socket is back and the Session is being re-opened + re-hydrated
    /// (the web's "Restoring session state").
    pub restoring: bool,
    /// The re-open on the new socket failed (the web's "Session recovery
    /// required" with the reason, `App.tsx:2738-2744`); a retry clears it.
    pub error: Option<String>,
}

/// The display a voluntary leave (Disconnect / Forget / a confirmed stop)
/// records: it ends a retained outage, and nothing re-dialing may replace it.
pub const OFFLINE: &str = "Offline";

impl ConnectionState {
    /// Record a state transition. `live` is true only for `ConnectionState::Live`.
    pub fn set(&self, display: String, live: bool) {
        let mut i = self.inner.lock().unwrap();
        if live {
            i.ever_live = true;
            i.outage = None;
        }
        if display == OFFLINE {
            i.outage = None;
        }
        i.display = display;
        i.live = live;
    }

    /// The state as display text (e.g. `"Live"`).
    pub fn display(&self) -> String {
        self.inner.lock().unwrap().display.clone()
    }

    /// Whether the connection is `Live`.
    pub fn is_live(&self) -> bool {
        self.inner.lock().unwrap().live
    }

    /// A12 — whether this connection was ever `Live`.
    pub fn ever_live(&self) -> bool {
        self.inner.lock().unwrap().ever_live
    }

    /// A12 — whether a voluntary leave closed this connection.
    pub fn is_offline(&self) -> bool {
        self.inner.lock().unwrap().display == OFFLINE
    }

    /// A12 — the live connection dropped: retain the conversation and record
    /// the re-dial attempt against `endpoint`. Ignored once the connection was
    /// closed on purpose (`Offline`) or before it was ever live (a first
    /// connect that fails is the Connect card's failure, not an outage).
    /// Returns whether an outage is now retained.
    pub fn note_outage(&self, endpoint: &str, attempt: Option<u32>, restoring: bool) -> bool {
        let mut i = self.inner.lock().unwrap();
        if !i.ever_live || i.display == OFFLINE || i.live {
            return false;
        }
        let o = i.outage.get_or_insert_with(|| Outage {
            endpoint: endpoint.to_owned(),
            attempt: 0,
            restoring: false,
            error: None,
        });
        if !endpoint.is_empty() {
            o.endpoint = endpoint.to_owned();
        }
        if let Some(n) = attempt {
            o.attempt = o.attempt.max(n);
            o.error = None; // re-dialing again: the last failure is history
        }
        if restoring {
            o.error = None;
        }
        o.restoring = restoring;
        true
    }

    /// A12 — the re-open on the new socket failed: the retained outage now
    /// says why (until a retry or the next re-dial).
    pub fn note_outage_error(&self, reason: &str) {
        let mut i = self.inner.lock().unwrap();
        if let Some(o) = i.outage.as_mut() {
            o.error = Some(reason.to_owned());
            o.restoring = false;
        }
    }

    /// A12 — the cumulative attempt counter moves on by one (a fresh
    /// transport restarts its own count at 1).
    pub fn bump_outage_attempt(&self) {
        let mut i = self.inner.lock().unwrap();
        if let Some(o) = i.outage.as_mut() {
            o.attempt += 1;
            o.restoring = false;
        }
    }

    /// A12 — the retained outage, if any.
    pub fn outage(&self) -> Option<Outage> {
        self.inner.lock().unwrap().outage.clone()
    }

    /// A12 — end the outage without going live (the transport gave up and
    /// nothing will re-dial): the window may return to the Connect card.
    pub fn end_outage(&self) {
        self.inner.lock().unwrap().outage = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_outage_is_retained_only_after_live_and_until_live_or_offline() {
        let c = ConnectionState::default();
        c.set("Dialing".into(), false);
        assert!(!c.note_outage("http://h:1", Some(1), false), "a first connect is not an outage");
        assert!(c.outage().is_none());
        c.set("Live".into(), true);
        c.set("Reconnecting { attempt: 1 }".into(), false);
        assert!(c.note_outage("http://h:1", Some(1), false));
        c.note_outage("", Some(3), false);
        assert_eq!(c.outage().unwrap().attempt, 3);
        assert_eq!(c.outage().unwrap().endpoint, "http://h:1", "an empty endpoint keeps the known one");
        c.bump_outage_attempt();
        assert_eq!(c.outage().unwrap().attempt, 4, "a restarted transport keeps the count going");
        c.note_outage("http://h:1", Some(1), true);
        assert!(c.outage().unwrap().restoring);
        assert_eq!(c.outage().unwrap().attempt, 4, "never counts backwards");
        c.set("Live".into(), true);
        assert!(c.outage().is_none(), "live again ends it");
        c.set("Reconnecting { attempt: 1 }".into(), false);
        assert!(c.note_outage("http://h:1", Some(1), false));
        c.set(OFFLINE.into(), false);
        assert!(c.outage().is_none(), "a voluntary leave ends it");
        assert!(!c.note_outage("http://h:1", Some(2), false), "nothing re-opens it after Offline");
        assert!(c.is_offline());
    }
}
