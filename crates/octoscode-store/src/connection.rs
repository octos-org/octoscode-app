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
}

impl ConnectionState {
    /// Record a state transition. `live` is true only for `ConnectionState::Live`.
    pub fn set(&self, display: String, live: bool) {
        let mut i = self.inner.lock().unwrap();
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
}
