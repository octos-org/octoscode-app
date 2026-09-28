//! `session` state: the session list, the active session, and its transcript.
//!
//! Owns the [`Timeline`] the transcript entries land in; a lane that adds
//! session-scoped state adds it here and nowhere else.
use std::sync::Mutex;

use crate::timeline::Timeline;

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

/// The session domain: the list, the active id, and the transcript.
#[derive(Debug, Default)]
pub struct Sessions {
    inner: Mutex<Inner>,
    /// The transcript is its own lock: a `message/delta` fold must not block a
    /// session-list read, and the fan-out writes the two independently.
    pub timeline: Timeline,
}

#[derive(Debug, Default)]
struct Inner {
    sessions: Vec<Session>,
    active: Option<String>,
}

impl Sessions {
    /// Replace the list (from `session/list`). Keeps the active id if it still
    /// exists, else clears it.
    pub fn set_list(&self, sessions: Vec<Session>) {
        let mut i = self.inner.lock().unwrap();
        if let Some(active) = &i.active {
            if !sessions.iter().any(|s| &s.id == active) {
                i.active = None;
            }
        }
        i.sessions = sessions;
    }

    pub fn list(&self) -> Vec<Session> {
        self.inner.lock().unwrap().sessions.clone()
    }

    /// The session count — what the module tile shows.
    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().sessions.len()
    }

    pub fn set_active(&self, id: Option<String>) {
        self.inner.lock().unwrap().active = id;
    }

    pub fn active(&self) -> Option<String> {
        self.inner.lock().unwrap().active.clone()
    }
}
