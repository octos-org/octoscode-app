//! [F4] `task` + `tool` store-domain tests: fixture call → store state.
//!
//! Mirror of `crates/octoscode-client/tests/f4_task_tool.rs`'s notification
//! tests but at the store API level, so the state shape is verified without a
//! socket or a decoder. Each notification F4 handles has one test here plus one
//! fixture-decode test in the client crate.
use octoscode_store::domains::task::{Plan, PlanItem, Task, TaskSnapshot};
use octoscode_store::domains::tool::{McpServer, McpStatus, McpSummary};
use octoscode_store::Store;

fn row(id: &str) -> TaskSnapshot {
    TaskSnapshot {
        id: id.into(),
        tool_name: "bash".into(),
        state: "running".into(),
        status: "running".into(),
        title: None,
        role: None,
        source: None,
        summary: None,
        artifact_count: 0,
        output_files: vec![],
        error: None,
        updated_at: None,
    }
}

// ------------------------------------------------------- task/updated store

#[test]
fn task_updated_merges_a_sparse_live_update_onto_the_row() {
    let store = Store::new();
    // A full task/list row first ...
    store.domains.task.upsert_snapshot(TaskSnapshot {
        summary: Some("build the thing".into()),
        output_files: vec!["out.log".into()],
        artifact_count: 2,
        tool_name: "bash".into(),
        ..row("t1")
    });
    // ... then a sparse task/updated: empty tool_name/output_files must NOT
    // erase the row (web: `applyTaskUpdated`, supervision/model.ts:104).
    store.domains.task.upsert_snapshot(TaskSnapshot {
        id: "t1".into(),
        tool_name: String::new(),
        state: "completed".into(),
        status: "done".into(),
        title: None,
        role: None,
        source: None,
        summary: None,
        artifact_count: 0,
        output_files: vec![],
        error: None,
        updated_at: None,
    });

    let got = store.domains.task.snapshot("t1").expect("row kept");
    assert_eq!(got.tool_name, "bash", "tool name survives an empty update");
    assert_eq!(got.output_files, vec!["out.log".to_owned()]);
    assert_eq!(got.artifact_count, 2, "count survives a 0 in the update");
    assert_eq!(got.summary.as_deref(), Some("build the thing"));
    assert_eq!(got.state, "completed", "the transitioned state wins");
    assert_eq!(store.domains.task.snapshot_count(), 1);
}

#[test]
fn task_updated_stores_a_failure_detail_as_the_error() {
    let store = Store::new();
    store.domains.task.upsert_snapshot(TaskSnapshot {
        state: "failed".into(),
        error: Some("boom".into()),
        ..row("t2")
    });
    assert_eq!(
        store.domains.task.snapshot("t2").unwrap().error.as_deref(),
        Some("boom")
    );
}

// -------------------------------------------------- task/output/delta store

#[test]
fn task_output_delta_accumulates_per_task() {
    let store = Store::new();
    store.domains.task.append_output("t1", "Hel");
    store.domains.task.append_output("t1", "lo");
    store.domains.task.append_output("t2", "other");
    assert_eq!(store.domains.task.output("t1"), "Hello");
    assert_eq!(store.domains.task.output("t2"), "other");
    assert_eq!(store.domains.task.output("never"), "");
}

// ------------------------------------------------------- plan/updated store

#[test]
fn plan_updated_replaces_wholesale_and_clears_on_its_authoring_turn() {
    let store = Store::new();
    store.domains.task.set_plan(
        "s1",
        Plan {
            items: vec![PlanItem {
                id: "i1".into(),
                title: "one".into(),
                status: "in_progress".into(),
                priority: Some("P1".into()),
            }],
            title: None,
            updated_at_ms: 10,
            turn_id: Some("turn-a".into()),
        },
    );
    // A second plan REPLACES the first (web: plan.ts:20), never merges.
    store.domains.task.set_plan(
        "s1",
        Plan {
            items: vec![
                PlanItem { id: "i2".into(), title: "two".into(), status: "completed".into(), priority: None },
            ],
            title: Some("Building".into()),
            updated_at_ms: 20,
            turn_id: Some("turn-b".into()),
        },
    );
    let p = store.domains.task.plan("s1").unwrap();
    assert_eq!(p.items.len(), 1, "wholesale replacement drops the old item");
    assert_eq!(p.items[0].id, "i2");
    assert_eq!(p.title.as_deref(), Some("Building"));

    // A terminal for a DIFFERENT turn must not clear it (plan.ts:46).
    assert!(!store.domains.task.clear_plan_for_turn("s1", "turn-a"));
    assert!(store.domains.task.plan("s1").is_some());
    // The authoring turn's terminal clears it (plan.ts:42).
    assert!(store.domains.task.clear_plan_for_turn("s1", "turn-b"));
    assert!(store.domains.task.plan("s1").is_none());
}

#[test]
fn a_plan_without_an_authoring_turn_has_no_removal_key() {
    let store = Store::new();
    store.domains.task.set_plan(
        "s1",
        Plan { items: vec![], title: None, updated_at_ms: 1, turn_id: None },
    );
    assert!(!store.domains.task.clear_plan_for_turn("s1", "any-turn"));
    assert!(store.domains.task.plan("s1").is_some());
}

// --------------------------------------------------------- mcp/status store

#[test]
fn mcp_status_round_trips_into_the_tool_domain() {
    let store = Store::new();
    assert_eq!(store.domains.tool.mcp_server_count(), 0);
    store.domains.tool.set_mcp(McpStatus {
        session_id: "s1".into(),
        profile_id: "p1".into(),
        servers: vec![McpServer {
            id: "filesystem".into(),
            display_name: Some("Filesystem".into()),
            transport: Some("stdio".into()),
            status: "connected".into(),
            tool_count: 3,
            tools: vec!["read".into(), "write".into()],
            error: None,
        }],
        summary: McpSummary { connected: 1, connecting: 0, failed: 0, disabled: 0 },
    });
    assert_eq!(store.domains.tool.mcp_server_count(), 1);
    let m = store.domains.tool.mcp().unwrap();
    assert_eq!(m.servers[0].id, "filesystem");
    assert_eq!(m.summary.connected, 1);
}

// ------------------------------------------------- original stub preserved

#[test]
fn the_original_task_stub_shape_still_works() {
    let store = Store::new();
    store.domains.task.upsert(Task {
        id: "task1".into(),
        title: Some("build".into()),
        state: Some("running".into()),
    });
    assert_eq!(store.domains.task.count(), 1);
    assert_eq!(store.domains.task.list()[0].id, "task1");
}
