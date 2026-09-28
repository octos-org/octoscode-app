//! `autonomy` — the M15 surface: sub-agents, loops, monitors, session goals.
//!
//! Card **F1** owns this file. Its 29 methods are 21 requests + 8
//! notifications (`docs/protocol-matrix.csv` rows with `contract_src`
//! `octos-core/src/ui_protocol.rs:1068-1103` and `:1296-1308`).
//!
//! ## Where the types come from
//!
//! octos-core at the pinned `a6ea8505` types the **records** and the
//! **notification events**, but has **no request `Params`/`Result` structs**
//! for this surface — the server reads raw params
//! (`crates/octos-cli/src/api/ui_protocol_transport.rs:9388-9625`) and emits
//! results with `json!` (`crates/octos-cli/src/autonomy/agent_orchestrator.rs`
//! `autonomy_{goal,loop,monitor,agent}_json`). So:
//! - **Records and notifications reuse octos-core types** (`UiGoalRecord`,
//!   `UiLoopRecord`, `UiMonitorRecord`, `UiAgentRecord`, `UiAgentArtifact`,
//!   `UiLoopFire`, and the `*Event` structs) — those are the authoritative
//!   shapes.
//! - **Request envelopes are typed structs here**, each documented with the
//!   server's raw-param line and the web call site, because octos-core has no
//!   type to take. They mirror exactly what the web client sends
//!   (`packages/client/src/autonomy.ts`).
//!
//! Parity source of truth for params/defaults: the web's
//! `build*Params` functions and the `*Params` interfaces in
//! `packages/client/src/autonomy.ts` (cited per method).
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octos_core::ui_protocol::{
    UiAgentArtifact, UiAgentRecord, UiGoalRecord, UiLoopFire, UiLoopRecord, UiMonitorRecord,
};
use octoscode_store::domains::autonomy::{
    AgentRecord, GoalRecord, GoalState, LoopRecord, MonitorRecord,
};
use octoscode_store::Store;

use crate::method::Method;
use crate::registry::{NotificationHandler, Registry};

// ===========================================================================
// Params / Result envelopes (octos-core has no type for these — see module doc)
// ===========================================================================

/// Params for the list reads (`agent/list`, `loop/list`, `monitor/list`):
/// both optional. `crates/octos-cli/src/api/ui_protocol_transport.rs:9388`
/// (`RawAutonomyListParams`); web `LoopListParams`/`AutonomyRPC` list calls
/// (`packages/client/src/autonomy.ts:628`, `:1010`).
#[derive(Debug, Default, Serialize)]
pub struct AutonomyListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
}

/// Params for the session-scoped reads/writes (`session/goal/get|clear`):
/// `{session_id, profile_id?}`. `…ui_protocol_transport.rs:9396`
/// (`RawAutonomySessionParams`).
#[derive(Debug, Serialize)]
pub struct GoalSessionParams {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
}

/// `session/goal/get` result: `{session_id, profile_id, goal|null}`; web
/// `parseSessionGoalGetResult` (`packages/client/src/autonomy.ts:318`).
#[derive(Debug, Deserialize)]
pub struct GoalGetResult {
    pub session_id: String,
    pub profile_id: String,
    #[serde(default)]
    pub goal: Option<UiGoalRecord>,
}

/// `session/goal/set` params. `token_budget` is forwarded ONLY when the
/// caller set one — never defaulted (web `SessionGoalSetParams` doc,
/// `packages/client/src/autonomy.ts:283-297`); server `RawGoalSetParams`
/// (`…ui_protocol_transport.rs:9524`).
#[derive(Debug, Serialize)]
pub struct GoalSetParams {
    pub session_id: String,
    pub objective: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition_actor: Option<String>,
}

/// `session/goal/set` result (`parseSessionGoalSetResult`, web `:403`).
#[derive(Debug, Deserialize)]
pub struct GoalSetResult {
    pub session_id: String,
    pub profile_id: String,
    pub goal: UiGoalRecord,
    pub generation: u64,
    pub transition_actor: String,
}

/// `session/goal/clear` result (`parseSessionGoalClearResult`, web `:426`).
/// `goal` is always `null`; kept typed as `Option` so the shape round-trips.
#[derive(Debug, Deserialize)]
pub struct GoalClearResult {
    pub session_id: String,
    pub profile_id: String,
    pub cleared: bool,
    #[serde(default)]
    pub goal: Option<UiGoalRecord>,
    pub generation: u64,
    pub transition_actor: String,
}

/// Agent-scoped params (`agent/status/read`, `agent/artifact/list`).
/// `…ui_protocol_transport.rs:9403` (`RawAgentParams`).
#[derive(Debug, Serialize)]
pub struct AgentParams {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
}

/// `agent/status/read` result (`parseAgentStatusReadResult`, web `:1451`).
#[derive(Debug, Deserialize)]
pub struct AgentStatusReadResult {
    pub session_id: String,
    pub agent: UiAgentRecord,
}

/// `agent/output/read` params; web `AgentOutputReadParams`
/// (`packages/client/src/autonomy.ts:1410`), server `RawAgentOutputParams`
/// (`…ui_protocol_transport.rs:9412`).
#[derive(Debug, Serialize)]
pub struct AgentOutputReadParams {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub cursor: Option<AgentOutputCursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutputCursor {
    pub offset: u64,
}

/// `agent/output/read` result (`parseAgentOutputReadResult`, web `:1466`).
#[derive(Debug, Deserialize)]
pub struct AgentOutputReadResult {
    pub agent_id: String,
    pub session_id: String,
    pub source: String,
    pub text: String,
    #[serde(default)]
    pub cursor: Option<AgentOutputCursor>,
    #[serde(default)]
    pub next_cursor: Option<AgentOutputCursor>,
    pub has_more: bool,
    pub complete: bool,
}

/// `agent/artifact/list` result (`parseAgentArtifactListResult`, web `:1342`).
#[derive(Debug, Deserialize)]
pub struct AgentArtifactListResult {
    pub session_id: String,
    pub agent_id: String,
    pub artifacts: Vec<UiAgentArtifact>,
}

/// `agent/artifact/read` params. The web's `AgentArtifactSelector` forbids
/// sending both selectors (`packages/client/src/autonomy.ts:1319`); server
/// `RawAgentArtifactReadParams` (`…ui_protocol_transport.rs:9425`).
#[derive(Debug, Serialize)]
pub struct AgentArtifactReadParams {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
}

/// `agent/artifact/read` result (`parseAgentArtifactReadResult`, web `:1361`).
#[derive(Debug, Deserialize)]
pub struct AgentArtifactReadResult {
    pub session_id: String,
    pub agent_id: String,
    pub artifact: UiAgentArtifact,
    /// Core redacts credentials here; `null` is a valid value.
    #[serde(default)]
    pub content: Option<String>,
}

/// `agent/interrupt` / `agent/close` result
/// (`parseAgentControlResult`, web `:1381`).
#[derive(Debug, Deserialize)]
pub struct AgentControlResult {
    pub session_id: String,
    pub agent_id: String,
    pub status: String,
    pub ok: bool,
    pub interrupted: bool,
    pub closed: bool,
    pub already_terminal: bool,
}

/// `loop/create` params; web `LoopCreateParams` + `buildLoopCreateParams`
/// (`packages/client/src/autonomy.ts:597`, `:672`), server `RawLoopCreateParams`
/// (`…ui_protocol_transport.rs:9548`). `command` is the `/loop <text>`
/// shorthand (a wire alias of `input` on the server).
#[derive(Debug, Serialize)]
pub struct LoopCreateParams {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

/// `loop/create` result (`parseLoopCreateResult`, web `:775`).
#[derive(Debug, Deserialize)]
pub struct LoopCreateResult {
    pub session_id: String,
    pub profile_id: String,
    pub loop_id: String,
    #[serde(rename = "loop")]
    pub loop_state: UiLoopRecord,
    pub ok: bool,
    pub status: String,
    pub created: bool,
    #[serde(default)]
    pub fire: Option<UiLoopFire>,
}

/// `loop/list` result (`parseLoopListResult`, web `:801`). `session_id` is
/// `null` for an unscoped (profile-wide) listing.
#[derive(Debug, Deserialize)]
pub struct LoopListResult {
    #[serde(default)]
    pub session_id: Option<String>,
    pub profile_id: String,
    pub loops: Vec<UiLoopRecord>,
}

/// `loop/{pause,resume,delete,fire_now}` params: `{loop_id, session_id?,
/// profile_id?}`. Web `LoopControlParams` (`packages/client/src/autonomy.ts:639`);
/// server `RawLoopIdParams` (`…ui_protocol_transport.rs:9563`).
#[derive(Debug, Serialize)]
pub struct LoopControlParams {
    pub loop_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
}

impl LoopControlParams {
    pub fn new(loop_id: impl Into<String>) -> Self {
        Self { loop_id: loop_id.into(), session_id: None, profile_id: None }
    }
}

/// `loop/pause` / `loop/resume` result (`parseLoopPauseResumeResult`, web `:850`).
#[derive(Debug, Deserialize)]
pub struct LoopPauseResumeResult {
    pub session_id: String,
    pub loop_id: String,
    #[serde(rename = "loop")]
    pub loop_state: UiLoopRecord,
    pub ok: bool,
    pub status: String,
}

/// `loop/delete` result (`parseLoopDeleteResult`, web `:824`).
#[derive(Debug, Deserialize)]
pub struct LoopDeleteResult {
    pub session_id: String,
    pub loop_id: String,
    #[serde(rename = "loop")]
    pub loop_state: UiLoopRecord,
    pub ok: bool,
    pub status: String,
    pub deleted: bool,
    #[serde(default)]
    pub reaped_cron_job_ids: Vec<String>,
}

/// `loop/fire_now` result (`parseLoopFireNowResult`, web `:873`).
#[derive(Debug, Deserialize)]
pub struct LoopFireNowResult {
    pub session_id: String,
    pub profile_id: String,
    pub loop_id: String,
    #[serde(rename = "loop")]
    pub loop_state: UiLoopRecord,
    pub ok: bool,
    pub status: String,
    #[serde(default)]
    pub fire: Option<UiLoopFire>,
}

/// `monitor/create` params; web `MonitorCreateParams` + `buildMonitorCreateParams`
/// (`packages/client/src/autonomy.ts:986`, `:1026`), server `RawMonitorCreateParams`
/// (`…ui_protocol_transport.rs:9574`).
#[derive(Debug, Serialize)]
pub struct MonitorCreateParams {
    pub session_id: String,
    pub name: String,
    pub argv: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_regex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_events_per_hour: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_id: Option<String>,
}

/// `monitor/create` result (`parseMonitorCreateResult`, web `:1112`).
#[derive(Debug, Deserialize)]
pub struct MonitorCreateResult {
    pub session_id: String,
    pub profile_id: String,
    pub monitor_id: String,
    pub monitor: UiMonitorRecord,
    pub ok: bool,
    pub status: String,
    pub created: bool,
}

/// `monitor/list` result (`parseMonitorListResult`, web `:1139`).
#[derive(Debug, Deserialize)]
pub struct MonitorListResult {
    #[serde(default)]
    pub session_id: Option<String>,
    pub profile_id: String,
    pub monitors: Vec<UiMonitorRecord>,
}

/// `monitor/{pause,resume,delete}` params; web `MonitorControlParams`
/// (`packages/client/src/autonomy.ts:1016`), server `RawMonitorIdParams`
/// (`…ui_protocol_transport.rs:9599`).
#[derive(Debug, Serialize)]
pub struct MonitorControlParams {
    pub monitor_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
}

impl MonitorControlParams {
    pub fn new(monitor_id: impl Into<String>) -> Self {
        Self { monitor_id: monitor_id.into(), session_id: None, profile_id: None }
    }
}

/// `monitor/{pause,resume,delete}` result (`parseMonitorControlResult`, web `:1164`).
#[derive(Debug, Deserialize)]
pub struct MonitorControlResult {
    pub session_id: String,
    pub profile_id: String,
    pub monitor_id: String,
    pub monitor: UiMonitorRecord,
    pub ok: bool,
    pub status: String,
    pub deleted: bool,
}

// ===========================================================================
// Method impls — 21 requests
// ===========================================================================

macro_rules! method {
    ($ty:ident, $name:expr, $params:ty, $result:ty) => {
        impl Method for $ty {
            const NAME: &'static str = $name;
            type Params = $params;
            type Result = $result;
        }
    };
}

/// The list reads (empty params are valid).
#[derive(Debug)]
pub struct AgentList;
method!(AgentList, methods::AGENT_LIST, AutonomyListParams, AgentListResult);

#[derive(Debug, Deserialize)]
pub struct AgentListResult {
    #[serde(default)]
    pub session_id: Option<String>,
    pub profile_id: String,
    pub agents: Vec<UiAgentRecord>,
}

#[derive(Debug)]
pub struct AgentStatusRead;
method!(AgentStatusRead, methods::AGENT_STATUS_READ, AgentParams, AgentStatusReadResult);

#[derive(Debug)]
pub struct AgentOutputRead;
method!(AgentOutputRead, methods::AGENT_OUTPUT_READ, AgentOutputReadParams, AgentOutputReadResult);

#[derive(Debug)]
pub struct AgentArtifactList;
method!(AgentArtifactList, methods::AGENT_ARTIFACT_LIST, AgentParams, AgentArtifactListResult);

#[derive(Debug)]
pub struct AgentArtifactRead;
method!(
    AgentArtifactRead,
    methods::AGENT_ARTIFACT_READ,
    AgentArtifactReadParams,
    AgentArtifactReadResult
);

#[derive(Debug)]
pub struct AgentInterrupt;
method!(AgentInterrupt, methods::AGENT_INTERRUPT, AgentParams, AgentControlResult);

#[derive(Debug)]
pub struct AgentClose;
method!(AgentClose, methods::AGENT_CLOSE, AgentParams, AgentControlResult);

#[derive(Debug)]
pub struct LoopCreate;
method!(LoopCreate, methods::LOOP_CREATE, LoopCreateParams, LoopCreateResult);

#[derive(Debug)]
pub struct LoopList;
method!(LoopList, methods::LOOP_LIST, AutonomyListParams, LoopListResult);

#[derive(Debug)]
pub struct LoopPause;
method!(LoopPause, methods::LOOP_PAUSE, LoopControlParams, LoopPauseResumeResult);

#[derive(Debug)]
pub struct LoopResume;
method!(LoopResume, methods::LOOP_RESUME, LoopControlParams, LoopPauseResumeResult);

#[derive(Debug)]
pub struct LoopDelete;
method!(LoopDelete, methods::LOOP_DELETE, LoopControlParams, LoopDeleteResult);

#[derive(Debug)]
pub struct LoopFireNow;
method!(LoopFireNow, methods::LOOP_FIRE_NOW, LoopControlParams, LoopFireNowResult);

#[derive(Debug)]
pub struct MonitorCreate;
method!(MonitorCreate, methods::MONITOR_CREATE, MonitorCreateParams, MonitorCreateResult);

#[derive(Debug)]
pub struct MonitorList;
method!(MonitorList, methods::MONITOR_LIST, AutonomyListParams, MonitorListResult);

#[derive(Debug)]
pub struct MonitorPause;
method!(MonitorPause, methods::MONITOR_PAUSE, MonitorControlParams, MonitorControlResult);

#[derive(Debug)]
pub struct MonitorResume;
method!(MonitorResume, methods::MONITOR_RESUME, MonitorControlParams, MonitorControlResult);

#[derive(Debug)]
pub struct MonitorDelete;
method!(MonitorDelete, methods::MONITOR_DELETE, MonitorControlParams, MonitorControlResult);

#[derive(Debug)]
pub struct GoalGet;
method!(GoalGet, methods::SESSION_GOAL_GET, GoalSessionParams, GoalGetResult);

#[derive(Debug)]
pub struct GoalSet;
method!(GoalSet, methods::SESSION_GOAL_SET, GoalSetParams, GoalSetResult);

#[derive(Debug)]
pub struct GoalClear;
method!(GoalClear, methods::SESSION_GOAL_CLEAR, GoalSessionParams, GoalClearResult);

/// Every request this domain implements, by wire name — the fan-out audit.
pub const REQUEST_METHODS: &[&str] = &[
    methods::AGENT_LIST,
    methods::AGENT_STATUS_READ,
    methods::AGENT_OUTPUT_READ,
    methods::AGENT_ARTIFACT_LIST,
    methods::AGENT_ARTIFACT_READ,
    methods::AGENT_INTERRUPT,
    methods::AGENT_CLOSE,
    methods::LOOP_CREATE,
    methods::LOOP_LIST,
    methods::LOOP_PAUSE,
    methods::LOOP_RESUME,
    methods::LOOP_DELETE,
    methods::LOOP_FIRE_NOW,
    methods::MONITOR_CREATE,
    methods::MONITOR_LIST,
    methods::MONITOR_PAUSE,
    methods::MONITOR_RESUME,
    methods::MONITOR_DELETE,
    methods::SESSION_GOAL_GET,
    methods::SESSION_GOAL_SET,
    methods::SESSION_GOAL_CLEAR,
];

// ===========================================================================
// octos-core record -> store record
// ===========================================================================

fn agent_from_ui(a: &UiAgentRecord) -> AgentRecord {
    AgentRecord {
        agent_id: a.agent_id.clone(),
        session_id: a.session_id.0.clone(),
        profile_id: a.profile_id.clone(),
        path: a.path.clone(),
        role: a.role.clone(),
        nickname: a.nickname.clone(),
        backend_kind: a.backend_kind.clone(),
        status: a.status.clone(),
        title: a.title.clone(),
        parent_agent_id: a.parent_agent_id.clone(),
        task_id: a.task_id.clone(),
        artifact_count: a.artifact_count,
        output_tail: a.output_tail.clone(),
        updated_at_ms: a.updated_at_ms,
    }
}

fn loop_from_ui(l: &UiLoopRecord) -> LoopRecord {
    LoopRecord {
        loop_id: l.loop_id.clone(),
        session_id: l.session_id.0.clone(),
        profile_id: l.profile_id.clone(),
        prompt: l.prompt.clone(),
        mode: l.mode.clone(),
        status: l.status.clone(),
        interval_seconds: l.interval_seconds,
        next_run_at_ms: l.next_run_at_ms,
        expires_at_ms: l.expires_at_ms,
        updated_at_ms: l.updated_at_ms,
        fires: 0,
    }
}

fn monitor_from_ui(m: &UiMonitorRecord) -> MonitorRecord {
    MonitorRecord {
        monitor_id: m.monitor_id.clone(),
        session_id: m.session_id.0.clone(),
        profile_id: m.profile_id.clone(),
        name: m.name.clone(),
        mode: m.mode.clone(),
        status: m.status.clone(),
        pause_reason: m.pause_reason.clone(),
        fires_used: m.fires_used,
        last_fired_at_ms: m.last_fired_at_ms,
        expires_at_ms: m.expires_at_ms,
        updated_at_ms: m.updated_at_ms,
    }
}

fn goal_from_ui(g: &UiGoalRecord) -> GoalRecord {
    GoalRecord {
        goal_id: g.goal_id.clone(),
        objective: g.objective.clone(),
        status: g.status.clone(),
        token_budget: g.token_budget,
        tokens_used: g.tokens_used,
        created_at_ms: g.created_at_ms,
        updated_at_ms: g.updated_at_ms,
    }
}

/// Fold a `session/goal/get` result into the store (an RPC reply, so it is
/// written unconditionally — the generation guard is for notifications only).
pub fn apply_goal_get(store: &Store, session_id: &str, goal: Option<UiGoalRecord>) {
    let state = GoalState {
        goal: goal.as_ref().map(goal_from_ui),
        transition_actor: None,
        generation: store.domains.autonomy.goal_generation(session_id),
    };
    store.domains.autonomy.set_goal(session_id, state);
}

// ===========================================================================
// Notifications — 8 handlers
// ===========================================================================

/// `agent/updated` (`octos-core/src/ui_protocol.rs:1296`).
pub struct AgentUpdatedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for AgentUpdatedHandler {
    const METHOD: &'static str = methods::AGENT_UPDATED;
    fn handle(&self, n: &UiNotification) {
        if let UiNotification::AgentUpdated(e) = n {
            self.store.note_seen(Self::METHOD);
            self.store.domains.autonomy.upsert_agent(agent_from_ui(&e.agent));
        }
    }
}

/// `session/goal/updated` — honours the #1959 generation guard.
pub struct GoalUpdatedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for GoalUpdatedHandler {
    const METHOD: &'static str = methods::SESSION_GOAL_UPDATED;
    fn handle(&self, n: &UiNotification) {
        if let UiNotification::SessionGoalUpdated(e) = n {
            self.store.note_seen(Self::METHOD);
            self.store.domains.autonomy.apply_goal_update(
                &e.session_id.0,
                goal_from_ui(&e.goal),
                Some(e.transition_actor.clone()),
                e.generation,
            );
        }
    }
}

/// `session/goal/cleared` — honours the #1959 generation guard.
pub struct GoalClearedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for GoalClearedHandler {
    const METHOD: &'static str = methods::SESSION_GOAL_CLEARED;
    fn handle(&self, n: &UiNotification) {
        if let UiNotification::SessionGoalCleared(e) = n {
            self.store.note_seen(Self::METHOD);
            self.store.domains.autonomy.apply_goal_clear(
                &e.session_id.0,
                Some(e.transition_actor.clone()),
                e.generation,
            );
        }
    }
}

/// `loop/updated`.
pub struct LoopUpdatedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for LoopUpdatedHandler {
    const METHOD: &'static str = methods::LOOP_UPDATED;
    fn handle(&self, n: &UiNotification) {
        if let UiNotification::LoopUpdated(e) = n {
            self.store.note_seen(Self::METHOD);
            // `deleted: true` removes the row (web `LoopUpdatedEvent.deleted`).
            if e.deleted == Some(true) {
                self.store.domains.autonomy.remove_loop(&e.loop_state.loop_id);
            } else {
                self.store.domains.autonomy.upsert_loop(loop_from_ui(&e.loop_state));
            }
        }
    }
}

/// `loop/fired` — bumps the loop's fire counter.
pub struct LoopFiredHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for LoopFiredHandler {
    const METHOD: &'static str = methods::LOOP_FIRED;
    fn handle(&self, n: &UiNotification) {
        if let UiNotification::LoopFired(e) = n {
            self.store.note_seen(Self::METHOD);
            if let Some(record) = &e.loop_state {
                self.store.domains.autonomy.upsert_loop(loop_from_ui(record));
            }
            self.store.domains.autonomy.note_loop_fired(&e.loop_id);
        }
    }
}

/// `monitor/updated`.
pub struct MonitorUpdatedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for MonitorUpdatedHandler {
    const METHOD: &'static str = methods::MONITOR_UPDATED;
    fn handle(&self, n: &UiNotification) {
        if let UiNotification::MonitorUpdated(e) = n {
            self.store.note_seen(Self::METHOD);
            if e.deleted == Some(true) {
                self.store.domains.autonomy.remove_monitor(&e.monitor_state.monitor_id);
            } else {
                self.store
                    .domains
                    .autonomy
                    .upsert_monitor(monitor_from_ui(&e.monitor_state));
            }
        }
    }
}

/// `monitor/fired` — bumps the monitor's fire counter.
pub struct MonitorFiredHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for MonitorFiredHandler {
    const METHOD: &'static str = methods::MONITOR_FIRED;
    fn handle(&self, n: &UiNotification) {
        if let UiNotification::MonitorFired(e) = n {
            self.store.note_seen(Self::METHOD);
            self.store
                .domains
                .autonomy
                .note_monitor_fired(&e.monitor_id, e.fired_at_ms);
        }
    }
}

/// `monitor/expired`.
pub struct MonitorExpiredHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for MonitorExpiredHandler {
    const METHOD: &'static str = methods::MONITOR_EXPIRED;
    fn handle(&self, n: &UiNotification) {
        if let UiNotification::MonitorExpired(e) = n {
            self.store.note_seen(Self::METHOD);
            if let Some(record) = &e.monitor_state {
                self.store
                    .domains
                    .autonomy
                    .upsert_monitor(monitor_from_ui(record));
            }
            self.store
                .domains
                .autonomy
                .mark_monitor_expired(&e.monitor_id, e.reason.clone());
        }
    }
}

/// Every notification this domain handles, by wire name — the fan-out audit.
pub const NOTIFICATION_METHODS: &[&str] = &[
    methods::AGENT_UPDATED,
    methods::SESSION_GOAL_UPDATED,
    methods::SESSION_GOAL_CLEARED,
    methods::LOOP_UPDATED,
    methods::LOOP_FIRED,
    methods::MONITOR_UPDATED,
    methods::MONITOR_FIRED,
    methods::MONITOR_EXPIRED,
];

/// Register this domain's notification handlers.
///
/// Requests: the 21 [`Method`] impls above. `agent/output/delta`,
/// `agent/artifact/updated` and `session/goal/operator_transition` are NOT in
/// card F1's list; they stay on the registry's tolerated-unknown arm (logged
/// by name, never fatal).
pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(AgentUpdatedHandler { store: store.clone() });
    reg.register(GoalUpdatedHandler { store: store.clone() });
    reg.register(GoalClearedHandler { store: store.clone() });
    reg.register(LoopUpdatedHandler { store: store.clone() });
    reg.register(LoopFiredHandler { store: store.clone() });
    reg.register(MonitorUpdatedHandler { store: store.clone() });
    reg.register(MonitorFiredHandler { store: store.clone() });
    reg.register(MonitorExpiredHandler { store });
}
