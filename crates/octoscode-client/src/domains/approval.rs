//! `approval/*` — the approval sheet (request, decide, cancel).
//!
//! Requests: `approval/respond`, `approval/scopes/list` (stub — the fan-out
//! lane adds the typed [`crate::Method`] impls). Notifications:
//! `approval/requested`, `approval/decided`, `approval/cancelled`,
//! `approval/auto_resolved`.
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octoscode_store::Store;

use crate::registry::{NotificationHandler, Registry};

/// `approval/requested` — the server is asking the person to decide.
pub struct ApprovalRequestedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for ApprovalRequestedHandler {
    const METHOD: &'static str = methods::APPROVAL_REQUESTED;
    fn handle(&self, notification: &UiNotification) {
        if matches!(notification, UiNotification::ApprovalRequested(_)) {
            self.store.note_seen(Self::METHOD);
        }
    }
}

pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(ApprovalRequestedHandler { store });
}
