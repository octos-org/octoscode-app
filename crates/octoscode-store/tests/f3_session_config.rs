//! Card #F3 store tests — the session + config projections this lane owns.
//!
//! One test per notification handled (fixture JSON → store state), plus the
//! #1959 goal generation gate, which is the one piece of real logic here.
use octoscode_store::domains::config::{
    LaunchDecision, LaunchResolution, ReplayLoss, SnapshotList, WarningNotice, WorkspaceSnapshot,
};
use octoscode_store::domains::session::OrchestrationSnapshot;
use octoscode_store::Store;

// ---------------------------------------------------------------- session/*

#[test]
fn session_event_stores_the_last_bridged_frame() {
    let store = Store::new();
    assert!(store.domains.session.bridged_event("s1").is_none());

    store.domains.session.note_bridged_event(
        "s1",
        "turn.error",
        serde_json::json!({"code": "boom"}),
    );
    let got = store.domains.session.bridged_event("s1").expect("stored");
    assert_eq!(got.kind, "turn.error");
    assert_eq!(got.payload["code"], "boom");

    // A later frame replaces the earlier one for the same session.
    store
        .domains
        .session
        .note_bridged_event("s1", "turn.completed", serde_json::json!({}));
    assert_eq!(store.domains.session.bridged_event("s1").unwrap().kind, "turn.completed");
    // Other sessions are untouched.
    assert!(store.domains.session.bridged_event("s2").is_none());
}

#[test]
fn session_orchestration_snapshot_round_trips() {
    let store = Store::new();
    assert!(store.domains.session.orchestration("s1").is_none());

    store.domains.session.set_orchestration(
        "s1",
        OrchestrationSnapshot {
            active: true,
            running_agents: 2,
            pending_continuations: 1,
            phase: Some("agents".into()),
        },
    );
    let got = store.domains.session.orchestration("s1").expect("stored");
    assert!(got.active);
    assert_eq!(got.running_agents, 2);
    assert_eq!(got.pending_continuations, 1);
    assert_eq!(got.phase.as_deref(), Some("agents"));
}

#[test]
fn goal_update_then_clear_projects_the_current_goal() {
    let store = Store::new();
    assert!(store.domains.session.goal("s1").is_none());

    assert!(store.domains.session.apply_goal_update(
        "s1",
        1,
        Some(serde_json::json!({"title": "ship F3"})),
    ));
    assert_eq!(
        store.domains.session.goal("s1").unwrap()["title"],
        "ship F3"
    );
    assert_eq!(store.domains.session.goal_generation("s1"), 1);

    assert!(store.domains.session.apply_goal_clear("s1", 2));
    assert!(store.domains.session.goal("s1").is_none());
    assert_eq!(store.domains.session.goal_generation("s1"), 2);
}

#[test]
fn goal_generation_gate_drops_stale_updates_and_clears() {
    let store = Store::new();
    store.domains.session.apply_goal_clear("s1", 5);

    // A stale update (generation 4 <= 5) must NOT resurrect the cleared goal.
    assert!(!store.domains.session.apply_goal_update(
        "s1",
        4,
        Some(serde_json::json!({"title": "stale"})),
    ));
    assert!(store.domains.session.goal("s1").is_none());
    assert_eq!(store.domains.session.goal_generation("s1"), 5);

    // The same generation is also stale (strictly greater is required).
    assert!(!store.domains.session.apply_goal_update("s1", 5, None));

    // A strictly newer update applies.
    assert!(store.domains.session.apply_goal_update(
        "s1",
        6,
        Some(serde_json::json!({"title": "fresh"})),
    ));
    assert_eq!(store.domains.session.goal("s1").unwrap()["title"], "fresh");

    // A stale clear after a newer update is dropped too.
    assert!(!store.domains.session.apply_goal_clear("s1", 5));
    assert!(store.domains.session.goal("s1").is_some());
}

#[test]
fn goal_generation_zero_is_always_applied() {
    // An older backend that does not stamp sends generation 0; per the
    // `SessionGoalUpdatedEvent` doc that means "always apply".
    let store = Store::new();
    assert!(store.domains.session.apply_goal_update(
        "s1",
        9,
        Some(serde_json::json!({"title": "nine"})),
    ));
    assert!(store.domains.session.apply_goal_update(
        "s1",
        0,
        Some(serde_json::json!({"title": "unstamped"})),
    ));
    assert_eq!(store.domains.session.goal("s1").unwrap()["title"], "unstamped");
    // The gate keeps the highest generation seen.
    assert_eq!(store.domains.session.goal_generation("s1"), 9);
}

// ----------------------------------------------------------------- config/*

#[test]
fn launch_resolution_round_trips() {
    let store = Store::new();
    assert!(store.domains.config.launch().is_none());

    store.domains.config.set_launch(LaunchResolution {
        decision: LaunchDecision::CrossProfile,
        resolved_profile: Some("coding".into()),
        existing_profiles: vec!["other".into()],
    });
    let got = store.domains.config.launch().expect("stored");
    assert_eq!(got.decision, LaunchDecision::CrossProfile);
    assert_eq!(got.decision.as_str(), "cross_profile");
    assert_eq!(got.resolved_profile.as_deref(), Some("coding"));
    assert_eq!(got.existing_profiles, vec!["other".to_string()]);
}

#[test]
fn snapshot_list_projection_round_trips() {
    let store = Store::new();
    assert!(!store.domains.config.snapshots().available);

    store.domains.config.set_snapshots(SnapshotList {
        enabled: true,
        available: true,
        snapshots: vec![WorkspaceSnapshot {
            id: "snap-1".into(),
            label: "before edit".into(),
            timestamp_unix: 1_700_000_000,
        }],
    });
    let got = store.domains.config.snapshots();
    assert!(got.enabled && got.available);
    assert_eq!(got.snapshots.len(), 1);
    assert_eq!(got.snapshots[0].id, "snap-1");
}

#[test]
fn server_shutdown_ack_is_recorded() {
    let store = Store::new();
    assert!(!store.domains.config.stopping());
    store.domains.config.set_stopping(true);
    assert!(store.domains.config.stopping());
}

#[test]
fn warning_is_stored_per_session() {
    let store = Store::new();
    assert!(store.domains.config.warning("s1").is_none());

    store.domains.config.note_warning(
        "s1",
        WarningNotice {
            code: "slow_tool".into(),
            message: "the tool is slow".into(),
        },
    );
    let got = store.domains.config.warning("s1").expect("stored");
    assert_eq!(got.code, "slow_tool");
    assert_eq!(got.message, "the tool is slow");
    assert!(store.domains.config.warning("s2").is_none());
}

#[test]
fn replay_loss_is_stored_with_its_cursor() {
    let store = Store::new();
    assert!(store.domains.config.replay_loss("s1").is_none());

    store.domains.config.note_replay_loss(
        "s1",
        ReplayLoss {
            session_id: "s1".into(),
            dropped_count: 3,
            last_durable_cursor: Some(serde_json::json!({"stream": "main", "seq": 42})),
        },
    );
    let got = store.domains.config.replay_loss("s1").expect("stored");
    assert_eq!(got.dropped_count, 3);
    assert_eq!(got.last_durable_cursor.unwrap()["seq"], 42);
}
