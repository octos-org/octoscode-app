//! A10 — the Agents panel: the native agent RPCs (`agent/list`,
//! `agent/status/read`, `agent/output/read`, `agent/artifact/list`,
//! `agent/artifact/read`, `agent/interrupt`, `agent/close`) and the
//! "Request parallel agents" ordinary-turn spawn.
//!
//! Web oracle: `features/autonomy/AgentPanel.tsx` (the section of the
//! autonomy dialog that `/agents` opens, `registry.ts:383-397`), its store
//! ops (`autonomy/store.ts:743-869`), `agent-spawn.ts` and
//! `agent-spawn-admission.ts`. No Stage-A board covers this surface: it is
//! built with the native dialog kit (`board3/ui.rs` — the same backdrop,
//! centred card, close glyph and type ramp as the other dialogs) and the web
//! component as the reference.
//!
//! The state the panel shows lives in the store
//! (`octoscode_store::domains::autonomy` — the agent roster plus the
//! single-viewer [`AgentViewer`]); this module keeps only the UI-local form
//! drafts. Every RPC goes through the typed client methods
//! (`octoscode_client::domains::autonomy`), with `session_id` and WITHOUT a
//! `profile_id` — the web never sends one for agent ops, and the recorded
//! c24 traffic shows the server refusing a profile-scoped agent read
//! ("agent is outside the requested profile scope").
use octoscode_client::domains::autonomy::{
    agent_record, AgentArtifactList, AgentArtifactRead, AgentArtifactReadParams, AgentClose,
    AgentInterrupt, AgentList, AgentOutputCursor, AgentOutputRead, AgentOutputReadParams, AgentParams,
    AgentStatusRead, AutonomyListParams,
};
use octoscode_store::domains::autonomy::{
    AgentArtifactRow, AgentArtifactView, AgentOutputView, AgentRecord, AgentViewer,
};
use octoscode_store::Store;

use super::host::{Job, Outcome};
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};

/// The error family the panel's alert line reads (`agentsError`).
pub const FAMILY: &str = "agents";
/// The feature every agent control needs besides its own method
/// (`packages/client/src/autonomy.ts:31-40,112-117`).
pub const FEATURE: &str = "coding.agent_control.v1";
/// Terminal statuses: Interrupt/Close are disabled (`AgentPanel.tsx:7`).
pub const TERMINAL: &[&str] = &["completed", "failed", "interrupted", "closed"];

/// The web's copy.
pub const EMPTY: &str = "No agents running.";
pub const SPAWN_HINT: &str =
    "Sends the native agent request through this session\u{2019}s ordinary prompt queue. Available only while idle.";
pub const SPAWN_INVALID: &str = "Choose a positive whole-number count and enter the task.";
pub const SPAWN_REFUSED: &str =
    "Agent request was not queued. The owning session must still be idle and ready.";

/// UI-local drafts (the values the protocol never carries). Each input has a
/// live value (`changed` events) and the snapshot the DSL carries — the mount
/// must not rebuild a focused input on every keystroke.
#[derive(Debug, Clone)]
pub struct AgentsState {
    pub count: String,
    pub count_snap: String,
    pub task: String,
    pub task_snap: String,
    pub query_id: String,
    pub query_snap: String,
    pub path: String,
    pub path_snap: String,
    /// "Inspect or control an agent by ID" is a closed `<details>`.
    pub by_id_open: bool,
    pub spawn_error: Option<String>,
    /// The refusal line was edited past (hidden live until the next click).
    pub error_hidden: bool,
    /// Bumped when a request clears the task: the form's container id
    /// carries it, so the mount rebuilds the inputs from their (cleared)
    /// snapshots even when the click and the reply land between two frames.
    pub form_gen: u64,
    /// The roster is loading (`agent/list` in flight).
    pub loading: bool,
    /// A turn is running or queued in the owning session (the spawn is
    /// idle-only); the host reports it every frame.
    pub turn_busy: bool,
}

impl Default for AgentsState {
    fn default() -> Self {
        Self {
            count: "1".into(),
            count_snap: "1".into(),
            task: String::new(),
            task_snap: String::new(),
            query_id: String::new(),
            query_snap: String::new(),
            path: String::new(),
            path_snap: String::new(),
            by_id_open: false,
            spawn_error: None,
            error_hidden: false,
            form_gen: 0,
            loading: false,
            turn_busy: false,
        }
    }
}

impl AgentsState {
    /// Freeze the live drafts into the DSL snapshots (before a remount the
    /// user asked for).
    pub fn snap(&mut self) {
        self.error_hidden = false;
        self.count_snap = self.count.clone();
        self.task_snap = self.task.clone();
        self.query_snap = self.query_id.clone();
        self.path_snap = self.path.clone();
    }
}

/// Which agent controls the server advertises (each needs its method AND
/// `coding.agent_control.v1`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Caps {
    pub list: bool,
    pub status: bool,
    pub output: bool,
    pub artifacts: bool,
    pub artifact_read: bool,
    pub interrupt: bool,
    pub close: bool,
    pub turn_start: bool,
}

impl Caps {
    pub fn of(store: &Store) -> Self {
        let feature = store.capabilities().iter().any(|c| c == FEATURE);
        let m = store.domains.config.supported_methods();
        let has = |name: &str| feature && m.iter().any(|x| x == name);
        Caps {
            list: has("agent/list"),
            status: has("agent/status/read"),
            output: has("agent/output/read"),
            artifacts: has("agent/artifact/list"),
            artifact_read: has("agent/artifact/read"),
            interrupt: has("agent/interrupt"),
            close: has("agent/close"),
            turn_start: m.iter().any(|x| x == "turn/start"),
        }
    }
    /// The panel renders nothing when no agent control is advertised
    /// (`AgentPanel.tsx:22-31`).
    pub fn any(&self) -> bool {
        self.list || self.status || self.output || self.artifacts || self.artifact_read || self.interrupt || self.close
    }
}

/// `composeAgentSpawn` (`agent-spawn.ts:2-11`): a whole count 1..=u32::MAX
/// and a non-blank task, or `None`. Multiline task text is kept.
pub fn compose_spawn(count: &str, task: &str) -> Option<String> {
    let n: u64 = count.trim().parse().ok()?;
    if n == 0 || n > u32::MAX as u64 {
        return None;
    }
    let question = task.trim();
    if question.is_empty() {
        return None;
    }
    Some(format!("Spawn {n} agent(s) to accomplish in parallel: {question}"))
}

/// The agent a per-row action addresses: the roster row `index`, or (`id`
/// actions) the by-ID field.
fn target(st: &AgentsState, store: &Store, action: &str, index: usize) -> Option<String> {
    if action.contains(".id.") {
        let id = st.query_id.trim().to_owned();
        return (!id.is_empty()).then_some(id);
    }
    store.domains.autonomy.agents().get(index).map(|a| a.agent_id.clone())
}

/// The host reports whether a turn runs in the owning session (the spawn is
/// idle-only, `agent-spawn-admission.ts:10-43`).
pub fn note_turn_busy(busy: bool) {
    let mut st = super::host::state();
    if st.agents.turn_busy != busy {
        st.agents.turn_busy = busy;
    }
}

/// The live gates (applied after every mount without a remount): the
/// spawn button's primary/disabled variant, the by-ID actions (a non-blank
/// id), "Read artifact by path" (id and path), the edited-past refusal.
pub fn visibility(st: &AgentsState) -> Vec<(String, bool)> {
    let ready = !st.turn_busy && compose_spawn(&st.count, &st.task).is_some();
    let has = !st.query_id.trim().is_empty();
    let path = has && !st.path.trim().is_empty();
    vec![
        ("b3_agents_spawn_on".into(), ready),
        ("b3_agents_spawn_off".into(), !ready),
        ("b3_agents_id_on".into(), has),
        ("b3_agents_id_off".into(), !has),
        ("b3_agents_path_on".into(), path),
        ("b3_agents_path_off".into(), !path),
        ("b3_agents_spawn_error".into(), st.spawn_error.is_some() && !st.error_hidden),
    ]
}

// ------------------------------------------------------------------ actions

/// Route one `b3.agents.*` action (`index` = the `#<row>` suffix).
pub fn perform(st: &mut AgentsState, action: &str, index: usize, store: &Store) -> Outcome {
    let caps = Caps::of(store);
    st.snap();
    let viewer = store.domains.autonomy.agent_viewer();
    let one = |ok: bool, job: Job| if ok { Outcome::Spawn(job) } else { Outcome::Done };
    match action {
        "b3.agents.refresh" => one(caps.list, Job::AgentsLoad),
        "b3.agents.by_id" => {
            st.by_id_open = !st.by_id_open;
            Outcome::Done
        }
        "b3.agents.spawn" => {
            if !(caps.list && caps.turn_start) {
                return Outcome::Done;
            }
            match compose_spawn(&st.count, &st.task) {
                None => {
                    st.spawn_error = Some(SPAWN_INVALID.into());
                    Outcome::Done
                }
                Some(text) if st.turn_busy => {
                    let _ = text;
                    st.spawn_error = Some(SPAWN_REFUSED.into());
                    Outcome::Done
                }
                Some(text) => {
                    st.spawn_error = None;
                    Outcome::Spawn(Job::AgentsSpawn(text))
                }
            }
        }
        "b3.agents.output_more" => match viewer.output.as_ref() {
            Some(o) if o.has_more && !viewer.output_busy => {
                Outcome::Spawn(Job::AgentOutput(o.agent_id.clone(), true))
            }
            _ => Outcome::Done,
        },
        "b3.agents.artifact" => {
            // Read artifact row `index` of the listed artifacts, by id.
            match viewer.artifacts.as_ref().and_then(|(a, rows)| rows.get(index).map(|r| (a.clone(), r.id.clone()))) {
                Some((agent, id)) if caps.artifact_read => {
                    Outcome::Spawn(Job::AgentArtifactRead(agent, Some(id), None))
                }
                _ => Outcome::Done,
            }
        }
        "b3.agents.id.read_path" => {
            let (id, path) = (st.query_id.trim().to_owned(), st.path.trim().to_owned());
            one(caps.artifact_read && !id.is_empty() && !path.is_empty(), Job::AgentArtifactRead(id, None, Some(path)))
        }
        a => {
            let Some(id) = target(st, store, a, index) else { return Outcome::Done };
            let verb = a.rsplit('.').next().unwrap_or("");
            let terminal = !a.contains(".id.")
                && store.domains.autonomy.agent(&id).is_some_and(|r| TERMINAL.contains(&r.status.as_str()));
            let pending = viewer.pending.iter().any(|p| p == &id);
            match verb {
                "status" => one(caps.status, Job::AgentStatus(id)),
                "output" => one(caps.output, Job::AgentOutput(id, false)),
                "artifacts" => one(caps.artifacts, Job::AgentArtifacts(id)),
                "interrupt" => one(caps.interrupt && !terminal && !pending, Job::AgentControl(id, "interrupt")),
                "close" => one(caps.close && !terminal && !pending, Job::AgentControl(id, "close")),
                _ => Outcome::Unrouted,
            }
        }
    }
}

/// A text input changed (`agents.count` / `agents.task` / `agents.id` /
/// `agents.path`). An edit hides the spawn refusal (live, no remount; the
/// web clears its error on edit).
pub fn input_changed(st: &mut AgentsState, key: &str, text: &str) {
    match key {
        "agents.count" => {
            st.count = text.to_owned();
            st.error_hidden = true;
        }
        "agents.task" => {
            st.task = text.to_owned();
            st.error_hidden = true;
        }
        "agents.id" => st.query_id = text.to_owned(),
        "agents.path" => st.path = text.to_owned(),
        _ => {}
    }
}

// --------------------------------------------------------------- transport

fn session_of(conv: &crate::flow::Conversation) -> String {
    conv.session_id()
}

/// Record a failure as the panel's alert line — only while the op stays
/// authorized (`#runGuarded`: "records the error only while the op stays
/// authorized"; a newer epoch drops it).
/// A13 — the plain lead over a failed agents read: the family error names
/// its method (`fail`), so the lead says which read failed; the cause shows
/// muted under it (`ui::error_line`).
pub fn failure_lead(e: &str) -> &'static str {
    match e.split_once(": ").map(|(method, _)| method) {
        Some("agent/status/read") => "Couldn't read the agent's status.",
        Some("agent/output/read") => "Couldn't read the agent's output.",
        Some("agent/artifact/list") => "Couldn't list the agent's artifacts.",
        Some("agent/artifact/read") => "Couldn't open the agent's artifact.",
        _ => "Couldn't load this session's agents.",
    }
}

fn fail(store: &Store, epoch: u64, method: &str, e: impl std::fmt::Display) -> String {
    let msg = format!("{method}: {e}");
    store.domains.autonomy.record_error(FAMILY, epoch, &msg);
    msg
}

/// `agent/list {session_id}` — the envelope must echo the session exactly
/// (`requireListEnvelope`), and only records the session controls are kept
/// (`sessionControlsTarget`: exact or base-key match).
pub async fn load(conv: &crate::flow::Conversation) -> Result<String, String> {
    let store = &conv.store;
    let session = session_of(conv);
    let epoch = store.domains.autonomy.epoch();
    let rev = store.domains.autonomy.bump_revision("agents-list");
    super::host::state().agents.loading = true;
    let r = conv
        .client()
        .call::<AgentList>(AutonomyListParams { session_id: Some(session.clone()), profile_id: None })
        .await;
    super::host::state().agents.loading = false;
    match r {
        Ok(v) if v.session_id.as_deref() == Some(session.as_str()) => {
            if !store.domains.autonomy.epoch_admits(epoch) || store.domains.autonomy.revision("agents-list") != rev {
                return Ok("superseded".into());
            }
            let base = |s: &str| s.split('#').next().unwrap_or(s).to_owned();
            let agents: Vec<AgentRecord> = v
                .agents
                .iter()
                .filter(|a| a.session_id.0 == session || base(&a.session_id.0) == base(&session))
                .map(agent_record)
                .collect();
            let n = agents.len();
            store.domains.autonomy.set_agents(agents);
            store.domains.autonomy.clear_error(FAMILY);
            Ok(format!("{n} agent(s)"))
        }
        Ok(v) => Err(fail(
            store,
            epoch,
            "agent/list",
            format!("list envelope session {:?} does not exactly match the requested session", v.session_id),
        )),
        Err(e) => Err(fail(store, epoch, "agent/list", e)),
    }
}

fn params(conv: &crate::flow::Conversation, agent_id: &str) -> AgentParams {
    AgentParams { agent_id: agent_id.to_owned(), session_id: Some(session_of(conv)), profile_id: None }
}

/// `agent/status/read` into the detail viewer.
pub async fn read_status(conv: &crate::flow::Conversation, agent_id: String) -> Result<String, String> {
    let store = &conv.store;
    let ticket = store.domains.autonomy.begin_agent_detail();
    match conv.client().call::<AgentStatusRead>(params(conv, &agent_id)).await {
        Ok(v) if v.agent.agent_id == agent_id => {
            let rec = agent_record(&v.agent);
            let applied = store.domains.autonomy.finish_agent_detail(ticket, |vw| vw.status = Some(rec));
            Ok(if applied { "status shown".into() } else { "superseded".into() })
        }
        Ok(v) => {
            store.domains.autonomy.finish_agent_detail(ticket, |_| {});
            Err(fail(store, ticket.epoch, "agent/status/read", format!("returned agent {:?}", v.agent.agent_id)))
        }
        Err(e) => {
            store.domains.autonomy.finish_agent_detail(ticket, |_| {});
            Err(fail(store, ticket.epoch, "agent/status/read", e))
        }
    }
}

/// `agent/output/read` — a fresh read, or a load-more from the viewer's
/// cursor (same agent only). No `limit` is sent (the web sends none).
pub async fn read_output(conv: &crate::flow::Conversation, agent_id: String, more: bool) -> Result<String, String> {
    let store = &conv.store;
    let (ticket, cursor) = store.domains.autonomy.begin_agent_output(&agent_id, more);
    let p = AgentOutputReadParams {
        agent_id: agent_id.clone(),
        session_id: Some(session_of(conv)),
        profile_id: None,
        cursor: cursor.map(|offset| AgentOutputCursor { offset }),
        limit: None,
    };
    match conv.client().call::<AgentOutputRead>(p).await {
        Ok(v) => {
            let view = AgentOutputView {
                agent_id: v.agent_id.clone(),
                text: v.text.clone(),
                next_offset: v.next_cursor.as_ref().map(|c| c.offset),
                has_more: v.has_more,
            };
            let applied = store.domains.autonomy.finish_agent_output(ticket, &agent_id, v.cursor.is_some(), more, view);
            Ok(if applied { format!("{} chars", v.text.len()) } else { "superseded".into() })
        }
        Err(e) => {
            store.domains.autonomy.finish_agent_output(ticket, &agent_id, false, false, AgentOutputView::default());
            Err(fail(store, ticket.epoch, "agent/output/read", e))
        }
    }
}

fn row_of(a: &octos_core::ui_protocol::UiAgentArtifact) -> AgentArtifactRow {
    AgentArtifactRow {
        id: a.id.clone(),
        title: a.title.clone(),
        kind: a.kind.clone(),
        status: a.status.clone(),
        path: a.path.clone(),
    }
}

/// `agent/artifact/list` into the detail viewer.
pub async fn list_artifacts(conv: &crate::flow::Conversation, agent_id: String) -> Result<String, String> {
    let store = &conv.store;
    let ticket = store.domains.autonomy.begin_agent_detail();
    match conv.client().call::<AgentArtifactList>(params(conv, &agent_id)).await {
        Ok(v) if v.agent_id == agent_id => {
            let rows: Vec<AgentArtifactRow> = v.artifacts.iter().map(row_of).collect();
            let n = rows.len();
            let applied = store.domains.autonomy.finish_agent_detail(ticket, |vw| vw.artifacts = Some((agent_id.clone(), rows)));
            Ok(if applied { format!("{n} artifact(s)") } else { "superseded".into() })
        }
        Ok(v) => {
            store.domains.autonomy.finish_agent_detail(ticket, |_| {});
            Err(fail(store, ticket.epoch, "agent/artifact/list", format!("returned agent {:?}", v.agent_id)))
        }
        Err(e) => {
            store.domains.autonomy.finish_agent_detail(ticket, |_| {});
            Err(fail(store, ticket.epoch, "agent/artifact/list", e))
        }
    }
}

/// `agent/artifact/read` with EXACTLY one selector (`artifact_id` or
/// `path`); the echoed artifact must be the requested one.
pub async fn read_artifact(
    conv: &crate::flow::Conversation,
    agent_id: String,
    artifact_id: Option<String>,
    path: Option<String>,
) -> Result<String, String> {
    let store = &conv.store;
    if artifact_id.is_some() == path.is_some() {
        return Err("agent/artifact/read needs exactly one selector".into());
    }
    let ticket = store.domains.autonomy.begin_agent_detail();
    let p = AgentArtifactReadParams {
        agent_id: agent_id.clone(),
        artifact_id: artifact_id.clone(),
        path: path.clone(),
        session_id: Some(session_of(conv)),
        profile_id: None,
    };
    match conv.client().call::<AgentArtifactRead>(p).await {
        Ok(v) => {
            let echo_ok = v.agent_id == agent_id
                && artifact_id.as_deref().is_none_or(|id| v.artifact.id == id)
                && path.as_deref().is_none_or(|p| v.artifact.path.as_deref().is_none_or(|q| q == p));
            if !echo_ok {
                store.domains.autonomy.finish_agent_detail(ticket, |_| {});
                return Err(fail(store, ticket.epoch, "agent/artifact/read", "returned another artifact"));
            }
            let view = AgentArtifactView { agent_id: agent_id.clone(), artifact: row_of(&v.artifact), content: v.content.clone() };
            let applied = store.domains.autonomy.finish_agent_detail(ticket, |vw| vw.artifact = Some(view));
            Ok(if applied { "artifact shown".into() } else { "superseded".into() })
        }
        Err(e) => {
            store.domains.autonomy.finish_agent_detail(ticket, |_| {});
            Err(fail(store, ticket.epoch, "agent/artifact/read", e))
        }
    }
}

/// `agent/interrupt` | `agent/close`: refused locally while that agent has a
/// control in flight; the receipt must agree with the request
/// (`ok`, `status` = the requested transition, the booleans consistent).
pub async fn control(conv: &crate::flow::Conversation, agent_id: String, kind: &'static str) -> Result<String, String> {
    let store = &conv.store;
    let Some(epoch) = store.domains.autonomy.begin_agent_control(&agent_id) else {
        return Ok("already pending".into());
    };
    let want = if kind == "close" { "closed" } else { "interrupted" };
    let method = if kind == "close" { "agent/close" } else { "agent/interrupt" };
    let r = if kind == "close" {
        conv.client().call::<AgentClose>(params(conv, &agent_id)).await
    } else {
        conv.client().call::<AgentInterrupt>(params(conv, &agent_id)).await
    };
    match r {
        Ok(v) if v.ok && v.agent_id == agent_id && v.status == want && (v.interrupted || v.closed || v.already_terminal) => {
            store.domains.autonomy.finish_agent_control(&agent_id, epoch, Some(&v.status));
            Ok(format!("agent {want}"))
        }
        Ok(v) => {
            store.domains.autonomy.finish_agent_control(&agent_id, epoch, None);
            Err(fail(store, epoch, method, format!("the server answered status {:?} ok={}", v.status, v.ok)))
        }
        Err(e) => {
            store.domains.autonomy.finish_agent_control(&agent_id, epoch, None);
            Err(fail(store, epoch, method, e))
        }
    }
}

/// The spawn admission (`agent-spawn-admission.ts:10-43`) and the ordinary
/// queued turn: the composed text goes through `turn/start` exactly like a
/// typed prompt (never an `agent/spawn` RPC). Refused unless the session is
/// live and idle and both `turn/start` and `agent/list` are advertised.
pub async fn spawn(conv: &crate::flow::Conversation, text: String) -> Result<String, String> {
    let store = &conv.store;
    let caps = Caps::of(store);
    let idle = !conv.ui().lock().unwrap().turn_active();
    let admitted = store.is_live() && store.active_session().is_some() && idle && caps.list && caps.turn_start;
    if !admitted {
        let mut st = super::host::state();
        st.agents.spawn_error = Some(SPAWN_REFUSED.into());
        return Err("spawn not admitted".into());
    }
    match conv.start_turn_with_media(text.trim().to_owned(), Vec::new()).await {
        Ok(turn) => {
            let mut st = super::host::state();
            st.agents.task.clear();
            st.agents.task_snap.clear();
            st.agents.spawn_error = None;
            st.agents.form_gen += 1;
            Ok(format!("queued turn {turn}"))
        }
        Err(e) => {
            super::host::state().agents.spawn_error = Some(SPAWN_REFUSED.into());
            Err(e.to_string())
        }
    }
}

// -------------------------------------------------------------------- view

/// A bordered card with a title line ("Status — {id}").
fn card_title(d: &mut Dsl, id: &str, text: &str, inner_w: f64) {
    d.text(id, &ui::fit_w(text, inner_w, 13.0, Face::Semibold), &Txt::new(13.0, Face::Semibold, tok::TEXT).w(W::Fill));
}

/// One `dt`/`dd` pair as a row: the term muted on the left, the value on the
/// right of a fixed term column.
fn meta_row(d: &mut Dsl, id: &str, term: &str, value: &str, inner_w: f64, compact: bool) {
    let row = d.anon();
    if compact {
        d.view(&row, "width: Fill height: Fit flow: Down spacing: 1");
    } else {
        d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.0} spacing: 8");
    }
    let term_w = if compact { W::Fill } else { W::Px(118.0) };
    d.text("", term, &Txt::new(12.0, Face::Regular, tok::MUTED).w(term_w));
    let vw = if compact { inner_w } else { inner_w - 126.0 };
    d.text(id, &ui::fit_w(value, vw * 2.0, 12.5, Face::Regular), &Txt::new(12.5, Face::Regular, tok::TEXT).w(W::Fill).wrap());
    d.close();
}

/// A monospace output block (`<pre>`), wrapped.
fn pre(d: &mut Dsl, id: &str, text: &str) {
    let box_id = format!("{id}_box");
    d.surface(&box_id, "width: Fill height: Fit flow: Down padding: Inset{left: 10 right: 10 top: 8 bottom: 8}", tok::SURFACE2, 8.0, Some(tok::HAIRLINE));
    d.text(id, text.trim_end(), &Txt::new(12.0, Face::Mono, tok::TEXT).w(W::Fill).wrap());
    d.close();
}

/// The action pills for one agent (`actions(agentId, terminal)`,
/// `AgentPanel.tsx:32-97`): each only when its method is advertised;
/// Interrupt/Close disabled while unavailable, pending or terminal; Close is
/// the danger action.
fn actions(d: &mut Dsl, prefix: &str, base: &str, suffix: &str, caps: &Caps, enabled: bool, controllable: bool) {
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right{wrap: true} spacing: 8");
    let ev = |verb: &str| format!("{base}{verb}{suffix}");
    let kind = |on: bool| if on { Btn::Outline } else { Btn::OutlineOff };
    if caps.status {
        d.button(&format!("{prefix}_status"), "Read status", &ev("status"), kind(enabled), W::Fit, 30.0);
    }
    if caps.output {
        d.button(&format!("{prefix}_output"), "Read output", &ev("output"), kind(enabled), W::Fit, 30.0);
    }
    if caps.artifacts {
        d.button(&format!("{prefix}_artifacts"), "List artifacts", &ev("artifacts"), kind(enabled), W::Fit, 30.0);
    }
    if caps.interrupt {
        d.button(&format!("{prefix}_interrupt"), "Interrupt agent", &ev("interrupt"), kind(enabled && controllable), W::Fit, 30.0);
    }
    if caps.close {
        danger_button(d, &format!("{prefix}_close"), "Close agent", &ev("close"), enabled && controllable);
    }
    d.close();
}

/// The web's `.danger` button: red text on a white pill with a red hairline.
fn danger_button(d: &mut Dsl, id: &str, label: &str, event: &str, on: bool) {
    let w = ui::text_w(label, 13.0, Face::Medium) + 32.0;
    let (fg, line) = if on { (tok::RED_TEXT, "#f1b8bcff") } else { (tok::DISABLED_INK, tok::HAIRLINE) };
    d.surface(&format!("{id}_box"), &format!("width: {w} height: 30 flow: Overlay align: Align{{x: 0.5 y: 0.5}}"), tok::SURFACE, 15.0, Some(line));
    let inner = d.anon();
    d.view(&inner, "width: Fill height: Fill flow: Right align: Align{x: 0.5 y: 0.5}");
    d.text(&format!("{id}_label"), label, &Txt::new(13.0, Face::Medium, fg));
    d.close();
    if on {
        d.tap(id, event);
    }
    d.close();
}

/// A labelled one-line input ("Agent count", "Agent ID").
fn field(d: &mut Dsl, label_id: &str, label: &str, input_id: &str, key: &str, value: &str, placeholder: &str, mono: bool, w: W) {
    let col = d.anon();
    d.view(&col, &format!("width: {} height: Fit flow: Down spacing: 6", match w {
        W::Px(v) => format!("{v}"),
        W::Fill => "Fill".into(),
        W::Fit => "Fit".into(),
    }));
    ui::field_label(d, label_id, label);
    d.input(input_id, key, value, placeholder, mono, 36.0);
    d.close();
}

fn status_chip(d: &mut Dsl, id: &str, status: &str) {
    let (fg, bg) = match status {
        "running" | "active" | "started" => (tok::GREEN_TEXT, tok::GREEN_BG),
        "failed" => (tok::RED_TEXT, tok::RED_BG),
        "interrupted" | "closed" | "completed" => (tok::MUTED, tok::SURFACE2),
        _ => (tok::AMBER, tok::AMBER_BG),
    };
    d.chip(id, status, fg, bg, None, false);
}

/// The panel. Section order is the web's: the spawn form, the roster, the
/// by-ID inspector, the busy line, then the status / artifacts / artifact /
/// output cards, and the alert line.
pub fn build(d: &mut Dsl, st: &AgentsState, frame: &Frame, store: &Store) {
    let width = frame.dialog_w(720.0);
    let compact = frame.compact(width);
    let pad = ui::dialog_pad(frame, width);
    let inner_w = width - 2.0 * pad - 10.0 - 28.0; // scroll gutter + card padding
    let caps = Caps::of(store);
    let viewer: AgentViewer = store.domains.autonomy.agent_viewer();
    let agents = store.domains.autonomy.agents();
    ui::shell_open(d, frame, width);
    // Header: "Agents" + refresh + close; the session it lists under it.
    let row = d.anon();
    d.view(&row, "width: Fill height: 32 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 4");
    d.text("b3_title", "Agents", &ui::title().w(W::Fill));
    if caps.list {
        ui::icon_button(d, "b3_agents_refresh", "b3_refresh.svg", 16.0, "b3.agents.refresh");
    }
    ui::close_glyph(d, "b3.close");
    d.close();
    let session = store.active_session().unwrap_or_default();
    d.text("b3_agents_scope", &session, &ui::micro().w(W::Fill));
    d.gap(W::Fill, 12.0);
    ui::body_open(d, frame, width, 60.0);
    if !caps.any() {
        d.text(
            "b3_agents_unsupported",
            "This server does not advertise agent controls.",
            &ui::meta().w(W::Fill).wrap(),
        );
        ui::body_close(d);
        ui::shell_close(d);
        return;
    }
    // ---- Request parallel agents (`AgentPanel.tsx:102-164`).
    if caps.list && caps.turn_start {
        ui::card_open(d, "b3_agents_spawn", 10.0);
        ui::section_title(d, "b3_agents_spawn_title", "Request parallel agents");
        let fields = format!("b3_agents_form_{}", st.form_gen);
        if compact {
            d.view(&fields, "width: Fill height: Fit flow: Down spacing: 10");
            field(d, "", "Agent count", "b3_agents_count", "agents.count", &st.count_snap, "1", false, W::Fill);
        } else {
            d.view(&fields, "width: Fill height: Fit flow: Right spacing: 10");
            field(d, "", "Agent count", "b3_agents_count", "agents.count", &st.count_snap, "1", false, W::Px(110.0));
        }
        field(d, "", "Agent task", "b3_agents_task", "agents.task", &st.task_snap, "Describe the task for the agents", false, W::Fill);
        d.close();
        d.text("b3_agents_spawn_hint", SPAWN_HINT, &ui::meta().w(W::Fill).wrap());
        if let Some(e) = &st.spawn_error {
            d.text("b3_agents_spawn_error", e, &Txt::new(12.0, Face::Regular, tok::RED_TEXT).w(W::Fill).wrap());
        }
        // Both variants are emitted; the live gate shows one ([`visibility`]:
        // no remount while typing, which would rebuild the focused input).
        let foot = d.anon();
        d.view(&foot, "width: Fill height: Fit flow: Overlay align: Align{x: 1.0 y: 0.5}");
        d.view("b3_agents_spawn_off", "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5}");
        d.button("b3_agents_spawn_disabled", "Request parallel agents", "b3.agents.spawn", Btn::Disabled, W::Fit, 34.0);
        d.close();
        d.view("b3_agents_spawn_on", "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5}");
        d.button("b3_agents_spawn_go", "Request parallel agents", "b3.agents.spawn", Btn::Primary, W::Fit, 34.0);
        d.close();
        d.close();
        d.close();
        d.gap(W::Fill, 12.0);
    }
    // ---- The roster (`:165-195`).
    if caps.list {
        ui::field_label(d, "b3_agents_roster_label", "Agents");
        if st.loading && agents.is_empty() {
            d.text("b3_agents_loading", "Loading agents…", &ui::meta());
        } else if agents.is_empty() {
            d.text("b3_agents_empty", EMPTY, &ui::meta().w(W::Fill));
        }
        for (i, a) in agents.iter().enumerate() {
            let id = format!("b3_agents_row_{i}");
            ui::card_open(d, &id, 8.0);
            let head = d.anon();
            d.view(&head, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
            d.text(&format!("{id}_name"), &ui::fit_w(&a.nickname, inner_w - 120.0, 13.0, Face::Semibold), &Txt::new(13.0, Face::Semibold, tok::TEXT).w(W::Fill));
            status_chip(d, &format!("{id}_status"), &a.status);
            d.close();
            meta_row(d, &format!("{id}_role"), "Role", &a.role, inner_w, compact);
            let last = a.last_task.clone().or_else(|| a.title.clone()).unwrap_or_else(|| "—".into());
            meta_row(d, &format!("{id}_last"), "Last task", &last, inner_w, compact);
            if let Some(tail) = a.output_tail.as_deref().filter(|t| !t.trim().is_empty()) {
                pre(d, &format!("{id}_tail"), tail);
            }
            let pending = viewer.pending.iter().any(|p| p == &a.agent_id);
            let terminal = TERMINAL.contains(&a.status.as_str());
            actions(d, &format!("{id}_act"), "b3.agents.", &format!("#{i}"), &caps, !a.agent_id.trim().is_empty(), !pending && !terminal);
            d.close();
        }
        d.gap(W::Fill, 4.0);
    }
    // ---- Inspect or control an agent by ID (`:196-232`), closed by default.
    d.view("b3_agents_by_id_head", "width: Fill height: 30 flow: Overlay");
    let h = d.anon();
    d.view(&h, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} spacing: 6");
    d.icon("", if st.by_id_open { "b3_chevron_down_dark.svg" } else { "b3_chevron_right_dark.svg" }, 13.0, tok::TEXT);
    d.text("b3_agents_by_id_label", "Inspect or control an agent by ID", &Txt::new(13.0, Face::Regular, tok::TEXT));
    d.close();
    d.tap("b3_agents_by_id", "b3.agents.by_id");
    d.close();
    if st.by_id_open {
        ui::card_open(d, "b3_agents_by_id_card", 10.0);
        field(d, "", "Agent ID", "b3_agents_query", "agents.id", &st.query_snap, "agent id", true, W::Fill);
        // The id's actions are enabled only for a non-blank id: both rows are
        // emitted and the live gate shows one (no remount while typing).
        let both = d.anon();
        d.view(&both, "width: Fill height: Fit flow: Overlay");
        d.view("b3_agents_id_off", "width: Fill height: Fit flow: Down");
        actions(d, "b3_agents_id_off", "b3.agents.id.", "", &caps, false, true);
        d.close();
        d.view("b3_agents_id_on", "width: Fill height: Fit flow: Down");
        actions(d, "b3_agents_id_act", "b3.agents.id.", "", &caps, true, true);
        d.close();
        d.close();
        if caps.artifact_read {
            field(d, "", "Artifact path", "b3_agents_path", "agents.path", &st.path_snap, "path/to/artifact", true, W::Fill);
            let foot = d.anon();
            d.view(&foot, "width: Fill height: Fit flow: Overlay");
            d.view("b3_agents_path_off", "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5}");
            d.button("b3_agents_read_path_disabled", "Read artifact by path", "b3.agents.id.read_path", Btn::OutlineOff, W::Fit, 30.0);
            d.close();
            d.view("b3_agents_path_on", "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5}");
            d.button("b3_agents_read_path", "Read artifact by path", "b3.agents.id.read_path", Btn::Outline, W::Fit, 30.0);
            d.close();
            d.close();
        }
        d.close();
    }
    if viewer.detail_busy {
        d.text("b3_agents_detail_busy", "Reading agent details…", &ui::meta());
    }
    if let Some(a) = &viewer.status {
        d.gap(W::Fill, 8.0);
        ui::card_open(d, "b3_agents_status_card", 6.0);
        card_title(d, "b3_agents_status_title", &format!("Status — {}", a.agent_id), inner_w);
        meta_row(d, "b3_agents_status_status", "Status", &a.status, inner_w, compact);
        meta_row(d, "b3_agents_status_session", "Owner session", &a.session_id, inner_w, compact);
        meta_row(d, "b3_agents_status_profile", "Profile", &a.profile_id, inner_w, compact);
        meta_row(d, "b3_agents_status_backend", "Backend", &a.backend_kind, inner_w, compact);
        meta_row(d, "b3_agents_status_count", "Artifacts", &a.artifact_count.to_string(), inner_w, compact);
        if let Some(s) = a.summary.as_deref().filter(|s| !s.trim().is_empty()) {
            d.text("b3_agents_status_summary", s, &ui::body().w(W::Fill).wrap());
        }
        d.close();
    }
    if let Some((agent, rows)) = &viewer.artifacts {
        d.gap(W::Fill, 8.0);
        ui::card_open(d, "b3_agents_artifacts_card", 6.0);
        card_title(d, "b3_agents_artifacts_title", &format!("Artifacts — {agent}"), inner_w);
        if rows.is_empty() {
            d.text("b3_agents_artifacts_empty", "No artifacts available.", &ui::meta().w(W::Fill));
        }
        for (j, r) in rows.iter().enumerate() {
            let line = d.anon();
            d.view(&line, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
            let text = format!("{} — {} · {}", r.title, r.kind, r.status);
            d.text(&format!("b3_agents_artifact_{j}"), &ui::fit_w(&text, inner_w - 130.0, 12.5, Face::Regular), &Txt::new(12.5, Face::Regular, tok::TEXT).w(W::Fill));
            if caps.artifact_read {
                d.button(&format!("b3_agents_artifact_read_{j}"), "Read artifact", &format!("b3.agents.artifact#{j}"), Btn::Outline, W::Fit, 30.0);
            }
            d.close();
        }
        d.close();
    }
    if let Some(v) = &viewer.artifact {
        d.gap(W::Fill, 8.0);
        ui::card_open(d, "b3_agents_artifact_card", 6.0);
        card_title(d, "b3_agents_artifact_title", &format!("Artifact — {} / {}", v.agent_id, v.artifact.id), inner_w);
        d.text("b3_agents_artifact_name", &v.artifact.title, &ui::body().w(W::Fill).wrap());
        match &v.content {
            Some(c) => pre(d, "b3_agents_artifact_content", c),
            None => d.text("b3_agents_artifact_none", "No readable content available.", &ui::meta()),
        }
        d.close();
    }
    if let Some(o) = &viewer.output {
        d.gap(W::Fill, 8.0);
        ui::card_open(d, "b3_agents_output_card", 6.0);
        card_title(d, "b3_agents_output_title", &format!("Output — {}", o.agent_id), inner_w);
        pre(d, "b3_agents_output_text", if o.text.is_empty() { " " } else { &o.text });
        if o.has_more {
            let foot = d.anon();
            d.view(&foot, "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5}");
            d.button(
                "b3_agents_output_more",
                "Load more",
                "b3.agents.output_more",
                if viewer.output_busy { Btn::OutlineOff } else { Btn::Outline },
                W::Fit,
                30.0,
            );
            d.close();
        }
        d.close();
    }
    if let Some(a) = &viewer.activity {
        d.text("b3_agents_activity", a, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill));
    }
    if let Some(e) = store.domains.autonomy.error(FAMILY) {
        d.gap(W::Fill, 6.0);
        ui::error_line(d, "b3_agents_error", failure_lead(&e), &e);
    }
    ui::body_close(d);
    ui::shell_close(d);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A13 — a failed agents read leads with which read failed, in plain
    /// words; the family error (method-prefixed) stays under it, muted.
    #[test]
    fn a_failed_agents_read_names_the_read_in_plain_words() {
        assert_eq!(failure_lead("agent/list: rpc error -32601 (method not found)"), "Couldn't load this session's agents.");
        assert_eq!(failure_lead("agent/output/read: transport: channel closed"), "Couldn't read the agent's output.");
        assert_eq!(failure_lead("agent/status/read: returned agent \"a2\""), "Couldn't read the agent's status.");
        assert_eq!(failure_lead("agent/artifact/read: returned another artifact"), "Couldn't open the agent's artifact.");
        let mut d = Dsl::new();
        ui::error_line(&mut d, "b3_agents_error", failure_lead("agent/list: rpc error -32601 (x)"), "agent/list: rpc error -32601 (x)");
        let dsl = d.finish();
        assert!(dsl.contains("Couldn't load this session's agents.") && dsl.contains("b3_agents_error_detail"), "{dsl}");
    }

    #[test]
    fn the_spawn_text_is_the_webs() {
        assert_eq!(
            compose_spawn("3", "  audit the parser\nand the lexer  ").as_deref(),
            Some("Spawn 3 agent(s) to accomplish in parallel: audit the parser\nand the lexer")
        );
        assert_eq!(compose_spawn("0", "x"), None);
        assert_eq!(compose_spawn("1.5", "x"), None);
        assert_eq!(compose_spawn("4294967295", "x").is_some(), true, "the native u32 boundary");
        assert_eq!(compose_spawn("4294967296", "x"), None);
        assert_eq!(compose_spawn("2", "   "), None);
    }

    fn caps_store() -> Store {
        let store = Store::new();
        store.set_capabilities(vec![FEATURE.into()]);
        store.domains.config.set_supported_methods(
            ["agent/list", "agent/status/read", "agent/output/read", "agent/artifact/list", "agent/artifact/read", "agent/interrupt", "agent/close", "turn/start"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
        store
    }

    #[test]
    fn each_control_is_gated_on_its_method_and_the_feature() {
        let store = Store::new();
        store.domains.config.set_supported_methods(vec!["agent/list".into()]);
        assert!(!Caps::of(&store).list, "the method alone is not enough");
        store.set_capabilities(vec![FEATURE.into()]);
        let c = Caps::of(&store);
        assert!(c.list && !c.close && !c.status);
        let full = Caps::of(&caps_store());
        assert!(full.any() && full.close && full.interrupt && full.turn_start);
    }

    #[test]
    fn the_panel_lowers_balanced_with_every_control_wired() {
        let store = caps_store();
        store.set_active(Some("dsflash:main".into()));
        store.domains.autonomy.set_agents(vec![AgentRecord {
            agent_id: "agent-1".into(),
            session_id: "dsflash:main".into(),
            profile_id: "dsflash".into(),
            path: "/a".into(),
            role: "background_task".into(),
            nickname: "c24b-probe".into(),
            backend_kind: "spawn_child_session".into(),
            status: "running".into(),
            title: Some("c24b-probe".into()),
            parent_agent_id: None,
            task_id: None,
            artifact_count: 0,
            output_tail: Some("line 1\nline 2".into()),
            updated_at_ms: 1,
            last_task: Some("c24b-probe running".into()),
            summary: None,
        }]);
        let mut st = AgentsState { task: "audit".into(), ..Default::default() };
        st.snap();
        st.by_id_open = true;
        st.query_id = "agent-1".into();
        let mut d = Dsl::new();
        build(&mut d, &st, &Frame::DESKTOP, &store);
        let dsl = d.finish();
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
        let taps: Vec<String> = crate::screens::taps::wired_taps(&dsl).into_iter().map(|(_, e)| e).collect();
        for want in [
            "b3.agents.spawn", "b3.agents.status#0", "b3.agents.output#0", "b3.agents.artifacts#0",
            "b3.agents.interrupt#0", "b3.agents.close#0", "b3.agents.by_id", "b3.agents.id.status",
            "b3.agents.refresh", "b3.close",
        ] {
            assert!(taps.iter().any(|t| t == want), "{want} not wired: {taps:?}");
        }
        assert!(dsl.contains("c24b-probe running") && dsl.contains("Request parallel agents"));
        // A terminal agent cannot be interrupted or closed.
        let mut a = store.domains.autonomy.agents()[0].clone();
        a.status = "closed".into();
        store.domains.autonomy.set_agents(vec![a]);
        let mut d = Dsl::new();
        build(&mut d, &st, &Frame { avail_w: 360.0, avail_h: 776.0 }, &store);
        let taps: Vec<String> = crate::screens::taps::wired_taps(&d.finish()).into_iter().map(|(_, e)| e).collect();
        assert!(!taps.iter().any(|t| t == "b3.agents.close#0" || t == "b3.agents.interrupt#0"), "{taps:?}");
        assert!(taps.iter().any(|t| t == "b3.agents.status#0"));
    }

    #[test]
    fn actions_route_to_their_jobs_and_refusals_stay_local() {
        let store = caps_store();
        store.domains.autonomy.set_agents(vec![AgentRecord {
            agent_id: "agent-1".into(),
            session_id: "s".into(),
            profile_id: "p".into(),
            path: "/a".into(),
            role: "r".into(),
            nickname: "n".into(),
            backend_kind: "b".into(),
            status: "running".into(),
            title: None,
            parent_agent_id: None,
            task_id: None,
            artifact_count: 0,
            output_tail: None,
            updated_at_ms: 1,
            last_task: None,
            summary: None,
        }]);
        let mut st = AgentsState::default();
        assert_eq!(perform(&mut st, "b3.agents.status", 0, &store), Outcome::Spawn(Job::AgentStatus("agent-1".into())));
        assert_eq!(perform(&mut st, "b3.agents.close", 0, &store), Outcome::Spawn(Job::AgentControl("agent-1".into(), "close")));
        // A second click while the close is pending is refused locally.
        assert!(store.domains.autonomy.begin_agent_control("agent-1").is_some());
        assert_eq!(perform(&mut st, "b3.agents.close", 0, &store), Outcome::Done);
        // The by-ID path addresses the typed id.
        st.query_id = "agent-9".into();
        assert_eq!(perform(&mut st, "b3.agents.id.output", 0, &store), Outcome::Spawn(Job::AgentOutput("agent-9".into(), false)));
        // Spawn: an invalid count is refused on the form; a busy session too.
        st.count = "0".into();
        st.task = "x".into();
        assert_eq!(perform(&mut st, "b3.agents.spawn", 0, &store), Outcome::Done);
        assert_eq!(st.spawn_error.as_deref(), Some(SPAWN_INVALID));
        st.count = "2".into();
        st.turn_busy = true;
        assert_eq!(perform(&mut st, "b3.agents.spawn", 0, &store), Outcome::Done);
        assert_eq!(st.spawn_error.as_deref(), Some(SPAWN_REFUSED));
        st.turn_busy = false;
        assert_eq!(
            perform(&mut st, "b3.agents.spawn", 0, &store),
            Outcome::Spawn(Job::AgentsSpawn("Spawn 2 agent(s) to accomplish in parallel: x".into()))
        );
    }
}
