//! A31 — `skill/action/job/updated` has an owner (parity row 15).
//!
//! It used to be a card #22 IGNORE entry ("web ignores; job state read on
//! demand"), so the decoded event reached nothing. The skill-jobs domain now
//! handles it: the frame, decoded exactly as the transport decodes it,
//! folds the server's `SkillActionJobRecord` into the store under the
//! (Profile, Session) that carried it.
use std::sync::Arc;

use serde_json::json;

use octos_core::app_ui::AppUiBackendEvent;
use octoscode_client::domains;
use octoscode_client::registry::Registry;
use octoscode_store::domains::skill_jobs::JobStatus;
use octoscode_store::Store;

fn wired() -> (Registry, Arc<Store>) {
    let store = Arc::new(Store::new());
    let mut reg = Registry::new();
    domains::register_all(&mut reg, store.clone());
    (reg, store)
}

#[test]
fn the_job_update_is_handled_not_ignored() {
    let (reg, _) = wired();
    assert!(reg.handles("skill/action/job/updated"), "a handler owns it");
    assert!(!reg.is_ignored("skill/action/job/updated"), "no longer an ignore entry");
}

#[test]
fn a_decoded_update_folds_into_its_scope() {
    let (mut reg, store) = wired();
    let params = json!({
        "profile_id": "dsflash",
        "session_id": "dsflash:main",
        "job": {
            "job_id": "task-7", "batch_id": "batch-a", "profile_id": "dsflash", "session_id": "dsflash:main",
            "action_id": "source.import", "skill_id": "source-skill", "status": "succeeded",
            "input_path": "up://report.pdf", "filename": "report.pdf", "materialized_path": "uploads/report.pdf",
            "output": "Imported 14 pages", "result": {"success": true},
            "created_at": "2026-10-02T10:00:00.123456Z", "updated_at": "2026-10-02T10:01:30.5Z"
        }
    });
    let n = AppUiBackendEvent::from_method_and_params("skill/action/job/updated", params).expect("decodes");
    assert!(reg.dispatch(&n));
    assert_eq!(store.seen_count("skill/action/job/updated"), 1);
    let jobs = store.domains.skill_jobs.jobs("dsflash", "dsflash:main");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job_id, "task-7");
    assert_eq!(jobs[0].status, JobStatus::Succeeded);
    assert_eq!(jobs[0].output.as_deref(), Some("Imported 14 pages"));
    assert!(store.domains.skill_jobs.jobs("dsflash", "dsflash:other").is_empty());
    assert!(!store.has_unhandled());
}

#[test]
fn a_malformed_job_is_tolerated_and_dropped_by_name() {
    let (mut reg, store) = wired();
    let n = AppUiBackendEvent::from_method_and_params(
        "skill/action/job/updated",
        json!({"profile_id": "dsflash", "session_id": "dsflash:main", "job": {"status": "running"}}),
    )
    .expect("decodes");
    assert!(reg.dispatch(&n), "resolved (never fatal)");
    assert!(store.domains.skill_jobs.jobs("dsflash", "dsflash:main").is_empty());
}
