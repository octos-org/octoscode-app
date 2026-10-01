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

impl Session {
    /// The shared label STEM: the trimmed title, else the trimmed last prompt,
    /// else `None`. The web normalizes both fields at ingestion
    /// (`workspace-session-catalog.ts:79-80` — `entry.title?.trim() || null`,
    /// the same for `last_prompt`), so a whitespace-only title never wins over
    /// a real prompt. `None` = this row has no human label at all.
    ///
    /// The two web projections differ only in what they do with that `None`:
    /// the sidebar row keeps it `null` (`workspace-session-catalog.ts:120` —
    /// `title: entry.title ?? entry.lastPrompt`) while the display label falls
    /// back to the id (`model.ts:71` — `... || session.id`). So the stem lives
    /// here and each projection layers its own fallback.
    pub fn label_stem(&self) -> Option<String> {
        let trimmed = |value: &Option<String>| -> Option<String> {
            value
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
        };
        trimmed(&self.title).or_else(|| trimmed(&self.last_prompt))
    }

    /// #P4h1 row 301 — the DISPLAY label, ported from the web's `sessionLabel`
    /// (`features/workspace/model.ts:71`):
    /// `session.title?.trim() || session.last_prompt?.trim() || session.id`.
    ///
    /// The sidebar's `thread_rows` read this. It used to pass the title
    /// through and fall straight to the id, so an untitled session showed its
    /// raw id instead of the last prompt the web shows — and a whitespace-only
    /// title rendered as blank.
    pub fn display_label(&self) -> String {
        self.label_stem().unwrap_or_else(|| self.id.clone())
    }
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
    /// #P4g1 row 204: the `session/open` reply's `workspace_root` per session
    /// (`SessionOpened.workspace_root`, ui_protocol.rs:4528 @ a6ea8505), plus
    /// the resume-open rejections (`(requested, returned)` per session) — the
    /// native fail-closed record for an open that answered a DIFFERENT
    /// workspace than the one asked (web
    /// `candidate-session.ts:230-243 validateCandidateWorkspace`).
    workspace_roots: HashMap<String, String>,
    workspace_rejects: HashMap<String, (String, String)>,
    /// Per-session thinking preferences (the web's reasoning-effort panel):
    /// effort low|medium|high|max, show-reasoning, default-on. Board-3's
    /// screens/board3.rs writes them; the strip/composer read them.
    thinking: HashMap<String, ThinkingPrefs>,
    /// The resume dialog's selected candidate (screen 7): selecting a row
    /// arms the confirm gate; the gate itself refuses without the exact
    /// typed match (the web's disabled-until-confirmed button).
    pending_resume: HashMap<String, usize>,
}

/// The thinking-effort panel's three values. The store keeps them per
/// session; the fail-closed default is row 11's contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThinkingPrefs {
    pub effort: String,
    pub show_reasoning: bool,
    pub default_on: bool,
    /// Which folded thinking blocks are open (the card authors `row_0` open).
    pub expanded: Vec<String>,
}

impl Default for ThinkingPrefs {
    fn default() -> Self {
        Self {
            effort: "high".into(),
            show_reasoning: true,
            default_on: true,
            expanded: vec!["row_0".into()],
        }
    }
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
    /// Fold a `session/list` reply into the list (#34b — the #39a row-2 live
    /// defect). The web's sidebar is the tab-known registry AUGMENTED by the
    /// workspace catalog (`App.tsx:747-756`: the catalog "is keyed to the
    /// profile this connection opens sessions under"; it only adds) — a
    /// catalog read never REMOVES a session the tab already knows. The real
    /// gate proved why: after New chat its reply named only the fresh session
    /// (the catalog lags the tab), so the old replace folded `dsflash:main`
    /// out of the store — the sidebar emptied (`sessions: 0` on #39a's run)
    /// and the running turn became unreachable. Server rows WIN on update;
    /// locally-known rows the reply omits are RETAINED after them.
    pub fn set_list(&self, sessions: Vec<Session>) {
        let mut i = self.inner.lock().unwrap();
        let server_ids: Vec<String> = sessions.iter().map(|s| s.id.clone()).collect();
        let mut merged = sessions;
        for known in i.sessions.drain(..) {
            if !server_ids.contains(&known.id) {
                merged.push(known);
            }
        }
        if let Some(active) = &i.active {
            if !merged.iter().any(|s| &s.id == active) {
                i.active = None;
            }
        }
        i.sessions = merged;
    }

    pub fn list(&self) -> Vec<Session> {
        self.inner.lock().unwrap().sessions.clone()
    }

    /// Record a session the server has confirmed **open** (card #14 defect 3).
    ///
    /// The gate showed `sessions: 0` after a successful open: the sidebar had
    /// nothing until a `session/list` reply arrived, and on that connection no
    /// such reply came. The web treats an opened session as known immediately
    /// (`session/opened` seeds the tab-known registry;
    /// `src-web/apps/web/src/features/session/known-session-registry.ts`), with
    /// the workspace catalog only *augmenting* it. Insert (or refresh) the row
    /// so an open is visible without waiting on a catalog read.
    pub fn note_opened(&self, id: &str, title: Option<String>) {
        let mut i = self.inner.lock().unwrap();
        match i.sessions.iter_mut().find(|s| s.id == id) {
            Some(existing) => {
                if let Some(title) = title {
                    existing.title = Some(title);
                }
            }
            None => i.sessions.push(Session {
                id: id.to_owned(),
                title,
                message_count: 0,
                updated_at: None,
                last_prompt: None,
                active_turn: false,
            }),
        }
    }

    /// The session count — what the module tile shows.
    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().sessions.len()
    }

    pub fn set_active(&self, id: Option<String>) {
        self.inner.lock().unwrap().active = id;
    }

    /// #P4g1 row 204: record the open reply's `workspace_root` for `id`.
    pub fn set_workspace_root(&self, id: &str, root: &str) {
        self.inner
            .lock()
            .unwrap()
            .workspace_roots
            .insert(id.to_owned(), root.to_owned());
    }

    pub fn workspace_root(&self, id: &str) -> Option<String> {
        self.inner.lock().unwrap().workspace_roots.get(id).cloned()
    }

    /// #P4g1 row 204: record a rejected resume open (requested vs returned
    /// workspace). The reject is the fail-closed observable; the caller
    /// decides what the user sees.
    pub fn note_workspace_reject(&self, id: &str, requested: &str, returned: &str) {
        self.inner
            .lock()
            .unwrap()
            .workspace_rejects
            .insert(id.to_owned(), (requested.to_owned(), returned.to_owned()));
    }

    /// Every recorded `(session, requested, returned)` open rejection.
    pub fn workspace_rejects(&self) -> Vec<(String, String, String)> {
        self.inner
            .lock()
            .unwrap()
            .workspace_rejects
            .iter()
            .map(|(k, (a, b))| (k.clone(), a.clone(), b.clone()))
            .collect()
    }
    /// The session's thinking prefs, or the fail-closed default (row 11).
    pub fn thinking(&self, sid: &str) -> ThinkingPrefs {
        self.inner
            .lock()
            .unwrap()
            .thinking
            .get(sid)
            .cloned()
            .unwrap_or_default()
    }

    pub fn set_thinking_effort(&self, sid: &str, effort: &str) {
        let mut i = self.inner.lock().unwrap();
        let mut t = i.thinking.get(sid).cloned().unwrap_or_default();
        t.effort = effort.to_owned();
        i.thinking.insert(sid.to_owned(), t);
    }

    pub fn set_show_reasoning(&self, sid: &str, on: bool) {
        let mut i = self.inner.lock().unwrap();
        let mut t = i.thinking.get(sid).cloned().unwrap_or_default();
        t.show_reasoning = on;
        i.thinking.insert(sid.to_owned(), t);
    }

    pub fn set_thinking_default_on(&self, sid: &str, on: bool) {
        let mut i = self.inner.lock().unwrap();
        let mut t = i.thinking.get(sid).cloned().unwrap_or_default();
        t.default_on = on;
        i.thinking.insert(sid.to_owned(), t);
    }

    pub fn set_thinking_expanded(&self, sid: &str, expanded: Vec<String>) {
        let mut i = self.inner.lock().unwrap();
        let mut t = i.thinking.get(sid).cloned().unwrap_or_default();
        t.expanded = expanded;
        i.thinking.insert(sid.to_owned(), t);
    }

    /// The resume dialog's selected candidate row (None = nothing selected).
    pub fn pending_resume(&self, sid: &str) -> Option<usize> {
        self.inner.lock().unwrap().pending_resume.get(sid).copied()
    }

    pub fn set_pending_resume(&self, sid: &str, row: usize) {
        self.inner
            .lock()
            .unwrap()
            .pending_resume
            .insert(sid.to_owned(), row);
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
