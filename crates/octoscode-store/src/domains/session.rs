//! `session` state: the session list, the active session, and its transcript.
//!
//! Owns the [`Timeline`] the transcript entries land in; a lane that adds
//! session-scoped state adds it here and nowhere else.
//!
//! **Card #F3** adds the session-scoped projections the `session/*` fan-out
//! owns: the last bridged legacy event (`session/event`), the whole-job
//! orchestration snapshot (`session/orchestration`), and the persisted goal
//! with the #1959 generation gate (`session/goal/updated` / `cleared`).
use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

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

/// The whole-job orchestration snapshot (`session/orchestration`,
/// `SessionOrchestrationEvent` `ui_protocol.rs:5170`). Mirrors the wire fields
/// so the UI can render a job indicator that survives the
/// sub-agent-complete → master-re-entry gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchestrationSnapshot {
    pub active: bool,
    pub running_agents: u32,
    pub pending_continuations: u32,
    pub phase: Option<String>,
}

/// The last bridged legacy SSE frame (`session/event`,
/// `SessionEventBridgedEvent` `ui_protocol.rs:6386`).
#[derive(Debug, Clone, PartialEq)]
pub struct BridgedEvent {
    pub kind: String,
    pub payload: serde_json::Value,
}

/// **Not wired (2026-09-28):** `session/goal/*` is owned by the autonomy domain
/// (`store.domains.autonomy`); this projection stays only for its unit tests
/// and will be removed. Bind the UI to autonomy's goal state.
///
/// The persisted goal projection, plus the #1959 generation gate.
///
/// `SessionGoalUpdatedEvent`/`SessionGoalClearedEvent` (`ui_protocol.rs:5936`
/// / `:5953`) carry a monotonic `generation`; a client MUST drop an update or
/// clear whose generation is not greater than the last applied one for that
/// session, so a stale update cannot overtake a clear and resurrect the chip.
/// `0` means an older backend that does not stamp — treat it as "always apply".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GoalState {
    /// The goal record, or `None` when cleared.
    pub goal: Option<serde_json::Value>,
    /// The highest generation applied so far for this session.
    pub generation: u64,
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
    /// Per-session last bridged `session/event` frame.
    bridged: HashMap<String, BridgedEvent>,
    /// Per-session orchestration snapshot.
    orchestration: HashMap<String, OrchestrationSnapshot>,
    /// Per-session goal + generation gate.
    goals: HashMap<String, GoalState>,
    /// Per-session last context lifecycle event (card #13): the context state
    /// plus the event kind that produced it (`compaction_started`,
    /// `compaction_completed`, `normalization_reported`). The web renders the
    /// compaction spinner/bar from exactly this
    /// (`src-web/apps/web/src/features/.../context-*`).
    context: HashMap<String, ContextLifecycle>,
}

/// One context lifecycle event, flattened from the `context/*` notifications
/// (card #13 §3). `kind` names which event set it; `state` is the
/// `UiContextState` JSON (so the store needs no octos-core dependency).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextLifecycle {
    pub kind: String,
    /// `UiContextState` as JSON (session_id, generation, token_estimate, …).
    pub state: serde_json::Value,
    /// Present on `compaction_completed` / `normalization_reported`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
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

    // ---- card #F3: session/event, session/orchestration, session/goal/* ----

    /// Record the last bridged legacy frame for a session (`session/event`).
    pub fn note_bridged_event(&self, session: &str, kind: &str, payload: serde_json::Value) {
        self.inner.lock().unwrap().bridged.insert(
            session.to_owned(),
            BridgedEvent {
                kind: kind.to_owned(),
                payload,
            },
        );
    }

    /// The last bridged legacy frame for a session, if any.
    pub fn bridged_event(&self, session: &str) -> Option<BridgedEvent> {
        self.inner.lock().unwrap().bridged.get(session).cloned()
    }

    /// Record the whole-job orchestration snapshot (`session/orchestration`).
    pub fn set_orchestration(&self, session: &str, snapshot: OrchestrationSnapshot) {
        self.inner
            .lock()
            .unwrap()
            .orchestration
            .insert(session.to_owned(), snapshot);
    }

    /// The orchestration snapshot for a session, if any.
    pub fn orchestration(&self, session: &str) -> Option<OrchestrationSnapshot> {
        self.inner.lock().unwrap().orchestration.get(session).cloned()
    }

    /// Record a context lifecycle event (`context/compaction_started`,
    /// `context/compaction_completed`, `context/normalization_reported`).
    pub fn set_context(&self, session: &str, event: ContextLifecycle) {
        self.inner
            .lock()
            .unwrap()
            .context
            .insert(session.to_owned(), event);
    }

    /// The last context lifecycle event for a session, if any.
    pub fn context(&self, session: &str) -> Option<ContextLifecycle> {
        self.inner.lock().unwrap().context.get(session).cloned()
    }

    /// Apply a goal update under the #1959 generation gate. Returns whether it
    /// was applied (a stale generation is dropped).
    pub fn apply_goal_update(
        &self,
        session: &str,
        generation: u64,
        goal: Option<serde_json::Value>,
    ) -> bool {
        let mut i = self.inner.lock().unwrap();
        let slot = i.goals.entry(session.to_owned()).or_default();
        // `0` = unstamped legacy backend -> always apply; otherwise strictly newer.
        if generation != 0 && generation <= slot.generation {
            return false;
        }
        slot.generation = generation.max(slot.generation);
        slot.goal = goal;
        true
    }

    /// Apply a goal clear under the same gate. Returns whether it was applied.
    pub fn apply_goal_clear(&self, session: &str, generation: u64) -> bool {
        let mut i = self.inner.lock().unwrap();
        let slot = i.goals.entry(session.to_owned()).or_default();
        if generation != 0 && generation <= slot.generation {
            return false;
        }
        slot.generation = generation.max(slot.generation);
        slot.goal = None;
        true
    }

    /// The persisted goal for a session, if one is set.
    pub fn goal(&self, session: &str) -> Option<serde_json::Value> {
        self.inner.lock().unwrap().goals.get(session)?.goal.clone()
    }

    /// The last goal generation applied for a session.
    pub fn goal_generation(&self, session: &str) -> u64 {
        self.inner
            .lock()
            .unwrap()
            .goals
            .get(session)
            .map(|g| g.generation)
            .unwrap_or(0)
    }
}
