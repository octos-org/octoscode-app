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
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use serde_json::{json, Value};

use octoscode_store::Store;

use crate::bindings::Ctx;
use crate::flow::{Conversation, Direction};

/// The board-3 data slots (autonomy cards 03/04/05).
pub const BINDINGS: &[(&str, &str)] = &[
    ("goal.objective", "text: the active goal's objective (t_goal); null when none"),
    ("goal.status", "text: active | paused | complete"),
    ("goal.budget", "text: 'used / budget' with formatTokens (model.ts:126,129-136); null without a goal"),
    ("goal.fill", "number: tokens_used/token_budget (0..1) — the budget bar's fill; null without a goal"),
    ("goal.elapsed", "text: time_used_seconds at the atlas granularity ('0s'/'18m'/'1h 30m'; the web has no elapsed formatter — implemented per entry #30b2)"),
    ("goal.can_transition", "bool: status is active|paused (goalCanTransition — pause/resume/stop enabled)"),
    ("goal.available", "bool: the session advertised session/goal/get (fail closed)"),
    ("loops", "list: [{id,name,cadence,status}] — EXACTLY the store's items; rows instantiate per item (the #17 pattern)"),
    ("loops.count", "int: the loop count (the mounted row count follows it)"),
    ("loops.empty", "text: 'No loops in this session.' when 0 (AutonomyPanel.tsx:238), else ''"),
    ("loops.available", "bool: the session advertised loop/list"),
    ("monitors", "list: [{id,name,cmd,status,interval}] — exactly the store's items; cmd ellipsized, never mid-word clipped"),
    ("monitors.count", "int: the monitor count"),
    ("monitors.empty", "text: 'No monitors in this session.' when 0 (AutonomyPanel.tsx:338), else ''"),
    ("monitors.available", "bool: the session advertised monitor/list"),
    ("monitors.footer", "text: the pluralized count line ('1 monitor · N active')"),
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

/// A CLONE of the screen cache, for a caller outside this module that needs to
/// lower a card (#P4e1c: `lib.rs`'s mount arm).
///
/// The cache is a private `MutexGuard`-held `AutonomyState`; the lowering
/// functions take `&AutonomyState`, and the mount path holds no lock across
/// `lower_screen`. So the snapshot is taken under the lock and handed over by
/// value — the caller can never hold the guard, which would deadlock against a
/// later `resolve` on the UI thread.
pub fn state_snapshot() -> AutonomyState {
    state().clone()
}

/// The goal card's clickable controls, at the card's OWN placements.
///
/// #P4e1c — measured, not inferred. `service-actions.json` records the Stage-A
/// atlas bounds, but this card's `page.data.json` was laid out from a
/// different pass: Pause sits at [34,559] and Stop at [209,559] against
/// authored [30,559] and [193,559] — 4px and 16px of drift, far past
/// `taps::POS_TOLERANCE` (1.5px). So the shared `service-actions` path wires
/// ZERO goal controls. These are the placements the card actually renders.
///
/// Only the two controls that ARE `DesignNativeButton`s appear.
/// `clear_goal` is authored on a `Text` node (`page.card:112`), and the per-row
/// loop/monitor controls on `Svg` — neither can carry a handler without a hit
/// target the design does not have, which would be a new affordance.
pub fn goal_controls() -> Vec<(String, String, f64, f64)> {
    vec![
        ("pause_btn".to_owned(), "goal.pause".to_owned(), 34.0, 559.0),
        ("stop_btn".to_owned(), "goal.stop".to_owned(), 209.0, 559.0),
    ]
}

/// `split_row` — see below.
///
/// #P4e1c — the mounted card's per-row controls (a loop row's
/// pause/play/trash, a monitor row's pause/trash) are one widget each, so the
/// tap that fires has to name WHICH row. The shared tap dispatch carries a
/// widget name and an action id and passes a single index
/// (`lib.rs:2944`), which is 0 for every card tap, so a per-row action routed
/// through it would always address row 0 — a real defect the mount would have
/// shipped.
///
/// The row is carried IN the id as a `#<row>` suffix, the same shape the
/// composer uses elsewhere, and stripped here so the action table stays
/// one-owner: `ACTIONS`/`ROUTED` still list the BARE names.
///
/// `"monitor.pause#2"` → `("monitor.pause", Some(2))`; a bare id → `(_, None)`.
pub fn split_row(action: &str) -> (&str, Option<usize>) {
    match action.rsplit_once('#') {
        Some((base, row)) => match row.parse::<usize>() {
            Ok(index) => (base, Some(index)),
            // A name that merely contains '#' is a base name, not a row.
            Err(_) => (action, None),
        },
        None => (action, None),
    }
}

pub fn is_action(id: &str) -> bool {
    let (base, _) = split_row(id);
    ACTIONS.iter().any(|(a, _)| *a == base)
}

pub fn is_routed(id: &str) -> bool {
    let (base, _) = split_row(id);
    ROUTED.contains(&base)
}

/// The screen's UI-local cache: the last read results (the protocol pushes
/// goal/loop/monitor notifications, but the lists are read-on-demand —
/// the web keeps the same in `store.ts` state).
///
/// #P4e1c: `Clone` as well as `Default` — [`state_snapshot`] hands the cache to
/// `lib.rs`'s mount arm by value, and the caller must never hold this module's
/// lock across a lowering call (that would deadlock against a later `resolve`
/// on the UI thread). Every field is already `Clone`.
#[derive(Default, Clone)]
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

/// Fold into the screen cache — the production entry for the
/// `loop/updated`/`monitor/updated` notifications, and the test seam for
/// seeding 0/1/3-item stores.
pub fn update_state(f: impl FnOnce(&mut AutonomyState)) {
    f(&mut state());
}

/// The board-3 screens (the wired Stage-B cards).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen3 {
    Goal,
    Loops,
    Monitors,
}

impl Screen3 {
    pub fn card_dir(self) -> &'static str {
        match self {
            Screen3::Goal => "autonomy-03",
            Screen3::Loops => "autonomy-04",
            Screen3::Monitors => "autonomy-05",
        }
    }

    pub fn from_env() -> Option<Self> {
        match std::env::var("OCTOSCODE_SCREEN").as_deref() {
            Ok("goal") => Some(Screen3::Goal),
            Ok("loops") => Some(Screen3::Loops),
            Ok("monitors") => Some(Screen3::Monitors),
            _ => None,
        }
    }
}

/// The production lowering (the #29a connect precedent): the authored Stage-B
/// card + the live screen state → the makepad DSL the app mounts. The row,
/// sizing and empty-state rules live HERE (entry #30b4) — not in capture
/// tooling: rows instantiate per cached item (the #17 pattern), the card
/// heights follow the visible rows, the 0-item line renders in the body-font
/// slot, and the budget bar's fill is the `goal.fill` binding.
pub fn lower_screen(screen: Screen3, st: &AutonomyState) -> Result<String, String> {
    Ok(lower_tree(screen, st)?.dsl)
}

pub struct Lowered {
    pub dsl: String,
    pub card: octoscript_makepad::l0::PreparedCard,
    /// `l0::inspectable`'s original_id-keyed snapshot (text/w/h per node) —
    /// the assertions in the tests read THIS, not the DSL string.
    pub inventory: Vec<Value>,
    /// id → (y, w, h) collected after the apply pass — the geometry view
    /// (inspectable's snapshot carries no x/y).
    pub measured: HashMap<String, (f64, f64, f64)>,
}

pub fn lower_tree(screen: Screen3, st: &AutonomyState) -> Result<Lowered, String> {
    let dir = format!(
        "{}/{}",
        crate::design::dir("stage-b/autonomy/cards").display(),
        screen.card_dir()
    );
    let card_src = std::fs::read_to_string(format!("{dir}/page.card"))
        .map_err(|e| format!("read {dir}/page.card: {e}"))?;
    let data: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{dir}/page.data.json"))
            .map_err(|e| format!("read {dir}/page.data.json: {e}"))?,
    )
    .map_err(|e| format!("parse page.data.json: {e}"))?;
    let mut card = octoscript_makepad::l0::prepare(
        &card_src,
        &data,
        Path::new(&format!("{dir}/kit")),
    )
    .map_err(|e| format!("l0::prepare: {e}"))?;
    let tree = &mut card.tree;

    // ---- measure the authored geometry (immutable pass; types stay inferred
    // because octoscript-render is not a direct dependency of this crate).
    let mut geo: HashMap<String, (f64, f64, f64)> = HashMap::new(); // y, w, h
    let mut stack = vec![&*tree];
    while let Some(n) = stack.pop() {
        if let Some(id) = &n.attrs.id {
            geo.insert(
                id.clone(),
                (
                    n.attrs.y.unwrap_or(0.0),
                    n.attrs.w.unwrap_or(0.0) as f64,
                    n.attrs.h.unwrap_or(0.0) as f64,
                ),
            );
        }
        for c in &n.children {
            stack.push(c);
        }
    }
    let g = |id: &str| geo.get(id).copied().unwrap_or((0.0, 0.0, 0.0));

    // ---- decide the live values (pure, from the state)
    let mut hide: HashSet<String> = HashSet::new();
    // Monitors whose rows are paused → the resume (play) icon (#32c item 10).
    let mut play_ids: Vec<String> = Vec::new();
    let mut texts: Vec<(String, String, Option<f64>)> = Vec::new(); // id, text, w?
    let mut set_w: Vec<(String, f32)> = Vec::new();
    let mut set_h: Vec<(String, f32)> = Vec::new();
    let mut set_y: Vec<(String, f64)> = Vec::new();

    match screen {
        Screen3::Goal => {
            if let Some(goal) = &st.goal {
                let obj = goal["objective"].as_str().unwrap_or_default().to_owned();
                let status = goal["status"].as_str().unwrap_or_default().to_owned();
                let mut badge = status.clone();
                if let Some(first) = badge.get_mut(0..1) {
                    first.make_ascii_uppercase();
                }
                let used = goal["tokens_used"].as_u64().unwrap_or(0);
                let budget = goal["token_budget"].as_u64().unwrap_or(0);
                texts.push(("t_goal".into(), obj, None));
                texts.push(("goal_badge_label".into(), badge, None));
                texts.push((
                    "t_budget_val".into(),
                    format!("{} / {}", format_tokens(used), format_tokens(budget)),
                    None,
                ));
                texts.push((
                    "t_elapsed_val".into(),
                    elapsed_atlas(goal["time_used_seconds"].as_u64().unwrap_or(0)),
                    None,
                ));
                // The bar's fill is the goal.fill binding (used/budget).
                let fill = if budget > 0 { used as f64 / budget as f64 } else { 0.0 };
                set_w.push(("bar_fill".into(), (g("bar_track").1 * fill) as f32));
            }
        }
        Screen3::Loops => {
            let n = st.loops.len().min(3);
            for i in 0..n {
                let row = &st.loops[i];
                texts.push((
                    (format!("loop_{}_name", i + 1)),
                    row["prompt"].as_str().unwrap_or_default().to_owned(),
                    None,
                ));
                texts.push((
                    (format!("loop_{}_cad", i + 1)),
                    cadence(row),
                    None,
                ));
            }
            for i in (n + 1)..=3 {
                for id in geo.keys() {
                    if id.starts_with(&format!("loop_{i}_")) {
                        hide.insert(id.clone());
                    }
                }
            }
            if n == 0 {
                // The 0-item line renders in the body-font slot (loop_1_name,
                // Inter-500 — never the mono command slot; AutonomyPanel.tsx:238).
                texts.push((
                    "loop_1_name".into(),
                    "No loops in this session.".into(),
                    Some(280.0),
                ));
                for id in geo.keys() {
                    if id.starts_with("loop_1_") && id != "loop_1_name" {
                        hide.insert(id.clone());
                    }
                }
            }
            // The card sizes to its visible rows (authored pad preserved).
            let card = g("loops_card");
            let row_bottom = |i: u32| {
                (1..=i)
                    .flat_map(|r| {
                        geo.keys()
                            .filter(move |id| id.starts_with(&format!("loop_{r}_")))
                            .map(move |id| g(id))
                    })
                    .map(|(y, _w, h)| y + h)
                    .fold(0.0_f64, f64::max)
            };
            let pad = card.2 - (row_bottom(3) - card.0);
            let visible_bottom = if n == 0 {
                g("loop_1_name").0 + g("loop_1_name").2
            } else {
                row_bottom(n as u32)
            };
            set_h.push(("loops_card".into(), ((visible_bottom + pad) - card.0) as f32));
        }
        Screen3::Monitors => {
            let n = st.monitors.len();
            for i in 0..n.min(2) {
                let m = &st.monitors[i];
                if m["status"].as_str() == Some("paused") {
                    play_ids.push(format!("mon_{}_pause", i + 1));
                }
                let argv = m["argv"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_default();
                texts.push((
                    format!("mon_{}_cmd", i + 1),
                    fit_cmd(argv, MON_CMD_W, 14.21),
                    None,
                ));
                let mut state_txt = m["status"].as_str().unwrap_or_default().to_owned();
                if let Some(reason) = m["pause_reason"].as_str() {
                    state_txt = format!("{} ({})", state_txt, reason);
                }
                texts.push((format!("mon_{}_state", i + 1), state_txt, None));
                texts.push((
                    format!("mon_{}_int", i + 1),
                    interval_short(m["interval_seconds"].as_u64()),
                    None,
                ));
            }
            // A third item rides the CLONED card (the clone pass below copies
            // card 2's shape; the apply pass then writes these values into
            // the mon_3_* ids).
            if let Some(m3) = st.monitors.get(2) {
                if m3["status"].as_str() == Some("paused") {
                    play_ids.push("mon_3_pause".into());
                }
                let argv = m3["argv"]
                    .as_array()
                    .map(|a| {
                        a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(" ")
                    })
                    .unwrap_or_default();
                texts.push(("mon_3_cmd".into(), fit_cmd(argv, MON_CMD_W, 14.21), None));
                texts.push((
                    "mon_3_state".into(),
                    m3["status"].as_str().unwrap_or_default().to_owned(),
                    None,
                ));
                texts.push((
                    "mon_3_int".into(),
                    interval_short(m3["interval_seconds"].as_u64()),
                    None,
                ));
            }
            for i in (n.min(2) + 1)..=2 {
                for id in geo.keys() {
                    if *id == format!("mon_{i}") || id.starts_with(&format!("mon_{i}_")) {
                        hide.insert(id.clone());
                    }
                }
            }
            let pitch = g("mon_2").0 - g("mon_1").0;
            // The last visible card's bottom: 0 items → the first slot's top;
            // 1 item → card 1's bottom; n≥2 → card 2 shifted by the pitch.
            let last_bottom = |n: usize| {
                if n == 0 {
                    g("mon_1").0
                } else if n == 1 {
                    g("mon_1").0 + g("mon_1").2
                } else {
                    g("mon_2").0 + pitch * (n - 2) as f64 + g("mon_2").2
                }
            };
            // The footer follows the last card with the authored gap.
            let authored_gap = g("monitors_footer").0 - (g("mon_2").0 + g("mon_2").2);
            let shift = (last_bottom(n) + authored_gap) - g("monitors_footer").0;
            set_y.push(("monitors_footer".into(), g("monitors_footer").0 + shift));
            set_y.push((
                "monitors_footer_label".into(),
                g("monitors_footer_label").0 + shift,
            ));
            let active = st
                .monitors
                .iter()
                .filter(|m| m["status"].as_str() == Some("active"))
                .count();
            texts.push((
                "monitors_footer_label".into(),
                if n == 0 {
                    "No monitors in this session.".to_owned()
                } else {
                    format!(
                        "{n} {} · {active} active",
                        if n == 1 { "monitor" } else { "monitors" }
                    )
                },
                None,
            ));
        }
    }

    // ---- clone the authored card for stores larger than the template (the
    // lowering-layer instantiation; the mounted PortalList generalizes it).
    if screen == Screen3::Monitors && st.monitors.len() > 2 {
        let n = st.monitors.len();
        let pitch = g("mon_2").0 - g("mon_1").0;
        let mut work: Vec<&mut _> = vec![tree];
        let mut done = false;
        while let Some(node) = work.pop() {
            if !done {
                let at = node.children.iter().position(|c| c.attrs.id.as_deref() == Some("mon_2"));
                if let Some(at) = at {
                    for k in 3..=n {
                        let mut extra = node.children[at].clone();

                        let mut sub = vec![&mut extra];
                        while let Some(x) = sub.pop() {
                            if let Some(id) = &x.attrs.id {
                                if id.contains("mon_2") {
                                    x.attrs.id =
                                        Some(id.replace("mon_2", &format!("mon_{k}")));
                                }
                            }
                            x.attrs.y = x.attrs.y.map(|y| y + pitch * (k - 2) as f64);
                            for c in &mut x.children {
                                sub.push(c);
                            }
                        }
                        node.children.insert(at + k - 2, extra);
                    }
                    done = true;
                }
            }
            for c in &mut node.children {
                work.push(c);
            }
        }
    }

    // ---- #32c backlog 7/10: board-3 rows render at atlas size (the
    // accepted-8.5 note: text/icons ~10% small) and a paused monitor shows
    // the RESUME icon — the design's own play asset (autonomy-04's; the
    // monitors card only ever authored pause). Icons re-centre on growth.
    const PLAY_SRC: &str =
        "http://127.0.0.1:8170/ux-images/autonomy-04/assets/loop_1_play-7d9f31b010d1.svg";
    // #32c2 item 2: ONE width budget for every monitor command — the free
    // space from the cmd column to the interval column (mon_1_int x 214.94 −
    // an 8px gap − cmd x 29.8). The authored boxes were uneven (114.88 vs
    // 157.53), so row 1 ellipsized while rows 2-3 showed the same string.
    const MON_CMD_W: f64 = 177.0;
    if screen == Screen3::Loops || screen == Screen3::Monitors {
        let mut work = vec![&mut *tree];
        while let Some(n) = work.pop() {
            if let Some(id) = n.attrs.id.clone() {
                let row = (id.starts_with("loop_")
                    && id[5..].chars().next().is_some_and(|c| c.is_ascii_digit()))
                    || (id.starts_with("mon_")
                        && id[4..].chars().next().is_some_and(|c| c.is_ascii_digit()));
                if row {
                    if let Some(sz) = n.attrs.size.as_mut() {
                        *sz *= 1.1;
                    }
                    // The name/cadence boxes are authored to the SMALLER font
                    // (and unevenly per row — the after capture clipped
                    // "r1 replay probe" mid-word on row 2, and row 1 sat
                    // 3-13px right of rows 2-3: authored name x 23.85/22.38/
                    // 22.33, cad x 28.75/22.38/22.38). #32c2 item 1: ONE left
                    // edge — x=22.38 (rows 2-3's authored edge), width to the
                    // status-dot column (loop_N_dot x=208).
                    if screen == Screen3::Loops
                        && (id.ends_with("_name") || id.ends_with("_cad"))
                    {
                        n.attrs.x = Some(22.38);
                        n.attrs.w = Some((208.0 - 12.0 - 22.38) as f32);
                    }
                    if screen == Screen3::Monitors && id.ends_with("_cmd") {
                        n.attrs.w = Some(MON_CMD_W as f32);
                    }
                    let icon = ["dot", "pause", "play", "trash", "clock"]
                        .iter()
                        .any(|k| id.contains(k));
                    if icon {
                        // #32c2 item 5: re-centre by the ACTUAL growth
                        // (Δ = 0.1·w, not a flat 1.2px) — the 44px trash
                        // needs 2.2px or its right side crosses the card's
                        // inner edge (384) and clips.
                        let (ow, oh) = (n.attrs.w.unwrap_or(0.0), n.attrs.h.unwrap_or(0.0));
                        if let Some(w) = n.attrs.w.as_mut() {
                            *w *= 1.1;
                        }
                        if let Some(h) = n.attrs.h.as_mut() {
                            *h *= 1.1;
                        }
                        if let Some(x) = n.attrs.x.as_mut() {
                            *x -= f64::from(ow * 0.05);
                        }
                        if let Some(y) = n.attrs.y.as_mut() {
                            *y -= f64::from(oh * 0.05);
                        }
                    }
                    if id.ends_with("_pause") && play_ids.iter().any(|p| p == &id) {
                        n.attrs.src = Some(PLAY_SRC.to_owned());
                    }
                }
            }
            for c in &mut n.children {
                work.push(c);
            }
        }
    }

    // ---- apply the edits (mutable walk; text nodes blank before w=0 so the
    // lowered tree never carries content in a zero box).
    let mut work = vec![&mut *tree];
    while let Some(n) = work.pop() {
        if let Some(id) = n.attrs.id.clone() {
            if hide.contains(&id) {
                if n.attrs.text.is_some() {
                    n.attrs.text = Some(String::new());
                }
                n.attrs.w = Some(0.0);
            }
            for (tid, text, w) in &texts {
                if &id == tid {
                    n.attrs.text = Some(text.clone());
                    if let Some(w) = w {
                        n.attrs.w = Some(*w as f32);
                    }
                }
            }
            for (wid, w) in &set_w {
                if &id == wid {
                    n.attrs.w = Some(*w);
                }
            }
            for (hid, h) in &set_h {
                if &id == hid {
                    n.attrs.h = Some(*h);
                }
            }
            for (yid, y) in &set_y {
                if &id == yid {
                    n.attrs.y = Some(*y);
                }
            }
        }
        for c in &mut n.children {
            work.push(c);
        }
    }

    let mut measured: HashMap<String, (f64, f64, f64)> = HashMap::new();
    let mut walk = vec![&*tree];
    while let Some(n) = walk.pop() {
        if let Some(id) = &n.attrs.id {
            measured.insert(
                id.clone(),
                (
                    n.attrs.y.unwrap_or(0.0),
                    n.attrs.w.unwrap_or(0.0) as f64,
                    n.attrs.h.unwrap_or(0.0) as f64,
                ),
            );
        }
        for c in &n.children {
            walk.push(c);
        }
    }
    let inventory = octoscript_makepad::l0::inspectable(tree);
    let dsl = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(tree))
        .map_err(|e| format!("to_makepad_ui: {e}"))?;
    Ok(Lowered { dsl, card, inventory, measured })
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

/// The web's interval ladder (model.ts:138-146): "every Ns"/"every Nm"/
/// "hourly"/"every Nh"/"every minute"/"self-paced".
fn format_interval(seconds: Option<u64>) -> String {
    let Some(seconds) = seconds else {
        return "self-paced".to_owned();
    };
    if seconds % 60 == 0 {
        let minutes = seconds / 60;
        if minutes % 60 == 0 {
            let hours = minutes / 60;
            return if hours == 1 { "hourly".to_owned() } else { format!("every {hours}h") };
        }
        return if minutes == 1 { "every minute".to_owned() } else { format!("every {minutes}m") };
    }
    format!("every {seconds}s")
}

fn cadence(loop_row: &Value) -> String {
    format_interval(loop_row["interval_seconds"].as_u64())
}

/// The compact ladder for the monitor row's narrow slot: "1h"/"30m"/"30s".
fn interval_short(seconds: Option<u64>) -> String {
    let Some(seconds) = seconds else {
        return "self-paced".to_owned();
    };
    if seconds % 3600 == 0 {
        format!("{}h", seconds / 3600)
    } else if seconds % 60 == 0 {
        format!("{}m", seconds / 60)
    } else {
        format!("{seconds}s")
    }
}

/// Atlas-granular elapsed ('0s'/'18m'/'1h 30m'); the web renders no elapsed
/// formatter, so this is implemented per entry #30b2 (no cite).
fn elapsed_atlas(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    let rest = minutes % 60;
    if rest == 0 {
        format!("{hours}h")
    } else {
        format!("{hours}h {rest}m")
    }
}

/// Width-aware command ellipsis (#32c backlog 10): render verbatim while the
/// string fits its slot, truncate only then. Capacity from the mono metrics:
/// an l0 size renders at `size * 0.75` px (`octoscript-makepad design.rs:387`)
/// and the rendered mono advance measures ≈0.78em with the kit's tracking
/// (an 18-char command ≈164px in the 157.53px authored slot at the scaled
/// size — the #32c after capture).
fn fit_cmd(argv: String, slot_w: f64, size_sp: f64) -> String {
    let px = size_sp * 1.1 * 0.75;
    let capacity = ((slot_w / (px * 0.78)).floor() as usize).max(4);
    if argv.chars().count() <= capacity {
        return argv;
    }
    ellipsize(argv, capacity)
}

/// Ellipsize past `max` chars to `max-1` + '…' (a display string, so the
/// card's measured slot keeps the text off its edge).
fn ellipsize(text: String, max: usize) -> String {
    if text.chars().count() <= max {
        return text;
    }
    let head: String = text.chars().take(max - 1).collect();
    format!("{head}…")
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
        // The card shows used vs budget on a bar: the FILL is the binding.
        "goal.fill" => json!(st.goal.as_ref().and_then(|g| {
            let budget = g["token_budget"].as_u64().filter(|b| *b > 0)?;
            Some(g["tokens_used"].as_u64().unwrap_or(0) as f64 / budget as f64)
        })),
        // Atlas granularity ('0s'/'18m'/'1h 30m'); the web renders no elapsed
        // formatter, so this is implemented per entry #30b2 (no cite).
        "goal.elapsed" => json!(st.goal.as_ref().map(|g| {
            elapsed_atlas(g["time_used_seconds"].as_u64().unwrap_or(0))
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
        "loops.count" => json!(st.loops.len()),
        "loops.empty" => json!(if st.loops.is_empty() { "No loops in this session." } else { "" }),
        "loops.available" => json!(advertised(store, "loop/list")),
        "monitors" => json!(st
            .monitors
            .iter()
            .map(|m| json!({
                "id": m["monitor_id"],
                "name": m["name"],
                "cmd": ellipsize(
                    m["argv"].as_array().map(|a| {
                        a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(" ")
                    }).unwrap_or_default(),
                    13,
                ),
                "status": m["status"],
                // The monitor card's interval slot is narrow (the atlas shows
                // "30s"): the compact ladder the entry names — "1h"/"30m"/"30s"
                // (the minutes/seconds steps of formatInterval, model.ts:138-146).
                "interval": interval_short(m["interval_seconds"].as_u64()),
            }))
            .collect::<Vec<Value>>()),
        "monitors.count" => json!(st.monitors.len()),
        "monitors.empty" => json!(if st.monitors.is_empty() { "No monitors in this session." } else { "" }),
        "monitors.available" => json!(advertised(store, "monitor/list")),
        "monitors.footer" => {
            let total = st.monitors.len();
            let active = st
                .monitors
                .iter()
                .filter(|m| m["status"].as_str() == Some("active"))
                .count();
            json!(format!(
                "{total} {} · {active} active",
                if total == 1 { "monitor" } else { "monitors" }
            ))
        }
        _ => return None,
    })
}
