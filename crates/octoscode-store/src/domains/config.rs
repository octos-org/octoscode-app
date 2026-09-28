//! `config` state: the server's advertised capabilities and the config-level
//! singletons the fan-out owns.
//!
//! Implemented in the core card: the capability set from
//! `config/capabilities/list` (and the handshake). **Card #F3** adds the
//! `config` domain's own projections:
//! - the launch pre-session probe (`launch/resolve`),
//! - the workspace undo points (`snapshot/list` / `snapshot/restore`),
//! - the server stop ack (`server/shutdown`),
//! - the two config singletons `protocol/replay_lossy` and `warning`.
//!
//! The remaining config singletons (router, cron, memory, …) are other
//! fan-out lanes'; they add fields here.
use std::sync::Mutex;

/// The action a `launch/resolve` result tells the client to take
/// (`LaunchDecisionKind`, octos-core `ui_protocol.rs:3313`). Mirrored as a
/// plain string so the store stays serde-light; the wire form is snake_case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchDecision {
    Resume,
    Activate,
    CrossProfile,
    NoProfile,
}

impl LaunchDecision {
    /// The snake_case wire form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Resume => "resume",
            Self::Activate => "activate",
            Self::CrossProfile => "cross_profile",
            Self::NoProfile => "no_profile",
        }
    }
}

/// The stored `launch/resolve` outcome (the last pre-session probe).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchResolution {
    pub decision: LaunchDecision,
    pub resolved_profile: Option<String>,
    pub existing_profiles: Vec<String>,
}

/// One workspace undo point from `snapshot/list`
/// (`WorkspaceSnapshot`, `packages/client/src/history.ts:16`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    pub id: String,
    pub label: String,
    pub timestamp_unix: i64,
}

/// The `snapshot/list` projection for a session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotList {
    pub enabled: bool,
    pub available: bool,
    pub snapshots: Vec<WorkspaceSnapshot>,
}

/// The `warning` notification projection (`WarningEvent`,
/// octos-core `ui_protocol.rs:6233`): the last warning a session surfaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarningNotice {
    pub code: String,
    pub message: String,
}

/// The `protocol/replay_lossy` projection (`ReplayLossyEvent`,
/// `ui_protocol.rs:6324`): the last known durable cursor after a drop.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayLoss {
    pub session_id: String,
    pub dropped_count: u64,
    /// The last durable cursor the client can resume from, if the server named one.
    pub last_durable_cursor: Option<serde_json::Value>,
}

/// The config domain.
#[derive(Debug, Default)]
pub struct Config {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    capabilities: Vec<String>,
    launch: Option<LaunchResolution>,
    snapshots: SnapshotList,
    /// The `stopping` flag from `server/shutdown` (the server is going down).
    stopping: bool,
    /// The last `warning` per session.
    warnings: std::collections::HashMap<String, WarningNotice>,
    /// The last `protocol/replay_lossy` per session.
    replay_loss: std::collections::HashMap<String, ReplayLoss>,
}

impl Config {
    /// Replace the advertised capability list.
    pub fn set_capabilities(&self, capabilities: Vec<String>) {
        self.inner.lock().unwrap().capabilities = capabilities;
    }

    pub fn capabilities(&self) -> Vec<String> {
        self.inner.lock().unwrap().capabilities.clone()
    }

    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().capabilities.len()
    }

    /// Whether a capability id is advertised (case-insensitive, trimmed).
    pub fn has_capability(&self, id: &str) -> bool {
        let want = id.trim().to_ascii_lowercase();
        self.inner
            .lock()
            .unwrap()
            .capabilities
            .iter()
            .any(|c| c.trim().to_ascii_lowercase() == want)
    }

    // ---- card #F3: launch/resolve, snapshot/*, server/shutdown, warnings ----

    /// Record the last `launch/resolve` outcome.
    pub fn set_launch(&self, resolution: LaunchResolution) {
        self.inner.lock().unwrap().launch = Some(resolution);
    }

    /// The last `launch/resolve` outcome, if any.
    pub fn launch(&self) -> Option<LaunchResolution> {
        self.inner.lock().unwrap().launch.clone()
    }

    /// Replace the `snapshot/list` projection for the workspace.
    pub fn set_snapshots(&self, list: SnapshotList) {
        self.inner.lock().unwrap().snapshots = list;
    }

    /// The current `snapshot/list` projection.
    pub fn snapshots(&self) -> SnapshotList {
        self.inner.lock().unwrap().snapshots.clone()
    }

    /// Record that `server/shutdown` was acked (`{stopping:true}`).
    pub fn set_stopping(&self, stopping: bool) {
        self.inner.lock().unwrap().stopping = stopping;
    }

    /// Whether the server has acked a stop.
    pub fn stopping(&self) -> bool {
        self.inner.lock().unwrap().stopping
    }

    /// Record a `warning` for a session.
    pub fn note_warning(&self, session: &str, notice: WarningNotice) {
        self.inner
            .lock()
            .unwrap()
            .warnings
            .insert(session.to_owned(), notice);
    }

    /// The last `warning` for a session, if any.
    pub fn warning(&self, session: &str) -> Option<WarningNotice> {
        self.inner.lock().unwrap().warnings.get(session).cloned()
    }

    /// Record a `protocol/replay_lossy` for a session.
    pub fn note_replay_loss(&self, session: &str, loss: ReplayLoss) {
        self.inner
            .lock()
            .unwrap()
            .replay_loss
            .insert(session.to_owned(), loss);
    }

    /// The last `protocol/replay_lossy` for a session, if any.
    pub fn replay_loss(&self, session: &str) -> Option<ReplayLoss> {
        self.inner.lock().unwrap().replay_loss.get(session).cloned()
    }
}
