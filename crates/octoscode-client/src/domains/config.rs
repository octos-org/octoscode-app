//! `config/*` — server capabilities and configuration.
//!
//! Implemented in this card: `config/capabilities/list`.
use std::sync::Arc;

use serde::Deserialize;

use octos_core::ui_protocol::methods;
use octoscode_store::Store;

use crate::method::Method;
use crate::registry::Registry;

/// `config/capabilities/list` — the server's advertised capability object.
///
/// The result is NOT a bare list: it is the full `UiProtocolCapabilities`
/// object (`octos-core/src/ui_protocol.rs`), with `supported_methods` (73 on
/// the pinned serve) and `supported_notifications` (51). Verified live.
#[derive(Debug, Default, serde::Serialize)]
pub struct CapabilitiesList {}

#[derive(Debug, Deserialize)]
pub struct CapabilitiesListResult {
    #[serde(default)]
    pub capabilities: ServerCapabilities,
}

#[derive(Debug, Default, Deserialize)]
pub struct ServerCapabilities {
    #[serde(default)]
    pub capabilities_schema_version: u32,
    #[serde(default)]
    pub supported_methods: Vec<String>,
    #[serde(default)]
    pub supported_notifications: Vec<String>,
    #[serde(default)]
    pub supported_features: Vec<String>,
}

impl Method for CapabilitiesList {
    const NAME: &'static str = methods::CONFIG_CAPABILITIES_LIST;
    type Params = CapabilitiesListParams;
    type Result = CapabilitiesListResult;
}

#[derive(Debug, Default, serde::Serialize)]
pub struct CapabilitiesListParams {}

/// Owned elsewhere: the misc singleton rows (the fan-out lanes take these):
/// `background/activity`, `content/*`, `cron/*`, `diff/preview/get`,
/// `file/attached`, `launch/resolve`, `memory/*`, `permission/profile/*`,
/// `plan/updated`, `progress/updated`, `projection/envelope`,
/// `protocol/replay_lossy`, `queue/state`, `router/*`, `server/shutdown`,
/// `skill/action/job/updated`, `snapshot/*`, `system/status.get`,
/// `thread/graph/get`, `user_question/*`, `warning`, `mcp/status/list`.
pub fn register(_reg: &mut Registry, _store: Arc<Store>) {}
