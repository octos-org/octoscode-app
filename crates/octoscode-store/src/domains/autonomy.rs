//! `autonomy` state: sub-agents, loops, monitors and session goals (M15).
//!
//! Card F1 owns this file. It holds the four M15 record families plus the
//! legacy generic entity map (#10's shape, kept so `tests/domains.rs` keeps
//! passing).
//!
//! **Generation rule (#1959).** `session/goal/updated` and
//! `session/goal/cleared` both carry a monotonic `generation`. A client MUST
//! drop a goal event whose `generation` is not greater than the last one it
//! applied for that session, so a stale update cannot overtake a clear and
//! resurrect the goal chip. `0` means "an older backend that doesn't stamp" —
//! treated as always-apply. `octos-core/src/ui_protocol.rs:5936-5963`.
use std::collections::HashMap;
use std::sync::Mutex;

/// One sub-agent, from `agent/list` / `agent/status/read` / `agent/updated`.
/// The fields are the ones the parity matrix's autonomy UI renders
/// (`docs/parity-matrix.csv`), flattened from `UiAgentRecord`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRecord {
    pub agent_id: String,
    pub session_id: String,
    pub profile_id: String,
    pub path: String,
    pub role: String,
    pub nickname: String,
    pub backend_kind: String,
    pub status: String,
    pub title: Option<String>,
    pub parent_agent_id: Option<String>,
    pub task_id: Option<String>,
    pub artifact_count: usize,
    pub output_tail: Option<String>,
    pub updated_at_ms: i64,
}

/// One recurring loop (`loop/*`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopRecord {
    pub loop_id: String,
    pub session_id: String,
    pub profile_id: Option<String>,
    pub prompt: String,
    pub mode: String,
    pub status: String,
    pub interval_seconds: Option<u64>,
    pub next_run_at_ms: Option<i64>,
    pub expires_at_ms: i64,
    pub updated_at_ms: i64,
    /// How many `loop/fired` events this store has seen for the loop — the
    /// fire counter the UI shows (the record itself carries no such field).
    pub fires: u64,
}

/// One monitor (`monitor/*`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRecord {
    pub monitor_id: String,
    pub session_id: String,
    pub profile_id: Option<String>,
    pub name: String,
    pub mode: String,
    pub status: String,
    pub pause_reason: Option<String>,
    pub fires_used: u32,
    pub last_fired_at_ms: Option<i64>,
    pub expires_at_ms: Option<i64>,
    pub updated_at_ms: i64,
}

/// One persisted session goal (`UiGoalRecord`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalRecord {
    pub goal_id: String,
    pub objective: String,
    pub status: String,
    pub token_budget: u64,
    pub tokens_used: u64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// A session's goal plus the last generation applied (#1959 ordering).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoalState {
    pub goal: Option<GoalRecord>,
    pub transition_actor: Option<String>,
    /// The last `generation` applied for this session. `0` = never stamped.
    pub generation: u64,
}

/// One autonomy entity (the #10 generic shape, kept for compatibility).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutonomyEntity {
    pub id: String,
    pub kind: String,
    pub detail: Option<String>,
}

#[derive(Debug, Default)]
struct Inner {
    entities: HashMap<(String, String), AutonomyEntity>,
    agents: HashMap<String, AgentRecord>,
    loops: HashMap<String, LoopRecord>,
    monitors: HashMap<String, MonitorRecord>,
    goals: HashMap<String, GoalState>,
}

/// The autonomy domain.
#[derive(Debug, Default)]
pub struct Autonomy {
    inner: Mutex<Inner>,
}

impl Autonomy {
    // ---- legacy generic entities (#10; tests/domains.rs depends on these) ---

    pub fn upsert(&self, entity: AutonomyEntity) {
        self.inner
            .lock()
            .unwrap()
            .entities
            .insert((entity.kind.clone(), entity.id.clone()), entity);
    }

    pub fn of_kind(&self, kind: &str) -> Vec<AutonomyEntity> {
        let i = self.inner.lock().unwrap();
        let mut v: Vec<AutonomyEntity> = i
            .entities
            .values()
            .filter(|e| e.kind == kind)
            .cloned()
            .collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().entities.len()
    }

    // ---- agents -----------------------------------------------------------

    /// Replace the agent list (`agent/list`).
    pub fn set_agents(&self, agents: Vec<AgentRecord>) {
        let mut i = self.inner.lock().unwrap();
        i.agents = agents.into_iter().map(|a| (a.agent_id.clone(), a)).collect();
    }

    /// Upsert one agent (`agent/status/read` / `agent/updated`).
    pub fn upsert_agent(&self, agent: AgentRecord) {
        self.inner
            .lock()
            .unwrap()
            .agents
            .insert(agent.agent_id.clone(), agent);
    }

    pub fn agent(&self, agent_id: &str) -> Option<AgentRecord> {
        self.inner.lock().unwrap().agents.get(agent_id).cloned()
    }

    pub fn agents(&self) -> Vec<AgentRecord> {
        let i = self.inner.lock().unwrap();
        let mut v: Vec<AgentRecord> = i.agents.values().cloned().collect();
        v.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
        v
    }

    pub fn agent_count(&self) -> usize {
        self.inner.lock().unwrap().agents.len()
    }

    // ---- loops ------------------------------------------------------------

    pub fn set_loops(&self, loops: Vec<LoopRecord>) {
        let mut i = self.inner.lock().unwrap();
        i.loops = loops.into_iter().map(|l| (l.loop_id.clone(), l)).collect();
    }

    /// Upsert a loop, preserving the local `fires` counter across updates.
    pub fn upsert_loop(&self, mut record: LoopRecord) {
        let mut i = self.inner.lock().unwrap();
        if let Some(prev) = i.loops.get(&record.loop_id) {
            record.fires = prev.fires;
        }
        i.loops.insert(record.loop_id.clone(), record);
    }

    pub fn remove_loop(&self, loop_id: &str) -> bool {
        self.inner.lock().unwrap().loops.remove(loop_id).is_some()
    }

    pub fn loop_record(&self, loop_id: &str) -> Option<LoopRecord> {
        self.inner.lock().unwrap().loops.get(loop_id).cloned()
    }

    pub fn loops(&self) -> Vec<LoopRecord> {
        let i = self.inner.lock().unwrap();
        let mut v: Vec<LoopRecord> = i.loops.values().cloned().collect();
        v.sort_by(|a, b| a.loop_id.cmp(&b.loop_id));
        v
    }

    pub fn loop_count(&self) -> usize {
        self.inner.lock().unwrap().loops.len()
    }

    /// Bump a loop's fire counter (`loop/fired`). Returns the new count.
    pub fn note_loop_fired(&self, loop_id: &str) -> u64 {
        let mut i = self.inner.lock().unwrap();
        let e = i.loops.entry(loop_id.to_owned()).or_insert_with(|| LoopRecord {
            loop_id: loop_id.to_owned(),
            session_id: String::new(),
            profile_id: None,
            prompt: String::new(),
            mode: String::new(),
            status: String::new(),
            interval_seconds: None,
            next_run_at_ms: None,
            expires_at_ms: 0,
            updated_at_ms: 0,
            fires: 0,
        });
        e.fires += 1;
        e.fires
    }

    // ---- monitors ---------------------------------------------------------

    pub fn set_monitors(&self, monitors: Vec<MonitorRecord>) {
        let mut i = self.inner.lock().unwrap();
        i.monitors = monitors
            .into_iter()
            .map(|m| (m.monitor_id.clone(), m))
            .collect();
    }

    pub fn upsert_monitor(&self, record: MonitorRecord) {
        self.inner
            .lock()
            .unwrap()
            .monitors
            .insert(record.monitor_id.clone(), record);
    }

    pub fn remove_monitor(&self, monitor_id: &str) -> bool {
        self.inner.lock().unwrap().monitors.remove(monitor_id).is_some()
    }

    pub fn monitor(&self, monitor_id: &str) -> Option<MonitorRecord> {
        self.inner.lock().unwrap().monitors.get(monitor_id).cloned()
    }

    pub fn monitors(&self) -> Vec<MonitorRecord> {
        let i = self.inner.lock().unwrap();
        let mut v: Vec<MonitorRecord> = i.monitors.values().cloned().collect();
        v.sort_by(|a, b| a.monitor_id.cmp(&b.monitor_id));
        v
    }

    pub fn monitor_count(&self) -> usize {
        self.inner.lock().unwrap().monitors.len()
    }

    /// Record a `monitor/fired` (last fire time). Returns the new fire count,
    /// or `None` when the monitor is unknown.
    pub fn note_monitor_fired(&self, monitor_id: &str, fired_at_ms: Option<i64>) -> Option<u32> {
        let mut i = self.inner.lock().unwrap();
        let m = i.monitors.get_mut(monitor_id)?;
        m.fires_used = m.fires_used.saturating_add(1);
        if fired_at_ms.is_some() {
            m.last_fired_at_ms = fired_at_ms;
        }
        Some(m.fires_used)
    }

    /// Mark a monitor expired (`monitor/expired`).
    pub fn mark_monitor_expired(&self, monitor_id: &str, reason: Option<String>) -> bool {
        let mut i = self.inner.lock().unwrap();
        match i.monitors.get_mut(monitor_id) {
            Some(m) => {
                m.status = "expired".to_owned();
                if reason.is_some() {
                    m.pause_reason = reason;
                }
                true
            }
            None => false,
        }
    }

    // ---- goals ------------------------------------------------------------

    /// Apply a `session/goal/updated`. Drops a stale event (#1959): returns
    /// whether it was applied. `generation == 0` is always applied.
    pub fn apply_goal_update(
        &self,
        session_id: &str,
        goal: GoalRecord,
        transition_actor: Option<String>,
        generation: u64,
    ) -> bool {
        let mut i = self.inner.lock().unwrap();
        let entry = i.goals.entry(session_id.to_owned()).or_default();
        if generation != 0 && generation <= entry.generation {
            return false;
        }
        entry.goal = Some(goal);
        if transition_actor.is_some() {
            entry.transition_actor = transition_actor;
        }
        entry.generation = entry.generation.max(generation);
        true
    }

    /// Apply a `session/goal/cleared`. Same stale rule; returns whether applied.
    pub fn apply_goal_clear(
        &self,
        session_id: &str,
        transition_actor: Option<String>,
        generation: u64,
    ) -> bool {
        let mut i = self.inner.lock().unwrap();
        let entry = i.goals.entry(session_id.to_owned()).or_default();
        if generation != 0 && generation <= entry.generation {
            return false;
        }
        entry.goal = None;
        if transition_actor.is_some() {
            entry.transition_actor = transition_actor;
        }
        entry.generation = entry.generation.max(generation);
        true
    }

    /// Write a goal unconditionally (an RPC result, not a notification).
    pub fn set_goal(&self, session_id: &str, state: GoalState) {
        self.inner.lock().unwrap().goals.insert(session_id.to_owned(), state);
    }

    pub fn goal(&self, session_id: &str) -> Option<GoalRecord> {
        self.inner
            .lock()
            .unwrap()
            .goals
            .get(session_id)
            .and_then(|s| s.goal.clone())
    }

    pub fn goal_generation(&self, session_id: &str) -> u64 {
        self.inner
            .lock()
            .unwrap()
            .goals
            .get(session_id)
            .map(|s| s.generation)
            .unwrap_or(0)
    }

    /// Every session with a goal state, sorted.
    pub fn goal_sessions(&self) -> Vec<String> {
        let i = self.inner.lock().unwrap();
        let mut v: Vec<String> = i.goals.keys().cloned().collect();
        v.sort();
        v
    }
}
