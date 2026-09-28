//! `task/*` — the task list, cancel, artifacts, and the agent's plan.
//!
//! [F4] owns: `task/list`, `task/cancel`, `task/artifact/list`,
//! `task/artifact/read` (requests) plus the `task/updated`,
//! `task/output/delta` and `plan/updated` notifications.
//!
//! Params/Result are the **octos-core** types at `a6ea8505`
//! (`crates/octos-core/src/ui_protocol.rs`), so the wire shape is the contract's,
//! not a re-derived guess. Web call sites (parity):
//! - `task/list` — `src-web/apps/web/src/features/supervision/use-supervision.ts:124`
//!   (`{ session_id }`) → `TaskListEntry[]` (`model.ts:84`).
//! - `task/cancel` — `use-supervision.ts:329` (`{ task_id, session_id }`).
//! - `task/artifact/list` — `use-supervision.ts:190` (`{ session_id, task_id }`).
//! - `task/artifact/read` — `use-supervision.ts:385` (`{ session_id, task_id,
//!   artifact_id, limit_bytes: 262_144 }`; a `cursor` on the load-more path `:439`).
//! - typed wrappers: `src-web/packages/client/src/client.ts:604,612,631,642`.
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octos_core::ui_protocol::{
    PlanUpdatedEvent, TaskArtifactListParams, TaskArtifactListResult, TaskArtifactReadParams,
    TaskArtifactReadResult, TaskCancelParams, TaskCancelResult, TaskListParams, TaskListResult,
    TaskOutputDeltaEvent, TaskUpdatedEvent,
};
use octoscode_store::domains::task::{Plan, PlanItem, TaskSnapshot};
use octoscode_store::Store;

use crate::method::Method;
use crate::registry::{NotificationHandler, Registry};

// ----------------------------------------------------------------- requests

/// `task/list` — the session's background tasks (`TaskListResult.tasks`).
pub struct TaskList;

impl Method for TaskList {
    const NAME: &'static str = methods::TASK_LIST;
    type Params = TaskListParams;
    type Result = TaskListResult;
}

/// `task/cancel` — cancel a running/pending task.
pub struct TaskCancel;

impl Method for TaskCancel {
    const NAME: &'static str = methods::TASK_CANCEL;
    type Params = TaskCancelParams;
    type Result = TaskCancelResult;
}

/// `task/artifact/list` — one task's artifacts.
pub struct TaskArtifactList;

impl Method for TaskArtifactList {
    const NAME: &'static str = methods::TASK_ARTIFACT_LIST;
    type Params = TaskArtifactListParams;
    type Result = TaskArtifactListResult;
}

/// `task/artifact/read` — one artifact's content (windowed by `limit_bytes`).
pub struct TaskArtifactRead;

impl Method for TaskArtifactRead {
    const NAME: &'static str = methods::TASK_ARTIFACT_READ;
    type Params = TaskArtifactReadParams;
    type Result = TaskArtifactReadResult;
}

// ------------------------------------------------------------ notifications

/// `task/updated` — a task transitioned; merge the sparse update onto a row the
/// way the web's `applyTaskUpdated` does (`supervision/model.ts:104`).
pub struct TaskUpdatedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for TaskUpdatedHandler {
    const METHOD: &'static str = methods::TASK_UPDATED;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::TaskUpdated(TaskUpdatedEvent {
            task_id,
            title,
            state,
            runtime_detail,
            source,
            role,
            summary,
            artifact_count,
            ..
        }) = notification
        {
            self.store.note_seen(Self::METHOD);
            let state = task_state_wire(*state);
            let snapshot = TaskSnapshot::from_list_row(
                task_id.0.to_string(),
                String::new(),
                state.to_owned(),
                runtime_detail.clone().unwrap_or_else(|| state.to_owned()),
                None, // a live update carries no stable title; keep the row's
                role.clone(),
                source.clone(),
                summary.clone(),
                artifact_count.unwrap_or(0),
                Vec::new(),
                // `state == "failed"` carries the runtime detail as the error
                // (`supervision/model.ts:132`).
                if state == "failed" {
                    runtime_detail.clone()
                } else {
                    None
                },
                None,
            );
            // `title` is the tool/task label on a live update; the store keeps
            // it only when the row is new (`tool_name` empty keeps the old).
            let mut snapshot = snapshot;
            if snapshot.tool_name.is_empty() {
                snapshot.tool_name = title.clone();
            }
            self.store.domains.task.upsert_snapshot(snapshot);
        }
    }
}

/// `task/output/delta` — streamed task output; accumulate per task.
pub struct TaskOutputDeltaHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for TaskOutputDeltaHandler {
    const METHOD: &'static str = methods::TASK_OUTPUT_DELTA;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::TaskOutputDelta(TaskOutputDeltaEvent { task_id, text, .. }) =
            notification
        {
            self.store.note_seen(Self::METHOD);
            self.store
                .domains
                .task
                .append_output(&task_id.0.to_string(), text);
        }
    }
}

/// `plan/updated` — the agent's checklist; REPLACES a session's plan wholesale
/// (`supervision/plan.ts:20`). The turn that authored it is kept so the plan can
/// be dropped on that turn's terminal (`plan.ts:31`) — that clearing is driven
/// by the turn domain, not here.
pub struct PlanUpdatedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for PlanUpdatedHandler {
    const METHOD: &'static str = methods::PLAN_UPDATED;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::PlanUpdated(PlanUpdatedEvent {
            session_id,
            turn_id,
            plan,
            ..
        }) = notification
        {
            self.store.note_seen(Self::METHOD);
            let items = plan
                .items
                .iter()
                .map(|item| PlanItem {
                    id: item.id.clone(),
                    title: item.title.clone(),
                    status: match item.status {
                        octos_core::ui_protocol::PlanItemStatus::Pending => "pending",
                        octos_core::ui_protocol::PlanItemStatus::InProgress => "in_progress",
                        octos_core::ui_protocol::PlanItemStatus::Completed => "completed",
                    }
                    .to_owned(),
                    priority: item.priority.clone(),
                })
                .collect();
            self.store.domains.task.set_plan(
                &session_id.0,
                Plan {
                    items,
                    title: plan.title.clone(),
                    updated_at_ms: plan.updated_at_ms,
                    turn_id: turn_id.as_ref().map(|t| t.0.to_string()),
                },
            );
        }
    }
}

/// The wire (snake_case) form of `TaskRuntimeState`
/// (`octos-core/src/ui_protocol.rs:5673`).
fn task_state_wire(state: octos_core::ui_protocol::TaskRuntimeState) -> &'static str {
    use octos_core::ui_protocol::TaskRuntimeState as S;
    match state {
        S::Pending => "pending",
        S::Running => "running",
        S::Completed => "completed",
        S::Failed => "failed",
        S::Cancelled => "cancelled",
    }
}

pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(TaskUpdatedHandler { store: store.clone() });
    reg.register(TaskOutputDeltaHandler { store: store.clone() });
    reg.register(PlanUpdatedHandler { store });
}

/// `task/output/read` — read a background task's output (card #13 §3).
///
/// The transport ALSO carries a typed `OutboundCommand::RequestTaskOutput`
/// (`octos-app-transport/src/proto.rs:173`); this `Method` makes it reachable
/// through the client's generic request path, like the web
/// (`packages/client/src/tasks.ts`). Params/result are the octos-core types.
pub struct TaskOutputRead;

impl Method for TaskOutputRead {
    const NAME: &'static str = methods::TASK_OUTPUT_READ;
    type Params = octos_core::ui_protocol::TaskOutputReadParams;
    type Result = octos_core::ui_protocol::TaskOutputReadResult;
}
