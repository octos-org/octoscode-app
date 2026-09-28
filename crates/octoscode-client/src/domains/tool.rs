//! `tool/*` — the tool inventory, MCP status, and tool lifecycle notifications.
//!
//! [F4] adds `mcp/status/list` (an AppUI **extension** method: no octos-core
//! type, so the params/result are documented serde structs). Server source:
//! octos-cli `api/ui_protocol_transport.rs:273` (const), `:19399` (dispatch),
//! `:11152` (`mcp_status_list_result` → `coding_tool_contract.rs:641`
//! `mcp_status_list_payload`). Web call site:
//! `src-web/packages/client/src/inventory.ts:182` (`{ session_id, profile_id,
//! include_disabled: true }`) with the row shape `RuntimeMcp` (`inventory.ts:30`).
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octoscode_store::domains::tool::{McpServer, McpStatus, McpSummary};
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

/// `mcp/status/list` — the AppUI extension (`packages/client/src/inventory-methods.ts:4`).
/// Params mirror the web's `{session_id, profile_id, include_disabled: true}`
/// (`inventory.ts:184`).
#[derive(Debug, Clone, Serialize)]
pub struct McpStatusListParams {
    pub session_id: String,
    pub profile_id: String,
    pub include_disabled: bool,
}

impl Default for McpStatusListParams {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            profile_id: String::new(),
            include_disabled: true,
        }
    }
}

/// The `mcp/status/list` result (`RuntimeMcp`, `inventory.ts:30`). Reuses the
/// store's projected types so a lane reads one shape everywhere.
#[derive(Debug, Clone, Deserialize)]
pub struct McpStatusListResult {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub profile_id: String,
    #[serde(default)]
    pub servers: Vec<McpServer>,
    #[serde(default)]
    pub summary: McpSummary,
}

impl McpStatusListResult {
    /// Fold the result into the store's tool domain.
    pub fn into_status(self) -> McpStatus {
        McpStatus {
            session_id: self.session_id,
            profile_id: self.profile_id,
            servers: self.servers,
            summary: self.summary,
        }
    }
}

/// `mcp/status/list` — the method (params/result above).
pub struct McpStatusList;

impl Method for McpStatusList {
    const NAME: &'static str = "mcp/status/list";
    type Params = McpStatusListParams;
    type Result = McpStatusListResult;
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
