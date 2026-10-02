//! A30 — the peer dock in the sidebar (parity row 270). STUB: the API the
//! failing-first tests (`tests/a30_peer_dock.rs`) drive; the implementation
//! follows in the next commit.
use makepad_widgets::KeyCode;
use octoscode_store::domains::peer::Ack;
use octoscode_store::Store;

use super::board3::fleetview::Status;
use super::peers::RowAction;

pub const ACTION_FOLD: &str = "pd.fold";
pub const ACTION_FOCUS: &str = "pd.focus";
pub const ACTION_APPROVE: &str = "pd.approve";
pub const ACTION_APPROVE_SESSION: &str = "pd.approve_session";
pub const ACTION_DENY: &str = "pd.deny";
pub const ACTION_STOP: &str = "pd.stop";

pub fn routes(action: &str) -> bool {
    action.starts_with("pd.")
}

/// Where the dock is drawn: the phone drawer (`compact`) or the desktop
/// column, its content width and the height it shares with the session tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seat {
    pub compact: bool,
    pub width: f64,
    pub room: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingApproval {
    pub request_id: String,
    pub tool: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockRow {
    pub key: String,
    pub identity: Option<String>,
    pub label: String,
    pub status: Status,
    pub elapsed_ms: u64,
    pub tokens: u64,
    pub ack: Option<Ack>,
    pub approval: Option<PendingApproval>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drawn {
    pub key: String,
    pub identity: Option<String>,
    pub request_id: Option<String>,
    pub operation_id: Option<String>,
    pub turn_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lowered {
    pub dsl: String,
    pub taps: Vec<(String, String)>,
    pub texts: Vec<(String, String)>,
    pub folded: bool,
    pub height: f64,
    pub scroll_to: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub slot: usize,
    pub drawn: Drawn,
    pub action: RowAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Done,
    Spawn(Job),
    Refused(String),
}

pub fn rows(_store: &Store, _now_ms: u64) -> Vec<DockRow> {
    Vec::new()
}

pub fn pill(_rows: &[DockRow]) -> String {
    String::new()
}

pub fn folded(_compact: bool) -> bool {
    false
}

pub fn toggle(_compact: bool) -> bool {
    false
}

pub fn lower(_store: &Store, _now_ms: u64, _seat: Seat) -> Option<Lowered> {
    None
}

pub fn slot_of(_key: &str) -> Option<usize> {
    None
}

pub fn focused_slot() -> Option<usize> {
    None
}

pub fn key_action(_code: KeyCode, _ctrl: bool, _alt: bool, _logo: bool) -> Option<&'static str> {
    None
}

pub fn perform(_action: &str, _slot: usize, _store: &Store, _compact: bool) -> Outcome {
    Outcome::Done
}

pub async fn run(_job: Job, _conv: &crate::flow::Conversation) -> Result<String, String> {
    Err("not implemented".into())
}

pub fn reset() {}
