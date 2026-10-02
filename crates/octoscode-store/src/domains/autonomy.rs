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
    /// A10: the roster's "Last task" (`last_task ?? title ?? "—"`,
    /// web `AgentPanel.tsx:183`) and the status card's summary (`:262`).
    pub last_task: Option<String>,
    pub summary: Option<String>,
}

/// A10 — one artifact row of `agent/artifact/list` (web `AgentArtifact`,
/// `packages/client/src/autonomy.ts:1310-1316`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentArtifactRow {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub path: Option<String>,
}

/// A10 — the output viewer (`agent/output/read`): the text read so far and
/// the cursor a "Load more" continues from (web `state.agentOutput`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentOutputView {
    pub agent_id: String,
    pub text: String,
    pub next_offset: Option<u64>,
    pub has_more: bool,
}

/// A10 — the read artifact (`agent/artifact/read`): its row and content
/// (`None` = "No readable content available.").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentArtifactView {
    pub agent_id: String,
    pub artifact: AgentArtifactRow,
    pub content: Option<String>,
}

/// A10 — the Agents panel's single-viewer state (web `autonomy/store.ts`
/// `agentStatus` / `agentArtifacts` / `agentArtifact` / `agentOutput` /
/// `pendingAgentIds` / `agentDetailBusy` / `agentOutputBusy`). Status, the
/// artifact list and an artifact read share ONE detail viewer; the output has
/// its own. Each viewer is latest-request-wins: a result whose gate ticket is
/// no longer the newest is dropped (`store.ts:743-831`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentViewer {
    pub status: Option<AgentRecord>,
    pub artifacts: Option<(String, Vec<AgentArtifactRow>)>,
    pub artifact: Option<AgentArtifactView>,
    pub output: Option<AgentOutputView>,
    /// Agent ids with an interrupt/close in flight (a second click on the
    /// same id is refused — `controlAgent` returns `false`).
    pub pending: Vec<String>,
    pub detail_busy: bool,
    pub output_busy: bool,
    /// The last control's activity line ("Agent {id} {status}").
    pub activity: Option<String>,
}

/// The ticket a viewer read is dispatched under: its gate token, the
/// authority epoch and the `agents` revision at dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentTicket {
    pub token: u64,
    pub epoch: u64,
    pub revision: u64,
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
    // ---- P4e1b: the authority fence (web `autonomy/store.ts`) ----
    /// The commands identity this state belongs to (`store.ts:214-231`
    /// `#syncAuthority`). `None` = never bound.
    identity: Option<String>,
    /// Monotonic authority epoch, bumped on EVERY identity change. A result
    /// captured under an older epoch is a late old-client frame and is
    /// dropped (`store.ts:887-889` epoch admission).
    epoch: u64,
    /// The session this state is bound to. Switching sessions drops all
    /// prior autonomy state (`model.ts:101-104` `resetAutonomyForSession`).
    session_id: Option<String>,
    /// Per-family refresh revision (`store.ts:350-370`). Only an applied
    /// OWNING event/mutation on the SAME family supersedes that family's
    /// in-flight refresh snapshot; a foreign-family event never does.
    revisions: HashMap<String, u64>,
    /// Per-family error, recorded ONLY while the op stays authorized
    /// (`store.ts` `#runGuarded`: "records the error only while the op stays
    /// authorized"). Cleared by the authority change.
    errors: HashMap<String, String>,
    /// Per-family busy holder, stamped with the epoch that took it, so an
    /// authority change drops every busy/pending marker (`store.ts:222-224`).
    busy: HashMap<String, u64>,
    // ---- A10: the Agents panel viewers ----
    /// The roster in the server's list order (`agent/list`), new ids from
    /// `agent/updated` appended — the web keeps the list as the server sent
    /// it (`state.agents = result.agents`).
    agent_order: Vec<String>,
    viewer: AgentViewer,
    /// The newest detail (status / artifact list / artifact read) ticket.
    detail_gate: u64,
    /// The newest output ticket.
    output_gate: u64,
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
        let mut order: Vec<String> = Vec::new();
        for a in &agents {
            if !order.contains(&a.agent_id) {
                order.push(a.agent_id.clone());
            }
        }
        i.agent_order = order;
        i.agents = agents.into_iter().map(|a| (a.agent_id.clone(), a)).collect();
    }

    /// Upsert one agent (`agent/status/read` / `agent/updated`).
    pub fn upsert_agent(&self, agent: AgentRecord) {
        let mut i = self.inner.lock().unwrap();
        if !i.agent_order.contains(&agent.agent_id) {
            i.agent_order.push(agent.agent_id.clone());
        }
        i.agents.insert(agent.agent_id.clone(), agent);
    }

    pub fn agent(&self, agent_id: &str) -> Option<AgentRecord> {
        self.inner.lock().unwrap().agents.get(agent_id).cloned()
    }

    /// The roster in the server's order (ids never listed — a direct
    /// `upsert_agent` before any list — follow, sorted by id).
    pub fn agents(&self) -> Vec<AgentRecord> {
        let i = self.inner.lock().unwrap();
        let mut v: Vec<AgentRecord> = i.agent_order.iter().filter_map(|id| i.agents.get(id).cloned()).collect();
        let mut rest: Vec<AgentRecord> =
            i.agents.values().filter(|a| !i.agent_order.contains(&a.agent_id)).cloned().collect();
        rest.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
        v.extend(rest);
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

    // =======================================================================
    // P4e1b — the authority fence. Web oracle: `features/autonomy/store.ts`.
    //
    // Four rules, one mechanism:
    //   row 4 — an identity change bumps the epoch and drops everything;
    //            a result captured under an older epoch is refused;
    //   row 8 — an error is recorded only while its op holds the CURRENT
    //            epoch, and only for the family it belongs to;
    //   row 6 — a family is revision-guarded so a mid-refresh notification
    //            supersedes that family's stale snapshot, while a foreign
    //            family's event never does;
    //   row 9 — switching sessions drops all prior autonomy state.
    // =======================================================================

    /// #P4e1b row 4/9: bind this state to a commands identity. Returns `true`
    /// when the identity actually changed (an epoch bump happened).
    ///
    /// The identity is the client id + the session it is bound to — a new
    /// socket, a re-auth, a reconnect or a Core restart all present a new
    /// object for the same session id, and any of them retires the old one
    /// (`store.ts:208-213`: "ANY commands-identity change — a new object for
    /// the same session, re-auth, reconnect, or a Core daemon restart").
    pub fn bind_identity(&self, identity: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        if i.identity.as_deref() == Some(identity) && i.session_id.is_some() {
            return false;
        }
        let had_any = i.identity.is_some() || i.session_id.is_some();
        i.identity = Some(identity.to_owned());
        i.epoch += 1;
        // An identity change drops ALL busy/pending markers, revisions and
        // errors (`store.ts:222-224`) — they belonged to the retired authority.
        i.revisions.clear();
        i.busy.clear();
        i.errors.clear();
        // A10: the viewers belonged to the retired authority too.
        i.viewer = AgentViewer::default();
        if had_any {
            // `resetAutonomyForSession` (model.ts:101-104): a stale store must
            // never survive an identity change under the same session id.
            i.agents.clear();
            i.agent_order.clear();
            i.loops.clear();
            i.monitors.clear();
            i.goals.clear();
        }
        true
    }

    /// #P4e1b row 9: bind the state to a session. Returns `true` when the
    /// session actually changed, in which case all prior autonomy state is
    /// dropped. A same-id re-bind under a NEW identity still drops
    /// (`store.ts:113-115` "an identity change drops carried-over data under
    /// the same session id").
    pub fn bind_session(&self, session_id: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        if i.session_id.as_deref() == Some(session_id) {
            return false;
        }
        i.session_id = Some(session_id.to_owned());
        i.agents.clear();
        i.agent_order.clear();
        i.loops.clear();
        i.monitors.clear();
        i.goals.clear();
        i.revisions.clear();
        i.busy.clear();
        i.errors.clear();
        i.viewer = AgentViewer::default();
        true
    }

    // ---- A10: the Agents panel's viewers (web `autonomy/store.ts:743-869`) --

    /// A snapshot of the viewers.
    pub fn agent_viewer(&self) -> AgentViewer {
        self.inner.lock().unwrap().viewer.clone()
    }

    /// Begin a DETAIL read (status, artifact list or artifact read): the
    /// previous detail is cleared with the agents error (`clearAgentDetail`,
    /// `store.ts:971-979`), the viewer is busy, and the returned ticket is the
    /// newest — any older detail read in flight is now superseded.
    pub fn begin_agent_detail(&self) -> AgentTicket {
        let mut i = self.inner.lock().unwrap();
        i.detail_gate += 1;
        i.viewer.status = None;
        i.viewer.artifacts = None;
        i.viewer.artifact = None;
        i.viewer.detail_busy = true;
        i.errors.remove("agents");
        AgentTicket {
            token: i.detail_gate,
            epoch: i.epoch,
            revision: i.revisions.get("agents").copied().unwrap_or(0),
        }
    }

    /// Settle a detail read. `apply` runs only while the ticket is still the
    /// newest, the authority epoch is unchanged and no control/notification
    /// moved the `agents` revision since dispatch (web: "a status read
    /// captured before an acknowledged close cannot restore running
    /// details"). Returns whether it applied. Only the owning ticket clears
    /// the busy flag.
    pub fn finish_agent_detail(&self, ticket: AgentTicket, apply: impl FnOnce(&mut AgentViewer)) -> bool {
        let mut i = self.inner.lock().unwrap();
        if ticket.token != i.detail_gate {
            return false;
        }
        i.viewer.detail_busy = false;
        let current = i.identity.is_some()
            && ticket.epoch == i.epoch
            && ticket.revision == i.revisions.get("agents").copied().unwrap_or(0);
        if current {
            apply(&mut i.viewer);
        }
        current
    }

    /// Begin an OUTPUT read. A load-more continues from the viewer's cursor
    /// only for the SAME agent (otherwise a fresh read with no cursor —
    /// `store.ts:744-747`); the returned offset is the cursor to send.
    pub fn begin_agent_output(&self, agent_id: &str, load_more: bool) -> (AgentTicket, Option<u64>) {
        let mut i = self.inner.lock().unwrap();
        i.output_gate += 1;
        i.viewer.output_busy = true;
        let cursor = if load_more {
            i.viewer.output.as_ref().filter(|o| o.agent_id == agent_id).and_then(|o| o.next_offset)
        } else {
            None
        };
        (
            AgentTicket {
                token: i.output_gate,
                epoch: i.epoch,
                revision: i.revisions.get("agents").copied().unwrap_or(0),
            },
            cursor,
        )
    }

    /// Settle an output read: newest ticket + same epoch + the echoed agent is
    /// the requested one. A load-more on the same agent with a non-null echoed
    /// cursor APPENDS; anything else replaces (`store.ts:758-766`).
    pub fn finish_agent_output(
        &self,
        ticket: AgentTicket,
        requested: &str,
        echoed_cursor: bool,
        load_more: bool,
        out: AgentOutputView,
    ) -> bool {
        let mut i = self.inner.lock().unwrap();
        if ticket.token != i.output_gate {
            return false;
        }
        i.viewer.output_busy = false;
        if i.identity.is_none() || ticket.epoch != i.epoch || out.agent_id != requested {
            return false;
        }
        let append = load_more
            && echoed_cursor
            && i.viewer.output.as_ref().is_some_and(|o| o.agent_id == out.agent_id);
        if append {
            if let Some(o) = i.viewer.output.as_mut() {
                o.text.push_str(&out.text);
                o.next_offset = out.next_offset;
                o.has_more = out.has_more;
            }
        } else {
            i.viewer.output = Some(out);
        }
        i.errors.remove("agents");
        true
    }

    /// Begin an interrupt/close for `agent_id`: `None` when that agent already
    /// has a control in flight (the web's double-submit guard), else the
    /// epoch to settle under.
    pub fn begin_agent_control(&self, agent_id: &str) -> Option<u64> {
        let mut i = self.inner.lock().unwrap();
        if i.viewer.pending.iter().any(|p| p == agent_id) {
            return None;
        }
        i.viewer.pending.push(agent_id.to_owned());
        Some(i.epoch)
    }

    /// Settle a control. On success (`status` = the server's `interrupted` /
    /// `closed`) the roster row and the status card take the new status, the
    /// activity line names it, and the `agents` revision moves (so a detail
    /// read dispatched before it is dropped). The pending mark is released
    /// either way, but only while the authority is unchanged.
    pub fn finish_agent_control(&self, agent_id: &str, epoch: u64, status: Option<&str>) -> bool {
        let mut i = self.inner.lock().unwrap();
        if i.identity.is_none() || epoch != i.epoch {
            return false;
        }
        i.viewer.pending.retain(|p| p != agent_id);
        let Some(status) = status else { return false };
        if let Some(a) = i.agents.get_mut(agent_id) {
            a.status = status.to_owned();
        }
        if let Some(s) = i.viewer.status.as_mut().filter(|s| s.agent_id == agent_id) {
            s.status = status.to_owned();
        }
        i.viewer.activity = Some(format!("Agent {agent_id} {status}"));
        let next = i.revisions.get("agents").copied().unwrap_or(0) + 1;
        i.revisions.insert("agents".to_owned(), next);
        i.errors.remove("agents");
        true
    }

    /// The current authority epoch — the value a result must be captured under
    /// to be admitted.
    pub fn epoch(&self) -> u64 {
        self.inner.lock().unwrap().epoch
    }

    /// The bound session, if any.
    pub fn bound_session(&self) -> Option<String> {
        self.inner.lock().unwrap().session_id.clone()
    }

    /// #P4e1b row 4: is a result captured under `captured_epoch` still
    /// authorized? `false` after ANY identity change, so a late result from a
    /// retired commands identity is dropped rather than applied
    /// (`store.ts:887-889`; the same rule as the goal generation guard, one
    /// level up — that one orders goal events, this one orders commands).
    pub fn epoch_admits(&self, captured_epoch: u64) -> bool {
        let i = self.inner.lock().unwrap();
        i.identity.is_some() && captured_epoch == i.epoch
    }

    /// #P4e1b row 6: take a refresh revision for `family` (goal|loops|
    /// monitors|agents) and return the new value. Every refresh start takes
    /// one, so a later refresh always supersedes an earlier one's snapshot.
    pub fn bump_revision(&self, family: &str) -> u64 {
        let mut i = self.inner.lock().unwrap();
        let next = i.revisions.get(family).copied().unwrap_or(0) + 1;
        i.revisions.insert(family.to_owned(), next);
        next
    }

    /// The current revision of `family`.
    pub fn revision(&self, family: &str) -> u64 {
        self.inner.lock().unwrap().revisions.get(family).copied().unwrap_or(0)
    }

    /// #P4e1b row 6: an OWNING event on `family` supersedes that family's
    /// in-flight refresh. A foreign-family event must NOT (`store.ts:350-357`:
    /// "an unrelated-family owning event does not discard another family's
    /// refresh snapshot") — so the caller names its own family only.
    pub fn supersede_family(&self, family: &str) {
        let _ = self.bump_revision(family);
    }

    /// #P4e1b row 8: record a family error while the op stays authorized.
    /// The error is stamped with `captured_epoch`; if the authority has moved
    /// on it is dropped and NOT shown (`store.ts` "drops the error when a
    /// newer epoch superseded the request"). Returns whether it was recorded.
    pub fn record_error(&self, family: &str, captured_epoch: u64, message: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        if i.identity.is_none() || captured_epoch != i.epoch {
            return false;
        }
        i.errors.insert(family.to_owned(), message.to_owned());
        true
    }

    /// The recorded error for `family`, if any.
    pub fn error(&self, family: &str) -> Option<String> {
        self.inner.lock().unwrap().errors.get(family).cloned()
    }

    /// Clear a family error (the web nulls it on a successful op).
    pub fn clear_error(&self, family: &str) {
        self.inner.lock().unwrap().errors.remove(family);
    }

    /// Take a busy hold for `family` at `captured_epoch`.
    pub fn set_busy(&self, family: &str, captured_epoch: u64) {
        self.inner.lock().unwrap().busy.insert(family.to_owned(), captured_epoch);
    }

    /// Release a busy hold — but only when the holder is still the current
    /// authority, so a retired op cannot clear a live one's marker.
    pub fn release_busy(&self, family: &str, captured_epoch: u64) {
        let mut i = self.inner.lock().unwrap();
        if i.busy.get(family).copied() == Some(captured_epoch) {
            i.busy.remove(family);
        }
    }

    pub fn is_busy(&self, family: &str) -> bool {
        self.inner.lock().unwrap().busy.contains_key(family)
    }
}
