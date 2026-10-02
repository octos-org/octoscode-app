//! A31 — background skill-action jobs (parity row 15).
//!
//! FAILING-FIRST STUB: the method types exist; the notification is still
//! ignored (main's behaviour).
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use octoscode_store::Store;

use crate::method::Method;
use crate::registry::Registry;

/// The negotiated feature that gates the job event and the list/read methods.
pub const FEATURE: &str = "skill.action_jobs.v1";
/// `skill/action/job/list`.
pub const LIST_METHOD: &str = "skill/action/job/list";
/// `skill/action/job/updated`.
pub const UPDATED: &str = "skill/action/job/updated";

#[derive(Debug, Clone, Default, Serialize)]
pub struct JobListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub session_id: String,
}

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

pub fn register(_reg: &mut Registry, _store: Arc<Store>) {}
