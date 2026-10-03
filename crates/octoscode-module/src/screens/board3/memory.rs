//! A36 — Memory (board 5, `design/stage-a/phase4-new5`): what Octos
//! remembers for the Session's profile. STUB (failing-first): the API the
//! tests in `tests/a36_memory.rs` are written against; nothing is drawn or
//! sent yet.
use serde_json::Value;

use octoscode_store::Store;

use super::host::Outcome;
use super::ui::{Dsl, Frame};

pub const OVERVIEW: &str = "memory/overview";
pub const ENTITY: &str = "memory/entity";
pub const SEARCH: &str = "memory/search";
pub const LOAD: &str = "memory/load";
pub const INGEST: &str = "memory/ingest";
pub const SEARCH_LIMIT: u64 = 20;
pub const NOTE_SOURCE: &str = "octoscode";

/// A record's tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Knowledge,
    Episode,
    Document,
}

/// The search's kind filter (All sends no `kinds`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KindFilter {
    #[default]
    All,
    Knowledge,
    Episode,
    Document,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    Trusted,
    Untrusted,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DailyNote {
    pub date: String,
    pub content: String,
    pub truncated: bool,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntitySummary {
    pub name: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Overview {
    pub long_term: String,
    pub long_term_updated_at: Option<String>,
    pub long_term_truncated: bool,
    pub long_term_total_bytes: u64,
    pub today: String,
    pub today_truncated: bool,
    pub today_total_bytes: u64,
    pub recent: Vec<DailyNote>,
    pub entities: Vec<EntitySummary>,
    pub entities_truncated: bool,
    pub staging_notes: u64,
    pub staging_truncated: bool,
    pub refresh_enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub id: String,
    pub kind: Kind,
    pub source: String,
    pub title: String,
    pub abstract_: String,
    pub score: f64,
    pub timestamp: String,
    pub trust: Trust,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: String,
    pub kind: Kind,
    pub source: String,
    pub timestamp: String,
    pub title: String,
    pub abstract_: String,
    pub body: Option<String>,
    pub trust: Trust,
    pub visits: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub record: Record,
    pub page: Option<String>,
    pub page_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntityPage {
    pub name: String,
    pub content: String,
    pub truncated: bool,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestReport {
    pub inserted: u64,
    pub updated: u64,
    pub unchanged: u64,
}

/// Which profile a reply says answered (the upstream proposal's echo,
/// `docs/proposals/memory-profile-scope.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answered {
    Profile(String),
    NotReported,
}

/// Why a memory read or write did not land — each class has one bounded
/// sentence for the problem and one for the next step (D1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Scope,
    OtherProfile(String),
    NotRunning,
    NotFound,
    Forbidden,
    Invalid,
    Unavailable,
    Offline,
    Failed,
    TooLong,
}

/// What failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Read,
    Search,
    Open,
    Page,
    Add,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub op: Op,
    pub refusal: Refusal,
}

/// The dialog's pages (board 5 frames 2-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Overview,
    Results,
    Record,
    Entity,
    LongTerm,
    /// A daily note: `None` = today, `Some(i)` = `recent[i]`.
    Day(Option<usize>),
    Add,
}

#[derive(Debug, Clone, Default)]
pub struct MemState {
    pub page: Page,
    pub profile: String,
    pub ticket: u64,
    pub loading: bool,
    pub overview: Option<Overview>,
    pub confirmed: bool,
    pub error: Option<Failure>,
    pub query: String,
    pub query_snap: String,
    pub kind: KindFilter,
    pub searched: Option<(String, KindFilter)>,
    pub searching: bool,
    pub hits: Option<Vec<Hit>>,
    pub search_error: Option<Failure>,
    pub record_loading: bool,
    pub record: Option<Loaded>,
    pub record_error: Option<Failure>,
    pub entity_name: String,
    pub entity_loading: bool,
    pub entity: Option<EntityPage>,
    pub entity_error: Option<Failure>,
    pub add_title: String,
    pub add_note: String,
    pub add_title_snap: String,
    pub add_note_snap: String,
    pub adding: bool,
    pub add_error: Option<Failure>,
    pub receipt: Option<&'static str>,
}

pub fn answered(_v: &Value) -> Answered {
    Answered::NotReported
}
pub fn parse_overview(_v: &Value) -> Option<Overview> {
    None
}
pub fn parse_hits(_v: &Value) -> Option<Vec<Hit>> {
    None
}
pub fn parse_load(_v: &Value) -> Option<Loaded> {
    None
}
pub fn parse_entity(_v: &Value) -> Option<EntityPage> {
    None
}
pub fn parse_ingest(_v: &Value) -> Option<IngestReport> {
    None
}
/// The Add-a-note receipt for the server's counts.
pub fn receipt(_r: &IngestReport) -> &'static str {
    ""
}
pub fn overview_params(_profile: &str) -> Value {
    Value::Null
}
pub fn search_params(_profile: &str, _query: &str, _kind: KindFilter) -> Value {
    Value::Null
}
pub fn load_params(_profile: &str, _id: &str) -> Value {
    Value::Null
}
pub fn entity_params(_profile: &str, _name: &str) -> Value {
    Value::Null
}
/// One note as the record `memory/ingest` takes (`key` = 16 hex digits).
pub fn note_record(_title: &str, _note: &str, _timestamp: &str, _key: &str) -> Result<Value, Refusal> {
    Err(Refusal::Failed)
}
pub fn ingest_params(_profile: &str, _record: Value) -> Value {
    Value::Null
}
pub fn classify(_e: &octoscode_client::ClientError, _confirmed: bool) -> Refusal {
    Refusal::Failed
}
/// The bounded copy for a failure: (lead, problem, next step).
pub fn copy(_f: &Failure, _profile: &str) -> (&'static str, String, String) {
    ("", String::new(), String::new())
}
/// The scope line under the title (D1: plain text, the muted ink).
pub fn scope_line(_profile: &str) -> String {
    String::new()
}
/// Whether "Add note" is offered: the overview confirmed the profile and the
/// server advertises `memory/ingest`.
pub fn can_add(_st: &MemState, _store: &Store) -> bool {
    false
}

pub fn on_open(_st: &mut MemState) -> Outcome {
    Outcome::Done
}
pub fn perform(_st: &mut MemState, _action: &str, _index: usize, _store: &Store) -> Outcome {
    Outcome::Unrouted
}
pub fn input_changed(_st: &mut MemState, _key: &str, _text: &str) {}
pub fn input_returned(_st: &mut MemState, _key: &str, _store: &Store) -> Outcome {
    Outcome::Done
}
/// A job could not run (no live conversation).
pub fn job_unavailable(_st: &mut MemState, _job: &super::host::Job) {}
pub fn visibility(_st: &MemState) -> Vec<(String, bool)> {
    Vec::new()
}
/// D4 — Memory follows the app theme (frame 11 is the dark reference).
pub fn follow_theme(dsl: &str) -> String {
    dsl.to_owned()
}
pub fn build(_d: &mut Dsl, _st: &MemState, _frame: &Frame, _store: &Store) {}

pub async fn load_overview(_conv: &crate::flow::Conversation, _ticket: u64) -> Result<String, String> {
    Err("stub".into())
}
pub async fn search(_conv: &crate::flow::Conversation, _ticket: u64) -> Result<String, String> {
    Err("stub".into())
}
pub async fn open_record(_conv: &crate::flow::Conversation, _ticket: u64, _id: String) -> Result<String, String> {
    Err("stub".into())
}
pub async fn open_entity(_conv: &crate::flow::Conversation, _ticket: u64, _name: String) -> Result<String, String> {
    Err("stub".into())
}
pub async fn add_note(_conv: &crate::flow::Conversation, _ticket: u64) -> Result<String, String> {
    Err("stub".into())
}
