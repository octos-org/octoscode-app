//! A4 — the host-facing half of the native board-3 surfaces: which dialog is
//! open, its lowering for the `board3_dock` Splash, the one-owner routing of
//! its taps, its text inputs, and the transport jobs its opening/controls
//! dispatch through the production client.
//!
//! lib.rs touches this module in a few small places (each an added arm):
//! `sync_labels` mounts [`lower_open`] into `board3_splash`; `Event::Actions`
//! routes the published taps (`taps::split_row`, the #FX1 shared path) and
//! the inputs' `changed`/`returned` actions; `perform_action` sends every id
//! [`routes`] claims here BEFORE the conversation router (and before its
//! connection guard: most controls are UI-local); the key handler closes the
//! open dialog on Escape. `flow.rs::submit_draft` sends the web's slash
//! commands here ([`command`]) before the conversation router.
use std::sync::{Mutex, MutexGuard, OnceLock};

use octoscode_store::Store;

use super::ui::{Dsl, Frame};

/// The board-3 dialogs (screens 1, 4-7, 10-12; 8 and 9 are transcript-resident;
/// screens 2-3 — Add workspace / create folder — are board 1's picker and
/// folder browser, `screens::board1`, opened by the sidebar's + Add workspace).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    /// Screen 1 — Runtime inventory (`/tools`, `/mcp`).
    Inventory,
    /// Screen 5 — the inspector (`/threads`, `/turn`, `/permissions`).
    Inspector,
    /// Screen 6 — Thinking effort (`/thinking`).
    Thinking,
    /// Screen 7 — Resume chat (`/resume`).
    Resume,
    /// Screen 10 — Turn images (`/images`).
    Images,
    /// Screen 11 — Conversation history (`/rewind`).
    History,
    /// Screen 12 — Open a different session (`/sessions`).
    Switcher,
    /// Screen 4 — the Fleet destination (sidebar footer "Fleet").
    Fleet,
    /// Screen 12 (right) — the Vim key legend (`?` in Normal mode).
    Vim,
    /// A8 — the Session settings pane (the strip's click, `App.tsx:3139`).
    SessionPane,
}

impl Dialog {
    pub fn from_id(id: &str) -> Option<Dialog> {
        Some(match id {
            "inventory" | "tools" => Dialog::Inventory,
            "inspector" | "threads" => Dialog::Inspector,
            "thinking" => Dialog::Thinking,
            "resume" => Dialog::Resume,
            "images" => Dialog::Images,
            "history" | "rewind" => Dialog::History,
            "switcher" | "sessions" => Dialog::Switcher,
            "fleet" => Dialog::Fleet,
            "vim" => Dialog::Vim,
            "session-settings" | "session_pane" => Dialog::SessionPane,
            _ => return None,
        })
    }
}

/// All board-3 UI state (the values the protocol never carries — the same
/// category as `FlowUi` and `sidebar::SidebarUi`).
pub struct State {
    pub open: Option<Dialog>,
    pub frame: Frame,
    pub inv: super::inventory::InvState,
    pub insp: super::inspector::InspState,
    pub resume: super::resume::ResumeState,
    pub ck: super::checkpoints::CkState,
    pub switch: super::switcher::SwitchState,
    pub img: super::images::ImgState,
    pub fleet: super::fleetview::FleetState,
    pub strip: super::strip::StripState,
    /// The composer's Vim preference + mode (screen 12).
    pub vim: super::vim::VimState,
    /// A8 — the Session settings pane.
    pub pane: super::session_pane::PaneState,
    /// A8 — the dialog the board-3 splash last mounted (None after an open or
    /// a close), so a remount of the SAME dialog keeps its scroll position.
    pub mounted: Option<Dialog>,
    /// Text a finished job wants on the clipboard (the host writes it on the
    /// UI thread, where `cx` lives).
    pub pending_clipboard: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            open: None,
            frame: Frame::DESKTOP,
            inv: Default::default(),
            insp: Default::default(),
            resume: Default::default(),
            ck: Default::default(),
            switch: Default::default(),
            img: Default::default(),
            fleet: Default::default(),
            strip: Default::default(),
            vim: Default::default(),
            pane: Default::default(),
            mounted: None,
            pending_clipboard: None,
        }
    }
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

/// The board-3 state behind its one lock.
pub fn state() -> MutexGuard<'static, State> {
    STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

/// Test seam: reset between tests.
pub fn reset() {
    *state() = State::default();
}

pub fn is_open() -> bool {
    state().open.is_some()
}

pub fn open_dialog() -> Option<Dialog> {
    state().open
}

use super::vim::VimState;

/// The composer's Vim state (screen 12).
pub fn vim() -> VimState {
    state().vim
}

pub fn set_vim(v: VimState) {
    state().vim = v;
}

/// Wake the UI thread (a job folded something the dialog shows).
pub fn wake() {
    makepad_widgets::SignalToUI::set_ui_signal();
}

/// The host reports the module's laid-out size (logical px). The phone check
/// (`OCTOSENSE_WINDOW_SIZE=360x780`) caps it, so a desktop run can lay the
/// dialog out exactly as the phone does (the module's own rule for that env,
/// `lib.rs` `sync_chrome`).
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

/// Take the text a job left for the clipboard.
pub fn take_clipboard() -> Option<String> {
    state().pending_clipboard.take()
}

/// What the host mounts: the DSL, its tap targets (also recoverable with
/// `taps::wired_taps`) and its text inputs (widget id, input key).
#[derive(Debug, Clone)]
pub struct Lowered {
    pub dsl: String,
    pub taps: Vec<(String, String)>,
    pub inputs: Vec<(String, String)>,
}

impl Lowered {
    fn from(d: Dsl) -> Self {
        let taps = d.taps.clone();
        let inputs = d.inputs.clone();
        Lowered { dsl: d.finish(), taps, inputs }
    }
}

/// The conversation column's left offset inside the module (the Fleet pane
/// replaces the chat area, not the sidebar).
pub fn set_content_x(x: f64) {
    state().fleet.content_x = x.max(0.0);
}

/// Lower the open dialog against the live store, or `None` when closed.
pub fn lower_open(store: &Store) -> Option<Lowered> {
    let mut st = state();
    let open = st.open?;
    let mut d = Dsl::new();
    if open == Dialog::Fleet {
        // Announce a peer that changed into an announced state, once.
        let rows = super::fleetview::rows(&st.fleet, store);
        if let Some(a) = super::fleetview::announce(&st.fleet.seen, &rows) {
            makepad_widgets::log!("[octoscode] fleet announce: {a}");
            st.fleet.announcement = Some(a);
        }
        st.fleet.seen = rows.iter().map(|r| (r.label.clone(), r.phase)).collect();
    }
    match open {
        Dialog::Fleet => super::fleetview::build(&mut d, &st.fleet, &st.frame, store),
        Dialog::Inventory => super::inventory::build(&mut d, &st.inv, &st.frame, store),
        Dialog::Inspector => super::inspector::build(&mut d, &st.insp, &st.frame, store),
        Dialog::Thinking => super::thinking::build(&mut d, &st.frame, store),
        Dialog::Resume => super::resume::build(&mut d, &st.resume, &st.frame, store),
        Dialog::History => super::checkpoints::build(&mut d, &st.ck, &st.frame, store),
        Dialog::Switcher => super::switcher::build(&mut d, &st.switch, &st.frame, store, &st.vim),
        Dialog::Vim => super::vim::build_help(&mut d, &st.vim, &st.frame),
        Dialog::SessionPane => super::session_pane::build(&mut d, &st.pane, &st.frame, store),
        Dialog::Images => {
            let session = store.domains.session.active().unwrap_or_default();
            let drafts = crate::screens::media::drafts_for_session(&session);
            super::images::build(&mut d, &st.img, &st.frame, drafts.as_deref());
        }
    }
    Some(Lowered::from(d))
}

/// Transport work a control or an opening dispatches (run by the host on its
/// runtime, through the production client).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    /// `tool/status/list` | `mcp/status/list` (`inventory.ts:171-190`).
    InventoryLoad,
    /// `thread/graph/get` + `approval/scopes/list` (+ `turn/state/get`).
    InspectorLoad,
    /// `session/list {cwd, profile_id}`.
    ResumeLoad,
    /// `session/open` the confirmed candidate (no turn).
    ResumeOpen(String),
    /// `session/hydrate` -> checkpoints.
    CheckpointsLoad,
    /// `session/rollback` to the checkpoint key.
    Rewind(String),
    /// `session/hydrate` -> markdown -> clipboard.
    CopyMarkdown,
    /// `session/list`.
    SwitchLoad,
    /// `session/open` fresh.
    SwitchOpen(String),
    /// `POST /api/upload` per selected image.
    ImagesUpload,
    /// `profile/sub_providers/list` (the Start form's models).
    FleetLanes,
    /// prepare -> driver seat -> one dispatch.
    FleetStart(String, String),
    /// `peer/control` steer (op index, text).
    FleetSteer(usize, String),
    /// `session/status/read` for the strip's model.
    StatusRead,
    /// `GET /api/files` for a delivered file (entry id, preview?).
    FileFetch(u64, bool),
    /// A8 — the Session settings pane's reads (status stamp, permission
    /// presets, model list, driver disclosure).
    PaneLoad,
    /// A8 — `profile/llm/select` for the pane's model row.
    PaneModel(usize),
    /// A8 — `permission/profile/set` (a preset or the approval policy).
    PanePerm(super::session_pane::PermIntent),
    /// A8 — Resume chat: acquire -> release(internal) -> send once.
    PaneResumeChat,
}

/// What a routed action asks of the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Handled UI-locally; the next `sync_labels` repaints.
    Done,
    /// Run this job on the runtime (needs a live conversation).
    Spawn(Job),
    /// Write this text to the clipboard (needs the host's `cx`).
    Clipboard(String),
    /// Close the open dialog.
    Close,
    /// Open the platform file picker for images.
    PickFiles,
    /// Route another (non-board-3) action id through the host router.
    Action(String),
    /// Not a board-3 action.
    Unrouted,
}

/// Whether `action` belongs to the native board-3 surfaces (one owner).
pub fn routes(action: &str) -> bool {
    action.starts_with("b3.")
}

/// A8 — the body scroll a same-dialog remount must restore after its first
/// layout (`lib.rs` draw_walk applies it).
static PENDING_SCROLL: Mutex<Option<f64>> = Mutex::new(None);

pub fn set_pending_scroll(y: f64) {
    *PENDING_SCROLL.lock().unwrap_or_else(|p| p.into_inner()) = Some(y);
}

pub fn take_pending_scroll() -> Option<f64> {
    PENDING_SCROLL.lock().unwrap_or_else(|p| p.into_inner()).take()
}

/// A8 — the host is about to mount the open dialog: whether the splash
/// already shows this same dialog (a state-change remount, whose scroll
/// position the host keeps) — false for a fresh open.
pub fn note_mounted() -> bool {
    let mut st = state();
    let same = st.mounted.is_some() && st.mounted == st.open;
    st.mounted = st.open;
    same
}

/// Open `dialog` and return its load job.
pub fn open(dialog: Dialog) -> Outcome {
    let mut st = state();
    st.open = Some(dialog);
    st.mounted = None;
    match dialog {
        Dialog::Inventory => {
            st.inv.on_open();
            Outcome::Spawn(Job::InventoryLoad)
        }
        Dialog::Inspector => {
            st.insp.loading = true;
            st.insp.copied = false;
            Outcome::Spawn(Job::InspectorLoad)
        }
        Dialog::Thinking => Outcome::Done,
        Dialog::Resume => {
            st.resume.selected = None;
            st.resume.confirm.clear();
            st.resume.confirm_snap.clear();
            st.resume.query_snap = st.resume.query.clone();
            st.resume.loading = true;
            Outcome::Spawn(Job::ResumeLoad)
        }
        Dialog::History => {
            st.ck.confirm = None;
            st.ck.notice = None;
            st.ck.copy_label = None;
            st.ck.loading = true;
            Outcome::Spawn(Job::CheckpointsLoad)
        }
        Dialog::Switcher => {
            st.switch.error = None;
            Outcome::Spawn(Job::SwitchLoad)
        }
        Dialog::Images => {
            st.img.error = None;
            st.img.notice = None;
            Outcome::Done
        }
        Dialog::Fleet => {
            st.fleet.announcement = None;
            st.fleet.brief_snap = st.fleet.brief.clone();
            Outcome::Spawn(Job::FleetLanes)
        }
        Dialog::Vim => Outcome::Done,
        Dialog::SessionPane => super::session_pane::on_open(&mut st.pane),
    }
}

pub fn close() {
    let mut st = state();
    if st.open == Some(Dialog::Images) {
        drop(st);
        if let Some(store) = active_drafts() {
            store.cancel_uploads(); // closing cancels transfers, keeps selections
        }
        st = state();
    }
    st.open = None;
    st.mounted = None;
}

fn active_drafts() -> Option<std::sync::Arc<crate::screens::media::AttachmentDraftStore>> {
    let session = state().img.session.clone();
    crate::screens::media::drafts_for_session(&session)
}

/// Route one board-3 action. `index` is the `#<row>` the shared tap path
/// decoded (0 for single-shot controls).
pub fn perform(action: &str, index: usize, store: &Store) -> Outcome {
    let out = perform_inner(action, index, store);
    if out == Outcome::Close {
        close();
        return Outcome::Done;
    }
    out
}

fn perform_inner(action: &str, index: usize, store: &Store) -> Outcome {
    if action == "b3.close" {
        close();
        return Outcome::Done;
    }
    // The modal backdrop's press: swallowed, nothing happens.
    if action == "b3.noop" {
        return Outcome::Done;
    }
    if let Some(rest) = action.strip_prefix("b3.open.") {
        return match Dialog::from_id(rest) {
            Some(d) => open(d),
            None => Outcome::Unrouted,
        };
    }
    let session = store.domains.session.active().unwrap_or_default();
    if action.starts_with("b3.think.") {
        return super::thinking::perform(store, &session, action);
    }
    match action {
        // A8 — the strip opens the Session settings pane
        // (`App.tsx:3139-3156`), not the app's Settings.
        "b3.strip.settings" => return open(Dialog::SessionPane),
        "b3.file.download" => return Outcome::Spawn(Job::FileFetch(index as u64, false)),
        "b3.file.preview" => return Outcome::Spawn(Job::FileFetch(index as u64, true)),
        // Screen 12: the legend's help line, and the legend's way out.
        "b3.vim.help" => return open(Dialog::Vim),
        "b3.vim.off" => {
            set_vim(VimState { enabled: true, ..vim() }.toggled());
            return Outcome::Close;
        }
        _ => {}
    }
    let mut st = state();
    if action.starts_with("b3.inv.") {
        return super::inventory::perform(&mut st.inv, action, index);
    }
    if action.starts_with("b3.insp.") {
        return super::inspector::perform(&mut st.insp, action);
    }
    if action.starts_with("b3.resume.") {
        return super::resume::perform(&mut st.resume, action, index);
    }
    if action.starts_with("b3.ck.") {
        return super::checkpoints::perform(&mut st.ck, action, index);
    }
    if action.starts_with("b3.switch.") {
        return super::switcher::perform(&mut st.switch, action, index, store);
    }
    if action.starts_with("b3.img.") {
        let drafts = crate::screens::media::drafts_for_session(&session);
        return super::images::perform(&mut st.img, action, index, drafts.as_deref());
    }
    if action.starts_with("b3.fleet.") {
        return super::fleetview::perform(&mut st.fleet, action, index, store);
    }
    if action.starts_with("b3.sc.") {
        return super::session_pane::perform(&mut st.pane, action, index, store);
    }
    Outcome::Unrouted
}

/// A text input changed (the host reads `TextInput::changed`).
pub fn input_changed(key: &str, text: &str) {
    let mut st = state();
    match key.split('.').next().unwrap_or("") {
        "inv" => super::inventory::input_changed(&mut st.inv, key, text),
        "resume" => super::resume::input_changed(&mut st.resume, key, text),
        "fleet" => super::fleetview::input_changed(&mut st.fleet, key, text),
        _ => {}
    }
}

/// Return pressed in an input: the field's primary action.
pub fn input_returned(key: &str, store: &Store) -> Outcome {
    let out = {
        let mut st = state();
        match key.split('.').next().unwrap_or("") {
            "resume" if key == "resume.confirm" => super::resume::perform(&mut st.resume, "b3.resume.confirm", 0),
            _ => Outcome::Done,
        }
    };
    if out == Outcome::Close {
        close();
        return Outcome::Done;
    }
    out
}

/// The live (post-mount) visibility the inputs drive without remounting:
/// filtered rows, empty states, armed buttons.
pub fn live_visibility(store: &Store) -> Vec<(String, bool)> {
    let st = state();
    match st.open {
        Some(Dialog::Inventory) => super::inventory::visibility(&st.inv, store),
        Some(Dialog::Resume) => super::resume::visibility(&st.resume),
        _ => Vec::new(),
    }
}

fn receipt(conv: &crate::flow::Conversation, text: String) {
    let session = conv.session_id();
    conv.store.domains.session.timeline.append(
        &session,
        Some(crate::screens::palette::next_receipt_turn()),
        crate::screens::palette::REPORT_KIND,
        text,
    );
}

/// The slash commands the board-3 surfaces answer (the web's command
/// registry intents, `features/commands/registry.ts`). `None` = not ours.
pub fn command(name: &str, args: &str, conv: &crate::flow::Conversation) -> Option<Outcome> {
    let store = &conv.store;
    let session = conv.session_id();
    match name {
        "tools" | "tool-settings" => {
            state().inv.tab = super::inventory::Tab::Tools;
            Some(open(Dialog::Inventory))
        }
        "mcp" => {
            state().inv.tab = super::inventory::Tab::Mcp;
            Some(open(Dialog::Inventory))
        }
        "threads" | "thread" | "turn" | "permissions" | "permission" => {
            let active = conv.ui().lock().unwrap().active_turn();
            match super::inspector::parse(name, args, active.as_deref()) {
                Ok(mode) => {
                    state().insp.mode = mode;
                    Some(open(Dialog::Inspector))
                }
                Err(reason) => {
                    // `local-report.ts:105-113`: "/{command} is unavailable".
                    receipt(conv, format!("/{name} is unavailable — {reason}"));
                    Some(Outcome::Done)
                }
            }
        }
        "thinking" | "think" => {
            if args.trim().is_empty() {
                Some(open(Dialog::Thinking))
            } else {
                // The argument form sets the effort with no UI
                // (`App.tsx:1348-1350`); an unknown one is reported.
                match super::thinking::apply_arg(store, &session, args) {
                    Ok(level) => receipt(conv, format!("Thinking effort for new prompts: {level}.")),
                    Err(why) => receipt(conv, format!("/{name} is unavailable — {why}")),
                }
                Some(Outcome::Done)
            }
        }
        "resume" => {
            {
                let mut st = state();
                st.resume.query = args.trim().to_owned(); // seeds the search only
            }
            Some(open(Dialog::Resume))
        }
        "images" => {
            let drafts = crate::screens::media::drafts_for_conv(conv);
            let _ = drafts;
            {
                let mut st = state();
                st.img.session = session.clone();
                st.img.profile = conv.profile();
            }
            Some(open(Dialog::Images))
        }
        "rewind" | "backtrack" => Some(open(Dialog::History)),
        "sessions" | "ss" => Some(open(Dialog::Switcher)),
        // `App.tsx:1267-1269`: flip the preference (which also returns the
        // composer to Insert, `vim-edit.ts:31-33`); the field note shows it.
        "vimmode" | "vim-mode" => {
            let next = vim().toggled();
            set_vim(next);
            makepad_widgets::log!("[octoscode] vim mode -> {}", if next.enabled { "on" } else { "off" });
            Some(Outcome::Done)
        }
        _ => None,
    }
}

/// A job could not run (no live conversation): say so in the open surface
/// instead of spinning forever (fail closed, never a silent no-op).
pub fn job_unavailable(job: &Job) {
    let mut st = state();
    let msg = "A confirmed session and profile are required".to_owned();
    match job {
        Job::InventoryLoad => {
            st.inv.loading = false;
            st.inv.error = Some(msg);
        }
        Job::InspectorLoad => {
            st.insp.loading = false;
            st.insp.error = Some(msg);
        }
        Job::ResumeLoad | Job::ResumeOpen(_) => {
            st.resume.loading = false;
            st.resume.opening = false;
            st.resume.error = Some("A confirmed source Session is required to browse history.".into());
        }
        Job::CheckpointsLoad | Job::Rewind(_) | Job::CopyMarkdown => {
            st.ck.loading = false;
            st.ck.applying = false;
            st.ck.error = Some(msg);
        }
        Job::SwitchLoad | Job::SwitchOpen(_) => {
            st.switch.loading = false;
            st.switch.opening = None;
            st.switch.error = Some(msg);
        }
        Job::ImagesUpload => st.img.error = Some(msg),
        Job::FleetLanes => st.fleet.lanes_loading = false,
        Job::FleetStart(..) | Job::FleetSteer(..) => {
            st.fleet.starting = false;
            st.fleet.start_error = Some(msg);
        }
        Job::StatusRead => {}
        Job::PaneLoad => {
            st.pane.loading = false;
            st.pane.driver = crate::screens::driver_discovery::Inventory::Unavailable;
        }
        Job::PaneModel(_) => {
            st.pane.model_saving = false;
            st.pane.model_notice = Some("Couldn't save: the server is not connected".into());
        }
        Job::PanePerm(_) => {
            st.pane.perm_save = Some(super::session_pane::SaveState::Failed(msg));
        }
        Job::PaneResumeChat => {
            st.pane.resume_busy = false;
            st.pane.resume_notice = Some("Couldn't resume chat — nothing was sent".into());
        }
        Job::FileFetch(id, _) => {
            drop(st);
            let mut f = super::rows::file_state();
            let e = f.entry(*id).or_default();
            e.busy = false;
            e.error = Some("Session files are unavailable on this server".into());
        }
    }
}

/// The strip DSL for the active Session (empty when no Session is open —
/// the web mounts the strip only once `session.opened`).
pub fn lower_strip(store: &Store, active_turn: Option<&str>, mode: Option<&str>) -> String {
    if store.active_session().is_none() {
        return String::new();
    }
    let mut st = state();
    // A8 — a peer this app started holds the seat (the pane's disclosure).
    st.strip.self_held = super::session_pane::own_held(&st.pane);
    let note = st.vim.enabled.then(|| super::vim::note(&st.vim));
    super::strip::lower(store, &st.strip, active_turn, mode, note)
}

/// The composer component's measured width (0 = unknown -> the strip fills).
pub fn set_strip_width(w: f64) {
    state().strip.width = if w > 0.0 { Some(w.floor()) } else { None };
}

/// Whether the strip still needs this Session's `session/status/read` (asked
/// once per Session; the web polls the status pill, the strip reads it once).
pub fn strip_status_needed(session: &str) -> bool {
    let mut st = state();
    if st.strip.status_for.as_deref() == Some(session) {
        return false;
    }
    st.strip.status_for = Some(session.to_owned());
    true
}

/// Run a job through the production client. The result is folded into the
/// store / this module's state; the caller wakes the UI.
pub async fn run(job: Job, conv: &crate::flow::Conversation) -> Result<String, String> {
    match job {
        Job::InventoryLoad => super::inventory::load(conv).await,
        Job::InspectorLoad => super::inspector::load(conv).await,
        Job::ResumeLoad => super::resume::load(conv).await,
        Job::ResumeOpen(id) => super::resume::open(conv, id).await,
        Job::CheckpointsLoad => super::checkpoints::load(conv).await,
        Job::Rewind(key) => super::checkpoints::rewind(conv, key).await,
        Job::CopyMarkdown => super::checkpoints::copy_markdown(conv).await,
        Job::SwitchLoad => super::switcher::load(conv).await,
        Job::SwitchOpen(id) => super::switcher::open(conv, id).await,
        Job::ImagesUpload => super::images::upload(conv).await,
        Job::FleetLanes => super::fleetview::load_lanes(conv).await,
        Job::FleetStart(model, brief) => super::fleetview::start(conv, model, brief).await,
        Job::FleetSteer(op, text) => super::fleetview::steer(conv, op, text).await,
        Job::StatusRead => super::strip::load_status(conv).await,
        Job::FileFetch(id, preview) => super::rows::fetch(conv, id, preview).await,
        Job::PaneLoad => super::session_pane::load(conv).await,
        Job::PaneModel(i) => super::session_pane::select_model(conv, i).await,
        Job::PanePerm(intent) => super::session_pane::set_permission(conv, intent).await,
        // Boxed: resume chat sends the prompt through `submit_draft`, which
        // itself routes slash commands back into this function.
        Job::PaneResumeChat => Box::pin(super::session_pane::resume_chat(conv)).await,
    }
}

/// Files the platform picker (or a drop) handed over while the images
/// dialog is open.
pub fn files_chosen(paths: &[std::path::PathBuf]) {
    let session = state().img.session.clone();
    let Some(drafts) = crate::screens::media::drafts_for_session(&session) else {
        state().img.error = Some("Image controls could not be loaded. Try again.".into());
        return;
    };
    match super::images::select_paths(&drafts, paths) {
        Ok(added) => {
            let mut st = state();
            st.img.error = None;
            st.img.paths.extend(added);
        }
        Err(e) => state().img.error = Some(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dialog_id_round_trips_and_routes_are_b3_only() {
        for id in ["inventory", "inspector", "thinking", "resume", "images", "history", "switcher", "fleet", "vim"] {
            assert!(Dialog::from_id(id).is_some(), "{id}");
        }
        assert!(routes("b3.close"));
        assert!(Dialog::from_id("workspace").is_none(), "screens 2-3 are board 1's picker");
        assert!(!routes("thinking.effort.low"), "D3b's card ids stay with board3::owns");
    }

    #[test]
    fn the_phone_env_caps_the_frame() {
        // No env in tests: the frame is the reported rect.
        set_frame(412.0, 794.0);
        assert_eq!(state().frame.avail_w, 412.0);
        set_frame(990.0, 603.0);
        assert_eq!(state().frame.avail_w, 990.0);
    }
}
