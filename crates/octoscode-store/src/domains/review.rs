//! `review` state. Stub for the fan-out lane (`review/start`).
use std::sync::Mutex;

/// The review domain.
#[derive(Debug, Default)]
pub struct Reviews {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    /// The last review id the server started, if any.
    last_started: Option<String>,
}

impl Reviews {
    pub fn note_started(&self, id: String) {
        self.inner.lock().unwrap().last_started = Some(id);
    }

    pub fn last_started(&self) -> Option<String> {
        self.inner.lock().unwrap().last_started.clone()
    }
}
