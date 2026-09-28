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
    /// Card #13: live tool calls from `tool_start` / `tool_progress` /
    /// `tool_end` (bare notifications AND `projection/envelope` payloads).
    /// Keyed by `tool_call_id`, in first-seen order.
    calls: Vec<ToolCallRow>,
}

/// One live tool call, folded from the protocol (bare `tool/*` notifications
/// or the equivalent `projection/envelope` payloads).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallRow {
    pub tool_call_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments_preview: Option<String>,
    /// The latest progress message, when the tool reported one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// `running` until a terminal `tool_end` sets `done`/`failed`/`skipped`/`aborted`.
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
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

    // ---- live tool calls (card #13) -------------------------------------

    /// `tool_start`: begin (or restart) a call row.
    pub fn call_started(&self, tool_call_id: &str, name: &str, arguments_preview: Option<&str>) {
        let mut i = self.inner.lock().unwrap();
        i.calls.retain(|c| c.tool_call_id != tool_call_id);
        i.calls.push(ToolCallRow {
            tool_call_id: tool_call_id.to_owned(),
            name: name.to_owned(),
            arguments_preview: arguments_preview.map(str::to_owned),
            message: None,
            status: "running".to_owned(),
            output_preview: None,
            duration_ms: None,
        });
    }

    /// `tool_progress`: record the latest message on a running call.
    pub fn call_progress(&self, tool_call_id: &str, message: &str) {
        let mut i = self.inner.lock().unwrap();
        if let Some(c) = i.calls.iter_mut().find(|c| c.tool_call_id == tool_call_id) {
            c.message = Some(message.to_owned());
        }
    }

    /// `tool_end`: terminal status + optional output preview / duration.
    pub fn call_ended(
        &self,
        tool_call_id: &str,
        status: &str,
        output_preview: Option<&str>,
        duration_ms: Option<u64>,
    ) {
        let mut i = self.inner.lock().unwrap();
        let mapped = match status {
            "complete" => "done",
            "error" => "failed",
            "skipped" => "skipped",
            "aborted" => "aborted",
            other => other,
        };
        if let Some(c) = i.calls.iter_mut().find(|c| c.tool_call_id == tool_call_id) {
            c.status = mapped.to_owned();
            if let Some(p) = output_preview {
                c.output_preview = Some(p.to_owned());
            }
            c.duration_ms = duration_ms;
        } else {
            // An `end` with no `start` still yields a row (never lose it).
            i.calls.push(ToolCallRow {
                tool_call_id: tool_call_id.to_owned(),
                name: tool_call_id.to_owned(),
                arguments_preview: None,
                message: None,
                status: mapped.to_owned(),
                output_preview: output_preview.map(str::to_owned),
                duration_ms,
            });
        }
    }

    /// The live tool calls, in first-seen order.
    pub fn calls(&self) -> Vec<ToolCallRow> {
        self.inner.lock().unwrap().calls.clone()
    }

    /// Number of live tool calls.
    pub fn call_count(&self) -> usize {
        self.inner.lock().unwrap().calls.len()
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
