//! F1 — store tests: the autonomy domain's records and the #1959 goal ordering.
//!
//! No transport, no UI: each test drives the store's own API with the shape the
//! notification handlers write.
use octoscode_store::domains::autonomy::{
    AgentRecord, AutonomyEntity, GoalRecord, LoopRecord, MonitorRecord,
};
use octoscode_store::Store;

fn agent(id: &str, status: &str) -> AgentRecord {
    AgentRecord {
        agent_id: id.into(),
        session_id: "octoscode:main".into(),
        profile_id: "octoscode".into(),
        path: format!("/agents/{id}"),
        role: "reviewer".into(),
        nickname: "Edison".into(),
        backend_kind: "claude".into(),
        status: status.into(),
        title: None,
        parent_agent_id: None,
        task_id: None,
        artifact_count: 0,
        output_tail: None,
        updated_at_ms: 1,
    }
}

fn loop_record(id: &str, status: &str) -> LoopRecord {
    LoopRecord {
        loop_id: id.into(),
        session_id: "octoscode:main".into(),
        profile_id: Some("octoscode".into()),
        prompt: "check the build".into(),
        mode: "fixed_interval".into(),
        status: status.into(),
        interval_seconds: Some(300),
        next_run_at_ms: None,
        expires_at_ms: 0,
        updated_at_ms: 1,
        fires: 0,
    }
}

fn monitor(id: &str, status: &str) -> MonitorRecord {
    MonitorRecord {
        monitor_id: id.into(),
        session_id: "octoscode:main".into(),
        profile_id: Some("octoscode".into()),
        name: "disk".into(),
        mode: "poll".into(),
        status: status.into(),
        pause_reason: None,
        fires_used: 0,
        last_fired_at_ms: None,
        expires_at_ms: None,
        updated_at_ms: 1,
    }
}

fn goal(id: &str, status: &str) -> GoalRecord {
    GoalRecord {
        goal_id: id.into(),
        objective: "ship F1".into(),
        status: status.into(),
        token_budget: 200_000,
        tokens_used: 10,
        created_at_ms: 1,
        updated_at_ms: 1,
    }
}

// ---------------------------------------------------------------- agents

#[test]
fn agent_list_then_upsert_tracks_newest_state() {
    let store = Store::new();
    store.domains.autonomy.set_agents(vec![agent("a-1", "running")]);
    assert_eq!(store.domains.autonomy.agent_count(), 1);
    assert_eq!(store.domains.autonomy.agent("a-1").unwrap().status, "running");

    // `agent/updated` upserts the same id in place (no duplicate row).
    store.domains.autonomy.upsert_agent(agent("a-1", "completed"));
    assert_eq!(store.domains.autonomy.agent_count(), 1);
    assert_eq!(store.domains.autonomy.agent("a-1").unwrap().status, "completed");

    store.domains.autonomy.upsert_agent(agent("a-2", "running"));
    assert_eq!(store.domains.autonomy.agents().len(), 2);
}

// ---------------------------------------------------------------- loops

#[test]
fn loop_list_upsert_and_delete() {
    let store = Store::new();
    store.domains.autonomy.set_loops(vec![loop_record("loop-1", "active")]);
    assert_eq!(store.domains.autonomy.loop_count(), 1);

    store
        .domains
        .autonomy
        .upsert_loop(loop_record("loop-1", "paused"));
    assert_eq!(store.domains.autonomy.loop_count(), 1);
    assert_eq!(store.domains.autonomy.loop_record("loop-1").unwrap().status, "paused");

    // `loop/updated` with `deleted: true` removes the row.
    assert!(store.domains.autonomy.remove_loop("loop-1"));
    assert_eq!(store.domains.autonomy.loop_count(), 0);
    assert!(!store.domains.autonomy.remove_loop("loop-1"), "already gone");
}

#[test]
fn loop_fired_bumps_and_preserves_the_counter_across_updates() {
    let store = Store::new();
    store.domains.autonomy.set_loops(vec![loop_record("loop-1", "active")]);

    assert_eq!(store.domains.autonomy.note_loop_fired("loop-1"), 1);
    assert_eq!(store.domains.autonomy.note_loop_fired("loop-1"), 2);

    // An `loop/updated` (fresh record) must NOT reset the local fire counter.
    store
        .domains
        .autonomy
        .upsert_loop(loop_record("loop-1", "active"));
    assert_eq!(store.domains.autonomy.loop_record("loop-1").unwrap().fires, 2);

    // A fire for an unknown loop still creates a placeholder row (never drops).
    assert_eq!(store.domains.autonomy.note_loop_fired("loop-x"), 1);
    assert_eq!(store.domains.autonomy.loop_count(), 2);
}

// ---------------------------------------------------------------- monitors

#[test]
fn monitor_lifecycle_fired_and_expired() {
    let store = Store::new();
    store.domains.autonomy.set_monitors(vec![monitor("mon-1", "active")]);
    assert_eq!(store.domains.autonomy.monitor_count(), 1);

    assert_eq!(
        store.domains.autonomy.note_monitor_fired("mon-1", Some(1_700_000_000_000)),
        Some(1)
    );
    let m = store.domains.autonomy.monitor("mon-1").unwrap();
    assert_eq!(m.fires_used, 1);
    assert_eq!(m.last_fired_at_ms, Some(1_700_000_000_000));

    // A fire for an unknown monitor is a no-op, not a fabricated row.
    assert_eq!(store.domains.autonomy.note_monitor_fired("nope", None), None);
    assert_eq!(store.domains.autonomy.monitor_count(), 1);

    // `monitor/expired` flips the status and records the reason.
    assert!(store
        .domains
        .autonomy
        .mark_monitor_expired("mon-1", Some("timeout".into())));
    let m = store.domains.autonomy.monitor("mon-1").unwrap();
    assert_eq!(m.status, "expired");
    assert_eq!(m.pause_reason.as_deref(), Some("timeout"));

    assert!(!store.domains.autonomy.mark_monitor_expired("nope", None));
    assert!(store.domains.autonomy.remove_monitor("mon-1"));
    assert_eq!(store.domains.autonomy.monitor_count(), 0);
}

// ------------------------------------------------------- goals (#1959)

#[test]
fn goal_update_then_clear_apply_in_generation_order() {
    let store = Store::new();
    let a = &store.domains.autonomy;

    // generation 0 = "old backend, no stamp" -> always applied.
    assert!(a.apply_goal_update("s", goal("g1", "active"), Some("user".into()), 0));
    assert_eq!(a.goal("s").unwrap().goal_id, "g1");

    assert!(a.apply_goal_update("s", goal("g1", "paused"), Some("user".into()), 2));
    assert_eq!(a.goal("s").unwrap().status, "paused");

    assert!(a.apply_goal_clear("s", Some("user".into()), 3));
    assert!(a.goal("s").is_none());
    assert_eq!(a.goal_generation("s"), 3);
}

#[test]
fn a_stale_goal_update_cannot_resurrect_a_cleared_goal() {
    let store = Store::new();
    let a = &store.domains.autonomy;

    assert!(a.apply_goal_update("s", goal("g1", "active"), Some("user".into()), 5));
    assert!(a.apply_goal_clear("s", Some("user".into()), 6));

    // A late-arriving update stamped BELOW the clear is dropped (#1959).
    assert!(
        !a.apply_goal_update("s", goal("g1", "active"), Some("model".into()), 5),
        "a stale update must be dropped, not applied"
    );
    assert!(a.goal("s").is_none(), "the clear must survive");
    assert_eq!(a.goal_generation("s"), 6);

    // An update at the SAME generation is also stale (strictly-greater rule).
    assert!(!a.apply_goal_update("s", goal("g1", "active"), None, 6));

    // A newer update wins.
    assert!(a.apply_goal_update("s", goal("g2", "active"), Some("user".into()), 7));
    assert_eq!(a.goal("s").unwrap().goal_id, "g2");
}

#[test]
fn a_stale_goal_clear_is_dropped_too() {
    let store = Store::new();
    let a = &store.domains.autonomy;

    assert!(a.apply_goal_update("s", goal("g1", "active"), None, 9));
    // A clear stamped below the latest update must not erase it.
    assert!(!a.apply_goal_clear("s", None, 8));
    assert_eq!(a.goal("s").unwrap().goal_id, "g1");
}

#[test]
fn goals_are_per_session() {
    let store = Store::new();
    let a = &store.domains.autonomy;
    a.apply_goal_update("s1", goal("g1", "active"), None, 1);
    a.apply_goal_update("s2", goal("g2", "paused"), None, 1);

    assert_eq!(a.goal("s1").unwrap().goal_id, "g1");
    assert_eq!(a.goal("s2").unwrap().goal_id, "g2");
    assert_eq!(a.goal_sessions(), vec!["s1".to_owned(), "s2".to_owned()]);

    // A clear on s1 leaves s2 untouched.
    a.apply_goal_clear("s1", None, 2);
    assert!(a.goal("s1").is_none());
    assert_eq!(a.goal("s2").unwrap().goal_id, "g2");
}

// ---------------------------------------------------------------- legacy

#[test]
fn the_generic_entity_map_still_works() {
    // #10's shape is kept so its test and any early caller keep working.
    let store = Store::new();
    store.domains.autonomy.upsert(AutonomyEntity {
        id: "loop1".into(),
        kind: "loop".into(),
        detail: None,
    });
    assert_eq!(store.domains.autonomy.of_kind("loop").len(), 1);
    assert_eq!(store.domains.autonomy.count(), 1);
}
