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
        // #32b3: one synthetic turn whose fenced code block carries a 227-column
        // line — the long-code-line render capture (the web wraps: pre-wrap).
        "longcodeline" => ("longcodeline", "longcodeline-a6ea8505.jsonl"),
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

/// A10 — the `fleet` scenario's external-driver simulator. Every reply is a
/// fixture shape (`a10-fleet-driver-synthetic.jsonl`) with the per-request
/// ids echoed (operation id, requested lane, acquiring driver, control
/// target), the way `apps/web/scripts/mock-ui-server.mjs` answers its
/// `peer-control-*` workspaces; the binding's revision moves on acquire /
/// release, dispatches join the walked inventory, and a dispatched peer's
/// background attach gets its session's frames (turn/started; the FIRST
/// peer then asks for an approval). `lane-review` is listed but answers
/// `driver_model_unavailable` (the fixture's typed refusal frame: a lane
/// whose credentials the server lacks), so the refusal path is clickable.
struct FleetSim {
    get: Value,
    acquire: Value,
    lanes: Value,
    started: Value,
    requested: Value,
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
        };
        let ws = sim.workspace.clone();
        for v in [&mut sim.get, &mut sim.binding] {
            rewrite_session(v, "<WORKSPACE>", &ws);
        }
        for op in sim.ops.iter_mut() {
            rewrite_session(op, "<WORKSPACE>", &ws);
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
                let mut v = serde_json::json!({
                    "mode": "external", "recovery": "none", "binding": self.binding,
                });
                if p.get("operations").is_some() {
                    v["operations"] = serde_json::json!({
                        "items": self.ops,
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
        out
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
    let replies =
        if label == "screens" || label == "a10" || label == "fleet" { screens_replies() } else { BTreeMap::new() };
    let sequenced = if label == "a10" { a10_sequenced() } else { BTreeMap::new() };
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
    let standalone = if label == "fleet" {
        // The fleet fixture's inbound frames are replies + peer-session
        // frames, never standalone notifications.
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
        let fleet_frames = if label == "fleet" { frames.clone() } else { Vec::new() };
        tokio::spawn(async move {
            // A10 fleet: the per-connection external-driver simulator.
            let workspace = open_result["workspace_root"].as_str().unwrap_or("workspace").to_owned();
            let mut fleet = (label == "fleet").then(|| FleetSim::new(&fleet_frames, &workspace));
            // A10: how many sequenced replies each method has consumed.
            let mut seq_pos: BTreeMap<String, usize> = BTreeMap::new();
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else {
                return;
            };
            let (tx, mut rx) = ws.split();
            let tx = std::sync::Arc::new(tokio::sync::Mutex::new(tx));
            let mut played = 0usize;
            // The session id the app opens; every served frame is rewritten to it.
            let mut active_session = recorded.clone();

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
                            let ids: Vec<String> = ["operation_id", "target_operation_id", "expected_turn_id", "model", "expected_revision"]
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
                        let notifs = standalone.clone();
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
                    // A10 — a faithful sequenced reply (in order, the last
                    // one repeating), re-pointed at the opened session.
                    m if sequenced.contains_key(m) => {
                        let list = &sequenced[m];
                        let k = seq_pos.entry(m.to_owned()).or_insert(0);
                        let (mut body, from) = list[(*k).min(list.len() - 1)].clone();
                        *k += 1;
                        rewrite_session(&mut body, &from, &active_session);
                        println!("[replay-serve] -> {m} (faithful reply #{k})");
                        send(&tx, serde_json::json!({
                            "jsonrpc": "2.0", "id": id, "result": body
                        })).await;
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
