//! A31 — the Skills dialog's "Background jobs" section (parity row 15).
//!
//! FAILING-FIRST STUB: the API the row-15 tests address, with main's
//! behaviour (no read, no section).
use octoscode_store::domains::skill_jobs::{JobStatus, ListState, SkillJob};
use octoscode_store::Store;

use crate::flow::Conversation;
use crate::screens::board3::ui::tok;

pub use octoscode_client::domains::skill_jobs::{FEATURE, LIST_METHOD};

pub const MAX_ROWS: usize = 20;
pub const HEADING: &str = "Background jobs";
pub const RUNNING_COUNT: &str = "{count} running";
pub const QUEUED_COUNT: &str = "{value0} queued";
pub const FAILED_LEAD: &str = "Couldn't finish this job.";
pub const ABANDONED_NOTE: &str = "The server restarted before this job finished.";
pub const EMPTY: &str = "No background jobs in this Session.";
pub const LOADING: &str = "Loading background jobs…";
pub const LOAD_FAILED: &str = "Couldn't load background jobs.";
pub const ANNOUNCED_ONLY: &str =
    "Only jobs announced since the app connected are shown; this server doesn't list earlier jobs.";
pub const OMITTED: &str = "(+{value0} omitted)";

pub fn seeds(_store: &Store) -> bool {
    false
}

pub fn scope(_store: &Store) -> Option<(String, String)> {
    None
}

pub async fn refresh(_conv: &Conversation, _store: &Store) -> Result<String, String> {
    Ok(String::new())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chip {
    pub glyph: &'static str,
    pub word: &'static str,
    pub fg: &'static str,
    pub bg: &'static str,
}

pub fn chip(_status: &JobStatus) -> Chip {
    Chip { glyph: "?", word: "", fg: tok::MUTED, bg: tok::CHIP }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    None,
    Output(String),
    Failure { lead: &'static str, cause: String },
    Note(&'static str),
}

pub fn message(_job: &SkillJob) -> Message {
    Message::None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub job_id: String,
    pub skill: String,
    pub action: String,
    pub name: String,
    pub chip: Chip,
    pub time: String,
    pub message: Message,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub running: usize,
    pub queued: usize,
    pub announced_only: bool,
    pub state: ListState,
    pub rows: Vec<Row>,
    pub omitted: usize,
}

pub fn section(_store: &Store, _now_ms: u64) -> Option<Section> {
    None
}
