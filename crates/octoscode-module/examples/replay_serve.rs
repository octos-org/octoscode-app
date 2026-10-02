//! Card #15 §5 / card #19 — a **replay server**: committed real fixtures served
//! on a port so the real hidden app can mount its cards against recorded
//! traffic. **No model is called.**
//!
//! Card #19 extends it with **named scenarios**: `--scenario <name>` chooses
//! which committed recording in `crates/octoscode-client/tests/fixtures/` is
//! served, so the walk-runner can drive one capability at a time:
//!
//! ```sh
//! cargo run -p octoscode-module --example replay_serve -- 8380 --scenario approval
//! # then point the app at it (its own port block, 8370-8379):
//! OCTOS_BASE_URL=http://127.0.0.1:8380 OCTOS_PROFILE_ID=dsflash \
//!   MAKEPAD_HIDE_WINDOWS=1 MAKEPAD_WM_TEST_APP=octoscode \
//!   <octosense> --module octoscode --remote 8370
//! ```
//!
//! Scenarios (fixture → what it exercises):
//!
//! | name | fixture | exercises |
//! |---|---|---|
//! | `conversation` (default) | `live-gate-a6ea8505` | one full streamed turn |
//! | `interrupt` | `live-gate-a6ea8505` | the 2nd turn (terminal `interrupted`) |
//! | `live-turn` | `live-turn-a6ea8505` | the minimal captured turn |
//! | `approval` | `r5-turn-a6ea8505` | `approval/requested` + decisions |
//! | `autonomy` | `r1-autonomy-a6ea8505` | monitors, loops, session goals |
//! | `task` | `r4-task-a6ea8505` | `task/updated`, task artifacts |
//! | `peer` | `r6-peer-a6ea8505` | `peer/gather`, `peer/prepare` |
//! | `session` | `r3-session-a6ea8505` | the context-compaction lifecycle |
//!
//! ## Session-id rewriting (why the recording is portable)
//!
//! A recording carries the session id of the profile it was captured under
//! (`dsflash:main`, `octoscode49213:main`, …). The app opens `{OCTOS_PROFILE_ID}:main`.
//! If the two differ, every replayed frame lands in a *different* session than
//! the app has active and **nothing renders**. So the server rewrites the
//! recorded session id to the one the app actually requested in `session/open`
//! — across every served frame, including `cursor.stream`. That is what makes a
//! committed fixture portable between profiles.
//!
//! Handshake: `session/open` is answered from the recording (its own result
//! carries the negotiated capabilities + workspace root); `session/list` gets a
//! one-row reply naming the opened session; the recording's standalone
//! notifications (anything not tied to a turn — approvals, monitors, …) are
//! delivered right after open; `turn/start` replays the next unplayed recorded
//! turn; every other request gets `{}`.
use std::collections::BTreeMap;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

/// One recorded frame.
#[derive(Clone)]
struct Frame {
    dir: String,
    method: String,
    body: Value,
}

/// The scenario → fixture mapping. `(label, file)` under `tests/fixtures/`.
fn scenario_fixture(name: &str) -> (&'static str, &'static str) {
    match name {
        "interrupt" | "conversation" | "live-gate" | "" => {
            ("conversation", "live-gate-a6ea8505.jsonl")
        }
        "live-turn" => ("live-turn", "live-turn-a6ea8505.jsonl"),
        // Card #21c L2: two REAL consecutive turns from one live session, so a
        // `turn/start` twice reproduces the live "second turn never appears".
        "two-turn" => ("two-turn", "live-two-turn-a6ea8505.jsonl"),
        "approval" => ("approval", "r5-turn-a6ea8505.jsonl"),
        // Card #21c item 8: the same recording carries a real TOOL_CALL
        // (`tool_start`/`tool_progress`/`tool_end`), so a `tool-cell` row renders.
        "tool" | "tool-cell" => ("tool", "r5-turn-a6ea8505.jsonl"),
        "autonomy" => ("autonomy", "r1-autonomy-a6ea8505.jsonl"),
        "task" => ("task", "r4-task-a6ea8505.jsonl"),
        "peer" => ("peer", "r6-peer-a6ea8505.jsonl"),
        "session" => ("session", "r3-session-a6ea8505.jsonl"),
        // A5: the dialogs' click walk. The handshake + standalone notifications
        // are r1-autonomy's; every REQUEST the dialogs send is answered from
        // the recorded replies (`screens_replies`).
        "screens" => ("screens", "r1-autonomy-a6ea8505.jsonl"),
        // #32b3: one synthetic turn whose fenced code block carries a 227-column
        // line — the long-code-line render capture (the web wraps: pre-wrap).
        "longcodeline" => ("longcodeline", "longcodeline-a6ea8505.jsonl"),
        // A6: the conversation surfaces' walk — r23's real turns (reasoning,
        // a tool call, the user question, an approval, the plan), each
        // interaction HELD until the app answers it (see `surfaces`).
        "surfaces" => ("surfaces", "r23-conversation-a6ea8505.jsonl"),
        other => {
            eprintln!("[replay-serve] unknown scenario '{other}' — using `conversation`");
            ("conversation", "live-gate-a6ea8505.jsonl")
        }
    }
}

fn fixture(file: &str) -> Vec<Frame> {
    let path = format!(
        "{}/../octoscode-client/tests/fixtures/{file}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read the fixture {path}: {e}"));
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v.get("body").cloned().unwrap_or(Value::Null),
            }
        })
        .collect()
}

/// The recorded `session/open` result — the frame that carries capabilities,
/// the workspace root and the cursor.
fn recorded_open_result(frames: &[Frame]) -> Option<Value> {
    frames
        .iter()
        .filter(|f| f.dir == "in" && f.method == "session/open")
        .map(|f| &f.body)
        .find(|b| b.get("active_profile_id").is_some())
        .cloned()
}

/// The session id the recording was captured under (the open result's id, else
/// the most common `session_id` in the frames).
fn recorded_session(frames: &[Frame]) -> String {
    if let Some(id) = recorded_open_result(frames)
        .and_then(|b| b.get("session_id").and_then(|s| s.as_str()).map(str::to_owned))
    {
        return id;
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for f in frames {
        if let Some(s) = f.body.get("session_id").and_then(|s| s.as_str()) {
            *counts.entry(s.to_owned()).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(s, _)| s)
        .unwrap_or_else(|| "dsflash:main".to_owned())
}

/// Recursively rewrite the recorded session id → the app's requested id, so the
/// recording lands in the session the app actually has active.
fn rewrite_session(value: &mut Value, from: &str, to: &str) {
    match value {
        Value::String(s) => {
            if s.contains(from) {
                *s = s.replace(from, to);
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_session(item, from, to);
            }
        }
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                rewrite_session(v, from, to);
            }
        }
        _ => {}
    }
}

/// The recording's standalone notifications: decoded notifications NOT tied to a
/// turn (so they carry no `turn_id`), delivered right after open. `projection/envelope`
/// frames are turn-streamed instead; `session/open`/`capabilities`/`state:` are
/// handshake, not notifications.
fn standalone_notifications(frames: &[Frame]) -> Vec<Frame> {
    frames
        .iter()
        .filter(|f| f.dir == "in")
        .filter(|f| {
            !f.method.starts_with("state:")
                && f.method != "session/open"
                && f.method != "capabilities"
                && f.method != "projection/envelope"
        })
        .filter(|f| f.body.get("turn_id").is_none())
        .cloned()
        .collect()
}

/// A5 — the `screens` scenario's reply table: request method → the recorded
/// reply body, from the committed recordings. Direct replies come from the
/// fixtures that recorded the request AND its reply (r2-profile's
/// `in <method>`, r3-session's `res:<method>`, r6-peer, r4-task, c24b); the
/// autonomy reads and controls, whose replies r1 did not keep, answer with
/// the objects r1's own `loop/updated` / `monitor/updated` /
/// `session/goal/updated` notifications carried at the same step. Each entry
/// keeps the session id it was recorded under, for the rewrite.
fn screens_replies() -> BTreeMap<String, (Value, String)> {
    let mut out: BTreeMap<String, (Value, String)> = BTreeMap::new();
    // c24b before r4: its `task/list` recorded a RUNNING task (r4's is
    // empty), so the Tasks dialog has a Cancel to click.
    for file in [
        "r2-profile-a6ea8505.jsonl",
        "r3-session-a6ea8505.jsonl",
        "r6-peer-a6ea8505.jsonl",
        "c24b-subagent-a6ea8505.jsonl",
        "r4-task-a6ea8505.jsonl",
    ] {
        let frames = fixture(file);
        let session = recorded_session(&frames);
        let asked: std::collections::BTreeSet<String> = frames
            .iter()
            .filter(|f| f.dir == "out")
            .map(|f| f.method.clone())
            .collect();
        for f in frames.iter().filter(|f| f.dir == "in") {
            let method = f.method.strip_prefix("res:").unwrap_or(&f.method).to_owned();
            if !asked.contains(&method) || method == "session/open" || f.body.is_null() {
                continue;
            }
            out.entry(method).or_insert_with(|| (f.body.clone(), session.clone()));
        }
    }
    // r6 gathered twice: before and after `peer/prepare`; the later reply
    // carries the staged peer, so the Fleet dialog has a row to steer.
    let r6 = fixture("r6-peer-a6ea8505.jsonl");
    if let Some(last) = r6.iter().rev().find(|f| f.dir == "in" && f.method == "peer/gather") {
        out.insert("peer/gather".to_owned(), (last.body.clone(), recorded_session(&r6)));
    }
    let r1 = fixture("r1-autonomy-a6ea8505.jsonl");
    let r1_session = recorded_session(&r1);
    let bodies = |m: &str| -> Vec<Value> {
        r1.iter().filter(|f| f.dir == "in" && f.method == m).map(|f| f.body.clone()).collect()
    };
    let loops = bodies("loop/updated");
    let monitors = bodies("monitor/updated");
    let goal = bodies("session/goal/updated");
    let mut put = |m: &str, v: Value| {
        out.insert(m.to_owned(), (v, r1_session.clone()));
    };
    if let Some(first) = loops.first() {
        put("loop/list", serde_json::json!({ "loops": [first["loop"].clone()] }));
        put("loop/create", first.clone());
    }
    // r1's sequence: create, pause, resume, (fire), delete.
    if let Some(v) = loops.get(1) {
        put("loop/pause", v.clone());
    }
    if let Some(v) = loops.get(2) {
        put("loop/resume", v.clone());
        put("loop/fire_now", v.clone());
    }
    if let Some(v) = loops.get(3) {
        put("loop/delete", v.clone());
    }
    if let Some(first) = monitors.first() {
        put("monitor/list", serde_json::json!({ "monitors": [first["monitor"].clone()] }));
    }
    if let Some(v) = monitors.get(1) {
        put("monitor/pause", v.clone());
    }
    if let Some(v) = monitors.get(2) {
        put("monitor/resume", v.clone());
    }
    if let Some(v) = monitors.get(3) {
        put("monitor/delete", v.clone());
    }
    if let Some(g) = goal.first() {
        put("session/goal/get", g.clone());
        put("session/goal/set", g.clone());
    }
    if let Some(c) = bodies("session/goal/cleared").first() {
        put("session/goal/clear", c.clone());
    }
    out
}

/// A6 — the `surfaces` scenario: what each `turn/start` replays, in order,
/// and how the stream is HELD at an interaction until the app answers it.
///
/// | # | recorded turn (r23) | exercises |
/// |---|---|---|
/// | 1 | `…23b` 17×23 | reasoning (folded thinking rows) + the answer |
/// | 2 | `…23c` echo | a real tool call (a tool row to expand) |
/// | 3 | `…240` color | `user_question/requested` — held until `user_question/respond` |
/// | 4 | `…241` sudo  | `approval/requested` (r5's TYPED command approval in its place) — held until `approval/respond` |
/// | 5 | `…243` plan  | `plan/updated` + a fixture progress update — held until `turn/interrupt` |
/// | 6 | `…242` sudo  | `approval/auto_resolved` (fixture, r23's shape): the policy decides, no card |
///
/// Replies with no successful recording are fixture bodies in the
/// octos-core shapes (`TaskOutputReadResult`, `TaskArtifactListResult`,
/// `TaskArtifactReadResult`, `TaskCancelResult`, `UserQuestionRespondResult`,
/// `ApprovalRespondResult`); the rest are recorded: c24b's `task/list` (a
/// running and a completed task), r3's `session/status/read`, r4's live
/// `task/updated` + `task/output/delta`.
mod surfaces {
    use super::{fixture, recorded_session, Frame};
    use serde_json::{json, Value};

    pub const TURNS: &[&str] = &[
        "01920000-0000-7000-8000-00000000023b",
        "01920000-0000-7000-8000-00000000023c",
        "01920000-0000-7000-8000-000000000240",
        "01920000-0000-7000-8000-000000000241",
        "01920000-0000-7000-8000-000000000243",
        "01920000-0000-7000-8000-000000000242",
    ];

    /// Where a turn's stream stops until the app acts.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Hold {
        Question,
        Approval,
        Plan,
    }

    /// The frames of turn `n` (0-based) for the `surfaces` walk, each with
    /// the hold that follows it (if any).
    pub fn turn_frames(frames: &[Frame], n: usize) -> Vec<(Frame, Option<Hold>)> {
        let Some(turn) = TURNS.get(n.min(TURNS.len() - 1)) else { return Vec::new() };
        let r5 = fixture("r5-turn-a6ea8505.jsonl");
        let r5_session = recorded_session(&r5);
        let mut out = Vec::new();
        for f in frames.iter().filter(|f| f.dir == "in" && f.body.get("turn_id").and_then(|t| t.as_str()) == Some(turn)) {
            match f.method.as_str() {
                // The app answers; the decision is synthesized from ITS reply.
                "approval/decided" => continue,
                "approval/requested" if n == 5 => {
                    // Turn 6: a recorded scope policy resolves it (no card).
                    let b = &f.body;
                    out.push((
                        Frame {
                            dir: "in".into(),
                            method: "approval/auto_resolved".into(),
                            body: json!({
                                "session_id": b["session_id"], "approval_id": b["approval_id"],
                                "turn_id": b["turn_id"], "tool_name": b["tool_name"],
                                "scope": "session", "scope_match": "exact", "decision": "approve"
                            }),
                        },
                        None,
                    ));
                    continue;
                }
                "approval/requested" => {
                    // r5's typed command approval, re-pointed at this turn.
                    let mut a = r5
                        .iter()
                        .find(|x| x.dir == "in" && x.method == "approval/requested")
                        .cloned()
                        .expect("r5 records an approval");
                    super::rewrite_session(&mut a.body, &r5_session, f.body["session_id"].as_str().unwrap_or(""));
                    a.body["turn_id"] = f.body["turn_id"].clone();
                    out.push((a, Some(Hold::Approval)));
                    continue;
                }
                "user_question/requested" => {
                    out.push((f.clone(), Some(Hold::Question)));
                    continue;
                }
                "plan/updated" => {
                    out.push((f.clone(), None));
                    // A wholesale replacement as the work progresses (the
                    // update_plan tool resends the FULL list, plan.ts:20).
                    let mut next = f.clone();
                    if let Some(items) = next.body["plan"]["items"].as_array_mut() {
                        let statuses = ["completed", "in_progress", "pending"];
                        for (i, it) in items.iter_mut().enumerate() {
                            it["status"] = json!(statuses.get(i).copied().unwrap_or("pending"));
                        }
                    }
                    next.body["plan"]["updated_at_ms"] =
                        json!(next.body["plan"]["updated_at_ms"].as_i64().unwrap_or(0) + 4_000);
                    out.push((next, Some(Hold::Plan)));
                    continue;
                }
                _ => {}
            }
            out.push((f.clone(), None));
        }
        out
    }

    /// The scenario's request replies (`None` = the default `{}`).
    pub fn reply(method: &str, params: &Value, session: &str) -> Option<Value> {
        let task_id = params["task_id"].as_str().unwrap_or_default();
        Some(match method {
            "task/list" => {
                let c24b = fixture("c24b-subagent-a6ea8505.jsonl");
                let mut v = c24b
                    .iter()
                    .find(|f| f.dir == "in" && f.method == "task/list")
                    .map(|f| f.body.clone())
                    .expect("c24b records a task/list reply");
                v["session_id"] = json!(session);
                v
            }
            "session/status/read" => {
                let r3 = fixture("r3-session-a6ea8505.jsonl");
                let mut v = r3
                    .iter()
                    .find(|f| f.method == "res:session/status/read")
                    .map(|f| f.body.clone())
                    .expect("r3 records a status reply");
                if let Some(o) = v.as_object_mut() {
                    o.remove("capabilities");
                }
                v["session_id"] = json!(session);
                v
            }
            "task/output/read" => {
                let lines = [
                    "Compiling octos-cli v0.24.1",
                    "Finished test [unoptimized + debuginfo] target(s) in 1.23s",
                    "Running unittests src/lib.rs",
                    "running 12 tests",
                    "test steer_queue::drains_after_reconnect ... ok",
                    "test steer_queue::keeps_order ... ok",
                ];
                let text = format!("{}\n", lines.join("\n"));
                let len = text.len() as u64;
                json!({
                    "session_id": session, "task_id": task_id, "source": "runtime_projection",
                    "cursor": {"offset": 0}, "next_cursor": {"offset": len}, "text": text,
                    "bytes_read": len, "total_bytes": len + 2048, "truncated": true, "complete": false,
                    "live_tail_supported": true, "is_snapshot_projection": false,
                    "task_status": "running", "runtime_state": "executing_tool", "lifecycle_state": "running"
                })
            }
            "task/artifact/list" => json!({
                "session_id": session, "task_id": task_id,
                "artifacts": [
                    {"id": "art-1", "title": "test-report.md", "kind": "report", "status": "ready", "path": "artifacts/test-report.md"},
                    {"id": "art-2", "title": "coverage.json", "kind": "data", "status": "ready"}
                ]
            }),
            "task/artifact/read" => json!({
                "session_id": session, "task_id": task_id,
                "artifact": {"id": params["artifact_id"], "title": "test-report.md", "kind": "report", "status": "ready", "path": "artifacts/test-report.md"},
                "content": "# Test report\n12 passed, 0 failed\n",
                "has_more": false
            }),
            "task/cancel" => json!({"task_id": task_id, "status": "cancelled"}),
            _ => return None,
        })
    }

    /// r4's live supervision frames (`task/updated` + `task/output/delta`),
    /// delivered after open so the Trajectory merges them live.
    pub fn live_task_frames() -> Vec<Frame> {
        let r4 = fixture("r4-task-a6ea8505.jsonl");
        r4.into_iter()
            .filter(|f| f.dir == "in" && (f.method == "task/updated" || f.method == "task/output/delta"))
            .take(2)
            .collect()
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args.get(1).and_then(|p| p.parse().ok()).unwrap_or(8393);
    let delay_ms: u64 = args
        .iter()
        .position(|a| a == "--delay-ms")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    let scenario = args
        .iter()
        .position(|a| a == "--scenario")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_default();
    let (label, file) = scenario_fixture(&scenario);
    let replies = if label == "screens" { screens_replies() } else { BTreeMap::new() };
    if !replies.is_empty() {
        println!(
            "[replay-serve] screens: {} recorded replies: {:?}",
            replies.len(),
            replies.keys().collect::<Vec<_>>()
        );
    }

    let frames = fixture(file);
    // turn_id -> its frames. The app mints its own turn ids, so a `turn/start`
    // is matched to the next unplayed recorded turn.
    let mut by_turn: BTreeMap<String, Vec<Frame>> = BTreeMap::new();
    for f in frames.iter().filter(|f| f.dir == "in") {
        if let Some(t) = f.body.get("turn_id").and_then(|t| t.as_str()) {
            by_turn.entry(t.to_owned()).or_default().push(f.clone());
        }
    }
    let open_result = recorded_open_result(&frames).expect("the fixture has a session/open result");
    let recorded = recorded_session(&frames);
    let recorded_turns: Vec<String> = by_turn
        .iter()
        .filter(|(_, v)| v.iter().any(|f| f.method == "projection/envelope"))
        .map(|(k, _)| k.clone())
        .collect();
    // A5: the `screens` scenario answers each read with ONE recorded step's
    // objects (the loop as created, the goal as set); replaying r1's later
    // standalone deletes/clears first would make those reads stale on arrival
    // (and the generation gate rightly refuses them), so it sends none of
    // those. It does send each family's FIRST recorded update (the loop as created,
    // the monitor as created, the goal as set — the same step its reads
    // answer with), so the store's autonomy domain, and with it the
    // sidebar's GOALS / LOOPS rows, hold what the dialogs show.
    let standalone = if label == "screens" {
        let all = standalone_notifications(&frames);
        ["loop/updated", "monitor/updated", "session/goal/updated"]
            .iter()
            .filter_map(|m| all.iter().find(|f| f.method == *m).cloned())
            .collect()
    } else {
        standalone_notifications(&frames)
    };
    println!(
        "[replay-serve] scenario={label} fixture={file}: {} frames; recorded session `{recorded}`; \
         {} recorded turns {:?}; {} standalone notifications",
        frames.len(),
        recorded_turns.len(),
        recorded_turns,
        standalone.len()
    );

    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).await.expect("bind");
    println!("[replay-serve] listening on ws://{addr}/api/ui-protocol/ws (delay {delay_ms}ms)");

    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        println!("[replay-serve] client connected");
        let by_turn = by_turn.clone();
        let open_result = open_result.clone();
        let recorded = recorded.clone();
        let recorded_turns = recorded_turns.clone();
        let standalone = standalone.clone();
        let replies = replies.clone();
        tokio::spawn(async move {
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else {
                return;
            };
            let (tx, mut rx) = ws.split();
            let tx = std::sync::Arc::new(tokio::sync::Mutex::new(tx));
            let mut played = 0usize;
            // The session id the app opens; every served frame is rewritten to it.
            let mut active_session = recorded.clone();
            // A6 `surfaces`: the rest of a turn held at an interaction, and
            // the approval it waits on.
            type Held = (Option<surfaces::Hold>, Vec<(Frame, Option<surfaces::Hold>)>, Value);
            let held: std::sync::Arc<tokio::sync::Mutex<Held>> =
                std::sync::Arc::new(tokio::sync::Mutex::new((None, Vec::new(), Value::Null)));
            let all_frames = if label == "surfaces" { fixture(file) } else { Vec::new() };

            while let Some(Ok(msg)) = rx.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                println!("[replay-serve] <- {method} (id={id})");

                // A helper to send one JSON-RPC reply/notification.
                async fn send(tx: &std::sync::Arc<tokio::sync::Mutex<futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>, Message>>>, v: Value) {
                    let _ = tx.lock().await.send(Message::Text(v.to_string().into())).await;
                }

                // A6 `surfaces`: stream a turn's frames until a hold; the
                // remainder waits in `held` for the app's answer.
                async fn stream_until_hold(
                    tx: std::sync::Arc<tokio::sync::Mutex<futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>, Message>>>,
                    frames: Vec<(Frame, Option<surfaces::Hold>)>,
                    from: String,
                    to: String,
                    delay_ms: u64,
                    held: std::sync::Arc<tokio::sync::Mutex<(Option<surfaces::Hold>, Vec<(Frame, Option<surfaces::Hold>)>, Value)>>,
                ) {
                    let mut it = frames.into_iter();
                    while let Some((mut f, hold)) = it.next() {
                        rewrite_session(&mut f.body, &from, &to);
                        if f.method != "projection/envelope" && f.method != "progress/updated" {
                            println!("[replay-serve] surfaces -> {}", f.method);
                        }
                        let frame = serde_json::json!({"jsonrpc": "2.0", "method": f.method, "params": f.body});
                        let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                        if let Some(h) = hold {
                            println!("[replay-serve] surfaces: holding at {:?} ({} frames wait)", h, it.len());
                            *held.lock().await = (Some(h), it.collect(), f.body.clone());
                            return;
                        }
                    }
                }
                if label == "surfaces" {
                    let params = v["params"].clone();
                    let handled = match method.as_str() {
                        "turn/start" => {
                            send(&tx, serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {"accepted": true}})).await;
                            let frames = surfaces::turn_frames(&all_frames, played);
                            println!("[replay-serve] surfaces: turn #{played} ({} frames)", frames.len());
                            played += 1;
                            tokio::spawn(stream_until_hold(tx.clone(), frames, recorded.clone(), active_session.clone(), delay_ms, held.clone()));
                            true
                        }
                        "approval/respond" | "user_question/respond" | "turn/interrupt" => {
                            let (hold, rest, request) = {
                                let mut h = held.lock().await;
                                std::mem::replace(&mut *h, (None, Vec::new(), Value::Null))
                            };
                            let result = match method.as_str() {
                                "approval/respond" => serde_json::json!({
                                    "approval_id": params["approval_id"], "accepted": true,
                                    "status": "accepted", "runtime_resumed": true
                                }),
                                "user_question/respond" => serde_json::json!({
                                    "question_id": params["question_id"], "accepted": true, "runtime_resumed": true
                                }),
                                _ => serde_json::json!({}),
                            };
                            send(&tx, serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result})).await;
                            if method == "approval/respond" && hold == Some(surfaces::Hold::Approval) {
                                // The durable decision the server broadcasts.
                                send(&tx, serde_json::json!({"jsonrpc": "2.0", "method": "approval/decided", "params": {
                                    "session_id": active_session, "approval_id": params["approval_id"],
                                    "turn_id": request["turn_id"], "decision": params["decision"],
                                    "scope": params["approval_scope"], "decided_at": "2026-10-01T12:00:00Z",
                                    "decided_by": "", "auto_resolved": false
                                }})).await;
                            }
                            if hold.is_some() {
                                println!("[replay-serve] surfaces: {method} releases {} frames", rest.len());
                                tokio::spawn(stream_until_hold(tx.clone(), rest, recorded.clone(), active_session.clone(), delay_ms, held.clone()));
                            }
                            true
                        }
                        m => match surfaces::reply(m, &params, &active_session) {
                            Some(body) => {
                                println!("[replay-serve] -> {m} (surfaces reply)");
                                send(&tx, serde_json::json!({"jsonrpc": "2.0", "id": id, "result": body})).await;
                                true
                            }
                            None => false,
                        },
                    };
                    if handled {
                        continue;
                    }
                }
                match method.as_str() {
                    "session/open" => {
                        let requested = v["params"]["session_id"]
                            .as_str()
                            .unwrap_or(&recorded)
                            .to_owned();
                        active_session = requested.clone();
                        let mut opened = open_result.clone();
                        rewrite_session(&mut opened, &recorded, &requested);
                        if let Some(obj) = opened.as_object_mut() {
                            obj.insert("session_id".to_owned(), Value::String(requested));
                        }
                        send(&tx, serde_json::json!({
                            "jsonrpc": "2.0", "id": id, "result": {"opened": opened}
                        })).await;
                        // Deliver the recording's standalone notifications (approvals,
                        // monitors, task updates, …) so their cards can mount without
                        // needing a turn.
                        let tx2 = tx.clone();
                        let mut notifs = standalone.clone();
                        let from = recorded.clone();
                        let to = active_session.clone();
                        // A6 `surfaces`: r4's live task frames merge into the
                        // Trajectory (re-pointed from r4's own session).
                        let r4_session = if label == "surfaces" {
                            let r4 = fixture("r4-task-a6ea8505.jsonl");
                            recorded_session(&r4)
                        } else {
                            String::new()
                        };
                        if label == "surfaces" {
                            for mut f in surfaces::live_task_frames() {
                                rewrite_session(&mut f.body, &r4_session, &from);
                                notifs.push(f);
                            }
                        }
                        tokio::spawn(async move {
                            for mut f in notifs {
                                rewrite_session(&mut f.body, &from, &to);
                                let frame = serde_json::json!({
                                    "jsonrpc": "2.0", "method": f.method, "params": f.body
                                });
                                let _ = tx2.lock().await.send(Message::Text(frame.to_string().into())).await;
                                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                            }
                        });
                    }
                    "session/list" => {
                        let session = v["params"]["session_id"]
                            .as_str()
                            .unwrap_or(&active_session)
                            .to_owned();
                        send(&tx, serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"sessions": [{
                                "id": session,
                                "title": "Why does main.rs print 5?",
                                "message_count": 1,
                                "active_turn": false
                            }]}
                        })).await;
                    }
                    // #P4a1 — echo the requested mode as the read-back.
                    "permission/profile/set" => {
                        send(&tx, serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"current": {"mode": v["params"]["update"]["mode"]}}
                        })).await;
                    }
                    "turn/start" => {
                        send(&tx, serde_json::json!({
                            "jsonrpc": "2.0", "id": id, "result": {"accepted": true}
                        })).await;
                        let turn = recorded_turns
                            .get(played)
                            .cloned()
                            .unwrap_or_else(|| recorded_turns.first().cloned().unwrap_or_default());
                        played += 1;
                        let frames = by_turn.get(&turn).cloned().unwrap_or_default();
                        println!(
                            "[replay-serve] replaying {turn} ({} recorded frames)",
                            frames.len()
                        );
                        let tx2 = tx.clone();
                        let from = recorded.clone();
                        let to = active_session.clone();
                        tokio::spawn(async move {
                            for mut f in frames {
                                rewrite_session(&mut f.body, &from, &to);
                                let frame = serde_json::json!({
                                    "jsonrpc": "2.0", "method": f.method, "params": f.body
                                });
                                let _ = tx2.lock().await.send(Message::Text(frame.to_string().into())).await;
                                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                            }
                        });
                    }
                    // A5 — the `screens` scenario: the recorded reply for
                    // this method, re-pointed at the session the app opened.
                    m if replies.contains_key(m) => {
                        let (mut body, from) = replies[m].clone();
                        rewrite_session(&mut body, &from, &active_session);
                        println!("[replay-serve] -> {m} (recorded reply)");
                        send(&tx, serde_json::json!({
                            "jsonrpc": "2.0", "id": id, "result": body
                        })).await;
                    }
                    _ => {
                        send(&tx, serde_json::json!({
                            "jsonrpc": "2.0", "id": id, "result": {}
                        })).await;
                    }
                }
            }
        });
    }
}
