//! A31 — background skill-action jobs (parity row 15): the server's
//! `SkillActionJobRecord` snapshots, keyed by the (Profile, Session) that
//! owns them.
//!
//! FAILING-FIRST STUB: the API the row-15 tests address, with main's
//! behaviour (the event is dropped, nothing is listed).
use std::sync::Mutex;

/// A job's lifecycle state (`SkillActionJobStatus`, octos-cli
/// `api/skill_action_jobs.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Abandoned,
    /// A status this client does not know (a newer server).
    Unknown(String),
}

impl JobStatus {
    pub fn parse(s: &str) -> Self {
        JobStatus::Unknown(s.to_owned())
    }

    pub fn is_terminal(&self) -> bool {
        false
    }

    pub fn is_active(&self) -> bool {
        false
    }
}

/// One job (`SkillActionJobRecord`).
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
    pub fn from_wire(_profile_id: &str, _session_id: &str, _job: &serde_json::Value) -> Option<SkillJob> {
        None
    }

    pub fn display_name(&self) -> String {
        self.job_id.clone()
    }
}

/// What the client knows about one scope's job list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListState {
    NotRequested,
    Loading,
    Loaded,
    Failed(String),
}

#[derive(Debug, Default)]
pub struct SkillJobs {
    _inner: Mutex<()>,
}

impl SkillJobs {
    pub fn apply_update(&self, _job: SkillJob) -> bool {
        false
    }

    pub fn begin_list(&self, _profile_id: &str, _session_id: &str) -> u64 {
        0
    }

    pub fn apply_list(&self, _ticket: u64, _profile_id: &str, _session_id: &str, _jobs: Vec<SkillJob>) -> bool {
        false
    }

    pub fn fail_list(&self, _ticket: u64, _profile_id: &str, _session_id: &str, _error: String) {}

    pub fn jobs(&self, _profile_id: &str, _session_id: &str) -> Vec<SkillJob> {
        Vec::new()
    }

    pub fn list_state(&self, _profile_id: &str, _session_id: &str) -> ListState {
        ListState::NotRequested
    }
}
