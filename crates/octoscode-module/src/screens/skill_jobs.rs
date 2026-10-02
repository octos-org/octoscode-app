//! A31 — the Skills dialog's "Background jobs" section (parity row 15).
//!
//! Design: board 4 region 3 (`design/stage-a/phase4-new4/atlas.png`,
//! README "Row 15: skill-job status"; README copy and Errata win over the
//! pixels). The web has no job UI (`SkillsDialog.tsx`), so this is the
//! minimal honest one the board draws:
//!
//! - **Where.** At the top of the Skills dialog (under its description),
//!   above Installed: "Background jobs", the active-job count on the right
//!   ("1 running · 1 queued"), then one row per job, newest first.
//! - **Scope.** The jobs of the Profile AND Session the dialog is scoped to
//!   (the operator default): the dialog's Profile (its "Server Profile:"
//!   scope line) and the active Session. Another Session's or Profile's job
//!   never shows (the store keys every record by both).
//! - **Seed.** When the server advertises `skill.action_jobs.v1` (the open
//!   reply's `supported_features`) and `skill/action/job/list`, opening the
//!   dialog reads the list for that scope, so the section is honest after a
//!   reconnect; the `skill/action/job/updated` notifications keep it live.
//!   Without the feature it shows only the jobs announced since connecting
//!   and says so in a muted line ([`ANNOUNCED_ONLY`]).
//! - **Status** (README table; the plan labels of `fleet.rs` `status_word`):
//!   queued ○ Queued (blue), running ● Running (green), succeeded ✓ Done
//!   (grey) with the job's `output`, failed ✕ Failed (red) with "Couldn't
//!   finish this job." and the `error` as a muted cause, cancelled ✕ Stopped
//!   (grey), abandoned ✕ Stopped (grey) with "The server restarted before
//!   this job finished.".
//!
//! The race between the list reply (applied on the refresh task) and the
//! notifications (applied on the UI's event drain) is the store's
//! (`octoscode_store::domains::skill_jobs`): every reconcile is idempotent.
use octoscode_client::domains::skill_jobs::{JobListParams, SkillActionJobList};
use octoscode_store::domains::skill_jobs::{JobStatus, ListState, SkillJob};
use octoscode_store::Store;

use crate::flow::Conversation;
use crate::screens::board3::ui::{self, tok};
use crate::screens::dialog as dlg;

pub use octoscode_client::domains::skill_jobs::{FEATURE, LIST_METHOD};

/// The most rows the section draws: every active job always shows, then the
/// newest finished ones up to this many rows; the rest are counted.
pub const MAX_ROWS: usize = 20;

// ---- copy (English source keys; drawn through `tr`, zh in `i18n::native`
// unless noted).
pub const HEADING: &str = "Background jobs";
/// The web's own key ("{count} 个运行中").
pub const RUNNING_COUNT: &str = "{count} running";
pub const QUEUED_COUNT: &str = "{value0} queued";
pub const FAILED_LEAD: &str = "Couldn't finish this job.";
pub const ABANDONED_NOTE: &str = "The server restarted before this job finished.";
pub const EMPTY: &str = "No background jobs in this Session.";
pub const LOADING: &str = "Loading background jobs…";
pub const LOAD_FAILED: &str = "Couldn't load background jobs.";
pub const ANNOUNCED_ONLY: &str =
    "Only jobs announced since the app connected are shown; this server doesn't list earlier jobs.";
/// The web's own key ("（另有 {value0} 项未显示）").
pub const OMITTED: &str = "(+{value0} omitted)";

/// Whether this server seeds the section: it advertises the feature (the
/// `session/open` reply's `supported_features`) and the list method.
pub fn seeds(store: &Store) -> bool {
    store.domains.config.supported_features().iter().any(|f| f == FEATURE) && dlg::advertises(store, LIST_METHOD)
}

/// The (Profile, Session) the Skills dialog is scoped to: its "Server
/// Profile:" scope line's Profile and the active Session.
pub fn scope(store: &Store) -> Option<(String, String)> {
    let session = store.active_session().filter(|s| !s.trim().is_empty())?;
    let profile = store.domains.profile.current().filter(|p| !p.trim().is_empty())?;
    Some((profile, session))
}

/// The dialog's on-open read (`dialog.refresh.skill_jobs`, the host's
/// `Effect::RefreshSkillJobs` arm): `skill/action/job/list` for the
/// dialog's scope when the server seeds; otherwise nothing is sent (the
/// section keeps the jobs announced since connecting).
pub async fn refresh(conv: &Conversation, store: &Store) -> Result<String, String> {
    // The Skills reads are Profile-scoped like the web's; the store learns
    // the connection's Profile if no open named it (as `models::refresh`).
    if store.domains.profile.current().is_none() {
        store.domains.profile.set_current(conv.profile());
    }
    let Some((profile, session)) = scope(store) else {
        return Err("no active Session to read background jobs for".to_owned());
    };
    if !seeds(store) {
        return Ok(format!("{FEATURE} not advertised: only jobs announced since connecting"));
    }
    let jobs = &store.domains.skill_jobs;
    let ticket = jobs.begin_list(&profile, &session);
    let params = JobListParams { profile_id: Some(profile.clone()), session_id: session.clone() };
    let reply = match conv.client().call::<SkillActionJobList>(params).await {
        Ok(r) => r,
        Err(e) => {
            let e = e.to_string();
            jobs.fail_list(ticket, &profile, &session, e.clone());
            return Err(e);
        }
    };
    // The reply names the scope it answered; a list for another scope is
    // never shown under this one.
    let (rp, rs) = (
        reply.profile_id.clone().unwrap_or_else(|| profile.clone()),
        reply.session_id.clone().unwrap_or_else(|| session.clone()),
    );
    if rp != profile || rs != session {
        let e = format!("{LIST_METHOD} answered for {rp} / {rs}, not {profile} / {session}");
        jobs.fail_list(ticket, &profile, &session, e.clone());
        return Err(e);
    }
    let read: Vec<SkillJob> = reply.jobs.iter().filter_map(|j| SkillJob::from_wire(&profile, &session, j)).collect();
    let refused = reply.jobs.len() - read.len();
    let n = read.len();
    let applied = jobs.apply_list(ticket, &profile, &session, read);
    Ok(format!(
        "{n} job(s) listed for {session}{}{}",
        if refused > 0 { format!(", {refused} unreadable or foreign record(s) refused") } else { String::new() },
        if applied { "" } else { " (overtaken by a newer list: ignored)" }
    ))
}

/// A status chip: the README's glyph, word and ink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chip {
    pub glyph: &'static str,
    /// The English source word (drawn through `tr`).
    pub word: &'static str,
    pub fg: &'static str,
    pub bg: &'static str,
}

/// The README's status table (the Fleet's glyphs: ○ ● ✓ ✕; "Done" and
/// "Stopped" are `fleet.rs` `status_word`'s plan labels).
pub fn chip(status: &JobStatus) -> Chip {
    let c = |glyph, word, fg, bg| Chip { glyph, word, fg, bg };
    match status {
        JobStatus::Queued => c("○", "Queued", tok::BLUE_TEXT, tok::BLUE_BG),
        JobStatus::Running => c("●", "Running", tok::GREEN_TEXT, tok::GREEN_BG),
        JobStatus::Succeeded => c("✓", "Done", tok::TEXT, tok::CHIP),
        JobStatus::Failed => c("✕", "Failed", tok::RED_TEXT, tok::RED_BG),
        JobStatus::Cancelled | JobStatus::Abandoned => c("✕", "Stopped", tok::TEXT, tok::CHIP),
        // A newer server's status: the Fleet's honest unknown.
        JobStatus::Unknown(_) => c("?", "Outcome unknown", tok::MUTED, tok::CHIP),
    }
}

/// A row's message line(s).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    None,
    /// A finished job's `output` (its first line).
    Output(String),
    /// A failed job: the lead, then the server's `error` as a muted cause
    /// (empty when the server gave none).
    Failure { lead: &'static str, cause: String },
    /// A muted explanation (an abandoned job).
    Note(&'static str),
}

/// Whitespace collapsed, at most 512 characters (the kit's cause rule).
fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(512).collect()
}

pub fn message(job: &SkillJob) -> Message {
    match job.status {
        JobStatus::Succeeded => job
            .output
            .as_deref()
            .and_then(|o| o.lines().map(str::trim).find(|l| !l.is_empty()))
            .map(|l| Message::Output(clean(l)))
            .unwrap_or(Message::None),
        JobStatus::Failed => Message::Failure { lead: FAILED_LEAD, cause: job.error.as_deref().map(clean).unwrap_or_default() },
        JobStatus::Abandoned => Message::Note(ABANDONED_NOTE),
        _ => Message::None,
    }
}

/// One drawn row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub job_id: String,
    pub skill: String,
    pub action: String,
    pub name: String,
    pub chip: Chip,
    /// Since the job's last change (`updated_at`), the web's relative time.
    pub time: String,
    pub message: Message,
}

/// What the section draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub running: usize,
    pub queued: usize,
    /// The server does not seed the section (no `skill.action_jobs.v1`).
    pub announced_only: bool,
    pub state: ListState,
    pub rows: Vec<Row>,
    /// Older finished jobs not drawn ([`MAX_ROWS`]).
    pub omitted: usize,
}

fn row(job: &SkillJob, now_ms: u64) -> Row {
    Row {
        job_id: job.job_id.clone(),
        skill: job.skill_id.clone(),
        action: job.action_id.clone(),
        name: job.display_name(),
        chip: chip(&job.status),
        time: job
            .updated_ns()
            .filter(|ns| *ns >= 0)
            .map(|ns| ui::rel_time(now_ms, (ns / 1_000_000) as u64))
            .unwrap_or_default(),
        message: message(job),
    }
}

/// The section for the dialog's scope (`None`: no scope yet — no Session or
/// Profile — so nothing is drawn).
pub fn section(store: &Store, now_ms: u64) -> Option<Section> {
    let (profile, session) = scope(store)?;
    let jobs = store.domains.skill_jobs.jobs(&profile, &session);
    let running = jobs.iter().filter(|j| j.status == JobStatus::Running).count();
    let queued = jobs.iter().filter(|j| j.status == JobStatus::Queued).count();
    let announced_only = !seeds(store);
    let mut state = store.domains.skill_jobs.list_state(&profile, &session);
    if state == ListState::NotRequested && !announced_only {
        // The on-open read is about to start: say "Loading", not "none".
        state = ListState::Loading;
    }
    let room = MAX_ROWS.saturating_sub(jobs.iter().filter(|j| !j.status.is_terminal()).count());
    let (mut rows, mut finished, mut omitted) = (Vec::new(), 0usize, 0usize);
    for j in &jobs {
        if j.status.is_terminal() {
            if finished == room {
                omitted += 1;
                continue;
            }
            finished += 1;
        }
        rows.push(row(j, now_ms));
    }
    Some(Section { running, queued, announced_only, state, rows, omitted })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{self, Lang};

    #[test]
    fn every_copy_key_reads_in_chinese() {
        for en in [HEADING, FAILED_LEAD, ABANDONED_NOTE, EMPTY, LOADING, LOAD_FAILED, ANNOUNCED_ONLY, "Queued", "Running", "Done", "Failed", "Stopped", "Outcome unknown"] {
            let zh = i18n::tr_in(Lang::Zh, en);
            assert!(zh.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)), "{en:?} -> {zh:?}");
        }
        assert_eq!(i18n::text_in(Lang::Zh, RUNNING_COUNT, &[("count", "3")]), "3 个运行中");
        assert_eq!(i18n::text_in(Lang::Zh, QUEUED_COUNT, &[("value0", "2")]), "2 个排队中");
        assert_eq!(i18n::text_in(Lang::Zh, OMITTED, &[("value0", "4")]), "（另有 4 项未显示）");
        // The dialog around the section reads Chinese too (web keys,
        // aliases of the same controls, the warning's native wording).
        for en in [crate::screens::dialog::SKILLS_WARNING, "Skills", "Installed", "Registry", "Install", "Server Profile:", "tools"] {
            let zh = i18n::tr_in(Lang::Zh, en);
            assert!(zh.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)), "{en:?} -> {zh:?}");
        }
    }

    #[test]
    fn a_long_output_keeps_its_first_line() {
        let mut j = SkillJob::from_wire(
            "p",
            "p:s",
            &serde_json::json!({"job_id": "j", "status": "succeeded", "updated_at": "2026-10-02T10:00:00Z",
                                "output": "\n  Imported   14 pages\nwarnings: 2\n"}),
        )
        .unwrap();
        assert_eq!(message(&j), Message::Output("Imported 14 pages".into()));
        j.output = Some("   \n".into());
        assert_eq!(message(&j), Message::None);
    }
}
