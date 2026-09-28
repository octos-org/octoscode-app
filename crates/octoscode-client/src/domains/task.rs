//! `task/*` — the task list, per-task output, and task artifacts.
//!
//! Requests: `task/list`, `task/cancel`, `task/restart_from_node`,
//! `task/output/read`, `task/artifact/list`, `task/artifact/read` (stubs).
//! Notifications: `task/updated`, `task/output/delta` (recorded).
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octoscode_store::Store;

use crate::registry::{NotificationHandler, Registry};

/// `task/updated` — a task transitioned.
pub struct TaskUpdatedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for TaskUpdatedHandler {
    const METHOD: &'static str = methods::TASK_UPDATED;
    fn handle(&self, notification: &UiNotification) {
        if matches!(notification, UiNotification::TaskUpdated(_)) {
            self.store.note_seen(Self::METHOD);
        }
    }
}

/// `task/output/delta` — streamed task output.
pub struct TaskOutputDeltaHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for TaskOutputDeltaHandler {
    const METHOD: &'static str = methods::TASK_OUTPUT_DELTA;
    fn handle(&self, notification: &UiNotification) {
        if matches!(notification, UiNotification::TaskOutputDelta(_)) {
            self.store.note_seen(Self::METHOD);
        }
    }
}

pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(TaskUpdatedHandler { store: store.clone() });
    reg.register(TaskOutputDeltaHandler { store });
}
