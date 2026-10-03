//! A36 — Memory (board 5, `design/stage-a/phase4-new5`): what Octos
//! remembers for the Session's profile — its long-term memory (`MEMORY.md`),
//! today's and the last week's daily notes, the entity bank, the Recall
//! search, one record, and "Add note". Reached by a click: Settings >
//! Capabilities > Memory (`b3.open.memory`), shown only when the server
//! advertises `memory/overview`.
//!
//! # The operator's decisions (board 5, 2026-10-03)
//!
//! - **D1** (yes, two fixes): the scope line "Server Profile: dsflash" is
//!   plain text in the theme's muted ink, not monospace; a refusal explains
//!   the problem and the next step in bounded copy ([`copy`]) and never
//!   shows the server's raw error.
//! - **D2 A**: Memory is built for the SESSION's profile. Every call names it
//!   (`profile_id`, the upstream proposal `docs/proposals/memory-profile-
//!   scope.md`), and a reply is shown as that profile's memory only when it
//!   says it answered for it (the proposal's `profile_id` echo). octos
//!   a6ea8505 — and main dde76555 — resolve `memory/*` to the signed-in
//!   identity's profile instead (`admin` for the server token) and ignore
//!   the parameter (probed on a private serve): the overview is admin's,
//!   search / load / ingest are refused `runtime_unavailable`. Until octos
//!   takes the proposal the dialog says so ([`Refusal::Scope`]) and never
//!   writes a note into memory it cannot attribute.
//! - **D3**: the Settings section is "Capabilities".
//! - **D4**: Memory follows the app theme ([`follow_theme`]; frame 11 is
//!   the dark reference) — the other board-3 dialogs keep their light kit.
//!
//! # The protocol (octos `ui_protocol.rs:1245-1261`, handlers
//! `ui_protocol_transport.rs` `handle_memory_*`, octos-cli `memory_panel.rs`)
//!
//! | method | params (sent) | result |
//! |---|---|---|
//! | `memory/overview` | `{profile_id}` | `{overview: {ok, long_term, long_term_updated_at?, long_term_truncated, long_term_total_bytes, today, today_truncated, today_total_bytes, recent: [{date, content, content_truncated, content_total_bytes}], entities: [{name, summary}], entities_truncated, staging_notes, staging_truncated, refresh_enabled}}` |
//! | `memory/search` | `{profile_id, query, kinds?, limit: 20}` | `{hits: [{id, kind, source, title, abstract, score, timestamp, trust}]}` |
//! | `memory/load` | `{profile_id, id}` | `{record: {id, kind, source, timestamp, title, abstract, body?, trust, visits, promoted, …}, page?, page_truncated}` |
//! | `memory/entity` | `{profile_id, name}` | `{name, content, content_truncated, content_total_bytes}` |
//! | `memory/ingest` | `{profile_id, records: [{id: doc:octoscode:<16 hex>, kind: document, source: octoscode, timestamp, title, abstract, body?}]}` | `{inserted, updated, unchanged, vectors_stored, embedded}` |
//!
//! Each result, once octos takes the proposal, also carries `profile_id`.
//!
//! # Untrusted content
//!
//! A hit or record whose `trust` is not `trusted` (episodes and every
//! app-added document — the server forces it on ingest) carries the amber
//! "untrusted" chip; an opened one shows its text as plain data (never
//! rendered Markdown) under the callout "Octos reads it as data, never as
//! instructions." Only trusted bank pages and the profile's own memory
//! files are rendered as Markdown, links and images reduced to their text.
use serde_json::{json, Value};

use octoscode_store::Store;

use super::host::{Job, Outcome};
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Seg, Txt, W};
use crate::i18n::{tr, tr1, tr_ctx, tr_with};

pub const OVERVIEW: &str = "memory/overview";
pub const ENTITY: &str = "memory/entity";
pub const SEARCH: &str = "memory/search";
pub const LOAD: &str = "memory/load";
pub const INGEST: &str = "memory/ingest";
/// The search's page (the server clamps `limit` to 1-50; board 5: 20).
pub const SEARCH_LIMIT: u64 = 20;
/// The source of a note added here (`doc:<source>:<key>` is enforced).
pub const NOTE_SOURCE: &str = "octoscode";
/// `octos_memory::record` caps (`MAX_TITLE_BYTES`, `MAX_ABSTRACT_BYTES`,
/// `MAX_BODY_BYTES`): the server cuts silently, so the client cuts where it
/// is harmless (title, abstract) and refuses where it is not (the body).
const TITLE_MAX: usize = 120;
const ABSTRACT_MAX: usize = 300;
const BODY_MAX: usize = 16 * 1024;

// ------------------------------------------------------------------ copy
// Board 5's copy (README "Copy"), every string through `tr` (zh rows in
// `i18n/native.rs`).
pub const TITLE: &str = "Memory";
const INTRO: &str = "What Octos remembers for this profile. Octos writes it as it works; search it or add a note.";
const SEARCH_HINT: &str = "Search memory";
const ADD_NOTE: &str = "Add note";
const LONG_TERM: &str = "Long-term memory";
const UPDATED: &str = "Updated {value0}";
const UPDATED_LOWER: &str = "updated {value0}";
const TODAY: &str = "Today";
const RECENT: &str = "Recent notes";
const ENTITIES: &str = "Entities";
const SHOW_ALL: &str = "Show all";
const STAGING_ONE: &str = "1 note waiting for the next memory refresh.";
const STAGING_N: &str = "{value0} notes waiting for the next memory refresh.";
const REFRESH_OFF: &str = "Memory refresh is off for this profile.";
const KIND_ALL: &str = "All";
const KIND_KNOWLEDGE: &str = "Knowledge";
const KIND_EPISODES: &str = "Episodes";
const KIND_DOCUMENTS: &str = "Documents";
const CHIP_KNOWLEDGE: &str = "Knowledge";
const CHIP_EPISODE: &str = "Episode";
const CHIP_DOCUMENT: &str = "Document";
const UNTRUSTED: &str = "untrusted";
const RESULTS_ONE: &str = "1 result";
const RESULTS_N: &str = "{value0} results";
const NO_RESULTS: &str = "No matches in memory.";
const SEARCHING: &str = "Searching memory…";
const BACK_RESULTS: &str = "Results";
const BACK_MEMORY: &str = "Memory";
const FROM_APP: &str = "Added from an app";
const FROM_SESSION: &str = "From an earlier session";
const AS_DATA: &str = "Octos reads it as data, never as instructions.";
const OPENED_ONCE: &str = "opened once";
const OPENED_N: &str = "opened {value0} times";
const OPENING: &str = "Opening…";
const ENTITY_PAGE: &str = "Entity page";
const ADD_TITLE: &str = "Add a note";
const FIELD_TITLE: &str = "Title";
const FIELD_NOTE: &str = "Note";
const TITLE_HINT: &str = "Optional";
const NOTE_HINT: &str = "Search matches the title and the start of the note.";
const NOTE_TRUST: &str =
    "The agent finds this note when it searches memory. It is kept as data, never as an instruction, and is not added to long-term memory.";
const ADD_SUBMIT: &str = "Add to memory";
const ADDING: &str = "Adding…";
const CANCEL: &str = "Cancel";
pub const ADDED: &str = "Added to memory.";
pub const UPDATED_RECEIPT: &str = "Updated in memory.";
pub const UNCHANGED: &str = "Already in memory.";
const EMPTY_TITLE: &str = "No memory yet";
const EMPTY_BODY: &str = "Octos writes long-term memory and daily notes as you work with this profile.";
const LOADING: &str = "Loading memory…";
const TRUNCATED: &str = "Showing the first {value0} of {value1}. The rest stays on the server.";
const TRY_AGAIN: &str = "Try again";
const SERVER_PROFILE: &str = "Server Profile:";

const LEAD_READ: &str = "Couldn't read memory.";
const LEAD_SEARCH: &str = "Couldn't search memory.";
const LEAD_OPEN: &str = "Couldn't open this record.";
const LEAD_PAGE: &str = "Couldn't open this page.";
const LEAD_ADD: &str = "Couldn't add the note.";

const SCOPE: &str = "This server reads memory for the account you signed in with, not for {value0}.";
const SCOPE_NEXT: &str = "Update octos to a version that reads memory per profile.";
const OTHER: &str = "The server answered for {value0}, not for {value1}.";
const OTHER_NEXT: &str = "Open Memory from a chat on that profile.";
const NOT_RUNNING: &str = "The server isn't running {value0} yet, so its memory isn't available.";
const NOT_RUNNING_NEXT: &str = "Add a model provider for this profile, then try again.";
const NOT_FOUND: &str = "It is no longer in this profile's memory.";
const NOT_FOUND_NEXT: &str = "Refresh to see what is there now.";
const FORBIDDEN: &str = "This sign-in can't read the memory of {value0}.";
const FORBIDDEN_NEXT: &str = "Sign in with an account that owns this profile.";
const INVALID: &str = "The server didn't accept this request.";
const INVALID_NEXT: &str = "Check what you typed, then try again.";
const UNAVAILABLE: &str = "This server doesn't offer this part of memory.";
const UNAVAILABLE_NEXT: &str = "Connect to a server that does.";
const OFFLINE: &str = "The connection to the server dropped.";
const OFFLINE_NEXT: &str = "Reconnect, then try again.";
const FAILED: &str = "The server couldn't finish this request.";
const FAILED_NEXT: &str = "Try again in a moment.";
const TOO_LONG: &str = "This note is longer than memory keeps (16 KB).";
const TOO_LONG_NEXT: &str = "Shorten it, then add it again.";

// ---------------------------------------------------------------- model

/// A record's tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Knowledge,
    Episode,
    Document,
}

impl Kind {
    /// `RecordKind` as the server writes it (snake_case singular).
    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s {
            "knowledge" => Kind::Knowledge,
            "episode" => Kind::Episode,
            "document" => Kind::Document,
            _ => return None,
        })
    }

    fn chip(self) -> &'static str {
        match self {
            Kind::Knowledge => CHIP_KNOWLEDGE,
            Kind::Episode => CHIP_EPISODE,
            Kind::Document => CHIP_DOCUMENT,
        }
    }
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

impl KindFilter {
    const ALL: [KindFilter; 4] = [KindFilter::All, KindFilter::Knowledge, KindFilter::Episode, KindFilter::Document];

    fn wire(self) -> Option<&'static str> {
        match self {
            KindFilter::All => None,
            KindFilter::Knowledge => Some("knowledge"),
            KindFilter::Episode => Some("episode"),
            KindFilter::Document => Some("document"),
        }
    }

    fn id(self) -> &'static str {
        self.wire().unwrap_or("all")
    }

    fn label(self) -> &'static str {
        match self {
            // "All" alone is the sidebar's "In one list" key: a context row.
            KindFilter::All => tr_ctx("memory", KIND_ALL),
            KindFilter::Knowledge => tr(KIND_KNOWLEDGE),
            KindFilter::Episode => tr(KIND_EPISODES),
            KindFilter::Document => tr(KIND_DOCUMENTS),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    Trusted,
    Untrusted,
}

impl Trust {
    /// Anything but `trusted` is untrusted (fail safe).
    fn parse(s: Option<&str>) -> Trust {
        if s == Some("trusted") {
            Trust::Trusted
        } else {
            Trust::Untrusted
        }
    }
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

impl Overview {
    fn is_empty(&self) -> bool {
        self.long_term.trim().is_empty() && self.today.trim().is_empty() && self.recent.is_empty() && self.entities.is_empty()
    }
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
/// sentence for the problem and one for the next step ([`copy`], D1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The server does not say it answered for the Session's profile (octos
    /// before the proposal: it answers for the signed-in account).
    Scope,
    /// The server answered for another profile (named).
    OtherProfile(String),
    /// The profile has no runtime on the server (no model provider).
    NotRunning,
    NotFound,
    Forbidden,
    Invalid,
    /// The method is not advertised / not supported.
    Unavailable,
    /// No connection.
    Offline,
    Failed,
    /// A note past the server's 16 KiB body cap.
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
    /// The Session's profile every call names (set when a job runs).
    pub profile: String,
    /// The overview's generation (a reply for an older one is dropped).
    pub ticket: u64,
    pub loading: bool,
    pub overview: Option<Overview>,
    /// The overview said it answered for [`MemState::profile`].
    pub confirmed: bool,
    pub error: Option<Failure>,
    /// The live search text (every keystroke) and the snapshot the DSL
    /// embeds (refreshed only on a structural remount, so typing never
    /// rebuilds the field).
    pub query: String,
    pub query_snap: String,
    pub kind: KindFilter,
    /// The query + kind the results answer.
    pub searched: Option<(String, KindFilter)>,
    pub search_ticket: u64,
    pub searching: bool,
    pub hits: Option<Vec<Hit>>,
    pub search_error: Option<Failure>,
    pub record_ticket: u64,
    pub record_id: String,
    pub record_loading: bool,
    pub record: Option<Loaded>,
    pub record_error: Option<Failure>,
    pub entity_ticket: u64,
    pub entity_name: String,
    pub entity_loading: bool,
    pub entity: Option<EntityPage>,
    pub entity_error: Option<Failure>,
    pub add_title: String,
    pub add_note: String,
    pub add_title_snap: String,
    pub add_note_snap: String,
    pub add_ticket: u64,
    pub adding: bool,
    pub add_error: Option<Failure>,
    /// The last landed note's receipt (shown on the overview).
    pub receipt: Option<&'static str>,
}

// --------------------------------------------------------------- parsing

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k)?.as_str().map(str::to_owned)
}

fn flag(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}

/// The profile a result says answered (the proposal's top-level echo).
pub fn answered(v: &Value) -> Answered {
    match v.get("profile_id").and_then(Value::as_str) {
        Some(p) if !p.is_empty() => Answered::Profile(p.to_owned()),
        _ => Answered::NotReported,
    }
}

/// `memory/overview` -> the panel body (`MemoryOverviewResponse` + the RPC
/// layer's truncation fields). The panel's own fields are required; the
/// RPC truncation fields default to "not cut" (an older server has none).
pub fn parse_overview(v: &Value) -> Option<Overview> {
    let o = v.get("overview")?;
    let long_term = s(o, "long_term")?;
    let today = s(o, "today")?;
    let recent = o
        .get("recent")?
        .as_array()?
        .iter()
        .map(|n| {
            let content = s(n, "content")?;
            Some(DailyNote {
                date: s(n, "date")?,
                total_bytes: n.get("content_total_bytes").and_then(Value::as_u64).unwrap_or(content.len() as u64),
                truncated: flag(n, "content_truncated"),
                content,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let entities = o
        .get("entities")?
        .as_array()?
        .iter()
        .map(|e| Some(EntitySummary { name: s(e, "name")?, summary: s(e, "summary").unwrap_or_default() }))
        .collect::<Option<Vec<_>>>()?;
    Some(Overview {
        long_term_total_bytes: o.get("long_term_total_bytes").and_then(Value::as_u64).unwrap_or(long_term.len() as u64),
        long_term_truncated: flag(o, "long_term_truncated"),
        long_term_updated_at: s(o, "long_term_updated_at"),
        long_term,
        today_total_bytes: o.get("today_total_bytes").and_then(Value::as_u64).unwrap_or(today.len() as u64),
        today_truncated: flag(o, "today_truncated"),
        today,
        recent,
        entities,
        entities_truncated: flag(o, "entities_truncated"),
        staging_notes: o.get("staging_notes")?.as_u64()?,
        staging_truncated: flag(o, "staging_truncated"),
        refresh_enabled: o.get("refresh_enabled")?.as_bool()?,
    })
}

/// `memory/search` -> the hits, best first. A hit of a kind this client does
/// not know is left out (a newer server's tier), never mislabelled.
pub fn parse_hits(v: &Value) -> Option<Vec<Hit>> {
    let mut out = Vec::new();
    for h in v.get("hits")?.as_array()? {
        let Some(kind) = h.get("kind").and_then(Value::as_str).and_then(Kind::parse) else { continue };
        out.push(Hit {
            id: s(h, "id")?,
            kind,
            source: s(h, "source").unwrap_or_default(),
            title: s(h, "title").unwrap_or_default(),
            abstract_: s(h, "abstract").unwrap_or_default(),
            score: h.get("score").and_then(Value::as_f64).unwrap_or(0.0),
            timestamp: s(h, "timestamp").unwrap_or_default(),
            trust: Trust::parse(h.get("trust").and_then(Value::as_str)),
        });
    }
    Some(out)
}

/// `memory/load` -> the record (+ the bank page for a knowledge record).
pub fn parse_load(v: &Value) -> Option<Loaded> {
    let r = v.get("record")?;
    let record = Record {
        id: s(r, "id")?,
        kind: Kind::parse(r.get("kind")?.as_str()?)?,
        source: s(r, "source").unwrap_or_default(),
        timestamp: s(r, "timestamp").unwrap_or_default(),
        title: s(r, "title").unwrap_or_default(),
        abstract_: s(r, "abstract").unwrap_or_default(),
        body: s(r, "body").filter(|b| !b.trim().is_empty()),
        trust: Trust::parse(r.get("trust").and_then(Value::as_str)),
        visits: r.get("visits").and_then(Value::as_u64).unwrap_or(0),
    };
    Some(Loaded { record, page: s(v, "page"), page_truncated: flag(v, "page_truncated") })
}

/// `memory/entity` -> the page.
pub fn parse_entity(v: &Value) -> Option<EntityPage> {
    let content = s(v, "content")?;
    Some(EntityPage {
        name: s(v, "name")?,
        total_bytes: v.get("content_total_bytes").and_then(Value::as_u64).unwrap_or(content.len() as u64),
        truncated: flag(v, "content_truncated"),
        content,
    })
}

/// `memory/ingest` -> the upsert counts.
pub fn parse_ingest(v: &Value) -> Option<IngestReport> {
    Some(IngestReport {
        inserted: v.get("inserted")?.as_u64()?,
        updated: v.get("updated")?.as_u64()?,
        unchanged: v.get("unchanged")?.as_u64()?,
    })
}

/// The Add-a-note receipt for the server's counts (board 5 README).
pub fn receipt(r: &IngestReport) -> &'static str {
    if r.inserted > 0 {
        ADDED
    } else if r.updated > 0 {
        UPDATED_RECEIPT
    } else {
        UNCHANGED
    }
}

// ---------------------------------------------------------------- params

pub fn overview_params(profile: &str) -> Value {
    json!({"profile_id": profile})
}

pub fn search_params(profile: &str, query: &str, kind: KindFilter) -> Value {
    let mut p = json!({"profile_id": profile, "query": query.trim(), "limit": SEARCH_LIMIT});
    if let Some(k) = kind.wire() {
        p["kinds"] = json!([k]);
    }
    p
}

pub fn load_params(profile: &str, id: &str) -> Value {
    json!({"profile_id": profile, "id": id})
}

pub fn entity_params(profile: &str, name: &str) -> Value {
    json!({"profile_id": profile, "name": name})
}

pub fn ingest_params(profile: &str, record: Value) -> Value {
    json!({"profile_id": profile, "records": [record]})
}

/// The longest prefix of `s` that fits `max` bytes on a char boundary.
fn cut_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// One note as the record `memory/ingest` takes (`key` = 16 hex digits):
/// `doc:octoscode:<key>`, a document from `octoscode`, the title (or the
/// note's first line), the abstract = the note's first 300 bytes (what
/// search matches, with the title), the whole note as the body when it is
/// longer. No `trust`: the server forces every ingested record untrusted.
pub fn note_record(title: &str, note: &str, timestamp: &str, key: &str) -> Result<Value, Refusal> {
    let note = note.trim();
    if note.is_empty() {
        return Err(Refusal::Invalid);
    }
    if note.len() > BODY_MAX {
        return Err(Refusal::TooLong);
    }
    let title = match title.trim() {
        "" => note.lines().next().unwrap_or("").trim(),
        t => t,
    };
    let abstract_ = cut_bytes(note, ABSTRACT_MAX).trim_end();
    let mut r = json!({
        "id": format!("doc:{NOTE_SOURCE}:{key}"),
        "kind": "document",
        "source": NOTE_SOURCE,
        "timestamp": timestamp,
        "title": cut_bytes(title, TITLE_MAX).trim_end(),
        "abstract": abstract_,
    });
    if abstract_.len() < note.len() {
        r["body"] = json!(note);
    }
    Ok(r)
}

/// A fresh 16-hex-digit key per note (process-random, time-mixed).
fn note_key() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    format!("{:016x}", h.finish())
}

// --------------------------------------------------------------- refusals

/// The refusal class of a failed call, from the server's code and
/// `data.kind` — never its message. `confirmed`: the overview named the
/// Session's profile, so `runtime_unavailable` is THAT profile's runtime;
/// before the proposal it is the signed-in account's (the scope finding).
pub fn classify(e: &octoscode_client::ClientError, confirmed: bool) -> Refusal {
    use octoscode_client::ClientError;
    match e {
        ClientError::Rpc { error, .. } => {
            let kind = error.data.as_ref().and_then(|d| d.get("kind")).and_then(Value::as_str).unwrap_or("");
            match (error.code, kind) {
                (_, "runtime_unavailable" | "runtime_not_ready") | (-32140, _) => {
                    if confirmed {
                        Refusal::NotRunning
                    } else {
                        Refusal::Scope
                    }
                }
                (_, "not_found") | (-32170, _) => Refusal::NotFound,
                (_, "forbidden" | "permission_denied") | (-32003 | -32120, _) => Refusal::Forbidden,
                (_, "method_not_supported" | "unsupported_capability") | (-32601 | -32004 | -32130, _) => Refusal::Unavailable,
                (-32602, _) => Refusal::Invalid,
                _ => Refusal::Failed,
            }
        }
        ClientError::Transport { .. } => Refusal::Offline,
        ClientError::Decode { .. } => Refusal::Failed,
    }
}

/// The bounded copy for a failure (D1): the lead (an English key, drawn
/// through `tr`), the problem in one sentence and the next step in one —
/// in the current language, never the server's text.
pub fn copy(f: &Failure, profile: &str) -> (&'static str, String, String) {
    let lead = match f.op {
        Op::Read => LEAD_READ,
        Op::Search => LEAD_SEARCH,
        Op::Open => LEAD_OPEN,
        Op::Page => LEAD_PAGE,
        Op::Add => LEAD_ADD,
    };
    let two = |a: &str, b: &str| (tr(a).to_owned(), tr(b).to_owned());
    let (problem, next) = match &f.refusal {
        Refusal::Scope => (tr1(SCOPE, profile), tr(SCOPE_NEXT).to_owned()),
        Refusal::OtherProfile(p) => (tr_with(OTHER, &[("value0", p), ("value1", profile)]), tr(OTHER_NEXT).to_owned()),
        Refusal::NotRunning => (tr1(NOT_RUNNING, profile), tr(NOT_RUNNING_NEXT).to_owned()),
        Refusal::NotFound => two(NOT_FOUND, NOT_FOUND_NEXT),
        Refusal::Forbidden => (tr1(FORBIDDEN, profile), tr(FORBIDDEN_NEXT).to_owned()),
        Refusal::Invalid => two(INVALID, INVALID_NEXT),
        Refusal::Unavailable => two(UNAVAILABLE, UNAVAILABLE_NEXT),
        Refusal::Offline => two(OFFLINE, OFFLINE_NEXT),
        Refusal::Failed => two(FAILED, FAILED_NEXT),
        Refusal::TooLong => two(TOO_LONG, TOO_LONG_NEXT),
    };
    (lead, problem, next)
}

/// The scope line under the title (D1: plain text, the muted ink). The
/// web's "Server Profile:" key; Chinese takes no space after its colon.
pub fn scope_line(profile: &str) -> String {
    if crate::i18n::is_zh() {
        format!("{}{profile}", tr(SERVER_PROFILE))
    } else {
        format!("{} {profile}", tr(SERVER_PROFILE))
    }
}

fn advertised(store: &Store, method: &str) -> bool {
    store.domains.config.supported_methods().iter().any(|m| m == method)
}

/// Whether "Add note" is offered: the overview confirmed the Session's
/// profile (a note must never land in memory the server cannot attribute)
/// and the server advertises `memory/ingest`.
pub fn can_add(st: &MemState, store: &Store) -> bool {
    st.confirmed && advertised(store, INGEST)
}

// ---------------------------------------------------------------- actions

/// A fresh open: nothing of a previous profile's memory survives it.
pub fn on_open(st: &mut MemState) -> Outcome {
    let ticket = st.ticket + 1;
    *st = MemState {
        ticket,
        search_ticket: st.search_ticket,
        record_ticket: st.record_ticket,
        entity_ticket: st.entity_ticket,
        add_ticket: st.add_ticket,
        loading: true,
        ..Default::default()
    };
    Outcome::Spawn(Job::MemoryOverview(ticket))
}

fn reload(st: &mut MemState) -> Outcome {
    st.ticket += 1;
    st.loading = true;
    st.error = None;
    Outcome::Spawn(Job::MemoryOverview(st.ticket))
}

fn start_search(st: &mut MemState) -> Outcome {
    let q = st.query.trim().to_owned();
    if q.is_empty() {
        return Outcome::Done; // the server refuses a blank query (-32602)
    }
    st.searched = Some((q, st.kind));
    st.search_ticket += 1;
    st.searching = true;
    st.search_error = None;
    st.query_snap = st.query.clone();
    st.page = Page::Results;
    st.receipt = None;
    Outcome::Spawn(Job::MemorySearch(st.search_ticket))
}

fn open_record_at(st: &mut MemState, index: usize) -> Outcome {
    let Some(hit) = st.hits.as_ref().and_then(|h| h.get(index)) else { return Outcome::Done };
    st.record_id = hit.id.clone();
    st.record_ticket += 1;
    st.record_loading = true;
    st.record = None;
    st.record_error = None;
    st.page = Page::Record;
    Outcome::Spawn(Job::MemoryLoad(st.record_ticket, st.record_id.clone()))
}

fn open_entity_named(st: &mut MemState, name: String) -> Outcome {
    st.entity_name = name;
    st.entity_ticket += 1;
    st.entity_loading = true;
    st.entity = None;
    st.entity_error = None;
    st.page = Page::Entity;
    Outcome::Spawn(Job::MemoryEntity(st.entity_ticket, st.entity_name.clone()))
}

pub fn perform(st: &mut MemState, action: &str, index: usize, store: &Store) -> Outcome {
    if let Some(k) = action.strip_prefix("b3.mem.kind.") {
        let Some(kind) = KindFilter::ALL.into_iter().find(|f| f.id() == k) else { return Outcome::Done };
        st.kind = kind;
        st.query_snap = st.query.clone();
        return start_search(st);
    }
    match action {
        "b3.mem.refresh" => {
            if st.page == Page::Results && st.searched.is_some() {
                if st.searching {
                    return Outcome::Done;
                }
                return start_search(st);
            }
            if st.loading {
                return Outcome::Done;
            }
            st.receipt = None;
            st.query_snap = st.query.clone();
            reload(st)
        }
        "b3.mem.retry" => match st.page {
            Page::Results => start_search(st),
            Page::Record => {
                let id = st.record_id.clone();
                st.record_ticket += 1;
                st.record_loading = true;
                st.record_error = None;
                Outcome::Spawn(Job::MemoryLoad(st.record_ticket, id))
            }
            Page::Entity => open_entity_named(st, st.entity_name.clone()),
            _ => reload(st),
        },
        "b3.mem.add" => {
            if !can_add(st, store) {
                return Outcome::Done;
            }
            st.page = Page::Add;
            st.add_error = None;
            st.receipt = None;
            st.add_title_snap = st.add_title.clone();
            st.add_note_snap = st.add_note.clone();
            Outcome::Done
        }
        "b3.mem.search" => start_search(st),
        "b3.mem.clear" => {
            st.query.clear();
            st.query_snap.clear();
            st.searched = None;
            st.hits = None;
            st.search_error = None;
            st.searching = false;
            st.page = Page::Overview;
            Outcome::Done
        }
        "b3.mem.open" => open_record_at(st, index),
        "b3.mem.entity" => {
            let Some(name) = st.overview.as_ref().and_then(|o| o.entities.get(index)).map(|e| e.name.clone()) else {
                return Outcome::Done;
            };
            open_entity_named(st, name)
        }
        "b3.mem.lt" if st.overview.is_some() => {
            st.page = Page::LongTerm;
            Outcome::Done
        }
        "b3.mem.today" if st.overview.is_some() => {
            st.page = Page::Day(None);
            Outcome::Done
        }
        "b3.mem.day" if st.overview.as_ref().is_some_and(|o| index < o.recent.len()) => {
            st.page = Page::Day(Some(index));
            Outcome::Done
        }
        "b3.mem.back" => {
            st.query_snap = st.query.clone();
            st.page = match st.page {
                Page::Record if st.hits.is_some() || st.searched.is_some() => Page::Results,
                _ => Page::Overview,
            };
            Outcome::Done
        }
        "b3.mem.add.cancel" => {
            st.add_title.clear();
            st.add_note.clear();
            st.add_title_snap.clear();
            st.add_note_snap.clear();
            st.add_error = None;
            st.page = Page::Overview;
            Outcome::Done
        }
        "b3.mem.add.submit" => {
            if st.adding || !can_add(st, store) || st.add_note.trim().is_empty() {
                return Outcome::Done;
            }
            st.adding = true;
            st.add_error = None;
            st.add_title_snap = st.add_title.clone();
            st.add_note_snap = st.add_note.clone();
            st.add_ticket += 1;
            super::host::request_blur();
            Outcome::Spawn(Job::MemoryIngest(st.add_ticket))
        }
        _ => Outcome::Unrouted,
    }
}

pub fn input_changed(st: &mut MemState, key: &str, text: &str) {
    match key {
        "mem.query" => st.query = text.to_owned(),
        "mem.add.title" => st.add_title = text.to_owned(),
        "mem.add.note" => st.add_note = text.to_owned(),
        _ => {}
    }
}

/// Return in the search field runs the search (never on every keystroke).
pub fn input_returned(st: &mut MemState, key: &str, _store: &Store) -> Outcome {
    if key == "mem.query" {
        return start_search(st);
    }
    Outcome::Done
}

/// The post-mount visibility the inputs drive without a remount: the
/// search's clear control, the note form's armed / unarmed submit.
pub fn visibility(st: &MemState) -> Vec<(String, bool)> {
    let armed = !st.add_note.trim().is_empty() && !st.adding;
    vec![
        ("b3_mem_clear_box".to_owned(), !st.query.is_empty()),
        ("b3_mem_add_submit_on".to_owned(), armed),
        ("b3_mem_add_submit_off".to_owned(), !armed),
    ]
}

/// A job could not run (no live conversation): fail closed, visibly.
pub fn job_unavailable(st: &mut MemState, job: &Job) {
    let offline = |op| Some(Failure { op, refusal: Refusal::Offline });
    match job {
        Job::MemoryOverview(_) => {
            st.loading = false;
            st.error = offline(Op::Read);
        }
        Job::MemorySearch(_) => {
            st.searching = false;
            st.search_error = offline(Op::Search);
        }
        Job::MemoryLoad(..) => {
            st.record_loading = false;
            st.record_error = offline(Op::Open);
        }
        Job::MemoryEntity(..) => {
            st.entity_loading = false;
            st.entity_error = offline(Op::Page);
        }
        Job::MemoryIngest(_) => {
            st.adding = false;
            st.add_error = offline(Op::Add);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------- the jobs

/// One call: the capability gate, the request, the refusal class.
async fn call(conv: &crate::flow::Conversation, method: &str, params: Value, confirmed: bool) -> Result<Value, Refusal> {
    if !advertised(&conv.store, method) {
        return Err(Refusal::Unavailable);
    }
    conv.client().request(method, params).await.map_err(|e| classify(&e, confirmed))
}

/// The reply is this profile's, or the refusal that says whose it is not.
fn attributed(v: &Value, profile: &str) -> Result<(), Refusal> {
    match answered(v) {
        Answered::Profile(p) if p == profile => Ok(()),
        Answered::Profile(p) => Err(Refusal::OtherProfile(p)),
        Answered::NotReported => Err(Refusal::Scope),
    }
}

fn begin(conv: &crate::flow::Conversation) -> (String, bool) {
    let profile = conv.profile();
    let mut st = super::host::state();
    st.mem.profile = profile.clone();
    (profile, st.mem.confirmed)
}

pub async fn load_overview(conv: &crate::flow::Conversation, ticket: u64) -> Result<String, String> {
    let (profile, _) = begin(conv);
    // The overview decides the scope: a refusal before it is the scope's.
    let res = call(conv, OVERVIEW, overview_params(&profile), false).await;
    let mut st = super::host::state();
    let m = &mut st.mem;
    if m.ticket != ticket {
        return Ok("stale memory/overview reply dropped".into());
    }
    m.loading = false;
    let out = res.and_then(|v| {
        attributed(&v, &profile)?;
        parse_overview(&v).ok_or(Refusal::Failed)
    });
    super::host::wake();
    match out {
        Ok(o) => {
            let n = o.entities.len();
            m.overview = Some(o);
            m.confirmed = true;
            m.error = None;
            Ok(format!("memory overview for {profile}: {n} entities"))
        }
        Err(refusal) => {
            m.overview = None;
            m.confirmed = false;
            let why = format!("memory overview refused: {refusal:?}");
            m.error = Some(Failure { op: Op::Read, refusal });
            Err(why)
        }
    }
}

pub async fn search(conv: &crate::flow::Conversation, ticket: u64) -> Result<String, String> {
    let (profile, confirmed) = begin(conv);
    let Some((query, kind)) = super::host::state().mem.searched.clone() else {
        return Ok("no search".into());
    };
    let res = call(conv, SEARCH, search_params(&profile, &query, kind), confirmed).await;
    let mut st = super::host::state();
    let m = &mut st.mem;
    if m.search_ticket != ticket {
        return Ok("stale memory/search reply dropped".into());
    }
    m.searching = false;
    let out = res.and_then(|v| {
        attributed(&v, &profile)?;
        parse_hits(&v).ok_or(Refusal::Failed)
    });
    super::host::wake();
    match out {
        Ok(hits) => {
            let n = hits.len();
            m.hits = Some(hits);
            m.search_error = None;
            Ok(format!("memory search {query:?}: {n} hits"))
        }
        Err(refusal) => {
            m.hits = None;
            let why = format!("memory search refused: {refusal:?}");
            m.search_error = Some(Failure { op: Op::Search, refusal });
            Err(why)
        }
    }
}

pub async fn open_record(conv: &crate::flow::Conversation, ticket: u64, id: String) -> Result<String, String> {
    let (profile, confirmed) = begin(conv);
    let res = call(conv, LOAD, load_params(&profile, &id), confirmed).await;
    let mut st = super::host::state();
    let m = &mut st.mem;
    if m.record_ticket != ticket {
        return Ok("stale memory/load reply dropped".into());
    }
    m.record_loading = false;
    let out = res.and_then(|v| {
        attributed(&v, &profile)?;
        parse_load(&v).ok_or(Refusal::Failed)
    });
    super::host::wake();
    match out {
        Ok(l) => {
            m.record = Some(l);
            m.record_error = None;
            Ok(format!("memory record {id} opened"))
        }
        Err(refusal) => {
            let why = format!("memory/load refused: {refusal:?}");
            m.record_error = Some(Failure { op: Op::Open, refusal });
            Err(why)
        }
    }
}

pub async fn open_entity(conv: &crate::flow::Conversation, ticket: u64, name: String) -> Result<String, String> {
    let (profile, confirmed) = begin(conv);
    let res = call(conv, ENTITY, entity_params(&profile, &name), confirmed).await;
    let mut st = super::host::state();
    let m = &mut st.mem;
    if m.entity_ticket != ticket {
        return Ok("stale memory/entity reply dropped".into());
    }
    m.entity_loading = false;
    let out = res.and_then(|v| {
        attributed(&v, &profile)?;
        parse_entity(&v).ok_or(Refusal::Failed)
    });
    super::host::wake();
    match out {
        Ok(p) => {
            m.entity = Some(p);
            m.entity_error = None;
            Ok(format!("memory entity {name} opened"))
        }
        Err(refusal) => {
            let why = format!("memory/entity refused: {refusal:?}");
            m.entity_error = Some(Failure { op: Op::Page, refusal });
            Err(why)
        }
    }
}

pub async fn add_note(conv: &crate::flow::Conversation, ticket: u64) -> Result<String, String> {
    let (profile, confirmed) = begin(conv);
    let (title, note) = {
        let st = super::host::state();
        (st.mem.add_title.clone(), st.mem.add_note.clone())
    };
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let res = match note_record(&title, &note, &now, &note_key()) {
        Ok(record) if confirmed => call(conv, INGEST, ingest_params(&profile, record), confirmed).await,
        // Never a write into memory the server has not attributed.
        Ok(_) => Err(Refusal::Scope),
        Err(r) => Err(r),
    };
    let landed = {
        let mut st = super::host::state();
        let m = &mut st.mem;
        if m.add_ticket != ticket {
            return Ok("stale memory/ingest reply dropped".into());
        }
        m.adding = false;
        let out = res.and_then(|v| {
            attributed(&v, &profile)?;
            parse_ingest(&v).ok_or(Refusal::Failed)
        });
        match out {
            Ok(report) => {
                m.receipt = Some(receipt(&report));
                m.add_title.clear();
                m.add_note.clear();
                m.add_title_snap.clear();
                m.add_note_snap.clear();
                m.add_error = None;
                m.page = Page::Overview;
                m.ticket += 1;
                m.loading = true;
                Ok((m.ticket, report))
            }
            Err(refusal) => {
                // The draft stays (and is what the remounted form shows).
                m.add_title_snap = m.add_title.clone();
                m.add_note_snap = m.add_note.clone();
                let why = format!("memory/ingest refused: {refusal:?}");
                m.add_error = Some(Failure { op: Op::Add, refusal });
                Err(why)
            }
        }
    };
    super::host::wake();
    let (overview_ticket, report) = landed?;
    // What the note changed (the staging count, a later search) is read back.
    let _ = load_overview(conv, overview_ticket).await;
    Ok(format!("memory note added: {report:?}"))
}

// ------------------------------------------------------------------ theme

/// D4 — Memory follows the app theme (frame 11: `#1C1F22` surface,
/// `#2C2C2E` cards, `#38383A` hairlines, `#F5F5F7` text). The kit is drawn in
/// its light tokens; a dark look tints the line icons, gives the accents the
/// TOKENS table leaves alone (amber, the error box's border, the fields'
/// borders) their dark twins, then retints like every themed surface
/// (`theme::retint_dsl`). Light is the byte passthrough.
pub fn follow_theme(dsl: &str) -> String {
    use crate::screens::theme::{current_look, retint_dsl, Look};
    if current_look() == Look::Light {
        return dsl.to_owned();
    }
    let mut out = ui::themed_icons(dsl);
    for (light, dark) in DARK_ACCENTS {
        out = out.replace(light, dark);
    }
    retint_dsl(&out)
}

/// The dark twins of the kit accents TOKENS leaves light (checked >= 4.5:1
/// for text, 3:1 for a glyph or a border, against `#1C1F22` / the box fill).
const DARK_ACCENTS: &[(&str, &str)] = &[
    // the amber callout / chip: fill, border, ink
    (tok::AMBER_BG, "#3a2b17ff"),
    (tok::AMBER_LINE, "#7a5a27ff"),
    (tok::AMBER, "#f2b65eff"),
    // the refusal box's border
    ("#f5c2c7ff", "#6b2a30ff"),
    // the fields' borders
    ("#d1d1d6ff", "#4a4a4dff"),
    ("#d9d9dcff", "#4a4a4dff"),
    // a raised card on the dark dialog (frame 11's #2C2C2E)
    (tok::SURFACE2, "#2c2c2eff"),
];

// ------------------------------------------------------------------ dates

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const WEEKDAYS_ZH: [&str; 7] = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// "Fri 3 Oct" / "10月3日 周五" (board 5's day label).
fn day_label(date: chrono::NaiveDate) -> String {
    use chrono::Datelike;
    let wd = date.weekday().num_days_from_monday() as usize;
    if crate::i18n::is_zh() {
        format!("{}月{}日 {}", date.month(), date.day(), WEEKDAYS_ZH[wd])
    } else {
        format!("{} {} {}", WEEKDAYS[wd], date.day(), MONTHS[date.month0() as usize])
    }
}

/// "2 Oct" / "10月2日".
fn short_day(ms: u64) -> String {
    use chrono::Datelike;
    let Some(d) = chrono::DateTime::from_timestamp_millis(ms as i64) else { return String::new() };
    if crate::i18n::is_zh() {
        format!("{}月{}日", d.month(), d.day())
    } else {
        format!("{} {}", d.day(), MONTHS[d.month0() as usize])
    }
}

/// "30 Sep 2026" / "2026年9月30日".
fn full_day(ms: u64) -> String {
    use chrono::Datelike;
    let Some(d) = chrono::DateTime::from_timestamp_millis(ms as i64) else { return String::new() };
    if crate::i18n::is_zh() {
        format!("{}年{}月{}日", d.year(), d.month(), d.day())
    } else {
        format!("{} {} {}", d.day(), MONTHS[d.month0() as usize], d.year())
    }
}

/// "96 KB" — the truncation notice's sizes.
fn size_label(bytes: u64) -> String {
    if bytes >= 1024 {
        format!("{} KB", (bytes as f64 / 1024.0).round() as u64)
    } else {
        format!("{bytes} B")
    }
}

// ------------------------------------------------------------------ view

/// The geometry: desktop `min(640, 100%) x min(720, 100%)` in the backdrop's
/// 16 px inset — ONE size for every page, so moving between the overview,
/// the results and a record never resizes or re-centres the dialog (board 5
/// frame 2 draws it at the window's height); below 520 px the phone's
/// full-screen sheet (frames 4-12).
fn geometry(frame: &Frame) -> (bool, f64, f64) {
    if frame.avail_w < 520.0 {
        (true, frame.avail_w.floor(), frame.avail_h.floor())
    } else {
        (false, frame.dialog_w(640.0), frame.dialog_max_h().min(720.0))
    }
}

fn shell_open(d: &mut Dsl, sheet: bool, w: f64, h: f64, pad: f64) {
    let ids = &ui::B3_IDS;
    d.open(ids.root, "KeyboardView", "width: Fill height: Fill flow: Overlay align: Align{x: 0.5 y: 0.5} keyboard_min_shift: 56.");
    d.rule(ids.backdrop, "width: Fill height: Fill", tok::MASK);
    d.view(ids.backdrop_box, "width: Fill height: Fill flow: Overlay");
    d.tap(ids.backdrop_hit, ids.backdrop_event.unwrap_or("b3.noop"));
    d.close();
    if sheet {
        d.surface(
            ids.dialog,
            &format!("width: {w} height: {h} flow: Down padding: Inset{{left: {pad} right: {pad} top: 14 bottom: 0}}"),
            tok::SURFACE,
            0.0,
            None,
        );
    } else {
        d.surface(
            ids.dialog,
            &format!("width: {w} height: {h} flow: Down padding: Inset{{left: {pad} right: {pad} top: 18 bottom: 8}}"),
            tok::SURFACE,
            16.0,
            Some(tok::HAIRLINE),
        );
    }
}

/// The body: the scroll region under the fixed chrome, filling the rest of
/// the dialog (the scroll bar's 10 px gutter on the right).
fn body_open(d: &mut Dsl, sheet: bool) {
    let bottom = if sheet { 16 } else { 12 };
    d.open(
        ui::B3_IDS.scroll,
        "ScrollYView",
        &format!("width: Fill height: Fill flow: Down padding: Inset{{left: 0 top: 0 right: 10 bottom: {bottom}}}"),
    );
}

/// An icon with an explicit ink (the theme pass keeps it: `themed_icons`
/// tints only an icon without one).
fn icon_ink(d: &mut Dsl, id: &str, file: &str, size: f64, ink: &str) {
    let path = crate::design::icon_resource(file);
    d.open(
        id,
        "Svg",
        &format!(
            "width: {size} height: {size} animating: false draw_svg.svg: file_resource({path:?}) draw_svg.preserve_viewbox: true draw_svg.color: {ink}"
        ),
    );
    d.close();
}

/// Header of the overview / results: title, [Add note], refresh, close.
fn header_main(d: &mut Dsl, st: &MemState, store: &Store) {
    d.view("b3_mem_head", "width: Fill height: 32 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 6");
    d.text("b3_mem_title", tr(TITLE), &ui::title().w(W::Fill));
    if st.page == Page::Overview && can_add(st, store) && st.error.is_none() {
        d.button("b3_mem_add", tr(ADD_NOTE), "b3.mem.add", Btn::Outline, W::Fit, 30.0);
    }
    ui::icon_button(d, "b3_mem_refresh", "b3_refresh.svg", 16.0, "b3.mem.refresh");
    ui::close_glyph(d, "b3.close");
    d.close();
    d.gap(W::Fill, 2.0);
    // D1: plain text in the theme's muted ink (not monospace).
    let profile = if st.profile.is_empty() { "…".to_owned() } else { st.profile.clone() };
    d.text("b3_mem_scope", &scope_line(&profile), &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill));
}

/// Header of a sub-page: "‹ Back" (blue) and close.
fn header_back(d: &mut Dsl, label: &str) {
    d.view("b3_mem_head", "width: Fill height: 32 flow: Right align: Align{x: 0.0 y: 0.5}");
    let text = tr(label);
    let w = ui::text_w(text, 14.0, Face::Regular) + 22.0;
    d.view("b3_mem_back_box", &format!("width: {} height: 32 flow: Overlay align: Align{{x: 0.0 y: 0.5}}", w.ceil()));
    d.view("b3_mem_back_row", "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} spacing: 4");
    icon_ink(d, "b3_mem_back_icon", "b1_chevron_left.svg", 14.0, tok::BLUE_TEXT);
    d.text("b3_mem_back_label", text, &Txt::new(14.0, Face::Regular, tok::BLUE_TEXT));
    d.close();
    d.tap("b3_mem_back", "b3.mem.back");
    d.close();
    d.gap(W::Fill, 1.0);
    ui::close_glyph(d, "b3.close");
    d.close();
}

/// The search field: magnifier, the query, a clear control.
fn search_field(d: &mut Dsl, st: &MemState, sheet: bool) {
    let h = if sheet { 40.0 } else { 36.0 };
    d.surface(
        "b3_mem_query_field",
        &format!("width: Fill height: {h} flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 8 padding: Inset{{left: 12 right: 4 top: 0 bottom: 0}}"),
        tok::SURFACE,
        10.0,
        Some("#d1d1d6ff"),
    );
    d.icon("b3_mem_query_icon", "b3_search.svg", 16.0, tok::FAINT);
    d.inputs.push(("b3_mem_query".to_owned(), "mem.query".to_owned()));
    d.open("b3_mem_query", "TextInput", &ui::input_props(&st.query_snap, SEARCH_HINT, false, false));
    d.close();
    d.view(
        "b3_mem_clear_box",
        &format!(
            "width: 28 height: 28 flow: Overlay align: Align{{x: 0.5 y: 0.5}} visible: {}",
            !st.query_snap.is_empty()
        ),
    );
    d.icon("b3_mem_clear_icon", "b3_close.svg", 11.0, tok::MUTED);
    d.tap("b3_mem_clear", "b3.mem.clear");
    d.close();
    d.close();
}

/// A section's heading row: the title, a muted note on the right.
fn section_head(d: &mut Dsl, id: &str, title: &str, right: Option<&str>) {
    d.view(&format!("{id}_head"), "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
    d.text(&format!("{id}_title"), title, &Txt::new(13.5, Face::Semibold, tok::TEXT).w(W::Fill));
    if let Some(r) = right {
        d.text(&format!("{id}_note"), r, &Txt::new(12.0, Face::Regular, tok::MUTED));
    }
    d.close();
}

/// Markdown (the transcript's renderer) for the profile's OWN memory files
/// and trusted bank pages: links and images reduced to their text (memory
/// is read here, never navigated from). `chips`: inline code on a raised
/// chip (a page, frame 7); without, plain monospace (the overview's card,
/// frame 2: "only the project names are mono").
fn markdown(d: &mut Dsl, id: &str, body: &str, px: f64, heading_scale: f64, para: f64, chips: bool) {
    let text = crate::markdown::sanitize(&link_text(body), true);
    let (code_bg, code_pad) = if chips { (tok::CHIP, 3) } else { (tok::TRANSPARENT, 0) };
    let line = (px * 1.55).round();
    let style = |face| {
        format!(
            "TextStyle{{font_family: {} font_size: {} line_spacing: {:.4}}}",
            crate::fluid::family(face),
            px * 0.75,
            line / (1.1303 * px)
        )
    };
    let regular = style(crate::fluid::Face::Regular);
    let bold = style(crate::fluid::Face::SemiBold);
    let mono = style(crate::fluid::Face::Mono);
    d.raw(&format!(
        "{id} := Markdown{{width: Fill height: Fit padding: 0 margin: 0\n\
         body: {text:?}\n\
         font_size: {fs}\n\
         font_color: {ink}\n\
         paragraph_spacing: {para}\n\
         pre_code_spacing: 8\n\
         heading_base_scale: {heading_scale}\n\
         fixed_font_size_scale: 0.9\n\
         inline_code_padding: Inset{{left: {code_pad} right: {code_pad} top: 1 bottom: 1}}\n\
         inline_code_margin: Inset{{left: 0 right: 0 top: 0 bottom: 0}}\n\
         text_style_normal: {regular}\n\
         text_style_italic: {regular}\n\
         text_style_bold: {bold}\n\
         text_style_bold_italic: {bold}\n\
         text_style_fixed: {mono}\n\
         code_layout: Layout{{flow: Right{{wrap: true}} padding: Inset{{left: 10 right: 10 top: 8 bottom: 8}}}}\n\
         draw_block +: {{code_color: {chip} line_color: {ink} sep_color: {hair} quote_bg_color: {hair} quote_fg_color: {muted} table_header_bg_color: #00000000 table_border_color: {hair}}}\n\
         }}",
        fs = px * 0.75,
        ink = tok::TEXT,
        chip = code_bg,
        hair = tok::HAIRLINE,
        muted = tok::MUTED,
    ));
}

/// `[text](url)` -> `text`, `<http://…>` -> its address as text.
fn link_text(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut rest = md;
    while let Some(open) = rest.find('[') {
        let (head, tail) = rest.split_at(open);
        out.push_str(head);
        // `![alt](src)` is an image: `sanitize` keeps its alt text.
        if let Some(close) = tail.find("](") {
            if let Some(end) = tail[close + 2..].find(')') {
                if !tail[1..close].contains('\n') {
                    out.push_str(&tail[1..close]);
                    rest = &tail[close + 2 + end + 1..];
                    continue;
                }
            }
        }
        out.push('[');
        rest = &tail[1..];
    }
    out.push_str(rest);
    out.replace("<http", "http").replace(">", "")
}

/// The overview card's preview of `MEMORY.md`: its first two sections or
/// eight lines, whichever comes first.
fn preview(md: &str) -> (String, bool) {
    let mut out = Vec::new();
    let (mut headings, mut lines) = (0, 0);
    let all: Vec<&str> = md.lines().collect();
    for (i, l) in all.iter().enumerate() {
        if l.trim_start().starts_with('#') {
            headings += 1;
            if headings > 2 {
                return (out.join("\n").trim_end().to_owned(), true);
            }
        }
        if !l.trim().is_empty() {
            lines += 1;
            if lines > 8 {
                return (out.join("\n").trim_end().to_owned(), true);
            }
        }
        out.push(*l);
        if i + 1 == all.len() {
            break;
        }
    }
    (out.join("\n").trim_end().to_owned(), false)
}

/// A refusal (D1): the red lead, the problem, the next step — bounded copy.
fn refusal_box(d: &mut Dsl, id: &str, f: &Failure, profile: &str) {
    let (lead, problem, next) = copy(f, profile);
    d.surface(
        id,
        "width: Fill height: Fit flow: Down spacing: 4 padding: Inset{left: 14 right: 14 top: 12 bottom: 12}",
        tok::RED_BG,
        10.0,
        Some("#f5c2c7ff"),
    );
    d.text(&format!("{id}_lead"), tr(lead), &Txt::new(13.5, Face::Medium, tok::RED_TEXT).w(W::Fill).wrap());
    d.text(&format!("{id}_detail"), &problem, &Txt::new(12.5, Face::Regular, tok::TEXT).w(W::Fill).wrap());
    d.text(&format!("{id}_next"), &next, &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    if matches!(f.refusal, Refusal::Failed | Refusal::Offline | Refusal::NotFound) {
        d.gap(W::Fill, 2.0);
        d.link(&format!("{id}_retry"), tr(TRY_AGAIN), Some("b3.mem.retry"), 13.0);
    }
    d.close();
}

/// A centred muted line (loading, searching).
fn centred_note(d: &mut Dsl, id: &str, text: &str, spinner: bool) {
    d.view(id, "width: Fill height: Fit flow: Right align: Align{x: 0.5 y: 0.5} spacing: 8 padding: Inset{top: 18 bottom: 18}");
    if spinner {
        d.icon(&format!("{id}_icon"), "b1_spinner.svg", 16.0, tok::MUTED);
    }
    d.text(&format!("{id}_text"), text, &Txt::new(13.0, Face::Regular, tok::MUTED));
    d.close();
}

/// The amber truncation notice (frame 8): never a cut page shown as whole.
fn cut_notice(d: &mut Dsl, id: &str, shown: u64, total: u64) {
    d.surface(
        id,
        "width: Fill height: Fit flow: Right spacing: 10 padding: Inset{left: 12 right: 12 top: 10 bottom: 10}",
        tok::AMBER_BG,
        10.0,
        Some(tok::AMBER_LINE),
    );
    icon_ink(d, &format!("{id}_icon"), "b3_warning.svg", 16.0, tok::AMBER);
    let text = tr_with(TRUNCATED, &[("value0", &size_label(shown)), ("value1", &size_label(total))]);
    d.text(&format!("{id}_text"), &text, &Txt::new(12.5, Face::Regular, tok::TEXT).w(W::Fill).wrap());
    d.close();
}

/// A list card of tappable rows (entities, recent notes).
fn list_open(d: &mut Dsl, id: &str) {
    d.surface(id, "width: Fill height: Fit flow: Down padding: 0", tok::SURFACE, 10.0, Some(tok::HAIRLINE));
}

/// One tappable list row: a lead (mono name or a date), its text, a chevron.
#[allow(clippy::too_many_arguments)]
fn list_row(d: &mut Dsl, id: &str, event: &str, lead: &str, lead_mono: bool, text: &str, compact: bool, inner_w: f64, first: bool) {
    if !first {
        d.rule(&format!("{id}_rule"), "width: Fill height: 1", tok::HAIRLINE);
    }
    d.view(&format!("{id}_row"), "width: Fill height: Fit flow: Overlay");
    let lead_face = if lead_mono { Face::Mono } else { Face::Medium };
    if compact {
        d.view(
            &format!("{id}_cells"),
            "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{left: 14 right: 10 top: 10 bottom: 10}",
        );
        d.view(&format!("{id}_col"), "width: Fill height: Fit flow: Down spacing: 3");
        d.text(&format!("{id}_lead"), &ui::fit_w(lead, inner_w - 60.0, 13.0, lead_face), &Txt::new(13.0, lead_face, tok::TEXT).w(W::Fill));
        if !text.is_empty() {
            d.text(&format!("{id}_text"), text, &Txt::new(12.5, Face::Regular, tok::TEXT).w(W::Fill).wrap());
        }
        d.close();
    } else {
        d.view(
            &format!("{id}_cells"),
            "width: Fill height: 44 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 12 padding: Inset{left: 14 right: 10 top: 0 bottom: 0}",
        );
        let lead_w = 150.0;
        d.text(&format!("{id}_lead"), &ui::fit_w(lead, lead_w, 12.5, lead_face), &Txt::new(12.5, lead_face, tok::TEXT).w(W::Px(lead_w)));
        let budget = inner_w - lead_w - 14.0 - 10.0 - 24.0 - 24.0;
        d.text(&format!("{id}_text"), &ui::fit_w(text, budget, 12.5, Face::Regular), &Txt::new(12.5, Face::Regular, tok::TEXT).w(W::Fill));
    }
    d.icon(&format!("{id}_chev"), "b3_chevron_right.svg", 14.0, tok::MUTED);
    d.close();
    d.tap(id, event);
    d.close();
}

fn kind_chip(d: &mut Dsl, id: &str, kind: Kind) {
    d.chip(id, tr(kind.chip()), tok::TEXT, tok::CHIP, None, false);
}

fn untrusted_chip(d: &mut Dsl, id: &str) {
    d.chip(id, tr(UNTRUSTED), tok::AMBER, tok::AMBER_BG, Some(tok::AMBER_LINE), false);
}

/// Up to three lines of `s` at `px` in `width` (a label wraps at words, so
/// the budget keeps a margin).
fn abstract_lines(s: &str, width: f64, px: f64) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    ui::fit_w(&flat, width * 2.6, px, Face::Regular)
}

pub fn build(d: &mut Dsl, st: &MemState, frame: &Frame, store: &Store) {
    let (sheet, width, height) = geometry(frame);
    let pad = if sheet { 16.0 } else { 20.0 };
    // The content width inside the body (the scroll bar's 10 px gutter).
    let inner_w = width - 2.0 * pad - 10.0;
    shell_open(d, sheet, width, height, pad);
    match st.page {
        Page::Overview | Page::Results => {
            // The fixed chrome keeps the body's right edge (the scroll bar's
            // gutter), so the close glyph, the field and the cards line up.
            chrome_open(d);
            header_main(d, st, store);
            if st.page == Page::Overview && !sheet {
                d.gap(W::Fill, 8.0);
                d.text("b3_mem_intro", tr(INTRO), &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
            }
            d.gap(W::Fill, 12.0);
            search_field(d, st, sheet);
            if st.page == Page::Results {
                d.gap(W::Fill, 10.0);
                let options: Vec<(&str, String)> = KindFilter::ALL
                    .iter()
                    .map(|k| (k.label(), format!("b3.mem.kind.{}", k.id())))
                    .collect();
                let selected = KindFilter::ALL.iter().position(|k| *k == st.kind).unwrap_or(0);
                d.segmented("b3_mem_kind", &options, selected, W::Fill, Seg::Tab);
            }
            d.gap(W::Fill, 14.0);
            d.close();
            body_open(d, sheet);
            if st.page == Page::Overview {
                overview_body(d, st, sheet, inner_w);
            } else {
                results_body(d, st, sheet, inner_w);
            }
            d.close();
        }
        page => {
            // A sub-page: "‹ Results" / "‹ Memory" and close over its body.
            let back = if page == Page::Record && (st.hits.is_some() || st.searched.is_some()) { BACK_RESULTS } else { BACK_MEMORY };
            chrome_open(d);
            header_back(d, back);
            d.gap(W::Fill, 8.0);
            d.close();
            body_open(d, sheet);
            match page {
                Page::Record => record_body(d, st, inner_w),
                Page::Entity => entity_body(d, st),
                Page::LongTerm => long_term_body(d, st),
                Page::Day(which) => day_body(d, st, which),
                _ => add_body(d, st, sheet),
            }
            d.close();
        }
    }
    ui::shell_close(d);
}

/// The fixed chrome over the body, inset like the body's content (the
/// scroll bar's 10 px gutter on the right).
fn chrome_open(d: &mut Dsl) {
    d.view("b3_mem_chrome", "width: Fill height: Fit flow: Down padding: Inset{left: 0 top: 0 right: 10 bottom: 0}");
}

fn overview_body(d: &mut Dsl, st: &MemState, sheet: bool, inner_w: f64) {
    if let Some(f) = &st.error {
        refusal_box(d, "b3_mem_error", f, &st.profile);
        return;
    }
    let Some(o) = &st.overview else {
        centred_note(d, "b3_mem_loading", tr(LOADING), true);
        return;
    };
    if let Some(r) = st.receipt {
        d.text("b3_mem_receipt", tr(r), &Txt::new(12.5, Face::Medium, tok::GREEN_TEXT).w(W::Fill));
        d.gap(W::Fill, 10.0);
    }
    if o.is_empty() {
        d.view("b3_mem_empty", "width: Fill height: Fit flow: Down align: Align{x: 0.5 y: 0.0} spacing: 6 padding: Inset{top: 16 bottom: 16 left: 12 right: 12}");
        d.text("b3_mem_empty_title", tr(EMPTY_TITLE), &Txt::new(15.0, Face::Semibold, tok::TEXT));
        d.text(
            "b3_mem_empty_body",
            tr(EMPTY_BODY),
            &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Px((inner_w - 24.0).min(300.0))).wrap(),
        );
        d.close();
        staging_line(d, o);
        return;
    }
    let now = ui::now_ms();
    if !o.long_term.trim().is_empty() {
        let updated = o
            .long_term_updated_at
            .as_deref()
            .and_then(ui::parse_iso_ms)
            .map(|t| tr1(UPDATED, &ui::rel_ago(now, t)));
        section_head(d, "b3_mem_lt", tr(LONG_TERM), updated.as_deref());
        d.gap(W::Fill, 8.0);
        d.surface(
            "b3_mem_lt",
            "width: Fill height: Fit flow: Down spacing: 6 padding: Inset{left: 14 right: 14 top: 12 bottom: 10}",
            tok::SURFACE2,
            10.0,
            Some(tok::HAIRLINE),
        );
        let (text, _) = preview(&o.long_term);
        markdown(d, "b3_mem_lt_md", &text, 13.0, 1.4, 8.0, false);
        d.link("b3_mem_lt_more", tr(SHOW_ALL), Some("b3.mem.lt"), 13.0);
        d.close();
        d.gap(W::Fill, 16.0);
    }
    if !o.today.trim().is_empty() {
        let today = day_label(chrono::Local::now().date_naive());
        section_head(d, "b3_mem_today", tr(TODAY), Some(&today));
        d.gap(W::Fill, 6.0);
        d.view("b3_mem_today", "width: Fill height: Fit flow: Down spacing: 4");
        let (text, cut) = preview(&o.today);
        markdown(d, "b3_mem_today_md", &text, 13.0, 1.4, 6.0, false);
        if cut || o.today_truncated {
            d.link("b3_mem_today_more", tr(SHOW_ALL), Some("b3.mem.today"), 13.0);
        }
        d.close();
        d.gap(W::Fill, 16.0);
    }
    if !o.recent.is_empty() {
        section_head(d, "b3_mem_recent", tr(RECENT), Some(&o.recent.len().to_string()));
        d.gap(W::Fill, 8.0);
        list_open(d, "b3_mem_recent");
        for (i, n) in o.recent.iter().enumerate() {
            let date = chrono::NaiveDate::parse_from_str(&n.date, "%Y-%m-%d").map(day_label).unwrap_or_else(|_| n.date.clone());
            let first = n.content.lines().map(|l| l.trim().trim_start_matches(['-', '*', '#', ' '])).find(|l| !l.is_empty()).unwrap_or("");
            list_row(d, &format!("b3_mem_day_{i}"), &format!("b3.mem.day#{i}"), &date, false, first, sheet, inner_w, i == 0);
        }
        d.close();
        d.gap(W::Fill, 16.0);
    }
    if !o.entities.is_empty() {
        let count = if o.entities_truncated { format!("{}+", o.entities.len()) } else { o.entities.len().to_string() };
        section_head(d, "b3_mem_entities", tr(ENTITIES), Some(&count));
        d.gap(W::Fill, 8.0);
        list_open(d, "b3_mem_entities");
        for (i, e) in o.entities.iter().enumerate() {
            list_row(d, &format!("b3_mem_entity_{i}"), &format!("b3.mem.entity#{i}"), &e.name, true, &e.summary, sheet, inner_w, i == 0);
        }
        d.close();
        d.gap(W::Fill, 14.0);
    }
    staging_line(d, o);
}

fn staging_line(d: &mut Dsl, o: &Overview) {
    let text = if !o.refresh_enabled {
        tr(REFRESH_OFF).to_owned()
    } else if o.staging_notes == 0 {
        return;
    } else if o.staging_notes == 1 && !o.staging_truncated {
        tr(STAGING_ONE).to_owned()
    } else {
        let n = if o.staging_truncated { format!("{}+", o.staging_notes) } else { o.staging_notes.to_string() };
        tr1(STAGING_N, &n)
    };
    d.view("b3_mem_staging", "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{top: 2 bottom: 4}");
    d.icon("b3_mem_staging_icon", "b3_clock.svg", 15.0, tok::MUTED);
    d.text("b3_mem_staging_text", &text, &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    d.close();
}

fn results_body(d: &mut Dsl, st: &MemState, sheet: bool, inner_w: f64) {
    if st.searching {
        centred_note(d, "b3_mem_searching", tr(SEARCHING), true);
        return;
    }
    if let Some(f) = &st.search_error {
        refusal_box(d, "b3_mem_error", f, &st.profile);
        return;
    }
    let Some(hits) = &st.hits else { return };
    let count = match hits.len() {
        0 => tr(NO_RESULTS).to_owned(),
        1 => tr(RESULTS_ONE).to_owned(),
        n => tr1(RESULTS_N, &n.to_string()),
    };
    d.text("b3_mem_count", &count, &Txt::new(12.5, Face::Regular, tok::MUTED));
    d.gap(W::Fill, 6.0);
    for (i, h) in hits.iter().enumerate() {
        let id = format!("b3_mem_hit_{i}");
        d.rule(&format!("{id}_rule"), "width: Fill height: 1", tok::HAIRLINE);
        d.view(&format!("{id}_row"), "width: Fill height: Fit flow: Overlay");
        d.view(&format!("{id}_cells"), "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10 padding: Inset{top: 12 bottom: 12}");
        d.view(&format!("{id}_col"), "width: Fill height: Fit flow: Down spacing: 5");
        let text_w = inner_w - 34.0;
        d.text(&format!("{id}_title"), &ui::fit_w(&h.title, text_w, 14.0, Face::Semibold), &Txt::new(14.0, Face::Semibold, tok::TEXT).w(W::Fill));
        if !h.abstract_.is_empty() {
            d.text(&format!("{id}_abstract"), &abstract_lines(&h.abstract_, text_w, 13.0), &Txt::new(13.0, Face::Regular, tok::TEXT).w(W::Fill).wrap());
        }
        d.view(&format!("{id}_meta"), "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 6");
        kind_chip(d, &format!("{id}_kind"), h.kind);
        let date = ui::parse_iso_ms(&h.timestamp).map(short_day).unwrap_or_default();
        let meta = if date.is_empty() { h.source.clone() } else { format!("{} · {date}", h.source) };
        d.text(&format!("{id}_source"), &meta, &Txt::new(12.0, Face::Regular, tok::MUTED));
        if h.trust == Trust::Untrusted && !sheet {
            untrusted_chip(d, &format!("{id}_trust"));
        }
        d.close();
        if h.trust == Trust::Untrusted && sheet {
            d.view(&format!("{id}_trust_row"), "width: Fill height: Fit flow: Right");
            untrusted_chip(d, &format!("{id}_trust"));
            d.close();
        }
        d.close();
        d.icon(&format!("{id}_chev"), "b3_chevron_right.svg", 14.0, tok::MUTED);
        d.close();
        d.tap(&id, &format!("b3.mem.open#{i}"));
        d.close();
    }
}

fn record_body(d: &mut Dsl, st: &MemState, inner_w: f64) {
    if st.record_loading {
        centred_note(d, "b3_mem_opening", tr(OPENING), true);
        return;
    }
    if let Some(f) = &st.record_error {
        refusal_box(d, "b3_mem_error", f, &st.profile);
        return;
    }
    let Some(l) = &st.record else { return };
    let r = &l.record;
    d.text("b3_mem_rec_title", &r.title, &Txt::new(20.0, Face::Semibold, tok::TEXT).w(W::Fill).wrap());
    d.gap(W::Fill, 10.0);
    d.view("b3_mem_rec_chips", "width: Fill height: Fit flow: Right{wrap: true} spacing: 6");
    kind_chip(d, "b3_mem_rec_kind", r.kind);
    if !r.source.is_empty() {
        d.chip("b3_mem_rec_source", &r.source, tok::TEXT, tok::CHIP, None, false);
    }
    if r.trust == Trust::Untrusted {
        untrusted_chip(d, "b3_mem_rec_trust_chip");
    }
    d.close();
    d.gap(W::Fill, 10.0);
    let mut meta = ui::parse_iso_ms(&r.timestamp).map(full_day).unwrap_or_default();
    let opened = match r.visits {
        0 => String::new(),
        1 => tr(OPENED_ONCE).to_owned(),
        n => tr1(OPENED_N, &n.to_string()),
    };
    if !opened.is_empty() {
        if !meta.is_empty() {
            meta.push_str(" · ");
        }
        meta.push_str(&opened);
    }
    d.text("b3_mem_rec_meta", &meta, &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill));
    d.gap(W::Fill, 14.0);
    match (&l.page, r.trust) {
        // A trusted bank page: rendered like the entity page (frame 7).
        (Some(page), Trust::Trusted) => {
            markdown(d, "b3_mem_rec_page", page, 14.0, 1.45, 12.0, true);
            if l.page_truncated {
                d.gap(W::Fill, 12.0);
                cut_notice(d, "b3_mem_rec_cut", page.len() as u64, page.len() as u64);
            }
        }
        // Everything else is shown as data: plain text, never Markdown.
        _ => {
            let body = r.body.clone().unwrap_or_else(|| r.abstract_.clone());
            d.text("b3_mem_rec_body", &body, &Txt::new(14.0, Face::Regular, tok::TEXT).w(W::Fill).wrap());
        }
    }
    if r.trust == Trust::Untrusted {
        d.gap(W::Fill, 18.0);
        let lead = if r.kind == Kind::Episode { FROM_SESSION } else { FROM_APP };
        d.surface(
            "b3_mem_rec_trust",
            "width: Fill height: Fit flow: Right spacing: 10 padding: Inset{left: 14 right: 14 top: 12 bottom: 12}",
            tok::AMBER_BG,
            10.0,
            Some(tok::AMBER_LINE),
        );
        icon_ink(d, "b3_mem_rec_trust_icon", "b3_warning.svg", 18.0, tok::AMBER);
        d.view("b3_mem_rec_trust_col", "width: Fill height: Fit flow: Down spacing: 4");
        d.text("b3_mem_rec_trust_lead", tr(lead), &Txt::new(13.5, Face::Medium, tok::AMBER).w(W::Fill).wrap());
        d.text("b3_mem_rec_trust_body", tr(AS_DATA), &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
        d.close();
        d.close();
    }
    d.gap(W::Fill, 18.0);
    d.text("b3_mem_rec_id", &ui::fit_middle(&r.id, inner_w, 11.5, Face::Mono), &Txt::new(11.5, Face::Mono, tok::MUTED).w(W::Fill));
}

fn page_title(d: &mut Dsl, id: &str, title: &str, sub: &str, sub_mono: Option<&str>) {
    d.text(&format!("{id}_title"), title, &Txt::new(20.0, Face::Semibold, tok::TEXT).w(W::Fill).wrap());
    d.gap(W::Fill, 4.0);
    d.view(&format!("{id}_sub"), "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 6");
    if let Some(m) = sub_mono {
        d.text(&format!("{id}_file"), m, &Txt::new(12.0, Face::Mono, tok::MUTED));
    }
    if !sub.is_empty() {
        d.text(&format!("{id}_note"), sub, &Txt::new(12.5, Face::Regular, tok::MUTED));
    }
    d.close();
    d.gap(W::Fill, 12.0);
    d.rule(&format!("{id}_rule"), "width: Fill height: 1", tok::HAIRLINE);
    d.gap(W::Fill, 12.0);
}

fn entity_body(d: &mut Dsl, st: &MemState) {
    page_title(d, "b3_mem_ent", &st.entity_name, tr(ENTITY_PAGE), None);
    if st.entity_loading {
        centred_note(d, "b3_mem_opening", tr(OPENING), true);
        return;
    }
    if let Some(f) = &st.entity_error {
        refusal_box(d, "b3_mem_error", f, &st.profile);
        return;
    }
    let Some(p) = &st.entity else { return };
    markdown(d, "b3_mem_ent_md", &p.content, 14.0, 1.45, 12.0, true);
    if p.truncated {
        d.gap(W::Fill, 14.0);
        cut_notice(d, "b3_mem_ent_cut", p.content.len() as u64, p.total_bytes);
    }
}

fn long_term_body(d: &mut Dsl, st: &MemState) {
    let Some(o) = &st.overview else { return };
    let now = ui::now_ms();
    let updated = o
        .long_term_updated_at
        .as_deref()
        .and_then(ui::parse_iso_ms)
        .map(|t| format!("· {}", tr1(UPDATED_LOWER, &ui::rel_ago(now, t))))
        .unwrap_or_default();
    page_title(d, "b3_mem_lt_page", tr(LONG_TERM), &updated, Some("MEMORY.md"));
    markdown(d, "b3_mem_lt_full", &o.long_term, 14.0, 1.45, 12.0, true);
    if o.long_term_truncated {
        d.gap(W::Fill, 14.0);
        cut_notice(d, "b3_mem_lt_cut", o.long_term.len() as u64, o.long_term_total_bytes);
    }
}

fn day_body(d: &mut Dsl, st: &MemState, which: Option<usize>) {
    let Some(o) = &st.overview else { return };
    let (title, note, cut, total) = match which {
        None => (tr(TODAY).to_owned(), o.today.as_str(), o.today_truncated, o.today_total_bytes),
        Some(i) => match o.recent.get(i) {
            Some(n) => (
                chrono::NaiveDate::parse_from_str(&n.date, "%Y-%m-%d").map(day_label).unwrap_or_else(|_| n.date.clone()),
                n.content.as_str(),
                n.truncated,
                n.total_bytes,
            ),
            None => return,
        },
    };
    let sub = match which {
        None => day_label(chrono::Local::now().date_naive()),
        Some(_) => String::new(),
    };
    page_title(d, "b3_mem_day", &title, &sub, None);
    markdown(d, "b3_mem_day_md", note, 14.0, 1.45, 12.0, true);
    if cut {
        d.gap(W::Fill, 14.0);
        cut_notice(d, "b3_mem_day_cut", note.len() as u64, total);
    }
}

fn add_body(d: &mut Dsl, st: &MemState, sheet: bool) {
    d.text("b3_mem_add_head", tr(ADD_TITLE), &Txt::new(20.0, Face::Semibold, tok::TEXT).w(W::Fill));
    d.gap(W::Fill, 14.0);
    ui::field_label(d, "b3_mem_add_title_label", tr(FIELD_TITLE));
    d.gap(W::Fill, 6.0);
    d.input("b3_mem_add_title", "mem.add.title", &st.add_title_snap, tr(TITLE_HINT), false, if sheet { 40.0 } else { 36.0 });
    d.gap(W::Fill, 12.0);
    ui::field_label(d, "b3_mem_add_note_label", tr(FIELD_NOTE));
    d.gap(W::Fill, 6.0);
    d.input_multiline("b3_mem_add_note", "mem.add.note", &st.add_note_snap, "", if sheet { 120.0 } else { 110.0 });
    d.gap(W::Fill, 6.0);
    d.text("b3_mem_add_hint", tr(NOTE_HINT), &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    d.gap(W::Fill, 12.0);
    d.surface(
        "b3_mem_add_trust",
        "width: Fill height: Fit flow: Right spacing: 10 padding: Inset{left: 14 right: 14 top: 12 bottom: 12}",
        tok::SURFACE2,
        10.0,
        Some(tok::HAIRLINE),
    );
    d.icon("b3_mem_add_trust_icon", "b3_info.svg", 16.0, tok::MUTED);
    d.text("b3_mem_add_trust_text", tr(NOTE_TRUST), &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    d.close();
    if let Some(f) = &st.add_error {
        d.gap(W::Fill, 12.0);
        refusal_box(d, "b3_mem_error", f, &st.profile);
    }
    d.gap(W::Fill, 16.0);
    d.view("b3_mem_add_actions", "width: Fill height: Fit flow: Right spacing: 10");
    d.button("b3_mem_add_cancel", tr(CANCEL), "b3.mem.add.cancel", Btn::Outline, W::Fill, 40.0);
    // From the SNAPSHOT, never the live text: typing must not change the DSL
    // (a remount would rebuild the fields and drop what was typed);
    // `visibility` arms / disarms the submit live.
    let armed = !st.add_note_snap.trim().is_empty() && !st.adding;
    let submit = tr(if st.adding { ADDING } else { ADD_SUBMIT });
    d.view("b3_mem_add_submit_on", &format!("width: Fill height: Fit visible: {armed}"));
    d.button("b3_mem_add_submit", submit, "b3.mem.add.submit", Btn::Primary, W::Fill, 40.0);
    d.close();
    d.view("b3_mem_add_submit_off", &format!("width: Fill height: Fit visible: {}", !armed));
    d.button("b3_mem_add_submit_disabled", submit, "b3.noop", Btn::Disabled, W::Fill, 40.0);
    d.close();
    d.close();
    d.gap(W::Fill, 8.0);
}
