//! A4 — the host-facing half of the native board-3 surfaces: which dialog is
//! open, its lowering for the `board3_dock` Splash, the one-owner routing of
//! its taps, its text inputs, and the transport jobs its opening/controls
//! dispatch through the production client.
//!
//! lib.rs touches this module in four small places (each an added arm):
//! `sync_labels` mounts [`lower_open`] into `board3_splash`; `Event::Actions`
//! routes the published taps (`taps::split_row`, the #FX1 shared path) and
//! the inputs' `changed`/`returned` actions; `perform_action` sends every id
//! [`routes`] claims here BEFORE the conversation router (and before its
//! connection guard: most controls are UI-local); the key handler closes the
//! open dialog on Escape.
use std::sync::{Mutex, MutexGuard, OnceLock};

use octoscode_store::Store;

use super::ui::{Dsl, Frame};

/// The board-3 dialogs (screens 1-7, 10-12; 8 and 9 are transcript-resident).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    /// Screen 1 — Runtime inventory (`/tools`, `/mcp`).
    Inventory,
}

impl Dialog {
    pub fn id(self) -> &'static str {
        match self {
            Dialog::Inventory => "inventory",
        }
    }

    pub fn from_id(id: &str) -> Option<Dialog> {
        match id {
            "inventory" | "tools" => Some(Dialog::Inventory),
            _ => None,
        }
    }
}

/// All board-3 UI state (the values the protocol never carries — the same
/// category as `FlowUi` and `sidebar::SidebarUi`).
pub struct State {
    pub open: Option<Dialog>,
    pub frame: Frame,
    pub inv: super::inventory::InvState,
}

impl Default for State {
    fn default() -> Self {
        Self {
            open: None,
            frame: Frame::DESKTOP,
            inv: Default::default(),
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

/// The host reports the module's laid-out size (logical px).
pub fn set_frame(w: f64, h: f64) {
    if w > 0.0 && h > 0.0 {
        state().frame = Frame { avail_w: w, avail_h: h };
    }
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

/// Lower the open dialog against the live store, or `None` when closed.
pub fn lower_open(store: &Store) -> Option<Lowered> {
    let st = state();
    let open = st.open?;
    let mut d = Dsl::new();
    match open {
        Dialog::Inventory => super::inventory::build(&mut d, &st.inv, &st.frame, store),
    }
    Some(Lowered::from(d))
}

/// Transport work a control or an opening dispatches (run by the host on its
/// runtime, through the production client).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    /// `tool/status/list` + `mcp/status/list` (`inventory.ts:171-190`).
    InventoryLoad,
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
    /// Not a board-3 action.
    Unrouted,
}

/// Whether `action` belongs to the native board-3 surfaces (one owner).
pub fn routes(action: &str) -> bool {
    action.starts_with("b3.")
}

/// Open `dialog` (from a slash command or a button) and return the load job.
pub fn open(dialog: Dialog) -> Outcome {
    let mut st = state();
    st.open = Some(dialog);
    match dialog {
        Dialog::Inventory => {
            st.inv.on_open();
            Outcome::Spawn(Job::InventoryLoad)
        }
    }
}

pub fn close() {
    state().open = None;
}

/// Route one board-3 action. `index` is the `#<row>` the shared tap path
/// decoded (0 for single-shot controls).
pub fn perform(action: &str, index: usize, store: &Store) -> Outcome {
    let _ = store;
    if action == "b3.close" {
        close();
        return Outcome::Done;
    }
    if let Some(rest) = action.strip_prefix("b3.open.") {
        return match Dialog::from_id(rest) {
            Some(d) => open(d),
            None => Outcome::Unrouted,
        };
    }
    if action.starts_with("b3.inv.") {
        let mut st = state();
        return super::inventory::perform(&mut st.inv, action, index);
    }
    Outcome::Unrouted
}

/// A text input changed (the host reads `TextInput::changed`).
pub fn input_changed(key: &str, text: &str) {
    let mut st = state();
    if key.starts_with("inv.") {
        super::inventory::input_changed(&mut st.inv, key, text);
    }
}

/// Return pressed in an input: the field's primary action.
pub fn input_returned(key: &str) -> Outcome {
    let _ = key;
    Outcome::Done
}

/// The live (post-mount) visibility the inputs drive without remounting:
/// filtered rows, empty states, validation lines.
pub fn live_visibility(store: &Store) -> Vec<(String, bool)> {
    let st = state();
    match st.open {
        Some(Dialog::Inventory) => super::inventory::visibility(&st.inv, store),
        None => Vec::new(),
    }
}

/// The slash commands the board-3 surfaces answer (the web's command
/// registry intents, `features/commands/registry.ts`): `/tools` and `/mcp`
/// open the inventory on their mode.
pub fn command(name: &str, args: &str, store: &Store) -> Option<Outcome> {
    let _ = (args, store);
    match name {
        "tools" | "tool-settings" => {
            state().inv.tab = super::inventory::Tab::Tools;
            Some(open(Dialog::Inventory))
        }
        "mcp" => {
            state().inv.tab = super::inventory::Tab::Mcp;
            Some(open(Dialog::Inventory))
        }
        _ => None,
    }
}

/// A job could not run (no live conversation): say so in the open surface
/// instead of spinning forever (fail closed, never a silent no-op).
pub fn job_unavailable(job: &Job) {
    let mut st = state();
    match job {
        Job::InventoryLoad => {
            st.inv.loading = false;
            st.inv.error = Some("A confirmed session and profile are required".into());
        }
    }
}

/// Run a job through the production client. The result is folded into the
/// store / this module's state; the caller wakes the UI.
pub async fn run(job: Job, conv: &crate::flow::Conversation) -> Result<String, String> {
    match job {
        Job::InventoryLoad => super::inventory::load(conv).await,
    }
}
