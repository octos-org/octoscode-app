//! A31 — the replay server's `skill-jobs` scenario (parity row 15): one
//! Profile whose Sessions run background skill-action jobs.
//!
//! SYNTHETIC, in octos-cli's own shapes (`a6ea8505`): no recording carries a
//! job — the recorded clients never asked for `skill.action_jobs.v1`, and
//! octos sends job events only to a connection that did. Every frame here is
//! built like the server builds it:
//! - the record: `api/skill_action_jobs.rs` `SkillActionJobRecord`
//!   (`job_id` = the task id, `batch_id`, `profile_id`, `session_id`,
//!   `action_id`, `skill_id`, `status`, `input_path`, `filename`,
//!   `materialized_path`, `output`, `error`, `created_at`, `updated_at` in
//!   chrono's RFC 3339 `AutoSi` form);
//! - the event: `skill/action/job/updated` `{profile_id, session_id, job}`
//!   (`SkillActionJobUpdatedEvent`, emitted by
//!   `install_skill_action_job_projection_listener`);
//! - the list: `{profile_id, session_id, count, jobs}` oldest first
//!   (`raw_skill_action_job_list`, `project_skill_action_jobs` sorts by
//!   `updated_at` then `job_id`);
//! - an orphan across a restart is `abandoned` with the error
//!   "orphaned across restart" (`ORPHANED_ACROSS_RESTART`).
//!
//! Timestamps are relative to the moment a frame is sent, so the app's
//! relative times read the same on every run ("now", "2m", "5m", "20m",
//! "1h").
use serde_json::{json, Value};

pub const FEATURE: &str = "skill.action_jobs.v1";
pub const LIST: &str = "skill/action/job/list";
pub const READ: &str = "skill/action/job/read";
/// A Profile that is not the connection's (its job must never show).
pub const FOREIGN_PROFILE: &str = "acme";

/// RFC 3339 `ago_ms` before now, chrono's `AutoSi` (what serde writes).
pub fn ts(ago_ms: u64) -> String {
    (chrono::Utc::now() - chrono::Duration::milliseconds(ago_ms as i64))
        .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
}

/// The open reply advertises the feature and its two methods (a6ea8505
/// does when the client asked; `--drop-feature` / `--drop-method` withdraw
/// them for the no-feature walk).
pub fn advertise(open: &mut Value) {
    let caps = &mut open["capabilities"];
    if let Some(f) = caps["supported_features"].as_array_mut() {
        if !f.iter().any(|x| x == FEATURE) {
            f.push(json!(FEATURE));
        }
    }
    if let Some(m) = caps["supported_methods"].as_array_mut() {
        for name in [LIST, READ] {
            if !m.iter().any(|x| x == name) {
                m.push(json!(name));
            }
        }
    }
}

/// The second Session (the walk switches to it). A FULL id
/// (`<profile>:<channel>:<chat>`): A22's row-228 projection lists only full
/// Sessions of the profile.
pub fn other_session(home: &str) -> String {
    let profile = home.split(':').next().unwrap_or("dsflash");
    format!("{profile}:api:imports")
}

struct Spec {
    id: &'static str,
    skill: &'static str,
    action: &'static str,
    status: &'static str,
    file: &'static str,
    ago_ms: u64,
    output: Option<&'static str>,
    error: Option<&'static str>,
}

const fn spec(id: &'static str, status: &'static str, file: &'static str, ago_ms: u64) -> Spec {
    Spec { id, skill: "source-skill", action: "source.import", status, file, ago_ms, output: None, error: None }
}

fn record(profile: &str, session: &str, s: &Spec) -> Value {
    let mut j = json!({
        "job_id": s.id,
        "batch_id": "batch-q3",
        "profile_id": profile,
        "session_id": session,
        "action_id": s.action,
        "skill_id": s.skill,
        "status": s.status,
        "input_path": format!("up://{}", s.file),
        "filename": s.file,
        "materialized_path": format!("uploads/{}", s.file),
        "created_at": ts(s.ago_ms + 45_000),
        "updated_at": ts(s.ago_ms),
    });
    if let Some(o) = s.output {
        j["output"] = json!(o);
    }
    if let Some(e) = s.error {
        j["error"] = json!(e);
    }
    j
}

fn event(profile: &str, session: &str, s: &Spec) -> Value {
    json!({"jsonrpc": "2.0", "method": "skill/action/job/updated",
           "params": {"profile_id": profile, "session_id": session, "job": record(profile, session, s)}})
}

/// The home Session's jobs as the server's snapshot holds them when the
/// list is read — q3-review is still RUNNING there (stale: the update that
/// finished it goes out first, [`before_list`]) and notes.md is not in it
/// yet (queued after the snapshot).
fn home_snapshot() -> Vec<Spec> {
    vec![
        spec("job-archive", "abandoned", "archive.zip", 3_600_000).error_("orphaned across restart"),
        Spec { skill: "deck-skill", action: "deck.render", ..spec("job-deck", "cancelled", "q3-board.pptx", 1_200_000) },
        spec("job-scan", "failed", "scan-007.pdf", 300_000).error_("pdftotext exited with status 1"),
        spec("job-q3", "running", "q3-review.pdf", 180_000),
        spec("job-report", "running", "report.pdf", 12_000),
    ]
}

fn other_snapshot() -> Vec<Spec> {
    vec![
        spec("job-statement", "succeeded", "statement.pdf", 480_000).output_("Imported 3 pages"),
        spec("job-invoice", "running", "invoice.pdf", 20_000),
    ]
}

impl Spec {
    fn error_(mut self, e: &'static str) -> Self {
        self.error = Some(e);
        self
    }
    fn output_(mut self, o: &'static str) -> Self {
        self.output = Some(o);
        self
    }
}

/// The `skill/action/job/list` reply for `session` (the home Session, the
/// other one, or none).
pub fn list_reply(profile: &str, session: &str, home: &str) -> Value {
    let specs = if session == home {
        home_snapshot()
    } else if session == other_session(home) {
        other_snapshot()
    } else {
        Vec::new()
    };
    let jobs: Vec<Value> = specs.iter().map(|s| record(profile, session, s)).collect();
    json!({"profile_id": profile, "session_id": session, "count": jobs.len(), "jobs": jobs})
}

/// Announced just BEFORE the home list's reply: newer than its snapshot.
pub fn before_list(profile: &str, home: &str) -> Vec<Value> {
    vec![
        event(profile, home, &spec("job-q3", "succeeded", "q3-review.pdf", 120_000).output_("Imported 14 pages")),
        event(profile, home, &spec("job-notes", "queued", "notes.md", 30_000)),
    ]
}

/// Announced after the first open: a job of the OTHER Session and one of
/// ANOTHER Profile under the home Session's id (neither may show in the
/// home dialog). Without the feature (`announced_only`), also two of the
/// home Session's own jobs — all a client then knows.
pub fn after_open(profile: &str, home: &str, announced_only: bool) -> Vec<Value> {
    let mut out = vec![
        event(profile, &other_session(home), &spec("job-invoice", "running", "invoice.pdf", 20_000)),
        event(FOREIGN_PROFILE, home, &spec("job-foreign", "running", "secret.pdf", 15_000)),
    ];
    if announced_only {
        out.push(event(profile, home, &spec("job-scan", "failed", "scan-007.pdf", 300_000).error_("pdftotext exited with status 1")));
        out.push(event(profile, home, &spec("job-report", "running", "report.pdf", 12_000)));
    }
    out
}

/// The live transition the walk asks for (its trigger file): report.pdf
/// finishes and notes.md starts.
pub fn live(profile: &str, home: &str) -> Vec<Value> {
    vec![
        event(profile, home, &spec("job-report", "succeeded", "report.pdf", 0).output_("Imported 9 pages")),
        event(profile, home, &spec("job-notes", "running", "notes.md", 0)),
    ]
}

/// The Profile's installed skills (board 4 region 3's Installed list).
pub fn installed_skills(profile: &str) -> Value {
    json!({"count": 2, "profile_id": profile, "skills": [
        {"name": "source-skill", "version": "0.3.1", "tool_count": 4},
        {"name": "deck-skill", "version": "1.2.0", "tool_count": 2}
    ]})
}

/// The catalog: the home Session and the other one.
pub fn sessions(home: &str) -> Value {
    json!({"sessions": [
        {"id": home, "title": "Why does main.rs print 5?", "message_count": 1, "updated_at": ts(60_000), "active_turn": false},
        {"id": other_session(home), "title": "Import the Q3 reports", "message_count": 2, "updated_at": ts(600_000), "active_turn": false}
    ]})
}
