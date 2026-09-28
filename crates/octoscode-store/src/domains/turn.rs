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
}
