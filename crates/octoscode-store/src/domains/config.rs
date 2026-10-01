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

/// Card #22 §1: the session's durable-replay recovery phase.
///
/// Mirrors the web's `SessionRecoveryPhase` (`durable-session.ts:12-20`). A
/// `protocol/replay_lossy` moves the session to [`LossyPhase::Lossy`] and raises
/// a resync request (the web's `{kind:"recover"}`, `durable-session.ts:132-134`,
/// which `active-session-runtime.ts:1239-1260` turns into a `session/hydrate`).
/// The resync request is consumed by whoever owns the transport (the module), so
/// the state — not a side effect — is what this domain exposes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecoveryState {
    /// `healthy` | `lossy`; `healthy` is the default for a session never marked.
    pub phase: LossyPhase,
    /// The human detail the web composes (`durable-session.ts:133`), e.g.
    /// `"3 durable events dropped"`.
    pub detail: String,
    /// A resync (hydrate/reopen) is owed for this session and not yet taken.
    pub resync_pending: bool,
}

/// The recovery phase a `protocol/replay_lossy` drives (card #22 §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LossyPhase {
    /// No loss observed (the web's `"healthy"`, `durable-session.ts:104`).
    #[default]
    Healthy,
    /// Durable notifications were dropped; a resync is owed.
    Lossy,
}

impl LossyPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Lossy => "lossy",
        }
    }
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
    /// Card #22 §1: the per-session durable-replay recovery phase.
    recovery: std::collections::HashMap<String, RecoveryState>,
    /// #P4g1 row 217: the `session/open` reply's
    /// `capabilities.supported_methods` (`UiProtocolCapabilities`) — the
    /// method half of the coding gate. The feature half already lands in
    /// `capabilities` (the store's advertised-feature list).
    supported_methods: Vec<String>,
    /// #P4g1 row 217: the coding gate's current missing list (empty = open),
    /// evaluated from the open reply by the client's `features` module.
    coding_gate: Vec<String>,
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

    /// #P4g1 row 217: record the open reply's supported methods.
    pub fn set_supported_methods(&self, methods: Vec<String>) {
        self.inner.lock().unwrap().supported_methods = methods;
    }

    pub fn supported_methods(&self) -> Vec<String> {
        self.inner.lock().unwrap().supported_methods.clone()
    }

    /// #P4g1 row 217: record the coding gate's missing list (empty = open).
    pub fn set_coding_gate(&self, missing: Vec<String>) {
        self.inner.lock().unwrap().coding_gate = missing;
    }

    pub fn coding_gate(&self) -> Vec<String> {
        self.inner.lock().unwrap().coding_gate.clone()
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

    // ---- card #22 §1: the lossy -> resync recovery state --------------------

    /// Card #22 §1: mark `session` lossy and raise a resync, folding the
    /// `protocol/replay_lossy` event. Mirrors the web's `observe` returning
    /// `{kind:"recover"}` and setting `phase="lossy"` (`durable-session.ts:127-135`),
    /// which `active-session-runtime.ts:1239-1260` turns into a `session/hydrate`.
    /// Also records the [`ReplayLoss`] itself (kept for the existing tests).
    pub fn observe_replay_lossy(&self, loss: ReplayLoss) {
        let detail = format!(
            "{} durable event{} dropped",
            loss.dropped_count,
            if loss.dropped_count == 1 { "" } else { "s" }
        );
        let mut inner = self.inner.lock().unwrap();
        inner.recovery.insert(
            loss.session_id.clone(),
            RecoveryState {
                phase: LossyPhase::Lossy,
                detail,
                resync_pending: true,
            },
        );
        inner.replay_loss.insert(loss.session_id.clone(), loss);
    }

    /// Card #22 §1: the recovery state for `session` (`healthy` when never seen).
    pub fn recovery(&self, session: &str) -> RecoveryState {
        self.inner
            .lock()
            .unwrap()
            .recovery
            .get(session)
            .cloned()
            .unwrap_or_default()
    }

    /// Card #22 §1: whether a resync (hydrate/reopen) is owed for `session`.
    pub fn resync_pending(&self, session: &str) -> bool {
        self.recovery(session).resync_pending
    }

    /// Card #22 §1: consume the pending resync for `session` (returns whether one
    /// was outstanding). The caller issues the `session/hydrate`; the web hydrates
    /// once per lossy observation (`active-session-runtime.ts:1256`).
    pub fn take_resync(&self, session: &str) -> bool {
        let mut inner = self.inner.lock().unwrap();
        match inner.recovery.get_mut(session) {
            Some(state) if state.resync_pending => {
                state.resync_pending = false;
                true
            }
            _ => false,
        }
    }

    /// Card #22 §1: an authoritative hydrate completed — reset the session to
    /// `healthy` (the web's `commitHydrate` → `phase="healthy"`,
    /// `durable-session.ts:104-107`).
    pub fn mark_recovered(&self, session: &str) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(state) = inner.recovery.get_mut(session) {
            state.phase = LossyPhase::Healthy;
            state.detail.clear();
            state.resync_pending = false;
        }
    }
}
