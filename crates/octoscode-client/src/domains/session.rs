//! `session/*` — the session list, open/close, and session-scoped state.
//!
//! Implemented in this card: `session/list`, `session/open` (handler only —
//! `session/open` stays on the transport's typed `OutboundCommand::OpenSession`
//! because it carries the replay cursor bracket, so it is deliberately NOT a
//! [`Method`] here).
//!
//! **Card #F3** adds the session-scoped requests and notifications:
//! `session/btw`, `session/delete`, `session/files.list`, `session/fork`,
//! `session/rollback`, `session/status/read`, `session/compact`,
//! `session/compact/mode/set` (the last two are AppUI extensions), and the
//! notifications `session/event`, `session/orchestration`,
//! `session/goal/updated`, `session/goal/cleared`.
//!
//! **The new store shape (card #10):** the handler writes through its own
//! domain (`store.domains.session`); the store's flat convenience methods
//! ([`Store::set_active`]) remain for callers that predate the split.
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octoscode_store::{Session, Store};

use crate::method::Method;
use crate::registry::{NotificationHandler, Registry};

/// `session/list` — the legacy per-profile listing (`cwd: None`).
#[derive(Debug, Default, Serialize)]
pub struct SessionList {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// One row of `session/list` (subset of the server's `SessionInfo` used here;
/// serde ignores the rest).
#[derive(Debug, Clone, Deserialize)]
pub struct SessionListRow {
    pub id: String,
    #[serde(default)]
    pub message_count: usize,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub last_prompt: Option<String>,
    #[serde(default)]
    pub active_turn: bool,
}

#[derive(Debug, Deserialize)]
pub struct SessionListResult {
    #[serde(default)]
    pub sessions: Vec<SessionListRow>,
}

impl Method for SessionList {
    const NAME: &'static str = methods::SESSION_LIST;
    type Params = SessionListParams;
    type Result = SessionListResult;
}

/// The params actually sent: only `cwd` is ever meaningful, and we send none.
#[derive(Debug, Default, Serialize)]
pub struct SessionListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

impl From<SessionListRow> for Session {
    fn from(r: SessionListRow) -> Self {
        Session {
            id: r.id,
            title: r.title,
            message_count: r.message_count,
            updated_at: r.updated_at,
            last_prompt: r.last_prompt,
            active_turn: r.active_turn,
        }
    }
}

impl SessionListResult {
    /// Fold the server rows into the store's session list.
    pub fn into_sessions(self) -> Vec<Session> {
        self.sessions.into_iter().map(Session::from).collect()
    }
}

// ---------------------------------------------------------------------------
// Card #F3 — typed Params/Result come straight from octos-core @ a6ea8505 so
// they cannot drift from the pin. Web call sites are cited on each method.
// ---------------------------------------------------------------------------

/// `session/btw` — a quick aside question answered out-of-band while the
/// session's live turn keeps running (octos-core `ui_protocol.rs:3101`
/// `SessionBtwParams`). Web call site `packages/client/src/btw.ts:77`
/// (`{session_id, question}`; `topic` omitted).
pub struct SessionBtw;

impl Method for SessionBtw {
    const NAME: &'static str = methods::SESSION_BTW;
    type Params = octos_core::ui_protocol::SessionBtwParams;
    type Result = octos_core::ui_protocol::SessionBtwResult;
}

/// `session/delete` — remove a session (octos-core `ui_protocol.rs:3470`).
/// Web call site `packages/client/src/client.ts:837`.
pub struct SessionDelete;

impl Method for SessionDelete {
    const NAME: &'static str = methods::SESSION_DELETE;
    type Params = octos_core::ui_protocol::SessionDeleteParams;
    type Result = octos_core::ui_protocol::SessionDeleteResult;
}

/// `session/files.list` — the session's delivered files (octos-core
/// `ui_protocol.rs:3405`). Web call site `packages/client/src/client.ts:845`.
pub struct SessionFilesList;

impl Method for SessionFilesList {
    const NAME: &'static str = methods::SESSION_FILES_LIST;
    type Params = octos_core::ui_protocol::SessionFilesListParams;
    type Result = octos_core::ui_protocol::SessionFilesListResult;
}

/// `session/fork` — branch a new session off an existing one (octos-core
/// `ui_protocol.rs:3025`). Web call site `packages/client/src/history.ts:239`
/// (`{session_id, new_chat_id, copy_messages?}`).
pub struct SessionFork;

impl Method for SessionFork {
    const NAME: &'static str = methods::SESSION_FORK;
    type Params = octos_core::ui_protocol::SessionForkParams;
    type Result = octos_core::ui_protocol::SessionForkResult;
}

/// `session/rollback` — conversation-only rewind (octos-core
/// `ui_protocol.rs:3005`). Web call site `packages/client/src/history.ts:222`
/// (`{session_id, num_turns}`; the web refuses `num_turns == 0`).
pub struct SessionRollback;

impl Method for SessionRollback {
    const NAME: &'static str = methods::SESSION_ROLLBACK;
    type Params = octos_core::ui_protocol::SessionRollbackParams;
    type Result = octos_core::ui_protocol::SessionRollbackResult;
}

/// `session/status/read` — the status-pill poller (octos-core const
/// `ui_protocol.rs:1031`; the *result* is defined by the server, not
/// octos-core). Web call site `packages/client/src/client.ts:655`, parsed by
/// `session-status-result.ts:5`. We type only the fields the store needs and
/// ignore the rest (as `SessionListRow` does), which keeps the parity-critical
/// `session_id` check without mirroring an ever-growing server object.
pub struct SessionStatusRead;

/// Result of `session/status/read`. `session_id` is the identity check the web
/// performs (`session-status-result.ts:6`); the rest are the documented
/// fields the store surfaces.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionStatusReadResult {
    pub session_id: String,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub context_state: Option<octos_core::ui_protocol::UiContextState>,
    /// The server's runtime policy stamp (opaque here; the web keeps it whole).
    #[serde(default)]
    pub runtime_policy_stamp: Option<serde_json::Value>,
}

impl Method for SessionStatusRead {
    const NAME: &'static str = methods::SESSION_STATUS_READ;
    type Params = SessionStatusReadParams;
    type Result = SessionStatusReadResult;
}

/// Params the web actually sends: `{ session_id }` (`client.ts:656-660`).
#[derive(Debug, Default, Serialize)]
pub struct SessionStatusReadParams {
    pub session_id: String,
}

/// `session/compact` — force a context-compaction pass (AppUI extension).
/// Server: `octos-cli/src/api/ui_protocol_transport.rs:278` (const),
/// `handle_session_compact` `:4095`. Web call site
/// `packages/client/src/context-commands.ts:46` (`{session_id}`).
pub struct SessionCompact;

/// Params the web sends: `{ session_id }` (`context-commands.ts:46-48`).
#[derive(Debug, Serialize)]
pub struct SessionCompactParams {
    pub session_id: String,
}

/// Result of `session/compact`, mirroring the web's `CompactResult`
/// (`packages/client/src/context-state.ts:50`) and the server's
/// `{"compacted": …, …}` payload (`ui_protocol_transport.rs:3430`).
#[derive(Debug, Clone, Deserialize)]
pub struct SessionCompactResult {
    pub session_id: String,
    pub compacted: bool,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub input_generation: Option<u64>,
    #[serde(default)]
    pub output_generation: Option<u64>,
    #[serde(default)]
    pub token_estimate_before: Option<u64>,
    #[serde(default)]
    pub token_estimate_after: Option<u64>,
}

impl Method for SessionCompact {
    // AppUI extension: not a `methods::` const in octos-core, so the exact
    // wire name is spelled here (server const cited above).
    const NAME: &'static str = "session/compact";
    type Params = SessionCompactParams;
    type Result = SessionCompactResult;
}

/// `session/compact/mode/set` — the per-session compaction-mode override
/// (AppUI extension). Server: `ui_protocol_transport.rs:281` (const),
/// `handle_session_compact_mode_set` `:4065`. Web call site
/// `packages/client/src/context-commands.ts:59` (`{session_id, mode}`).
pub struct SessionCompactModeSet;

/// Params the web sends: `{ session_id, mode: "llm" | "heuristic" }`
/// (`context-commands.ts:59-62`).
#[derive(Debug, Serialize)]
pub struct SessionCompactModeSetParams {
    pub session_id: String,
    pub mode: String,
}

/// Result the server returns: `{ session_id, mode }`
/// (`ui_protocol_transport.rs:4083`).
#[derive(Debug, Clone, Deserialize)]
pub struct SessionCompactModeSetResult {
    pub session_id: String,
    pub mode: String,
}

impl Method for SessionCompactModeSet {
    const NAME: &'static str = "session/compact/mode/set";
    type Params = SessionCompactModeSetParams;
    type Result = SessionCompactModeSetResult;
}

// ---------------------------------------------------------------------------
// Notification handlers
// ---------------------------------------------------------------------------

/// `session/opened`-class: a session became active on this connection.
pub struct SessionOpenedHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for SessionOpenedHandler {
    const METHOD: &'static str = methods::SESSION_OPEN;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::SessionOpened(opened) = notification {
            self.store.note_seen(Self::METHOD);
            self.store
                .domains
                .session
                .set_active(Some(opened.session_id.0.clone()));
            self.store.domains.profile.set_current(
                opened
                    .active_profile_id
                    .clone()
                    .unwrap_or_else(|| opened.session_id.0.clone()),
            );
        }
    }
}

/// `session/event` — a legacy `/events/stream` SSE frame bridged onto the v1
/// ledger (`SessionEventBridgedEvent`, `ui_protocol.rs:6386`). The store keeps
/// the last bridged frame per session so a UI can surface the raw kind.
pub struct SessionEventBridgedHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for SessionEventBridgedHandler {
    const METHOD: &'static str = methods::SESSION_EVENT;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::SessionEventBridged(event) = notification {
            self.store.note_seen(Self::METHOD);
            self.store.domains.session.note_bridged_event(
                &event.session_id.0,
                &event.kind,
                event.payload.clone(),
            );
        }
    }
}

/// `session/orchestration` — whole-job orchestration status
/// (`SessionOrchestrationEvent`, `ui_protocol.rs:5170`). Written to the
/// session domain so a UI can render a live job indicator.
pub struct SessionOrchestrationHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for SessionOrchestrationHandler {
    const METHOD: &'static str = methods::SESSION_ORCHESTRATION;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::SessionOrchestration(event) = notification {
            self.store.note_seen(Self::METHOD);
            self.store.domains.session.set_orchestration(
                &event.session_id.0,
                octoscode_store::domains::session::OrchestrationSnapshot {
                    active: event.active,
                    running_agents: event.running_agents,
                    pending_continuations: event.pending_continuations,
                    phase: event.phase.clone(),
                },
            );
        }
    }
}

/// `session/goal/updated` — the persisted goal changed
/// (`SessionGoalUpdatedEvent`, `ui_protocol.rs:5936`). The generation gate is
/// applied in the store (a stale update never resurrects a cleared goal).
pub struct SessionGoalUpdatedHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for SessionGoalUpdatedHandler {
    const METHOD: &'static str = methods::SESSION_GOAL_UPDATED;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::SessionGoalUpdated(event) = notification {
            self.store.note_seen(Self::METHOD);
            self.store.domains.session.apply_goal_update(
                &event.session_id.0,
                event.generation,
                Some(serde_json::to_value(&event.goal).unwrap_or(serde_json::Value::Null)),
            );
        }
    }
}

/// `session/goal/cleared` — the persisted goal was cleared
/// (`SessionGoalClearedEvent`, `ui_protocol.rs:5953`).
pub struct SessionGoalClearedHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for SessionGoalClearedHandler {
    const METHOD: &'static str = methods::SESSION_GOAL_CLEARED;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::SessionGoalCleared(event) = notification {
            self.store.note_seen(Self::METHOD);
            self.store
                .domains
                .session
                .apply_goal_clear(&event.session_id.0, event.generation);
        }
    }
}

/// Register this domain's notification handlers.
pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(SessionOpenedHandler { store: store.clone() });
    reg.register(SessionEventBridgedHandler { store: store.clone() });
    reg.register(SessionOrchestrationHandler { store: store.clone() });
    reg.register(SessionGoalUpdatedHandler { store: store.clone() });
    reg.register(SessionGoalClearedHandler { store });
}
