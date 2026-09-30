//! Entry #30c — Stage C wiring for board **3.6 Fleet** (`autonomy-06`) and
//! **3.7 Tasks** (`autonomy-07`).
//!
//! The module owns the screens' meaning, the way `screens::models` does for
//! board 2 (8.8 condition 2): the Stage-B cards are DATA — `copy` slots and
//! `service-actions.json` control events — and this file owns both directions:
//!
//! - **bindings** — [`query_binding`] turns store state into the JSON a copy
//!   slot shows ([`COPY_SLOTS`] is the declared slot→id table); an arm that
//!   cannot derive a value returns `Null` so the authored Stage-B copy stays
//!   (the models.rs contract);
//! - **actions** — [`action_params`] is the PURE
//!   `(action, row, store, steer_text) → (method, params)` table, and
//!   [`spawn`] executes it through the production client
//!   (`Conversation::client()` → `Client::request`, the same generic path the
//!   web's `client.ts` request takes).
//!
//! ## Web oracle per behaviour (fleet / peers / supervision rows)
//!
//! - *Fleet destination + rows* — `FleetView.tsx:228-470` (parity row 109):
//!   the roster count includes peers that finished this session ("terminal
//!   rows … stay for the session's lifetime", `fleet-model.ts:20-24`), so the
//!   title counts ALL staged peers (`peer-manager.ts:763` roster counts).
//! - *Status-word projection* (parity row 110, `fleet-model.ts:32-59`'s
//!   10-word vocabulary) — the native store can honestly claim only two of
//!   those words today: a peer the model closed is `Done`
//!   (`peer/closed` → `Peer.closed`), a staged open peer is still working.
//!   Other slots return `Null` (authored copy stays); the full projection is
//!   reported as the remaining gap, not guessed.
//! - *Peer row actions* (parity row 114): **Steer = one `peer/control` steer
//!   frame**. The web's envelope carries the driver-seat capture
//!   (`session_id, driver_id, epoch, control_token, operation_id,
//!   target_operation_id, expected_turn_id, command`,
//!   `external-driver-peer-control.ts:694-704`); the native store has no
//!   driver-seat capture yet, so the frame carries the subset the store owns —
//!   `session_id` + `slug` + `command {kind:"steer", input:[{kind:"text"}]}`
//!   (the command shape is `:99/112-113/147`). The client's `PeerControl`
//!   method mirrors the web's `Value` params on purpose
//!   (`domains/peer.rs:226-235`). The gap is reported.
//! - *Cancel a cancellable task* (parity row 348) — `task/cancel` with
//!   `{task_id, session_id}` (`use-supervision.ts:307-329`,
//!   `client.ts:612`; octos-core `TaskCancelParams` @ 4231669:2530-2536).
//! - *Read live task output* (parity row 346) — `task/output/read` with a
//!   byte cursor: `{session_id, task_id, cursor:{offset:0}, limit_bytes}` —
//!   the exact request the c24b recording captured. `task.open.running`
//!   re-issues it for the running card.
//! - *Tasks rows* (parity row 345) — `task/list` +
//!   live `task/updated` merges (`supervision/model.ts:104`, the domain's own
//!   `TaskUpdatedHandler`); this file's `fold_task_updated` mirrors that
//!   mapping for the replay test.
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use octoscode_store::domains::autonomy::{GoalRecord, GoalState};
use octoscode_store::domains::peer::Peer;
use octoscode_store::domains::task::TaskSnapshot;
use octoscode_store::Store;

use crate::bindings::Ctx;
use crate::flow::{Conversation, FlowUi};

/// The two cards this screen owns: Stage-B id → title
/// (`design/stage-b/autonomy/cards/autonomy-{06,07}/contract.json`).
pub const SCREENS: &[(&str, &str)] = &[("autonomy-06", "Fleet"), ("autonomy-07", "Tasks")];

/// The declared copy-slot → binding-id table (`copy id`, `binding id`), read
/// off each card's authored `page.card`. Rows the store cannot derive map to a
/// binding whose arm returns `Null` — the authored copy stays.
pub const COPY_SLOTS: &[(&str, &str)] = &[
    // autonomy-06 Fleet
    ("t_title_text", "fleet.title"),
    ("fleet_goal_label_text", "fleet.goal"),
    ("peer_1_name_text", "fleet.peer1.name"),
    ("peer_1_status_text", "fleet.peer1.status"),
    ("peer_1_meta_text", "fleet.peer1.meta"),
    ("peer_2_name_text", "fleet.peer2.name"),
    ("peer_2_status_text", "fleet.peer2.status"),
    ("peer_2_meta_text", "fleet.peer2.meta"),
    ("peer_3_name_text", "fleet.peer3.name"),
    ("peer_3_status_text", "fleet.peer3.status"),
    ("peer_3_meta_text", "fleet.peer3.meta"),
    // autonomy-07 Tasks
    ("t_cmd_text", "tasks.run_cmd"),
    ("t_run_text", "tasks.run_status"),
    ("t_run_dur_text", "tasks.run_dur"),
    ("t_log0_text", "tasks.log0"),
    ("t_log1_text", "tasks.log1"),
    ("t_log2_text", "tasks.log2"),
    ("t_log3_text", "tasks.log3"),
    ("t_done_cmd_text", "tasks.done_cmd"),
    ("t_done_text", "tasks.done_status"),
    ("t_dur_text", "tasks.done_dur"),
];

/// The control events `service-actions.json` declares for the two cards
/// (autonomy-06: `peer_{1,2,3}_steer` → event `peer.steer`; autonomy-07:
/// `cancel` → `task.cancel`, `run_card` → `task.open.running`). The row-level
/// control names are owned too so the identical `peer.steer` event can never
/// be ambiguous (one-owner rule).
pub fn owns(action: &str) -> bool {
    matches!(
        action,
        "peer.steer" | "peer_1_steer" | "peer_2_steer" | "peer_3_steer" | "task.cancel"
            | "task.open.running"
    )
}

/// Whether `id` is one of this screen set's action ids (the lib.rs
/// `perform_action` router's arm).
pub fn is_action(id: &str) -> bool {
    owns(id)
}

// ------------------------------------------------------------------- bindings

fn peer_rows(store: &Store) -> Vec<Peer> {
    // The store's roster is a map (unordered); the card's rows must be
    // DETERMINISTIC across runs (a row maps to a steer target), so the
    // projection orders by slug.
    let mut rows = store.domains.peer.list();
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows
}

fn running_task(store: &Store) -> Option<TaskSnapshot> {
    store
        .domains
        .task
        .snapshots()
        .into_iter()
        .find(|t| t.state == "running")
}

fn settled_task(store: &Store) -> Option<TaskSnapshot> {
    store
        .domains
        .task
        .snapshots()
        .into_iter()
        .find(|t| t.state != "running")
}

/// The wire runtime state → the design's status word. The web maps wire
/// states to product words the same way (`supervision/plan.ts:70`
/// `planStatusLabel("completed") -> "Done"`; the plan labels are the Tasks
/// card's own vocabulary), and the card's authored copy is capitalized
/// ("Running" / "Done") — a raw wire state must not paint lowercase.
fn status_word(state: &str) -> String {
    match state {
        "pending" => "Pending".to_owned(),
        "running" => "Running".to_owned(),
        "done" | "completed" => "Done".to_owned(),
        "failed" => "Failed".to_owned(),
        "cancelled" | "canceled" => "Stopped".to_owned(),
        other => other.to_owned(),
    }
}

fn task_label(t: &TaskSnapshot) -> String {
    t.title
        .clone()
        .or_else(|| t.summary.clone())
        .unwrap_or_else(|| t.tool_name.clone())
}

/// A task row's output, split to one line (`task/output/delta` folds land in
/// `Tasks.output` via the domain's `TaskOutputDeltaHandler`).
fn output_line(store: &Store, task_id: &str, line: usize) -> Option<String> {
    store
        .domains
        .task
        .output(task_id)
        .lines()
        .nth(line)
        .map(str::to_owned)
}

/// Resolve a binding id. `Some(Value::Null)` = "this slot has no live value
/// on this store" — the authored copy stays. `None` = the id is not declared.
pub fn query_binding(ctx: &Ctx<'_>, id: &str) -> Option<Value> {
    let store = ctx.store;
    Some(match id {
        // The roster count: every staged peer this session ("…stay for the
        // session's lifetime", fleet-model.ts:20-24; peer-manager.ts:763).
        "fleet.title" => {
            let n = peer_rows(store).len();
            if n == 0 {
                Value::Null
            } else {
                json!(format!("Fleet · {n} peer{}", if n == 1 { "" } else { "s" }))
            }
        }
        "fleet.goal" => match store
            .active_session()
            .and_then(|s| store.domains.autonomy.goal(&s))
        {
            Some(g) => json!(g.objective),
            None => Value::Null,
        },
        // peer_{N}_* — the store's staged order (the web orders by status rank,
        // parity row 111; the native store cannot rank what it cannot see yet).
        other @ ("fleet.peer1.name"
        | "fleet.peer2.name"
        | "fleet.peer3.name"
        | "fleet.peer1.status"
        | "fleet.peer2.status"
        | "fleet.peer3.status"
        | "fleet.peer1.meta"
        | "fleet.peer2.meta"
        | "fleet.peer3.meta") => {
            let row: usize = other.trim_start_matches("fleet.peer").chars().next()?.to_digit(10)? as usize - 1;
            let p = peer_rows(store).into_iter().nth(row)?;
            match other.rsplit('.').next()? {
                "name" => json!(p.name),
                // Closed by the model → the one terminal word the store owns.
                "status" if p.closed => json!("Done"),
                // Open peers: the remaining §4.3 words need phase facts the
                // native store does not carry (parity row 110) — authored stays.
                "status" => Value::Null,
                // Elapsed/tokens are not in `Peer` — authored stays.
                _ => Value::Null,
            }
        }
        // ---- autonomy-07 Tasks ------------------------------------------------
        "tasks.run_cmd" => running_task(store).map_or(Value::Null, |t| json!(task_label(&t))),
        "tasks.run_status" => running_task(store).map_or(Value::Null, |t| json!(status_word(&t.state))),
        // No wall-clock durations in the store — authored stays.
        "tasks.run_dur" | "tasks.done_dur" => Value::Null,
        "tasks.done_cmd" => settled_task(store).map_or(Value::Null, |t| json!(task_label(&t))),
        "tasks.done_status" => settled_task(store).map_or(Value::Null, |t| json!(status_word(&t.state))),
        other @ ("tasks.log0" | "tasks.log1" | "tasks.log2" | "tasks.log3") => {
            let line: usize = other.trim_start_matches("tasks.log").parse().ok()?;
            let t = running_task(store).or_else(|| settled_task(store))?;
            match output_line(store, &t.id, line) {
                Some(l) => json!(l),
                None => Value::Null,
            }
        }
        _ => return None,
    })
}

// -------------------------------------------------------------------- actions

/// The PURE action table: `(action, row, store, steer_text) → (method,
/// params)`. Cited per row (see the module docs):
///
/// | action | method | params |
/// |---|---|---|
/// | `peer.steer` (row) | `peer/control` | `{session_id, slug, command:{kind:"steer",input:[{kind:"text",text}]}}` (`external-driver-peer-control.ts:694-704,99/147`; driver-seat keys reported as the native gap) |
/// | `task.cancel` | `task/cancel` | `{task_id, session_id}` (`use-supervision.ts:307-329`; `TaskCancelParams` @ 4231669:2530) |
/// | `task.open.running` | `task/output/read` | `{session_id, task_id, cursor:{offset:0}, limit_bytes:65536}` (the c24b recorded request shape; parity row 346) |
pub fn action_params(
    action: &str,
    row: usize,
    store: &Store,
    steer_text: &str,
) -> Option<(String, Value)> {
    let session = store.active_session().unwrap_or_default();
    match action {
        "peer.steer" | "peer_1_steer" | "peer_2_steer" | "peer_3_steer" => {
            let slug = peer_rows(store).into_iter().nth(row)?.name;
            Some((
                "peer/control".to_owned(),
                json!({
                    "session_id": session,
                    "slug": slug,
                    "command": { "kind": "steer", "input": [{ "kind": "text", "text": steer_text }] },
                }),
            ))
        }
        "task.cancel" => {
            let task = running_task(store)?;
            Some((
                "task/cancel".to_owned(),
                json!({ "task_id": task.id, "session_id": session }),
            ))
        }
        "task.open.running" => {
            let task = running_task(store).or_else(|| settled_task(store))?;
            Some((
                "task/output/read".to_owned(),
                json!({
                    "session_id": session,
                    "task_id": task.id,
                    "cursor": { "offset": 0 },
                    "limit_bytes": 65536,
                }),
            ))
        }
        _ => None,
    }
}

/// What an action means once routed. Transport work happens in [`spawn`].
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Steer peer row `row` with the composer's current draft (the web keeps
    /// per-row `steerDrafts`, `FleetView.tsx:226-227`; the native screen set
    /// has one draft — reported).
    Steer { row: usize, text: String },
    /// `task/cancel` the running task.
    CancelTask,
    /// `task/output/read` the running card's output.
    OpenRunning,
    /// The id was not one of this screen set's.
    Unhandled(String),
}

impl Effect {
    /// Test helper: did this effect fall through to Unhandled?
    pub fn is_unhandled(&self) -> bool {
        matches!(self, Effect::Unhandled(_))
    }
}

/// Route one action id to its effect. `index` is the emitting row for the
/// bare `peer.steer` event; the row-level control names (`peer_N_steer`) map
/// themselves so the event id is never ambiguous.
pub fn resolve(action: &str, index: usize, ctx: &Ctx<'_>) -> Effect {
    let row = match action {
        "peer_1_steer" => 0,
        "peer_2_steer" => 1,
        "peer_3_steer" => 2,
        _ => index,
    };
    match action {
        "peer.steer" | "peer_1_steer" | "peer_2_steer" | "peer_3_steer" => Effect::Steer {
            row,
            text: ctx.ui.lock().unwrap().draft(),
        },
        "task.cancel" => Effect::CancelTask,
        "task.open.running" => Effect::OpenRunning,
        other => Effect::Unhandled(other.to_owned()),
    }
}

// -------------------------------------------------------------- refresh/folds

/// Fold a recorded `task/list` result body into store snapshots — the same
/// projection the domain's `from_list_row` does (`supervision/model.ts:84`).
pub fn fold_task_list(v: Value, store: &Store) {
    let Some(tasks) = v.get("tasks").and_then(|t| t.as_array()) else {
        return;
    };
    for t in tasks {
        let Some(id) = t.get("id").and_then(|v| v.as_str()).map(str::to_owned) else {
            continue;
        };
        let state = t
            .get("state")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown")
            .to_owned();
        store.domains.task.upsert_snapshot(TaskSnapshot::from_list_row(
            id,
            t.get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned(),
            state.clone(),
            t.get("status")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .unwrap_or(state),
            t.get("title").and_then(|v| v.as_str()).map(str::to_owned),
            t.get("role").and_then(|v| v.as_str()).map(str::to_owned),
            t.get("source").and_then(|v| v.as_str()).map(str::to_owned),
            t.get("summary").and_then(|v| v.as_str()).map(str::to_owned),
            t.get("artifact_count").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            Vec::new(),
            None,
            None,
        ));
    }
}

/// Fold a `task/updated` notification body the way the domain's
/// `TaskUpdatedHandler` merges a sparse live update
/// (`supervision/model.ts:104`; `domains/task.rs:87-120`).
pub fn fold_task_updated(v: Value, store: &Store) {
    let Some(id) = v.get("task_id").and_then(|v| v.as_str()).map(str::to_owned) else {
        return;
    };
    let state = v
        .get("state")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown")
        .to_owned();
    let title = v.get("title").and_then(|t| t.as_str()).map(str::to_owned);
    let mut snapshot = TaskSnapshot::from_list_row(
        id,
        String::new(),
        state.clone(),
        v.get("runtime_detail")
            .and_then(|r| r.as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| state.clone()),
        None, // a live update carries no stable title; keep the row's
        None,
        None,
        v.get("summary").and_then(|s| s.as_str()).map(str::to_owned),
        0,
        Vec::new(),
        if state == "failed" {
            v.get("runtime_detail")
                .and_then(|r| r.as_str())
                .map(str::to_owned)
        } else {
            None
        },
        None,
    );
    if snapshot.tool_name.is_empty() {
        snapshot.tool_name = title.unwrap_or_default();
    }
    store.domains.task.upsert_snapshot(snapshot);
}

/// Fold a recorded `peer/gather` result body into the peer roster
/// (`PeerGatherEntry`: slug/topic/closed; `domains/peer.rs:118-150`).
pub fn fold_peer_gather(v: Value, store: &Store) {
    let Some(peers) = v.get("peers").and_then(|p| p.as_array()) else {
        return;
    };
    for p in peers {
        let Some(slug) = p.get("slug").and_then(|s| s.as_str()).map(str::to_owned) else {
            continue;
        };
        let mut peer = Peer::named(slug);
        peer.topic = p.get("topic").and_then(|t| t.as_str()).map(str::to_owned);
        store.domains.peer.upsert(peer);
        if p.get("closed").and_then(|c| c.as_bool()).unwrap_or(false) {
            store.domains.peer.mark_closed(
                p.get("slug").and_then(|s| s.as_str()).unwrap_or_default(),
            );
        }
    }
}

/// Pull the two fleet reads and fold them into the store — the web's load
/// path (`use-supervision.ts:124` task/list; `peer-commands.ts:66` gather,
/// which sends exactly `{session_id, profile_id}` — the r6 recording's own
/// request body). Production call site: lib.rs behind
/// `OCTOSCODE_STAGE_C_SCREENS`; the f30c replay calls it against the
/// recorded frames.
pub async fn refresh(conv: &Conversation, store: &Store) -> Result<usize, String> {
    let client = conv.client();
    let session = store.active_session().unwrap_or_default();
    let profile = conv.profile().to_owned();
    let mut done = 0usize;
    if let Ok(v) = client
        .request("task/list", json!({ "session_id": session }))
        .await
    {
        fold_task_list(v, store);
        done += 1;
    }
    if let Ok(v) = client
        .request("peer/gather", json!({ "session_id": session, "profile_id": profile }))
        .await
    {
        fold_peer_gather(v, store);
        done += 1;
    }
    Ok(done)
}

// --------------------------------------------------------------------- lower

fn screen_ns(screen_id: &str) -> &'static str {
    match screen_id {
        "autonomy-06" => "fleet",
        "autonomy-07" => "tasks",
        _ => "",
    }
}

fn cards_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../design/stage-b/autonomy/cards")
}

/// The card source with the CURRENT store values written into its `copy`
/// slots, plus its data + kit dir — what the mount host consumes (the same
/// chain models.rs runs for board 2). Pub for the f30c capture test.
pub fn lower_card_src(screen_id: &str, ctx: &Ctx<'_>) -> Result<(String, Value, PathBuf), String> {
    let dir = cards_root().join(screen_id);
    let read = |rel: &str| -> Result<String, String> {
        std::fs::read_to_string(dir.join(rel)).map_err(|e| format!("read {screen_id}/{rel}: {e}"))
    };
    let mut card_src = read("page.card")?;
    let data: Value = serde_json::from_str(&read("page.data.json")?)
        .map_err(|e| format!("parse {screen_id} data: {e}"))?;
    // Only this screen's namespace may write its copies (the models.rs
    // namespace guard — `t_title_text`-style collisions across cards).
    let ns = format!("{}.", screen_ns(screen_id));
    for (copy_id, binding) in COPY_SLOTS {
        if !binding.starts_with(&ns) {
            continue;
        }
        if let Some(v) = query_binding(ctx, binding) {
            if let Value::String(s) = v {
                if let Some(next) = crate::l0_host::set_copy(&card_src, copy_id, &s) {
                    card_src = next;
                }
            }
        }
    }
    Ok((card_src, data, dir.join("kit")))
}

/// Lower one screen card to Splash DSL with the CURRENT store values — the
/// module's own L0 chain (`l0::prepare` → `to_makepad_ui`), renamed per
/// screen like models.rs.
pub fn lower(screen_id: &str, ctx: &Ctx<'_>) -> Result<String, String> {
    let (card_src, data, kit_dir) = lower_card_src(screen_id, ctx)?;
    let prepared = octoscript_makepad::l0::prepare(&card_src, &data, &kit_dir)
        .map_err(|e| format!("prepare {screen_id}: {e}"))?;
    let mut tree = prepared.tree;
    octoscript_makepad::l0::inspectable(&mut tree);
    let ui = octoscript_makepad::design::to_makepad_ui(&tree)
        .map_err(|e| format!("to_makepad_ui {screen_id}: {e}"))?;
    let prefix = format!("scr_{}", screen_id.trim_start_matches("autonomy-"));
    Ok(ui.replace("beauty_0", &prefix))
}

// --------------------------------------------------------------------- spawn

/// Perform one routed effect through the production client (lib.rs's
/// `perform_action` arm; the workspace.rs `spawn` shape). The wire frame
/// comes from the ONE pure table ([`action_params`]) — the effect only names
/// the row and carries the steer draft captured at route time.
pub fn spawn(
    effect: Effect,
    rt: &tokio::runtime::Runtime,
    conv: &Arc<Conversation>,
    ui: &Mutex<FlowUi>,
    store: &Store,
) {
    let (action, row, text) = match &effect {
        Effect::Steer { row, text } => ("peer.steer", *row, text.clone()),
        Effect::CancelTask => ("task.cancel", 0, String::new()),
        Effect::OpenRunning => ("task.open.running", 0, String::new()),
        Effect::Unhandled(id) => {
            ::log::warn!("octoscode: fleet action unhandled {id:?}");
            return;
        }
    };
    let Some((method, params)) = action_params(action, row, store, &text) else {
        ::log::warn!("octoscode: fleet action {action}[{row}]: nothing to send (no target row/task)");
        return;
    };
    // A sent steer clears the composer draft (the web clears the row's
    // `steerDrafts` entry on send, `FleetView.tsx:505-507`).
    if matches!(effect, Effect::Steer { .. }) {
        ui.lock().unwrap().set_draft_inner("");
    }
    ::log::info!("octoscode: fleet action -> {method}");
    let conv = conv.clone();
    rt.spawn(async move {
        if let Err(e) = conv.client().request(&method, params).await {
            ::log::warn!("octoscode: fleet {method}: {e}");
        }
    });
}

/// The Stage-B fixture store for the capture host: three peers (two working,
/// one `Done`), the goal line, a running + a settled task with four output
/// lines — exactly the shape the accepted reviews show, so the LIVE slots
/// reproduce the reviewed renders (the f29c capture-test contract).
///
/// `OCTOSCODE_CAPTURE_PEERS=0|1|3` scales the roster (0 = the empty state:
/// every live slot resolves Null and the AUTHORED copy shows — the
/// empty-store contract the f30c coverage test pins); the default is 3.
/// `OCTOSCODE_CAPTURE_TASKS=0|1|3` scales the task rows the same way.
pub fn capture_store() -> std::sync::Arc<Store> {
    let store = std::sync::Arc::new(Store::new());
    let session = "dsflash:main";
    store.domains.session.set_active(Some(session.to_owned()));
    store.domains.autonomy.set_goal(
        session,
        GoalState {
            goal: Some(GoalRecord {
                goal_id: "g1".into(),
                objective: "Fix steer queue".into(),
                status: "active".into(),
                token_budget: 0,
                tokens_used: 0,
                created_at_ms: 0,
                updated_at_ms: 0,
            }),
            ..Default::default()
        },
    );
    let peers: &[&str] = match std::env::var("OCTOSCODE_CAPTURE_PEERS").as_deref() {
        Ok("0") => &[],
        Ok("1") => &["tests"],
        _ => &["tests", "docs", "review"],
    };
    for &name in peers {
        store.domains.peer.upsert(Peer::named(name));
    }
    if peers.contains(&"review") {
        store.domains.peer.mark_closed("review");
    }
    let tasks_env = std::env::var("OCTOSCODE_CAPTURE_TASKS").unwrap_or_else(|_| "2".into());
    let tasks: &str = tasks_env.as_str();
    if tasks != "0" {
        store.domains.task.upsert_snapshot(TaskSnapshot::from_list_row(
            "t-run".into(),
            "cargo test -p octos-cli steer_queue".into(),
            "running".into(),
            "running".into(),
            Some("cargo test -p octos-cli steer_queue".into()),
            None,
            None,
            None,
            0,
            Vec::new(),
            None,
            None,
        ));
    }
    if tasks == "2" || tasks == "3" {
        store.domains.task.upsert_snapshot(TaskSnapshot::from_list_row(
            "t-done".into(),
            "cargo clippy -p octos-cli".into(),
            "done".into(),
            "done".into(),
            Some("cargo clippy -p octos-cli".into()),
            None,
            None,
            None,
            0,
            Vec::new(),
            None,
            None,
        ));
    }
    if tasks == "3" {
        store.domains.task.upsert_snapshot(TaskSnapshot::from_list_row(
            "t-fmt".into(),
            "cargo fmt --check".into(),
            "done".into(),
            "done".into(),
            Some("cargo fmt --check".into()),
            None,
            None,
            None,
            0,
            Vec::new(),
            None,
            None,
        ));
    }
    for line in [
        "Compiling octos-cli v0.24.1 (/workspace/crates/octos-cli)",
        "Finished test [unoptimized + debuginfo] target(s) in 1.23s",
        "Running unittests src/lib.rs (target/debug/deps/octos_cli…)",
        "running 12 tests …",
    ] {
        store.domains.task.append_output("t-run", &format!("{line}\n"));
    }
    store
}
