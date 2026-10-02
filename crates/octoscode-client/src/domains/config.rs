//! `config/*` — server capabilities, the launch pre-session probe, the
//! workspace snapshots, and the config-level singletons this lane owns.
//!
//! Implemented in the core card: `config/capabilities/list`.
//!
//! **Card #F3** adds: `launch/resolve`, `snapshot/list`, `snapshot/restore`,
//! `server/shutdown` (the last three are AppUI extensions), and the
//! notifications `protocol/replay_lossy` and `warning`.
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octoscode_store::domains::config::{
    LaunchDecision, LaunchResolution, ReplayLoss, SnapshotList, WarningNotice, WorkspaceSnapshot,
};
use octoscode_store::Store;

use crate::method::Method;
use crate::registry::{NotificationHandler, Registry};

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

// ---------------------------------------------------------------------------
// Card #F3 — the config-level requests.
// ---------------------------------------------------------------------------

/// `launch/resolve` — the pre-session launch probe (octos-core
/// `ui_protocol.rs:1272` const, `LaunchResolveParams` `:3293`). Web call site
/// `packages/client/src/client.ts:830`; gated on
/// `session.workspace_cwd.v1` at `apps/web/src/features/session/use-octos-session.ts:3028`.
pub struct LaunchResolve;

impl Method for LaunchResolve {
    const NAME: &'static str = methods::LAUNCH_RESOLVE;
    type Params = octos_core::ui_protocol::LaunchResolveParams;
    type Result = octos_core::ui_protocol::LaunchResolveResult;
}

/// Fold a `LaunchResolveResult` into the store's launch projection.
///
/// A free function, not a `From` impl: both `From` and the target type live
/// outside this crate, so an impl here would violate the orphan rule.
pub fn launch_resolution_from(
    r: octos_core::ui_protocol::LaunchResolveResult,
) -> LaunchResolution {
    use octos_core::ui_protocol::LaunchDecisionKind as K;
    let decision = match r.decision {
        K::Resume => LaunchDecision::Resume,
        K::Activate => LaunchDecision::Activate,
        K::CrossProfile => LaunchDecision::CrossProfile,
        K::NoProfile => LaunchDecision::NoProfile,
    };
    LaunchResolution {
        decision,
        resolved_profile: r.resolved_profile,
        existing_profiles: r.existing_profiles,
    }
}

/// `snapshot/list` — the session workspace's undo points (AppUI extension).
/// Server: `octos-cli/src/api/ui_protocol_transport.rs:308` (const),
/// `raw_snapshot_list` `:14366`. Web call site
/// `packages/client/src/history.ts:193` (`{session_id}`).
pub struct SnapshotListMethod;

/// Params the web sends: `{ session_id }` (`history.ts:194-196`).
#[derive(Debug, Serialize)]
pub struct SnapshotListParams {
    pub session_id: String,
}

/// Result of `snapshot/list`, mirroring the web's `SnapshotList`
/// (`packages/client/src/history.ts:22`) and the server payload
/// (`ui_protocol_transport.rs:14380`).
#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotListResult {
    pub session_id: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub snapshots: Vec<SnapshotRow>,
}

/// One undo point (`WorkspaceSnapshot`, `history.ts:16`).
#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotRow {
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub timestamp_unix: i64,
}

impl Method for SnapshotListMethod {
    const NAME: &'static str = "snapshot/list";
    type Params = SnapshotListParams;
    type Result = SnapshotListResult;
}

impl SnapshotListResult {
    /// Fold into the store's snapshot projection.
    pub fn into_store(self) -> SnapshotList {
        SnapshotList {
            enabled: self.enabled,
            available: self.available,
            snapshots: self
                .snapshots
                .into_iter()
                .map(|s| WorkspaceSnapshot {
                    id: s.id,
                    label: s.label,
                    timestamp_unix: s.timestamp_unix,
                })
                .collect(),
        }
    }
}

/// `snapshot/restore` — roll the session workspace back to a snapshot (AppUI
/// extension). Server: `ui_protocol_transport.rs:310` (const),
/// `raw_snapshot_restore` `:14390`. Web call site `history.ts:203`
/// (`{session_id, snapshot_id}`).
pub struct SnapshotRestoreMethod;

/// Params the web sends: `{ session_id, snapshot_id }` (`history.ts:204-207`).
#[derive(Debug, Serialize)]
pub struct SnapshotRestoreParams {
    pub session_id: String,
    pub snapshot_id: String,
}

/// Result of `snapshot/restore`, mirroring the web's `SnapshotRestore`
/// (`history.ts:28`).
#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotRestoreResult {
    pub session_id: String,
    pub restored: String,
    #[serde(default)]
    pub snapshots: Vec<SnapshotRow>,
}

impl Method for SnapshotRestoreMethod {
    const NAME: &'static str = "snapshot/restore";
    type Params = SnapshotRestoreParams;
    type Result = SnapshotRestoreResult;
}

impl SnapshotRestoreResult {
    /// Fold into the store's snapshot projection (the restore returns the new list).
    pub fn into_store(self) -> SnapshotList {
        SnapshotList {
            enabled: true,
            available: true,
            snapshots: self
                .snapshots
                .into_iter()
                .map(|s| WorkspaceSnapshot {
                    id: s.id,
                    label: s.label,
                    timestamp_unix: s.timestamp_unix,
                })
                .collect(),
        }
    }
}

/// `server/shutdown` — stop this `octos serve` exactly as Ctrl+C would (AppUI
/// extension). Server: `ui_protocol_transport.rs:266` (const),
/// `handle_server_shutdown` `:9719`; advertised only where runnable (local
/// `--solo`). Web call site `packages/client/src/server-methods.ts:7`.
pub struct ServerShutdown;

/// Params the web sends: `{}` (`server-methods.ts` doc: `SHUTDOWN`).
#[derive(Debug, Default, Serialize)]
pub struct ServerShutdownParams {}

/// Result of `server/shutdown`: `{ stopping: true }` (server
/// `ui_protocol_transport.rs:9741`).
#[derive(Debug, Clone, Deserialize)]
pub struct ServerShutdownResult {
    #[serde(default)]
    pub stopping: bool,
}

impl Method for ServerShutdown {
    const NAME: &'static str = "server/shutdown";
    type Params = ServerShutdownParams;
    type Result = ServerShutdownResult;
}

// ---------------------------------------------------------------------------
// Notification handlers
// ---------------------------------------------------------------------------

/// `protocol/replay_lossy` — durable notifications were dropped under
/// backpressure (`ReplayLossyEvent`, octos-core `ui_protocol.rs:6324`).
///
/// Card #22 §1: this is more than a notice. The web's `DurableSessionProjection`
/// marks the session `phase="lossy"` with a human detail and returns
/// `{kind:"recover"}` (`src-web/apps/web/src/features/session/durable-session.ts:127-135`),
/// which `active-session-runtime.ts:1239-1260` turns into a resync: it clears no
/// buffer for a lossy event and calls `#hydrate(authority, "recovery")`. So the
/// handler must **mark the session lossy and raise a resync**, not just record a
/// count. We raise it as store state ([`octoscode_store::domains::config::RecoveryState::resync_pending`])
/// because the transport (and thus the `session/hydrate` call) lives above this
/// crate — see [`crate::domains::session::SessionHydrate`].
pub struct ReplayLossyHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for ReplayLossyHandler {
    const METHOD: &'static str = methods::REPLAY_LOSSY;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::ReplayLossy(event) = notification {
            self.store.note_seen(Self::METHOD);
            self.store.domains.config.observe_replay_lossy(ReplayLoss {
                session_id: event.session_id.0.clone(),
                dropped_count: event.dropped_count,
                last_durable_cursor: event
                    .last_durable_cursor
                    .as_ref()
                    .and_then(|c| serde_json::to_value(c).ok()),
            });
        }
    }
}

/// `warning` — a non-fatal warning for a session (`WarningEvent`,
/// octos-core `ui_protocol.rs:6233`). Stored per session, never fatal.
pub struct WarningHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for WarningHandler {
    const METHOD: &'static str = methods::WARNING;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::Warning(event) = notification {
            self.store.note_seen(Self::METHOD);
            self.store.domains.config.note_warning(
                &event.session_id.0,
                WarningNotice {
                    code: event.code.clone(),
                    message: event.message.clone(),
                },
            );
            // A4 — the web writes every warning into the transcript as a
            // system notice (`timeline/model.ts:432-439`: title = code or
            // "Warning", body = message or "The server reported a
            // warning.").
            // A6 — under the web's DETERMINISTIC ordinal id
            // (`nextNoticeId(entries, "warning")`, `entry-model.ts:101-110`):
            // two same-millisecond warnings keep two rows, and the id is a
            // function of the transcript, never of the wall clock
            // (`model.test.ts:1610-1637`).
            let tl = &self.store.domains.session.timeline;
            let notice_id = tl.next_notice_id(&event.session_id.0, "warning");
            tl.upsert_notice(
                &event.session_id.0,
                None,
                &notice_id,
                format!("{}: {}", event.code, event.message),
                serde_json::json!({"code": event.code, "message": event.message}),
            );
        }
    }
}

/// The misc singleton rows other lanes own (kept here as the domain index):
/// `background/activity`, `content/*`, `cron/*`, `diff/preview/get`,
/// `file/attached`, `memory/*`, `permission/profile/*`, `plan/updated`,
/// `progress/updated`, `projection/envelope`, `queue/state`, `router/*`,
/// `skill/action/job/updated`, `system/status.get`, `thread/graph/get`,
/// `user_question/*`, `mcp/status/list`.
pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(ReplayLossyHandler { store: store.clone() });
    reg.register(WarningHandler { store });
}
