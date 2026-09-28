//! Diagnostics — what the store has seen, by notification method.
//!
//! A cheap, shared activity counter: every domain's handler calls
//! [`Diagnostics::note`] with its method name, so a tile or a test can ask
//! "did anything arrive, and of what kind?" without each domain exposing its
//! own counter. Not a protocol domain, so it lives at the store root.
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct Diagnostics {
    seen: Mutex<HashMap<String, usize>>,
}

impl Diagnostics {
    /// Record one arrival of `method`.
    pub fn note(&self, method: &str) {
        *self.seen.lock().unwrap().entry(method.to_owned()).or_insert(0) += 1;
    }

    /// How many times `method` has arrived.
    pub fn count(&self, method: &str) -> usize {
        self.seen.lock().unwrap().get(method).copied().unwrap_or(0)
    }

    /// The total across every method.
    pub fn total(&self) -> usize {
        self.seen.lock().unwrap().values().sum()
    }

    /// Every method seen so far, sorted (for a test or a debug tile).
    pub fn methods(&self) -> Vec<String> {
        let mut v: Vec<String> = self.seen.lock().unwrap().keys().cloned().collect();
        v.sort_unstable();
        v
    }
}
