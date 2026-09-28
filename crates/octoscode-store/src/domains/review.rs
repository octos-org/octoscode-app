//! `review` state: the last native review the server started (`review/start`).
use std::sync::Mutex;

/// One accepted `review/start`: the Session and turn it belongs to and how
/// many review specialists the server admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartedReview {
    pub session_id: String,
    pub turn_id: String,
    pub agent_count: u32,
}

/// The review domain.
#[derive(Debug, Default)]
pub struct Reviews {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    /// The last review id the server started, if any.
    last_started: Option<String>,
    /// The last accepted `review/start`, if any.
    last_review: Option<StartedReview>,
}

impl Reviews {
    pub fn note_started(&self, id: String) {
        self.inner.lock().unwrap().last_started = Some(id);
    }

    pub fn last_started(&self) -> Option<String> {
        self.inner.lock().unwrap().last_started.clone()
    }

    /// Record an accepted `review/start` (the web's `ReviewStartResult`).
    pub fn note_review(&self, review: StartedReview) {
        self.inner.lock().unwrap().last_review = Some(review);
    }

    /// The last accepted review, if any.
    pub fn last_review(&self) -> Option<StartedReview> {
        self.inner.lock().unwrap().last_review.clone()
    }
}
