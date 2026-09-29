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
    /// Card #22 — the no-silent-drops guard: per-kind count of inbound
    /// notification kinds that reached NEITHER a handler NOR an explicit
    /// `ignored(reason)` entry. Empty in a healthy build; a non-zero entry names
    /// traffic we are silently dropping.
    unhandled: Mutex<HashMap<String, usize>>,
    /// Card #22 — per-kind count of `projection/envelope` payload `type`s folded
    /// by the envelope handler (every decoded payload kind lands here).
    payload_types: Mutex<HashMap<String, usize>>,
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

    /// Card #22: record that `method` reached neither a handler nor an explicit
    /// ignore entry (a silent drop unless the caller logs it).
    pub fn note_unhandled(&self, method: &str) {
        *self
            .unhandled
            .lock()
            .unwrap()
            .entry(method.to_owned())
            .or_insert(0) += 1;
    }

    /// Card #22: how many times `method` arrived unhandled.
    pub fn unhandled_count(&self, method: &str) -> usize {
        self.unhandled
            .lock()
            .unwrap()
            .get(method)
            .copied()
            .unwrap_or(0)
    }

    /// Card #22: every unhandled method, sorted (must be empty in a healthy run).
    pub fn unhandled_methods(&self) -> Vec<String> {
        let mut v: Vec<String> = self.unhandled.lock().unwrap().keys().cloned().collect();
        v.sort_unstable();
        v
    }

    /// Card #22: whether anything arrived unhandled.
    pub fn has_unhandled(&self) -> bool {
        !self.unhandled.lock().unwrap().is_empty()
    }

    /// Card #22: record one `projection/envelope` payload `type` folded.
    pub fn note_payload_type(&self, kind: &str) {
        *self
            .payload_types
            .lock()
            .unwrap()
            .entry(kind.to_owned())
            .or_insert(0) += 1;
    }

    /// Card #22: how many times a payload `type` was folded.
    pub fn payload_type_count(&self, kind: &str) -> usize {
        self.payload_types
            .lock()
            .unwrap()
            .get(kind)
            .copied()
            .unwrap_or(0)
    }

    /// Card #22: every payload `type` folded so far, sorted.
    pub fn payload_types(&self) -> Vec<String> {
        let mut v: Vec<String> = self.payload_types.lock().unwrap().keys().cloned().collect();
        v.sort_unstable();
        v
    }
}
