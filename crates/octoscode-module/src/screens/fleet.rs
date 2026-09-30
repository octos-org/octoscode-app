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
    // autonomy-06 Fleet — the STATIC slots. The peer/task rows are GENERATED
    // per data item (#30c2, LESSONS "list screens must render per item"):
    // their copy slots (`copy.peer_r{i}_*_text`, `copy.run_r{i}_*_text`,
    // `copy.done_r{i}_*_text`) are minted with the rows and filled through
    // the dynamic binding ids (`fleet.peer_r{i}.*`, `tasks.run_r{i}.*`,
    // `tasks.done_r{i}.*`) in `lower_card_src` — the f30c row tests pin them.
    ("t_title_text", "fleet.title"),
    ("fleet_goal_label_text", "fleet.goal"),
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

/// Running tasks in a STABLE order (id-sorted) — row `i` must always be the
/// same item (the steer/cancel targets depend on it).
fn running_tasks(store: &Store) -> Vec<TaskSnapshot> {
    let mut rows: Vec<TaskSnapshot> = store
        .domains
        .task
        .snapshots()
        .into_iter()
        .filter(|t| t.state == "running")
        .collect();
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

/// Terminal (non-running) tasks, same stability contract.
fn settled_tasks(store: &Store) -> Vec<TaskSnapshot> {
    let mut rows: Vec<TaskSnapshot> = store
        .domains
        .task
        .snapshots()
        .into_iter()
        .filter(|t| t.state != "running")
        .collect();
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

fn running_task(store: &Store) -> Option<TaskSnapshot> {
    running_tasks(store).into_iter().next()
}

fn settled_task(store: &Store) -> Option<TaskSnapshot> {
    settled_tasks(store).into_iter().next()
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
            // LIVE at every count (#30c2): 0 must not show the design's
            // sample "Fleet · 3 peers".
            let n = peer_rows(store).len();
            json!(format!("Fleet · {n} peer{}", if n == 1 { "" } else { "s" }))
        }
        "fleet.goal" => match store
            .active_session()
            .and_then(|s| store.domains.autonomy.goal(&s))
        {
            Some(g) => json!(g.objective),
            None => Value::Null,
        },
        // peer_r{i}_* — GENERATED rows (#30c2): row i of the slug-ordered
        // roster; the badge/status style is chosen per item in the row
        // builder, the bindings only carry the item's text.
        other if other.starts_with("fleet.peer_r") => {
            let rest = other.trim_start_matches("fleet.peer_r");
            let (idx, field) = rest.split_once('.')?;
            let i: usize = idx.parse().ok()?;
            let p = peer_rows(store).into_iter().nth(i)?;
            match field {
                "name" => json!(p.name),
                "status" if p.closed => json!("Done"),
                // The remaining §4.3 words need phase facts the native store
                // does not carry (parity row 110) — authored (empty) stays.
                "status" => Value::Null,
                // Elapsed/tokens are not in `Peer` — authored (empty) stays.
                _ => Value::Null,
            }
        }
        // ---- autonomy-07 Tasks — GENERATED blocks (#30c2): run block i of
        // the id-sorted running items, done block i of the terminal items.
        other if other.starts_with("tasks.run_r") => {
            let rest = other.trim_start_matches("tasks.run_r");
            let (idx, field) = rest.split_once('.')?;
            let i: usize = idx.parse().ok()?;
            let t = running_tasks(store).into_iter().nth(i)?;
            match field {
                "cmd" => json!(task_label(&t)),
                "status" => json!(status_word(&t.state)),
                // No wall-clock durations in the store — authored stays.
                "dur" => Value::Null,
                f if f.starts_with("log") => {
                    let line: usize = f.trim_start_matches("log").parse().ok()?;
                    match output_line(store, &t.id, line) {
                        Some(l) => json!(l),
                        None => Value::Null,
                    }
                }
                _ => Value::Null,
            }
        }
        other if other.starts_with("tasks.done_r") => {
            let rest = other.trim_start_matches("tasks.done_r");
            let (idx, field) = rest.split_once('.')?;
            let i: usize = idx.parse().ok()?;
            let t = settled_tasks(store).into_iter().nth(i)?;
            match field {
                "cmd" => json!(task_label(&t)),
                "status" => json!(status_word(&t.state)),
                _ => Value::Null,
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
    crate::design::dir("stage-b/autonomy/cards")
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
    let mut data: Value = serde_json::from_str(&read("page.data.json")?)
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
    // #30c2: the authored card's rows are the DESIGN's fixed sample rows —
    // replace them with GENERATED rows, one per data item (LESSONS "list
    // screens must render per item"), 0 → the web's empty state.
    let card_src = rewrite_rows(screen_id, card_src, &mut data, ctx);
    Ok((card_src, data, dir.join("kit")))
}

// ------------------------------------------------- generated rows (#30c2)

/// The web's empty-state copy, verbatim: "No peers yet"
/// (`FleetView.tsx:537-540`) and "No background tasks in this session."
/// (`SessionTrajectory.tsx:137-139`).
const FLEET_EMPTY: &str = "No peers yet";
const TASKS_EMPTY: &str = "No background tasks in this session.";

/// First-index..past-last-index of the `{…}` block whose text starts at
/// `anchor` (the anchor's own opening brace included).
fn block_span(src: &str, anchor: &str) -> Option<(usize, usize)> {
    let start = src.find(anchor)?;
    let open = src[start..].find('{')? + start;
    let mut depth = 0usize;
    for (i, ch) in src[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((start, open + i + 1));
                }
            }
            _ => {}
        }
    }
    None
}

/// Mint `copy <id> { class: user-copy, en: <value> }` declarations after the
/// card's LAST authored copy line (the L0 copy table).
fn mint_copies(card_src: &str, minted: &[(String, String)]) -> String {
    let mut lines: Vec<String> = card_src.lines().map(str::to_owned).collect();
    let Some(pos) = lines.iter().rposition(|l| l.trim_start().starts_with("copy ")) else {
        return card_src.to_owned();
    };
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + minted.len());
    out.extend_from_slice(&lines[..=pos]);
    for (id, en) in minted {
        out.push(format!("copy {id} {{ class: user-copy, en: {en:?} }}"));
    }
    out.extend_from_slice(&lines[pos + 1..]);
    out.join("\n") + "\n"
}

/// Keep only the STATIC placements (the authored row placements describe the
/// design's sample rows and must not survive the rewrite).
fn retain_static_placements(data: &mut Value, keep: &[&str]) {
    if let Some(map) = data
        .get_mut("$kit")
        .and_then(|k| k.get_mut("placements"))
        .and_then(|p| p.as_object_mut())
    {
        map.retain(|k, _| keep.contains(&k.as_str()));
    }
}

/// Insert generated placements. `component` MUST be the exact kit id the DSL
/// instantiates for that instance — `l0::prepare` pairs them by name and
/// rejects a mismatch ("component/placement mismatch", the failure the
/// component-less first cut hit).
fn put_placements(data: &mut Value, rows: &[(String, &'static str, f64, f64, f64, f64)]) {
    if let Some(map) = data
        .get_mut("$kit")
        .and_then(|k| k.get_mut("placements"))
        .and_then(|p| p.as_object_mut())
    {
        for (name, component, x, y, w, h) in rows {
            map.insert(
                name.clone(),
                serde_json::json!({
                    "component": component,
                    "layout": {"x": x, "y": y, "w": w, "h": h},
                }),
            );
        }
    }
}

/// The web's `formatElapsed` (`peer-row-view.ts:127-136`): `42s`, `1m30s`,
/// `2h05m`.
fn format_elapsed(staged_at_ms: u64) -> String {
    let secs = (octoscode_store::domains::peer::now_ms().saturating_sub(staged_at_ms) / 1000) as u32;
    if secs < 60 {
        return format!("{secs}s");
    }
    if secs < 3600 {
        return format!("{}m{:02}s", secs / 60, secs % 60);
    }
    format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
}

fn rewrite_rows(screen_id: &str, card_src: String, data: &mut Value, ctx: &Ctx<'_>) -> String {
    match screen_id {
        "autonomy-06" => rewrite_fleet_rows(card_src, data, ctx),
        "autonomy-07" => rewrite_tasks_rows(card_src, data, ctx),
        _ => card_src,
    }
}

/// autonomy-06: one peer row per item, the badge + status component chosen
/// FROM THE ITEM (closed → the terminal gray badge `Surfacefe8deb6b02b9` +
/// `Textda5b0618c4a3` + "Done"; open → the active green badge
/// `Surface46bf93b36e13` + `Text0ee31649c829` + "Working" — the design bound
/// them to ROW POSITION instead, the flagged defect). Names get a widened
/// column (the authored 42px clipped "review" to "reviev"). 0 peers → the
/// empty-state text, no rows.
fn rewrite_fleet_rows(card_src: String, data: &mut Value, ctx: &Ctx<'_>) -> String {
    let peers = peer_rows(ctx.store);
    // #32c item 11: 0 peers → a COMPACT empty card with no sample goal
    // heading — the goal group's DSL block and placements go with the rows.
    let empty = peers.is_empty();
    let keep: Vec<&str> = if empty {
        vec!["page", "fleet_screen", "fleet_card", "t_title"]
    } else {
        vec!["page", "fleet_screen", "fleet_card", "t_title", "fleet_goal", "fleet_goal_label"]
    };
    retain_static_placements(data, &keep);
    let mut card_src = card_src;
    if empty {
        if let Some((gs, ge)) =
            block_span(&card_src, "Group3d2637879433(instance: \"fleet_goal\") {")
        {
            card_src = format!("{}{}", &card_src[..gs], &card_src[ge..]);
        }
        // The heading TEXT is a top-level `copy` (page.card:7, outside the
        // group) — blank it too, or the sample goal still renders.
        card_src = card_src.replace(
            "copy fleet_goal_label_text { class: user-copy, en: \"Fix steer queue\" }",
            "copy fleet_goal_label_text { class: user-copy, en: \"\" }",
        );
    }
    let Some((fs, _fe)) = block_span(&card_src, "Surface5fa5be74391b(instance: \"fleet_card\") {")
    else {
        return card_src;
    };
    let line_end = card_src[fs..].find('\n').map(|i| fs + i + 1).unwrap_or(card_src.len());
    const ROW: &str = "Group3d2637879433";
    const NAME: &str = "Text6965a34baa9d";
    const STEER: &str = "Text05c2e4254ba7";
    const DIV: &str = "Surfacec4c38149242a";
    const BADGE_ON: &str = "Surface46bf93b36e13";
    const BADGE_ON_T: &str = "Text0ee31649c829";
    const BADGE_OFF: &str = "Surfacefe8deb6b02b9";
    const BADGE_OFF_T: &str = "Textda5b0618c4a3";
    let mut body = String::new();
    let mut minted: Vec<(String, String)> = Vec::new();
    let mut placed: Vec<(String, &'static str, f64, f64, f64, f64)> = Vec::new();

    if peers.is_empty() {
        body.push_str(
            "      Text6965a34baa9d(instance: \"fleet_empty\", text: copy.fleet_empty_text)\n",
        );
        minted.push(("fleet_empty_text".to_owned(), FLEET_EMPTY.to_owned()));
        placed.push(("fleet_empty".to_owned(), NAME, 32.0, 240.0, 340.0, 24.0));
        // Compact: empty line bottom 264 + the authored pad 12 (606−594) −
        // card y 217 ≈ 59 (the authored 389 was the 3 sample rows).
        placed.push((
            "fleet_card".to_owned(),
            "Surface5fa5be74391b",
            11.0,
            217.0,
            383.0,
            59.0,
        ));
    } else {
        const Y0: f64 = 227.92;
        const PITCH: f64 = 138.36;
        for (i, p) in peers.iter().enumerate() {
            let y = Y0 + i as f64 * PITCH;
            let (badge, status_comp, badge_w, status) = if p.closed {
                (BADGE_OFF, BADGE_OFF_T, 69.0, "Done")
            } else {
                // An open peer is live on the roster's activity axis
                // (peer-roster.ts:40-42) → the §4.3 word "Working".
                (BADGE_ON, BADGE_ON_T, 86.0, "Working")
            };
            body.push_str(&format!(
                "      Group3d2637879433(instance: \"peer_r{i}\") {{\n        \
                 {badge}(instance: \"peer_r{i}_badge\") {{\n          \
                 {status_comp}(instance: \"peer_r{i}_status\", text: copy.peer_r{i}_status_text)\n        }}\n        \
                 Text6965a34baa9d(instance: \"peer_r{i}_name\", text: copy.peer_r{i}_name_text)\n        \
                 Text53a53f8f1184(instance: \"peer_r{i}_meta\", text: copy.peer_r{i}_meta_text)\n        \
                 Text05c2e4254ba7(instance: \"peer_r{i}_steer\", text: copy.peer_r{i}_steer_text)\n      }}\n"
            ));
            if i + 1 < peers.len() {
                body.push_str(&format!(
                    "      Surfacec4c38149242a(instance: \"peer_divider_r{i}\") {{\n\n      }}\n"
                ));
                placed.push((format!("peer_divider_r{i}"), DIV, 19.19, y + 129.63, 366.62, 1.09));
            }
            placed.push((format!("peer_r{i}"), ROW, 11.0, y, 383.0, 89.37));
            placed.push((format!("peer_r{i}_badge"), badge, 28.0, y + 27.08, badge_w, 52.0));
            placed.push((format!("peer_r{i}_status"), status_comp, 41.91, y + 41.86, 58.93, 26.88));
            // WIDENED: the authored 42px name column clipped longer slugs.
            placed.push((format!("peer_r{i}_name"), NAME, 147.48, y + 28.14, 160.0, 22.3));
            placed.push((
                format!("peer_r{i}_meta"),
                "Text53a53f8f1184",
                147.48,
                y + 71.15,
                160.0,
                21.39,
            ));
            placed.push((format!("peer_r{i}_steer"), STEER, 329.77, y + 46.44, 42.63, 24.13));
            minted.push((format!("peer_r{i}_status_text"), status.to_owned()));
            minted.push((format!("peer_r{i}_name_text"), p.name.clone()));
            // #32c item 11: the meta line (elapsed · tokens). Tokens have no
            // store source yet → the web's own zero-value dash.
            minted.push((
                format!("peer_r{i}_meta_text"),
                format!("{} · —", format_elapsed(p.staged_at_ms)),
            ));
            minted.push((format!("peer_r{i}_steer_text"), "Steer".to_owned()));
        }
    }
    put_placements(data, &placed);
    let card_src = format!("{}{}    }}\n  }}\n}}\n", &card_src[..line_end], body);
    mint_copies(&card_src, &minted)
}

/// autonomy-07: one run block per RUNNING item (the console's four output
/// lines are the item's own), one done block per TERMINAL item — the authored
/// card fixed exactly one of each (the flagged defect). 0 items → the
/// empty-state text. The authored fixed cards (the done_card self-closing
/// line + the run_card block) are REMOVED — the generated blocks hang
/// directly under tasks_card.
fn rewrite_tasks_rows(card_src: String, data: &mut Value, ctx: &Ctx<'_>) -> String {
    // The authored icon placements carry the capture-time SVG srcs (a kit
    // Vector node cannot lower without one — "design SVG resource required");
    // capture them before the static retain drops the authored rows.
    // The authored src lives INSIDE the placement's layout object (the
    // captured page.data.json: `icon_run_task.layout.src`) — read it there.
    let icon_run_src = data["$kit"]["placements"]["icon_run_task"]["layout"]["src"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let icon_done_src = data["$kit"]["placements"]["icon_done_task"]["layout"]["src"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    retain_static_placements(data, &["page", "tasks_card", "t02"]);
    let runs = running_tasks(ctx.store);
    let dones = settled_tasks(ctx.store);
    // 1. drop the authored done_card LINE (self-closing, before the run block).
    let card_src = card_src
        .lines()
        .filter(|l| !l.contains("Surfaceed7038384cce(instance: \"done_card\")"))
        .map(|l| format!("{l}\n"))
        .collect::<String>();
    // 2. cut the authored run_card BLOCK: keep the source up to the run_card
    //    anchor line, append the generated body + the card's closers.
    let Some((rs, _re)) = block_span(&card_src, "Surfacee81dee70a29b(instance: \"run_card\") {")
    else {
        return card_src;
    };
    let line_end = card_src[rs..].find('\n').map(|i| rs + i + 1).unwrap_or(card_src.len());
    const RUN: &str = "Surfacee81dee70a29b";
    const ICON: &str = "Vectore0a378fde04f";
    const CMD: &str = "Textfda4b2da12b8";
    const PILL: &str = "Surfaceec5e698cce2f";
    const PILL_TXT: &str = "Text97dbc02c6f15";
    const DUR: &str = "Text1e0cc765bf27";
    const CONSOLE: &str = "Surfacef9797a8e8795";
    const LOG01: &str = "Texte890c3523ee8";
    const LOG2: &str = "Textf26085efae71";
    const LOG3: &str = "Textd49e66b4cabb";
    const CURSOR: &str = "Text60155355f2e8";
    const GRP: &str = "Group3d2637879433";
    const CANCEL_S: &str = "Surfaceff776c252975";
    const CANCEL_B: &str = "NativeButton02f1d1bd99b4";
    const CANCEL_T: &str = "Text6f1917ec00eb";
    const DONE: &str = "Surfaceed7038384cce";
    const DCMD: &str = "Textd55469de8b8b";
    const DPILL: &str = "Surfaceffa407c932d5";
    const DPILL_TXT: &str = "Text4cb0e7db43f8";
    const DDUR: &str = "Textc440e7c590d3";
    let mut body = String::new();
    let mut minted: Vec<(String, String)> = Vec::new();
    let mut placed: Vec<(String, &'static str, f64, f64, f64, f64)> = Vec::new();

    if runs.is_empty() && dones.is_empty() {
        body.push_str(
            "    Textd55469de8b8b(instance: \"tasks_empty\", text: copy.tasks_empty_text)\n",
        );
        minted.push(("tasks_empty_text".to_owned(), TASKS_EMPTY.to_owned()));
        placed.push(("tasks_empty".to_owned(), DCMD, 32.0, 208.5, 342.0, 44.0));
    } else {
        const RUN_H: f64 = 387.0;
        for (i, t) in runs.iter().enumerate() {
            let y = 188.5 + i as f64 * (RUN_H + 8.0);
            body.push_str(&format!(
                "    Surfacee81dee70a29b(instance: \"run_r{i}\") {{\n      \
                 Vectore0a378fde04f(instance: \"run_r{i}_icon\")\n      \
                 Textfda4b2da12b8(instance: \"run_r{i}_cmd\", text: copy.run_r{i}_cmd_text)\n      \
                 Surfaceec5e698cce2f(instance: \"run_r{i}_pill\") {{\n        \
                 Text97dbc02c6f15(instance: \"run_r{i}_status\", text: copy.run_r{i}_status_text)\n      }}\n      \
                 Text1e0cc765bf27(instance: \"run_r{i}_dur\", text: copy.run_r{i}_dur_text)\n      \
                 Surfacef9797a8e8795(instance: \"run_r{i}_console\") {{\n        \
                 Texte890c3523ee8(instance: \"run_r{i}_log0\", text: copy.run_r{i}_log0_text)\n        \
                 Texte890c3523ee8(instance: \"run_r{i}_log1\", text: copy.run_r{i}_log1_text)\n        \
                 Textf26085efae71(instance: \"run_r{i}_log2\", text: copy.run_r{i}_log2_text)\n        \
                 Textd49e66b4cabb(instance: \"run_r{i}_log3\", text: copy.run_r{i}_log3_text)\n        \
                 Text60155355f2e8(instance: \"run_r{i}_cursor\", text: copy.t_cursor_text)\n      }}\n      \
                 Group3d2637879433(instance: \"run_r{i}_cancel\") {{\n        \
                 Surfaceff776c252975(instance: \"run_r{i}_cancel_surface\") {{\n\n        }}\n        \
                 NativeButton02f1d1bd99b4(instance: \"run_r{i}_cancel_control\", enabled: cancel_control_enabled)\n        \
                 Text6f1917ec00eb(instance: \"run_r{i}_cancel_label\", text: copy.cancel_label_text)\n      }}\n    }}\n"
            ));
            placed.push((format!("run_r{i}"), RUN, 20.0, y, 350.0, RUN_H));
            placed.push((format!("run_r{i}_icon"), ICON, 33.0, y + 34.0, 18.0, 18.2));
            placed.push((format!("run_r{i}_cmd"), CMD, 53.0, y + 32.0, 260.0, 20.0));
            placed.push((format!("run_r{i}_pill"), PILL, 274.0, y + 20.0, 63.0, 42.0));
            placed.push((format!("run_r{i}_status"), PILL_TXT, 283.78, y + 31.46, 45.91, 23.57));
            placed.push((format!("run_r{i}_dur"), DUR, 341.33, y + 34.06, 24.0, 21.5));
            placed.push((format!("run_r{i}_console"), CONSOLE, 35.0, y + 86.0, 320.0, 197.0));
            for (k, comp) in [LOG01, LOG01, LOG2, LOG3].iter().enumerate() {
                let ly = y + 116.0 + 41.0 * k as f64;
                let lh = if k == 3 { 15.0 } else { 41.0 };
                placed.push((format!("run_r{i}_log{k}"), comp, 40.0, ly, 310.0, lh));
            }
            placed.push((format!("run_r{i}_cursor"), CURSOR, 155.0, y + 235.0, 6.0, 22.0));
            placed.push((format!("run_r{i}_cancel"), GRP, 37.0, y + 309.0, 225.0, 55.0));
            placed.push((format!("run_r{i}_cancel_surface"), CANCEL_S, 37.0, y + 309.0, 225.0, 55.0));
            placed.push((format!("run_r{i}_cancel_control"), CANCEL_B, 37.0, y + 309.0, 225.0, 55.0));
            placed.push((format!("run_r{i}_cancel_label"), CANCEL_T, 127.65, y + 325.03, 45.91, 21.61));
            minted.push((format!("run_r{i}_cmd_text"), task_label(t)));
            minted.push((format!("run_r{i}_status_text"), status_word(&t.state)));
            minted.push((format!("run_r{i}_dur_text"), String::new()));
            let lines: Vec<String> = (0..4)
                .map(|k| output_line(ctx.store, &t.id, k).unwrap_or_default())
                .collect();
            // #32c item 11: no output yet → the waiting line, never the
            // design's sample log.
            let waiting = lines.iter().all(|l| l.is_empty());
            for (k, line) in lines.iter().enumerate() {
                let text = if waiting && k == 0 {
                    "Waiting for output\u{2026}".to_owned()
                } else {
                    line.clone()
                };
                minted.push((format!("run_r{i}_log{k}_text"), text));
            }
        }
        let done_y0 = 188.5 + runs.len() as f64 * (RUN_H + 8.0) + 8.0;
        for (j, t) in dones.iter().enumerate() {
            let y = done_y0 + j as f64 * 56.0;
            // The done container's kit component is slot=false (the authored
            // card uses it as a SELF-CLOSING backdrop with the content as
            // SIBLINGS — nesting children gets them silently dropped).
            body.push_str(&format!(
                "    Surfaceed7038384cce(instance: \"done_r{j}\")\n      \
                 Vectore0a378fde04f(instance: \"done_r{j}_icon\")\n      \
                 Textd55469de8b8b(instance: \"done_r{j}_cmd\", text: copy.done_r{j}_cmd_text)\n      \
                 Surfaceffa407c932d5(instance: \"done_r{j}_pill\") {{\n        \
                 Text4cb0e7db43f8(instance: \"done_r{j}_status\", text: copy.done_r{j}_status_text)\n      }}\n      \
                 Textc440e7c590d3(instance: \"done_r{j}_dur\", text: copy.done_r{j}_dur_text)\n"
            ));
            placed.push((format!("done_r{j}"), DONE, 20.0, y, 350.0, 49.0));
            placed.push((format!("done_r{j}_icon"), ICON, 29.0, y + 16.0, 16.0, 16.0));
            placed.push((format!("done_r{j}_cmd"), DCMD, 49.0, y + 15.0, 260.0, 20.0));
            placed.push((format!("done_r{j}_pill"), DPILL, 271.0, y + 2.0, 54.0, 43.0));
            placed.push((format!("done_r{j}_status"), DPILL_TXT, 282.0, y + 15.0, 31.94, 18.68));
            placed.push((format!("done_r{j}_dur"), DDUR, 349.55, y + 15.0, 19.72, 18.68));
            minted.push((format!("done_r{j}_cmd_text"), task_label(t)));
            minted.push((format!("done_r{j}_status_text"), status_word(&t.state)));
            minted.push((format!("done_r{j}_dur_text"), String::new()));
        }
    }
    // #32c item 11: the card sizes to its content (authored pad 17 = 756 −
    // 739; the authored 648 fixed height overflowed at 3 items).
    let content_bottom = if runs.is_empty() && dones.is_empty() {
        285.0 + 44.0
    } else {
        let runs_bottom = if runs.is_empty() {
            0.0
        } else {
            188.5 + (runs.len() - 1) as f64 * (387.0 + 8.0) + 387.0
        };
        let done_bottom = if dones.is_empty() {
            0.0
        } else {
            let done_y0c = 188.5 + runs.len() as f64 * (387.0 + 8.0) + 8.0;
            done_y0c + (dones.len() - 1) as f64 * 56.0 + 49.0
        };
        runs_bottom.max(done_bottom)
    };
    placed.push((
        "tasks_card".to_owned(),
        "Surfaceb4ab38079b26",
        12.0,
        108.0,
        372.0,
        content_bottom + 17.0 - 108.0,
    ));
    // #32c2 item 3: the heading sits at normal top padding (the card's 22px
    // inset), not the authored y=206.43 that left ~200 device px dead above.
    placed.push(("t02".to_owned(), "Texte9bb098090a8", 34.24, 130.0, 52.89, 28.46));
    put_placements(data, &placed);
    // Re-attach the SVG srcs: Vector nodes lower to `http_resource(src)` —
    // a placement without one fails to_makepad_ui.
    if let Some(map) = data
        .get_mut("$kit")
        .and_then(|k| k.get_mut("placements"))
        .and_then(|p| p.as_object_mut())
    {
        for (name, entry) in map.iter_mut() {
            let src = if name.starts_with("run_r") && name.ends_with("_icon") {
                icon_run_src.as_str()
            } else if name.starts_with("done_r") && name.ends_with("_icon") {
                icon_done_src.as_str()
            } else {
                continue;
            };
            entry["layout"]["src"] = serde_json::json!(src);
        }
    }
    // Cut AT the anchor start: the authored run_card line (with its `{`) and
    // everything after it go; the generated blocks hang directly under
    // tasks_card and the two closers below close tasks_card + root.
    let _ = line_end;
    let card_src = format!("{}{}  }}\n}}\n", &card_src[..rs], body);
    mint_copies(&card_src, &minted)
}
/// Lower one screen card to Splash DSL with the CURRENT store values — the
/// module's own L0 chain (`l0::prepare` → `to_makepad_ui`), renamed per
/// screen like models.rs.
pub fn lower(screen_id: &str, ctx: &Ctx<'_>) -> Result<String, String> {
    let (card_src, data, kit_dir) = lower_card_src(screen_id, ctx)?;
    let prepared = octoscript_makepad::l0::prepare(&card_src, &data, &kit_dir)
        .map_err(|e| format!("prepare {screen_id}: {e}"))?;
    let mut tree = prepared.tree;
    // #32c item 11 + #32c2 item 4: the fleet's Done rows wear the web's
    // terminal state — GREY TEXT ON A GREY PILL (`--dsw-alias-label-secondary`
    // #61666b, theme.css:82; the kit badge surface bg_fa0d0938e19f is
    // greenish #E6F6E9). The surface is the text node's parent (peer_rN_badge
    // → peer_rN_status), so the pass carries the parent down.
    if screen_id == "autonomy-06" {
        // Grey text on a grey pill: the badge SURFACE (id `peer_rN_badge`)
        // and the Done TEXT (id `peer_rN_status`) are both directly
        // addressable — no parent tracking needed.
        let mut work = vec![&mut tree];
        while let Some(n) = work.pop() {
            if n.attrs.text.as_deref() == Some("Done") {
                n.attrs.color = Some(0xFF61_66_6B);
            }
            if n
                .attrs
                .id
                .as_deref()
                .is_some_and(|id| id.ends_with("_badge"))
            {
                // Only the TERMINAL pill goes grey (the #32c2 item-4 review):
                // decide from the badge's OWN status text child — "Done" (and
                // the other terminal words) take the neutral surface, a
                // "Working" pill keeps the kit's green one.
                let terminal = n
                    .children
                    .iter()
                    .any(|c| matches!(c.attrs.text.as_deref(), Some("Done")));
                if terminal {
                    n.attrs.bg = Some(0xFFE9_EA_EC);
                }
            }
            for c in &mut n.children {
                work.push(c);
            }
        }
    }
    octoscript_makepad::l0::inspectable(&mut tree);
    let ui = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(&tree))
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
