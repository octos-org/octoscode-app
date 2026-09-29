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
    /// Card #13: the JSONL frame recorder (`OCTOSCODE_TRACE_FILE`). Records
    /// every inbound event here; the outbound half is recorded by `Client`.
    frames: octoscode_client::trace::FrameTrace,
    /// How many `TraceSink` entries `flush_trace` has already written.
    flushed: Mutex<usize>,
    ui: Arc<Mutex<FlowUi>>,
    client: Client,
    cmd_tx: tokio::sync::mpsc::Sender<OutboundCommand>,
    registry: Mutex<Registry>,
    profile: String,
    /// Card #14 defect 4: mutable, so a New chat adopts a fresh id and a resume
    /// adopts a listed one. Read through [`Conversation::session_id`].
    session_id: Mutex<String>,
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
        let frames = octoscode_client::trace::FrameTrace::from_env();
        if frames.is_enabled() {
            ::log::info!("octoscode: frame trace -> {:?}", frames.path());
        }
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
                frames: frames.clone(),
                flushed: Mutex::new(0),
                ui: Arc::new(Mutex::new(FlowUi::default())),
                client: Client::with_trace(cmd_tx.clone(), frames.clone()),
                cmd_tx,
                registry: Mutex::new(registry),
                profile: profile.to_owned(),
                session_id: Mutex::new(format!("{profile}:main")),
                started: Instant::now(),
            },
            evt_rx,
        ))
    }

    pub fn profile(&self) -> &str {
        &self.profile
    }

    /// The session the flow currently drives — `<profile>:main` until a
    /// `session/new` or a resume changes it.
    pub fn session_id(&self) -> String {
        self.session_id.lock().unwrap().clone()
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    /// The outbound command channel — for fire-and-forget sends the module
    /// makes from the UI thread (`session/list` on the refresh button).
    pub fn command_sender(&self) -> &tokio::sync::mpsc::Sender<OutboundCommand> {
        &self.cmd_tx
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
        let id = self.session_id();
        self.open_workspace_as(&id, cwd).await
    }

    /// The id-taking open: `session/open` for `id` (card #14 defect 4, so a
    /// fresh chat and a resume share ONE path).
    pub async fn open_workspace_as(
        &self,
        id: &str,
        cwd: Option<String>,
    ) -> Result<String, String> {
        let session_id = octos_core::SessionKey(id.to_owned());
        // Record the outbound frame BEFORE `cwd` moves into the params.
        self.frames.out(
            "session/open",
            &serde_json::json!({
                "session_id": session_id.0,
                "profile_id": self.profile,
                "cwd": cwd,
            }),
        );
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
        // Adopt the id we opened, so `turn/start` / `turn/interrupt` drive the
        // session that is actually live (a fresh chat or a resume).
        *self.session_id.lock().unwrap() = session_id.0.clone();
        if let Err(e) = self.refresh_sessions().await {
            ::log::warn!("octoscode: session/list after open: {e}");
        }
        ::log::info!("octoscode: workspace open requested for {}", session_id.0);
        Ok(session_id.0)
    }

    /// `turn/start` — the web's generic request (`client.ts:488`), not a typed
    /// command. Returns the turn id it generated (UUID v7, as the web's
    /// `turn.turnId`).
    pub async fn start_turn(&self, text: impl Into<String>) -> Result<String, ClientError> {
        let turn_id = TurnId::new().0.to_string();
        let params = serde_json::json!({
            "session_id": self.session_id(),
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
        {
            let mut ui = self.ui.lock().unwrap();
            ui.begin_turn(&turn_id, self.started);
            // Card #13 §4: the draft clears on send, so the composer is empty
            // for the next prompt (the web clears it when the turn is
            // dispatched). The text is already captured in `params`.
            ui.set_draft_inner(String::new());
        }
        match self.client.request("turn/start", params).await {
            Ok(v) => {
                ::log::info!("octoscode: turn started {turn_id}");
                Ok(turn_id)
            }
            Err(e) => {
                ::log::error!("octoscode: turn/start failed for {turn_id}: {e}");
                Err(e)
            }
        }
    }

    /// `turn/steer` — send the queued input into the LIVE turn's input buffer
    /// (card #21 §3: the composer's "Steer now" control). The AppUI extension
    /// shape: `{session_id, expected_turn_id, input:[{kind:"text",text}]}`
    /// (`domains/turn.rs:141`, web `steer.ts:41`).
    pub async fn steer(&self, text: &str) -> Result<serde_json::Value, ClientError> {
        let expected = self.ui.lock().unwrap().active_turn();
        let params = serde_json::json!({
            "session_id": self.session_id(),
            "expected_turn_id": expected,
            "input": [{"kind": "text", "text": text}],
        });
        self.trace.record(self.started, Direction::Out, "turn/steer", None, None);
        self.client.request("turn/steer", params).await
    }

    /// `turn/interrupt` — `{session_id, turn_id}` (`ui_protocol.rs:2097`).
    pub async fn interrupt(&self, turn_id: &str) -> Result<serde_json::Value, ClientError> {
        ::log::info!("octoscode: interrupting turn {turn_id}");
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
                serde_json::json!({"session_id": self.session_id(), "turn_id": turn_id}),
            )
            .await
    }

    /// Mint the id for a **new chat** (card #14 defect 4).
    ///
    /// Every gate run reused `dsflash:main`, so context leaked between runs.
    /// The web mints a fresh, profile-neutral id per new session
    /// (`src-web/apps/web/src/features/session/session-identity.ts:10`
    /// `freshWebSessionId`, then `:23` `bindWebSessionIdToProfile` → the
    /// resolved profile is embedded exactly once). Resume stays possible: an
    /// existing id is simply passed to [`Conversation::open_session`].
    pub fn fresh_session_id(&self) -> String {
        Self::fresh_session_id_for(&self.profile)
    }

    /// The id-minting rule, as a pure function so it is testable without a
    /// transport.
    ///
    /// A fresh, profile-scoped, unique id, mirroring the web: `freshWebSessionId`
    /// mints a random one (`session-identity.ts:10`) and
    /// `bindWebSessionIdToProfile` embeds the resolved profile exactly once
    /// (`session-identity.ts:23`). We mint `<profile>:<uuid>` (the uuid from
    /// `TurnId`, a UUID newtype octos-core already exposes — no new dep).
    pub fn fresh_session_id_for(profile: &str) -> String {
        format!("{profile}:{}", TurnId::new().0)
    }

    /// Open a specific session id — the resume path (an id the server listed),
    /// or a freshly minted one from [`Conversation::fresh_session_id`].
    pub async fn open_session(&self, id: &str, cwd: Option<String>) -> Result<String, String> {
        self.open_workspace_as(id, cwd).await
    }

    /// `session.new` — a **New chat**: mint a fresh session id, adopt it, and
    /// open it (card #14 defect 4).
    ///
    /// Every gate run reused `dsflash:main`, so context leaked between runs.
    /// The web mints a fresh id per new session
    /// (`session-identity.ts:10` `freshWebSessionId`, bound to the profile at
    /// `:23`). Resume is unchanged: [`Conversation::open_session`] takes any
    /// existing id the server listed.
    pub async fn new_chat(&self, cwd: Option<String>) -> Result<String, String> {
        let id = Self::fresh_session_id_for(&self.profile);
        ::log::info!("octoscode: new chat -> {id}");
        self.open_workspace_as(&id, cwd).await
    }

    /// `session/list` — re-ask for the session rows and fold them into the
    /// store (the `session.refresh` action). Returns the row count.
    pub async fn refresh_sessions(&self) -> Result<usize, ClientError> {
        let result = self
            .client
            .call::<octoscode_client::domains::session::SessionList>(
                octoscode_client::domains::session::SessionListParams::default(),
            )
            .await?;
        let sessions = result.into_sessions();
        let n = sessions.len();
        self.store.set_sessions(sessions);
        Ok(n)
    }

    /// `composer.submit` — the composer's send button: `turn/start` with the
    /// current draft (`bindings.json` `composer.submit`).
    pub async fn submit_draft(&self) -> Result<String, ClientError> {
        let text = self.ui.lock().unwrap().draft();
        self.start_turn(text).await
    }

    /// Flush the in-memory [`TraceSink`] transitions into the frame file
    /// (card #13 §1: the file also carries the flow's own transitions, so a
    /// fixture is self-contained). Writes only entries not yet flushed, so
    /// calling it repeatedly is safe.
    /// Whether the JSONL frame recorder is on (`OCTOSCODE_TRACE_FILE` set).
    pub fn trace_enabled(&self) -> bool {
        self.frames.is_enabled()
    }

    pub fn flush_trace(&self) {
        if !self.frames.is_enabled() {
            return;
        }
        let entries = self.trace.entries();
        let mut flushed = self.flushed.lock().unwrap();
        for e in entries.iter().skip(*flushed) {
            self.frames.raw_line(serde_json::json!({
                "dir": "flow",
                "method": e.method,
                "at_ms": e.at_ms,
                "note": e.note,
            }));
        }
        *flushed = entries.len();
    }

    /// Log a flow transition at info (card #13 §4: connect, open, turn
    /// start/complete/error, interrupt). One line per transition, named.
    fn log_transition(&self, e: &FlowEvent) {
        match e {
            FlowEvent::Live => ::log::info!("octoscode: connection live"),
            FlowEvent::WorkspaceOpened(id) => ::log::info!("octoscode: workspace opened {id}"),
            FlowEvent::TurnStarted(id) => ::log::info!("octoscode: turn started {id}"),
            FlowEvent::TurnEnded { turn_id, error: None } => {
                ::log::info!("octoscode: turn completed {turn_id}")
            }
            FlowEvent::TurnEnded {
                turn_id,
                error: Some(err),
            } => ::log::error!("octoscode: turn failed {turn_id}: {err}"),
            FlowEvent::ApprovalPending => ::log::info!("octoscode: approval requested"),
            FlowEvent::QuestionPending => ::log::info!("octoscode: user question requested"),
            _ => {}
        }
    }

    /// Drain one transport event into the store, the flow's own UI state, and
    /// the trace. Returns what happened, for a test or a log.
    pub fn on_event(&self, evt: TransportEvent) -> FlowEvent {
        // Card #13 §1: every inbound frame, one JSONL line, before dispatch.
        self.frames
            .inbound(&trace_method(&evt), &trace_params(&evt));
        let out = self.dispatch(&evt);
        self.log_transition(&out);
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
                ui.end_turn(&turn_id, true);
                FlowEvent::TurnEnded { turn_id, error: None }
            }
            UiNotification::TurnError(e) => {
                let turn_id = e.turn_id.0.to_string();
                ui.end_turn(&turn_id, false);
                FlowEvent::TurnEnded {
                    turn_id,
                    error: Some(format!("{}: {}", e.code, e.message)),
                }
            }
            UiNotification::MessageDelta(e) => {
                ui.touch_turn(&e.turn_id.0.to_string());
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
            UiNotification::EnvelopeV2(frame) => {
                use octos_core::ui_protocol::{PayloadV2, TurnTerminalOutcome};
                let turn_id = frame.envelope.turn_id.clone();
                match &frame.envelope.payload {
                    PayloadV2::AssistantDelta { text, .. } => {
                        ui.touch_turn(&turn_id);
                        FlowEvent::Delta { turn_id, bytes: text.len() }
                    }
                    PayloadV2::ToolStart { tool_call_id, name, .. } => {
                        ui.note_tool_started(tool_call_id, name);
                        FlowEvent::ToolStarted {
                            tool_call_id: tool_call_id.clone(),
                            name: name.clone(),
                        }
                    }
                    PayloadV2::ToolEnd { tool_call_id, status, .. } => {
                        let ok = matches!(
                            status,
                            octos_core::ui_protocol::EnvelopeToolEndStatus::Complete
                        );
                        ui.note_tool_completed(tool_call_id, tool_call_id, ok, None);
                        FlowEvent::ToolCompleted { tool_call_id: tool_call_id.clone(), ok }
                    }
                    PayloadV2::TurnTerminal { outcome, error, .. } => match outcome {
                        TurnTerminalOutcome::Completed => {
                            ui.end_turn(&turn_id, true);
                            FlowEvent::TurnEnded { turn_id, error: None }
                        }
                        other => {
                            ui.end_turn(&turn_id, false);
                            let label = format!("{other:?}");
                            FlowEvent::TurnEnded {
                                turn_id,
                                error: Some(
                                    error
                                        .as_ref()
                                        .map(|e| format!("{}: {}", e.code, e.message))
                                        .unwrap_or(label),
                                ),
                            }
                        }
                    },
                    other => FlowEvent::Other(format!("envelope:{other:?}").chars().take(48).collect()),
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

    /// End the live turn (test support; production uses `end_turn`). Ends
    /// whichever turn is live, by its own id, so the id gate is satisfied.
    pub fn end_turn_now(&mut self, ok: bool) {
        let id = self.active_turn.as_ref().map(|(id, _)| id.clone()).unwrap_or_default();
        self.end_turn(&id, ok);
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

    /// A turn becomes live. A `turn/started` names the turn, so the id is
    /// authoritative — a new turn supersedes whatever was live.
    fn begin_turn(&mut self, turn_id: &str, _started: Instant) {
        self.active_turn = Some((turn_id.to_owned(), Instant::now()));
    }

    /// A delta arrived for `turn_id`. If no turn is live yet (a delta can beat
    /// `turn/started`), this turn becomes live. A delta for a DIFFERENT turn
    /// never hijacks the live one — the live L1/L2 defect was a stale frame
    /// clearing/replacing a running turn (LESSONS 5).
    fn touch_turn(&mut self, turn_id: &str) {
        match &self.active_turn {
            None => self.active_turn = Some((turn_id.to_owned(), Instant::now())),
            Some((id, _)) if id == turn_id => {}
            Some(_) => {}
        }
    }

    /// A terminal arrived for `turn_id`. **Gated by id**: a terminal for a turn
    /// other than the live one is ignored, so a late/stale terminal can never
    /// clear a running turn — which is exactly how the live `×` click stopped
    /// sending `turn/interrupt` (L1): the running turn's `active_turn` was
    /// cleared by a different turn's terminal, so `turn.interrupt` resolved to
    /// `Unhandled`.
    fn end_turn(&mut self, turn_id: &str, _ok: bool) {
        match &self.active_turn {
            Some((id, started)) if id == turn_id => {
                let started = *started;
                self.last_worked = Some(started.elapsed());
                self.active_turn = None;
            }
            // A terminal for another turn, or no live turn: do not touch the
            // live turn's state.
            _ => return,
        }
        self.last_completed_at = Some(std::time::SystemTime::now());
        self.approval_pending = false;
        self.question_pending = false;
    }
}

/// The params half of an inbound frame, for the JSONL trace. The transport
/// hands us typed notifications, so we serialize the decoded payload —
/// exactly the shape a replay test needs.
fn trace_params(evt: &TransportEvent) -> serde_json::Value {
    fn with_cursor(mut v: serde_json::Value, cursor: &Option<octos_core::ui_protocol::UiCursor>) -> serde_json::Value {
        if let (Some(c), serde_json::Value::Object(m)) = (cursor, &mut v) {
            m.insert(
                "cursor".to_owned(),
                serde_json::json!({"stream": c.stream, "seq": c.seq}),
            );
        }
        v
    }
    match evt {
        TransportEvent::DurableNotification { payload, cursor } => with_cursor(
            octoscode_client::trace::wire_params(payload),
            cursor,
        ),
        TransportEvent::EphemeralNotification { payload } => {
            octoscode_client::trace::wire_params(payload)
        }
        TransportEvent::RpcResult(LifecycleResult::SessionOpen(r)) => serde_json::json!({
            "session_id": r.opened.session_id.0,
            "cursor": r.opened.cursor.as_ref().map(|c| serde_json::json!({"stream": c.stream, "seq": c.seq})),
            "capabilities": r.opened.capabilities,
        }),
        TransportEvent::ConnectionState(s) => serde_json::json!({"state": format!("{s:?}")}),
        TransportEvent::CapabilityNegotiated(caps) => {
            serde_json::json!({"accepted": caps.raw.keys().cloned().collect::<Vec<_>>()})
        }
        TransportEvent::SessionsListed { sessions } => sessions.clone(),
        TransportEvent::SessionHydrated { session_id, result } => {
            serde_json::json!({"session_id": session_id, "result": result})
        }
        TransportEvent::RpcError { method, error, .. } => {
            serde_json::json!({"method": method, "code": error.code, "message": error.message})
        }
        other => serde_json::json!({ "debug": format!("{other:?}") }),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Card #14 defect 4: a new chat gets a FRESH id (never the reused
    /// `<profile>:main`), profile-scoped like the web's
    /// `bindWebSessionIdToProfile` (`session-identity.ts:23`).
    #[test]
    fn defect4_a_new_chat_mints_a_fresh_profile_scoped_id() {
        let a = Conversation::fresh_session_id_for("dsflash");
        let b = Conversation::fresh_session_id_for("dsflash");
        assert_ne!(a, b, "two new chats must not share an id");
        assert_ne!(a, "dsflash:main", "a new chat must not reuse the fixed session");
        assert!(
            a.starts_with("dsflash:"),
            "the id is profile-scoped, like the web's bindWebSessionIdToProfile; got {a:?}"
        );
    }
}
