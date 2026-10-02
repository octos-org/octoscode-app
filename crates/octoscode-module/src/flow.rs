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
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use octos_app_transport::{
    LifecycleResult, OutboundCommand, ProfileId, SecretString, TransportConfig, TransportEvent,
};
use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::{SessionOpenParams, TurnId};
use octoscode_client::{Client, ClientError, Registry};
use octoscode_store::Store;
use url::Url;

// A7 — the turn controller (queue / steer / recovery / reconcile) on this
// conversation; a child module so it reads the private plumbing.
#[path = "flow_controller.rs"]
mod controller;
pub use controller::hydrated_turns;
// A12 — the transport link: one stable command/event channel over a WS
// transport that can be replaced (give-up, "Retry now") under the SAME
// conversation.
#[path = "flow_link.rs"]
pub mod link;

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

/// One turn's own terminal result (**card #21j**).
///
/// The wire's `turn_terminal` carries `outcome` / `error` / `token_usage` and
/// **no duration** (`ui_protocol.rs:4006-4012`), so `worked` / `completed_at`
/// are measurements only the party that sent `turn/start` has. Kept **per
/// turn** so a later turn's terminal can never change an earlier turn's row.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TurnEnd {
    /// `completed` / `errored` / `interrupted` / `rate_limited`, from that
    /// turn's own `turn_terminal`.
    pub outcome: String,
    /// Wall-clock duration, measured locally between `turn/started` and the
    /// terminal. `None` until the turn settled while it was the live one.
    pub worked: Option<Duration>,
    /// When the terminal landed (locally), for `answer.timestamp`.
    pub completed_at: Option<SystemTime>,
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
    /// **Per-turn** terminal results, keyed by turn id (card #21j). A later
    /// turn's terminal can never change an earlier turn's settled row, which is
    /// the defect the gate showed: turn 1 (completed) rendered "Interrupted"
    /// once turn 2's terminal landed.
    turn_ends: HashMap<String, TurnEnd>,
    /// The turn whose terminal most recently settled. The **global** `answer.*`
    /// bindings project this turn; the timeline's settled rows are per-turn and
    /// carry their own turn id instead (`screen::Row::turn`).
    last_settled_turn: Option<String>,
    /// `approval.pending` — an `approval/requested` is outstanding.
    approval_pending: bool,
    /// `question.pending` — a `user_question/requested` is outstanding.
    question_pending: bool,
    /// `tools[].expanded` — which tool rows the person disclosed. UI-local:
    /// the card toggles it, the store never sees it (`bindings.json` note).
    expanded: Vec<String>,
    /// `answer.expand` state (the "worked for" disclosure).
    answer_expanded: bool,
    /// A1 — the settled turns whose tool group the person FOLDED under its
    /// "Worked for" header (UI-local, per turn; absent = shown, the web's
    /// always-visible tool headers).
    folded_turns: Vec<String>,
    /// Card #28e: UI-local chrome toggles for board 4. All UI-local — the
    /// store/protocol never sees them (the `tools[].expanded` precedent).
    review_open: bool,
    settings_open: bool,
    palette_open: bool,
    /// A7 — the code block whose Copy was pressed: (answer row key, block
    /// index, when). Shows "Copied" for one second (`CodeBlock.tsx:72-80`).
    code_copied: Option<(String, usize, Instant)>,
    /// A7 — not-sent / interrupted text waiting for its composer to empty.
    parked_restores: Vec<(String, String)>,
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
    ///
    /// Only the LIVE path renders this row (`screen.rs:113-116`), so it never
    /// carries a terminal marker; the settled interrupted turn shows its marker
    /// through `answer.worked_for` instead.
    pub fn turn_activity(&self) -> String {
        match &self.active_turn {
            Some((_, started)) => format!("Working · {}s", started.elapsed().as_secs()),
            None => String::new(),
        }
    }

    /// Record a terminal outcome against the turn it names (**card #21j**).
    ///
    /// Per-turn, not session-level: a later turn's terminal can never rewrite an
    /// earlier turn's settled row (the #21e gate on the *live* id recorded only
    /// one global outcome, so turn 2's `interrupted` restamped turn 1). The
    /// LIVE-turn bookkeeping (`active_turn`, the measured duration) still lives
    /// in [`Self::end_turn`], which stays gated by id so a stale terminal can
    /// never stop a running turn (the L1 lesson, `f21c_live.rs`).
    pub fn note_outcome(&mut self, turn_id: &str, outcome: &str) {
        self.turn_ends
            .entry(turn_id.to_owned())
            .or_default()
            .outcome = outcome.to_owned();
        // The GLOBAL `answer.*` projection follows the *live* turn's terminal, so
        // a stale terminal for an already-settled turn cannot relabel the current
        // row (card #21e's gate; `f21c_live.rs`). Per-turn rows read their own
        // `turn_ends` entry regardless, which is the whole of card #21j.
        if matches!(&self.active_turn, Some((id, _)) if id == turn_id) {
            self.last_settled_turn = Some(turn_id.to_owned());
        }
    }

    /// The settled result of one turn, if that turn has a terminal yet.
    pub fn turn_end(&self, turn_id: &str) -> Option<&TurnEnd> {
        self.turn_ends.get(turn_id)
    }

    /// The label for ONE turn's `worked-for` row (**card #21j**) — that turn's
    /// own terminal, never another's.
    ///
    /// Card #21d item 4 / #21e item 1: the atlas shows the row as a small grey
    /// disclosure with a trailing chevron (`design/components/worked-for/
    /// page.card` — `Worked for 3m 4s ›`), and the `›` is the affordance that
    /// says the row toggles. A turn that did not complete cleanly shows the
    /// terminal marker instead, because a stopped/failed turn has no duration
    /// worth reporting: `interrupted` → `Interrupted` (the web's terminal note,
    /// `timeline/model.ts:262-264`), `errored` → `Failed`, `rate_limited` →
    /// `Rate limited`. No atlas art exists for the error/limited labels (the
    /// design fixtures only draw the success row), so the wording mirrors the
    /// web's `Turn failed` / `Turn rate limited` system titles
    /// (`timeline/model.ts:766-770`) in the row's one-word style.
    ///
    /// `turn = None` (the global `answer.worked_for` binding) projects the most
    /// recently **settled** turn.
    pub fn worked_for_for(&self, turn: Option<&str>) -> String {
        let end = match turn {
            Some(t) => self.turn_ends.get(t),
            None => self
                .last_settled_turn
                .as_deref()
                .and_then(|t| self.turn_ends.get(t)),
        };
        let Some(end) = end else { return String::new() };
        match end.outcome.as_str() {
            "interrupted" => return "Interrupted".to_owned(),
            "errored" => return "Failed".to_owned(),
            "rate_limited" => return "Rate limited".to_owned(),
            _ => {}
        }
        match end.worked {
            Some(d) => {
                let secs = d.as_secs();
                if secs >= 60 {
                    format!("Worked for {}m {}s ›", secs / 60, secs % 60)
                } else {
                    format!("Worked for {secs}s ›")
                }
            }
            None => String::new(),
        }
    }

    /// `answer.worked_for` — the most recently settled turn's label (the global
    /// binding; a timeline row uses [`Self::worked_for_for`] with its own turn).
    pub fn worked_for(&self) -> String {
        self.worked_for_for(None)
    }

    /// `answer.timestamp` — the last turn's completion, as a display label.
    ///
    /// Card #21d item 4: the app showed the raw epoch (`t=1790660000`). The
    /// atlas (`design/components/answer-actions/page.card:6`) writes
    /// `Sep 28, 9:41 PM`, and the web's `formatRelativeTime`
    /// (`features/shell/relative-time.ts:1-18`) returns `now` / `5m` / `3h` /
    /// `2d` for anything under a week, else a `Mon D` date. Mirror both: a fresh
    /// turn reads `now`, an older one the atlas-shaped `Sep 28, 9:41 PM`.
    pub fn answer_timestamp(&self) -> String {
        self.answer_timestamp_for(None)
    }

    /// The completion timestamp for ONE turn (**card #21j**), or the most
    /// recently settled turn for `None` — the per-turn companion of
    /// [`Self::worked_for_for`], so the settled tail of an earlier turn does
    /// not re-label itself when a later turn completes.
    pub fn answer_timestamp_for(&self, turn: Option<&str>) -> String {
        let end = match turn {
            Some(t) => self.turn_ends.get(t),
            None => self
                .last_settled_turn
                .as_deref()
                .and_then(|t| self.turn_ends.get(t)),
        };
        end.and_then(|e| e.completed_at)
            .map(|t| format_completed_at(t, SystemTime::now()))
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

    /// A1 — is `turn`'s tool group folded under its "Worked for" header?
    pub fn is_turn_folded(&self, turn: &str) -> bool {
        self.folded_turns.iter().any(|t| t == turn)
    }

    /// A1 — flip `turn`'s tool-group fold (the worked-for row's click);
    /// returns true when the group is now folded.
    pub fn toggle_turn_fold(&mut self, turn: &str) -> bool {
        if let Some(at) = self.folded_turns.iter().position(|t| t == turn) {
            self.folded_turns.remove(at);
            false
        } else {
            self.folded_turns.push(turn.to_owned());
            true
        }
    }

    /// A1 — every folded turn (the row model skips their tool rows).
    pub fn folded_turns(&self) -> Vec<String> {
        self.folded_turns.clone()
    }

    /// A6 — replace the folded turns (Expand all opens every tool group).
    pub fn set_folded_turns(&mut self, turns: Vec<String>) {
        self.folded_turns = turns;
    }

    /// A6 — the disclosed tool rows' keys (the fold memory, `folds.ts`).
    pub fn expanded_keys(&self) -> Vec<String> {
        self.expanded.clone()
    }

    /// A6 — replace the disclosed tool rows (Expand all / Collapse all /
    /// pruning, `folds.ts:22-44`).
    pub fn set_expanded_keys(&mut self, keys: Vec<String>) {
        self.expanded = keys;
    }

    /// Card #28e — UI-local chrome state (board 4). All three are toggles the
    /// view reads on every redraw; none reaches the protocol.
    pub fn review_open(&self) -> bool {
        self.review_open
    }

    pub fn toggle_review(&mut self) -> bool {
        self.review_open = !self.review_open;
        self.review_open
    }

    pub fn settings_open(&self) -> bool {
        self.settings_open
    }

    pub fn toggle_settings(&mut self) -> bool {
        self.settings_open = !self.settings_open;
        self.settings_open
    }

    pub fn palette_open(&self) -> bool {
        self.palette_open
    }

    pub fn set_palette_open(&mut self, open: bool) {
        self.palette_open = open;
    }

    pub fn toggle_palette(&mut self) -> bool {
        self.palette_open = !self.palette_open;
        self.palette_open
    }
    pub fn toggle_answer_expanded(&mut self) -> bool {
        self.answer_expanded = !self.answer_expanded;
        self.answer_expanded
    }

    pub fn approval_pending(&self) -> bool {
        self.approval_pending
    }

    /// A7 — how long a code block's Copy control reads "Copied"
    /// (`CodeBlock.tsx:75-79`: a 1 000 ms reset timer).
    pub const CODE_COPIED_FOR: Duration = Duration::from_millis(1_000);

    /// A7 — a turn that never started (a collision, a rejection) or whose
    /// wait was released: clear the live turn without recording a terminal
    /// result (no "Worked for", no outcome marker).
    pub fn abandon_turn(&mut self, turn_id: &str) {
        if matches!(&self.active_turn, Some((id, _)) if id == turn_id) {
            self.active_turn = None;
        }
    }

    /// A7 — park text handed back while the owning composer was busy; it
    /// returns when that composer is empty (`restoreUnsentTurn`).
    pub fn park_restore(&mut self, session: &str, text: &str) {
        if !text.trim().is_empty() {
            self.parked_restores.push((session.to_owned(), text.to_owned()));
        }
    }

    /// A7 — the oldest parked text for `session`, once.
    pub fn take_parked_restore(&mut self, session: &str) -> Option<String> {
        let i = self.parked_restores.iter().position(|(s, _)| s == session)?;
        Some(self.parked_restores.remove(i).1)
    }

    /// A7 — record a code block's copy (`row` = the answer row's key).
    pub fn note_code_copied(&mut self, row: &str, block: usize) {
        self.code_copied = Some((row.to_owned(), block, Instant::now()));
    }

    /// A7 — the block of `row` whose Copy was pressed less than a second ago.
    pub fn code_copied(&self, row: &str) -> Option<usize> {
        match &self.code_copied {
            Some((r, k, at)) if r == row && at.elapsed() < Self::CODE_COPIED_FOR => Some(*k),
            _ => None,
        }
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

/// A8 — the web's `SessionRuntimeScope` (`session-scope.ts:9-28`): a Session
/// is NOT identified by its id alone — the endpoint, the workspace root and
/// the profile are part of the key, and an opaque AUTHORITY EPOCH tells apart
/// the auth identity behind a connection (a new connection = a new epoch;
/// never a credential, so keys and logs stay credential-free).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionScope {
    pub endpoint: String,
    pub workspace_root: String,
    pub profile_id: String,
    pub session_id: String,
    pub authority_epoch: u64,
}

impl SessionScope {
    /// `sessionRuntimeScopeKey`: a JSON tuple of the trimmed parts (a `::`
    /// delimiter could collide; a JSON array cannot).
    pub fn key(&self) -> String {
        serde_json::json!([
            self.endpoint.trim(),
            self.workspace_root.trim(),
            self.profile_id.trim(),
            self.session_id.trim(),
            self.authority_epoch,
        ])
        .to_string()
    }
}

/// Bumped by every `Conversation::connect` (each connection is a new auth
/// identity behind its endpoint).
static AUTHORITY_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A8 — the turn ids THIS client dispatched (`turn/start`), newest last,
/// bounded. The strip's "Another client is working in this session" word
/// is a live turn this client never sent (the web's
/// `queue.active.origin === "adopted"`, `App.tsx:2063-2072`).
static OWN_TURNS: Mutex<Vec<String>> = Mutex::new(Vec::new());
const OWN_TURNS_MAX: usize = 64;

/// Record a turn id this client dispatched.
pub fn note_own_turn(turn_id: &str) {
    let mut g = OWN_TURNS.lock().unwrap_or_else(|p| p.into_inner());
    if !g.iter().any(|t| t == turn_id) {
        g.push(turn_id.to_owned());
        let n = g.len();
        if n > OWN_TURNS_MAX {
            g.drain(..n - OWN_TURNS_MAX);
        }
    }
}

/// Whether this client dispatched `turn_id`.
pub fn is_own_turn(turn_id: &str) -> bool {
    OWN_TURNS.lock().unwrap_or_else(|p| p.into_inner()).iter().any(|t| t == turn_id)
}

/// Test seam.
pub fn forget_own_turns() {
    OWN_TURNS.lock().unwrap_or_else(|p| p.into_inner()).clear();
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
    /// #32h: mutable AFTER connect — the phone has no OCTOS_PROFILE_ID, so
    /// the baked fallback ("octoscode") may not exist server-side and every
    /// session/open dies with -32120 "agent is outside the requested
    /// profile scope" (fixture: the server's active profile is
    /// `<name>-<pid>`, what `profile/local/create` mints). connect_now
    /// ensures + adopts the real id (adopt_profile) before the first turn.
    profile: Mutex<String>,
    /// Card #14 defect 4: mutable, so a New chat adopts a fresh id and a resume
    /// adopts a listed one. Read through [`Conversation::session_id`].
    session_id: Mutex<String>,
    /// #32h: set by open_workspace_as — submit must never target an
    /// un-opened session (the web's first message creates the thread).
    workspace_opened: Mutex<bool>,
    /// #P4g1 row 204: the cwd the in-flight `session/open` requested, so the
    /// reply arm can compare it with the server's returned
    /// `workspace_root` (the web's `requireExactWorkspace` resume path,
    /// `candidate-session.ts:230-243`).
    pending_open_cwd: Mutex<Option<String>>,
    /// A8 — this connection's authority epoch (the scope key's last part).
    epoch: u64,
    /// A8 — the transport has EVER been Live on this connection: a later
    /// handshake is a RECONNECT, which re-opens the active Session and
    /// hydrates it under a new authority generation
    /// (`active-session-runtime.ts`: connect -> open -> hydrate -> ready,
    /// every reconnect a new generation). Never reset (A7's `was_live` is the
    /// last transition's, reset by a drop).
    ever_live: Mutex<bool>,
    /// A8 — the generation (`open_seq`) each in-flight `session/hydrate` was
    /// requested under, by session: a reply from a retired generation is a
    /// stale authority and fails closed. A15: a QUEUE per session, in send
    /// order — every open now hydrates, so an open, a resync and a second
    /// open can each have one in flight; the transport's reply names only the
    /// session, and one socket answers them in order. A re-dialed socket
    /// clears it (the dropped socket's requests are never answered).
    hydrate_gen: Mutex<HashMap<String, std::collections::VecDeque<u64>>>,
    /// A8 — the new-session defaults armed by `new_chat` for the FRESH id
    /// only; the open reply for exactly that id takes them once
    /// (`App.tsx:649` `appliedDefaultsForSession`).
    creation_defaults: Arc<crate::screens::session_defaults::Pending>,
    /// #P4e1b row 4: bumped by every `session/open`, so each open presents a
    /// NEW commands identity to the autonomy fence and retires the previous
    /// one (web `autonomy/store.ts:208-213`: a new object for the same
    /// session id, a re-auth, a reconnect or a Core restart all reset ALL
    /// data, watermarks, revisions and busy holders). Starts at 0 and is
    /// bumped BEFORE the open is sent, so the reply arm reads the new value.
    open_seq: Arc<Mutex<u64>>,
    started: Instant,
    /// A4 — the HTTP side of the same server (the web's `mediaCommands`:
    /// `/api/upload`, `/api/files`, `packages/client/src/media.ts:14-35`) and
    /// the credential the socket carries. Never logged.
    http_base: String,
    bearer: String,
    /// A7 — the shared handle (set by [`Conversation::attach`]) so a transport
    /// event can start the next queued prompt on the runtime.
    weak_self: Mutex<std::sync::Weak<Conversation>>,
    /// A7 — queue heads to start when no shared handle is attached yet
    /// (drained by [`Conversation::pump`]).
    pending_starts: Mutex<Vec<octoscode_store::domains::composer::PromptTurn>>,
    /// A7 — the connection was live before the last transition (a drop
    /// suspends the transport generation; the next Live reconciles).
    was_live: Mutex<bool>,
    transport_suspended: Mutex<bool>,
    /// A12 — the transport link (stable channels over a replaceable
    /// transport; outage bookkeeping for the requests).
    link: Arc<link::Link>,
    /// A12 — the outage's attempt count before the current transport
    /// started (each transport counts its own re-dials from 1).
    attempt_base: Mutex<u32>,
    /// A15 — the last turn edge (`<turn>:start|end`) that re-listed the
    /// catalog (a turn's end arrives both bare and as an envelope).
    catalog_edge: Mutex<Option<String>>,
    /// A19 — every accepted open is remembered for this server (the web's
    /// tab state, `App.tsx:976-992`; `screens::remembered`). Off unless the
    /// host turns it on, so a test conversation never writes the file.
    remember_opens: Mutex<bool>,
    /// A19 — the next `session/open`'s outcome, for a caller that must know
    /// it (a restore the server refuses falls back to a fresh launch, the
    /// web's `restoreRejected`, `use-octos-session.ts:2988-3000`).
    open_watch: Mutex<Option<tokio::sync::oneshot::Sender<Result<String, String>>>>,
}

/// A4 — the HTTP origin for the media endpoints (`media.ts:14-35`): `ws` ->
/// `http`, `wss` -> `https`, no query, and a socket path
/// (`…/api/ui-protocol/ws`) or `/` reduced to the origin prefix; no trailing
/// slash.
pub fn http_base_of(base: &str) -> String {
    let Ok(mut u) = Url::parse(base) else { return base.trim_end_matches('/').to_owned() };
    let scheme = match u.scheme() {
        "ws" => "http",
        "wss" => "https",
        other => other,
    }
    .to_owned();
    let _ = u.set_scheme(&scheme);
    u.set_query(None);
    u.set_fragment(None);
    let path = u.path().trim_end_matches('/').to_owned();
    let path = path.strip_suffix("/api/ui-protocol/ws").unwrap_or(&path).to_owned();
    u.set_path(&path);
    u.to_string().trim_end_matches('/').to_owned()
}

impl Conversation {
    /// Build a conversation and spawn its transport. `waker` wakes the UI
    /// thread (makepad's `SignalToUI`) after each event is queued.
    ///
    /// The base URL carries the web's `ui_feature=` query params
    /// ([`octoscode_client::features`]) — our transport clones `base_url`
    /// verbatim, so they reach the socket as the web sends them.
        /// #32h: discover a REAL profile id before the WS upgrade. The server
    /// accepts any `X-Profile-Id` on the socket, but the session is scoped
    /// to that header — a non-existent profile never answers session/open
    /// (the silent submit: the baked "octoscode" does not exist). The solo
    /// login (POST api/auth/solo — anonymous, the web's "Use local solo
    /// server" flow) returns the server's local user whose id IS a verified
    /// top-level profile id (`resolve_solo_user` keeps only users passing
    /// `is_top_level_profile_id` — solo_auth.rs:127, auth_handlers.rs:59).
    /// None = discovery unavailable; the caller falls back and the ensure
    /// chain still guards the session.
    pub async fn discover_solo_profile(base: &str) -> Option<String> {
        // Step 1: the solo login (anonymous) mints an admin session token.
        let url = format!("{}/api/auth/solo", base.trim_end_matches('/'));
        let call = reqwest::Client::new()
            .post(&url)
            .json(&serde_json::json!({}))
            .send();
        let resp = tokio::time::timeout(std::time::Duration::from_secs(15), call)
            .await
            .ok()?
            .ok()?;
        let body: serde_json::Value = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            resp.json(),
        )
        .await
        .ok()?
        .ok()?;
        let token = body.get("token")?.as_str()?.to_owned();
        let solo_id = body.get("user")?.get("id")?.as_str()?.to_owned();
        if solo_id.is_empty() {
            return None;
        }
        // Step 2 (the outer loop's device finding): an EXISTING profile can
        // still have NO runtime — turn/start dies with -32603 "No
        // ProfileRuntime registered" (the device: 'octoscode-desktop' had no
        // model; 'dsflash' carries config.llm.primary). Rank the admin list:
        // a profile whose llm config has a primary rides first; the solo
        // id (which certainly exists) is the fallback.
        let list_url = format!("{}/api/admin/profiles", base.trim_end_matches('/'));
        let call = reqwest::Client::new()
            .get(&list_url)
            .header("Authorization", format!("Bearer {token}"))
            .send();
        let list: Option<serde_json::Value> =
            match tokio::time::timeout(std::time::Duration::from_secs(15), call).await {
                Ok(Ok(resp)) => tokio::time::timeout(
                    std::time::Duration::from_secs(15),
                    resp.json::<serde_json::Value>(),
                )
                .await
                .ok()
                .and_then(|r| r.ok()),
                _ => None,
            };
        if let Some(rows) = list.as_ref().and_then(|v| v.as_array()) {
            let runnable = rows.iter().find_map(|row| {
                let id = row.get("id")?.as_str()?;
                row.pointer("/config/llm/primary")
                    .is_some()
                    .then(|| id.to_owned())
            });
            if let Some(id) = runnable {
                return Some(id);
            }
        }
        Some(solo_id)
    }

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
        // A12 — the link owns the transport (and replaces it when it gives
        // up); the app holds its stable channels.
        let (link, cmd_tx, evt_rx) = link::Link::start(cfg, waker);
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
                profile: Mutex::new(profile.to_owned()),
                session_id: Mutex::new(format!("{profile}:main")),
                workspace_opened: Mutex::new(false),
                pending_open_cwd: Mutex::new(None),
                creation_defaults: crate::screens::session_defaults::Pending::new(),
                epoch: AUTHORITY_EPOCH.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1,
                ever_live: Mutex::new(false),
                hydrate_gen: Mutex::new(HashMap::new()),
                open_seq: Arc::new(Mutex::new(0)),
                started: Instant::now(),
                http_base: http_base_of(base),
                bearer: bearer.to_owned(),
                weak_self: Mutex::new(std::sync::Weak::new()),
                pending_starts: Mutex::new(Vec::new()),
                was_live: Mutex::new(false),
                transport_suspended: Mutex::new(false),
                link,
                attempt_base: Mutex::new(0),
                catalog_edge: Mutex::new(None),
                remember_opens: Mutex::new(false),
                open_watch: Mutex::new(None),
            },
            evt_rx,
        ))
    }

    /// A19 — remember every accepted open for this server
    /// ([`crate::screens::remembered::note_opened`]).
    pub fn remember_opens(&self, on: bool) {
        *self.remember_opens.lock().unwrap() = on;
    }

    /// A19 — the outcome of the NEXT `session/open` this connection sends:
    /// `Ok(session)` once the server's answer is adopted, `Err(reason)` when
    /// it refuses (an RPC error, or another workspace than requested).
    pub fn watch_next_open(&self) -> tokio::sync::oneshot::Receiver<Result<String, String>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        *self.open_watch.lock().unwrap() = Some(tx);
        rx
    }

    fn settle_open_watch(&self, outcome: Result<String, String>) {
        if let Some(tx) = self.open_watch.lock().unwrap().take() {
            let _ = tx.send(outcome);
        }
    }

    /// A4 — `POST <base>/api/upload` (multipart, field `file`, one file per
    /// request; `packages/client/src/media.ts:114-146`): returns the single
    /// upload handle the server answers with. Headers as the web sends them:
    /// `X-Profile-Id`, plus `Authorization: Bearer` when a token exists.
    pub async fn upload_file(&self, name: &str, mime: &str, bytes: Vec<u8>) -> Result<String, String> {
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(name.to_owned())
            .mime_str(mime)
            .map_err(|e| format!("upload: {e}"))?;
        let form = reqwest::multipart::Form::new().part("file", part);
        let mut req = reqwest::Client::new()
            .post(format!("{}/api/upload", self.http_base))
            .header("X-Profile-Id", self.profile())
            .multipart(form);
        if !self.bearer.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.bearer));
        }
        let resp = req.send().await.map_err(|_| {
            "Authenticated file transfer failed; check the connection and server file permissions".to_owned()
        })?;
        if !resp.status().is_success() {
            return Err(format!("Upload was not confirmed ({}).", resp.status().as_u16()));
        }
        let v: serde_json::Value = resp.json().await.map_err(|_| "Invalid attachment receipt".to_owned())?;
        match v.as_array().map(|a| a.as_slice()) {
            Some([serde_json::Value::String(handle)]) => Ok(handle.clone()),
            _ => Err("Invalid attachment receipt".to_owned()),
        }
    }

    /// A4 — `GET <base>/api/files?path=<reference>&session=<session>` (the
    /// web's delivered-file download, `media.ts:147-165`).
    pub async fn download_file(&self, reference: &str) -> Result<Vec<u8>, String> {
        if reference.trim().is_empty() {
            return Err("Invalid file reference".into());
        }
        let mut req = reqwest::Client::new()
            .get(format!("{}/api/files", self.http_base))
            .query(&[("path", reference), ("session", &self.session_id())])
            .header("X-Profile-Id", self.profile());
        if !self.bearer.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.bearer));
        }
        let resp = req.send().await.map_err(|_| {
            "Authenticated file transfer failed; check the connection and server file permissions".to_owned()
        })?;
        if !resp.status().is_success() {
            return Err("This file could not be loaded.".into());
        }
        resp.bytes().await.map(|b| b.to_vec()).map_err(|_| "This file could not be loaded.".into())
    }

    pub fn profile(&self) -> String {
        self.profile.lock().unwrap().clone()
    }

    /// A8 — the server's HTTP origin (the drafts' principal read, REST).
    pub fn http_base(&self) -> String {
        self.http_base.clone()
    }

    /// A8 — the connection's credential, for the one REST read that needs it
    /// (`/api/auth/me`). Never logged.
    pub(crate) fn bearer(&self) -> String {
        self.bearer.clone()
    }

    /// #P4e1b row 4: the commands identity this connection currently
    /// presents to the autonomy fence. The `open_seq` counter makes every
    /// `session/open` a NEW identity for the same session id — which is
    /// exactly the case the web treats as an authority reset
    /// (`autonomy/store.ts:208-213`).
    pub fn identity(&self) -> String {
        let seq = *self.open_seq.lock().unwrap();
        format!("{}#{}", self.profile(), seq)
    }

    /// A8 — the active Session's runtime scope (endpoint, workspace root,
    /// profile, session, authority epoch).
    pub fn scope(&self) -> SessionScope {
        let session_id = self.session_id();
        SessionScope {
            endpoint: self.http_base.clone(),
            workspace_root: self.store.domains.session.workspace_root(&session_id).unwrap_or_default(),
            profile_id: self.profile(),
            session_id,
            authority_epoch: self.epoch,
        }
    }

    /// A8 — `scope().key()`: async results captured under one key are
    /// dropped when the key changed before they landed.
    pub fn scope_key(&self) -> String {
        self.scope().key()
    }

    /// A8 — the current authority generation (bumped by every open and every
    /// reconnect re-open).
    pub fn generation(&self) -> u64 {
        *self.open_seq.lock().unwrap()
    }

    /// A8 — ask for the canonical `session/hydrate` of `session` under the
    /// CURRENT generation (the reply arm refuses it once the generation moved).
    pub fn request_hydrate(&self, session: &str) -> bool {
        let gen = self.generation();
        self.hydrate_gen.lock().unwrap().entry(session.to_owned()).or_default().push_back(gen);
        match self.cmd_tx.try_send(OutboundCommand::HydrateSession { session_id: session.to_owned() }) {
            Ok(()) => {
                // A15 — the frame the transport writes (`proto.rs`
                // `HydrateSession`: include messages), on the trace too.
                self.frames
                    .out("session/hydrate", &serde_json::json!({"session_id": session, "include": ["messages"]}));
                true
            }
            Err(e) => {
                if let Some(q) = self.hydrate_gen.lock().unwrap().get_mut(session) {
                    q.pop_back();
                }
                ::log::warn!("octoscode: session/hydrate send failed for {session}: {e}");
                false
            }
        }
    }

    /// #32h: take the server-verified profile id (`profile/local/create`'s
    /// `profile_id`) and re-prefix the not-yet-opened session id, so the
    /// next `session/open` / `turn/start` carries a profile that EXISTS.
    pub fn adopt_profile(&self, id: String) {
        *self.profile.lock().unwrap() = id.clone();
        let mut sid = self.session_id.lock().unwrap();
        if sid.ends_with(":main") {
            *sid = format!("{id}:main");
        }
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
        // A19 — a fresh connection carries no profile id; the requested id
        // keeps the old built-in base then.
        let base = Some(self.profile()).filter(|p| !p.trim().is_empty()).unwrap_or_else(|| "octoscode".to_owned());
        let id = format!("{base}-{}", std::process::id());
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
        self.open_workspace_with(id, cwd, None).await
    }

    /// The open with an optional session-scoped sandbox (A8: a CREATED
    /// session's new-session default, `App.tsx:1922-1932`; a re-open never
    /// carries one).
    pub async fn open_workspace_with(
        &self,
        id: &str,
        cwd: Option<String>,
        sandbox: Option<octos_core::ui_protocol::SessionSandboxParams>,
    ) -> Result<String, String> {
        // A12 — no Session change while the connection is down: the open would
        // be dropped at the link, yet the window would switch to a Session the
        // server never opened (and the reconnect would re-open THAT one).
        if self.in_outage() {
            makepad_widgets::log!("[octoscode] session/open {id} refused while reconnecting");
            return Err(link::NOT_CONNECTED.to_owned());
        }
        let session_id = octos_core::SessionKey(id.to_owned());
        // #P4e1b row 4: retire the previous commands identity BEFORE the open
        // goes out, so the reply arm binds the NEW one and any result captured
        // under the old identity is refused from the moment it is issued.
        {
            let mut seq = self.open_seq.lock().unwrap();
            *seq += 1;
        }
        // #P4g1 row 204: remember what THIS open asked for, so the reply arm
        // can fail closed on a different returned workspace.
        *self.pending_open_cwd.lock().unwrap() = cwd.clone();
        // Record the outbound frame BEFORE `cwd` moves into the params.
        self.frames.out(
            "session/open",
            &serde_json::json!({
                "session_id": session_id.0,
                "profile_id": self.profile(),
                "cwd": cwd,
                "sandbox": sandbox,
            }),
        );
        let params = SessionOpenParams {
            session_id: session_id.clone(),
            topic: None,
            profile_id: Some(self.profile()),
            cwd,
            sandbox,
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
        // #34b — seed the opened session NOW, synchronously: the web's
        // tab-known registry knows the session the moment the tab is created
        // (known-session-registry), so a session/list reply that lags the tab
        // (the #39a row-2 gate behavior) can neither drop it nor dangle the
        // active id. The session/open RpcResult arm re-notes idempotently.
        self.store.note_session_opened(&session_id.0, None);
        self.store.set_active(Some(session_id.0.clone()));
        if let Err(e) = self.refresh_sessions().await {
            ::log::warn!("octoscode: session/list after open: {e}");
        }
        *self.workspace_opened.lock().unwrap() = true;
        ::log::info!("octoscode: workspace open requested for {}", session_id.0);
        Ok(session_id.0)
    }

    /// `turn/start` — the web's generic request (`client.ts:488`), not a typed
    /// command. Returns the turn id it generated (UUID v7, as the web's
    /// `turn.turnId`). Delegates to [`Conversation::start_turn_with_id`] with a
    /// freshly minted id.
    pub async fn start_turn(&self, text: impl Into<String>) -> Result<String, ClientError> {
        let turn_id = TurnId::new().0.to_string();
        self.start_turn_with_id(text, turn_id).await
    }

    /// `turn/start` carrying PRE-UPLOADED attachments.
    ///
    /// P4d4 row 2: the media batch is taken at the ACCEPTED local enqueue
    /// boundary and rides THIS turn's params. `media` is omitted entirely when
    /// the batch is empty (octos-core `ui_protocol.rs:2044-2045`,
    /// `skip_serializing_if = "Vec::is_empty"`), so a text-only turn is
    /// byte-identical to [`Conversation::start_turn`].
    pub async fn start_turn_with_media(
        &self,
        text: impl Into<String>,
        media: Vec<crate::screens::media::TurnMedia>,
    ) -> Result<String, ClientError> {
        let turn_id = TurnId::new().0.to_string();
        let text: String = text.into();
        let mut params = serde_json::json!({
            "session_id": self.session_id(),
            "turn_id": turn_id,
            "input": [{"kind": "text", "text": text}],
        });
        if !media.is_empty() {
            params["media"] = serde_json::Value::Array(media.iter().map(|m| m.to_value()).collect());
        }
        if let Some(effort) = crate::screens::board3::thinking::effort_param(&self.store, &self.session_id()) {
            params["reasoning_effort"] = serde_json::Value::String(effort);
        }
        self.trace.record(
            self.started,
            Direction::Out,
            "turn/start",
            None,
            Some(format!("turn={turn_id}")),
        );
        note_own_turn(&turn_id);
        // The optimistic row is the same as a text-only send.
        self.store.domains.session.timeline.upsert_user_message(
            &self.session_id(),
            &turn_id,
            &text,
            serde_json::json!({"optimistic": true}),
        );
        {
            let mut ui = self.ui.lock().unwrap();
            ui.begin_turn(&turn_id, self.started);
            ui.set_draft_inner(String::new());
        }
        match self.client.request("turn/start", params).await {
            Ok(_) => {
                makepad_widgets::log!("[octoscode] turn started: {turn_id}");
                Ok(turn_id)
            }
            Err(e) => {
                // #34a's rule: a FAILED send restores the draft.
                self.ui.lock().unwrap().set_draft_inner(text.clone());
                Err(e)
            }
        }
    }

    /// A8 — return a failed prompt to its OWN Session: the composer when it
    /// still shows that Session, else that Session's stored draft
    /// (`session-composer-drafts.ts` restores stay on their owning record).
    fn return_prompt(&self, owner: &str, text: &str) {
        if self.session_id() == owner {
            self.ui.lock().unwrap().set_draft_inner(text.to_owned());
        } else {
            crate::screens::drafts::restore_for(&crate::screens::drafts::key_of(self, owner), text);
        }
    }

    /// `turn/start` with an explicit `turn_id`.
    ///
    /// The web mints the turn id client-side and sends it in the request
    /// (`packages/client/src/client.ts:488`; the composer's
    /// `acceptLocalDispatch(turn.turnId, "running")`,
    /// `src-web/apps/web/src/features/composer/use-turn-controller.ts:413`), so
    /// the id is the caller's. Splitting it out lets a **replay** drive the flow
    /// with the recorded turn id — the way the real session ran — so the
    /// server's `user_message` envelope (which carries that same id) dedups into
    /// the optimistic row.
    pub async fn start_turn_with_id(
        &self,
        text: impl Into<String>,
        turn_id: String,
    ) -> Result<String, ClientError> {
        let text: String = text.into();
        // A8 — the Session this prompt belongs to (a failed send returns it
        // there, even if the composer moved to another Session meanwhile).
        let owner = self.session_id();
        #[allow(unused_mut)]
        let mut params = serde_json::json!({
            "session_id": self.session_id(),
            "turn_id": turn_id,
            "input": [{"kind": "text", "text": text}],
        });
        // A4 — the Session's thinking effort rides every new prompt, omitted
        // for the Profile default (`use-turn-controller.ts:398-410`).
        if let Some(effort) = crate::screens::board3::thinking::effort_param(&self.store, &self.session_id()) {
            params["reasoning_effort"] = serde_json::Value::String(effort);
        }
        self.trace.record(
            self.started,
            Direction::Out,
            "turn/start",
            None,
            Some(format!("turn={turn_id}")),
        );
        note_own_turn(&turn_id);
        // Card #26 §1: insert the user's row NOW, keyed by the turn id, so the
        // prompt shows immediately — and still shows for an INTERRUPTED turn,
        // whose server `user_message` envelope never arrives (live-gate turn 2).
        // The web does the same: an optimistic `user:${turnId}` row on dispatch
        // (`use-turn-controller.ts:413` `acceptLocalDispatch`,
        // `timeline/model.ts:997-1005` `user:${turnId}`), deduped when the
        // canonical copy lands (`upsertUser`, `model.ts:1019-1043`). Our
        // `upsert_user_message` is the same dedup: one row per turn id, so the
        // later server copy updates this row in place rather than adding a second.
        self.store.domains.session.timeline.upsert_user_message(
            &self.session_id(),
            &turn_id,
            &text,
            serde_json::json!({"optimistic": true}),
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
                makepad_widgets::log!("[octoscode] turn started: {turn_id}");
                Ok(turn_id)
            }
            Err(e) => {
                makepad_widgets::log!("[octoscode] turn/start failed for {turn_id}: {e}");
                // #34a's rule (the /bogus fix): a FAILED send restores the
                // user's text — the clear happened optimistically before the
                // request, so put it back (and drop the optimistic row's
                // turn from live, the web's dispatch rollback).
                self.return_prompt(&owner, &text);
                makepad_widgets::log!(
                    "[octoscode] draft restored: {} chars",
                    text.chars().count()
                );
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
        // A7 — the turn controller's interrupt gate (`use-turn-controller.ts:
        // 860-930`) for a turn the composer admitted: a start Core has not
        // accepted yet is never interrupted ("Turn is still starting"), a turn
        // already interrupting is not asked twice, and the interrupted prompt
        // is stashed for ITS OWN terminal.
        let session = self.session_id();
        let composer = &self.store.domains.composer;
        let known = composer.snapshot(&session).active.map(|a| a.turn_id).as_deref() == Some(turn_id);
        if known {
            if composer.dispatching_turn(&session).as_deref() == Some(turn_id) {
                self.store.domains.session.timeline.upsert_notice(
                    &session,
                    Some(turn_id.to_owned()),
                    &format!("still-starting:{turn_id}"),
                    "Turn is still starting",
                    "Octos has not accepted this turn yet, so no interrupt was sent.",
                    "",
                );
                makepad_widgets::SignalToUI::set_ui_signal();
                return Ok(serde_json::Value::Null);
            }
            if !composer.begin_interrupt(&session, turn_id) {
                return Ok(serde_json::Value::Null);
            }
        }
        ::log::info!("octoscode: interrupting turn {turn_id}");
        self.trace.record(
            self.started,
            Direction::Out,
            "turn/interrupt",
            None,
            Some(format!("turn={turn_id}")),
        );
        let reply = self
            .client
            .request(
                "turn/interrupt",
                serde_json::json!({"session_id": session, "turn_id": turn_id}),
            )
            .await;
        if reply.is_err() && known {
            self.store.domains.composer.interrupt_failed(&session, turn_id);
        }
        reply
    }

    /// Card #26 §2: if the current session owes a resync (a
    /// `protocol/replay_lossy` marked it lossy — card #22 §1), consume that
    /// flag and send `session/hydrate`.
    ///
    /// This is the production consumer of
    /// [`octoscode_store::domains::config::Config::resync_pending`], the hook
    /// card #22 left for whoever owns the transport. The web does the same from
    /// its recovery path: on a lossy event the runtime calls
    /// `#hydrate(authority, "recovery")`
    /// (`src-web/apps/web/src/features/session/active-session-runtime.ts:1256`),
    /// which issues `session/hydrate` (`packages/client/src/client.ts:480`). The
    /// reply arrives as [`TransportEvent::SessionHydrated`] and
    /// [`Conversation::dispatch`] folds it and calls
    /// [`octoscode_store::domains::config::Config::mark_recovered`].
    ///
    /// `take_resync` makes this idempotent: a second call is a no-op until
    /// another lossy event raises the flag.
    pub fn maybe_resync(&self) -> bool {
        let session = self.session_id();
        if !self.store.domains.config.resync_pending(&session) {
            return false;
        }
        if !self.store.domains.config.take_resync(&session) {
            return false;
        }
        // Fire-and-forget on the transport's command channel (the same path
        // `session/open` uses). Best-effort: a closed channel just logs.
        let sent = self.request_hydrate(&session);
        if sent {
            ::log::info!("octoscode: resync requested — session/hydrate {session}");
        }
        sent
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
        Self::fresh_session_id_for(&self.profile())
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
        // A8 — a FULL Session id (`<profile>:api:<chat>`, the web's
        // `bindWebSessionIdToProfile` shape), so the identity grammar
        // (`screens::session_identity`) recognises what this app created.
        crate::screens::session_identity::fresh_full_id(profile, &TurnId::new().0.to_string())
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
        // A19 — no Session without a profile: a fresh connection's first
        // Session comes from the launch decision (`screens::launch`), which
        // adopts the resolved profile before it opens.
        if self.profile().trim().is_empty() {
            makepad_widgets::log!("[octoscode] new chat refused: no profile yet (the launch decides it)");
            return Err("Choose a workspace first: the server has not named a profile for this connection yet.".to_owned());
        }
        let id = Self::fresh_session_id_for(&self.profile());
        ::log::info!("octoscode: new chat -> {id}");
        // A8 — the new-session defaults apply at CREATION only
        // (`session-defaults.ts:4-9`): the sandbox rides this open; the
        // permission mode + network go out once the open for THIS id lands.
        let defaults = crate::screens::session_defaults::current();
        if defaults.stored {
            self.creation_defaults.arm(&id, defaults.value.clone());
        }
        let advertised = self
            .store
            .capabilities()
            .iter()
            .any(|f| f == crate::screens::session_defaults::SANDBOX_FEATURE);
        let sandbox = crate::screens::session_defaults::open_sandbox(&defaults.value, advertised);
        self.open_workspace_with(&id, cwd, sandbox).await
    }

    /// A15 — fold one ACCEPTED canonical hydrate into `session`'s transcript:
    /// the web's `timelineFromHydrate` (`timeline/model.ts:42-240`) reduced to
    /// the native store's append-only rules (`Timeline::fold_hydrated_messages`).
    ///
    /// * **Turn identity.** Core stamps a turn's user/assistant/tool rows with
    ///   `thread_id` = the turn UUID and usually omits `turn_id`; the web maps
    ///   a thread to its turn (`turnByThread`, `model.ts:47-60`,
    ///   `session-record-manager.ts:1052-1066`). The native row model groups a
    ///   transcript BY TURN (`screen::timeline_rows_folded`: one prompt, its
    ///   tools, its answer), so every row takes `turn_id ?? thread_id`: a turn
    ///   this client streamed live is recognised (A12's no-duplicate rule) and
    ///   a history turn keeps its own prompt and answer instead of collapsing
    ///   into one group.
    /// * **Tool rows.** The replayed tool envelopes (`replayed_tool_envelopes`
    ///   + the tool records of `replayed_projection_envelopes`, deduped by
    ///   (thread, seq), `model.ts:198-221`) are folded as the turn's tool cards
    ///   — name, arguments, status, output — exactly as live delivery draws
    ///   them; such a turn's persisted `tool` rows are the cards' outputs
    ///   (`coalesceHydratedTools`, `model.ts:864+`) and are not repeated. A
    ///   turn with no replayed card (a server restart evicts the replay
    ///   window) keeps its rows as "Tool output" rows.
    /// * A card this client already holds (same `tool_call_id`) is never
    ///   drawn twice; nothing is deleted.
    ///
    /// Returns the entries added.
    fn fold_history(&self, session: &str, h: &octos_core::ui_protocol::SessionHydrateResult) -> usize {
        use octos_core::ui_protocol::PayloadV2;
        let timeline = &self.store.domains.session.timeline;
        // The replayed tool records, once each, in delivery order.
        let mut seen_env: std::collections::HashSet<(String, u64)> = std::collections::HashSet::new();
        let mut tool_envs: Vec<&octos_core::ui_protocol::EnvelopeV2> = h
            .replayed_tool_envelopes
            .iter()
            .flatten()
            .chain(h.replayed_projection_envelopes.iter().flatten().filter(|e| {
                matches!(
                    e.payload,
                    PayloadV2::ToolStart { .. } | PayloadV2::ToolProgress { .. } | PayloadV2::ToolEnd { .. }
                )
            }))
            .filter(|e| seen_env.insert((e.thread_id.clone(), e.seq)))
            .collect();
        tool_envs.sort_by_key(|e| e.cursor.as_ref().map(|c| c.seq).unwrap_or(e.seq));
        let rows: Vec<octoscode_store::timeline::HydratedRow> = h
            .messages
            .iter()
            .flatten()
            .map(|m| octoscode_store::timeline::HydratedRow {
                seq: m.seq,
                role: m.role.as_str(),
                content: m.content.as_str(),
                turn_id: m.turn_id.as_ref().map(|t| t.0.to_string()).or_else(|| m.thread_id.clone()),
                reasoning: m.reasoning_content.as_deref(),
            })
            .collect();
        // Cards only for a turn the transcript holds (its persisted rows, or
        // rows streamed live): the replay window can retain a turn's tool
        // records after its rows are gone (a rollback, a reset store), and a
        // card with no prompt and no answer would float below every turn.
        let mut held: std::collections::HashSet<String> = rows
            .iter()
            .filter(|r| r.role == "user" || r.role == "assistant")
            .filter_map(|r| r.turn_id.clone())
            .collect();
        held.extend(
            timeline
                .entries(session)
                .into_iter()
                .filter(|e| e.kind == octoscode_store::EntryKind::USER_MESSAGE || e.kind == octoscode_store::EntryKind::ASSISTANT_TEXT)
                .filter_map(|e| e.turn_id),
        );
        tool_envs.retain(|e| held.contains(e.turn_id.as_str()));
        let carded: std::collections::HashSet<&str> = tool_envs
            .iter()
            .filter(|e| matches!(e.payload, PayloadV2::ToolStart { .. }))
            .map(|e| e.turn_id.as_str())
            .collect();
        let rows: Vec<octoscode_store::timeline::HydratedRow> = rows
            .into_iter()
            .filter(|r| !(r.role == "tool" && r.turn_id.as_deref().is_some_and(|t| carded.contains(t))))
            .collect();
        let mut added = if rows.is_empty() { 0 } else { timeline.fold_hydrated_messages(session, &rows) };
        // A turn that did not complete (stopped, failed, rate limited) keeps
        // its terminal notice: Core persists no row for an interrupted turn,
        // and the web renders that server truth ("This turn was stopped
        // before it completed.", `model.ts:240-270`) rather than an empty
        // gap. Natively the SAME notice the live terminal draws, under the
        // same `terminal:<turn>` id (`turn::terminal_notice`), so a turn
        // stopped live is never noted twice.
        // The turns in stream order, by their retained terminal (Core keeps
        // every thread's terminal, even a compacted one's): a restored notice
        // goes before the next turn this transcript holds, where the web puts
        // a message-less terminal turn ("before the next turn").
        let mut terminals: Vec<(u64, &octos_core::ui_protocol::EnvelopeV2)> = h
            .replayed_projection_envelopes
            .iter()
            .flatten()
            .filter(|e| matches!(e.payload, PayloadV2::TurnTerminal { .. }))
            .map(|e| (e.cursor.as_ref().map(|c| c.seq).unwrap_or(e.seq), e))
            .collect();
        terminals.sort_by_key(|(seq, _)| *seq);
        for (k, (_, env)) in terminals.iter().enumerate() {
            let PayloadV2::TurnTerminal { outcome, error, .. } = &env.payload else { continue };
            use octos_core::ui_protocol::TurnTerminalOutcome as O;
            let name = match outcome {
                O::Completed => continue,
                O::Errored => "errored",
                O::Interrupted => "interrupted",
                O::RateLimited => "rate_limited",
            };
            let before = timeline.len(session);
            let err = error.as_ref().map(|e| (e.code.as_str(), e.message.as_str()));
            octoscode_client::domains::turn::terminal_notice(&self.store, session, &env.turn_id, name, err, None);
            added += timeline.len(session) - before;
            let entries = timeline.entries(session);
            let key = format!("terminal:{}", env.turn_id);
            let notice = entries
                .iter()
                .find(|e| e.data.get("notice_id").and_then(|v| v.as_str()) == Some(key.as_str()))
                .map(|e| e.id);
            let next = terminals[k + 1..]
                .iter()
                .map(|(_, e)| e.turn_id.as_str())
                .find(|t| entries.iter().any(|e| e.turn_id.as_deref() == Some(*t)));
            if let (Some(id), Some(next)) = (notice, next) {
                timeline.move_before_turn(session, id, next);
            }
        }
        if tool_envs.is_empty() {
            return added;
        }
        let mut cards: std::collections::HashSet<String> = timeline
            .of_kind(session, octoscode_store::EntryKind::TOOL_CALL)
            .into_iter()
            .filter_map(|e| e.data.get("tool_call_id").and_then(|v| v.as_str()).map(str::to_owned))
            .collect();
        let known_calls: std::collections::HashSet<String> =
            self.store.domains.tool.calls().into_iter().map(|c| c.tool_call_id).collect();
        let mut drawn: std::collections::HashSet<String> = std::collections::HashSet::new();
        for env in tool_envs {
            match &env.payload {
                PayloadV2::ToolStart { tool_call_id, name, arguments_preview } => {
                    if !cards.insert(tool_call_id.clone()) {
                        continue; // already on screen (live, or an earlier hydrate)
                    }
                    self.store.domains.tool.call_started(tool_call_id, name, arguments_preview.as_deref());
                    timeline.append_data(
                        session,
                        Some(env.turn_id.clone()),
                        octoscode_store::EntryKind::TOOL_CALL,
                        name.clone(),
                        serde_json::json!({
                            "tool_call_id": tool_call_id,
                            "status": "running",
                            "hydrate_id": format!("hydrate:tool:{tool_call_id}"),
                            "replayed": true,
                        }),
                    );
                    drawn.insert(tool_call_id.clone());
                    added += 1;
                }
                PayloadV2::ToolEnd { tool_call_id, status, output_preview, duration_ms, .. }
                    if drawn.contains(tool_call_id) || known_calls.contains(tool_call_id) =>
                {
                    let wire = match status {
                        octos_core::ui_protocol::EnvelopeToolEndStatus::Complete => "complete",
                        octos_core::ui_protocol::EnvelopeToolEndStatus::Error => "error",
                        octos_core::ui_protocol::EnvelopeToolEndStatus::Skipped => "skipped",
                        octos_core::ui_protocol::EnvelopeToolEndStatus::Aborted => "aborted",
                    };
                    self.store.domains.tool.call_ended(tool_call_id, wire, output_preview.as_deref(), *duration_ms);
                }
                _ => {}
            }
        }
        added
    }

    /// The created ids whose new-session defaults were applied (test seam).
    pub fn defaults_applied(&self) -> Vec<String> {
        self.creation_defaults.applied()
    }

    /// A8 — restore the Session's PARKED interactions (approvals and user
    /// questions still pending server-side) from its canonical hydrate
    /// (`session/hydrate {include: ["pending_approvals"]}`; the transport's
    /// own hydrate asks for messages only). Folded only when the reply names
    /// this Session AND no newer generation started meanwhile (the web:
    /// "restore a parked approval/question from a canonical hydrate with its
    /// generation"); each restored interaction must belong to this Session.
    fn spawn_restore_parked(&self, session: String, methods: &[String]) {
        // Only where a parked interaction can be answered (`approval/respond`
        // or `user_question/respond` advertised) and read canonically.
        let answerable = methods.iter().any(|m| m == "approval/respond" || m == "user_question/respond");
        if !answerable || !methods.iter().any(|m| m == "session/hydrate") {
            return;
        }
        let Ok(handle) = tokio::runtime::Handle::try_current() else { return };
        let client = self.client.clone();
        let store = self.store.clone();
        let gen_cell = self.open_seq.clone();
        let generation = *gen_cell.lock().unwrap();
        handle.spawn(async move {
            let reply = client
                .call::<octoscode_client::domains::session::SessionHydrate>(octos_core::ui_protocol::SessionHydrateParams {
                    session_id: octos_core::SessionKey(session.clone()),
                    after: None,
                    include: vec!["pending_approvals".to_owned()],
                })
                .await;
            let Ok(h) = reply else { return };
            if h.session_id.0 != session || *gen_cell.lock().unwrap() != generation {
                ::log::warn!("octoscode: parked interactions for {session} from a retired generation — not restored");
                return;
            }
            let (mut approvals, mut questions) = (0, 0);
            for a in h.pending_approvals.unwrap_or_default() {
                if a.session_id.0 != session {
                    continue;
                }
                let preview = a
                    .typed_details
                    .as_ref()
                    .and_then(|d| d.diff.as_ref())
                    .map(|d| octoscode_client::protocol_id::preview_id_string(&d.preview_id))
                    .filter(|id| octoscode_client::protocol_id::is_protocol_uuid(&serde_json::json!(id)));
                let id = a.approval_id.0.to_string();
                store.domains.approval.request_with_preview(&id, Some(a.tool_name.clone()), preview);
                // A6's takeover card draws from the same detail a live
                // `approval/requested` records: a restored approval is asked
                // again, exactly like the one that parked it.
                store.domains.approval.set_detail(&id, octoscode_client::domains::approval::approval_detail(&a));
                approvals += 1;
            }
            for q in h.pending_questions.unwrap_or_default() {
                if q.session_id.0 != session {
                    continue;
                }
                store.domains.approval.set_question(octoscode_store::domains::approval::PendingQuestion {
                    question_id: q.question_id.0.to_string(),
                    session_id: q.session_id.0.clone(),
                    turn_id: q.turn_id.0.to_string(),
                    title: q.title.clone(),
                    body: q.body.clone(),
                    questions: serde_json::to_value(&q.questions).unwrap_or(serde_json::Value::Null),
                });
                questions += 1;
            }
            if approvals + questions > 0 {
                makepad_widgets::log!("[octoscode] restored {approvals} parked approval(s), {questions} question(s) for {session}");
                makepad_widgets::SignalToUI::set_ui_signal();
            }
        });
    }

    /// A12 — the outage bookkeeping for one transport state: a drop of a
    /// connection that was live is RETAINED (the shell stays, the banner
    /// counts the re-dials against the server in use), a re-dialed socket is
    /// "restoring", and a transport that gave up is replaced while the
    /// conversation is retained (the web never stops retrying an opened
    /// Session). A first connect that never went live keeps the old
    /// behaviour: its failure is the Connect card's.
    fn note_link(&self, s: &octos_app_transport::ConnectionState, t: link::Transition) {
        use octos_app_transport::ConnectionState as C;
        let conn = &self.store.connection;
        let base = *self.attempt_base.lock().unwrap();
        match (s, t) {
            (C::Live, _) => {
                // Back: the next outage counts its re-dials from 1 again.
                *self.attempt_base.lock().unwrap() = 0;
            }
            (C::Reconnecting { attempt }, _) => {
                conn.note_outage(&self.http_base, Some(base + attempt), false);
            }
            (_, link::Transition::Redial) => {
                conn.note_outage(&self.http_base, None, true);
            }
            (C::Dialing | C::Idle, _) => {
                if conn.outage().is_some() {
                    conn.note_outage(&self.http_base, None, false);
                }
            }
            (C::Failed, _) => {
                if conn.ever_live() && conn.note_outage(&self.http_base, None, false) {
                    // Retained: a fresh transport keeps re-dialing (its own
                    // count restarts at 1; the banner's keeps going).
                    *self.attempt_base.lock().unwrap() = conn.outage().map(|o| o.attempt).unwrap_or(base);
                    makepad_widgets::log!(
                        "[octoscode] link: the transport gave up on {} — starting a fresh one",
                        self.http_base
                    );
                    self.link.respawn();
                } else {
                    // Nothing retained: queued commands fail instead of
                    // waiting for a transport that will never come.
                    self.link.mark_dead();
                }
            }
            _ => {}
        }
    }

    /// A12 — the banner's "Retry now": re-dial at once instead of waiting
    /// out the transport's backoff (up to 30 s between attempts,
    /// `ws/mod.rs` `RECONNECT_DELAY_MAX`). Only while an outage is retained.
    pub fn retry_now(&self) -> bool {
        let conn = &self.store.connection;
        let Some(o) = conn.outage() else { return false };
        if conn.is_live() || self.link.is_closed() {
            return false;
        }
        *self.attempt_base.lock().unwrap() = o.attempt;
        makepad_widgets::log!("[octoscode] link: retry now — re-dialing {}", o.endpoint);
        self.link.respawn()
    }

    /// A12 — the server this conversation's transport dials (the HTTP
    /// origin, never a default).
    pub fn endpoint(&self) -> String {
        self.http_base.clone()
    }

    /// A12 — the connection dropped and the Session is not back yet (still
    /// re-dialing, or re-opening on a new socket): the composer and the
    /// transport-bound actions refuse (the web replaces the composer with
    /// its recovery banner until the Session is healthy again,
    /// `App.tsx:2724-2754`).
    pub fn in_outage(&self) -> bool {
        self.store.outage().is_some() || self.link.in_outage()
    }

    /// A8 — the reconnect path (`active-session-runtime.ts` recovery): bump
    /// the generation BEFORE anything goes out (every result captured under
    /// the old one is now stale), re-open the active Session at its workspace
    /// and ask for its canonical hydrate. A15: the hydrate is asked by the
    /// open's reply arm (every open hydrates), so a refused re-open asks for
    /// nothing and an accepted one asks exactly once.
    fn reopen_after_reconnect(&self) {
        let session = self.session_id();
        let cwd = self.store.domains.session.workspace_root(&session);
        // A15 — the dropped socket's in-flight hydrates are never answered.
        self.hydrate_gen.lock().unwrap().clear();
        *self.open_seq.lock().unwrap() += 1;
        *self.pending_open_cwd.lock().unwrap() = cwd.clone();
        let params = SessionOpenParams {
            session_id: octos_core::SessionKey(session.clone()),
            topic: None,
            profile_id: Some(self.profile()),
            cwd,
            sandbox: None,
            after: None,
            client_commands: None,
        };
        self.frames.out("session/open", &serde_json::json!({"session_id": session, "reconnect": true}));
        match self.cmd_tx.try_send(OutboundCommand::OpenSession(params)) {
            Ok(()) => {
                ::log::info!("octoscode: reconnect — re-opened {session} (generation {})", self.generation());
            }
            Err(e) => ::log::warn!("octoscode: reconnect re-open of {session} failed: {e}"),
        }
    }

    /// Send the creation-time permission default for `session` off the event
    /// path (the reply arm cannot await). Unadvertised = surfaced failure.
    fn apply_permission_default(
        &self,
        session: &str,
        defaults: &crate::screens::session_defaults::SessionDefaults,
        methods: &[String],
    ) {
        use crate::screens::session_defaults as sd;
        if !methods.iter().any(|m| m == "permission/profile/set") {
            sd::note_apply_result(Err("permission/profile/set is not advertised".into()));
            return;
        }
        let params = sd::permission_params(session, defaults);
        let client = self.client.clone();
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            ::log::warn!("octoscode: new-session permission default: no runtime");
            return;
        };
        let session = session.to_owned();
        handle.spawn(async move {
            let r = client.request("permission/profile/set", params).await;
            makepad_widgets::log!(
                "[octoscode] new-session permission default for {session}: {}",
                if r.is_ok() { "applied" } else { "failed" }
            );
            sd::note_apply_result(r.map(|_| ()).map_err(|e| e.to_string()));
            makepad_widgets::SignalToUI::set_ui_signal();
        });
    }

    /// `session/list` — re-ask for the session rows and fold them into the
    /// store (the `session.refresh` action). Returns the row count.
    ///
    /// A15 — the rows carry the server's titles (`title`, else `last_prompt`:
    /// octos titles a Session from its first prompt), so the list is the
    /// web's per-workspace catalog, `session/list {cwd, profile_id}`
    /// (`workspace-session-catalog.ts:188-196`), whenever the server offers
    /// it (`session/list` + `session.workspace_cwd.v1`,
    /// `supportsWorkspaceSessionCatalog` `:56-63`) and the active Session's
    /// workspace is known: octos keeps a workspace's Sessions in
    /// `<cwd>/.octos/<profile>`, and the legacy unscoped listing does not see
    /// them — the live smoke's Session stayed "New chat" after several turns.
    pub async fn refresh_sessions(&self) -> Result<usize, ClientError> {
        let result = self
            .client
            .call::<octoscode_client::domains::session::SessionList>(self.catalog_params())
            .await?;
        let sessions = result.into_sessions();
        let n = sessions.len();
        self.store.set_sessions(sessions);
        Ok(n)
    }

    /// A15 — the catalog's params: `{cwd, profile_id}` for the active
    /// Session's workspace under the opened Profile (`App.tsx:751-760`
    /// `catalogProfileId`: the opened `active_profile_id`, else the
    /// connection's), else the legacy `{}`.
    pub fn catalog_params(&self) -> octoscode_client::domains::session::SessionListParams {
        let config = &self.store.domains.config;
        let offered = config.supported_methods().iter().any(|m| m == "session/list")
            && config.supported_features().iter().any(|f| f == "session.workspace_cwd.v1");
        let root = self
            .store
            .active_session()
            .and_then(|s| self.store.domains.session.workspace_root(&s))
            .filter(|r| !r.trim().is_empty());
        match root {
            Some(cwd) if offered => {
                let profile = self
                    .store
                    .domains
                    .profile
                    .current()
                    .filter(|p| !p.trim().is_empty())
                    .unwrap_or_else(|| self.profile());
                octoscode_client::domains::session::SessionListParams { cwd: Some(cwd), profile_id: Some(profile) }
            }
            _ => octoscode_client::domains::session::SessionListParams::default(),
        }
    }

    /// A15 — re-list the catalog off the event path (the web re-lists when
    /// the opened Session changes or a turn starts or finishes: the catalog's
    /// `refreshKey` is `authority \n opened.session_id \n queue.active.turnId`,
    /// `App.tsx:753-760`, "so a new conversation or a retitled one shows up
    /// without a reload"). Needs the shared handle ([`Conversation::attach`]).
    fn spawn_catalog_refresh(&self, why: &'static str) {
        let Some(me) = self.weak_self.lock().unwrap().upgrade() else { return };
        let Ok(handle) = tokio::runtime::Handle::try_current() else { return };
        handle.spawn(async move {
            match me.refresh_sessions().await {
                Ok(n) => ::log::info!("octoscode: session/list after {why}: {n} rows"),
                Err(e) => ::log::warn!("octoscode: session/list after {why}: {e}"),
            }
            makepad_widgets::SignalToUI::set_ui_signal();
        });
    }

    /// `composer.submit` — the composer's send button: `turn/start` with the
    /// current draft (`bindings.json` `composer.submit`).
    ///
    /// Card #26: an **empty draft starts no turn**. Found by the live proof — the
    /// composer's send control also carries STOP while a turn is live, and a
    /// stop-glyph click that lands just after the turn settled routed to
    /// `composer.submit` with the (already cleared) draft, minting an optimistic
    /// row with empty text and no server copy. The web refuses the same way at
    /// its submit entry (`use-turn-controller.ts:631` `!text.trim()`, and
    /// `:602` for a queued turn), so a whitespace-only prompt never becomes a
    /// turn. Returns an empty id (no turn) rather than an error: refusing an
    /// empty prompt is not a failure.
    pub async fn submit_draft(&self) -> Result<String, ClientError> {
        let text = self.ui.lock().unwrap().draft();
        if text.trim().is_empty() {
            // #32h: this was ::log::debug! + Ok — the invisible silent drop
            // the phone showed (no drop log, draft kept). Visible now.
            makepad_widgets::log!("[octoscode] submit ignored: empty draft");
            return Ok(String::new());
        }
        // A12 — while the connection is down the composer refuses HONESTLY:
        // the text stays, the banner says it was not sent, nothing is queued
        // to fire later (the web hides its composer behind the recovery
        // banner until the Session is healthy, `App.tsx:2724-2754`).
        if self.in_outage() {
            let t = text.trim_start();
            let cmd = t.starts_with('/').then(|| t.split_whitespace().next().unwrap_or(t)).filter(|c| c.len() > 1);
            crate::screens::reconnect::note_held(crate::screens::reconnect::held_line(cmd));
            makepad_widgets::log!(
                "[octoscode] submit held: reconnecting to {} (draft kept, {} chars)",
                self.http_base,
                text.chars().count()
            );
            return Ok(String::new());
        }
        // A19 — a fresh connection has no profile until the launch decides
        // one: the web shows its launch panel instead of a composer then
        // (`App.tsx:2470-2490`); nothing is sent and the text stays.
        if !*self.workspace_opened.lock().unwrap() && self.profile().trim().is_empty() {
            makepad_widgets::log!("[octoscode] submit held: no Session yet (the launch decides the profile)");
            return Ok(String::new());
        }
        // The web's first message creates the thread: never turn/start on a
        // session the server has not opened.
        if !*self.workspace_opened.lock().unwrap() {
            if let Err(e) = self.open_workspace(None).await {
                makepad_widgets::log!("[octoscode] submit: ensure thread failed: {e}");
                return Err(ClientError::Transport {
                    method: "session/open".to_owned(),
                    reason: e,
                });
            }
        }
        // #34a row 209 — a slash command is LOCAL on the web: it never
        // reaches the model, and a failed/unknown command restores the input
        // (`surface-recovery.spec.ts:186` "restores input and never sends
        // command text to the model"). An unresolved command fails closed
        // HERE, before dispatch: the draft clear lives in
        // `start_turn_with_id`, so returning early keeps the user's text
        // editable in the composer.
        // #P4d3 — the web's command layer (registry.ts:647 looksLikeSlashCommand,
        // :654 parseCommandInvocation, :656 findCommand, :679
        // commandAvailability): parse the invocation, resolve it against the
        // ported 47-command registry, act on the match. A PATH-shaped input
        // ("/home/user/x/y", "/c/d") is a PROMPT and reaches the model verbatim —
        // the old arm refused every leading-slash input, paths included.
        // A5 — the web's `isLocalShellBang` (`intent.ts:175`): a `!command`
        // runs on the TUI host only, so it is REPORTED and never sent to the
        // model; the text stays editable (`local-report.ts:97`).
        if crate::screens::palette::is_local_shell_bang(&text) {
            let session = self.session_id();
            self.store.domains.session.timeline.append(
                &session,
                Some(crate::screens::palette::next_receipt_turn()),
                crate::screens::palette::REPORT_KIND,
                "Local shell unavailable — Octoscode's ! command runs on the TUI host. \
                 This app cannot execute a local process, so nothing was sent."
                    .to_owned(),
            );
            makepad_widgets::SignalToUI::set_ui_signal();
            ::log::info!("octoscode: local shell bang: receipt appended, draft kept");
            return Ok(String::new());
        }
        // A7 — `/steer [on|off]` (`intent.ts:96-108`, `set-steer`): the
        // Session's local steering opt-in. Never sent to the model.
        if let Some((name, args)) = crate::screens::palette::parse_command_invocation(&text) {
            if matches!(name.to_ascii_lowercase().as_str(), "steer" | "steer-mid-turn" | "steermode") {
                let receipt = self.steer_command(&args);
                makepad_widgets::log!("[octoscode] command /steer: {receipt}");
                return Ok(String::new());
            }
        }
        // A4 — the board-3 surfaces answer their web commands locally
        // (`/tools`, `/mcp`, `/threads`, `/turn`, `/permissions`,
        // `/thinking`, `/resume`, `/images`, `/rewind`, `/undo`, `/fork`,
        // `/sessions`, `/vimmode`; registry.ts intents). The invocation never
        // reaches the model: open the surface, clear the draft, run its load.
        if let Some((name, args)) = crate::screens::palette::parse_command_invocation(&text) {
            if let Some(outcome) = crate::screens::board3::host::command(&name, &args, self) {
                self.ui.lock().unwrap().set_draft_inner(String::new());
                // A8 — a consumed command is not an unsent draft: clear its
                // saved text too, or A7's recovery puts "/resume" back when
                // the Session comes round again ("sent drafts stay cleared").
                crate::drafts::save(&self.session_id(), "");
                makepad_widgets::SignalToUI::set_ui_signal();
                makepad_widgets::log!("[octoscode] command /{name}: board-3 surface ({outcome:?})");
                if let crate::screens::board3::host::Outcome::Spawn(job) = outcome {
                    if let Err(e) = crate::screens::board3::host::run(job, self).await {
                        makepad_widgets::log!("[octoscode] command /{name}: {e}");
                    }
                    makepad_widgets::SignalToUI::set_ui_signal();
                }
                return Ok(String::new());
            }
        }
        match crate::screens::palette::match_command(&text) {
            None => {}
            // A5 — a known, runnable command is LOCAL: it runs its native
            // effect (opens its dialog, …) and never reaches the model. The
            // host drains the queue on the Signal this raises.
            Some(crate::screens::palette::CommandMatch::Known(args, name)) => {
                if crate::screens::palette::queue_run(&name, &args) {
                    self.ui.lock().unwrap().set_draft_inner(String::new());
                    crate::drafts::save(&self.session_id(), "");
                    makepad_widgets::SignalToUI::set_ui_signal();
                    makepad_widgets::log!("[octoscode] command /{name}: queued to run locally");
                    return Ok(String::new());
                }
            }
            Some(crate::screens::palette::CommandMatch::NotRunnable(name)) => {
                // A KNOWN name the native build cannot run: report WHY and
                // consume the invocation — never dispatched, never a silent
                // no-op (registry.ts:478's fail-closed explanation).
                let session = self.session_id();
                self.store.domains.session.timeline.append(
                    &session,
                    Some(crate::screens::palette::next_receipt_turn()),
                    crate::screens::palette::REPORT_KIND,
                    format!(
                        "/{name} is not available in this native build — \
                         nothing was sent to the model."
                    ),
                );
                self.ui.lock().unwrap().set_draft_inner(String::new());
                crate::drafts::save(&self.session_id(), "");
                // #P4a's lesson, again: an async arm on the tokio thread that
                // mutates the store never repaints by itself — wake the UI or
                // the receipt stays invisible until some other event draws.
                makepad_widgets::SignalToUI::set_ui_signal();
                ::log::info!(
                    "octoscode: command /{name}: not runnable natively — \
                     receipt appended, composer cleared"
                );
                return Ok(String::new());
            }
            Some(crate::screens::palette::CommandMatch::Unknown(name)) => {
                // A slash-shaped input that names nothing: fail closed
                // VISIBLY (the receipt row) and keep the text editable — the
                // web's surface-recovery rule (surface-recovery.spec.ts:186).
                let session = self.session_id();
                self.store.domains.session.timeline.append(
                    &session,
                    Some(crate::screens::palette::next_receipt_turn()),
                    crate::screens::palette::REPORT_KIND,
                    format!(
                        "Unsupported command: /{name} — kept in the composer, \
                         nothing was sent to the model."
                    ),
                );
                makepad_widgets::SignalToUI::set_ui_signal();
                ::log::info!(
                    "octoscode: command /{name}: unknown — receipt appended, \
                     draft kept"
                );
                return Ok(String::new());
            }
        }
        // A7 — admission is refused while an unknown-outcome turn is held for
        // recovery (`enqueuePrompt`: `if (recovery || …) return false`): the
        // text stays in the composer and no attachment is consumed.
        let session = self.session_id();
        if self.store.domains.composer.recovery(&session).is_some() {
            makepad_widgets::log!("[octoscode] submit held: the last response's outcome is unknown");
            return Ok(String::new());
        }
        // A4 — the AttachmentsDialog's draft rides THIS turn when every image
        // is uploaded; any row not yet uploaded refuses the send and keeps
        // both the prompt and the draft (`session-composer-drafts.ts:183-188`).
        let media = match crate::screens::media::take_for_submit(self) {
            Ok(Some(media)) => media,
            Ok(None) => Vec::new(),
            Err(msg) => {
                self.store.domains.session.timeline.append(
                    &session,
                    Some(crate::screens::palette::next_receipt_turn()),
                    crate::screens::palette::REPORT_KIND,
                    msg,
                );
                makepad_widgets::SignalToUI::set_ui_signal();
                return Ok(String::new());
            }
        };
        self.submit_prompt(text, media).await
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
            FlowEvent::TurnStarted(id) => makepad_widgets::log!("[octoscode] turn started: {id}"),
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
                // A12 — a voluntary leave (Disconnect / Forget / a confirmed
                // stop: the store reads Offline) is final for this
                // conversation: a transport still winding down never brings
                // it back.
                if self.store.connection.is_offline() || self.link.is_closed() {
                    return FlowEvent::Other(format!("after-leave {s:?}"));
                }
                let transition = self.link.note_state(s);
                let live = matches!(s, octos_app_transport::ConnectionState::Live);
                self.store.set_connection(format!("{s:?}"), live);
                self.note_link(s, transition);
                // A7: a drop suspends the turn controller's transport
                // generation; the next Live reconciles from a hydrate.
                let dropped = matches!(
                    s,
                    octos_app_transport::ConnectionState::Reconnecting { .. }
                        | octos_app_transport::ConnectionState::Failed
                        | octos_app_transport::ConnectionState::Idle
                        | octos_app_transport::ConnectionState::Dialing
                );
                self.note_connection(live, dropped);
                if live {
                    *self.ever_live.lock().unwrap() = true;
                }
                // A8 — a handshake AFTER an earlier Live is a reconnect. The
                // WS transport parks a re-dialed socket in Handshaking until a
                // `session/open` answers (it does not replay the opens
                // itself, and the server dropped the old socket's session
                // subscriptions), so the flow re-opens the active Session at
                // its workspace and hydrates it, under a NEW authority
                // generation (`active-session-runtime.ts` recovery).
                // A12 — keyed on the SOCKET, not on an earlier Live: a socket
                // that dropped before its first `session/open` answered was
                // never Live, and its re-dial parked in Handshaking forever.
                // Any re-dialed socket re-opens the Session the window shows.
                if transition == link::Transition::Redial
                    && (*self.ever_live.lock().unwrap() || self.store.active_session().is_some())
                {
                    self.reopen_after_reconnect();
                }
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
                // #P4g1 row 204: a resume that ASKED for a workspace must not
                // accept an open that answers a different one — the web's
                // `validateCandidateWorkspace` throws "The server opened a
                // different workspace from the saved link."
                // (`candidate-session.ts:230-243`; fresh launches pass no cwd
                // and may be canonicalized, exactly like the web's
                // `requireExactWorkspace = false` default.) Fail closed:
                // record the reject, never adopt this open's authority.
                let requested = self.pending_open_cwd.lock().unwrap().take();
                if let (Some(req), Some(actual)) =
                    (requested.as_deref(), r.opened.workspace_root.as_deref())
                {
                    if !req.is_empty() && req != actual {
                        ::log::warn!(
                            "octoscode: session/open returned workspace {actual:?} for requested {req:?} — open rejected"
                        );
                        self.store.domains.session.note_workspace_reject(
                            &r.opened.session_id.0,
                            req,
                            actual,
                        );
                        // A12 — a reconnect's re-open answered another
                        // workspace: recovery is required, never adopted.
                        if self.store.outage().is_some_and(|o| o.restoring) {
                            self.store
                                .connection
                                .note_outage_error("The server opened a different workspace from this conversation's.");
                        }
                        // A19 — a restore that lands elsewhere is refused.
                        self.settle_open_watch(Err(format!("the server opened {actual} instead of {req}")));
                        return FlowEvent::Other("session/open-workspace-mismatch".to_owned());
                    }
                }
                if let Some(root) = &r.opened.workspace_root {
                    self.store
                        .domains
                        .session
                        .set_workspace_root(&r.opened.session_id.0, root);
                }
                // A9 — the opened Profile (the web's `session.opened
                // .active_profile_id`, Settings > General's Profile row); the
                // `session/open` notification handler folds the same field.
                if let Some(profile) = &r.opened.active_profile_id {
                    self.store.domains.profile.set_current(profile.clone());
                }
                // A4 — the Session's initial thinking effort is the open
                // reply's `reasoning_effort` (session-composer-drafts.ts:40),
                // and the show-thinking preference applies to the first opened
                // Session (App.tsx:524-537).
                if let Some(level) = &r.opened.reasoning_effort {
                    if let Ok(serde_json::Value::String(e)) = serde_json::to_value(level) {
                        self.store
                            .domains
                            .session
                            .set_thinking_effort(&r.opened.session_id.0, &e);
                    }
                }
                crate::screens::board3::thinking::apply_pref_once(&self.store, &r.opened.session_id.0);
                // #34b — seed the opened session BEFORE anything folds a
                // session/list reply: the web treats an opened session as
                // known immediately (known-session-registry), so the reply's
                // catalog lag can neither drop it nor dangle the active id.
                self.store
                    .note_session_opened(&r.opened.session_id.0, None);
                self.store.set_active(Some(r.opened.session_id.0.clone()));
                // #P4e1b rows 4+9: bind the autonomy state to THIS commands
                // identity and the session the open reply names. The identity is
                // per-socket-and-open: any later open, reconnect or re-auth
                // presents a new one, and the fence then drops every late
                // result, busy marker and carried-over row (web
                // `features/autonomy/store.ts:208-231` `#syncAuthority`).
                self.store
                    .domains
                    .autonomy
                    .bind_identity(&self.identity());
                self.store
                    .domains
                    .autonomy
                    .bind_session(&r.opened.session_id.0);
                // #P4g1 row 217: the open reply's `UiProtocolCapabilities`
                // carries the advertised methods AND features — record both
                // and evaluate the web's coding gate
                // (coding-capabilities.ts:38-66 via
                // `octoscode_client::features::missing_coding_session_requirements`).
                let caps = &r.opened.capabilities;
                self.store
                    .domains
                    .config
                    .set_supported_methods(caps.supported_methods.clone());
                // A8 — parked interactions come back from the canonical
                // hydrate of THIS open (`session-interaction-ledger.ts:137`).
                self.spawn_restore_parked(r.opened.session_id.0.clone(), &caps.supported_methods);
                // A8 — the ONE creation-time `permission/profile/set` for a
                // session `new_chat` created (`App.tsx:1936-1975`): only the
                // armed fresh id, once; a failure is surfaced, never retried.
                if let Some(defaults) = self.creation_defaults.take(&r.opened.session_id.0) {
                    self.apply_permission_default(&r.opened.session_id.0, &defaults, &caps.supported_methods);
                }
                // A7: the features too (safe steering needs
                // `event.turn_steer_dropped.v1`, App.tsx:2849-2858).
                self.store
                    .domains
                    .config
                    .set_supported_features(caps.supported_features.clone());
                self.store.domains.config.set_coding_gate(
                    octoscode_client::features::missing_coding_session_requirements(
                        &caps.supported_methods,
                        &caps.supported_features,
                    ),
                );
                // A15 — EVERY open loads the Session's history, as the web's
                // does: a candidate open is `openSession` then
                // `hydrateSession({session_id: opened.session_id})`
                // (`candidate-session.ts:166-183`, also the retained-owner path
                // `active-session-runtime.ts:159-176`), and a reconnect re-opens
                // and `#hydrate(authority, "reconnect")`s (`:1065`). So the
                // startup open, a sidebar row, Resume, a switch, a New chat
                // and a reconnect's re-open each ask once, here, when the open
                // the server answered is adopted (before this, only a reconnect
                // or a lossy replay did: a restarted app showed an empty "New
                // chat"). The reply folds through `SessionHydrated` below —
                // append-only, a row already streamed live recognised by
                // Core's thread id = turn id (A12), so a re-open never
                // duplicates. Only where `session/hydrate` is advertised (the
                // web's coding gate requires it, `coding-capabilities.ts:14-16`).
                if caps.supported_methods.iter().any(|m| m == "session/hydrate") {
                    self.request_hydrate(&r.opened.session_id.0);
                }
                // A15 — and the catalog re-lists for the opened Session's
                // workspace, now that it is known (the web's `refreshKey`
                // carries `opened.session_id`): the server's titles.
                self.spawn_catalog_refresh("open");
                // A19 — the committed open is what the next launch restores
                // (the web: `App.tsx:976-992` session id, active profile and
                // workspace root into the saved connection).
                if *self.remember_opens.lock().unwrap() {
                    let profile = r
                        .opened
                        .active_profile_id
                        .clone()
                        .filter(|p| !p.trim().is_empty())
                        .unwrap_or_else(|| self.profile());
                    crate::screens::remembered::note_opened(
                        &self.http_base,
                        &profile,
                        &r.opened.session_id.0,
                        r.opened.workspace_root.as_deref(),
                    );
                }
                self.settle_open_watch(Ok(r.opened.session_id.0.clone()));
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
                // A10: a TRACKED peer session's own frames (a roster row
                // that is not the active master session) fold into that row
                // and never reach the master's timeline or UI
                // (`peerSessionEventFor`, session-peer-coordinator.ts:139).
                if crate::screens::peers::fold_frame(&self.store, payload) {
                    return FlowEvent::Other(format!("peer-session {}", payload.method()));
                }
                // #P4g1 rows 205/213: the runtime scope gate. The durable
                // projection (projection/envelope, protocol/replay_lossy)
                // routes by session scope — out-of-scope frames are
                // `wrong_session` ignores on the web
                // (`durable-session.ts:117-124`) — and peer lifecycle events
                // route ONLY by their full originating SessionKey
                // (`scope.ts:27-31`). Foreign frames are logged by name and
                // never reach a handler or the UI fold (no silent drop: the
                // log line + this FlowEvent record the drop).
                if self.out_of_runtime_scope(payload) {
                    ::log::debug!(
                        "octoscode: {} for a foreign session dropped (runtime scope {:?})",
                        payload.method(),
                        self.store.active_session()
                    );
                    return FlowEvent::Other(format!("wrong-session {}", payload.method()));
                }
                let ev = self.note_notification(payload);
                // A15 — a turn starting or finishing re-lists the catalog
                // (the web's `refreshKey` carries the active turn id,
                // `App.tsx:753-760`): the server's title for a new Session
                // (its first prompt) and the row's recency show up.
                let turn_edge = match &ev {
                    FlowEvent::TurnStarted(t) => Some(format!("{t}:start")),
                    FlowEvent::TurnEnded { turn_id, .. } => Some(format!("{turn_id}:end")),
                    _ => None,
                };
                if let Some(edge) = turn_edge {
                    let fresh = {
                        let mut last = self.catalog_edge.lock().unwrap();
                        let fresh = last.as_deref() != Some(edge.as_str());
                        *last = Some(edge);
                        fresh
                    };
                    if fresh {
                        self.spawn_catalog_refresh("a turn edge");
                    }
                }
                self.registry.lock().unwrap().dispatch(payload);
                // A6: the open task detail appends this session's live
                // `task/output/delta` by byte offset (`use-supervision.ts:516-522`).
                crate::screens::surfaces::observe(payload, self.store.active_session().as_deref());
                // A7: the turn controller's view (activity / terminal /
                // returned steering) — after the store folded the frame.
                self.composer_observe(payload);
                // Card #26 §2: a `protocol/replay_lossy` just marked the session
                // lossy and raised a resync; issue the `session/hydrate` now.
                // (Web: `active-session-runtime.ts:1256` `#hydrate(…, "recovery")`.)
                self.maybe_resync();
                ev
            }
            // Card #26 §2: the authoritative hydrate reply. Fold its
            // continuation checkpoints and clear the lossy phase — the web's
            // `commitHydrate` (`durable-session.ts:101-107`).
            TransportEvent::SessionHydrated { session_id, result } => {
                // A15 — this reply answers the OLDEST in-flight read of the
                // session (one socket answers in order), whatever it decodes
                // to: an undecodable reply must not leave its generation
                // behind for the next reply to be judged by.
                let (requested_gen, current_in_flight) = {
                    let mut map = self.hydrate_gen.lock().unwrap();
                    let q = map.entry(session_id.clone()).or_default();
                    let g = q.pop_front();
                    (g, q.contains(&self.generation()))
                };
                match serde_json::from_value::<octos_core::ui_protocol::SessionHydrateResult>(
                    result.clone(),
                ) {
                    Ok(h) => {
                        // #P4g1 row 206: verify the returned session id BEFORE
                        // committing anything — the web's `commitHydrate`
                        // throws `Hydrate returned session …, expected …`
                        // (`durable-session.ts:83-87`; the
                        // `HydrateSessionMismatchError` "inherently fatal"
                        // routing fault, `active-session-runtime.ts:371-380`).
                        // Fail closed: fold nothing, keep the lossy phase so
                        // the resync stays owed, and name the mismatch in the
                        // log (no silent drop).
                        // A8 — a reply requested under a RETIRED generation (a
                        // reconnect or another open since) is a stale
                        // authority: fail closed, fold nothing, and ask again
                        // under the current one while the session still owes
                        // its resync (the web's "fails recovery preparation
                        // before committing the hydrate cursor").
                        if requested_gen.is_some_and(|g| g != self.generation()) {
                            ::log::warn!(
                                "octoscode: session/hydrate for {session_id} from a retired generation — stale authority, not committed"
                            );
                            // A15: unless a hydrate of the CURRENT generation
                            // is already on its way (the new open's own).
                            if self.store.active_session().as_deref() == Some(session_id.as_str())
                                && self.store.domains.config.recovery(session_id).phase
                                    != octoscode_store::domains::config::LossyPhase::Healthy
                                && !current_in_flight
                            {
                                self.request_hydrate(session_id);
                            }
                            return FlowEvent::Other("session/hydrate-stale-authority".to_owned());
                        }
                        if h.session_id.0 != *session_id {
                            ::log::warn!(
                                "octoscode: session/hydrate returned session {} for requested {session_id} — hydrate commit rejected",
                                h.session_id.0
                            );
                            FlowEvent::Other("session/hydrate-mismatch".to_owned())
                        } else {
                            if let Some(seqs) = &h.projection_thread_sequences {
                                self.store.domains.turn.fold_hydrate(seqs);
                            }
                            // #P4g1 row 206: adopt the hydrate's cursor — the
                            // web's `commitHydrate` takes
                            // `#cursor = { ...result.cursor }`
                            // (`durable-session.ts:84`); max-wins in the store.
                            self.store.domains.turn.adopt_hydrate_cursor(
                                &h.cursor.stream,
                                h.cursor.seq,
                            );
                            // Card #P4b2 (canonical hydrate recovery): rebuild the
                            // transcript from the authoritative snapshot — the
                            // web's `restoreCanonicalHydrate`
                            // (`timeline/canonical-hydrate.ts:31`) reduced to the
                            // store's rules: seq order, durable bodies finalized,
                            // idempotent by seq identity, NEVER a delete. Sits
                            // INSIDE the #P4g1 mismatch guard's else: only a
                            // matching snapshot commits anything.
                            let added = self.fold_history(session_id, &h);
                            self.store.domains.config.mark_recovered(&session_id);
                            ::log::info!(
                                "octoscode: session/hydrate folded for {session_id} (+{added} rows)"
                            );
                            FlowEvent::Other("session/hydrate".to_owned())
                        }
                    }
                    Err(e) => {
                        ::log::warn!("octoscode: session/hydrate decode: {e}");
                        FlowEvent::Other("session/hydrate-decode-error".to_owned())
                    }
                }
            }
            TransportEvent::RpcError { method, error, .. } => {
                ::log::warn!("octoscode: rpc error {method}: {}", error.message);
                // A12 — the re-open on a re-dialed socket was refused: the
                // banner turns into the web's "Session recovery required"
                // with the reason (Retry now re-dials and re-opens again).
                if method == "session/open" && self.store.outage().is_some_and(|o| o.restoring) {
                    self.store.connection.note_outage_error(&error.message);
                    makepad_widgets::log!("[octoscode] link: the re-open was refused: {}", error.message);
                }
                // A19 — a refused open settles its watcher (a restore falls
                // back to a fresh launch).
                if method == "session/open" {
                    self.settle_open_watch(Err(error.message.clone()));
                }
                FlowEvent::Other(format!("rpc-error {method}"))
            }
            other => FlowEvent::Other(format!("{other:?}")),
        }
    }

    /// Record the flow's own view of a notification (tool rows, turn timing,
    /// pending approvals) — the things the store's domain handlers do not keep.
    /// #P4g1 rows 205/213 — the runtime scope gate, the module's seam for the
    /// web's `notificationMatchesSessionScope`
    /// (`src-web/apps/web/src/features/session/scope.ts:7-44`) +
    /// `DurableProjector.observe`'s `wrong_session` ignore
    /// (`durable-session.ts:117-124`):
    /// - `projection/envelope` and `protocol/replay_lossy` are DURABLE
    ///   projections of ONE session: a frame whose session is not the
    ///   runtime's active session is a `wrong_session` ignore on the web and
    ///   must not reach a handler or the UI fold here.
    /// - `peer/staged` / `peer/closed` "route only by their full originating
    ///   SessionKey" (`scope.ts:27-31`) — slug/topic are payload, never the
    ///   routing key.
    /// - A topicless/legacy bare envelope decodes with the EMPTY session key;
    ///   the web passes an undefined `session_id` through for non-peer events
    ///   (`scope.ts:23-24`), so the empty key passes too.
    /// Foreign frames are never a silent drop: the caller logs them by name
    /// and records a `wrong-session <method>` FlowEvent.
    fn out_of_runtime_scope(&self, n: &UiNotification) -> bool {
        let active = self.store.active_session();
        let foreign = |sid: &str| match &active {
            Some(a) => a != sid,
            None => true,
        };
        match n {
            UiNotification::EnvelopeV2(frame) => {
                !frame.session_id.0.is_empty() && foreign(&frame.session_id.0)
            }
            UiNotification::ReplayLossy(e) => foreign(&e.session_id.0),
            // Peer lifecycle: the full originating SessionKey, exactly.
            UiNotification::PeerStaged(e) => foreign(&e.session_id.0),
            UiNotification::PeerClosed(e) => foreign(&e.session_id.0),
            _ => false,
        }
    }

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
                    PayloadV2::TurnTerminal { outcome, error, .. } => {
                        // Card #21e item 1: record the outcome so the tail row can
                        // show an "Interrupted" marker instead of a bare duration.
                        ui.note_outcome(
                            &turn_id,
                            match outcome {
                                TurnTerminalOutcome::Completed => "completed",
                                TurnTerminalOutcome::Errored => "errored",
                                TurnTerminalOutcome::Interrupted => "interrupted",
                                TurnTerminalOutcome::RateLimited => "rate_limited",
                            },
                        );
                        match outcome {
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
                        }
                    }
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

    /// End the turn named by `turn_id` (test support; the L1 gate's own entry).
    /// A terminal for a DIFFERENT turn must leave the live turn untouched.
    pub fn end_turn_for_test(&mut self, turn_id: &str, ok: bool) {
        self.end_turn(turn_id, ok);
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
                // **Card #21j**: the measured duration belongs to THIS turn's own
                // record, so a later turn's terminal cannot restamp it.
                let end = self.turn_ends.entry(turn_id.to_owned()).or_default();
                end.worked = Some(started.elapsed());
                end.completed_at = Some(SystemTime::now());
                self.active_turn = None;
                self.last_settled_turn = Some(turn_id.to_owned());
            }
            // A terminal for another turn, or no live turn: do not touch the
            // live turn's state.
            _ => return,
        }
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

/// Format a completed-turn instant the way the atlas labels it
/// (`design/components/answer-actions/page.card:6` → `Sep 28, 9:41 PM`), with
/// `now` for a just-finished turn — the web's `formatRelativeTime` rule
/// (`features/shell/relative-time.ts:1-18`: `< 60s` → `now`).
///
/// Pure std (no date crate in this crate's dependency set): the UTC civil date
/// comes from Howard Hinnant's `civil_from_days`. A local-clock offset is out
/// of scope for this design label; a fresh turn renders `now` either way.
fn format_completed_at(at: std::time::SystemTime, now: std::time::SystemTime) -> String {
    format_completed_at_offset(at, now, local_offset_secs(at))
}

/// The platform time zone's offset from UTC (seconds east) at `at`. The web's
/// `Intl.DateTimeFormat` renders local time; this label was UTC (an answer at
/// 21:09 local read "4:09 AM" in the judge's live capture).
pub(crate) fn local_offset_secs(at: std::time::SystemTime) -> i64 {
    use chrono::Offset;
    let local: chrono::DateTime<chrono::Local> = at.into();
    i64::from(local.offset().fix().local_minus_utc())
}

/// [`format_completed_at`] at an explicit UTC offset (tests pin 0).
fn format_completed_at_offset(
    at: std::time::SystemTime,
    now: std::time::SystemTime,
    offset_secs: i64,
) -> String {
    let secs = at
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let now_secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if now_secs.saturating_sub(secs) < 60 {
        return "now".to_owned();
    }
    let secs = (secs as i64 + offset_secs).max(0) as u64;
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    // days since 1970-01-01 (floored), then Hinnant's civil-from-days.
    let z = (secs / 86_400) as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let _ = y;
    let tod = secs % 86_400;
    let (h24, min) = (tod / 3600, (tod % 3600) / 60);
    let (h12, ampm) = match h24 {
        0 => (12, "AM"),
        1..=11 => (h24, "AM"),
        12 => (12, "PM"),
        _ => (h24 - 12, "PM"),
    };
    format!(
        "{} {}, {}:{:02} {}",
        MONTHS[(m - 1) as usize], d, h12, min, ampm
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Card #21e item 1: the "Interrupted" marker comes from the LIVE turn's own
    /// `turn_terminal` outcome, so its `note_outcome` is gated by turn id exactly
    /// like `end_turn` (LESSONS 5 / the L1 gate). A terminal for a different,
    /// already-settled turn must not stamp its outcome onto the tail row.
    #[test]
    fn the_interrupted_marker_ignores_a_terminal_for_another_turn() {
        let mut ui = FlowUi::default();
        ui.begin_turn_now("turn-B");
        // A stale terminal for the settled turn-A must not set the marker.
        ui.note_outcome("turn-A", "interrupted");
        assert_eq!(ui.worked_for(), "", "turn-A's terminal must not mark turn-B");
        // turn-B's own terminal does.
        ui.note_outcome("turn-B", "interrupted");
        assert_eq!(ui.worked_for(), "Interrupted");
    }

    /// #P4d1 row 147 — Esc still interrupts while a user question waits. The
    /// question fold only raises `question_pending` (the
    /// UserQuestionRequested arm); it never clears the live turn, so the
    /// keyboard's Escape keeps routing to Interrupt (the web:
    /// UserQuestionPanel.tsx:79 onEscape -> onInterrupt). Only the turn's
    /// OWN terminal retires it — then Esc is a no-op again.
    #[test]
    fn esc_still_interrupts_while_a_question_waits() {
        let mut ui = FlowUi::default();
        ui.begin_turn_now("turn-q");
        ui.set_pending_for_test(false, true);
        // The question wait never ends the live turn…
        assert!(
            ui.turn_active(),
            "a pending question must not retire the live turn"
        );
        // …so the keyboard's Escape still routes to Interrupt.
        let action = crate::screens::keys::resolve(
            makepad_widgets::KeyCode::Escape,
            false, false, false, false,
            false, // palette_open
            false, // approval_pending
            None,  // approval_preview (#P4f2 row 7 — no approval is showing)
            ui.turn_active(),
            true,  // draft_empty
        );
        assert_eq!(action, crate::screens::keys::KeyAction::Interrupt);
        // The turn's own terminal is what retires it — then Esc ignores.
        ui.end_turn_now(false);
        assert!(!ui.turn_active());
        let action = crate::screens::keys::resolve(
            makepad_widgets::KeyCode::Escape,
            false, false, false, false,
            false, false,
            None, // approval_preview (#P4f2 row 7)
            ui.turn_active(),
            true,
        );
        assert_eq!(action, crate::screens::keys::KeyAction::Ignore);
    }

    /// Card #21d item 4: the label must be the atlas's `Sep 28, 9:41 PM`
    /// (`design/components/answer-actions/page.card:6`), and a just-finished
    /// turn reads `now` (the web's `relative-time.ts:1-18` rule, `< 60s`).
    #[test]
    fn answer_timestamp_is_a_display_label_not_the_raw_epoch() {
        use std::time::{Duration, UNIX_EPOCH};
        // 2025-09-28T21:41:00Z
        let at = UNIX_EPOCH + Duration::from_secs(1_759_095_660);
        assert_eq!(
            format_completed_at_offset(at, at + Duration::from_secs(5), 0),
            "now",
            "a fresh turn is 'now', like the web"
        );
        assert_eq!(
            format_completed_at_offset(at, at + Duration::from_secs(3600), 0),
            "Sep 28, 9:41 PM",
            "an older turn matches the atlas label"
        );
        // midnight and noon render 12-hour, not 0/24
        let midnight = UNIX_EPOCH + Duration::from_secs(1_767_139_500); // 2025-12-31T00:05Z
        assert_eq!(
            format_completed_at_offset(midnight, midnight + Duration::from_secs(3600), 0),
            "Dec 31, 12:05 AM"
        );
        let noon = UNIX_EPOCH + Duration::from_secs(1_735_732_800); // 2025-01-01T12:00Z
        assert_eq!(
            format_completed_at_offset(noon, noon + Duration::from_secs(3600), 0),
            // day is `numeric` (unpadded), like the web's
            // `Intl.DateTimeFormat({month:"short", day:"numeric"})`
            "Jan 1, 12:00 PM"
        );
    }

    /// Judge fix: the label is LOCAL time, like the web's Intl formatter.
    /// 2025-10-02T04:09Z at UTC-7 is Oct 1, 9:09 PM; at UTC+8 Oct 2, 12:09 PM.
    #[test]
    fn answer_timestamp_is_local_time() {
        use std::time::{Duration, UNIX_EPOCH};
        let at = UNIX_EPOCH + Duration::from_secs(1_759_378_140); // 2025-10-02T04:09Z
        let later = at + Duration::from_secs(3600);
        assert_eq!(format_completed_at_offset(at, later, -7 * 3600), "Oct 1, 9:09 PM");
        assert_eq!(format_completed_at_offset(at, later, 8 * 3600), "Oct 2, 12:09 PM");
        assert_eq!(format_completed_at_offset(at, later, 0), "Oct 2, 4:09 AM");
    }

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
