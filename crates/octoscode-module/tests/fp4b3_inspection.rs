//! Card #P4b3 — the three inspection READS through the production typed path.
//!
//! Rows 324/325/326 of docs/parity-matrix.csv: `turn/state/get`,
//! `thread/graph/get` and `approval/scopes/list` have typed `Method` impls in
//! the client (domains/turn.rs:124/:171, domains/approval.rs:45) but nothing
//! exercised them through [`Client::call`] — the production path every caller
//! uses (params serialize → `OutboundCommand::Request` → the transport's
//! oneshot → typed decode). This test drives all three against a spawned
//! responder that answers with the RECORDED fixture values (never invented):
//! r23-conversation-a6ea8505.jsonl lines 9–12 and r5-turn-a6ea8505.jsonl
//! lines 8–9, quoted verbatim below.
//!
//! Row 324's "never invented point-in-time semantics" is asserted at the type
//! level: `ThreadGraphGetParams::at: None` must SERIALIZE TO ABSENCE on the
//! wire (the read is at current head, no `at` key at all).

use octos_app_transport::OutboundCommand;
use octoscode_client::domains::approval::ApprovalScopesList;
use octoscode_client::domains::turn::{ThreadGraphGet, TurnStateGet};
use octoscode_client::Client;
use serde_json::{json, Value};

/// The recorded `turn/state/get` RESULT (r23-conversation-a6ea8505.jsonl line 10).
fn recorded_turn_state() -> Value {
    json!({
        "context": {
            "compaction": {"count": 0, "last": null},
            "schema": "octos.context.lifecycle.v1",
            "state": {
                "generation": 17, "item_count": 12, "last_checkpoint_id": null,
                "last_compaction_id": null, "recovery_state": "exact",
                "session_id": "dsflash:main", "thread_id": null,
                "token_estimate": 625, "transcript_hash": "sha256:[hex-redacted]"
            }
        },
        "context_state": {
            "generation": 17, "item_count": 12, "recovery_state": "exact",
            "session_id": "dsflash:main", "token_estimate": 625,
            "transcript_hash": "sha256:[hex-redacted]"
        },
        "running": false,
        "session_id": "dsflash:main",
        "state": "unknown",
        "turn_id": "01920000-0000-7000-8000-00000000023a"
    })
}

/// The recorded `thread/graph/get` RESULT (r23 line 12): three threads,
/// no orphans, the pre-turn cursor.
fn recorded_thread_graph() -> Value {
    json!({
        "cursor": {"seq": 278, "stream": "dsflash:main"},
        "orphans": [],
        "session_id": "dsflash:main",
        "threads": [
            {"message_seqs": [0, 1], "root_seq": 0, "status": "unknown",
             "thread_id": "01920000-0000-7000-8000-00000000023b"},
            {"message_seqs": [2, 3, 4, 5], "root_seq": 2, "status": "unknown",
             "thread_id": "01920000-0000-7000-8000-00000000023c"},
            {"message_seqs": [6, 7], "root_seq": 6, "status": "unknown",
             "thread_id": "01920000-0000-7000-8000-00000000023d"}
        ]
    })
}

/// The recorded `approval/scopes/list` RESULT (r5-turn line 9): the live
/// probe's session had no standing scopes.
fn recorded_scopes() -> Value {
    json!({"scopes": []})
}

/// Spawn the transport's REPLACEMENT: a task that receives the client's
/// `OutboundCommand::Request`s, checks the wire shape the typed params
/// produced, and answers with the recorded result. Returns nothing — the
/// caller only needs the client half.
fn spawn_responder(mut rx: tokio::sync::mpsc::Receiver<OutboundCommand>) {
    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            let OutboundCommand::Request { method, params, reply } = cmd else {
                continue; // tolerate future transport commands
            };
            match method.as_str() {
                "turn/state/get" => {
                    // The typed params serialized to the recorded wire shape.
                    assert_eq!(params["session_id"], json!("dsflash:main"));
                    assert_eq!(
                        params["turn_id"],
                        json!("01920000-0000-7000-8000-00000000023a")
                    );
                    let _ = reply.send(Ok(recorded_turn_state()));
                }
                "thread/graph/get" => {
                    assert_eq!(params["session_id"], json!("dsflash:main"));
                    // Row 324's read-only/current-head clause, at the type
                    // level: `at: None` is skipped — the wire carries NO `at`
                    // key at all.
                    assert!(params.get("at").is_none(), "at=None must serialize to absence, got {params}");
                    let _ = reply.send(Ok(recorded_thread_graph()));
                }
                "approval/scopes/list" => {
                    assert_eq!(params["session_id"], json!("dsflash:main"));
                    let _ = reply.send(Ok(recorded_scopes()));
                }
                other => panic!("the test issued an unexpected method: {other}"),
            }
        }
    });
}

#[tokio::test]
async fn turn_state_get_decodes_the_recorded_result_through_the_typed_path() {
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    spawn_responder(rx);
    let client = Client::new(tx);

    // Params built through the wire types (TurnId is a UUID — the recorded
    // active-turn id parses).
    let params: octos_core::ui_protocol::TurnStateGetParams =
        serde_json::from_value(json!({
            "session_id": "dsflash:main",
            "turn_id": "01920000-0000-7000-8000-00000000023a"
        }))
        .expect("the recorded params decode into the core type");
    let r: octos_core::ui_protocol::TurnStateGetResult =
        client.call::<TurnStateGet>(params).await.expect("typed call");

    assert_eq!(r.state, octos_core::ui_protocol::TurnLifecycleState::Unknown,
        "the recorded state was pre-turn 'unknown'");
    assert_eq!(r.session_id.0, "dsflash:main");
    assert_eq!(r.turn_id.0.to_string(), "01920000-0000-7000-8000-00000000023a",
        "the typed round-trip preserves the captured turn UUID");
    assert_eq!(r.context_state.as_ref().expect("recorded context_state").generation, 17);
}

#[tokio::test]
async fn thread_graph_get_reads_current_head_through_the_typed_path() {
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    spawn_responder(rx);
    let client = Client::new(tx);

    let params: octos_core::ui_protocol::ThreadGraphGetParams =
        serde_json::from_value(json!({"session_id": "dsflash:main"}))
            .expect("params decode (at absent = current head)");
    let r: octos_core::ui_protocol::ThreadGraphGetResult =
        client.call::<ThreadGraphGet>(params).await.expect("typed call");

    assert_eq!(r.cursor.seq, 278);
    assert_eq!(r.cursor.stream, "dsflash:main");
    assert_eq!(r.threads.len(), 3, "the recording listed three threads");
    assert!(r.orphans.is_empty(), "steady state: no orphans recorded");
}

#[tokio::test]
async fn approval_scopes_list_decodes_through_the_typed_path() {
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    spawn_responder(rx);
    let client = Client::new(tx);

    let params: octos_core::ui_protocol::ApprovalScopesListParams =
        serde_json::from_value(json!({"session_id": "dsflash:main"}))
            .expect("params decode");
    let r: octos_core::ui_protocol::ApprovalScopesListResult =
        client.call::<ApprovalScopesList>(params).await.expect("typed call");

    assert!(r.scopes.is_empty(), "the recorded probe session had no standing scopes");
}
