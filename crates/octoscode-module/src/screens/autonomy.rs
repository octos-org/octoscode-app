//! #30b — Stage C wiring for board 3.3 Goal, 3.4 Loops, 3.5 Monitors
//! (`design/stage-b/autonomy/cards/autonomy-{03,04,05}`).
//!
//! Same door rule as the other screens (8.8 condition 2): the cards see
//! binding/action ids only. This module owns the autonomy table — action ids
//! live ONLY here (one-owner rule, LESSONS) — with a pure [`resolve`], an
//! async [`apply`] over the production client
//! ([`Conversation::client`] → `octoscode_client::Client::request`), and
//! [`spawn`] for the UI thread.
//!
//! Behaviour citations (docs/parity-matrix.csv autonomy rows):
//! * 42 — goal read: `session/goal/get` (`AutonomyPanel.tsx:114`,
//!   `store.ts:358`);
//! * 43 — goal set: objective + optional token budget, blank omitted
//!   (`AutonomyPanel.tsx:194,219`, `model.ts:195` strict optional parsing);
//! * 44 — goal clear (`AutonomyPanel.tsx:174`, `store.ts:503`);
//! * 45 — pause/resume/stop is a TWO-STEP read-then-set: fresh `goal/get`,
//!   then `goal/set` with the same objective and the target `status`
//!   (`store.ts:545-560` "TUI semantics: fresh goal/get, then user goal/set";
//!   status param: `packages/client/src/autonomy.ts:292-294`);
//! * 46 — generation admission on every notification/result
//!   (`model.ts:243`, `autonomy.ts:528`);
//! * 47 — loops list + per-action pause/resume/delete/fire-now
//!   (`AutonomyPanel.tsx:235,262,305`, `store.ts:625`);
//! * 49 — zero-token monitors: list + pause/resume/delete
//!   (`AutonomyPanel.tsx:335,410`, `store.ts:699`);
//! * 50 — monitor create: name + argv array + optional filter regex, poll
//!   mode (`AutonomyPanel.tsx:80`, `model.ts:215`).
//!
//! The recorded traffic is `crates/octoscode-client/tests/fixtures/r1-autonomy-a6ea8505.jsonl`
//! (goal/loop methods) and `c24b-subagent-a6ea8505.jsonl` (monitor/create):
//! the recorded shapes — goal `{"goal_id":"goal_01","objective":"r1 replay
//! probe","status":"active","token_budget":100000000,...}`, loop `loop_01`
//! fixed_interval 3600, monitor `monitor_01` poll/ERROR — are what the
//! replay tests assert (hermetic: fixture values only, no environment).
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use serde_json::{json, Value};

use octoscode_store::Store;

use crate::bindings::Ctx;
use crate::flow::{Conversation, Direction};

/// The board-3 data slots (autonomy cards 03/04/05).
pub const BINDINGS: &[(&str, &str)] = &[
    ("goal.objective", "text: the active goal's objective (t_goal); null when none"),
    ("goal.status", "text: active | paused | complete"),
    ("goal.budget", "text: 'used / budget tokens' (model.ts:126); null without a goal"),
    ("goal.elapsed", "text: the goal's time_used_seconds, '0s'-style"),
    ("goal.can_transition", "bool: status is active|paused (goalCanTransition — pause/resume/stop enabled)"),
    ("goal.available", "bool: the session advertised session/goal/get (fail closed)"),
    ("loops", "list: [{id,name,cadence,status}] from loop/list"),
    ("loops.available", "bool: the session advertised loop/list"),
    ("monitors", "list: [{id,name,cmd,status,interval}] from monitor/list"),
    ("monitors.available", "bool: the session advertised monitor/list"),
    ("monitors.footer", "text: 'N monitors · M active' summary line"),
];

/// The autonomy action ids. Owned ONLY by this table (one-owner rule).
pub const ACTIONS: &[(&str, &str)] = &[
    ("goal.refresh", "session/goal/get"),
    ("goal.set", "session/goal/set with the entry text as objective (budget optional: 'objective | 2000')"),
    ("goal.clear", "session/goal/clear"),
    ("goal.pause", "two-step: fresh goal/get, then goal/set status=paused (store.ts:545-560)"),
    ("goal.resume", "two-step: fresh goal/get, then goal/set status=active"),
    ("goal.stop", "two-step: fresh goal/get, then goal/set status=complete"),
    ("loops.refresh", "loop/list"),
    ("loop.pause", "loop/pause for the row's loop id (index into the cached list)"),
    ("loop.resume", "loop/resume for the row's loop id"),
    ("loop.delete", "loop/delete for the row's loop id"),
    ("loop.fire_now", "loop/fire_now for the row's loop id"),
    ("monitors.refresh", "monitor/list"),
    ("monitor.pause", "monitor/pause for the row's monitor id"),
    ("monitor.resume", "monitor/resume for the row's monitor id"),
    ("monitor.delete", "monitor/delete for the row's monitor id"),
    ("monitor.create", "monitor/create: value 'name | <argv json> | [filter regex]' (poll mode)"),
];

/// The action ids [`resolve`] routes.
pub const ROUTED: &[&str] = &[
    "goal.refresh",
    "goal.set",
    "goal.clear",
    "goal.pause",
    "goal.resume",
    "goal.stop",
    "loops.refresh",
    "loop.pause",
    "loop.resume",
    "loop.delete",
    "loop.fire_now",
    "monitors.refresh",
    "monitor.pause",
    "monitor.resume",
    "monitor.delete",
    "monitor.create",
];

pub fn is_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id)
}

pub fn is_routed(id: &str) -> bool {
    ROUTED.contains(&id)
}

/// The screen's UI-local cache: the last read results (the protocol pushes
/// goal/loop/monitor notifications, but the lists are read-on-demand — the
/// web keeps the same in `store.ts` state).
#[derive(Default)]
pub struct AutonomyState {
    pub goal: Option<Value>,
    pub goal_generation: u64,
    pub loops: Vec<Value>,
    pub monitors: Vec<Value>,
}

fn state() -> MutexGuard<'static, AutonomyState> {
    static STATE: OnceLock<Mutex<AutonomyState>> = OnceLock::new();
    STATE
        .get_or_init(|| Mutex::new(AutonomyState::default()))
        .lock()
        .unwrap()
}

/// Clear the screen cache (tests share the process-global; a reconnect can
/// legitimately start from an empty slate).
pub fn reset_state() {
    *state() = AutonomyState::default();
}

fn advertised(store: &Store, method: &str) -> bool {
    store.capabilities().iter().any(|c| c == method)
}

/// The concrete thing the module does for a routed autonomy action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    RefreshGoal,
    /// `session/goal/set` — objective + optional token budget.
    SetGoal { objective: String, token_budget: Option<u64> },
    ClearGoal,
    /// Two-step transition to `paused` | `active` | `complete`.
    Transition(String),
    RefreshLists,
    LoopPause(String),
    LoopResume(String),
    LoopDelete(String),
    LoopFireNow(String),
    MonitorPause(String),
    MonitorResume(String),
    MonitorDelete(String),
    /// `monitor/create` in poll mode: name, argv array, optional filter regex.
    MonitorCreate { name: String, argv: Vec<String>, filter_regex: Option<String> },
    Unhandled(String),
}

/// Route one autonomy action. Pure. Never panics: a gated-out or unresolvable
/// id is [`Effect::Unhandled`] (logged by name, LESSONS 6).
///
/// `index` selects the loop/monitor row; `value` carries the entry text for
/// `goal.set` / `monitor.create`.
pub fn resolve(action: &str, index: usize, value: Option<&str>, ctx: &Ctx<'_>) -> Effect {
    // Fail closed per family: an unadvertised method never leaves the module
    // (the web renders these sections only when advertised —
    // AutonomyPanel.test.tsx "renders the goal section only when goal methods
    // are advertised").
    let method_ok = match action {
        "goal.refresh" | "goal.set" | "goal.clear" | "goal.pause" | "goal.resume" | "goal.stop" => {
            advertised(ctx.store, "session/goal/get") && advertised(ctx.store, "session/goal/set")
        }
        "loops.refresh" | "loop.pause" | "loop.resume" | "loop.delete" | "loop.fire_now" => {
            advertised(ctx.store, "loop/list")
        }
        "monitors.refresh" | "monitor.pause" | "monitor.resume" | "monitor.delete"
        | "monitor.create" => advertised(ctx.store, "monitor/list"),
        _ => false,
    };
    if !method_ok {
        return Effect::Unhandled(format!("{action}[not-advertised]"));
    }
    match action {
        "goal.refresh" => Effect::RefreshGoal,
        "goal.set" => {
            let Some(text) = value.map(str::trim).filter(|t| !t.is_empty()) else {
                return Effect::Unhandled("goal.set[empty]".to_owned());
            };
            // Strict optional budget (model.ts:195): "objective | 2000" sets
            // one; a bare objective omits the field entirely (never defaulted).
            match text.rsplit_once(" | ") {
                Some((objective, budget)) => match budget.trim().parse::<u64>() {
                    Ok(budget) => Effect::SetGoal {
                        objective: objective.trim().to_owned(),
                        token_budget: Some(budget),
                    },
                    Err(_) => Effect::Unhandled(format!("goal.set[budget={budget:?}]")),
                },
                None => Effect::SetGoal {
                    objective: text.to_owned(),
                    token_budget: None,
                },
            }
        }
        "goal.clear" => Effect::ClearGoal,
        "goal.pause" => Effect::Transition("paused".to_owned()),
        "goal.resume" => Effect::Transition("active".to_owned()),
        "goal.stop" => Effect::Transition("complete".to_owned()),
        "loops.refresh" => Effect::RefreshLists,
        "loop.pause" => id_at(&state().loops, index).map_or_else(
            || Effect::Unhandled(format!("{action}[{index}]")),
            Effect::LoopPause,
        ),
        "loop.resume" => id_at(&state().loops, index).map_or_else(
            || Effect::Unhandled(format!("{action}[{index}]")),
            Effect::LoopResume,
        ),
        "loop.delete" => id_at(&state().loops, index).map_or_else(
            || Effect::Unhandled(format!("{action}[{index}]")),
            Effect::LoopDelete,
        ),
        "loop.fire_now" => id_at(&state().loops, index).map_or_else(
            || Effect::Unhandled(format!("{action}[{index}]")),
            Effect::LoopFireNow,
        ),
        "monitors.refresh" => Effect::RefreshLists,
        "monitor.pause" => id_at(&state().monitors, index).map_or_else(
            || Effect::Unhandled(format!("{action}[{index}]")),
            Effect::MonitorPause,
        ),
        "monitor.resume" => id_at(&state().monitors, index).map_or_else(
            || Effect::Unhandled(format!("{action}[{index}]")),
            Effect::MonitorResume,
        ),
        "monitor.delete" => id_at(&state().monitors, index).map_or_else(
            || Effect::Unhandled(format!("{action}[{index}]")),
            Effect::MonitorDelete,
        ),
        "monitor.create" => {
            // value: "name | argv json | [filter]" — the argv must parse as a
            // JSON string array (model.ts:215 validates before the request).
            let Some(text) = value.map(str::trim).filter(|t| !t.is_empty()) else {
                return Effect::Unhandled("monitor.create[empty]".to_owned());
            };
            let mut parts = text.splitn(3, " | ");
            let Some(name) = parts.next().map(str::trim).filter(|n| !n.is_empty()) else {
                return Effect::Unhandled("monitor.create[no-name]".to_owned());
            };
            let Some(argv_json) = parts.next() else {
                return Effect::Unhandled("monitor.create[no-argv]".to_owned());
            };
            let Ok(argv) = serde_json::from_str::<Vec<String>>(argv_json) else {
                return Effect::Unhandled("monitor.create[argv-not-array]".to_owned());
            };
            if argv.is_empty() {
                return Effect::Unhandled("monitor.create[argv-empty]".to_owned());
            }
            let filter_regex = parts
                .next()
                .map(str::trim)
                .filter(|f| !f.is_empty())
                .map(str::to_owned);
            Effect::MonitorCreate {
                name: name.to_owned(),
                argv,
                filter_regex,
            }
        }
        other => Effect::Unhandled(other.to_owned()),
    }
}

fn id_at(list: &[Value], index: usize) -> Option<String> {
    list.get(index)?["loop_id"]
        .as_str()
        .or_else(|| list.get(index)?["monitor_id"].as_str())
        .map(str::to_owned)
}

/// Perform the effect against the production client (async; spawn from the UI
/// thread, await from tests/replays).
pub async fn apply(effect: Effect, conv: &Conversation) -> Result<(), String> {
    let client = conv.client();
    let ids = json!({"profile_id": conv.profile(), "session_id": conv.session_id()});
    match effect {
        Effect::RefreshGoal => {
            let result = client
                .request("session/goal/get", ids)
                .await
                .map_err(|e| e.to_string())?;
            record(conv, "session/goal/get", None);
            // Generation admission (parity 46): a stamp older than the one we
            // hold cannot regress the screen.
            let generation = result["generation"].as_u64().unwrap_or(0);
            let mut st = state();
            if admits(st.goal_generation, generation) {
                st.goal = result["goal"].as_object().map(|o| Value::Object(o.clone()));
                st.goal_generation = generation;
            }
            Ok(())
        }
        Effect::SetGoal {
            objective,
            token_budget,
        } => {
            let mut params = ids.clone();
            params["objective"] = json!(objective);
            if let Some(budget) = token_budget {
                params["token_budget"] = json!(budget);
            }
            params["transition_actor"] = json!("user");
            let result = client
                .request("session/goal/set", params)
                .await
                .map_err(|e| e.to_string())?;
            record(conv, "session/goal/set", result["goal"]["goal_id"].as_str());
            let generation = result["generation"].as_u64().unwrap_or(0);
            let mut st = state();
            if admits(st.goal_generation, generation) {
                st.goal = result["goal"].as_object().map(|o| Value::Object(o.clone()));
                st.goal_generation = generation;
            }
            Ok(())
        }
        Effect::ClearGoal => {
            let result = client
                .request("session/goal/clear", ids)
                .await
                .map_err(|e| e.to_string())?;
            record(
                conv,
                "session/goal/clear",
                result["cleared"].as_bool().map(|_| "clear"),
            );
            let generation = result["generation"].as_u64().unwrap_or(0);
            let mut st = state();
            if admits(st.goal_generation, generation) {
                st.goal = None;
                st.goal_generation = generation;
            }
            Ok(())
        }
        // Parity 45: TUI semantics — a FRESH goal/get, then goal/set carrying
        // the fresh objective and the target status (store.ts:545-560).
        Effect::Transition(status) => {
            let fresh = client
                .request("session/goal/get", ids.clone())
                .await
                .map_err(|e| e.to_string())?;
            let Some(objective) = fresh["goal"]["objective"].as_str().map(str::to_owned) else {
                return Err("There is no current unfinished goal to change.".to_owned());
            };
            let mut params = ids;
            params["objective"] = json!(objective);
            params["status"] = json!(status);
            params["transition_actor"] = json!("user");
            let result = client
                .request("session/goal/set", params)
                .await
                .map_err(|e| e.to_string())?;
            record(conv, "session/goal/set", Some(status.as_str()));
            let generation = result["generation"].as_u64().unwrap_or(0);
            let mut st = state();
            if admits(st.goal_generation, generation) {
                st.goal = result["goal"].as_object().map(|o| Value::Object(o.clone()));
                st.goal_generation = generation;
            }
            Ok(())
        }
        Effect::RefreshLists => {
            let loops = client
                .request("loop/list", ids.clone())
                .await
                .map_err(|e| e.to_string())?;
            record(conv, "loop/list", None);
            let monitors = client
                .request("monitor/list", ids)
                .await
                .map_err(|e| e.to_string())?;
            record(conv, "monitor/list", None);
            let mut st = state();
            st.loops = loops["loops"].as_array().cloned().unwrap_or_default();
            st.monitors = monitors["monitors"].as_array().cloned().unwrap_or_default();
            Ok(())
        }
        Effect::LoopPause(id) => simple(conv, "loop/pause", "loop_id", &id).await,
        Effect::LoopResume(id) => simple(conv, "loop/resume", "loop_id", &id).await,
        Effect::LoopDelete(id) => simple(conv, "loop/delete", "loop_id", &id).await,
        Effect::LoopFireNow(id) => simple(conv, "loop/fire_now", "loop_id", &id).await,
        Effect::MonitorPause(id) => simple(conv, "monitor/pause", "monitor_id", &id).await,
        Effect::MonitorResume(id) => simple(conv, "monitor/resume", "monitor_id", &id).await,
        Effect::MonitorDelete(id) => simple(conv, "monitor/delete", "monitor_id", &id).await,
        Effect::MonitorCreate {
            name,
            argv,
            filter_regex,
        } => {
            // The recorded create body (c24b-subagent-a6ea8505.jsonl): poll
            // mode + argv array + optional filter_regex.
            let mut params = json!({
                "name": name,
                "argv": argv,
                "mode": "poll",
                "interval_seconds": 1,
                "batch_ms": Value::Null,
                "goal_id": Value::Null,
                "max_events_per_hour": Value::Null,
                "persistent": Value::Null,
                "timeout_secs": Value::Null,
                "session_id": conv.session_id(),
            });
            if let Some(filter) = filter_regex {
                params["filter_regex"] = json!(filter);
            }
            let result = client
                .request("monitor/create", params)
                .await
                .map_err(|e| e.to_string())?;
            record(conv, "monitor/create", result["monitor"]["monitor_id"].as_str());
            Ok(())
        }
        Effect::Unhandled(id) => {
            ::log::warn!("octoscode: unhandled screen action {id:?}");
            Ok(())
        }
    }
}

async fn simple(conv: &Conversation, method: &str, key: &str, id: &str) -> Result<(), String> {
    conv.client()
        .request(method, json!({key: id}))
        .await
        .map_err(|e| e.to_string())?;
    record(conv, method, Some(id));
    Ok(())
}

fn record(conv: &Conversation, method: &str, note: Option<&str>) {
    conv.trace.record(
        std::time::Instant::now(),
        Direction::Out,
        method,
        None,
        note.map(str::to_owned),
    );
}

/// Generation admission (parity 46, `autonomy.ts:528`): generation 0 is an
/// unstamped legacy backend and always applies; otherwise only a result at
/// least as new as the one we hold may land.
pub fn admits(held: u64, incoming: u64) -> bool {
    held == 0 || incoming >= held
}

/// Spawn the effect on the module's runtime (the UI-thread entry).
pub fn spawn(effect: Effect, rt: &tokio::runtime::Runtime, conv: Arc<Conversation>) {
    rt.spawn(async move {
        if let Err(e) = apply(effect, &conv).await {
            ::log::warn!("octoscode: autonomy action: {e}");
        }
    });
}

/// The web's `formatTokens` (model.ts:129-136): ≥1M → "…M", ≥1k → "…k",
/// one decimal only when the division is fractional.
fn format_tokens(value: u64) -> String {
    let trim = |v: f64| {
        if v.fract() == 0.0 {
            format!("{}", v as u64)
        } else {
            format!("{v:.1}")
        }
    };
    if value >= 1_000_000 {
        format!("{}M", trim(value as f64 / 1_000_000.0))
    } else if value >= 1_000 {
        format!("{}k", trim(value as f64 / 1_000.0))
    } else {
        value.to_string()
    }
}

fn cadence(loop_row: &Value) -> String {    match loop_row["mode"].as_str() {
        Some("fixed_interval") => format!("every {}s", loop_row["interval_seconds"].as_u64().unwrap_or(0)),
        Some(other) => other.to_owned(),
        None => "unknown".to_owned(),
    }
}

/// Resolve an autonomy binding id. `None` when undeclared. Always JSON.
pub fn query(ctx: &Ctx<'_>, id: &str) -> Option<Value> {
    let store = ctx.store;
    let st = state();
    Some(match id {
        "goal.objective" => json!(st.goal.as_ref().map(|g| g["objective"].clone())),
        "goal.status" => json!(st.goal.as_ref().map(|g| g["status"].clone())),
        // model.ts:126/129-136 — 'used / budget' with formatTokens (≥1M → M,
        // ≥1k → k). The card's label already names the unit ("Token budget"),
        // so the web's " tokens" suffix is dropped to fit the measured Stage B
        // slot (the raw 19-char string clipped: 30b-live-goal.png round 1).
        "goal.budget" => json!(st.goal.as_ref().and_then(|g| {
            let used = g["tokens_used"].as_u64()?;
            let budget = g["token_budget"].as_u64().filter(|b| *b > 0)?;
            Some(format!("{} / {}", format_tokens(used), format_tokens(budget)))
        })),
        "goal.elapsed" => json!(st.goal.as_ref().map(|g| {
            format!("{}s", g["time_used_seconds"].as_u64().unwrap_or(0))
        })),
        "goal.can_transition" => json!(st
            .goal
            .as_ref()
            .map(|g| matches!(g["status"].as_str(), Some("active") | Some("paused")))
            .unwrap_or(false)),
        "goal.available" => json!(advertised(store, "session/goal/get")),
        "loops" => json!(st
            .loops
            .iter()
            .map(|l| json!({
                "id": l["loop_id"],
                "name": l["prompt"],
                "cadence": cadence(l),
                "status": l["status"],
            }))
            .collect::<Vec<Value>>()),
        "loops.available" => json!(advertised(store, "loop/list")),
        "monitors" => json!(st
            .monitors
            .iter()
            .map(|m| json!({
                "id": m["monitor_id"],
                "name": m["name"],
                "cmd": m["argv"],
                "status": m["status"],
                "interval": format!("{}s", m["interval_seconds"].as_u64().unwrap_or(0)),
            }))
            .collect::<Vec<Value>>()),
        "monitors.available" => json!(advertised(store, "monitor/list")),
        "monitors.footer" => {
            let total = st.monitors.len();
            let active = st
                .monitors
                .iter()
                .filter(|m| m["status"].as_str() == Some("active"))
                .count();
            json!(format!("{total} monitors · {active} active"))
        }
        _ => return None,
    })
}
