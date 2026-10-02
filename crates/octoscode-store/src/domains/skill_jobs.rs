//! A31 — background skill-action jobs (parity row 15): the server's
//! `SkillActionJobRecord` snapshots, kept per (Profile, Session).
//!
//! ## Wire sources (octos `a6ea8505`)
//!
//! - `skill/action/job/updated` — `SkillActionJobUpdatedEvent
//!   {profile_id, session_id, job}` (octos-core `ui_protocol.rs:6464`),
//!   emitted by octos-cli's projection listener on every task change
//!   (`ui_protocol_transport.rs` `install_skill_action_job_projection_listener`).
//!   `job` is the record below; the listener only emits a record whose own
//!   profile/session equal the envelope's.
//! - `skill/action/job/list` — `{profile_id, session_id, count, jobs}`
//!   (`raw_skill_action_job_list`), the Session's retained jobs, oldest
//!   first, at most 256 (`MAX_RETAINED_JOBS_PER_SESSION`: every active job
//!   plus the newest finished ones).
//! - The record: `api/skill_action_jobs.rs` `SkillActionJobRecord` —
//!   `job_id` (= the task id), `batch_id`, `profile_id`, `session_id`,
//!   `action_id`, `skill_id`, `status` (`queued | running | succeeded |
//!   failed | cancelled | abandoned`), optional `input_path`, `filename`,
//!   `materialized_path`, `output`, `error`, `result`, and RFC 3339
//!   `created_at` / `updated_at` (chrono `AutoSi`: 0, 3, 6 or 9 fraction
//!   digits — so the text does NOT sort by time).
//!
//! ## The reconcile (outer/LESSONS.md: replies and notifications apply on
//! different tasks)
//!
//! Every record of one job is compared by one total order, [`Key`]: a
//! terminal status first (a finished job never turns active again), then
//! `updated_at` as an instant, then the status rank, then the content. The
//! store keeps the greatest record seen, so applying the same records in any
//! order — a list reply before or after the notifications around it, a
//! replayed notification, a duplicate — reaches the same state, and a
//! record that is not greater changes nothing.
//!
//! A list reply also says what the server NO LONGER has: a job the client
//! knew before the request was sent that the reply does not name was pruned
//! (or lost with the server's ledger), so it goes. A job announced after the
//! request was sent stays — the reply's snapshot may predate it. "Before"
//! is the store's own apply counter, never a clock. A reply overtaken by a
//! newer applied reply for the same scope is ignored whole.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use serde_json::Value;

/// The server's per-Session retention (`MAX_RETAINED_JOBS_PER_SESSION`):
/// the client keeps no more per (Profile, Session), dropping the oldest
/// finished jobs first and never an active one.
pub const MAX_JOBS_PER_SCOPE: usize = 256;

/// A job's lifecycle state (`SkillActionJobStatus`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    /// Orphaned across a server restart, or parked awaiting re-attachment
    /// (`projected_status`).
    Abandoned,
    /// A status this client does not know (a newer server): kept and shown,
    /// never treated as finished.
    Unknown(String),
}

impl JobStatus {
    pub fn parse(s: &str) -> Self {
        match s.trim() {
            "queued" => JobStatus::Queued,
            "running" => JobStatus::Running,
            "succeeded" => JobStatus::Succeeded,
            "failed" => JobStatus::Failed,
            "cancelled" => JobStatus::Cancelled,
            "abandoned" => JobStatus::Abandoned,
            other => JobStatus::Unknown(other.to_owned()),
        }
    }

    /// The wire word.
    pub fn wire(&self) -> &str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Succeeded => "succeeded",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
            JobStatus::Abandoned => "abandoned",
            JobStatus::Unknown(s) => s,
        }
    }

    /// Finished: the job will not change state again.
    pub fn is_terminal(&self) -> bool {
        matches!(self, JobStatus::Succeeded | JobStatus::Failed | JobStatus::Cancelled | JobStatus::Abandoned)
    }

    /// Queued or running (the section header counts these).
    pub fn is_active(&self) -> bool {
        matches!(self, JobStatus::Queued | JobStatus::Running)
    }

    /// The lifecycle order, for two records with the same `updated_at`.
    fn rank(&self) -> u8 {
        match self {
            JobStatus::Queued => 0,
            JobStatus::Unknown(_) => 1,
            JobStatus::Running => 2,
            _ => 3,
        }
    }
}

/// One job (`SkillActionJobRecord`), under the (Profile, Session) that
/// carried it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillJob {
    pub profile_id: String,
    pub session_id: String,
    pub job_id: String,
    pub batch_id: String,
    pub action_id: String,
    pub skill_id: String,
    pub status: JobStatus,
    pub filename: Option<String>,
    pub input_path: Option<String>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl SkillJob {
    /// Read one server record under the scope that carried it (the event's
    /// envelope, or the list reply's own `profile_id` / `session_id`).
    /// `None` for a record without its identity or status, and for a record
    /// whose own `profile_id` / `session_id` name another scope — the
    /// server's listener emits only matching pairs, so a mismatch is never
    /// shown under this scope.
    pub fn from_wire(profile_id: &str, session_id: &str, job: &Value) -> Option<SkillJob> {
        let o = job.as_object()?;
        let text = |k: &str| o.get(k).and_then(Value::as_str).map(str::to_owned);
        let some = |k: &str| text(k).filter(|v| !v.trim().is_empty());
        let job_id = some("job_id")?;
        let status = JobStatus::parse(&some("status")?);
        if text("profile_id").is_some_and(|p| p != profile_id) || text("session_id").is_some_and(|s| s != session_id) {
            return None;
        }
        Some(SkillJob {
            profile_id: profile_id.to_owned(),
            session_id: session_id.to_owned(),
            job_id,
            batch_id: text("batch_id").unwrap_or_default(),
            action_id: text("action_id").unwrap_or_default(),
            skill_id: text("skill_id").unwrap_or_default(),
            status,
            filename: some("filename"),
            input_path: some("input_path"),
            output: some("output"),
            error: some("error"),
            created_at: text("created_at").unwrap_or_default(),
            updated_at: text("updated_at").unwrap_or_default(),
        })
    }

    /// `updated_at` as nanoseconds since the epoch (`None`: unreadable).
    pub fn updated_ns(&self) -> Option<i128> {
        parse_instant_ns(&self.updated_at)
    }

    /// What the row names: the file, else the input's file name, else the
    /// job id.
    pub fn display_name(&self) -> String {
        if let Some(f) = &self.filename {
            return f.clone();
        }
        if let Some(leaf) = self
            .input_path
            .as_deref()
            .map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p).trim())
            .filter(|leaf| !leaf.is_empty())
        {
            return leaf.to_owned();
        }
        self.job_id.clone()
    }
}

/// The total order of one job's records (the reconcile, module docs).
type Key = (bool, Option<i128>, u8, Option<String>, Option<String>, Option<String>, String);

fn key(j: &SkillJob) -> Key {
    (
        j.status.is_terminal(),
        j.updated_ns(),
        j.status.rank(),
        j.output.clone(),
        j.error.clone(),
        j.filename.clone(),
        format!("{}|{}|{}|{}", j.status.wire(), j.batch_id, j.action_id, j.skill_id),
    )
}

/// RFC 3339 (`2026-10-02T10:00:00Z`, any fraction length, `Z` or `±HH:MM`)
/// to nanoseconds since the epoch.
pub fn parse_instant_ns(s: &str) -> Option<i128> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b't' | b' ') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let t = s.get(r)?;
        t.bytes().all(|c| c.is_ascii_digit()).then(|| t.parse().ok())?
    };
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, se) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let mut rest = &s[19..];
    let mut frac: i128 = 0;
    if let Some(r) = rest.strip_prefix('.') {
        let digits = r.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let kept = &r[..digits.min(9)];
        frac = kept.parse::<i128>().ok()? * 10i128.pow(9 - kept.len() as u32);
        rest = &r[digits..];
    }
    let offset_s: i64 = match rest {
        "Z" | "z" => 0,
        _ => {
            let sign = match rest.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            if rest.len() != 6 || rest.as_bytes()[3] != b':' {
                return None;
            }
            let oh: i64 = rest[1..3].parse().ok()?;
            let om: i64 = rest[4..6].parse().ok()?;
            sign * (oh * 3600 + om * 60)
        }
    };
    // Howard Hinnant's days-from-civil (proleptic Gregorian).
    let yy = if mo <= 2 { y - 1 } else { y };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = days * 86_400 + h * 3600 + mi * 60 + se - offset_s;
    Some(secs as i128 * 1_000_000_000 + frac)
}

/// What the client knows about one scope's job list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ListState {
    /// Never read (or the server does not offer the list).
    #[default]
    NotRequested,
    /// A `skill/action/job/list` is in flight.
    Loading,
    /// The newest request's reply applied.
    Loaded,
    /// The newest request failed (its error as the client reported it).
    Failed(String),
}

#[derive(Debug)]
struct Entry {
    job: SkillJob,
    key: Key,
    /// The store's apply counter when this record last changed.
    seq: u64,
}

#[derive(Debug, Default)]
struct Scope {
    jobs: BTreeMap<String, Entry>,
    /// The newest list request sent (its ticket).
    requested: u64,
    /// The newest list reply applied (its ticket).
    applied: u64,
    state: ListState,
}

#[derive(Debug, Default)]
struct Inner {
    /// The apply counter: every update, request and reply takes the next.
    seq: u64,
    scopes: BTreeMap<(String, String), Scope>,
}

/// The skill-job domain.
#[derive(Debug, Default)]
pub struct SkillJobs {
    inner: Mutex<Inner>,
}

impl Scope {
    /// Keep the greater record (module docs); `true` when it changed.
    fn merge(&mut self, job: SkillJob, seq: u64) -> bool {
        let k = key(&job);
        match self.jobs.get_mut(&job.job_id) {
            Some(e) if k <= e.key => false,
            Some(e) => {
                e.job = job;
                e.key = k;
                e.seq = seq;
                true
            }
            None => {
                self.jobs.insert(job.job_id.clone(), Entry { job, key: k, seq });
                self.prune();
                true
            }
        }
    }

    /// The retention: past [`MAX_JOBS_PER_SCOPE`], the oldest finished jobs
    /// go (never an active one).
    fn prune(&mut self) {
        if self.jobs.len() <= MAX_JOBS_PER_SCOPE {
            return;
        }
        let mut done: Vec<(Option<i128>, String)> = self
            .jobs
            .values()
            .filter(|e| e.job.status.is_terminal())
            .map(|e| (e.job.updated_ns(), e.job.job_id.clone()))
            .collect();
        done.sort();
        let excess = self.jobs.len() - MAX_JOBS_PER_SCOPE;
        for (_, id) in done.into_iter().take(excess) {
            self.jobs.remove(&id);
        }
    }
}

impl SkillJobs {
    /// One `skill/action/job/updated` record. `true` when it changed what
    /// the client knows (a duplicate or an older record does not).
    pub fn apply_update(&self, job: SkillJob) -> bool {
        let mut inner = self.inner.lock().unwrap();
        inner.seq += 1;
        let seq = inner.seq;
        let scope = inner.scopes.entry((job.profile_id.clone(), job.session_id.clone())).or_default();
        scope.merge(job, seq)
    }

    /// A `skill/action/job/list` for (profile, session) is about to be sent:
    /// its ticket, for [`Self::apply_list`] / [`Self::fail_list`].
    pub fn begin_list(&self, profile_id: &str, session_id: &str) -> u64 {
        let mut inner = self.inner.lock().unwrap();
        inner.seq += 1;
        let ticket = inner.seq;
        let scope = inner.scopes.entry((profile_id.to_owned(), session_id.to_owned())).or_default();
        scope.requested = ticket;
        scope.state = ListState::Loading;
        ticket
    }

    /// The reply to the request `ticket`: every listed record merges; a job
    /// the client knew before the request that the reply does not name is
    /// gone; a job announced since stays. A reply older than one already
    /// applied changes nothing (`false`).
    pub fn apply_list(&self, ticket: u64, profile_id: &str, session_id: &str, jobs: Vec<SkillJob>) -> bool {
        let mut inner = self.inner.lock().unwrap();
        inner.seq += 1;
        let seq = inner.seq;
        let scope = inner.scopes.entry((profile_id.to_owned(), session_id.to_owned())).or_default();
        if ticket < scope.applied {
            return false;
        }
        let mut listed = BTreeSet::new();
        for job in jobs.into_iter().filter(|j| j.profile_id == profile_id && j.session_id == session_id) {
            listed.insert(job.job_id.clone());
            scope.merge(job, seq);
        }
        scope.jobs.retain(|id, e| listed.contains(id) || e.seq > ticket);
        scope.applied = ticket;
        if ticket >= scope.requested {
            scope.state = ListState::Loaded;
        }
        true
    }

    /// The request `ticket` failed. Only the newest request's failure is
    /// recorded; what the client knew stays.
    pub fn fail_list(&self, ticket: u64, profile_id: &str, session_id: &str, error: String) {
        let mut inner = self.inner.lock().unwrap();
        let scope = inner.scopes.entry((profile_id.to_owned(), session_id.to_owned())).or_default();
        if ticket < scope.applied || ticket < scope.requested {
            return;
        }
        scope.state = ListState::Failed(error);
    }

    /// The scope's jobs, newest first (`updated_at`, then the job id).
    pub fn jobs(&self, profile_id: &str, session_id: &str) -> Vec<SkillJob> {
        let inner = self.inner.lock().unwrap();
        let Some(scope) = inner.scopes.get(&(profile_id.to_owned(), session_id.to_owned())) else {
            return Vec::new();
        };
        let mut out: Vec<SkillJob> = scope.jobs.values().map(|e| e.job.clone()).collect();
        out.sort_by(|a, b| b.updated_ns().cmp(&a.updated_ns()).then_with(|| b.job_id.cmp(&a.job_id)));
        out
    }

    pub fn list_state(&self, profile_id: &str, session_id: &str) -> ListState {
        let inner = self.inner.lock().unwrap();
        inner
            .scopes
            .get(&(profile_id.to_owned(), session_id.to_owned()))
            .map(|s| s.state.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instants_parse_every_server_shape() {
        let z = parse_instant_ns("2026-10-02T10:00:00Z").unwrap();
        assert_eq!(parse_instant_ns("2026-10-02T10:00:00.5Z").unwrap() - z, 500_000_000);
        assert_eq!(parse_instant_ns("2026-10-02T10:00:00.123456789Z").unwrap() - z, 123_456_789);
        assert_eq!(parse_instant_ns("2026-10-02T12:00:00+02:00").unwrap(), z);
        assert_eq!(parse_instant_ns("2026-10-02T09:30:00-00:30").unwrap(), z);
        assert_eq!(z, 1_790_935_200 * 1_000_000_000);
        for bad in ["", "yesterday", "2026-10-02", "2026-13-02T10:00:00Z", "2026-10-02T10:00:00", "2026-10-02T10:00:00.Z"] {
            assert_eq!(parse_instant_ns(bad), None, "{bad:?}");
        }
    }
}
