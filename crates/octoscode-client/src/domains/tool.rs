//! `tool/*` — the tool inventory and tool lifecycle notifications.
//!
//! Implemented in this card: `tool/status/list` (an AppUI extension method)
//! and the `tool/started|progress|completed` notifications (recorded on the
//! store's seen-counter; their full timeline treatment is the fan-out lane's).
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octoscode_store::Store;

use crate::method::Method;
use crate::registry::{NotificationHandler, Registry};

/// `tool/status/list` — the AppUI extension (`packages/client/src/inventory.ts:171`).
/// Params mirror the web's `{session_id, profile_id, include_denied}`.
#[derive(Debug, Clone, Serialize)]
pub struct ToolStatusList {
    pub session_id: String,
    pub profile_id: String,
    pub include_denied: bool,
}

impl Default for ToolStatusList {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            profile_id: String::new(),
            include_denied: true,
        }
    }
}

/// One tool row (`RuntimeTool` in `packages/client/src/inventory.ts:15`).
/// Only the projected fields the web keeps; unknown server fields are ignored.
#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeTool {
    pub name: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub policy: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// The `tool/status/list` result (`RuntimeTools`).
#[derive(Debug, Clone, Deserialize)]
pub struct ToolStatusListResult {
    #[serde(default)]
    pub tools: Vec<RuntimeTool>,
}

impl Method for ToolStatusList {
    const NAME: &'static str = "tool/status/list";
    type Params = ToolStatusList;
    type Result = ToolStatusListResult;
}

/// `tool/started` — a tool call began.
pub struct ToolStartedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for ToolStartedHandler {
    const METHOD: &'static str = methods::TOOL_STARTED;
    fn handle(&self, notification: &UiNotification) {
        if matches!(notification, UiNotification::ToolStarted(_)) {
            self.store.note_seen(Self::METHOD);
        }
    }
}

/// `tool/progress` — a tool call reported progress.
pub struct ToolProgressHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for ToolProgressHandler {
    const METHOD: &'static str = methods::TOOL_PROGRESS;
    fn handle(&self, notification: &UiNotification) {
        if matches!(notification, UiNotification::ToolProgress(_)) {
            self.store.note_seen(Self::METHOD);
        }
    }
}

/// `tool/completed` — a tool call finished.
pub struct ToolCompletedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for ToolCompletedHandler {
    const METHOD: &'static str = methods::TOOL_COMPLETED;
    fn handle(&self, notification: &UiNotification) {
        if matches!(notification, UiNotification::ToolCompleted(_)) {
            self.store.note_seen(Self::METHOD);
        }
    }
}

pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(ToolStartedHandler { store: store.clone() });
    reg.register(ToolProgressHandler { store: store.clone() });
    reg.register(ToolCompletedHandler { store });
}
