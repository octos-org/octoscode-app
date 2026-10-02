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
//! | `fleet` | `a10-fleet-driver-synthetic` (SYNTHETIC) | the external-driver chain: walk, acquire, prepare, dispatch, peer frames, peer/control |
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
        // A10: the `screens` replies plus, for the methods no recording
        // carries, the faithful `a10-*-faithful.jsonl` frames served IN ORDER
        // (a load-more's second read gets the second reply).
        "a10" => ("a10", "r1-autonomy-a6ea8505.jsonl"),
        // A10 fleet: the FAITHFUL external-driver fixture (synthetic — no
        // live server advertises `external_driver_v1`; built by
        // tools/fixtures/a10_fleet_fixture.py from r6/r23/c24 + the web's
        // protocol). Its open advertises the driver methods; `FleetSim`
        // answers the chain, echoing each request's ids like the web's own
        // fixture server.
        "fleet" => ("fleet", "a10-fleet-driver-synthetic.jsonl"),
        // A9: the Activity walk. r4-task's handshake (task/list,
        // task/output/read advertised); `session/list` names five sessions of
        // the opened Profile; `task/list` answers each from the recorded c24b
        // snapshots (`activity_task_reply`).
        "activity" => ("activity", "r4-task-a6ea8505.jsonl"),
        // #32b3: one synthetic turn whose fenced code block carries a 227-column
        // line — the long-code-line render capture (the web wraps: pre-wrap).
        "longcodeline" => ("longcodeline", "longcodeline-a6ea8505.jsonl"),
        // A6: the conversation surfaces' walk — r23's real turns (reasoning,
        // a tool call, the user question, an approval, the plan), each
        // interaction HELD until the app answers it (see `surfaces`).
        "surfaces" => ("surfaces", "r23-conversation-a6ea8505.jsonl"),
        // A15: history on open — r43a's RECORDED canonical hydrate (six
        // turns, their tool envelopes) answers every `session/hydrate` of the
        // recorded Session; any other Session (a New chat) has none; the
        // catalog names it by its first prompt (see `history_reply`). Run the
        // app with OCTOS_PROFILE_ID=dsflash (the recorded profile).
        "history" => ("history", "r43a-recovery-a6ea8505.jsonl"),
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

/// A15 — the `history` scenario's answers: the recorded canonical hydrate for
/// the recorded (`home`) Session, an empty one for any other (a New chat:
/// octos has nothing persisted for it), and the catalog row octos lists for
/// the home Session — titled by its first prompt (the session file's
/// `title`, cut at 50 chars), its last prompt, its row count — attesting the
/// scope when the request names `{cwd, profile_id}` (`SessionListResult`).
/// As octos a6ea8505 does: the legacy unscoped listing does not see a
/// workspace's `<cwd>/.octos/<profile>` store (empty), and a status read
/// that names no profile for a key that embeds none falls back to `_main`
/// and is refused (`raw_profile_id`, `profile_unresolved_error`).
fn history_reply(method: &str, p: &Value, home: &str, hydrate: &Value) -> Option<Result<Value, Value>> {
    match method {
        "session/hydrate" => {
            let s = p["session_id"].as_str().unwrap_or(home);
            Some(Ok(if s == home {
                hydrate.clone()
            } else {
                serde_json::json!({"session_id": s, "cursor": {"stream": s, "seq": 1}, "messages": []})
            }))
        }
        "session/status/read"
            if p.get("profile_id").and_then(|v| v.as_str()).is_none_or(str::is_empty)
                && p["session_id"].as_str().is_some_and(|s| s.split(':').count() < 3) =>
        {
            Some(Err(serde_json::json!({
                "code": -32602,
                "message": "profile '_main' is not configured for this AppUI session",
                "data": {"kind": "profile_unresolved", "profile_id": "_main", "recoverable": true}
            })))
        }
        "session/list" => {
            let msgs = hydrate["messages"].as_array().cloned().unwrap_or_default();
            let users: Vec<String> = msgs
                .iter()
                .filter(|m| m["role"] == "user")
                .filter_map(|m| m["content"].as_str().map(str::to_owned))
                .collect();
            let row = serde_json::json!({
                "id": home,
                "title": users.first().map(|t| t.chars().take(50).collect::<String>()),
                "last_prompt": users.last(),
                "message_count": msgs.len(),
                "updated_at": "2026-10-01T15:48:13Z",
                "active_turn": false
            });
            Some(Ok(match (p["cwd"].as_str(), p["profile_id"].as_str()) {
                (Some(cwd), Some(profile)) => {
                    serde_json::json!({"sessions": [row], "workspace_root": cwd, "profile_id": profile})
                }
                _ => serde_json::json!({"sessions": []}),
            }))
        }
        _ => None,
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

/// A10 — every `a10-*-faithful.jsonl` fixture's inbound replies, per method,
/// in file order (`(body, session)`; the session the frames name is the one
/// rewritten to the opened session).
fn a10_sequenced() -> BTreeMap<String, Vec<(Value, String)>> {
    let mut out: BTreeMap<String, Vec<(Value, String)>> = BTreeMap::new();
    let dir = format!("{}/../octoscode-client/tests/fixtures", env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.starts_with("a10-") && n.ends_with("-faithful.jsonl"))
                // The seats' fixture is served by the stateful simulator.
                .filter(|n| n != "a10-seats-faithful.jsonl" && n != "a10-routes-faithful.jsonl")
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    for file in files {
        let frames = fixture(&file);
        let session = frames
            .iter()
            .find(|f| f.dir == "note")
            .and_then(|f| f.body.get("session").and_then(|s| s.as_str()).map(str::to_owned))
            .unwrap_or_else(|| recorded_session(&frames));
        for f in frames.iter().filter(|f| f.dir == "in") {
            out.entry(f.method.clone()).or_default().push((f.body.clone(), session.clone()));
        }
    }
    out
}

/// A10 — the composer seats' simulator (scenario a10): the permission
/// profile and the selected model follow each set/select, so a click walk
/// sees the read-back it caused. The permission state starts from the
/// recorded r2 list; the session model list and the per-model select
/// replies are `a10-seats-faithful.jsonl` (selecting the r2-route fallback
/// answers with r2's recorded select reply).
#[derive(Clone)]
struct SeatSim {
    current: Value,
    profiles: Value,
    models: Vec<Value>,
    selects: Vec<Value>,
    r2_select: Value,
    /// r26's recorded interrupted `turn_terminal` envelope (re-pointed at
    /// the app's turn when it presses Stop).
    r26_terminal: Value,
    /// Envelope sequence for the terminals this simulator emits.
    seq: u64,
    /// The Profile's configured providers (r2's recorded config with the
    /// r2-route fallback), changed by each upsert / delete.
    config: Value,
    /// `a10-routes-faithful.jsonl`: the fetch_models and passing test replies.
    fetched: Value,
    tested: Value,
}

impl SeatSim {
    fn load() -> Self {
        let r2 = fixture("r2-profile-a6ea8505.jsonl");
        let list = r2
            .iter()
            .find(|f| f.dir == "in" && f.method == "permission/profile/list")
            .map(|f| f.body.clone())
            .unwrap_or_default();
        let r2_select = r2
            .iter()
            .find(|f| f.dir == "in" && f.method == "profile/llm/select")
            .map(|f| f.body.clone())
            .unwrap_or_default();
        let seats = fixture("a10-seats-faithful.jsonl");
        let models = seats
            .iter()
            .find(|f| f.dir == "in" && f.method == "profile/llm/list")
            .and_then(|f| f.body["models"].as_array().cloned())
            .unwrap_or_default();
        let selects = seats
            .iter()
            .filter(|f| f.dir == "in" && f.method == "profile/llm/select")
            .map(|f| f.body.clone())
            .collect();
        let r26_terminal = fixture("r26-interrupted-a6ea8505.jsonl")
            .into_iter()
            .find(|f| f.dir == "in" && f.body["payload"]["type"] == "turn_terminal" && f.body["payload"]["data"]["outcome"] == "interrupted")
            .map(|f| f.body)
            .unwrap_or_default();
        let config = r2
            .iter()
            .filter(|f| f.dir == "in" && f.method == "profile/llm/list")
            .map(|f| f.body.clone())
            .find(|b| b["fallbacks"].as_array().is_some_and(|a| !a.is_empty()))
            .unwrap_or_default();
        let routes = fixture("a10-routes-faithful.jsonl");
        let reply = |m: &str| routes.iter().find(|f| f.dir == "in" && f.method == m).map(|f| f.body.clone()).unwrap_or_default();
        let (fetched, tested) = (reply("profile/llm/fetch_models"), reply("profile/llm/test"));
        SeatSim {
            current: list["current"].clone(),
            profiles: list["profiles"].clone(),
            models,
            selects,
            r2_select,
            r26_terminal,
            seq: 0,
            config,
            fetched,
            tested,
        }
    }

    /// The reply (or the JSON-RPC error) for one seat method.
    fn answer(&mut self, method: &str, params: &Value, session: &str) -> Result<Value, Value> {
        match method {
            "permission/profile/list" => Ok(serde_json::json!({
                "session_id": session, "current": self.current, "profiles": self.profiles,
            })),
            "permission/profile/set" => {
                let network = match params["update"]["network"].as_str() {
                    Some(n) => Value::from(n),
                    None => self.current["network"].clone(),
                };
                let want = serde_json::json!({ "mode": params["update"]["mode"], "network": network });
                let offered = self.profiles.as_array().is_some_and(|p| p.contains(&want)) || want == self.current;
                if !offered {
                    return Err(serde_json::json!({"code": -32602, "message": "permission profile not offered for this session"}));
                }
                self.current = want;
                Ok(serde_json::json!({"applied": true, "current": self.current, "session_id": session}))
            }
            "profile/llm/list" => Ok(serde_json::json!({"session_id": session, "models": self.models})),
            // A10 — the configured providers (the Routes dialog).
            "profile/llm/list@profile" => Ok(self.config.clone()),
            "profile/llm/fetch_models" => {
                let mut r = self.fetched.clone();
                r["family_id"] = params["selection"]["family_id"].clone();
                Ok(r)
            }
            "profile/llm/test" => Ok(self.tested.clone()),
            "profile/llm/upsert" => {
                let sel = &params["selection"];
                let row = serde_json::json!({
                    "family_id": sel["family_id"], "model_id": sel["model_id"], "model": sel["model_id"],
                    "provider": sel["family_id"], "route": sel["route"], "route_id": sel["route"]["route_id"],
                    "has_api_key": true, "available": true, "selected": false,
                });
                if let Some(f) = self.config["fallbacks"].as_array_mut() {
                    f.push(row);
                }
                let mut r = self.config.clone();
                r["applied"] = Value::Bool(true);
                Ok(r)
            }
            "profile/llm/delete" => {
                let hit = |m: &Value| {
                    m["family_id"] == params["family_id"] && m["model_id"] == params["model_id"]
                        && m["route"]["route_id"] == params["route_id"]
                };
                if let Some(f) = self.config["fallbacks"].as_array_mut() {
                    f.retain(|m| !hit(m));
                }
                if hit(&self.config["primary"]) {
                    self.config["primary"] = Value::Null;
                }
                let mut r = self.config.clone();
                r["applied"] = Value::Bool(true);
                r["restart_required"] = Value::Bool(true);
                Ok(r)
            }
            // A10 — native review (octos-core `ReviewStartResult`, the web's
            // `parseReviewStartResult`): the request's own turn echoed.
            "review/start" => Ok(serde_json::json!({
                "session_id": session, "turn_id": params["turn_id"], "accepted": true,
                "workflow": "code_review", "backend": "native", "agent_count": 3,
            })),
            "profile/llm/select" => {
                let model = params["model_id"].as_str().unwrap_or("").to_owned();
                let route = params["route_id"].as_str().unwrap_or("").to_owned();
                let Some(i) = self.models.iter().position(|m| m["model"] == model.as_str() && m["route"] == route.as_str()) else {
                    return Err(serde_json::json!({"code": -32602, "message": "unknown model"}));
                };
                let row = self.models[i].clone();
                if row["available"] != Value::Bool(true) {
                    return Ok(serde_json::json!({"applied": false, "session_id": session, "selected": row}));
                }
                if row["selected"] == Value::Bool(true) {
                    return Ok(serde_json::json!({"applied": true, "runtime_disposition": "unchanged", "session_id": session, "selected": row}));
                }
                for m in self.models.iter_mut() {
                    let hit = m["model"] == model.as_str() && m["route"] == route.as_str();
                    m["selected"] = Value::Bool(hit);
                }
                let fixture_reply = self
                    .selects
                    .iter()
                    .find(|r| r["selected"]["model"] == model.as_str() && r["selected"]["route"] == route.as_str() && r["applied"] == Value::Bool(true))
                    .cloned();
                let mut reply = match fixture_reply {
                    Some(r) => r,
                    None if route == "r2-route" => self.r2_select.clone(),
                    None => serde_json::json!({"applied": true, "runtime_disposition": "reloaded", "selected": self.models[i]}),
                };
                reply["session_id"] = Value::from(session);
                Ok(reply)
            }
            _ => Err(serde_json::json!({"code": -32601, "message": "not a seat method"})),
        }
    }
}

/// A10 — the `fleet` scenario's external-driver simulator. Every reply is a
/// fixture shape (`a10-fleet-driver-synthetic.jsonl`) with the per-request
/// ids echoed (operation id, requested lane, acquiring driver, control
/// target), the way `apps/web/scripts/mock-ui-server.mjs` answers its
/// `peer-control-*` workspaces; the binding's revision moves on acquire /
/// release, dispatches join the walked inventory, and a dispatched peer's
/// background attach gets its session's frames (turn/started; the FIRST
/// peer then asks for an approval, the SECOND a question). `lane-review` is listed but answers
/// `driver_model_unavailable` (the fixture's typed refusal frame: a lane
/// whose credentials the server lacks), so the refusal path is clickable.
struct FleetSim {
    get: Value,
    acquire: Value,
    lanes: Value,
    started: Value,
    requested: Value,
    question: Value,
    decided: Value,
    turn_error: Value,
    refusal: Value,
    binding: Value,
    revision: u64,
    token: Option<String>,
    ops: Vec<Value>,
    slugs: Vec<String>,
    receipts: BTreeMap<String, Value>,
    controls: Vec<String>,
    approvals: BTreeMap<String, String>,
    dispatched: u64,
    workspace: String,
    /// A10 seat walk: the driver mode (`external` after an acquire or a
    /// parking release, `internal` after a `next: internal` handback).
    external: bool,
    /// A10 seat walk (`--revoke-file <path>`): when the file exists, the next
    /// renew finds the lease taken by another app (`driver_fence_stale`);
    /// the file is removed so a later acquire holds again.
    revoke_file: Option<String>,
}

/// One reply + the notifications that follow it (`(delay ms, method, params)`).
type SimOut = (Result<Value, Value>, Vec<(u64, String, Value)>);

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl FleetSim {
    fn new(frames: &[Frame], workspace: &str) -> Self {
        let body = |m: &str| -> Value {
            frames
                .iter()
                .find(|f| f.dir == "in" && f.method == m)
                .map(|f| f.body.clone())
                .unwrap_or_else(|| panic!("the fleet fixture has no {m}"))
        };
        let get = body("session/driver/get");
        let binding = get["binding"].clone();
        let revision = binding["revision"].as_u64().unwrap_or(0);
        let ops = get["operations"]["items"].as_array().cloned().unwrap_or_default();
        let mut sim = FleetSim {
            acquire: body("session/driver/acquire"),
            lanes: body("profile/sub_providers/list"),
            started: body("turn/started"),
            requested: body("approval/requested"),
            question: body("user_question/requested"),
            decided: body("approval/decided"),
            turn_error: body("turn/error"),
            refusal: body("err:peer/dispatch"),
            get,
            binding,
            revision,
            token: None,
            ops,
            slugs: Vec::new(),
            receipts: BTreeMap::new(),
            controls: Vec::new(),
            approvals: BTreeMap::new(),
            dispatched: 0,
            workspace: workspace.to_owned(),
            external: true,
            revoke_file: None,
        };
        let ws = sim.workspace.clone();
        for v in [&mut sim.get, &mut sim.binding] {
            rewrite_session(v, "<WORKSPACE>", &ws);
        }
        // The fixture's prior dispatch is anchored 18 minutes before this
        // run (its recorded epoch-ms would read as months of elapsed time).
        let then = now_ms().saturating_sub(18 * 60_000);
        for op in sim.ops.iter_mut() {
            rewrite_session(op, "<WORKSPACE>", &ws);
            op["created_at_ms"] = then.into();
            op["started_at_ms"] = (then + 1000).into();
            op["acceptance"]["accepted_at_ms"] = then.into();
        }
        sim
    }

    fn handles(method: &str) -> bool {
        matches!(
            method,
            "session/driver/get"
                | "session/driver/acquire"
                | "session/driver/renew"
                | "session/driver/release"
                | "profile/sub_providers/list"
                | "peer/prepare"
                | "peer/dispatch"
                | "peer/control"
        )
    }

    fn refuse(&self, kind: &str) -> Value {
        let mut e = self.refusal.clone();
        e["data"]["kind"] = Value::String(kind.to_owned());
        e
    }

    /// The held fence matches the request's (`driver_fence_stale` otherwise).
    fn fenced(&self, p: &Value) -> bool {
        self.token.as_deref().is_some_and(|t| p["control_token"] == t)
            && p["driver_id"] == self.binding["driver_id"]
            && p["epoch"] == self.binding["epoch"]
    }

    fn slugify(text: &str) -> String {
        let mut out = String::new();
        for c in text.chars().flat_map(char::to_lowercase) {
            if c.is_ascii_alphanumeric() {
                out.push(c);
            } else if !out.ends_with('-') && !out.is_empty() {
                out.push('-');
            }
            if out.len() >= 24 {
                break;
            }
        }
        let out = out.trim_matches('-').to_owned();
        if out.is_empty() { "peer".to_owned() } else { out }
    }

    fn reply(&mut self, method: &str, p: &Value, base: &str) -> SimOut {
        let now = now_ms();
        match method {
            "session/driver/get" => {
                let mut v = if self.external {
                    serde_json::json!({"mode": "external", "recovery": "none", "binding": self.binding})
                } else if self.binding.is_null() {
                    // Never bound (a cold master): no binding, revision 0.
                    serde_json::json!({"mode": "internal", "recovery": "none", "binding": null})
                } else {
                    // Handed back: the RETAINED binding, inactive (lease 0) —
                    // its revision is the next acquire's CAS basis.
                    let mut b = self.binding.clone();
                    b["lease_expires_at_ms"] = 0.into();
                    serde_json::json!({"mode": "internal", "recovery": "none", "binding": b})
                };
                if p.get("operations").is_some() {
                    // The page is strictly ordered by operation id (UTF-8
                    // bytes) — the protocol's contract the walk enforces.
                    let mut items = self.ops.clone();
                    items.sort_by(|a, b| a["operation_id"].as_str().cmp(&b["operation_id"].as_str()));
                    v["operations"] = serde_json::json!({
                        "items": items,
                        "snapshot": format!("synthetic-snapshot-{}", self.revision),
                        "observed_revision": self.revision.to_string(),
                        "complete": true,
                        "next_cursor": null,
                    });
                }
                (Ok(v), Vec::new())
            }
            "session/driver/acquire" => {
                if p["expected_revision"].as_u64() != Some(self.revision) {
                    return (Err(self.refuse("driver_revision_conflict")), Vec::new());
                }
                self.revision += 1;
                self.external = true;
                let epoch = self.binding["epoch"].as_u64().unwrap_or(0) + 1;
                let lease = p["lease_seconds"].as_u64().unwrap_or(120);
                self.binding["driver_id"] = p["driver_id"].clone();
                self.binding["epoch"] = epoch.into();
                self.binding["revision"] = self.revision.into();
                self.binding["lease_expires_at_ms"] = (now + lease * 1000).into();
                let token = format!("synthetic-control-token-{epoch}");
                self.token = Some(token.clone());
                let mut a = self.acquire.clone();
                a["control_token"] = Value::String(token);
                a["binding"] = self.binding.clone();
                (Ok(a), Vec::new())
            }
            "session/driver/renew" => {
                if let Some(path) = self.revoke_file.as_deref().filter(|f| std::path::Path::new(f).exists()) {
                    // Another app acquired: our proof is dead from now on.
                    let _ = std::fs::remove_file(path);
                    self.revision += 1;
                    let epoch = self.binding["epoch"].as_u64().unwrap_or(0) + 1;
                    self.binding["driver_id"] = "octos-tui".into();
                    self.binding["epoch"] = epoch.into();
                    self.binding["revision"] = self.revision.into();
                    self.token = None;
                    println!("[replay-serve] fleet sim: the lease was revoked (another app acquired)");
                }
                if !self.fenced(p) {
                    return (Err(self.refuse("driver_fence_stale")), Vec::new());
                }
                let lease = p["lease_seconds"].as_u64().unwrap_or(120);
                self.binding["lease_expires_at_ms"] = (now + lease * 1000).into();
                (Ok(serde_json::json!({ "lease_expires_at_ms": self.binding["lease_expires_at_ms"] })), Vec::new())
            }
            "session/driver/release" => {
                if !self.fenced(p) {
                    return (Err(self.refuse("driver_fence_stale")), Vec::new());
                }
                self.revision += 1;
                self.token = None;
                self.binding["revision"] = self.revision.into();
                self.binding["lease_expires_at_ms"] = 0.into();
                let next = p["next"].as_str().unwrap_or("external").to_owned();
                self.external = next == "external";
                let binding = if next == "external" { self.binding.clone() } else { Value::Null };
                (Ok(serde_json::json!({ "mode": next, "recovery": "none", "binding": binding })), Vec::new())
            }
            "profile/sub_providers/list" => (Ok(self.lanes.clone()), Vec::new()),
            "peer/prepare" => {
                let title = p["title"].as_str().filter(|t| !t.trim().is_empty()).or(p["brief"].as_str()).unwrap_or("peer");
                let mut slug = Self::slugify(title);
                let mut n = 2;
                while self.slugs.contains(&slug) {
                    slug = format!("{}-{n}", Self::slugify(title));
                    n += 1;
                }
                self.slugs.push(slug.clone());
                let profile = p["profile_id"].as_str().unwrap_or("dsflash");
                let peer = serde_json::json!({
                    "slug": slug, "topic": format!("peer-{slug}"), "profile_id": profile, "cwd": self.workspace,
                    "brief_path": format!("~/.octos/profiles/{profile}/data/peers/{slug}/brief.md"),
                });
                let mut v = peer.clone();
                v["peers"] = serde_json::json!([peer]);
                (Ok(v), Vec::new())
            }
            "peer/dispatch" => {
                if !self.fenced(p) {
                    return (Err(self.refuse("driver_fence_stale")), Vec::new());
                }
                let op = p["operation_id"].as_str().unwrap_or("").to_owned();
                if let Some(r) = self.receipts.get(&op) {
                    let mut r = r.clone();
                    r["duplicate"] = true.into();
                    return (Ok(r), Vec::new());
                }
                let lane = p["model"].as_str().unwrap_or("");
                if lane == "lane-review" {
                    return (Err(self.refuse("driver_model_unavailable")), Vec::new());
                }
                let Some(model) = self.lanes["sub_providers"]
                    .as_array()
                    .and_then(|ls| ls.iter().find(|l| l["key"] == lane))
                    .map(|l| l["model"].clone())
                else {
                    return (Err(self.refuse("driver_model_unavailable")), Vec::new());
                };
                let slug = match p["dispatch"]["kind"].as_str() {
                    Some("existing_slug") => p["dispatch"]["slug"].as_str().unwrap_or("peer").to_owned(),
                    _ => p["dispatch"]["title"].as_str().map(Self::slugify).unwrap_or_else(|| "peer".to_owned()),
                };
                self.dispatched += 1;
                let adopted = format!("{base}#peer-{slug}");
                let turn = format!("00000000-0000-4000-8000-{:012x}", 0xd1 + self.dispatched);
                let acceptance = serde_json::json!({
                    "model": model, "model_lane": lane, "workspace_root": self.workspace, "scoped_goal": null,
                    "adopted_turn_id": turn, "adopted_session_id": adopted, "slug": slug,
                    "accepted_at_ms": now, "payload_digest": format!("synthetic-payload-digest-{}", self.dispatched),
                });
                self.ops.push(serde_json::json!({
                    "operation_id": op, "kind": "peer_dispatch", "lifecycle": "started",
                    "created_at_ms": now, "started_at_ms": now, "acceptance": acceptance,
                }));
                let mut r = acceptance.clone();
                r["operation_id"] = Value::String(op.clone());
                r["state"] = "accepted".into();
                r["duplicate"] = false.into();
                self.receipts.insert(op, r.clone());
                (Ok(r), Vec::new())
            }
            "peer/control" => {
                if !self.fenced(p) {
                    return (Err(self.refuse("driver_fence_stale")), Vec::new());
                }
                let target = p["target_operation_id"].as_str().unwrap_or("");
                let (slug, session) = match self.ops.iter().find(|o| o["operation_id"] == target) {
                    Some(o) => (
                        o["acceptance"]["slug"].as_str().unwrap_or("").to_owned(),
                        o["acceptance"]["adopted_session_id"].as_str().unwrap_or("").to_owned(),
                    ),
                    // The acquire's pending work (the seat's target).
                    None => ("lint-sweep".to_owned(), format!("{base}#peer-lint-sweep")),
                };
                let op = p["operation_id"].as_str().unwrap_or("").to_owned();
                let duplicate = self.controls.contains(&op);
                self.controls.push(op.clone());
                let turn = p["expected_turn_id"].clone();
                let mut pushes = Vec::new();
                match p["command"]["kind"].as_str() {
                    Some("approval_respond") => {
                        if let Some(approval) = self.approvals.remove(&session) {
                            let mut d = self.decided.clone();
                            d["session_id"] = Value::String(session.clone());
                            d["turn_id"] = turn.clone();
                            d["approval_id"] = Value::String(approval);
                            d["decision"] = p["command"]["decision"].clone();
                            pushes.push((200, "approval/decided".to_owned(), d));
                        }
                    }
                    Some("interrupt") => {
                        let mut e = self.turn_error.clone();
                        e["session_id"] = Value::String(session.clone());
                        e["turn_id"] = turn.clone();
                        pushes.push((300, "turn/error".to_owned(), e));
                        if let Some(o) = self.ops.iter_mut().find(|o| o["operation_id"] == target) {
                            o["lifecycle"] = "terminal".into();
                            o["terminal_at_ms"] = now.into();
                        }
                    }
                    _ => {}
                }
                let r = serde_json::json!({
                    "operation_id": op, "state": "accepted", "target_operation_id": target,
                    "expected_turn_id": turn, "target_session_id": session, "slug": slug,
                    "accepted_at_ms": now, "payload_digest": "synthetic-payload-digest", "duplicate": duplicate,
                });
                (Ok(r), pushes)
            }
            _ => (Ok(serde_json::json!({})), Vec::new()),
        }
    }

    /// A dispatched peer's background attach: its own frames follow.
    fn attached(&mut self, peer: &str) -> Vec<(u64, String, Value)> {
        let Some(op) = self.ops.iter().find(|o| o["acceptance"]["adopted_session_id"] == peer) else {
            return Vec::new();
        };
        let turn = op["acceptance"]["adopted_turn_id"].clone();
        let first = self.ops.iter().filter(|o| o["acceptance"]["model_lane"].is_string()).position(|o| o["acceptance"]["adopted_session_id"] == peer);
        let mut started = self.started.clone();
        started["session_id"] = Value::String(peer.to_owned());
        started["turn_id"] = turn.clone();
        let mut out = vec![(300, "turn/started".to_owned(), started)];
        // The FIRST dispatched peer asks for an approval (the row's
        // Approve / Deny); later ones keep working.
        if first == Some(1) && !self.approvals.contains_key(peer) {
            let id = format!("01a0eb92-9444-7101-aa6f-{:012x}", self.dispatched);
            let mut r = self.requested.clone();
            r["session_id"] = Value::String(peer.to_owned());
            r["turn_id"] = turn;
            r["approval_id"] = Value::String(id.clone());
            self.approvals.insert(peer.to_owned(), id);
            out.push((1500, "approval/requested".to_owned(), r));
        }
        // The SECOND asks a question (r23's recorded question): the row's
        // answer card. A question_respond is acknowledged by its receipt; the
        // row stays blocked until the turn moves (the web's semantics).
        if first == Some(2) {
            let mut q = self.question.clone();
            q["session_id"] = Value::String(peer.to_owned());
            q["turn_id"] = op["acceptance"]["adopted_turn_id"].clone();
            q["question_id"] = Value::String(format!("01a0eb8f-7b23-7030-9f26-{:012x}", self.dispatched));
            out.push((1500, "user_question/requested".to_owned(), q));
        }
        out
    }
}

/// A9 — the `activity` scenario's sessions (suffix, title): the opened
/// session first; all scoped to the opened Profile.
const ACTIVITY_SESSIONS: &[(&str, &str)] = &[
    ("main", "Fix steer queue drop on reconnect"),
    ("fork", "Add session fork"),
    ("bump", "Bump octos-core to a6ea8505"),
    ("review", "Review PR #2566"),
    ("hydrate", "Why is hydrate slow?"),
];

/// A9 — the recorded c24b `task/list` snapshots, split by state.
fn activity_recorded_tasks() -> (Vec<Value>, Vec<Value>) {
    let all: Vec<Value> = fixture("c24b-subagent-a6ea8505.jsonl")
        .into_iter()
        .filter(|f| f.dir == "in" && f.method == "task/list")
        .flat_map(|f| f.body["tasks"].as_array().cloned().unwrap_or_default())
        .collect();
    let running = all.iter().filter(|t| t["state"] == "running").cloned().collect();
    let done = all.iter().filter(|t| t["state"] == "completed").cloned().collect();
    (running, done)
}

/// A9 — one session's `task/list` reply: `main` the recorded running
/// snapshot, `fork` the recorded completed one, `bump` the recorded entry in
/// the terminal `failed` state (derived: no failed task was recorded),
/// `review` a reply naming ANOTHER session (the fail-closed case), `hydrate`
/// a JSON-RPC error (the unavailable case).
fn activity_task_reply(session: &str) -> Result<Value, Value> {
    let (running, done) = activity_recorded_tasks();
    let suffix = session.rsplit(':').next().unwrap_or("");
    let tasks = match suffix {
        "main" => running,
        "fork" => done,
        "bump" => {
            let mut t = running.first().cloned().unwrap_or(Value::Null);
            t["state"] = Value::from("failed");
            t["status"] = Value::from("failed");
            t["error"] = Value::from("cargo build: 2 errors");
            t["summary"] = Value::from("Rebuild after the octos-core bump");
            vec![t]
        }
        "review" => {
            let profile = session.split(':').next().unwrap_or("");
            return Ok(serde_json::json!({"session_id": format!("{profile}:private"), "tasks": running}));
        }
        "hydrate" => return Err(serde_json::json!({"code": -32603, "message": "task snapshot unavailable"})),
        _ => vec![],
    };
    Ok(serde_json::json!({"session_id": session, "tasks": tasks}))
}

/// A6 — the `surfaces` scenario: what each `turn/start` replays, in order,
/// and how the stream is HELD at an interaction until the app answers it.
///
/// | # | recorded turn (r23) | exercises |
/// |---|---|---|
/// | 1 | `…23b` 17×23 | reasoning (folded thinking rows) + the answer |
/// | 2 | `…23c` echo | a real tool call (a tool row to expand) + two delivered files (`file_attached`: a PDF to download, a PNG to preview; `GET /api/files` answered on this port) |
/// | 3 | `…240` color | three `user_question/requested` in a row (the recorded single-select, a multi-select variant, one more) — each held until `user_question/respond` or `turn/interrupt` |
/// | 4 | `…241` sudo  | four `approval/requested` in a row (r5's TYPED command approval, and one typed DIFF approval whose `diff/preview/get` is answered) — each held until `approval/respond` |
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
        // Envelopes after an inserted fixture frame move up one per-thread
        // seq (the client drops a non-increasing seq as stale).
        let mut shift = 0u64;
        for f in frames.iter().filter(|f| f.dir == "in" && f.body.get("turn_id").and_then(|t| t.as_str()) == Some(turn)) {
            let mut f = f.clone();
            if shift > 0 && f.method == "projection/envelope" {
                f.body["seq"] = json!(f.body["seq"].as_u64().unwrap_or(0) + shift);
            }
            let f = &f;
            match f.method.as_str() {
                // The app answers; the decision is synthesized from ITS reply.
                "approval/decided" => continue,
                "approval/requested" if n == 5 => {
                    // Turn 6: a scope policy resolves it (no card), with the
                    // decision this turn's recording carries (its
                    // `approval/decided`), so the answer that follows agrees.
                    let b = &f.body;
                    let decision = frames
                        .iter()
                        .find(|x| x.method == "approval/decided" && x.body["approval_id"] == b["approval_id"])
                        .and_then(|x| x.body["decision"].as_str())
                        .unwrap_or("approve")
                        .to_owned();
                    out.push((
                        Frame {
                            dir: "in".into(),
                            method: "approval/auto_resolved".into(),
                            body: json!({
                                "session_id": b["session_id"], "approval_id": b["approval_id"],
                                "turn_id": b["turn_id"], "tool_name": b["tool_name"],
                                "scope": "session", "scope_match": "exact", "decision": decision
                            }),
                        },
                        None,
                    ));
                    continue;
                }
                "approval/requested" => {
                    // r5's typed command approval, re-pointed at this turn —
                    // four requests in a row, one per decision the walk
                    // makes: Deny; a typed DIFF approval (Review diff, then
                    // Approve for session); Approve once; the `S` key.
                    let mut a = r5
                        .iter()
                        .find(|x| x.dir == "in" && x.method == "approval/requested")
                        .cloned()
                        .expect("r5 records an approval");
                    super::rewrite_session(&mut a.body, &r5_session, f.body["session_id"].as_str().unwrap_or(""));
                    a.body["turn_id"] = f.body["turn_id"].clone();
                    let base = a.body["approval_id"].as_str().unwrap_or_default().to_owned();
                    let with_id = |suffix: &str| {
                        let mut x = a.clone();
                        x.body["approval_id"] = json!(format!("{}{suffix}", &base[..base.len() - 2]));
                        x
                    };
                    out.push((with_id("aa"), Some(Hold::Approval)));
                    let mut diff = with_id("ab");
                    diff.body["approval_kind"] = json!("diff");
                    diff.body["tool_name"] = json!("apply_patch");
                    diff.body["title"] = json!("Apply a patch to src/main.rs");
                    diff.body["body"] = json!("Adds the --version flag and documents it in --help.");
                    diff.body["risk"] = json!("medium");
                    diff.body["typed_details"] = json!({"kind": "diff", "diff": {
                        "preview_id": DIFF_PREVIEW, "operation": "apply_patch", "file_count": 1,
                        "additions": 4, "deletions": 1, "summary": "src/main.rs: +4 -1"
                    }});
                    out.push((diff, Some(Hold::Approval)));
                    out.push((with_id("ac"), Some(Hold::Approval)));
                    out.push((with_id("ad"), Some(Hold::Approval)));
                    continue;
                }
                "user_question/requested" => {
                    // Three questions in a row, each held until answered:
                    // the recorded single-select (Enter submits), a
                    // multi-select variant (arrow keys + Space, the submit
                    // button), and a third one the walk stops the turn on.
                    out.push((f.clone(), Some(Hold::Question)));
                    let qid = f.body["question_id"].as_str().unwrap_or_default().to_owned();
                    let with_id = |suffix: &str| {
                        let mut x = f.clone();
                        x.body["question_id"] = json!(format!("{}{suffix}", &qid[..qid.len() - 2]));
                        x
                    };
                    let mut multi = with_id("a1");
                    multi.body["title"] = json!("Which accents should the theme use?");
                    multi.body["body"] = json!("Pick any that apply; the first one leads.");
                    multi.body["questions"][0]["header"] = json!("Accents");
                    multi.body["questions"][0]["question"] = json!("Which accents should the theme use?");
                    multi.body["questions"][0]["multi_select"] = json!(true);
                    out.push((multi, Some(Hold::Question)));
                    let mut dark = with_id("a2");
                    dark.body["title"] = json!("Which color for the dark theme?");
                    dark.body["questions"][0]["header"] = json!("Dark theme");
                    dark.body["questions"][0]["question"] = json!("Which color for the dark theme?");
                    out.push((dark, Some(Hold::Question)));
                    continue;
                }
                // Turn 2: the tool's delivered file (a fixture `file_attached`
                // envelope built from the turn's own recorded tool_end frame),
                // so the transcript shows a Download row the walk can click.
                "projection/envelope" if n == 1 && f.body["payload"]["type"] == "tool_end" => {
                    out.push((f.clone(), None));
                    // A report (download) and a chart image (preview).
                    let files = [
                        ("/home/user/src/octos/out/r23-report.pdf", "application/pdf", 2048u64),
                        ("/home/user/src/octos/out/coverage.png", "image/png", fixture_png().len() as u64),
                    ];
                    for (path, mime, size) in files {
                        shift += 1;
                        let mut file = f.clone();
                        file.body["payload"] = json!({"type": "file_attached", "data": {
                            "path": path, "mime": mime, "size_bytes": size,
                            "attachment_owner": {"tool_call_id": f.body["payload"]["data"]["tool_call_id"]}
                        }});
                        file.body["seq"] = json!(f.body["seq"].as_u64().unwrap_or(0) + shift);
                        out.push((file, None));
                    }
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

    /// The diff approval's preview (r30a's recorded `diff/preview/get` id).
    pub const DIFF_PREVIEW: &str = "01920000-0000-7000-8000-0000000000f1";

    /// The scenario's request replies (`None` = the default `{}`).
    pub fn reply(method: &str, params: &Value, session: &str) -> Option<Value> {
        let task_id = params["task_id"].as_str().unwrap_or_default();
        Some(match method {
            // `DiffPreviewGetResult` (octos-core): the patch the diff
            // approval asks about (no successful reply is recorded).
            "diff/preview/get" => json!({
                "status": "ready", "source": "pending_store",
                "preview": {
                    "session_id": session, "preview_id": params["preview_id"],
                    "title": "Add a --version flag",
                    "files": [{"path": "src/main.rs", "status": "modified", "hunks": [{
                        "header": "@@ -10,6 +10,9 @@ fn main() {",
                        "lines": [
                            {"kind": "context", "content": "    let args = Args::parse();", "old_line": 10, "new_line": 10},
                            {"kind": "removed", "content": "    run(args);", "old_line": 11},
                            {"kind": "added", "content": "    if args.version {", "new_line": 11},
                            {"kind": "added", "content": "        println!(\"octos {}\", env!(\"CARGO_PKG_VERSION\"));", "new_line": 12},
                            {"kind": "added", "content": "        return;", "new_line": 13},
                            {"kind": "added", "content": "    }", "new_line": 14},
                            {"kind": "context", "content": "}", "old_line": 12, "new_line": 15}
                        ]
                    }]}]
                }
            }),
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
                // Two pages by UTF-8 byte cursor: the first read, then the
                // rest from `cursor.offset` (complete).
                let first = [
                    "Compiling octos-cli v0.24.1",
                    "Finished test [unoptimized + debuginfo] target(s) in 1.23s",
                    "Running unittests src/lib.rs",
                    "running 12 tests",
                    "test steer_queue::drains_after_reconnect ... ok",
                    "test steer_queue::keeps_order ... ok",
                ];
                let rest = [
                    "test steer_queue::replays_in_seq_order ... ok",
                    "test result: ok. 12 passed; 0 failed; 0 ignored",
                ];
                let page_1 = format!("{}\n", first.join("\n"));
                let page_2 = format!("{}\n", rest.join("\n"));
                let total = (page_1.len() + page_2.len()) as u64;
                let at = params.pointer("/cursor/offset").and_then(|o| o.as_u64()).unwrap_or(0);
                let (text, complete) = if at == 0 { (page_1, false) } else { (page_2, true) };
                let len = text.len() as u64;
                json!({
                    "session_id": session, "task_id": task_id, "source": "runtime_projection",
                    "cursor": {"offset": at}, "next_cursor": {"offset": at + len}, "text": text,
                    "bytes_read": len, "total_bytes": total.max(at + len), "truncated": !complete, "complete": complete,
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

    /// The delivered chart image: a 240x135 RGB PNG built at runtime
    /// (stored deflate + CRC-32 + Adler-32, no dependencies) — five blue bars
    /// on a light grid.
    pub fn fixture_png() -> Vec<u8> {
        let (w, h) = (240u32, 135u32);
        let heights = [60u32, 95, 75, 115, 88];
        let mut raw = Vec::with_capacity(((w * 3 + 1) * h) as usize);
        for y in 0..h {
            raw.push(0); // filter: none
            for x in 0..w {
                let bar = (x / 48) as usize;
                let in_bar = (8..40).contains(&(x % 48));
                let px = if in_bar && h - y <= heights[bar] {
                    [0x2f, 0x6f, 0xeb]
                } else if y % 27 == 0 {
                    [0xe5, 0xe5, 0xea]
                } else {
                    [0xfa, 0xfa, 0xfb]
                };
                raw.extend_from_slice(&px);
            }
        }
        let mut z = vec![0x78, 0x01];
        let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
        for (i, block) in blocks.iter().enumerate() {
            z.push(u8::from(i + 1 == blocks.len()));
            let len = block.len() as u16;
            z.extend_from_slice(&len.to_le_bytes());
            z.extend_from_slice(&(!len).to_le_bytes());
            z.extend_from_slice(block);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &x in &raw {
            a = (a + x as u32) % 65_521;
            b = (b + a) % 65_521;
        }
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());
        fn crc32(bytes: &[u8]) -> u32 {
            let mut c = 0xffff_ffffu32;
            for &x in bytes {
                c ^= x as u32;
                for _ in 0..8 {
                    c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
                }
            }
            !c
        }
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut chunk = |kind: &[u8], data: &[u8]| {
            png.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let mut body = kind.to_vec();
            body.extend_from_slice(data);
            png.extend_from_slice(&body);
            png.extend_from_slice(&crc32(&body).to_be_bytes());
        };
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&w.to_be_bytes());
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit RGB
        chunk(b"IHDR", &ihdr);
        chunk(b"IDAT", &z);
        chunk(b"IEND", &[]);
        png
    }

    /// r4's live `task/updated` frames (running, then completed), delivered
    /// after the first `task/list` so the Trajectory merges them live.
    pub fn live_task_frames() -> Vec<Frame> {
        let r4 = fixture("r4-task-a6ea8505.jsonl");
        r4.into_iter().filter(|f| f.dir == "in" && f.method == "task/updated").collect()
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
    // A9: `--task-delay-ms N` holds every activity `task/list` reply N ms (a
    // slow catalog, for the loading fallback's Cancel).
    let task_delay_ms: u64 = args
        .iter()
        .position(|a| a == "--task-delay-ms")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let scenario = args
        .iter()
        .position(|a| a == "--scenario")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_default();
    let (label, file) = scenario_fixture(&scenario);
    // A15: `history` answers the seats' and the strip's reads (the profile's
    // models, the permission list, the status stamp) from the same recordings.
    let replies = if label == "screens" || label == "a10" || label == "fleet" || label == "history" {
        screens_replies()
    } else {
        BTreeMap::new()
    };
    let sequenced = if label == "a10" { a10_sequenced() } else { BTreeMap::new() };
    let seat_sim = (label == "a10").then(SeatSim::load);
    // A10: `--slow <method>=<ms>` (repeatable) delays that method's faithful reply.
    let slow: BTreeMap<String, u64> = args
        .windows(2)
        .filter(|w| w[0] == "--slow")
        .filter_map(|w| w[1].split_once('=').and_then(|(m, ms)| Some((m.to_owned(), ms.parse().ok()?))))
        .collect();
    if !sequenced.is_empty() {
        println!(
            "[replay-serve] a10: faithful sequenced replies: {:?}",
            sequenced.iter().map(|(m, v)| format!("{m} x{}", v.len())).collect::<Vec<_>>()
        );
    }
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
    let mut open_result = recorded_open_result(&frames).expect("the fixture has a session/open result");
    // A10 seat walk: `--drop-method <m>` / `--drop-feature <f>` withdraw one
    // capability from the advertised set (the web e2e's no-method /
    // no-feature variants).
    for w in args.windows(2) {
        let key = match w[0].as_str() {
            "--drop-method" => "supported_methods",
            "--drop-feature" => "supported_features",
            _ => continue,
        };
        if let Some(list) = open_result["capabilities"][key].as_array_mut() {
            list.retain(|x| x != w[1].as_str());
            println!("[replay-serve] capabilities: {} withdrawn", w[1]);
        }
    }
    let fleet_cold = args.iter().any(|a| a == "--fleet-cold");
    let revoke_file = args.iter().position(|a| a == "--revoke-file").and_then(|i| args.get(i + 1)).cloned();
    // A16: `--fail-scoped-list N` — the first N session-scoped
    // `profile/llm/list` reads (per connection) answer an error.
    let fail_scoped_list: u64 = args
        .iter()
        .position(|a| a == "--fail-scoped-list")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let first_turn: usize = args
        .iter()
        .position(|a| a == "--first-turn")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
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
    let standalone = if label == "fleet" || label == "history" {
        // The fleet fixture's inbound frames are replies + peer-session
        // frames, never standalone notifications. A15 `history`: the
        // transcript comes from the hydrate alone.
        Vec::new()
    } else if label == "screens" || label == "a10" {
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
        let sequenced = sequenced.clone();
        let slow = slow.clone();
        let revoke_file = revoke_file.clone();
        let mut seat_sim = seat_sim.clone();
        let fleet_frames = if label == "fleet" { frames.clone() } else { Vec::new() };
        let activity = label == "activity";
        // A15: the recorded canonical hydrate (the `history` scenario).
        let history = label == "history";
        let recorded_hydrate = if history {
            frames
                .iter()
                .find(|f| f.dir == "in" && f.method == "session/hydrate")
                .map(|f| f.body.clone())
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        tokio::spawn(async move {
            // A10 fleet: the per-connection external-driver simulator.
            let workspace = open_result["workspace_root"].as_str().unwrap_or("workspace").to_owned();
            let mut fleet = (label == "fleet").then(|| {
                let mut sim = FleetSim::new(&fleet_frames, &workspace);
                if fleet_cold {
                    // A COLD master: internal, never bound (revision 0).
                    sim.external = false;
                    sim.binding = Value::Null;
                    sim.revision = 0;
                }
                sim.revoke_file = revoke_file.clone();
                sim
            });
            // A10: how many sequenced replies each method has consumed.
            let mut seq_pos: BTreeMap<String, usize> = BTreeMap::new();
            // A16: the injected session-scoped list failures still to answer.
            let mut fail_scoped_list = fail_scoped_list;
            // A6 `surfaces`: the web's delivered-file download
            // (`GET /api/files?path=…&session=…`, `media.ts:147-165`) is plain
            // HTTP on the same port; answer it with a small PDF body.
            if label == "surfaces" {
                let mut head = [0u8; 16];
                let n = stream.peek(&mut head).await.unwrap_or(0);
                if String::from_utf8_lossy(&head[..n]).starts_with("GET /api/files") {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut stream = stream;
                    let mut buf = vec![0u8; 8192];
                    let k = stream.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..k]).to_string();
                    let line = req.lines().next().unwrap_or("").to_owned();
                    println!("[replay-serve] surfaces: {line}");
                    let (ctype, body): (&str, Vec<u8>) = if line.contains(".png") {
                        ("image/png", surfaces::fixture_png())
                    } else {
                        ("application/pdf", b"%PDF-1.7\n% r23-report (fixture)\n%%EOF\n".to_vec())
                    };
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                    let _ = stream.write_all(&body).await;
                    return;
                }
            }
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else {
                return;
            };
            let (tx, mut rx) = ws.split();
            let tx = std::sync::Arc::new(tokio::sync::Mutex::new(tx));
            let mut played = first_turn;
            // A9 — session/open requests on this connection (the activity
            // scenario holds a SWITCH's session/list reply back, so the walk
            // can reopen Activity while that switch is still in flight).
            let mut opens = 0usize;
            // A6 `surfaces`: r4's live task frames go out once.
            let mut live_sent = false;
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

                // A15 `history`: the recorded history and its catalog row.
                if history {
                    if let Some(reply) = history_reply(&method, &v["params"], &recorded, &recorded_hydrate) {
                        println!("[replay-serve] -> {method} (history) {}", v["params"]["session_id"]);
                        let frame = match reply {
                            Ok(r) => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": r}),
                            Err(e) => serde_json::json!({"jsonrpc": "2.0", "id": id, "error": e}),
                        };
                        send(&tx, frame).await;
                        continue;
                    }
                }

                // A10 fleet: send `(delay, method, params)` notifications
                // after a reply, in order.
                fn push_later(
                    tx: &std::sync::Arc<tokio::sync::Mutex<futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>, Message>>>,
                    frames: Vec<(u64, String, Value)>,
                ) {
                    if frames.is_empty() {
                        return;
                    }
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        for (delay, m, params) in frames {
                            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                            println!("[replay-serve] => {m} (fleet)");
                            let frame = serde_json::json!({"jsonrpc": "2.0", "method": m, "params": params});
                            let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
                        }
                    });
                }

                // A10 fleet: a dispatched peer's background attach.
                if method == "session/open" {
                    if let (Some(sim), Some(peer)) = (fleet.as_mut(), v["params"]["session_id"].as_str().filter(|s| s.contains("#peer-"))) {
                        let peer = peer.to_owned();
                        let mut opened = open_result.clone();
                        rewrite_session(&mut opened, &recorded, &peer);
                        if let Some(obj) = opened.as_object_mut() {
                            obj.insert("session_id".to_owned(), Value::String(peer.clone()));
                        }
                        send(&tx, serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {"opened": opened}})).await;
                        let mut frames = sim.attached(&peer);
                        for f in frames.iter_mut() {
                            rewrite_session(&mut f.2, &recorded, &active_session);
                        }
                        push_later(&tx, frames);
                        continue;
                    }
                }
                if let Some(sim) = fleet.as_mut().filter(|_| FleetSim::handles(&method)) {
                    let (reply, mut pushes) = sim.reply(&method, &v["params"], &active_session);
                    let frame = match reply {
                        Ok(mut r) => {
                            rewrite_session(&mut r, &recorded, &active_session);
                            // The ids each frame carried (operation / target /
                            // expected turn / lane): the walk's wire proof.
                            let p = &v["params"];
                            let ids: Vec<String> = ["operation_id", "target_operation_id", "expected_turn_id", "model", "expected_revision", "next"]
                                .iter()
                                .filter_map(|k| p.get(*k).filter(|x| !x.is_null()).map(|x| format!("{k}={}", x.to_string().trim_matches('"'))))
                                .chain(p["command"]["kind"].as_str().map(|k| format!("command={k}")))
                                .collect();
                            println!("[replay-serve] -> {method} (fleet sim) {}", ids.join(" "));
                            serde_json::json!({"jsonrpc": "2.0", "id": id, "result": r})
                        }
                        Err(e) => {
                            let op = v["params"]["operation_id"].as_str().unwrap_or("").to_owned();
                            println!("[replay-serve] -> {method} REFUSED {} (fleet sim) operation_id={op}", e["data"]["kind"]);
                            serde_json::json!({"jsonrpc": "2.0", "id": id, "error": e})
                        }
                    };
                    send(&tx, frame).await;
                    for f in pushes.iter_mut() {
                        rewrite_session(&mut f.2, &recorded, &active_session);
                    }
                    push_later(&tx, pushes);
                    continue;
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
                            let mut frames = surfaces::turn_frames(&all_frames, played);
                            // The recorded turn plays AS the app's turn: its id
                            // (the turn/start `turn_id`, which a real server
                            // adopts) replaces the recorded one in every frame,
                            // and the canonical user message carries what the
                            // app actually sent (some recordings redact it) —
                            // so it settles onto the app's own optimistic row.
                            let recorded_turn = surfaces::TURNS[played.min(surfaces::TURNS.len() - 1)];
                            if let Some(app_turn) = params["turn_id"].as_str().filter(|t| !t.is_empty()) {
                                for (f, _) in frames.iter_mut() {
                                    let text = f.body.to_string().replace(recorded_turn, app_turn);
                                    if let Ok(v) = serde_json::from_str(&text) {
                                        f.body = v;
                                    }
                                }
                            }
                            if let Some(typed) = params["input"][0]["text"].as_str().filter(|t| !t.is_empty()) {
                                for (f, _) in frames.iter_mut() {
                                    if f.method == "projection/envelope" && f.body["payload"]["type"] == "user_message" {
                                        f.body["payload"]["data"]["text"] = serde_json::json!(typed);
                                    }
                                }
                            }
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
                                send(&tx, serde_json::json!({"jsonrpc": "2.0", "id": id, "result": body.clone()})).await;
                                // r4's live task frames merge into the list
                                // the app just folded (`applyTaskUpdated`,
                                // model.ts:104-137): once per connection,
                                // after the first authoritative task/list,
                                // re-pointed from r4's own session.
                                // The opened task's output keeps streaming:
                                // one live `task/output/delta` at the first
                                // page's `next_cursor` (r4's frame shape).
                                if m == "task/output/read" && params.pointer("/cursor/offset").is_none() {
                                    let tx2 = tx.clone();
                                    let delta = serde_json::json!({"jsonrpc": "2.0", "method": "task/output/delta", "params": {
                                        "session_id": active_session, "task_id": params["task_id"],
                                        "cursor": body["next_cursor"], "text": "test steer_queue::reconnect_resumes_the_queue ... ok\n"
                                    }});
                                    tokio::spawn(async move {
                                        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
                                        println!("[replay-serve] surfaces -> task/output/delta (live)");
                                        send(&tx2, delta).await;
                                    });
                                }
                                if m == "task/list" && !live_sent {
                                    live_sent = true;
                                    let tx2 = tx.clone();
                                    let to = active_session.clone();
                                    let from = recorded_session(&fixture("r4-task-a6ea8505.jsonl"));
                                    tokio::spawn(async move {
                                        for (i, mut f) in surfaces::live_task_frames().into_iter().enumerate() {
                                            let pause = if i == 0 { 400 } else { 1_500 };
                                            tokio::time::sleep(std::time::Duration::from_millis(pause)).await;
                                            rewrite_session(&mut f.body, &from, &to);
                                            println!("[replay-serve] surfaces -> {} (live)", f.method);
                                            send(&tx2, serde_json::json!({"jsonrpc": "2.0", "method": f.method, "params": f.body})).await;
                                        }
                                    });
                                }
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
                        opens += 1;
                        let requested = v["params"]["session_id"]
                            .as_str()
                            .unwrap_or(&recorded)
                            .to_owned();
                        active_session = requested.clone();
                        let mut opened = open_result.clone();
                        rewrite_session(&mut opened, &recorded, &requested);
                        if let Some(obj) = opened.as_object_mut() {
                            obj.insert("session_id".to_owned(), Value::String(requested));
                            // A9 — the activity scenario opens in the requested
                            // cwd, as a server does (its recording had none).
                            // A15: so does `history` (r43a's root is a
                            // `<WORKSPACE>` placeholder).
                            if activity || history {
                                if let Some(cwd) = v["params"]["cwd"].as_str() {
                                    obj.insert("workspace_root".to_owned(), Value::String(cwd.to_owned()));
                                }
                                // …under the Profile it asked for (the
                                // recording's own id would leak otherwise).
                                if let Some(p) = v["params"]["profile_id"].as_str() {
                                    obj.insert("active_profile_id".to_owned(), Value::String(p.to_owned()));
                                }
                            }
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
                    // A9 — the activity scenario's session catalog.
                    "session/list" if activity => {
                        let profile = active_session.split(':').next().unwrap_or("").to_owned();
                        let rows: Vec<Value> = ACTIVITY_SESSIONS
                            .iter()
                            .map(|(suffix, title)| serde_json::json!({
                                "id": format!("{profile}:{suffix}"),
                                "title": title,
                                "message_count": 4,
                                "updated_at": "2026-09-29T05:16:54Z",
                                "active_turn": false
                            }))
                            .collect();
                        let frame = serde_json::json!({
                            "jsonrpc": "2.0", "id": id, "result": {"sessions": rows}
                        });
                        if opens > 1 {
                            // A switch (not the first open): its open settles
                            // 6 s later, the window the walk reopens Activity in.
                            println!("[replay-serve] holding the switch's session/list for 6 s");
                            let tx2 = tx.clone();
                            tokio::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_secs(6)).await;
                                send(&tx2, frame).await;
                            });
                        } else {
                            send(&tx, frame).await;
                        }
                    }
                    "task/list" if activity => {
                        let session = v["params"]["session_id"].as_str().unwrap_or("").to_owned();
                        let frame = match activity_task_reply(&session) {
                            Ok(r) => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": r}),
                            Err(e) => serde_json::json!({"jsonrpc": "2.0", "id": id, "error": e}),
                        };
                        println!("[replay-serve] -> task/list {session}");
                        if task_delay_ms > 0 {
                            let tx2 = tx.clone();
                            tokio::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(task_delay_ms)).await;
                                send(&tx2, frame).await;
                            });
                        } else {
                            send(&tx, frame).await;
                        }
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
                    // A10 — the composer seats' simulator (scenario a10).
                    m @ ("permission/profile/list" | "permission/profile/set" | "profile/llm/select" | "review/start"
                        | "profile/llm/fetch_models" | "profile/llm/test" | "profile/llm/upsert" | "profile/llm/delete")
                        if seat_sim.is_some() =>
                    {
                        let sim = seat_sim.as_mut().expect("a10");
                        let frame = match sim.answer(m, &v["params"], &active_session) {
                            Ok(r) => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": r}),
                            Err(e) => serde_json::json!({"jsonrpc": "2.0", "id": id, "error": e}),
                        };
                        println!("[replay-serve] -> {m} (seat simulator) {}", v["params"]);
                        match slow.get(m).copied() {
                            Some(ms) => {
                                let tx2 = tx.clone();
                                tokio::spawn(async move {
                                    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                                    let _ = tx2.lock().await.send(Message::Text(frame.to_string().into())).await;
                                });
                            }
                            None => send(&tx, frame).await,
                        }
                    }
                    // A10 — the Stop control's walk: a start Core accepts
                    // only after `--slow turn/start=<ms>` (Starting…), a turn
                    // that stays live, an interrupt answered after `--slow
                    // turn/interrupt=<ms>` (Stopping…) and then r26's recorded
                    // interrupted terminal for the app's own turn.
                    "turn/start" if seat_sim.is_some() => {
                        let ms = slow.get("turn/start").copied().unwrap_or(0);
                        println!("[replay-serve] -> turn/start (a10: accepted after {ms} ms; live until interrupted) {}", v["params"]["turn_id"]);
                        let frame = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {"accepted": true}});
                        let tx2 = tx.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                            let _ = tx2.lock().await.send(Message::Text(frame.to_string().into())).await;
                        });
                    }
                    "turn/interrupt" if seat_sim.is_some() => {
                        let ms = slow.get("turn/interrupt").copied().unwrap_or(0);
                        let sim = seat_sim.as_mut().expect("a10");
                        sim.seq += 1;
                        let seq = 900_000 + sim.seq;
                        let turn = v["params"]["turn_id"].clone();
                        let mut term = sim.r26_terminal.clone();
                        term["turn_id"] = turn.clone();
                        term["thread_id"] = turn.clone();
                        term["session_id"] = Value::from(active_session.clone());
                        term["seq"] = Value::from(seq);
                        term["cursor"] = serde_json::json!({"seq": seq, "stream": active_session});
                        println!("[replay-serve] -> turn/interrupt (a10: answered after {ms} ms, then r26's interrupted terminal) {turn}");
                        let reply = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {}});
                        let note = serde_json::json!({"jsonrpc": "2.0", "method": "projection/envelope", "params": term});
                        let tx2 = tx.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                            let _ = tx2.lock().await.send(Message::Text(reply.to_string().into())).await;
                            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                            let _ = tx2.lock().await.send(Message::Text(note.to_string().into())).await;
                        });
                    }
                    "profile/llm/list" if seat_sim.is_some() => {
                        let sim = seat_sim.as_mut().expect("a10");
                        let scoped = v["params"].get("session_id").is_some();
                        // A16: `--fail-scoped-list <n>` answers the first n
                        // session-scoped reads with an error (the Session
                        // settings pane's unread state, then its Try again).
                        if scoped && fail_scoped_list > 0 {
                            fail_scoped_list -= 1;
                            println!("[replay-serve] -> profile/llm/list (session-scoped: injected ERROR, {fail_scoped_list} left)");
                            send(&tx, serde_json::json!({"jsonrpc": "2.0", "id": id,
                                "error": {"code": -32000, "message": "profile store unavailable"}})).await;
                            continue;
                        }
                        let m = if scoped { "profile/llm/list" } else { "profile/llm/list@profile" };
                        let r = sim.answer(m, &v["params"], &active_session).unwrap_or_default();
                        println!("[replay-serve] -> profile/llm/list (seat simulator, {})", if scoped { "session-scoped" } else { "profile config" });
                        send(&tx, serde_json::json!({"jsonrpc": "2.0", "id": id, "result": r})).await;
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
                    // A10 — a faithful sequenced reply (in order, the last
                    // one repeating), re-pointed at the opened session.
                    m if sequenced.contains_key(m) => {
                        let list = &sequenced[m];
                        let k = seq_pos.entry(m.to_owned()).or_insert(0);
                        let (mut body, from) = list[(*k).min(list.len() - 1)].clone();
                        *k += 1;
                        rewrite_session(&mut body, &from, &active_session);
                        // A faithful refusal (`{"__error__": {code, message,
                        // data}}`) answers as a JSON-RPC error.
                        let frame = match body.get("__error__").cloned() {
                            Some(err) => {
                                println!("[replay-serve] -> {m} (faithful ERROR #{k})");
                                serde_json::json!({"jsonrpc": "2.0", "id": id, "error": err})
                            }
                            None => {
                                println!("[replay-serve] -> {m} (faithful reply #{k})");
                                serde_json::json!({"jsonrpc": "2.0", "id": id, "result": body})
                            }
                        };
                        // `--slow <method>=<ms>`: answer this method late (a
                        // walk can then see the in-flight state), without
                        // holding up the other requests.
                        match slow.get(m).copied() {
                            Some(ms) => {
                                let tx2 = tx.clone();
                                tokio::spawn(async move {
                                    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                                    let _ = tx2.lock().await.send(Message::Text(frame.to_string().into())).await;
                                });
                            }
                            None => send(&tx, frame).await,
                        }
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
