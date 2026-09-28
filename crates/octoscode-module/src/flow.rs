//! The conversation flow — the live gate's path, as one small async API.
//!
//! Card #12 §2. The gate walks: connect (WS) → pick profile → **open a
//! workspace** → `turn/start` → stream `message/delta` (+ tool/turn
//! notifications) into the store timeline → `turn/interrupt` →
//! `turn/completed|error`.
//!
//! ## Why a flow and not just the module's event loop
//!
//! Two things the gate needs are *not* store state:
//! - **Turn timing.** `answer.worked_for` / `turn.activity` are durations the
//!   wire never carries; only the party that sent `turn/start` knows `now`.
//! - **A protocol trace.** "every frame, direction, method, id, ms" (card §2)
//!   — the transport has no trace hook, so the flow records the frames **it**
//!   sends and the events **it** drains, at the boundary it owns.
//!
//! ## Citations (the web's path)
//!
//! - Features: [`crate::features`] = `client.ts:106-128` via `url.ts:24-28`.
//! - `session/open` with a workspace cwd: `session-config/session-defaults.ts:5-7`
//!   ("sandbox into `session/open`, … applied at CREATION only") and
//!   `session/workspace-session-catalog.ts:15-18` (the `<cwd>/.octos/<profile>`
//!   store the server writes each session to).
//! - `turn/start`: the web calls **the generic request**, not a typed command —
//!   `client.ts:488` `return this.request(CORE_UI_METHODS.TURN_START, params)`.
//!   Params `{session_id, turn_id, input:[{kind:"text",text}]}`,
//!   `apps/web/src/features/composer/use-turn-controller.ts:398-412`.
//! - `turn/interrupt`: `{session_id, turn_id}` (`ui_protocol.rs:2097`).
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use octos_app_transport::{
    LifecycleResult, OutboundCommand, ProfileId, SecretString, TransportConfig, TransportEvent,
    ws,
};
use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::{SessionOpenParams, TurnId};
use octoscode_client::{Client, ClientError, Registry};
use octoscode_store::Store;
use url::Url;

/// Which way a traced frame went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Client → server (a command we sent).
    Out,
    /// Server → client (an event we drained).
    In,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Out => "->",
            Self::In => "<-",
        }
    }
}

/// One traced frame: direction, method, the id when there is one, and ms
/// since the conversation started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEntry {
    pub seq: u64,
    pub at_ms: u128,
    pub direction: Direction,
    pub method: String,
    /// JSON-RPC id (`None` for notifications, which carry no id).
    pub id: Option<String>,
    /// A short human note (e.g. the turn id, a delta's byte count).
    pub note: Option<String>,
}

impl std::fmt::Display for TraceEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:>6}ms {} {}{}{}",
            self.at_ms,
            self.direction.as_str(),
            self.method,
            self.id.as_deref().map(|i| format!(" id={i}")).unwrap_or_default(),
            self.note.as_deref().map(|n| format!(" ({n})")).unwrap_or_default(),
        )
    }
}

/// The trace sink the gate report prints. Cheap to clone-share.
#[derive(Debug, Clone, Default)]
pub struct TraceSink(Arc<Mutex<Vec<TraceEntry>>>);

impl TraceSink {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one frame. `method`, `id` and `note` are the frame's own.
    pub fn record(
        &self,
        started: Instant,
        direction: Direction,
        method: impl Into<String>,
        id: Option<String>,
        note: Option<String>,
    ) {
        let mut v = self.0.lock().unwrap();
        let seq = v.len() as u64;
        v.push(TraceEntry {
            seq,
            at_ms: started.elapsed().as_millis(),
            direction,
            method: method.into(),
            id,
            note,
        });
    }

    /// Every frame, in order.
    pub fn entries(&self) -> Vec<TraceEntry> {
        self.0.lock().unwrap().clone()
    }

    /// How many frames were traced.
    pub fn len(&self) -> usize {
        self.0.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The human-readable trace the card's gate report prints.
    pub fn render(&self) -> String {
        self.entries()
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// One tool row for the `tools` binding (`tool/started` … `tool/completed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRow {
    pub tool_call_id: String,
    pub name: String,
    pub summary: String,
    pub status: String,
}

/// The flow's own UI-relevant facts — what no protocol method reports.
#[derive(Debug, Default)]
pub struct FlowUi {
    /// `composer.draft` — the text input's current value.
    draft: String,
    /// Tool rows, keyed by call id (a progress/completed updates its row).
    tools: Vec<ToolRow>,
    /// `tool.output` — the completed tools' output previews, in order.
    tool_output: Vec<String>,
    /// The turn currently in flight, with when it started.
    active_turn: Option<(String, Instant)>,
    /// The last completed turn's wall-clock duration (`answer.worked_for`).
    last_worked: Option<Duration>,
    /// `answer.timestamp` — when the last turn completed.
    last_completed_at: Option<std::time::SystemTime>,
    /// `approval.pending` — an `approval/requested` is outstanding.
    approval_pending: bool,
    /// `question.pending` — a `user_question/requested` is outstanding.
    question_pending: bool,
    /// `tools[].expanded` — which tool rows the person disclosed. UI-local:
    /// the card toggles it, the store never sees it (`bindings.json` note).
    expanded: Vec<String>,
    /// `answer.expand` state (the "worked for" disclosure).
    answer_expanded: bool,
}

impl FlowUi {
    pub fn draft(&self) -> String {
        self.draft.clone()
    }

    pub fn set_draft_inner(&mut self, text: impl Into<String>) {
        self.draft = text.into();
    }

    pub fn tools(&self) -> Vec<ToolRow> {
        self.tools.clone()
    }

    pub fn tool_output(&self) -> Vec<String> {
        self.tool_output.clone()
    }

    pub fn active_turn(&self) -> Option<String> {
        self.active_turn.as_ref().map(|(id, _)| id.clone())
    }

    pub fn turn_active(&self) -> bool {
        self.active_turn.is_some()
    }

    /// `turn.activity` — "Working · 12s" while a turn is live (card §2:
    /// the web's activity row, parity `timeline`), else empty.
    pub fn turn_activity(&self) -> String {
        match &self.active_turn {
            Some((_, started)) => format!("Working · {}s", started.elapsed().as_secs()),
            None => String::new(),
        }
    }

    /// `answer.worked_for` — "Worked for 3m 4s" for the last completed turn.
    pub fn worked_for(&self) -> String {
        match self.last_worked {
            Some(d) => {
                let secs = d.as_secs();
                if secs >= 60 {
                    format!("Worked for {}m {}s", secs / 60, secs % 60)
                } else {
                    format!("Worked for {secs}s")
                }
            }
            None => String::new(),
        }
    }

    /// `answer.timestamp` — the last turn's completion, as a compact label.
    pub fn answer_timestamp(&self) -> String {
        self.last_completed_at
            .map(|t| {
                let secs = t
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                format!("t={secs}")
            })
            .unwrap_or_default()
    }

    /// `tools[].expanded` — is this row disclosed?
    pub fn is_expanded(&self, key: &str) -> bool {
        self.expanded.iter().any(|k| k == key)
    }

    /// Flip a row's disclosure; returns the new state.
    pub fn toggle_expanded(&mut self, key: &str) -> bool {
        if let Some(pos) = self.expanded.iter().position(|k| k == key) {
            self.expanded.remove(pos);
            false
        } else {
            self.expanded.push(key.to_owned());
            true
        }
    }

    /// `answer.expand` — the "worked for" disclosure toggle.
    pub fn answer_expanded(&self) -> bool {
        self.answer_expanded
    }

    pub fn toggle_answer_expanded(&mut self) -> bool {
        self.answer_expanded = !self.answer_expanded;
        self.answer_expanded
    }

    pub fn approval_pending(&self) -> bool {
        self.approval_pending
    }

    pub fn question_pending(&self) -> bool {
        self.question_pending
    }

    fn note_tool_started(&mut self, call_id: &str, name: &str) {
        self.tools.retain(|t| t.tool_call_id != call_id);
        self.tools.push(ToolRow {
            tool_call_id: call_id.to_owned(),
            name: name.to_owned(),
            summary: name.to_owned(),
            status: "running".to_owned(),
        });
    }

    fn note_tool_progress(&mut self, call_id: &str, message: Option<&str>) {
        if let Some(t) = self.tools.iter_mut().find(|t| t.tool_call_id == call_id) {
            t.status = "running".to_owned();
            if let Some(m) = message {
                t.summary = m.to_owned();
            }
        }
    }

    fn note_tool_completed(&mut self, call_id: &str, name: &str, ok: bool, preview: Option<&str>) {
        if let Some(t) = self.tools.iter_mut().find(|t| t.tool_call_id == call_id) {
            t.status = if ok { "done" } else { "failed" }.to_owned();
            if let Some(p) = preview {
                t.summary = p.lines().next().unwrap_or(p).to_owned();
            }
        } else {
            self.tools.push(ToolRow {
                tool_call_id: call_id.to_owned(),
                name: name.to_owned(),
                summary: name.to_owned(),
                status: if ok { "done" } else { "failed" }.to_owned(),
            });
        }
        if let Some(p) = preview {
            self.tool_output.push(p.to_owned());
        }
    }
}

/// What one drained event did, for a test or a log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowEvent {
    Connecting(String),
    Live,
    Capabilities(usize),
    WorkspaceOpened(String),
    TurnStarted(String),
    TurnEnded { turn_id: String, error: Option<String> },
    Delta { turn_id: String, bytes: usize },
    ToolStarted { tool_call_id: String, name: String },
    ToolCompleted { tool_call_id: String, ok: bool },
    ApprovalPending,
    QuestionPending,
    Other(String),
}

/// The conversation: one transport, one store, one trace.
pub struct Conversation {
    pub store: Arc<Store>,
    pub trace: TraceSink,
    ui: Arc<Mutex<FlowUi>>,
    client: Client,
    cmd_tx: tokio::sync::mpsc::Sender<OutboundCommand>,
    registry: Mutex<Registry>,
    profile: String,
    session_id: String,
    started: Instant,
}

impl Conversation {
    /// Build a conversation and spawn its transport. `waker` wakes the UI
    /// thread (makepad's `SignalToUI`) after each event is queued.
    ///
    /// The base URL carries the web's `ui_feature=` query params
    /// ([`octoscode_client::features`]) — our transport clones `base_url`
    /// verbatim, so they reach the socket as the web sends them.
    pub fn connect(
        base: &str,
        bearer: &str,
        profile: &str,
        workspace_cwd: Option<String>,
        waker: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<(Self, tokio::sync::mpsc::Receiver<TransportEvent>), String> {
        // The web sends its features as repeated `ui_feature=` query params
        // (`url.ts:24-28`). Our transport clones `base_url` verbatim into the
        // WS URI, so setting them here is the web-identical connect path.
        let mut base_url = Url::parse(base).map_err(|e| format!("bad OCTOS_BASE_URL: {e}"))?;
        {
            let pairs: Vec<(String, String)> = base_url
                .query_pairs()
                .filter(|(k, _)| k != "ui_feature")
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();
            base_url.set_query(None);
            let mut q = base_url.query_pairs_mut();
            for (k, v) in &pairs {
                q.append_pair(k, v);
            }
            for f in octoscode_client::features::WEB_UI_FEATURES {
                q.append_pair("ui_feature", f);
            }
        }
        let cfg = TransportConfig {
            base_url,
            bearer: SecretString::new(bearer.to_owned()),
            profile_id: ProfileId::new(profile.to_owned()),
            cursor: None,
            cursor_file: None,
            requested_capabilities: octoscode_client::features::web_capabilities(),
            workspace_cwd,
            local_kernel: false,
        };
        let (cmd_tx, evt_rx) = ws::spawn_with_waker(cfg, waker);
        let store = Arc::new(Store::new());
        let mut registry = Registry::new();
        octoscode_client::domains::register_all(&mut registry, store.clone());
        let trace = TraceSink::new();
        trace.record(
            Instant::now(),
            Direction::Out,
            "connect",
            None,
            Some(format!("features={}", octoscode_client::features::WEB_UI_FEATURES.len())),
        );
        Ok((
            Self {
                store,
                trace,
                ui: Arc::new(Mutex::new(FlowUi::default())),
                client: Client::new(cmd_tx.clone()),
                cmd_tx,
                registry: Mutex::new(registry),
                profile: profile.to_owned(),
                session_id: format!("{profile}:main"),
                started: Instant::now(),
            },
            evt_rx,
        ))
    }

    pub fn profile(&self) -> &str {
        &self.profile
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    /// The flow's own UI state, shareable (the binding table holds this).
    pub fn ui(&self) -> Arc<Mutex<FlowUi>> {
        self.ui.clone()
    }

    /// The inner borrow — for the module's UI thread, which already holds a
    /// lock on the bridge.
    pub fn ui_ref(&self) -> &Mutex<FlowUi> {
        &self.ui
    }

    pub fn set_draft(&self, text: impl Into<String>) {
        self.ui.lock().unwrap().set_draft_inner(text);
    }

    /// `profile/local/create` — onboard a profile on a fresh solo serve
    /// (the precondition; not a matrix row).
    pub async fn create_profile(&self) -> Result<String, ClientError> {
        let id = format!("{}-{}", self.profile, std::process::id());
        let result = self
            .client
            .request(
                "profile/local/create",
                serde_json::json!({"requested_id": id, "name": "OctosCode", "username": id}),
            )
            .await?;
        self.trace.record(
            self.started,
            Direction::Out,
            "profile/local/create",
            None,
            Some(id.clone()),
        );
        Ok(result["profile_id"].as_str().unwrap_or(&id).to_owned())
    }

    /// **Open a workspace** — `session/open` carrying the workspace cwd, the
    /// way the web does (`session-defaults.ts:5-7`). Sends the transport's
    /// typed `OpenSession` (it owns the replay-cursor bracket).
    pub async fn open_workspace(&self, cwd: Option<String>) -> Result<String, String> {
        let session_id = octos_core::SessionKey::new(&self.profile, "main");
        let params = SessionOpenParams {
            session_id: session_id.clone(),
            topic: None,
            profile_id: Some(self.profile.clone()),
            cwd,
            sandbox: None,
            after: None,
            client_commands: None,
        };
        self.trace.record(
            self.started,
            Direction::Out,
            "session/open",
            None,
            Some(format!("session={}", session_id.0)),
        );
        self.cmd_tx
            .send(OutboundCommand::OpenSession(params))
            .await
            .map_err(|_| "transport channel closed".to_owned())?;
        Ok(session_id.0)
    }

    /// `turn/start` — the web's generic request (`client.ts:488`), not a typed
    /// command. Returns the turn id it generated (UUID v7, as the web's
    /// `turn.turnId`).
    pub async fn start_turn(&self, text: impl Into<String>) -> Result<String, ClientError> {
        let turn_id = TurnId::new().0.to_string();
        let params = serde_json::json!({
            "session_id": self.session_id,
            "turn_id": turn_id,
            "input": [{"kind": "text", "text": text.into()}],
        });
        self.trace.record(
            self.started,
            Direction::Out,
            "turn/start",
            None,
            Some(format!("turn={turn_id}")),
        );
        // The turn is live from the moment we dispatch (the web does the same:
        // `acceptLocalDispatch(turn.turnId, "running")`, use-turn-controller
        // `:413`), so the composer shows STOP before the ACK lands.
        self.ui
            .lock()
            .unwrap()
            .begin_turn(&turn_id, self.started);
        self.client.request("turn/start", params).await?;
        Ok(turn_id)
    }

    /// `turn/interrupt` — `{session_id, turn_id}` (`ui_protocol.rs:2097`).
    pub async fn interrupt(&self, turn_id: &str) -> Result<serde_json::Value, ClientError> {
        self.trace.record(
            self.started,
            Direction::Out,
            "turn/interrupt",
            None,
            Some(format!("turn={turn_id}")),
        );
        self.client
            .request(
                "turn/interrupt",
                serde_json::json!({"session_id": self.session_id, "turn_id": turn_id}),
            )
            .await
    }

    /// `composer.submit` — the composer's send button: `turn/start` with the
    /// current draft (`bindings.json` `composer.submit`).
    pub async fn submit_draft(&self) -> Result<String, ClientError> {
        let text = self.ui.lock().unwrap().draft();
        self.start_turn(text).await
    }

    /// Drain one transport event into the store, the flow's own UI state, and
    /// the trace. Returns what happened, for a test or a log.
    pub fn on_event(&self, evt: TransportEvent) -> FlowEvent {
        let out = self.dispatch(&evt);
        self.trace.record(
            self.started,
            Direction::In,
            trace_method(&evt),
            None,
            Some(format!("{out:?}")),
        );
        out
    }

    fn dispatch(&self, evt: &TransportEvent) -> FlowEvent {
        match evt {
            TransportEvent::ConnectionState(s) => {
                let live = matches!(s, octos_app_transport::ConnectionState::Live);
                self.store.set_connection(format!("{s:?}"), live);
                if live {
                    FlowEvent::Live
                } else {
                    FlowEvent::Connecting(format!("{s:?}"))
                }
            }
            TransportEvent::CapabilityNegotiated(caps) => {
                let accepted: Vec<String> = caps.raw.keys().cloned().collect();
                let n = accepted.len();
                self.store.set_capabilities(accepted);
                FlowEvent::Capabilities(n)
            }
            TransportEvent::RpcResult(LifecycleResult::SessionOpen(r)) => {
                self.store.set_active(Some(r.opened.session_id.0.clone()));
                FlowEvent::WorkspaceOpened(r.opened.session_id.0.clone())
            }
            TransportEvent::SessionsListed { sessions } => {
                if let Ok(rows) = serde_json::from_value::<
                    Vec<octoscode_client::domains::session::SessionListRow>,
                >(sessions.clone())
                {
                    self.store
                        .set_sessions(rows.into_iter().map(Into::into).collect());
                }
                FlowEvent::Other("session/list".to_owned())
            }
            TransportEvent::DurableNotification { payload, .. }
            | TransportEvent::EphemeralNotification { payload } => {
                let ev = self.note_notification(payload);
                self.registry.lock().unwrap().dispatch(payload);
                ev
            }
            TransportEvent::RpcError { method, error, .. } => {
                ::log::warn!("octoscode: rpc error {method}: {}", error.message);
                FlowEvent::Other(format!("rpc-error {method}"))
            }
            other => FlowEvent::Other(format!("{other:?}")),
        }
    }

    /// Record the flow's own view of a notification (tool rows, turn timing,
    /// pending approvals) — the things the store's domain handlers do not keep.
    fn note_notification(&self, n: &UiNotification) -> FlowEvent {
        let mut ui = self.ui.lock().unwrap();
        match n {
            UiNotification::TurnStarted(e) => {
                ui.begin_turn(&e.turn_id.0.to_string(), self.started);
                FlowEvent::TurnStarted(e.turn_id.0.to_string())
            }
            UiNotification::TurnCompleted(e) => {
                let turn_id = e.turn_id.0.to_string();
                ui.end_turn(true);
                FlowEvent::TurnEnded { turn_id, error: None }
            }
            UiNotification::TurnError(e) => {
                let turn_id = e.turn_id.0.to_string();
                ui.end_turn(false);
                FlowEvent::TurnEnded {
                    turn_id,
                    error: Some(format!("{}: {}", e.code, e.message)),
                }
            }
            UiNotification::MessageDelta(e) => {
                ui.touch_turn();
                FlowEvent::Delta {
                    turn_id: e.turn_id.0.to_string(),
                    bytes: e.text.len(),
                }
            }
            UiNotification::ToolStarted(e) => {
                ui.note_tool_started(&e.tool_call_id, &e.tool_name);
                FlowEvent::ToolStarted {
                    tool_call_id: e.tool_call_id.clone(),
                    name: e.tool_name.clone(),
                }
            }
            UiNotification::ToolProgress(e) => {
                ui.note_tool_progress(&e.tool_call_id, e.message.as_deref());
                FlowEvent::Other("tool/progress".to_owned())
            }
            UiNotification::ToolCompleted(e) => {
                let ok = e.success.unwrap_or(true);
                ui.note_tool_completed(
                    &e.tool_call_id,
                    &e.tool_name,
                    ok,
                    e.output_preview.as_deref(),
                );
                FlowEvent::ToolCompleted {
                    tool_call_id: e.tool_call_id.clone(),
                    ok,
                }
            }
            UiNotification::ApprovalRequested(_) => {
                ui.approval_pending = true;
                FlowEvent::ApprovalPending
            }
            UiNotification::UserQuestionRequested(_) => {
                ui.question_pending = true;
                FlowEvent::QuestionPending
            }
            other => FlowEvent::Other(other.method().to_owned()),
        }
    }
}

impl FlowUi {
    // ---- test support -------------------------------------------------
    // Deterministic 1-arg helpers so a binding/store test needs no clock.

    /// Mark a turn live (test support; production uses `begin_turn`).
    pub fn begin_turn_now(&mut self, turn_id: &str) {
        self.active_turn = Some((turn_id.to_owned(), Instant::now()));
    }

    /// End the live turn (test support; production uses `end_turn`).
    pub fn end_turn_now(&mut self, ok: bool) {
        self.end_turn(ok);
    }

    /// Note a tool starting (test support; the event path uses this too).
    pub fn note_tool_started_for_test(&mut self, call_id: &str, name: &str) {
        self.note_tool_started(call_id, name);
    }

    /// Note a tool completing (test support).
    pub fn note_tool_completed_for_test(
        &mut self,
        call_id: &str,
        name: &str,
        ok: bool,
        preview: Option<&str>,
    ) {
        self.note_tool_completed(call_id, name, ok, preview);
    }

    /// Force the pending flags (test support).
    pub fn set_pending_for_test(&mut self, approval: bool, question: bool) {
        self.approval_pending = approval;
        self.question_pending = question;
    }

    fn begin_turn(&mut self, turn_id: &str, _started: Instant) {
        self.active_turn = Some((turn_id.to_owned(), Instant::now()));
    }

    fn touch_turn(&mut self) {
        if self.active_turn.is_none() {
            self.active_turn = Some((String::new(), Instant::now()));
        }
    }

    fn end_turn(&mut self, _ok: bool) {
        if let Some((_, started)) = self.active_turn.take() {
            self.last_worked = Some(started.elapsed());
        }
        self.last_completed_at = Some(std::time::SystemTime::now());
        self.approval_pending = false;
        self.question_pending = false;
    }
}

fn trace_method(evt: &TransportEvent) -> String {
    match evt {
        TransportEvent::ConnectionState(s) => format!("state:{s:?}"),
        TransportEvent::CapabilityNegotiated(_) => "capabilities".to_owned(),
        TransportEvent::RpcResult(LifecycleResult::SessionOpen(_)) => "session/open".to_owned(),
        TransportEvent::RpcResult(LifecycleResult::TurnStart(_)) => "turn/start".to_owned(),
        TransportEvent::RpcResult(LifecycleResult::TurnInterrupt(_)) => "turn/interrupt".to_owned(),
        TransportEvent::RpcError { method, .. } => format!("error:{method}"),
        TransportEvent::SessionsListed { .. } => "session/list".to_owned(),
        TransportEvent::SessionHydrated { .. } => "session/hydrate".to_owned(),
        TransportEvent::DurableNotification { payload, .. }
        | TransportEvent::EphemeralNotification { payload } => payload.method().to_owned(),
    }
}
