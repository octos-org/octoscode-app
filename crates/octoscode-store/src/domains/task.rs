//! `task` state: background tasks, their output, and the agent's plan.
//!
//! [F4] Owns: `task/list`, `task/cancel`, `task/artifact/list`,
//! `task/artifact/read` request state (the rows), plus the `task/updated`,
//! `task/output/delta` and `plan/updated` notifications.
//!
//! The row shape follows the web client's `SupervisedTask` projection
//! (`src-web/apps/web/src/features/supervision/model.ts:12`, `tasksFromList`
//! `:84`, `applyTaskUpdated` `:104`) and the plan follows `plan.ts`
//! (`applyPlanUpdated` `:20` replaces wholesale; `clearPlanForTurn` `:31` drops
//! a plan when its *authoring* turn terminates).
use std::collections::HashMap;
use std::sync::Mutex;

/// One background task, as the original stub described it (kept verbatim so
/// existing callers keep compiling).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub title: Option<String>,
    pub state: Option<String>,
}

/// A task row shaped for the supervision UI (`SupervisedTask`,
/// `model.ts:12`): the fields `task/list` and `task/updated` both carry, merged
/// the way the web's `applyTaskUpdated` merges a sparse live update onto a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSnapshot {
    pub id: String,
    pub tool_name: String,
    pub state: String,
    pub status: String,
    pub title: Option<String>,
    pub role: Option<String>,
    pub source: Option<String>,
    pub summary: Option<String>,
    pub artifact_count: u32,
    pub output_files: Vec<String>,
    pub error: Option<String>,
    pub updated_at: Option<String>,
    /// A6: `current_phase` (`SupervisedTask.phase`, `model.ts:98`). Set with
    /// [`TaskSnapshot::with_phase`]; a sparse live update keeps the row's.
    pub phase: Option<String>,
    /// A6: the session the row belongs to (`task/list`'s `session_id`, a
    /// `task/updated`'s own) — the trajectory is session-local
    /// (`SessionTrajectory.tsx:16`). `None` = unknown (kept for old callers).
    pub session_id: Option<String>,
}

impl TaskSnapshot {
    /// A6: attach the row's `current_phase`.
    pub fn with_phase(mut self, phase: Option<String>) -> Self {
        self.phase = phase;
        self
    }

    /// A6: attach the session the row was listed/updated under.
    pub fn with_session(mut self, session: &str) -> Self {
        self.session_id = Some(session.to_owned());
        self
    }

    /// The `task/list` projection (`tasksFromList`, `model.ts:84`).
    pub fn from_list_row(
        id: String,
        tool_name: String,
        state: String,
        status: String,
        title: Option<String>,
        role: Option<String>,
        source: Option<String>,
        summary: Option<String>,
        artifact_count: u32,
        output_files: Vec<String>,
        error: Option<String>,
        updated_at: Option<String>,
    ) -> Self {
        Self {
            id,
            tool_name,
            state,
            status,
            title,
            role,
            source,
            summary,
            artifact_count,
            output_files,
            error,
            updated_at,
            phase: None,
            session_id: None,
        }
    }
}

/// One plan item (`UiPlanItem`; status is the wire snake_case string).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanItem {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: Option<String>,
}

/// The agent's plan for one session. `plan/updated` REPLACES it wholesale
/// (`plan.ts:20`), so this is stored per session, not merged item-by-item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub items: Vec<PlanItem>,
    pub title: Option<String>,
    pub updated_at_ms: i64,
    /// The turn that authored the plan, when the server named one — the key
    /// `clear_plan_for_turn` matches on.
    pub turn_id: Option<String>,
}

/// The task domain.
#[derive(Debug, Default)]
pub struct Tasks {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    tasks: HashMap<String, Task>,
    /// The richer `task/list`/`task/updated` projection, keyed by task id.
    snapshots: HashMap<String, TaskSnapshot>,
    /// The plan per session (a plan with no authoring turn has no removal key).
    plans: HashMap<String, Plan>,
    /// Accumulated `task/output/delta` text per task.
    output: HashMap<String, String>,
    /// A6: recency per task id (the trajectory's newest-first order).
    touched: HashMap<String, u64>,
    touch_seq: u64,
}

impl Tasks {
    // ---- the original stub API (kept) ------------------------------------

    pub fn upsert(&self, task: Task) {
        self.inner.lock().unwrap().tasks.insert(task.id.clone(), task);
    }

    pub fn list(&self) -> Vec<Task> {
        let i = self.inner.lock().unwrap();
        let mut v: Vec<Task> = i.tasks.values().cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().tasks.len()
    }

    // ---- task rows (task/list + task/updated) ----------------------------

    /// Upsert one row, merging a sparse live update the way
    /// `applyTaskUpdated` does (`model.ts:104`): a field the incoming snapshot
    /// leaves empty keeps the existing value (tool name, output files, role,
    /// source, summary).
    pub fn upsert_snapshot(&self, incoming: TaskSnapshot) {
        let mut i = self.inner.lock().unwrap();
        let merged = match i.snapshots.get(&incoming.id) {
            None => incoming,
            Some(existing) => {
                let pick = |new: Option<String>, old: Option<String>| new.or(old);
                TaskSnapshot {
                    id: incoming.id.clone(),
                    tool_name: if incoming.tool_name.is_empty() {
                        existing.tool_name.clone()
                    } else {
                        incoming.tool_name.clone()
                    },
                    state: incoming.state.clone(),
                    status: incoming.status.clone(),
                    title: pick(incoming.title.clone(), existing.title.clone()),
                    role: pick(incoming.role.clone(), existing.role.clone()),
                    source: pick(incoming.source.clone(), existing.source.clone()),
                    summary: pick(incoming.summary.clone(), existing.summary.clone()),
                    artifact_count: if incoming.artifact_count == 0 {
                        existing.artifact_count
                    } else {
                        incoming.artifact_count
                    },
                    output_files: if incoming.output_files.is_empty() {
                        existing.output_files.clone()
                    } else {
                        incoming.output_files.clone()
                    },
                    error: pick(incoming.error.clone(), existing.error.clone()),
                    updated_at: pick(incoming.updated_at.clone(), existing.updated_at.clone()),
                    phase: pick(incoming.phase.clone(), existing.phase.clone()),
                    session_id: pick(incoming.session_id.clone(), existing.session_id.clone()),
                }
            }
        };
        // A6: the trajectory lists the newest update first (the web's
        // `applyTaskUpdated` returns `[next, ...rest]`, `model.ts:136`).
        i.touch_seq += 1;
        let seq = i.touch_seq;
        i.touched.insert(merged.id.clone(), seq);
        i.snapshots.insert(merged.id.clone(), merged);
    }

    /// A6: one session's rows, most recently listed/updated first (the web's
    /// trajectory order: `tasksFromList` keeps the server's order and every
    /// `task/updated` moves its row to the top, `model.ts:104-137`). A row
    /// with no recorded session belongs to no session's trajectory.
    pub fn session_rows(&self, session: &str) -> Vec<TaskSnapshot> {
        let i = self.inner.lock().unwrap();
        let mut v: Vec<(u64, TaskSnapshot)> = i
            .snapshots
            .values()
            .filter(|t| t.session_id.as_deref() == Some(session))
            .map(|t| (i.touched.get(&t.id).copied().unwrap_or(0), t.clone()))
            .collect();
        v.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
        v.into_iter().map(|(_, t)| t).collect()
    }

    /// A6: replace a session's rows with an authoritative `task/list`, in the
    /// server's order (first row on top), keeping rows of other sessions.
    pub fn replace_session_rows(&self, session: &str, rows: Vec<TaskSnapshot>) {
        let mut i = self.inner.lock().unwrap();
        let stale: Vec<String> = i
            .snapshots
            .values()
            .filter(|t| t.session_id.as_deref() == Some(session))
            .map(|t| t.id.clone())
            .collect();
        for id in stale {
            i.snapshots.remove(&id);
            i.touched.remove(&id);
        }
        // The server's first row ends with the highest sequence (top).
        let n = rows.len() as u64;
        let base = i.touch_seq;
        for (k, mut row) in rows.into_iter().enumerate() {
            row.session_id = Some(session.to_owned());
            i.touched.insert(row.id.clone(), base + n - k as u64);
            i.snapshots.insert(row.id.clone(), row);
        }
        i.touch_seq = base + n + 1;
    }

    pub fn snapshot(&self, id: &str) -> Option<TaskSnapshot> {
        self.inner.lock().unwrap().snapshots.get(id).cloned()
    }

    /// All rows, sorted by id (deterministic for tests).
    pub fn snapshots(&self) -> Vec<TaskSnapshot> {
        let i = self.inner.lock().unwrap();
        let mut v: Vec<TaskSnapshot> = i.snapshots.values().cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    pub fn snapshot_count(&self) -> usize {
        self.inner.lock().unwrap().snapshots.len()
    }

    // ---- plans (plan/updated) --------------------------------------------

    /// Replace a session's plan wholesale (`plan.ts:20`).
    pub fn set_plan(&self, session: &str, plan: Plan) {
        self.inner.lock().unwrap().plans.insert(session.to_owned(), plan);
    }

    pub fn plan(&self, session: &str) -> Option<Plan> {
        self.inner.lock().unwrap().plans.get(session).cloned()
    }

    /// Drop a session's plan when its authoring turn terminates. Turn-matched,
    /// so a replayed terminal for an older turn cannot clear a newer plan
    /// (`clearPlanForTurn`, `plan.ts:31`). Returns whether a plan was removed.
    pub fn clear_plan_for_turn(&self, session: &str, turn_id: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        let matches = i
            .plans
            .get(session)
            .map(|p| p.turn_id.as_deref() == Some(turn_id))
            .unwrap_or(false);
        if matches {
            i.plans.remove(session);
        }
        matches
    }

    // ---- task output (task/output/delta) ---------------------------------

    pub fn append_output(&self, task_id: &str, text: &str) {
        self.inner
            .lock()
            .unwrap()
            .output
            .entry(task_id.to_owned())
            .or_default()
            .push_str(text);
    }

    /// #P4b g-timeline [18]: the web's `appendTaskOutputDelta`
    /// (supervision/model.ts:143-165) in store form. `offset` is the frame's
    /// byte cursor: a frame ending at/before the expected offset is a STALE
    /// replay (no-op), a frame starting PAST it is a cursor GAP (fail closed —
    /// the buffer is never corrupted), an overlapping frame contributes only
    /// its non-overlapping suffix. A frame whose overlap boundary would split
    /// a UTF-8 char also fails closed (a malformed split is not a resync).
    /// Returns whether the buffer changed. The gap's user-facing error copy
    /// lands with the output drill-down UI (row [17]'s gap).
    pub fn append_output_checked(&self, task_id: &str, offset: u64, text: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        let expected = i.output.get(task_id).map(|s| s.len() as u64).unwrap_or(0);
        let delta_end = offset.saturating_add(text.len() as u64);
        if delta_end <= expected {
            return false; // stale replay: nothing to add
        }
        if offset > expected {
            return false; // cursor gap: fail closed
        }
        let overlap = (expected - offset) as usize;
        let Some(suffix) = text.get(overlap..) else {
            return false; // the overlap would split a UTF-8 char: malformed
        };
        i.output
            .entry(task_id.to_owned())
            .or_default()
            .push_str(suffix);
        true
    }

    pub fn output(&self, task_id: &str) -> String {
        self.inner
            .lock()
            .unwrap()
            .output
            .get(task_id)
            .cloned()
            .unwrap_or_default()
    }
}
