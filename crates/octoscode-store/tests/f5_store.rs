//! F5 — store-domain tests for the turn/approval/review state added by this
//! lane. No transport, no socket: each domain is exercised directly, so a
//! test never sets up the others.
//!
//! The notification → store hop (fixture wire JSON through the registry) is
//! covered in `crates/octoscode-client/tests/f5_methods.rs`; these tests pin
//! the store API those handlers call.
use octoscode_store::domains::approval::{PendingApproval, StoredScope};
use octoscode_store::domains::review::StartedReview;
use octoscode_store::domains::turn::{DroppedSteer, ThreadRow};
use octoscode_store::Store;

// ------------------------------------------------------------------- turn

#[test]
fn steer_dropped_appends_in_arrival_order() {
    let store = Store::new();
    let turns = &store.domains.turn;
    // A turn can drop twice: drained at interrupt, then again at turn end.
    turns.steer_dropped("s", "t1", vec!["first".into()], "interrupted");
    turns.steer_dropped("s", "t1", vec!["second".into(), "third".into()], "turn_ended");
    let dropped = turns.dropped_steers();
    assert_eq!(dropped.len(), 2);
    assert_eq!(dropped[0], DroppedSteer {
        session_id: "s".into(),
        turn_id: "t1".into(),
        inputs: vec!["first".into()],
        reason: "interrupted".into(),
    });
    // Buffer order preserved, and the UI restores in arrival order.
    assert_eq!(dropped[1].inputs, vec!["second".to_owned(), "third".to_owned()]);
    assert_eq!(dropped[1].reason, "turn_ended");
}

#[test]
fn thread_graph_read_replaces_the_previous_rows() {
    let store = Store::new();
    let turns = &store.domains.turn;
    assert!(turns.threads().is_empty(), "empty until a read lands");

    turns.set_threads(vec![
        ThreadRow {
            thread_id: "th1".into(),
            root_seq: 1,
            status: "completed".into(),
            message_count: 3,
        },
        ThreadRow {
            thread_id: "th2".into(),
            root_seq: 9,
            status: "active".into(),
            message_count: 1,
        },
    ]);
    assert_eq!(turns.threads().len(), 2);
    assert_eq!(turns.threads()[1].root_seq, 9);

    // A fresh read replaces, never accumulates (the graph is a snapshot).
    turns.set_threads(vec![ThreadRow {
        thread_id: "th3".into(),
        root_seq: 20,
        status: "active".into(),
        message_count: 0,
    }]);
    assert_eq!(turns.threads().len(), 1);
    assert_eq!(turns.threads()[0].thread_id, "th3");
}

// --------------------------------------------------------------- approval

#[test]
fn approval_request_is_idempotent_by_id() {
    let store = Store::new();
    let approvals = &store.domains.approval;
    approvals.request("a1", Some("shell".into()));
    approvals.request("a1", Some("bash".into())); // a replay does not duplicate
    let pending = approvals.pending();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].target.as_deref(), Some("shell"), "first request wins");
}

#[test]
fn approval_settle_marks_decided_and_records_auto_resolution() {
    let store = Store::new();
    let approvals = &store.domains.approval;
    approvals.request("a1", Some("shell".into()));
    approvals.request("a2", Some("shell".into()));

    assert!(approvals.settle("a1", false), "the client decided a1");
    assert!(approvals.settle("a2", true), "a policy auto-resolved a2");
    let pending = approvals.pending();
    assert!(pending[0].decided && !pending[0].auto_resolved);
    assert!(pending[1].decided && pending[1].auto_resolved);

    // A decision for an id never seen is ignored, never invented.
    assert!(!approvals.settle("nope", false));
    assert_eq!(approvals.pending().len(), 2);
}

#[test]
fn approval_cancel_is_distinct_from_decided() {
    let store = Store::new();
    let approvals = &store.domains.approval;
    approvals.request("a1", Some("shell".into()));
    assert!(approvals.cancel("a1"));
    let row = &approvals.pending()[0];
    assert!(row.cancelled, "a cancelled approval is marked cancelled");
    assert!(!row.decided, "cancelled is not a decision");
    // An unknown id is not an error.
    assert!(!approvals.cancel("nope"));
}

#[test]
fn approval_push_still_accepts_a_prebuilt_row() {
    // The public `push` constructor (used by the module/tests) still works with
    // the widened struct.
    let store = Store::new();
    store.domains.approval.push(PendingApproval {
        id: "a1".into(),
        target: None,
        decided: false,
        auto_resolved: false,
        cancelled: false,
        preview_id: None,
    });
    assert_eq!(store.domains.approval.pending().len(), 1);
}

#[test]
fn approval_scopes_read_replaces_and_stores_turn_binding() {
    let store = Store::new();
    let approvals = &store.domains.approval;
    assert!(approvals.scopes().is_empty());

    approvals.set_scopes(vec![
        StoredScope {
            scope: "shell".into(),
            scope_match: "prefix".into(),
            decision: "approve".into(),
            turn_id: Some("t1".into()),
        },
        StoredScope {
            scope: "net".into(),
            scope_match: "exact".into(),
            decision: "deny".into(),
            turn_id: None,
        },
    ]);
    let scopes = approvals.scopes();
    assert_eq!(scopes.len(), 2);
    assert_eq!(scopes[0].turn_id.as_deref(), Some("t1"));
    assert_eq!(scopes[1].turn_id, None, "a session-scoped entry has no turn");

    approvals.set_scopes(Vec::new());
    assert!(approvals.scopes().is_empty(), "a fresh read replaces");
}

// ----------------------------------------------------------------- review

#[test]
fn review_note_review_keeps_the_last_accepted_start() {
    let store = Store::new();
    let reviews = &store.domains.review;
    assert!(reviews.last_review().is_none());

    reviews.note_review(StartedReview {
        session_id: "s".into(),
        turn_id: "t1".into(),
        agent_count: 3,
    });
    let last = reviews.last_review().expect("a review was recorded");
    assert_eq!(last.turn_id, "t1");
    assert_eq!(last.agent_count, 3);

    // A later accepted start replaces the previous one.
    reviews.note_review(StartedReview {
        session_id: "s".into(),
        turn_id: "t2".into(),
        agent_count: 1,
    });
    assert_eq!(reviews.last_review().unwrap().turn_id, "t2");
}

#[test]
fn review_note_started_keeps_its_original_contract() {
    // The pre-existing id-only API is unchanged.
    let store = Store::new();
    store.domains.review.note_started("r1".into());
    assert_eq!(store.domains.review.last_started().as_deref(), Some("r1"));
}
