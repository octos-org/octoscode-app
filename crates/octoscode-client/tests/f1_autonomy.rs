//! F1 — fake-WS round-trips for the autonomy domain's 21 request methods.
//!
//! Each test drives the production `Client::call` path against a fake
//! transport that (a) asserts the wire `method` and the exact **params JSON the
//! web sends** and (b) replies with the server's real result shape (from the
//! emitters in `crates/octos-cli/src/autonomy/agent_orchestrator.rs`). The
//! error tests assert the typed error carries the method name.
use std::sync::{Arc, Mutex};

use octos_app_transport::OutboundCommand;
use octos_core::ui_protocol::RpcError;
use octoscode_client::domains::autonomy as au;
use octoscode_client::{Client, ClientError};
use serde_json::{json, Value};
use tokio::sync::mpsc;

/// A fake transport: records every `(method, params)` it is asked to send and
/// answers from a scripted reply. This is the same command/oneshot contract
/// the real WS transport implements, so the client path under test is real.
fn fake_transport(
    reply: Result<Value, RpcError>,
) -> (mpsc::Sender<OutboundCommand>, Arc<Mutex<Vec<(String, Value)>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let captured = seen.clone();
    let (tx, mut rx) = mpsc::channel::<OutboundCommand>(8);
    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            if let OutboundCommand::Request { method, params, reply: r } = cmd {
                captured.lock().unwrap().push((method, params));
                let _ = r.send(reply.clone());
            }
        }
    });
    (tx, seen)
}

fn ok(reply: Value) -> Result<Value, RpcError> {
    Ok(reply)
}

fn assert_sent(seen: &Arc<Mutex<Vec<(String, Value)>>>, method: &str) -> Value {
    let v = seen.lock().unwrap().clone();
    assert_eq!(v.len(), 1, "exactly one frame sent");
    assert_eq!(v[0].0, method, "wire method name");
    v[0].1.clone()
}

// ---------------------------------------------------------------- fixtures

/// `autonomy_agent_json` (agent_orchestrator.rs:20661).
fn agent_fixture() -> Value {
    json!({
        "agent_id": "a-1", "session_id": "octoscode:main", "path": "/agents/a-1",
        "role": "reviewer", "nickname": "Edison", "backend_kind": "claude",
        "status": "running", "profile_id": "octoscode",
        "created_at_ms": 1_700_000_000_000i64, "updated_at_ms": 1_700_000_001_000i64,
        "artifact_count": 1,
        "artifacts": [{"id": "art-1", "title": "notes", "kind": "markdown", "status": "ready"}]
    })
}

/// `autonomy_loop_json` (:20924).
fn loop_fixture() -> Value {
    json!({
        "loop_id": "loop-1", "session_id": "octoscode:main", "profile_id": "octoscode",
        "prompt": "check the build", "mode": "fixed_interval", "interval_seconds": 300,
        "status": "active", "next_run_at_ms": 1_700_000_300_000i64,
        "last_run_at_ms": null, "expires_at_ms": 1_700_100_000_000i64,
        "created_at_ms": 1_700_000_000_000i64, "updated_at_ms": 1_700_000_000_000i64
    })
}

/// `autonomy_monitor_json` (:20022).
fn monitor_fixture() -> Value {
    json!({
        "monitor_id": "mon-1", "session_id": "octoscode:main", "profile_id": "octoscode",
        "name": "disk", "argv": ["df", "-h"], "filter_regex": "9[0-9]%",
        "mode": "poll", "interval_seconds": 60, "batch_ms": 100,
        "max_events_per_hour": 60, "persistent": false, "status": "active",
        "fires_used": 0, "created_at_ms": 1_700_000_000_000i64,
        "updated_at_ms": 1_700_000_000_000i64
    })
}

/// `autonomy_goal_json` (:20910).
fn goal_fixture() -> Value {
    json!({
        "goal_id": "goal-1", "objective": "ship F1", "status": "active",
        "token_budget": 200000, "tokens_used": 1234, "time_used_seconds": 42,
        "created_at_ms": 1_700_000_000_000i64, "updated_at_ms": 1_700_000_001_000i64
    })
}

// ---------------------------------------------------------------- agent/*

#[tokio::test]
async fn agent_list_round_trips() {
    let (tx, seen) = fake_transport(ok(json!({
        "session_id": "octoscode:main", "profile_id": "octoscode", "agents": [agent_fixture()]
    })));
    let client = Client::new(tx);
    let r = client
        .call::<au::AgentList>(au::AutonomyListParams {
            session_id: Some("octoscode:main".into()),
            profile_id: Some("octoscode".into()),
        })
        .await
        .expect("agent/list");
    let p = assert_sent(&seen, "agent/list");
    assert_eq!(p["session_id"], "octoscode:main");
    assert_eq!(r.agents.len(), 1);
    assert_eq!(r.agents[0].nickname, "Edison");
    assert_eq!(r.agents[0].artifact_count, 1);
}

#[tokio::test]
async fn agent_status_read_round_trips() {
    let (tx, seen) = fake_transport(ok(json!({
        "session_id": "octoscode:main", "agent": agent_fixture()
    })));
    let client = Client::new(tx);
    let r = client
        .call::<au::AgentStatusRead>(au::AgentParams {
            agent_id: "a-1".into(),
            session_id: None,
            profile_id: None,
        })
        .await
        .expect("agent/status/read");
    let p = assert_sent(&seen, "agent/status/read");
    assert_eq!(p["agent_id"], "a-1");
    // Optional fields the caller left None must NOT be sent (web omits them).
    assert!(p.get("session_id").is_none(), "None session_id omitted");
    assert_eq!(r.agent.role, "reviewer");
}

#[tokio::test]
async fn agent_output_read_round_trips() {
    let (tx, seen) = fake_transport(ok(json!({
        "agent_id": "a-1", "session_id": "octoscode:main", "source": "stdout",
        "text": "hello", "cursor": {"offset": 0}, "next_cursor": {"offset": 5},
        "has_more": false, "complete": true
    })));
    let client = Client::new(tx);
    let r = client
        .call::<au::AgentOutputRead>(au::AgentOutputReadParams {
            agent_id: "a-1".into(),
            session_id: None,
            profile_id: None,
            cursor: Some(au::AgentOutputCursor { offset: 0 }),
            limit: Some(5),
        })
        .await
        .expect("agent/output/read");
    let p = assert_sent(&seen, "agent/output/read");
    assert_eq!(p["cursor"]["offset"], 0);
    assert_eq!(p["limit"], 5);
    assert_eq!(r.text, "hello");
    assert_eq!(r.next_cursor.unwrap().offset, 5);
}

#[tokio::test]
async fn agent_artifact_list_round_trips() {
    let (tx, seen) = fake_transport(ok(json!({
        "session_id": "octoscode:main", "agent_id": "a-1",
        "artifacts": [{"id": "art-1", "title": "notes", "kind": "markdown", "status": "ready"}]
    })));
    let client = Client::new(tx);
    let r = client
        .call::<au::AgentArtifactList>(au::AgentParams {
            agent_id: "a-1".into(),
            session_id: Some("octoscode:main".into()),
            profile_id: None,
        })
        .await
        .expect("agent/artifact/list");
    assert_sent(&seen, "agent/artifact/list");
    assert_eq!(r.artifacts.len(), 1);
    assert_eq!(r.artifacts[0].id, "art-1");
}

#[tokio::test]
async fn agent_artifact_read_round_trips() {
    let (tx, seen) = fake_transport(ok(json!({
        "session_id": "octoscode:main", "agent_id": "a-1",
        "artifact": {"id": "art-1", "title": "notes", "kind": "markdown", "status": "ready"},
        "content": "# notes"
    })));
    let client = Client::new(tx);
    let r = client
        .call::<au::AgentArtifactRead>(au::AgentArtifactReadParams {
            agent_id: "a-1".into(),
            artifact_id: Some("art-1".into()),
            path: None,
            session_id: None,
            profile_id: None,
        })
        .await
        .expect("agent/artifact/read");
    let p = assert_sent(&seen, "agent/artifact/read");
    // Never send two selectors (web `AgentArtifactSelector`, autonomy.ts:1319).
    assert_eq!(p["artifact_id"], "art-1");
    assert!(p.get("path").is_none());
    assert_eq!(r.content.as_deref(), Some("# notes"));
}

#[tokio::test]
async fn agent_interrupt_and_close_round_trip() {
    for (ty, method, status) in [
        ("interrupt", "agent/interrupt", "interrupted"),
        ("close", "agent/close", "closed"),
    ] {
        let (tx, seen) = fake_transport(ok(json!({
            "session_id": "octoscode:main", "agent_id": "a-1", "status": status,
            "ok": true, "interrupted": status == "interrupted", "closed": status == "closed",
            "already_terminal": false
        })));
        let client = Client::new(tx);
        let params = au::AgentParams {
            agent_id: "a-1".into(),
            session_id: None,
            profile_id: None,
        };
        let r = match ty {
            "interrupt" => client.call::<au::AgentInterrupt>(params).await,
            _ => client.call::<au::AgentClose>(params).await,
        }
        .unwrap_or_else(|e| panic!("agent/{ty}: {e}"));
        assert_sent(&seen, method);
        assert!(r.ok);
        assert_eq!(r.status, status);
    }
}

// ---------------------------------------------------------------- loop/*

#[tokio::test]
async fn loop_create_round_trips() {
    let (tx, seen) = fake_transport(ok(json!({
        "session_id": "octoscode:main", "profile_id": "octoscode", "loop_id": "loop-1",
        "loop": loop_fixture(), "ok": true, "status": "active", "created": true,
        "fire": {"queued": true, "duplicate": false, "continuation_id": 7}
    })));
    let client = Client::new(tx);
    let r = client
        .call::<au::LoopCreate>(au::LoopCreateParams {
            session_id: "octoscode:main".into(),
            prompt: Some("check the build".into()),
            command: None,
            interval_seconds: Some(300),
            mode: Some("fixed_interval".into()),
        })
        .await
        .expect("loop/create");
    let p = assert_sent(&seen, "loop/create");
    assert_eq!(p["prompt"], "check the build");
    assert_eq!(p["interval_seconds"], 300);
    assert!(p.get("command").is_none(), "unused shorthand omitted");
    assert!(r.created);
    assert_eq!(r.loop_id, "loop-1");
    assert_eq!(r.fire.unwrap().continuation_id, Some(7));
}

#[tokio::test]
async fn loop_list_round_trips_with_null_session() {
    // An unscoped listing legitimately returns session_id: null.
    let (tx, seen) = fake_transport(ok(json!({
        "session_id": null, "profile_id": "octoscode", "loops": [loop_fixture()]
    })));
    let client = Client::new(tx);
    let r = client
        .call::<au::LoopList>(au::AutonomyListParams::default())
        .await
        .expect("loop/list");
    let p = assert_sent(&seen, "loop/list");
    assert!(p.get("session_id").is_none(), "empty params send no keys");
    assert_eq!(r.session_id, None);
    assert_eq!(r.loops.len(), 1);
    assert_eq!(r.loops[0].mode, "fixed_interval");
}

#[tokio::test]
async fn loop_control_methods_round_trip() {
    for (name, method, result) in [
        (
            "pause",
            "loop/pause",
            json!({"session_id":"octoscode:main","loop_id":"loop-1","loop":loop_fixture(),"ok":true,"status":"paused"}),
        ),
        (
            "resume",
            "loop/resume",
            json!({"session_id":"octoscode:main","loop_id":"loop-1","loop":loop_fixture(),"ok":true,"status":"active"}),
        ),
    ] {
        let (tx, seen) = fake_transport(ok(result));
        let client = Client::new(tx);
        let params = au::LoopControlParams::new("loop-1");
        let r = match name {
            "pause" => client.call::<au::LoopPause>(params).await,
            _ => client.call::<au::LoopResume>(params).await,
        }
        .unwrap_or_else(|e| panic!("loop/{name}: {e}"));
        let p = assert_sent(&seen, method);
        assert_eq!(p["loop_id"], "loop-1");
        assert!(r.ok);
    }
}

#[tokio::test]
async fn loop_delete_round_trips() {
    let (tx, seen) = fake_transport(ok(json!({
        "session_id": "octoscode:main", "loop_id": "loop-1", "loop": loop_fixture(),
        "ok": true, "status": "deleted", "deleted": true, "reaped_cron_job_ids": ["cron-9"]
    })));
    let client = Client::new(tx);
    let r = client
        .call::<au::LoopDelete>(au::LoopControlParams::new("loop-1"))
        .await
        .expect("loop/delete");
    assert_sent(&seen, "loop/delete");
    assert!(r.deleted);
    assert_eq!(r.reaped_cron_job_ids, vec!["cron-9".to_owned()]);
}

#[tokio::test]
async fn loop_fire_now_round_trips() {
    let (tx, seen) = fake_transport(ok(json!({
        "session_id": "octoscode:main", "profile_id": "octoscode", "loop_id": "loop-1",
        "loop": loop_fixture(), "ok": true, "status": "active",
        "fire": {"queued": false, "reason": "cooldown", "message": "too soon"}
    })));
    let client = Client::new(tx);
    let r = client
        .call::<au::LoopFireNow>(au::LoopControlParams::new("loop-1"))
        .await
        .expect("loop/fire_now");
    assert_sent(&seen, "loop/fire_now");
    let fire = r.fire.expect("fire outcome");
    assert_eq!(fire.queued, false);
    assert_eq!(fire.reason.as_deref(), Some("cooldown"));
}

// ---------------------------------------------------------------- monitor/*

#[tokio::test]
async fn monitor_create_round_trips() {
    let (tx, seen) = fake_transport(ok(json!({
        "session_id": "octoscode:main", "profile_id": "octoscode", "monitor_id": "mon-1",
        "monitor": monitor_fixture(), "ok": true, "status": "active", "created": true
    })));
    let client = Client::new(tx);
    let r = client
        .call::<au::MonitorCreate>(au::MonitorCreateParams {
            session_id: "octoscode:main".into(),
            name: "disk".into(),
            argv: vec!["df".into(), "-h".into()],
            filter_regex: Some("9[0-9]%".into()),
            mode: Some("poll".into()),
            interval_seconds: Some(60),
            batch_ms: None,
            timeout_secs: None,
            persistent: None,
            max_events_per_hour: None,
            goal_id: None,
        })
        .await
        .expect("monitor/create");
    let p = assert_sent(&seen, "monitor/create");
    assert_eq!(p["name"], "disk");
    assert_eq!(p["argv"], json!(["df", "-h"]));
    assert_eq!(p["mode"], "poll");
    // Optional fields left None are omitted (the server applies its defaults).
    assert!(p.get("batch_ms").is_none());
    assert!(r.created);
    assert_eq!(r.monitor.monitor_id, "mon-1");
}

#[tokio::test]
async fn monitor_list_round_trips() {
    let (tx, seen) = fake_transport(ok(json!({
        "session_id": "octoscode:main", "profile_id": "octoscode", "monitors": [monitor_fixture()]
    })));
    let client = Client::new(tx);
    let r = client
        .call::<au::MonitorList>(au::AutonomyListParams {
            session_id: Some("octoscode:main".into()),
            profile_id: None,
        })
        .await
        .expect("monitor/list");
    assert_sent(&seen, "monitor/list");
    assert_eq!(r.monitors.len(), 1);
    assert_eq!(r.monitors[0].argv, vec!["df".to_owned(), "-h".to_owned()]);
}

#[tokio::test]
async fn monitor_control_methods_round_trip() {
    for (name, method, deleted) in [
        ("pause", "monitor/pause", false),
        ("resume", "monitor/resume", false),
        ("delete", "monitor/delete", true),
    ] {
        let (tx, seen) = fake_transport(ok(json!({
            "session_id": "octoscode:main", "profile_id": "octoscode", "monitor_id": "mon-1",
            "monitor": monitor_fixture(), "ok": true, "status": "active", "deleted": deleted
        })));
        let client = Client::new(tx);
        let params = au::MonitorControlParams::new("mon-1");
        let r = match name {
            "pause" => client.call::<au::MonitorPause>(params).await,
            "resume" => client.call::<au::MonitorResume>(params).await,
            _ => client.call::<au::MonitorDelete>(params).await,
        }
        .unwrap_or_else(|e| panic!("monitor/{name}: {e}"));
        let p = assert_sent(&seen, method);
        assert_eq!(p["monitor_id"], "mon-1");
        assert_eq!(r.deleted, deleted);
    }
}

// ------------------------------------------------------------- session/goal/*

#[tokio::test]
async fn goal_get_round_trips() {
    let (tx, seen) = fake_transport(ok(json!({
        "session_id": "octoscode:main", "profile_id": "octoscode", "goal": goal_fixture()
    })));
    let client = Client::new(tx);
    let r = client
        .call::<au::GoalGet>(au::GoalSessionParams {
            session_id: "octoscode:main".into(),
            profile_id: None,
        })
        .await
        .expect("session/goal/get");
    let p = assert_sent(&seen, "session/goal/get");
    assert_eq!(p["session_id"], "octoscode:main");
    assert_eq!(r.goal.unwrap().token_budget, 200000);
}

#[tokio::test]
async fn goal_get_accepts_a_null_goal() {
    let (tx, _seen) = fake_transport(ok(json!({
        "session_id": "octoscode:main", "profile_id": "octoscode", "goal": null
    })));
    let client = Client::new(tx);
    let r = client
        .call::<au::GoalGet>(au::GoalSessionParams {
            session_id: "octoscode:main".into(),
            profile_id: None,
        })
        .await
        .expect("session/goal/get");
    assert!(r.goal.is_none());
}

#[tokio::test]
async fn goal_set_round_trips_and_omits_an_absent_budget() {
    let (tx, seen) = fake_transport(ok(json!({
        "session_id": "octoscode:main", "profile_id": "octoscode", "goal": goal_fixture(),
        "generation": 3, "transition_actor": "user"
    })));
    let client = Client::new(tx);
    let r = client
        .call::<au::GoalSet>(au::GoalSetParams {
            session_id: "octoscode:main".into(),
            objective: "ship F1".into(),
            profile_id: None,
            status: None,
            // NO budget: the web never invents one (autonomy.ts:283).
            token_budget: None,
            transition_actor: Some("user".into()),
        })
        .await
        .expect("session/goal/set");
    let p = assert_sent(&seen, "session/goal/set");
    assert_eq!(p["objective"], "ship F1");
    assert!(p.get("token_budget").is_none(), "an absent budget must not be defaulted");
    assert_eq!(r.generation, 3);
    assert_eq!(r.transition_actor, "user");
}

#[tokio::test]
async fn goal_clear_round_trips() {
    let (tx, seen) = fake_transport(ok(json!({
        "session_id": "octoscode:main", "profile_id": "octoscode", "cleared": true,
        "goal": null, "generation": 4, "transition_actor": "user"
    })));
    let client = Client::new(tx);
    let r = client
        .call::<au::GoalClear>(au::GoalSessionParams {
            session_id: "octoscode:main".into(),
            profile_id: None,
        })
        .await
        .expect("session/goal/clear");
    assert_sent(&seen, "session/goal/clear");
    assert!(r.cleared);
    assert!(r.goal.is_none());
}

// ------------------------------------------------------------- error paths

#[tokio::test]
async fn an_rpc_error_carries_the_method_name() {
    let (tx, _seen) = fake_transport(Err(RpcError {
        code: -32601,
        message: "method not found".into(),
        data: None,
    }));
    let client = Client::new(tx);
    match client
        .call::<au::LoopList>(au::AutonomyListParams::default())
        .await
    {
        Err(ClientError::Rpc { method, error }) => {
            assert_eq!(method, "loop/list");
            assert_eq!(error.code, -32601);
        }
        other => panic!("expected an RPC error, got {other:?}"),
    }
}

#[tokio::test]
async fn every_request_method_is_declared_and_named() {
    // The domain's audit table must name all 21 requests with unique names
    // (guards against a copy-paste duplicating a wire method).
    assert_eq!(au::REQUEST_METHODS.len(), 21);
    let mut seen: Vec<&str> = au::REQUEST_METHODS.to_vec();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), 21, "wire method names must be unique");

    // And each is really wired to a `Method` impl whose Params serialize.
    fn shape<M: octoscode_client::Method>(params: M::Params) -> (String, Value) {
        (M::NAME.to_owned(), serde_json::to_value(params).expect("params serialize"))
    }
    let (name, v) = shape::<au::GoalGet>(au::GoalSessionParams {
        session_id: "s".into(),
        profile_id: None,
    });
    assert_eq!(name, "session/goal/get");
    assert_eq!(v["session_id"], "s");
}
