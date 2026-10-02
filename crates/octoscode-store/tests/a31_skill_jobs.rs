//! A31 — the skill-job store's reconcile rules (parity row 15).
//!
//! Notifications (`skill/action/job/updated`) and `skill/action/job/list`
//! replies reach the store on different tasks in the native client, so the
//! store must reach the same state whatever order they apply in: the newest
//! record per job by `updated_at`, a terminal status never regressing to an
//! active one, duplicates changing nothing, and each (Profile, Session)
//! kept apart.
use serde_json::{json, Value};

use octoscode_store::domains::skill_jobs::{JobStatus, ListState, SkillJob, SkillJobs};

const P: &str = "dsflash";
const S: &str = "dsflash:main";

fn rec(profile: &str, session: &str, id: &str, status: &str, updated_at: &str) -> Value {
    json!({
        "job_id": id, "batch_id": "b", "profile_id": profile, "session_id": session,
        "action_id": "source.import", "skill_id": "source-skill", "status": status,
        "filename": format!("{id}.pdf"), "created_at": "2026-10-02T10:00:00Z", "updated_at": updated_at,
    })
}

fn job(id: &str, status: &str, updated_at: &str) -> SkillJob {
    SkillJob::from_wire(P, S, &rec(P, S, id, status, updated_at)).expect("a server record")
}

fn status_of(store: &SkillJobs, id: &str) -> Option<JobStatus> {
    store.jobs(P, S).into_iter().find(|j| j.job_id == id).map(|j| j.status)
}

#[test]
fn from_wire_reads_the_server_record_and_refuses_a_foreign_scope() {
    let mut r = rec(P, S, "j1", "failed", "2026-10-02T10:00:01.5Z");
    r["error"] = json!("pdftotext exited with status 1");
    r["output"] = json!(null);
    r["result"] = json!({"success": false});
    let j = SkillJob::from_wire(P, S, &r).expect("parsed");
    assert_eq!(j.status, JobStatus::Failed);
    assert_eq!(j.error.as_deref(), Some("pdftotext exited with status 1"));
    assert_eq!(j.output, None);
    assert_eq!(j.display_name(), "j1.pdf");
    // The record's own scope must be the envelope's.
    assert!(SkillJob::from_wire(P, "dsflash:other", &r).is_none(), "another Session's record");
    assert!(SkillJob::from_wire("other", S, &r).is_none(), "another Profile's record");
    // A record without its identity is refused, never half-shown.
    let mut bad = r.clone();
    bad.as_object_mut().unwrap().remove("job_id");
    assert!(SkillJob::from_wire(P, S, &bad).is_none());
    // No filename: the input's file name, else the job id.
    let mut n = rec(P, S, "j2", "queued", "2026-10-02T10:00:00Z");
    n.as_object_mut().unwrap().remove("filename");
    n["input_path"] = json!("up://inbox/notes.md");
    assert_eq!(SkillJob::from_wire(P, S, &n).unwrap().display_name(), "notes.md");
    n.as_object_mut().unwrap().remove("input_path");
    assert_eq!(SkillJob::from_wire(P, S, &n).unwrap().display_name(), "j2");
    // A status this client does not know is kept, not dropped.
    let u = SkillJob::from_wire(P, S, &rec(P, S, "j3", "paused", "2026-10-02T10:00:00Z")).unwrap();
    assert_eq!(u.status, JobStatus::Unknown("paused".into()));
}

#[test]
fn timestamps_compare_by_instant_not_by_text() {
    // "…:00.5Z" sorts BEFORE "…:00Z" as text but is later in time; an
    // offset names the same instant differently.
    let s = SkillJobs::default();
    s.apply_update(job("j", "running", "2026-10-02T10:00:00.5Z"));
    assert!(!s.apply_update(job("j", "queued", "2026-10-02T10:00:00Z")), "an older record changes nothing");
    assert_eq!(status_of(&s, "j"), Some(JobStatus::Running));
    assert!(s.apply_update(job("j", "running", "2026-10-02T12:00:01+02:00")), "10:00:01Z is newer");
    assert_eq!(s.jobs(P, S)[0].updated_at, "2026-10-02T12:00:01+02:00");
}

#[test]
fn a_newer_record_wins_and_a_duplicate_changes_nothing() {
    let s = SkillJobs::default();
    assert!(s.apply_update(job("j", "queued", "2026-10-02T10:00:00Z")));
    assert!(!s.apply_update(job("j", "queued", "2026-10-02T10:00:00Z")), "a duplicate is a no-op");
    assert!(s.apply_update(job("j", "running", "2026-10-02T10:00:05Z")));
    assert!(!s.apply_update(job("j", "queued", "2026-10-02T10:00:00Z")), "an older replay is a no-op");
    assert_eq!(status_of(&s, "j"), Some(JobStatus::Running));
    assert_eq!(s.jobs(P, S).len(), 1, "one row per job");
}

#[test]
fn a_terminal_status_never_regresses_to_an_active_one() {
    let s = SkillJobs::default();
    s.apply_update(job("j", "succeeded", "2026-10-02T10:00:05Z"));
    for (status, at) in [("running", "2026-10-02T10:00:04Z"), ("queued", "2026-10-02T10:00:05Z"), ("running", "2026-10-02T10:09:00Z")] {
        assert!(!s.apply_update(job("j", status, at)), "{status} at {at}");
        assert_eq!(status_of(&s, "j"), Some(JobStatus::Succeeded));
    }
    // A later terminal record (a restart's sweep, a result attached) is newer news.
    assert!(s.apply_update(job("j", "abandoned", "2026-10-02T10:10:00Z")));
    assert_eq!(status_of(&s, "j"), Some(JobStatus::Abandoned));
}

/// Every order of application of the same records reaches the same state.
#[test]
fn the_order_of_application_never_changes_the_result() {
    let records = [
        job("a", "queued", "2026-10-02T10:00:00Z"),
        job("a", "running", "2026-10-02T10:00:01Z"),
        job("a", "succeeded", "2026-10-02T10:00:02Z"),
        job("a", "running", "2026-10-02T10:00:09Z"), // an anomaly: never revives a finished job
        job("b", "queued", "2026-10-02T10:00:00Z"),
        job("b", "failed", "2026-10-02T10:00:03.250Z"),
    ];
    let mut want: Option<Vec<SkillJob>> = None;
    let n = records.len();
    let mut idx: Vec<usize> = (0..n).collect();
    // Heap's algorithm: all 720 permutations.
    let mut c = vec![0usize; n];
    let mut check = |order: &[usize]| {
        let s = SkillJobs::default();
        for &i in order {
            s.apply_update(records[i].clone());
        }
        let got = s.jobs(P, S);
        match &want {
            None => want = Some(got),
            Some(w) => assert_eq!(&got, w, "order {order:?}"),
        }
    };
    check(&idx);
    let mut i = 0;
    while i < n {
        if c[i] < i {
            if i % 2 == 0 {
                idx.swap(0, i);
            } else {
                idx.swap(c[i], i);
            }
            check(&idx);
            c[i] += 1;
            i = 0;
        } else {
            c[i] = 0;
            i += 1;
        }
    }
    let want = want.unwrap();
    assert_eq!(want.iter().map(|j| (j.job_id.as_str(), j.status.clone())).collect::<Vec<_>>(),
               vec![("b", JobStatus::Failed), ("a", JobStatus::Succeeded)]);
}

/// The list reply against the updates around it: a job announced after the
/// request was sent survives a snapshot that lacks it; a stale snapshot
/// never regresses a newer update; a job the client knew BEFORE the request
/// that the server no longer lists is gone (the server pruned it).
#[test]
fn a_list_reply_merges_with_the_updates_around_it() {
    let s = SkillJobs::default();
    s.apply_update(job("known-gone", "succeeded", "2026-10-02T09:00:00Z"));
    s.apply_update(job("j", "queued", "2026-10-02T10:00:00Z"));
    let ticket = s.begin_list(P, S);
    assert_eq!(s.list_state(P, S), ListState::Loading);
    // While the request is in flight: j finishes, and a new job is announced.
    s.apply_update(job("j", "succeeded", "2026-10-02T10:00:09Z"));
    s.apply_update(job("fresh", "queued", "2026-10-02T10:00:10Z"));
    // The snapshot predates both.
    let snapshot = vec![job("j", "running", "2026-10-02T10:00:05Z"), job("listed", "failed", "2026-10-02T09:59:00Z")];
    assert!(s.apply_list(ticket, P, S, snapshot));
    assert_eq!(s.list_state(P, S), ListState::Loaded);
    let got: Vec<(String, JobStatus)> = s.jobs(P, S).into_iter().map(|j| (j.job_id, j.status)).collect();
    assert_eq!(
        got,
        vec![
            ("fresh".into(), JobStatus::Queued),
            ("j".into(), JobStatus::Succeeded),
            ("listed".into(), JobStatus::Failed),
        ],
        "newest first; known-gone dropped"
    );
}

#[test]
fn a_list_reply_overtaken_by_a_newer_one_is_ignored() {
    let s = SkillJobs::default();
    let old = s.begin_list(P, S);
    let new = s.begin_list(P, S);
    assert!(s.apply_list(new, P, S, vec![job("x", "running", "2026-10-02T10:00:05Z")]));
    // The older request answers last, with an older world.
    assert!(!s.apply_list(old, P, S, vec![job("y", "queued", "2026-10-02T09:00:00Z")]));
    assert_eq!(s.jobs(P, S).into_iter().map(|j| j.job_id).collect::<Vec<_>>(), ["x"]);
    // A stale failure does not mark a loaded list failed either.
    s.fail_list(old, P, S, "boom".into());
    assert_eq!(s.list_state(P, S), ListState::Loaded);
    let t = s.begin_list(P, S);
    s.fail_list(t, P, S, "profile store unavailable".into());
    assert_eq!(s.list_state(P, S), ListState::Failed("profile store unavailable".into()));
    assert_eq!(s.jobs(P, S).len(), 1, "a failed read keeps what the client knew");
}

#[test]
fn each_profile_and_session_is_kept_apart() {
    let s = SkillJobs::default();
    s.apply_update(job("mine", "running", "2026-10-02T10:00:00Z"));
    let other_session = SkillJob::from_wire(P, "dsflash:other", &rec(P, "dsflash:other", "theirs", "running", "2026-10-02T10:00:00Z")).unwrap();
    let other_profile = SkillJob::from_wire("acme", S, &rec("acme", S, "acme-job", "running", "2026-10-02T10:00:00Z")).unwrap();
    s.apply_update(other_session);
    s.apply_update(other_profile);
    assert_eq!(s.jobs(P, S).into_iter().map(|j| j.job_id).collect::<Vec<_>>(), ["mine"]);
    assert_eq!(s.jobs(P, "dsflash:other").into_iter().map(|j| j.job_id).collect::<Vec<_>>(), ["theirs"]);
    assert_eq!(s.jobs("acme", S).into_iter().map(|j| j.job_id).collect::<Vec<_>>(), ["acme-job"]);
    // A list for one scope never touches another's jobs.
    let t = s.begin_list(P, S);
    s.apply_list(t, P, S, vec![]);
    assert!(s.jobs(P, S).is_empty(), "the server lists none for this scope");
    assert_eq!(s.jobs(P, "dsflash:other").len(), 1);
    assert_eq!(s.jobs("acme", S).len(), 1);
    assert_eq!(s.list_state(P, "dsflash:other"), ListState::NotRequested);
}

#[test]
fn a_scope_keeps_at_most_the_servers_retention() {
    let s = SkillJobs::default();
    for i in 0..300 {
        s.apply_update(job(&format!("j{i:03}"), "succeeded", &format!("2026-10-02T10:{:02}:{:02}Z", i / 60, i % 60)));
    }
    s.apply_update(job("old-but-active", "queued", "2026-10-02T09:00:00Z"));
    let jobs = s.jobs(P, S);
    assert_eq!(jobs.len(), 256, "the server keeps 256 per Session; so does the client");
    assert!(jobs.iter().any(|j| j.job_id == "old-but-active"), "an active job is never pruned");
    assert!(jobs.iter().any(|j| j.job_id == "j299") && !jobs.iter().any(|j| j.job_id == "j000"), "the oldest finished go first");
}
