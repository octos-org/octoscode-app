//! `tool` state: the runtime tool inventory from `tool/status/list`, and the
//! MCP server status from `mcp/status/list`.
//!
//! [F4] adds the MCP status projection (`RuntimeMcp`,
//! `src-web/packages/client/src/inventory.ts:30`; the server builder is
//! `mcp_status_list_payload`, octos-cli `api/coding_tool_contract.rs:641`).
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// One tool row (`RuntimeTool` in the web client's `packages/client/src/inventory.ts:15`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// One MCP server row (`RuntimeMcpServer`, `inventory.ts:21`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServer {
    pub id: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub transport: Option<String>,
    pub status: String,
    #[serde(default)]
    pub tool_count: u32,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// The MCP status summary counts (`RuntimeMcp.summary`, `inventory.ts:34`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpSummary {
    #[serde(default)]
    pub connected: u32,
    #[serde(default)]
    pub connecting: u32,
    #[serde(default)]
    pub failed: u32,
    #[serde(default)]
    pub disabled: u32,
}

/// The last `mcp/status/list` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpStatus {
    pub session_id: String,
    pub profile_id: String,
    pub servers: Vec<McpServer>,
    pub summary: McpSummary,
}

/// The tool domain: the last inventory seen, plus the MCP status.
#[derive(Debug, Default)]
pub struct Tools {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    tools: Vec<RuntimeTool>,
    mcp: Option<McpStatus>,
}

impl Tools {
    pub fn set(&self, tools: Vec<RuntimeTool>) {
        self.inner.lock().unwrap().tools = tools;
    }

    pub fn list(&self) -> Vec<RuntimeTool> {
        self.inner.lock().unwrap().tools.clone()
    }

    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().tools.len()
    }

    /// Store the last MCP status (`mcp/status/list`).
    pub fn set_mcp(&self, status: McpStatus) {
        self.inner.lock().unwrap().mcp = Some(status);
    }

    pub fn mcp(&self) -> Option<McpStatus> {
        self.inner.lock().unwrap().mcp.clone()
    }

    /// Number of MCP servers in the last status (0 when none seen).
    pub fn mcp_server_count(&self) -> usize {
        self.inner
            .lock()
            .unwrap()
            .mcp
            .as_ref()
            .map(|m| m.servers.len())
            .unwrap_or(0)
    }
}
