//! A9 — **Activity**: the operator-opened, read-only scan of background tasks
//! across the confirmed sessions of the active Profile.
//!
//! ## Web oracle (behaviour and copy; cited)
//!
//! - Opened only on demand (`ActivityDialog.tsx:10` "Load cross-session
//!   scanning only when the operator opens Activity"), from the `/activity`
//!   command (`registry.ts:289`, alias `act`; `App.tsx:1336`).
//! - The catalog (`catalog.ts:12-38`): one `task/list {session_id}` per
//!   ALREADY-CONFIRMED session — never `session/open` — in batches of four
//!   concurrent reads with no session cap; a reply naming another session, or
//!   a failed read, puts that session in `unavailableSessions` (fail closed).
//! - The targets (`App.tsx:771-776`): the navigable sessions of the active
//!   Profile, each id once. Natively that is the store's `session/list` rows
//!   scoped `<profile>:<chat>` (the scope rule `board3::resume` uses).
//! - Refresh (`use-activity-catalog.ts:43-76`): only while open and only when
//!   the server advertises `task/list`; the next read starts 10 s after the
//!   previous one settled; a closed dialog never publishes a late reply.
//! - The model (`workspace/model.ts:93-170`): state mapping
//!   pending|running → running, failed|cancelled → failed, completed → done;
//!   title = summary ‖ role ‖ tool name; detail = role · phase · status ·
//!   error; sort running < failed < unknown < done < idle, then newest
//!   `updated_at`, then title; counts all/running/failed/done; search over
//!   session id/label, task id, title, detail, tool and state.
//! - The navigator (`ActivityNavigator.tsx:20-195`): eyebrow, title, scope
//!   line, close; search + All/Running/Failed/Done with counts; the
//!   blocked-switch warning; one row per task (state dot, title, session
//!   label, detail · time) with **Inspect** on the current session's rows and
//!   **Open session** on the others; the empty and unavailable states; the
//!   footer ("Read-only · refreshes every 10 seconds" / "Esc closes").
//!
//! ## Look
//!
//! No approved board covers Activity, so the surface is built with the
//! native dialog kit A4's board-3 dialogs use (`board3::ui`: the same type
//! ramp, tokens, hairlines, pills and segmented control) over the web's
//! layout (`app/styles.css:1382-1615`): a `min(900px, 100%) ×
//! min(720px, 100%)` card with a fixed header / toolbar / scrolling results /
//! footer, full-bleed on a phone (`styles.css:2121-2147`).
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use octos_core::ui_protocol::{TaskListEntry, TaskListParams, TaskRuntimeState};
use octoscode_client::domains::task::TaskList;
use octoscode_store::Store;

use super::board3::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};
use crate::i18n::{tr, tr1, tr_with};

// ----------------------------------------------------------------- the model

/// The status filter (`workspace/model.ts:75`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    Running,
    Failed,
    Done,
}

impl Filter {
    pub const ALL: [Filter; 4] = [Filter::All, Filter::Running, Filter::Failed, Filter::Done];

    pub fn id(self) -> &'static str {
        match self {
            Filter::All => "all",
            Filter::Running => "running",
            Filter::Failed => "failed",
            Filter::Done => "done",
        }
    }

    /// The button label (`text-transform: capitalize`, styles.css:1472).
    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::Running => "Running",
            Filter::Failed => "Failed",
            Filter::Done => "Done",
        }
    }

    pub fn from_id(id: &str) -> Option<Filter> {
        Filter::ALL.into_iter().find(|f| f.id() == id)
    }
}

/// A row's state (`SessionActivityStatus`, `workspace/model.ts:8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    Running,
    Failed,
    Unknown,
    Done,
    Idle,
}

impl RowState {
    /// The sort priority (`model.ts:146`).
    fn priority(self) -> u8 {
        match self {
            RowState::Running => 0,
            RowState::Failed => 1,
            RowState::Unknown => 2,
            RowState::Done => 3,
            RowState::Idle => 4,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            RowState::Running => "running",
            RowState::Failed => "failed",
            RowState::Unknown => "unknown",
            RowState::Done => "done",
            RowState::Idle => "idle",
        }
    }

    /// The state dot (`styles.css:1544-1563`): running = the info accent,
    /// failed/unknown = the error red, done = the success green, else grey.
    fn dot(self) -> &'static str {
        match self {
            RowState::Running => tok::BLUE,
            RowState::Failed | RowState::Unknown => tok::RED,
            RowState::Done => tok::GREEN,
            RowState::Idle => tok::FAINT,
        }
    }
}

/// The task fields the projection reads (a [`TaskListEntry`] subset, so a
/// recorded reply and a test fixture project identically).
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub id: String,
    pub tool_name: String,
    /// The wire `state` (`pending|running|completed|failed|cancelled`).
    pub state: String,
    pub status: String,
    pub role: Option<String>,
    pub summary: Option<String>,
    pub current_phase: Option<String>,
    pub error: Option<String>,
    /// RFC 3339.
    pub updated_at: Option<String>,
}

impl Task {
    pub fn from_entry(e: &TaskListEntry) -> Task {
        let state = match e.state {
            TaskRuntimeState::Pending => "pending",
            TaskRuntimeState::Running => "running",
            TaskRuntimeState::Completed => "completed",
            TaskRuntimeState::Failed => "failed",
            TaskRuntimeState::Cancelled => "cancelled",
        };
        Task {
            id: e.id.to_string(),
            tool_name: e.tool_name.clone(),
            state: state.to_owned(),
            status: e.status.clone(),
            role: e.role.clone(),
            summary: e.summary.clone(),
            current_phase: e.current_phase.clone(),
            error: e.error.clone(),
            updated_at: Some(e.updated_at.to_rfc3339()),
        }
    }
}

/// `taskActivityStatus` (`model.ts:165-170`).
pub fn task_state(t: &Task) -> RowState {
    match t.state.as_str() {
        "pending" | "running" => RowState::Running,
        "failed" | "cancelled" => RowState::Failed,
        "completed" => RowState::Done,
        _ => RowState::Unknown,
    }
}

/// One projected row (`WorkspaceActivityRow`, `model.ts:77-86`).
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub session_id: String,
    pub session_title: String,
    pub task_id: String,
    pub title: String,
    pub detail: String,
    pub state: RowState,
    pub updated_at: Option<String>,
    search: String,
}

/// The projected catalog: the filtered rows and the unfiltered counts.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Model {
    pub rows: Vec<Row>,
    /// all / running / failed / done.
    pub counts: [usize; 4],
}

fn non_empty(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// Every row, sorted, before any filter (`model.ts:101-155`).
pub fn all_rows(tasks: &BTreeMap<String, Vec<Task>>, labels: &BTreeMap<String, String>) -> Vec<Row> {
    let mut rows: Vec<Row> = tasks
        .iter()
        .flat_map(|(session_id, list)| {
            let session_title = labels.get(session_id).cloned().unwrap_or_else(|| session_id.clone());
            list.iter().map(move |t| {
                let title = non_empty(&t.summary)
                    .or_else(|| non_empty(&t.role))
                    .unwrap_or(&t.tool_name)
                    .to_owned();
                let detail = [t.role.as_deref(), t.current_phase.as_deref(), Some(t.status.as_str()), t.error.as_deref()]
                    .into_iter()
                    .flatten()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                let search = [
                    session_id.as_str(),
                    session_title.as_str(),
                    t.id.as_str(),
                    title.as_str(),
                    detail.as_str(),
                    t.tool_name.as_str(),
                    t.state.as_str(),
                ]
                .join(" ")
                .to_lowercase();
                Row {
                    session_id: session_id.clone(),
                    session_title: session_title.clone(),
                    task_id: t.id.clone(),
                    title,
                    detail,
                    state: task_state(t),
                    updated_at: t.updated_at.clone(),
                    search,
                }
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        a.state
            .priority()
            .cmp(&b.state.priority())
            .then_with(|| {
                let at = a.updated_at.as_deref().and_then(ui::parse_iso_ms);
                let bt = b.updated_at.as_deref().and_then(ui::parse_iso_ms);
                match (at, bt) {
                    (Some(x), Some(y)) => y.cmp(&x),
                    _ => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                }
            })
    });
    rows
}

/// Whether `row` passes the free-text search (`model.ts:157-163`).
pub fn matches(row: &Row, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || row.search.contains(&q)
}

/// `buildWorkspaceActivityModel` (`model.ts:93-163`).
pub fn build_model(
    tasks: &BTreeMap<String, Vec<Task>>,
    labels: &BTreeMap<String, String>,
    query: &str,
    filter: Filter,
) -> Model {
    let rows = all_rows(tasks, labels);
    let count = |s: RowState| rows.iter().filter(|r| r.state == s).count();
    let counts = [rows.len(), count(RowState::Running), count(RowState::Failed), count(RowState::Done)];
    let rows = rows
        .into_iter()
        .filter(|r| filter_ok(r, filter) && matches(r, query))
        .collect();
    Model { rows, counts }
}

fn filter_ok(r: &Row, f: Filter) -> bool {
    match f {
        Filter::All => true,
        Filter::Running => r.state == RowState::Running,
        Filter::Failed => r.state == RowState::Failed,
        Filter::Done => r.state == RowState::Done,
    }
}

// --------------------------------------------------------------- the catalog

/// What one catalog read produced (`ActivityCatalogResult`, `catalog.ts:6-9`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catalog {
    pub tasks_by_session: BTreeMap<String, Vec<Task>>,
    pub unavailable: Vec<String>,
}

/// Concurrent reads per batch (`catalog.ts:23`).
pub const BATCH: usize = 4;
/// The auto-refresh period (`use-activity-catalog.ts:66`).
pub const REFRESH: Duration = Duration::from_secs(10);

/// `readActivityCatalog` (`catalog.ts:12-38`) on the production client: one
/// `task/list {session_id}` per unique confirmed id, [`BATCH`] at a time, no
/// cap on the number of sessions. A failed read or a reply that names a
/// different session fails closed into `unavailable`. Never opens a session.
pub async fn read_catalog(client: &octoscode_client::Client, ids: &[String]) -> Catalog {
    let mut seen = HashSet::new();
    let unique: Vec<String> = ids.iter().filter(|id| seen.insert(id.as_str())).cloned().collect();
    let mut out = Catalog::default();
    for chunk in unique.chunks(BATCH) {
        let handles: Vec<_> = chunk
            .iter()
            .map(|id| {
                let client = client.clone();
                let id = id.clone();
                tokio::spawn(async move {
                    let reply = client
                        .call::<TaskList>(TaskListParams {
                            session_id: octos_core::SessionKey(id.clone()),
                            topic: None,
                        })
                        .await;
                    (id, reply)
                })
            })
            .collect();
        for (h, id) in handles.into_iter().zip(chunk) {
            match h.await {
                Ok((id, Ok(reply))) if reply.session_id.0 == id => {
                    out.tasks_by_session
                        .insert(id, reply.tasks.iter().map(Task::from_entry).collect());
                }
                // A wrong-session snapshot or a failed read (`catalog.ts:28-32`).
                _ => out.unavailable.push(id.clone()),
            }
        }
    }
    out
}

/// The confirmed targets (`App.tsx:771-776`): the store's `session/list` rows
/// scoped to the active Profile (`<profile>:<chat>`), each id once, with the
/// display label the sidebar shows (`Session::display_label`).
pub fn targets(store: &Store, profile: &str) -> Vec<(String, String)> {
    if profile.is_empty() {
        return Vec::new();
    }
    let prefix = format!("{profile}:");
    let mut seen = HashSet::new();
    store
        .sessions()
        .into_iter()
        .filter(|s| s.id.starts_with(&prefix) && s.id.len() > prefix.len())
        .filter(|s| seen.insert(s.id.clone()))
        .map(|s| (s.id.clone(), s.display_label()))
        .collect()
}

/// `supportsMethod(capabilities, TASK_LIST)` (`use-activity-catalog.ts:37`).
pub fn advertised(store: &Store) -> bool {
    store.domains.config.supported_methods().iter().any(|m| m == "task/list")
}

/// Inspecting the current session's task needs a live connection and the
/// task output or artifact read (`App.tsx:3650-3655`).
pub fn inspect_available(store: &Store) -> bool {
    let m = store.domains.config.supported_methods();
    store.is_live()
        && !switch_blocked()
        && m.iter().any(|x| x == "task/output/read" || x == "task/artifact/list")
}

// ---------------------------------------------------------------- the state

/// The dialog's UI state (values the protocol never carries).
#[derive(Debug, Clone)]
pub struct ActState {
    pub open: bool,
    /// Bumped by every open and close: a read started under another
    /// generation never publishes (the effect cleanup's `current = false`).
    pub gen: u64,
    /// The live search text (typing never remounts the input).
    pub query: String,
    /// The text the input had when the dialog was last lowered.
    pub query_snap: String,
    pub filter: Filter,
    /// `task/list` is advertised.
    pub available: bool,
    pub loading: bool,
    pub tasks: BTreeMap<String, Vec<Task>>,
    pub labels: BTreeMap<String, String>,
    pub unavailable: Vec<String>,
    /// `"N Session task snapshots unavailable"` (`use-activity-catalog.ts:61-63`).
    pub error: Option<String>,
    /// Completed reads (diagnostics; a test can wait on it).
    pub reads: u64,
    /// The rows the last lowering showed, in order: a row tap resolves
    /// against what the operator SAW, not a newer refresh.
    pub shown: Vec<Row>,
    pub frame: Frame,
}

impl Default for ActState {
    fn default() -> Self {
        ActState {
            open: false,
            gen: 0,
            query: String::new(),
            query_snap: String::new(),
            filter: Filter::All,
            available: false,
            loading: false,
            tasks: BTreeMap::new(),
            labels: BTreeMap::new(),
            unavailable: Vec::new(),
            error: None,
            reads: 0,
            shown: Vec::new(),
            frame: Frame::DESKTOP,
        }
    }
}

static STATE: OnceLock<Mutex<ActState>> = OnceLock::new();

/// The host reports the module's laid-out size; the phone check
/// (`OCTOSENSE_WINDOW_SIZE=360x780`) caps it the way `board3::host::set_frame`
/// does, so a desktop run lays the dialog out exactly as the phone does.
pub fn set_frame(w: f64, h: f64) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let (mut w, mut h) = (w, h);
    if let Ok(sz) = std::env::var("OCTOSENSE_WINDOW_SIZE") {
        if let Some((a, b)) = sz.split_once('x') {
            if let (Ok(a), Ok(b)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
                w = w.min(a);
                h = h.min(b);
            }
        }
    }
    state().frame = Frame { avail_w: w, avail_h: h };
}

pub fn state() -> MutexGuard<'static, ActState> {
    STATE
        .get_or_init(|| Mutex::new(ActState::default()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

pub fn is_open() -> bool {
    state().open
}

/// Test seam.
pub fn reset() {
    let frame = state().frame;
    *state() = ActState { frame, ..ActState::default() };
    SWITCHING.store(0, Ordering::SeqCst);
}

/// In-flight session switches (the web's `workspaceProduct.transitioning`,
/// `use-octos-session.ts:1272`): while one is pending, opening another
/// session from Activity is refused with the warning.
static SWITCHING: AtomicUsize = AtomicUsize::new(0);

pub fn note_switch_started() {
    SWITCHING.fetch_add(1, Ordering::SeqCst);
}

pub fn note_switch_finished() {
    let _ = SWITCHING.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| Some(n.saturating_sub(1)));
}

pub fn switch_blocked() -> bool {
    SWITCHING.load(Ordering::SeqCst) > 0
}

/// Open the dialog (`setState({...EMPTY, activityAvailable})`, :49): a fresh
/// generation, an empty catalog, All, no query. Returns the generation to
/// read under when `task/list` is advertised (the host starts the refresh
/// loop); `None` = nothing to read (the dialog shows why).
pub fn open(store: &Store) -> Option<u64> {
    let mut st = state();
    let frame = st.frame;
    let gen = st.gen + 1;
    let available = advertised(store);
    *st = ActState { open: true, gen, available, frame, ..ActState::default() };
    available.then_some(gen)
}

/// Close (Escape, ×, the backdrop): the in-flight loop stops publishing.
pub fn close() {
    let mut st = state();
    st.open = false;
    st.gen += 1;
    st.shown.clear();
}

/// Whether a loop started under `gen` may still run.
pub fn is_current(gen: u64) -> bool {
    let st = state();
    st.open && st.gen == gen
}

fn mark_loading(gen: u64) -> bool {
    let mut st = state();
    if !(st.open && st.gen == gen) {
        return false;
    }
    st.loading = true;
    true
}

/// Publish one read (`use-activity-catalog.ts:56-65`); refused (false) when the
/// dialog closed or reopened since the read started.
pub fn publish(gen: u64, cat: Catalog, labels: BTreeMap<String, String>) -> bool {
    let mut st = state();
    if !(st.open && st.gen == gen) {
        return false;
    }
    st.loading = false;
    st.available = true;
    st.error = (!cat.unavailable.is_empty())
        .then(|| format!("{} Session task snapshots unavailable", cat.unavailable.len()));
    st.tasks = cat.tasks_by_session;
    st.unavailable = cat.unavailable;
    st.labels = labels;
    st.reads += 1;
    // The remount this publish causes carries the text being typed.
    st.query_snap = st.query.clone();
    true
}

/// One read of the catalog under `gen` on the production client (the host's
/// loop calls this every [`REFRESH`]). `Ok(false)` = superseded: stop.
pub async fn read_once(conv: &crate::flow::Conversation, gen: u64) -> bool {
    let targets = targets(&conv.store, &conv.profile());
    if !mark_loading(gen) {
        return false;
    }
    wake();
    let ids: Vec<String> = targets.iter().map(|(id, _)| id.clone()).collect();
    let labels: BTreeMap<String, String> = targets.into_iter().collect();
    let cat = read_catalog(conv.client(), &ids).await;
    let ok = publish(gen, cat, labels);
    wake();
    ok
}

/// The refresh loop: read now, then every `period` after the previous read
/// settled, until the dialog closes or reopens (`setTimeout(refresh, 10000)`
/// after each read, `use-activity-catalog.ts:66`).
pub async fn refresh_loop(conv: std::sync::Arc<crate::flow::Conversation>, gen: u64, period: Duration) {
    while read_once(&conv, gen).await {
        tokio::time::sleep(period).await;
        if !is_current(gen) {
            break;
        }
    }
}

fn wake() {
    makepad_widgets::SignalToUI::set_ui_signal();
}

// ------------------------------------------------------------------ actions

pub const ACTION_OPEN: &str = "activity.open";
pub const ACTION_CLOSE: &str = "a9.act.close";
pub const ACTION_FILTER: &str = "a9.act.filter.";
pub const ACTION_ROW: &str = "a9.act.row";
/// The backdrop closes (`closeOnBackdrop`, ActivityNavigator.tsx:52).
pub const ACTION_BACKDROP: &str = "a9.act.backdrop";
/// The search field's input key.
pub const INPUT_SEARCH: &str = "act.search";
/// The card's own swallow layer (a press on its empty space does nothing).
pub const ACTION_NOOP: &str = "a9.act.noop";

pub fn routes(action: &str) -> bool {
    action == ACTION_OPEN
        || action == ACTION_CANCEL
        || action == ACTION_CLOSE
        || action == ACTION_BACKDROP
        || action == ACTION_ROW
        || action == ACTION_NOOP
        || action.starts_with(ACTION_FILTER)
}

/// What a tap asks the host to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Start the refresh loop under this generation.
    Read(u64),
    /// Nothing more (state already changed; re-lower).
    Done,
    /// Open this session (its store index; the sidebar's `thread.open`).
    OpenSession { index: usize, session: String },
    /// Run another owner's action (Inspect opens the task view).
    Action(String),
    Unrouted,
}

/// Apply one action. Row taps resolve against [`ActState::shown`].
pub fn perform(action: &str, row: usize, store: &Store) -> Outcome {
    if action == ACTION_OPEN {
        return match open(store) {
            Some(gen) => Outcome::Read(gen),
            None => Outcome::Done,
        };
    }
    if action == ACTION_CLOSE || action == ACTION_BACKDROP || action == ACTION_CANCEL {
        close();
        return Outcome::Done;
    }
    if action == ACTION_NOOP {
        return Outcome::Done;
    }
    if let Some(f) = action.strip_prefix(ACTION_FILTER).and_then(Filter::from_id) {
        let mut st = state();
        st.filter = f;
        st.query_snap = st.query.clone();
        return Outcome::Done;
    }
    if action == ACTION_ROW {
        let Some(r) = state().shown.get(row).cloned() else {
            return Outcome::Unrouted;
        };
        let current = store.active_session().as_deref() == Some(r.session_id.as_str());
        if current {
            if !inspect_available(store) {
                return Outcome::Unrouted;
            }
            close();
            return Outcome::Action("dialog.open.tasks".to_owned());
        }
        if switch_blocked() {
            return Outcome::Unrouted;
        }
        let index = store.sessions().iter().position(|s| s.id == r.session_id);
        close();
        return match index {
            Some(index) => Outcome::OpenSession { index, session: r.session_id },
            None => Outcome::Unrouted,
        };
    }
    Outcome::Unrouted
}

/// The search field changed (no remount: rows re-filter by visibility).
pub fn input_changed(key: &str, text: &str) {
    if key == INPUT_SEARCH {
        state().query = text.to_owned();
    }
}

// ------------------------------------------------------------- the surface

/// The mounted dialog: DSL, its taps and inputs.
#[derive(Debug, Clone)]
pub struct Lowered {
    pub dsl: String,
    pub taps: Vec<(String, String)>,
    pub inputs: Vec<(String, String)>,
}

/// The web's dialog box (`styles.css:1393-1404`): `min(900px, 100%)` wide,
/// `min(720px, 100%)` tall inside the 24 px backdrop inset; full-bleed below
/// the compact width (`styles.css:2121-2131`).
pub fn dialog_box(frame: &Frame) -> (f64, f64, bool) {
    let compact = frame.avail_w < 640.0;
    if compact {
        (frame.avail_w.floor(), frame.avail_h.floor(), true)
    } else {
        let w = (frame.avail_w - 32.0).min(900.0).floor();
        let h = (frame.avail_h - 32.0).min(720.0).max(360f64.min(frame.avail_h - 32.0)).floor();
        (w, h, false)
    }
}

/// `formatActivityTime` (`ActivityNavigator.tsx:184-194`): `Oct 1, 09:41 AM`
/// in the device's zone; an unparsable value is shown verbatim.
pub fn format_time(value: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(value) {
        Ok(t) => t.with_timezone(&chrono::Local).format("%b %-d, %I:%M %p").to_string(),
        Err(_) => value.to_owned(),
    }
}

/// The footer's left text (`ActivityNavigator.tsx:170-176`).
pub fn footer_text(st: &ActState) -> &'static str {
    tr(if st.loading {
        "Refreshing task snapshots…"
    } else if st.available {
        "Read-only · refreshes every 10 seconds"
    } else {
        "Read-only · snapshots unavailable"
    })
}

/// A24 — a filter's name as the web shows it: `t(candidate)` over the
/// lowercase id (`ActivityNavigator.tsx:91`, capitalized by CSS).
pub fn filter_name(f: Filter) -> String {
    let name = tr(f.id());
    let mut c = name.chars();
    match c.next() {
        Some(first) => first.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// The empty state's detail (`ActivityNavigator.tsx:154-166`).
pub fn empty_detail(query: &str, filter: Filter) -> String {
    let q = query.trim();
    // A24: the web's keys (`ActivityNavigator.tsx:158-166`); the filter
    // reads in the current language (the web passes its raw id).
    let name = tr(filter.id());
    if !q.is_empty() {
        tr_with("No {value0} task matches “{value1}”.", &[("value0", name), ("value1", q)])
    } else if filter == Filter::All {
        tr("No task snapshots are available yet.").to_owned()
    } else {
        tr1("No {value0} tasks are available.", name)
    }
}

const ROW_PX: f64 = 13.0;

/// Lower the open dialog against the live store (`None` when closed).
pub fn lower(store: &Store) -> Option<Lowered> {
    let mut st = state();
    if !st.open {
        return None;
    }
    // The first catalog read has not settled: the web's loading fallback for
    // a dismissable surface (`SurfaceBoundary.tsx:25-33/:44-83`, "Loading
    // <name>…" + Cancel); a cancelled load never opens afterwards (the close
    // bumps the generation, so the late read is refused).
    if st.available && st.reads == 0 {
        let frame = st.frame;
        drop(st);
        return Some(lower_loading(&frame));
    }
    let frame = st.frame;
    let (w, h, compact) = dialog_box(&frame);
    let model = build_model(&st.tasks, &st.labels, "", st.filter);
    st.shown = model.rows.clone();
    let blocked = switch_blocked();
    let inspect = inspect_available(store);
    let active = store.active_session();
    let pad = if compact { 16.0 } else { 20.0 };
    let mut d = Dsl::new();

    // Backdrop: the web's mask; a press outside the card closes it
    // (`closeOnBackdrop`). The card swallows its own presses.
    d.view("a9_act_root", "width: Fill height: Fill flow: Overlay align: Align{x: 0.5 y: 0.5}");
    d.rule("a9_act_mask", "width: Fill height: Fill", tok::MASK);
    d.view("a9_act_backdrop_box", "width: Fill height: Fill flow: Overlay");
    d.tap("a9_act_backdrop", ACTION_BACKDROP);
    d.close();
    let (radius, border) = if compact { (0.0, None) } else { (16.0, Some(tok::HAIRLINE)) };
    d.surface(
        "a9_act_dialog",
        &format!("width: {w} height: {h} flow: Overlay"),
        tok::SURFACE,
        radius,
        border,
    );
    // The card's own swallow layer: a press on its empty space must not
    // reach the backdrop (it would close the dialog).
    d.view("a9_act_swallow_box", "width: Fill height: Fill flow: Overlay");
    d.tap("a9_act_swallow", ACTION_NOOP);
    d.close();
    d.view("a9_act_col", "width: Fill height: Fill flow: Down");

    // ---- header (`styles.css:1410-1443`)
    d.view(
        "a9_act_header",
        &format!(
            "width: Fill height: Fit flow: Right align: Align{{x: 0.0 y: 0.0}} spacing: 16 padding: Inset{{left: {pad} right: {pad} top: 18 bottom: 15}}"
        ),
    );
    d.view("a9_act_head_col", "width: Fill height: Fit flow: Down spacing: 4");
    d.text("a9_act_eyebrow", &tr("Across recent sessions").to_uppercase(), &Txt::new(11.0, Face::Medium, tok::MUTED));
    d.text("a9_act_title", tr("Activity"), &Txt::new(20.0, Face::Semibold, tok::TEXT));
    d.text(
        "a9_act_scope",
        tr("Server-owned tasks; no session is opened by this scan."),
        &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
    );
    d.close();
    // The 32 px bordered close (`styles.css:1431-1443`).
    d.surface(
        "a9_act_close_box",
        "width: 32 height: 32 flow: Overlay align: Align{x: 0.5 y: 0.5}",
        tok::SURFACE,
        9.0,
        Some(tok::HAIRLINE),
    );
    d.icon("a9_act_close_icon", "b3_close.svg", 14.0, tok::MUTED);
    d.tap("a9_act_close", ACTION_CLOSE);
    d.close();
    d.close();
    d.hairline();

    // ---- toolbar (`styles.css:1445-1484`): search, then the status filters
    // (side by side on a desktop card, stacked on a phone).
    let tool_flow = if compact { "Down" } else { "Right" };
    d.view(
        "a9_act_toolbar",
        &format!(
            "width: Fill height: Fit flow: {tool_flow} align: Align{{x: 0.0 y: 0.5}} spacing: 12 padding: Inset{{left: {pad} right: {pad} top: 12 bottom: 12}}"
        ),
    );
    d.surface(
        "a9_act_search_field",
        "width: Fill height: 38 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{left: 12 right: 12 top: 0 bottom: 0}",
        tok::SURFACE,
        10.0,
        Some("#d1d1d6ff"),
    );
    d.icon("a9_act_search_icon", "b3_search.svg", 16.0, tok::FAINT);
    search_input(&mut d, &st.query_snap);
    d.close();
    let labels: Vec<String> = Filter::ALL
        .iter()
        .enumerate()
        .map(|(i, f)| format!("{} {}", filter_name(*f), model.counts[i]))
        .collect();
    let options: Vec<(&str, String)> = Filter::ALL
        .iter()
        .zip(&labels)
        .map(|(f, l)| (l.as_str(), format!("{ACTION_FILTER}{}", f.id())))
        .collect();
    let selected = Filter::ALL.iter().position(|f| *f == st.filter).unwrap_or(0);
    let seg_w = if compact { W::Fill } else { W::Px(392.0) };
    d.segmented("a9_act_filter", &options, selected, seg_w, ui::Seg::Tab);
    d.close();
    d.hairline();

    // ---- the blocked-switch warning (`ActivityNavigator.tsx:97-101`)
    if blocked {
        d.surface(
            "a9_act_warning",
            &format!(
                "width: Fill height: Fit flow: Right spacing: 8 align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: {pad} right: {pad} top: 8 bottom: 8}}"
            ),
            tok::AMBER_BG,
            0.0,
            None,
        );
        d.icon("a9_act_warning_icon", "b3_warning.svg", 14.0, tok::AMBER);
        d.text(
            "a9_act_warning_text",
            tr("Finish the workspace transition before opening another session."),
            &Txt::new(12.0, Face::Regular, tok::TEXT).w(W::Fill).wrap(),
        );
        d.close();
        d.rule("a9_act_warning_rule", "width: Fill height: 1", tok::AMBER_LINE);
    }

    // ---- results (`styles.css:1499-1601`): a scroll region filling the
    // card between the toolbar and the footer.
    d.open(
        "a9_act_results",
        "ScrollYView",
        &format!(
            "width: Fill height: Fill flow: Down padding: Inset{{left: {} right: {} top: 8 bottom: 8}}",
            if compact { 8.0 } else { 12.0 },
            if compact { 8.0 } else { 12.0 }
        ),
    );
    // The status lines (`role="status"` paragraphs, not alerts) sit on the
    // rows' inset so they line up with the state dots.
    if !st.available {
        status_line(&mut d, "a9_act_unavailable", tr("This server does not advertise task snapshots."));
    }
    if let Some(e) = &st.error {
        status_line(&mut d, "a9_act_error", tr(e));
    }
    // The text column's width: card - results padding - row padding - dot
    // column - gaps - the action pill (desktop) - the scroll gutter.
    let btn_w = ui::text_w(tr("Open session"), 12.0, Face::Medium).max(ui::text_w(tr("Inspect"), 12.0, Face::Medium)) + 24.0;
    let text_w = if compact {
        w - 2.0 * 8.0 - 2.0 * 8.0 - 9.0 - 10.0 - 6.0
    } else {
        w - 2.0 * 12.0 - 2.0 * 8.0 - 9.0 - 10.0 - 10.0 - btn_w - 6.0
    };
    for (i, r) in model.rows.iter().enumerate() {
        let current = active.as_deref() == Some(r.session_id.as_str());
        let enabled = if current { inspect } else { !blocked };
        row(&mut d, i, r, current, enabled, compact, text_w, btn_w);
    }
    // The empty state is always emitted (hidden while a row shows), so the
    // search can reveal it without a remount.
    d.view(
        "a9_act_empty",
        "width: Fill height: 220 flow: Down spacing: 6 align: Align{x: 0.5 y: 0.5}",
    );
    d.text("a9_act_empty_title", tr("No matching activity"), &Txt::new(13.0, Face::Medium, tok::TEXT));
    d.text(
        "a9_act_empty_detail",
        &empty_detail(&st.query_snap, st.filter),
        &Txt::new(12.0, Face::Regular, tok::MUTED),
    );
    d.close();
    d.close(); // results
    d.hairline();

    // ---- footer (`styles.css:1603-1614`)
    d.view(
        "a9_act_footer",
        &format!(
            "width: Fill height: 36 flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 8 padding: Inset{{left: {pad} right: {pad} top: 0 bottom: 0}}"
        ),
    );
    d.text("a9_act_footer_status", footer_text(&st), &Txt::new(11.0, Face::Mono, tok::MUTED).w(W::Fill));
    // The keyboard hint only where there is a keyboard (a phone frame closes
    // with the ×).
    if !compact {
        d.text("a9_act_footer_esc", tr("Esc closes"), &Txt::new(11.0, Face::Mono, tok::MUTED));
    }
    d.close();

    d.close(); // col
    d.close(); // dialog
    d.close(); // root
    let taps = d.taps.clone();
    let inputs = d.inputs.clone();
    Some(Lowered { dsl: d.finish(), taps, inputs })
}

/// Cancel the loading Activity (the loading fallback's only action).
pub const ACTION_CANCEL: &str = "a9.act.cancel";

/// The loading fallback (`UnavailableSurface` with `loading`): a centred
/// panel "Loading activity…", what it is waiting for, and Cancel.
fn lower_loading(frame: &Frame) -> Lowered {
    let w = (frame.avail_w - 32.0).min(360.0).max(240.0).floor();
    let mut d = Dsl::new();
    d.view("a9_act_root", "width: Fill height: Fill flow: Overlay align: Align{x: 0.5 y: 0.5}");
    d.rule("a9_act_mask", "width: Fill height: Fill", tok::MASK);
    d.view("a9_act_backdrop_box", "width: Fill height: Fill flow: Overlay");
    d.tap("a9_act_backdrop", ACTION_CANCEL);
    d.close();
    d.surface(
        "a9_act_loading",
        &format!("width: {w} height: Fit flow: Down spacing: 10 padding: Inset{{left: 22 right: 22 top: 22 bottom: 20}}"),
        tok::SURFACE,
        16.0,
        Some(tok::HAIRLINE),
    );
    d.text("a9_act_loading_title", tr("Loading activity…"), &Txt::new(17.0, Face::Semibold, tok::TEXT).w(W::Fill));
    d.text(
        "a9_act_loading_detail",
        tr("Reading task snapshots from your confirmed sessions."),
        &Txt::new(13.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
    );
    d.view("a9_act_loading_actions", "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5} margin: Inset{top: 4}");
    d.button("a9_act_loading_cancel", tr("Cancel"), ACTION_CANCEL, Btn::Outline, W::Fit, 36.0);
    d.close();
    d.close();
    d.close();
    let taps = d.taps.clone();
    Lowered { dsl: d.finish(), taps, inputs: Vec::new() }
}

fn status_line(d: &mut Dsl, id: &str, text: &str) {
    d.view(&format!("{id}_box"), "width: Fill height: Fit flow: Down padding: Inset{left: 8 right: 8 top: 6 bottom: 6}");
    d.text(id, text, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    d.close();
}

fn search_input(d: &mut Dsl, snap: &str) {
    d.inputs.push(("a9_act_search".to_owned(), INPUT_SEARCH.to_owned()));
    let style = ui::text_style(Face::Regular, 13.5);
    d.open(
        "a9_act_search",
        "TextInput",
        &format!(
            "width: Fill height: Fit padding: Inset{{left: 0 right: 0 top: 4 bottom: 4}} margin: 0\ntext: {} empty_text: {}\nflow: Right is_read_only: false\ndraw_bg +: {{pixel: fn() {{return vec4(0.0, 0.0, 0.0, 0.0)}}}}\ndraw_text +: {{color: {t} color_hover: {t} color_focus: {t} color_down: {t} color_disabled: {f} color_empty: {f} color_empty_hover: {f} color_empty_focus: {f}}}\ndraw_text.text_style: {style}\ndraw_cursor +: {{color: {t}}}\ndraw_selection +: {{color: #2f6feb33 color_hover: #2f6feb33 color_focus: #2f6feb40 color_down: #2f6feb40 color_empty: #00000000 color_disabled: #00000000}}",
            ui::lit(snap),
            ui::lit(tr("Search session, task, role, or status…")),
            t = tok::TEXT,
            f = tok::FAINT,
        ),
    );
    d.close();
}

/// One task row (`ActivityNavigator.tsx:110-151`, `styles.css:1505-1583`).
#[allow(clippy::too_many_arguments)]
fn row(d: &mut Dsl, i: usize, r: &Row, current: bool, enabled: bool, compact: bool, text_w: f64, btn_w: f64) {
    let id = format!("a9_act_row_{i}");
    d.view(&id, "width: Fill height: Fit flow: Down");
    d.view(
        &format!("{id}_line"),
        "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10 padding: Inset{left: 8 right: 8 top: 10 bottom: 10}",
    );
    // The dot column (9 px; the dot sits on the title line).
    d.view(&format!("{id}_dotcol"), "width: 9 height: Fit flow: Down padding: Inset{left: 1 right: 1 top: 6 bottom: 0}");
    d.dot(r.state.dot(), 7.0);
    d.close();
    d.view(&format!("{id}_text"), "width: Fill height: Fit flow: Down spacing: 3");
    d.text(
        &format!("{id}_title"),
        &ui::fit_w(&r.title, text_w, ROW_PX, Face::Medium),
        &Txt::new(ROW_PX, Face::Medium, tok::TEXT),
    );
    d.text(
        &format!("{id}_session"),
        &ui::fit_w(&r.session_title, text_w, 12.0, Face::Regular),
        &Txt::new(12.0, Face::Regular, tok::MUTED),
    );
    let detail = if r.detail.is_empty() { r.task_id.clone() } else { r.detail.clone() };
    let detail = match &r.updated_at {
        Some(t) => format!("{detail} · {}", format_time(t)),
        None => detail,
    };
    d.text(
        &format!("{id}_detail"),
        &ui::fit_w(&detail, text_w, 11.0, Face::Mono),
        &Txt::new(11.0, Face::Mono, tok::MUTED),
    );
    let label = tr(if current { "Inspect" } else { "Open session" });
    let kind = if enabled { Btn::Outline } else { Btn::OutlineOff };
    let event = format!("{ACTION_ROW}#{i}");
    d.close(); // text
    if !compact {
        pill(d, &format!("{id}_act"), label, &event, kind, btn_w);
    }
    d.close(); // line
    if compact {
        // Phone: the action sits under the text, in the text's column
        // (`styles.css:2141-2147`: grid-column 2), so the state dot stays
        // centred on the text block, not on text + button.
        d.view(
            &format!("{id}_actrow"),
            "width: Fill height: Fit flow: Right padding: Inset{left: 27 right: 8 top: 0 bottom: 10}",
        );
        pill(d, &format!("{id}_act"), label, &event, kind, btn_w);
        d.close();
    }
    d.hairline();
    d.close(); // row
}

/// The row's action pill (`styles.css:1565-1583`): a 30 px outline button
/// with micro text; a disabled one routes nothing.
fn pill(d: &mut Dsl, id: &str, label: &str, event: &str, kind: Btn, w: f64) {
    let (fg, border) = match kind {
        Btn::OutlineOff => (tok::DISABLED_INK, tok::HAIRLINE),
        _ => (tok::TEXT, "#c7c7ccff"),
    };
    d.surface(
        &format!("{id}_box"),
        &format!("width: {w} height: 30 flow: Overlay align: Align{{x: 0.5 y: 0.5}}"),
        tok::SURFACE,
        8.0,
        Some(border),
    );
    d.text(&format!("{id}_label"), label, &Txt::new(12.0, Face::Medium, fg));
    if kind != Btn::OutlineOff {
        d.tap(id, event);
    }
    d.close();
}

/// Row/empty visibility for the live search text (no remount).
pub fn live_visibility() -> Vec<(String, bool)> {
    let st = state();
    if !st.open {
        return Vec::new();
    }
    let mut any = false;
    let mut out: Vec<(String, bool)> = st
        .shown
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let on = matches(r, &st.query);
            any |= on;
            (format!("a9_act_row_{i}"), on)
        })
        .collect();
    out.push(("a9_act_empty".to_owned(), !any));
    out
}

/// Texts that follow the live search (no remount).
pub fn live_texts() -> Vec<(String, String)> {
    let st = state();
    if !st.open {
        return Vec::new();
    }
    vec![("a9_act_empty_detail".to_owned(), empty_detail(&st.query, st.filter))]
}

// ------------------------------------------------------------ capture seed

/// The capture seed (`OCTOSCODE_ACTIVITY_SEED`): the recorded `task/list`
/// shapes (`crates/octoscode-client/tests/fixtures/c24b-subagent-a6ea8505.jsonl`
/// replies, ids and stamps kept) spread over the board-2 seed's sessions, so
/// a hidden capture shows every row state without a server. `blocked` also
/// marks a session switch in flight (the warning). No transport.
pub fn seed(store: &Store, variant: &str) {
    let task = |id: &str, tool: &str, state: &str, status: &str, role: Option<&str>, summary: Option<&str>, phase: Option<&str>, error: Option<&str>, at: &str| Task {
        id: id.to_owned(),
        tool_name: tool.to_owned(),
        state: state.to_owned(),
        status: status.to_owned(),
        role: role.map(str::to_owned),
        summary: summary.map(str::to_owned),
        current_phase: phase.map(str::to_owned),
        error: error.map(str::to_owned),
        updated_at: Some(at.to_owned()),
    };
    let ids: Vec<(String, String)> = store
        .sessions()
        .into_iter()
        .map(|s| (s.id.clone(), s.display_label()))
        .collect();
    let pick = |i: usize| ids.get(i).cloned().unwrap_or_else(|| (format!("seed:{i}"), format!("Session {i}")));
    let mut tasks: BTreeMap<String, Vec<Task>> = BTreeMap::new();
    let (s0, s1, s2, s3) = (pick(0), pick(1), pick(3), pick(2));
    tasks.insert(
        s0.0.clone(),
        vec![
            task("01a0eb98-2eb6-7e33-9c35-6458c7140096", "c24b-probe", "running", "running", Some("test_worker"), Some("cargo test -p octos-cli steer_queue"), Some("executing_tool"), None, "2026-09-29T05:16:54.326588Z"),
            task("01a0eb98-2eb6-7e33-9c35-6458c7140097", "bash", "completed", "completed", None, Some("cargo clippy -p octos-cli"), None, None, "2026-09-29T05:12:10.000000Z"),
        ],
    );
    tasks.insert(
        s1.0.clone(),
        vec![task("01a0eb98-3100-7e33-9c35-6458c71400a1", "spawn_agent", "completed", "completed", Some("reviewer"), Some("Review the session fork design"), Some("joined"), None, "2026-09-29T04:58:31.000000Z")],
    );
    tasks.insert(
        s2.0.clone(),
        vec![task("01a0eb98-3200-7e33-9c35-6458c71400b2", "bash", "failed", "failed", Some("implementer"), Some("Rebuild after the octos-core bump"), Some("build"), Some("cargo build: 2 errors"), "2026-09-29T04:40:02.000000Z")],
    );
    tasks.insert(
        s3.0.clone(),
        vec![task("01a0eb98-3300-7e33-9c35-6458c71400c3", "delegate", "running", "running", Some("explorer"), Some("Profile the hydrate path"), Some("reading"), None, "2026-09-29T05:15:40.000000Z")],
    );
    let labels: BTreeMap<String, String> = [s0, s1, s2, s3].into_iter().collect();
    let mut st = state();
    let frame = st.frame;
    *st = ActState {
        open: true,
        gen: st.gen + 1,
        available: true,
        tasks,
        labels,
        reads: 1,
        frame,
        ..ActState::default()
    };
    drop(st);
    if variant == "blocked" {
        note_switch_started();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: &str, state: &str, summary: Option<&str>, role: Option<&str>, at: &str) -> Task {
        Task {
            id: id.into(),
            tool_name: "bash".into(),
            state: state.into(),
            status: state.into(),
            role: role.map(Into::into),
            summary: summary.map(Into::into),
            current_phase: None,
            error: None,
            updated_at: Some(at.into()),
        }
    }

    fn lock() -> MutexGuard<'static, ()> {
        crate::screens::theme::test_lock()
    }

    /// `catalog.test.ts:5-40` — active work beyond 100 tasks is kept, and the
    /// search reaches the confirmed display label.
    #[test]
    fn running_work_beyond_100_tasks_is_kept_and_the_label_is_searchable() {
        let mut tasks = BTreeMap::new();
        let list: Vec<Task> = (0..101)
            .map(|i| {
                t(
                    &format!("task-{i}"),
                    if i == 100 { "running" } else { "completed" },
                    None,
                    None,
                    "2026-09-06T00:01:00Z",
                )
            })
            .collect();
        tasks.insert("s1".to_owned(), list);
        let labels: BTreeMap<String, String> = [("s1".to_owned(), "Design review".to_owned())].into();
        let m = build_model(&tasks, &labels, "design", Filter::Running);
        assert_eq!(m.rows.iter().map(|r| r.task_id.as_str()).collect::<Vec<_>>(), ["task-100"]);
        assert_eq!(m.rows[0].session_title, "Design review");
        assert_eq!(m.counts, [101, 1, 0, 100]);
    }

    /// `model.ts:146-154`: running < failed < unknown < done, then newest.
    #[test]
    fn rows_sort_by_state_priority_then_newest_then_title() {
        let mut tasks = BTreeMap::new();
        tasks.insert(
            "p:a".to_owned(),
            vec![
                t("1", "completed", Some("old done"), None, "2026-09-01T00:00:00Z"),
                t("2", "completed", Some("new done"), None, "2026-09-02T00:00:00Z"),
                t("3", "cancelled", Some("stopped"), None, "2026-09-01T00:00:00Z"),
                t("4", "pending", None, Some("reviewer"), "2026-09-01T00:00:00Z"),
                t("5", "weird", Some("odd"), None, "2026-09-01T00:00:00Z"),
            ],
        );
        let m = build_model(&tasks, &BTreeMap::new(), "", Filter::All);
        let order: Vec<&str> = m.rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(order, ["reviewer", "stopped", "odd", "new done", "old done"]);
        assert_eq!(m.rows[0].state, RowState::Running, "pending maps to running");
        assert_eq!(m.rows[1].state, RowState::Failed, "cancelled maps to failed");
        assert_eq!(m.rows[2].state, RowState::Unknown);
        assert_eq!(m.counts, [5, 1, 1, 2]);
        // The label falls back to the id when the session is unconfirmed.
        assert_eq!(m.rows[0].session_title, "p:a");
    }

    #[test]
    fn title_and_detail_follow_the_web_projection() {
        let mut task = t("9", "failed", Some("  "), Some("implementer"), "2026-09-01T00:00:00Z");
        task.current_phase = Some("build".into());
        task.error = Some("2 errors".into());
        let mut tasks = BTreeMap::new();
        tasks.insert("p:x".to_owned(), vec![task]);
        let m = build_model(&tasks, &BTreeMap::new(), "", Filter::All);
        // A whitespace summary does not win over the role (`?.trim() ||`).
        assert_eq!(m.rows[0].title, "implementer");
        assert_eq!(m.rows[0].detail, "implementer · build · failed · 2 errors");
        assert!(matches(&m.rows[0], "2 ERRORS"), "case-insensitive");
        assert!(matches(&m.rows[0], "bash"), "the tool name is searchable");
        assert!(!matches(&m.rows[0], "nothing"));
    }

    #[test]
    fn the_empty_copy_names_the_filter_and_the_query() {
        assert_eq!(empty_detail("", Filter::All), "No task snapshots are available yet.");
        assert_eq!(empty_detail("", Filter::Failed), "No failed tasks are available.");
        assert_eq!(empty_detail(" zz ", Filter::Running), "No running task matches “zz”.");
    }

    #[test]
    fn targets_are_the_profiles_confirmed_sessions_once() {
        use octoscode_store::domains::session::Session;
        let store = Store::new();
        let s = |id: &str, title: &str| Session {
            id: id.into(),
            title: Some(title.into()),
            message_count: 1,
            updated_at: None,
            last_prompt: None,
            active_turn: false,
        };
        store.set_sessions(vec![s("p:a", "Alpha"), s("q:b", "Foreign"), s("p:", "bare"), s("p:a", "dup"), s("p:c", "Gamma")]);
        let t = targets(&store, "p");
        assert_eq!(t, vec![("p:a".to_owned(), "Alpha".to_owned()), ("p:c".to_owned(), "Gamma".to_owned())]);
        assert!(targets(&store, "").is_empty(), "no Profile, nothing confirmed");
    }

    #[test]
    fn a_closed_or_reopened_dialog_never_publishes_a_late_read() {
        let _g = lock();
        reset();
        let store = Store::new();
        store.domains.config.set_supported_methods(vec!["task/list".into()]);
        let g1 = open(&store).expect("advertised");
        close();
        assert!(!publish(g1, Catalog::default(), BTreeMap::new()), "closed");
        let g2 = open(&store).expect("advertised");
        let g3 = open(&store).expect("advertised");
        assert!(!publish(g2, Catalog::default(), BTreeMap::new()), "superseded");
        assert!(publish(g3, Catalog { tasks_by_session: BTreeMap::new(), unavailable: vec!["p:x".into()] }, BTreeMap::new()));
        assert_eq!(state().error.as_deref(), Some("1 Session task snapshots unavailable"));
        reset();
    }

    #[test]
    fn an_unadvertised_server_is_never_read() {
        let _g = lock();
        reset();
        let store = Store::new();
        assert_eq!(perform(ACTION_OPEN, 0, &store), Outcome::Done, "no read job");
        assert!(is_open() && !state().available);
        assert_eq!(footer_text(&state()), "Read-only · snapshots unavailable");
        let low = lower(&store).expect("open");
        assert!(low.dsl.contains("This server does not advertise task snapshots."));
        reset();
    }

    #[test]
    fn rows_route_inspect_or_open_and_respect_the_switch_guard() {
        use octoscode_store::domains::session::Session;
        let _g = lock();
        reset();
        let store = Store::new();
        let s = |id: &str| Session { id: id.into(), title: None, message_count: 1, updated_at: None, last_prompt: None, active_turn: false };
        store.set_sessions(vec![s("p:main"), s("p:other")]);
        store.set_active(Some("p:main".into()));
        store.set_connection("Live".into(), true);
        store
            .domains
            .config
            .set_supported_methods(vec!["task/list".into(), "task/output/read".into()]);
        let mut tasks = BTreeMap::new();
        tasks.insert("p:main".into(), vec![t("1", "running", Some("mine"), None, "2026-09-02T00:00:00Z")]);
        tasks.insert("p:other".into(), vec![t("2", "completed", Some("theirs"), None, "2026-09-01T00:00:00Z")]);
        let gen = open(&store).unwrap();
        assert!(publish(gen, Catalog { tasks_by_session: tasks, unavailable: vec![] }, BTreeMap::new()));
        let low = lower(&store).unwrap();
        // Row 0 is the current session's running task: Inspect.
        assert!(low.dsl.contains("\"Inspect\"") && low.dsl.contains("\"Open session\""));
        assert_eq!(perform(ACTION_ROW, 0, &store), Outcome::Action("dialog.open.tasks".into()));
        assert!(!is_open(), "the dialog closes on its action");
        // Reopen: row 1 opens the owning session by its store index.
        let gen = open(&store).unwrap();
        let mut tasks = BTreeMap::new();
        tasks.insert("p:other".into(), vec![t("2", "completed", Some("theirs"), None, "2026-09-01T00:00:00Z")]);
        publish(gen, Catalog { tasks_by_session: tasks.clone(), unavailable: vec![] }, BTreeMap::new());
        lower(&store);
        // A switch in flight: the open is refused (the warning shows).
        note_switch_started();
        let blocked = lower(&store).unwrap();
        assert!(blocked.dsl.contains("Finish the workspace transition before opening another session."));
        assert!(!blocked.taps.iter().any(|(_, e)| e.starts_with(ACTION_ROW)), "a disabled pill routes nothing");
        assert_eq!(perform(ACTION_ROW, 0, &store), Outcome::Unrouted);
        note_switch_finished();
        lower(&store);
        assert_eq!(perform(ACTION_ROW, 0, &store), Outcome::OpenSession { index: 1, session: "p:other".into() });
        reset();
    }

    #[test]
    fn the_search_filters_by_visibility_without_a_remount() {
        let _g = lock();
        reset();
        let store = Store::new();
        store.domains.config.set_supported_methods(vec!["task/list".into()]);
        let gen = open(&store).unwrap();
        let mut tasks = BTreeMap::new();
        tasks.insert("p:a".into(), vec![t("1", "running", Some("alpha job"), None, "2026-09-02T00:00:00Z"), t("2", "completed", Some("beta job"), None, "2026-09-01T00:00:00Z")]);
        publish(gen, Catalog { tasks_by_session: tasks, unavailable: vec![] }, BTreeMap::new());
        let first = lower(&store).unwrap().dsl;
        input_changed(INPUT_SEARCH, "beta");
        let vis = live_visibility();
        assert!(vis.contains(&("a9_act_row_0".into(), false)));
        assert!(vis.contains(&("a9_act_row_1".into(), true)));
        assert!(vis.contains(&("a9_act_empty".into(), false)));
        input_changed(INPUT_SEARCH, "zzz");
        assert!(live_visibility().contains(&("a9_act_empty".into(), true)));
        assert_eq!(live_texts()[0].1, "No all task matches “zzz”.");
        // A filter tap re-lowers (the selection is drawn); the typed text is
        // carried into the new input.
        perform("a9.act.filter.done", 0, &store);
        let second = lower(&store).unwrap().dsl;
        assert_ne!(first, second);
        assert!(second.contains("text: \"zzz\""));
        reset();
    }

    #[test]
    fn the_first_load_shows_the_cancelable_fallback_and_a_cancelled_load_never_opens() {
        let _g = lock();
        reset();
        let store = Store::new();
        store.domains.config.set_supported_methods(vec!["task/list".into()]);
        let gen = match perform(ACTION_OPEN, 0, &store) {
            Outcome::Read(g) => g,
            other => panic!("{other:?}"),
        };
        let low = lower(&store).unwrap();
        assert!(low.dsl.contains("Loading activity…"));
        assert!(low.taps.iter().any(|(_, e)| e == ACTION_CANCEL));
        assert!(!low.dsl.contains("a9_act_dialog"), "no dialog before the first read");
        // Cancel: the late read is refused and nothing opens.
        assert_eq!(perform(ACTION_CANCEL, 0, &store), Outcome::Done);
        assert!(!publish(gen, Catalog::default(), BTreeMap::new()));
        assert!(!is_open() && lower(&store).is_none());
        // A settled first read shows the navigator.
        let gen = match perform(ACTION_OPEN, 0, &store) {
            Outcome::Read(g) => g,
            other => panic!("{other:?}"),
        };
        assert!(publish(gen, Catalog::default(), BTreeMap::new()));
        assert!(lower(&store).unwrap().dsl.contains("a9_act_dialog"));
        reset();
    }

    #[test]
    fn the_dialog_is_the_web_box_and_full_bleed_on_a_phone() {
        let desk = Frame { avail_w: 990.0, avail_h: 603.0 };
        assert_eq!(dialog_box(&desk), (900.0, 571.0, false));
        let big = Frame { avail_w: 1400.0, avail_h: 900.0 };
        assert_eq!(dialog_box(&big), (900.0, 720.0, false));
        let phone = Frame { avail_w: 360.0, avail_h: 780.0 };
        assert_eq!(dialog_box(&phone), (360.0, 780.0, true));
    }

    #[test]
    fn taps_are_in_the_shared_tap_shape_and_balanced() {
        let _g = lock();
        reset();
        let store = Store::new();
        seed(&store, "");
        let low = lower(&store).unwrap();
        assert_eq!(crate::screens::taps::wired_taps(&low.dsl), low.taps);
        assert_eq!(low.dsl.matches('{').count(), low.dsl.matches('}').count());
        for (_, e) in &low.taps {
            let (base, _) = crate::screens::taps::split_row(e);
            assert!(routes(base), "{e} is routed");
        }
        reset();
    }
}
