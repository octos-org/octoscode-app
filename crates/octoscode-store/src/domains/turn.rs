//! `turn` state: which turns are in flight, and the transcript folding.
//!
//! The folding itself lives in [`crate::timeline`] (it is shared shape, not
//! turn state); this domain owns *which* turn is current, so the notification
//! handlers and the module can ask without re-deriving it.
use std::collections::HashSet;
use std::sync::Mutex;

/// The turn domain: the turns this store has seen, per session.
#[derive(Debug, Default)]
pub struct Turns {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    /// Turns that started and have not completed/errored.
    in_flight: HashSet<String>,
    /// Every turn id seen, in arrival order (for a debug tile or a test).
    seen: Vec<String>,
    /// `turn/steer_dropped`: accepted-but-undrained steer inputs, per turn.
    /// Kept so the composer can hand the text back instead of losing it
    /// (`docs/parity/g-composer.csv` row 53).
    dropped_steers: Vec<DroppedSteer>,
    /// `thread/graph/get`: the last thread graph read, flattened to the
    /// fields the inspection surface shows (thread id, root seq, status and
    /// message count). Overwritten on each read.
    thread_rows: Vec<ThreadRow>,
}

/// One flattened `ThreadGraphEntry` (`thread/graph/get`, UPCR-2026-010):
/// the thread's id, its root message seq, the open status string, and how
/// many messages the thread owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRow {
    pub thread_id: String,
    pub root_seq: u64,
    pub status: String,
    pub message_count: usize,
}

/// One `turn/steer_dropped` payload: the session/turn it belongs to, the
/// inputs the server could not drain (buffer order preserved), and the reason
/// (`"interrupted"` when the client interrupted the turn, else `"turn_ended"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedSteer {
    pub session_id: String,
    pub turn_id: String,
    pub inputs: Vec<String>,
    pub reason: String,
}

impl Turns {
    pub fn started(&self, turn_id: &str) {
        let mut i = self.inner.lock().unwrap();
        i.in_flight.insert(turn_id.to_owned());
        i.seen.push(turn_id.to_owned());
    }

    pub fn ended(&self, turn_id: &str) {
        self.inner.lock().unwrap().in_flight.remove(turn_id);
    }

    /// Whether a turn is currently in flight.
    pub fn is_in_flight(&self, turn_id: &str) -> bool {
        self.inner.lock().unwrap().in_flight.contains(turn_id)
    }

    /// How many turns are in flight (across all sessions).
    pub fn in_flight_count(&self) -> usize {
        self.inner.lock().unwrap().in_flight.len()
    }

    /// Every turn id seen, in order.
    pub fn seen(&self) -> Vec<String> {
        self.inner.lock().unwrap().seen.clone()
    }

    /// Record a `turn/steer_dropped` payload. Appends, so a turn that drops
    /// twice (e.g. drained at interrupt, then again at end) keeps both, in
    /// arrival order — the UI restores them in that order.
    pub fn steer_dropped(
        &self,
        session_id: &str,
        turn_id: &str,
        inputs: Vec<String>,
        reason: &str,
    ) {
        self.inner.lock().unwrap().dropped_steers.push(DroppedSteer {
            session_id: session_id.to_owned(),
            turn_id: turn_id.to_owned(),
            inputs,
            reason: reason.to_owned(),
        });
    }

    /// Every dropped-steer payload seen, in arrival order.
    pub fn dropped_steers(&self) -> Vec<DroppedSteer> {
        self.inner.lock().unwrap().dropped_steers.clone()
    }

    /// Replace the thread graph with a fresh `thread/graph/get` read.
    pub fn set_threads(&self, rows: Vec<ThreadRow>) {
        self.inner.lock().unwrap().thread_rows = rows;
    }

    /// The last thread graph read (empty until one lands).
    pub fn threads(&self) -> Vec<ThreadRow> {
        self.inner.lock().unwrap().thread_rows.clone()
    }
}
