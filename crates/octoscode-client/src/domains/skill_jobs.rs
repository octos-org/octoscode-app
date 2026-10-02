//! A31 — background skill-action jobs (parity row 15): the
//! `skill/action/job/updated` notification and the `skill/action/job/list`
//! read, both gated on the negotiated feature `skill.action_jobs.v1`.
//!
//! Server (octos `a6ea8505`, `crates/octos-cli/src/api/ui_protocol_transport.rs`):
//! - the event is `SkillActionJobUpdatedEvent {profile_id, session_id, job}`
//!   (octos-core `ui_protocol.rs:6464`), appended to the ledger by
//!   `install_skill_action_job_projection_listener` on every change of a
//!   skill-action task, so it is durable (replayed after a reconnect);
//! - `skill/action/job/list` (`raw_skill_action_job_list`) takes
//!   `{session_id, profile_id?, batch_id?, action_id?}` and answers
//!   `{profile_id, session_id, count, jobs}`; `skill/action/job/read`
//!   answers one `{job}` (unused: the list carries every field);
//! - a connection that sent features WITHOUT `skill.action_jobs.v1` gets
//!   neither (`skill_action_jobs_available`: the notification filter drops
//!   the event, live and on replay; the two methods are not advertised).
//!   The native client asks for it (`crate::features::NATIVE_UI_FEATURES`).
//!
//! The web has no consumer of either (its `SkillsDialog.tsx` has no job UI;
//! the event is only a name in `generated/core-contract.ts:123`). The
//! native consumer is the Skills dialog's "Background jobs" section
//! (`octoscode-module` `screens/skill_jobs.rs`, drawn by
//! `screens/dialog_view.rs` `skills`).
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octoscode_store::domains::skill_jobs::SkillJob;
use octoscode_store::Store;

use crate::method::Method;
use crate::registry::{NotificationHandler, Registry};

/// The negotiated feature that gates the job event and the list/read methods
/// (`APPUI_FEATURE_SKILL_ACTION_JOBS_V1`).
pub const FEATURE: &str = "skill.action_jobs.v1";
/// `skill/action/job/list` (`APPUI_METHOD_SKILL_ACTION_JOB_LIST`).
pub const LIST_METHOD: &str = "skill/action/job/list";
/// `skill/action/job/updated`.
pub const UPDATED: &str = methods::SKILL_ACTION_JOB_UPDATED;

/// `RawSkillActionJobListParams`: the Session is required; the Profile is
/// checked against the Session's and the connection's
/// (`validate_session_scope`).
#[derive(Debug, Clone, Default, Serialize)]
pub struct JobListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub session_id: String,
}

/// The list reply: the scope the server resolved, and the records (kept as
/// JSON; `SkillJob::from_wire` reads each under that scope).
#[derive(Debug, Clone, Deserialize)]
pub struct JobListResult {
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub count: u64,
    #[serde(default)]
    pub jobs: Vec<Value>,
}

pub struct SkillActionJobList;

impl Method for SkillActionJobList {
    const NAME: &'static str = LIST_METHOD;
    type Params = JobListParams;
    type Result = JobListResult;
}

/// `skill/action/job/updated` → the store's job table, under the event's
/// (Profile, Session). A record that cannot be read, or that names another
/// scope than its envelope, is dropped and logged by name (never fatal).
pub struct SkillJobUpdatedHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for SkillJobUpdatedHandler {
    const METHOD: &'static str = methods::SKILL_ACTION_JOB_UPDATED;
    fn handle(&self, notification: &UiNotification) {
        let UiNotification::SkillActionJobUpdated(e) = notification else {
            return;
        };
        self.store.note_seen(Self::METHOD);
        match SkillJob::from_wire(&e.profile_id, &e.session_id.0, &e.job) {
            Some(job) => {
                let (id, status) = (job.job_id.clone(), job.status.wire().to_owned());
                let changed = self.store.domains.skill_jobs.apply_update(job);
                log::debug!(
                    "octoscode: {} {}/{} job {id} {status} ({})",
                    Self::METHOD,
                    e.profile_id,
                    e.session_id.0,
                    if changed { "applied" } else { "not newer: kept" }
                );
            }
            None => log::warn!(
                "octoscode: {} for {}/{}: an unreadable or foreign job record dropped",
                Self::METHOD,
                e.profile_id,
                e.session_id.0
            ),
        }
    }
}

pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(SkillJobUpdatedHandler { store });
}
