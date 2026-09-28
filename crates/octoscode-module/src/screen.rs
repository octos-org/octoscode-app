//! Card #17 — the **conversation screen's row model**: what each virtualized
//! list instantiates, in what order, and how each item is lowered.
//!
//! The layout and the two `PortalList`s live in [`crate::lib`]'s `script_mod!`;
//! this module is the pure part — the display order and the per-item lowering —
//! so both are testable without a window.
//!
//! ## Display order (card #17)
//!
//! The store's timeline is **arrival** order; a real turn can send the user
//! message *after* 150 delta frames (`upsertUserMessage`, `timeline.rs`). The
//! screen shows **display** order:
//!
//! ```text
//! per turn (turns in first-seen order):
//!   user-bubble                     (the person's message — FIRST)
//!   assistant-prose                 (the answer; reasoning is folded away)
//!   tool-cell × N                   (the turn's tool calls, in order)
//!   tail: working-row (if the turn is live) | worked-for + answer-actions
//! ```
//!
//! ## Per-item lowering
//!
//! [`lower_cached`] lowers [`components::ItemKind`] with the item's own values
//! ([`components::item_copies`]) through the L0 chain, memoised by
//! `(kind, index, values)` so a redraw of an unchanged row is a cache hit — a
//! `PortalList` re-instantiates its visible rows every frame, so without the
//! cache a 2,000-entry list would re-lower ~20 items per frame.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use octoscode_store::timeline::EntryKind;
use octoscode_store::Store;

use crate::components::{self, ItemKind};
use crate::flow::FlowUi;
use crate::Bridge;

/// One thread-list row (the left column).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRow {
    pub title: String,
    pub meta: String,
    pub active: bool,
}

/// One timeline row: the component to instantiate, and the index its per-item
/// binding projects onto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub kind: ItemKind,
    /// The binding index (`timeline.entries[index]`, or `tools[index]`).
    pub index: usize,
}

/// The left column's rows, from the store's session list.
pub fn thread_rows(store: &Arc<Store>) -> Vec<ThreadRow> {
    let active = store.active_session();
    store
        .sessions()
        .into_iter()
        .map(|s| ThreadRow {
            title: s.title.unwrap_or_else(|| s.id.clone()),
            meta: format!("{} messages", s.message_count),
            active: active.as_deref() == Some(s.id.as_str()),
        })
        .collect()
}

/// The timeline rows (the center column), in display order. `live` = a turn is
/// in flight (`turn.active`), which appends a `working-row` to the last turn.
pub fn timeline_rows(store: &Arc<Store>, live: bool) -> Vec<Row> {
    let Some(session) = store.active_session() else {
        return Vec::new();
    };
    let entries = store.domains.session.timeline.entries(&session);

    // Group by turn id, turns in first-seen order (the store's arrival order
    // decides which turn is "later"; within a turn display order is fixed).
    let mut turn_order: Vec<String> = Vec::new();
    let mut by_turn: HashMap<String, Vec<(usize, EntryKind, String)>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        let key = e.turn_id.clone().unwrap_or_default();
        if !by_turn.contains_key(&key) {
            turn_order.push(key.clone());
        }
        by_turn.entry(key).or_default().push((i, e.kind, e.text.clone()));
    }

    let mut out = Vec::new();
    for (ti, turn) in turn_order.iter().enumerate() {
        let group = &by_turn[turn];
        let is_last = ti + 1 == turn_order.len();

        // user-bubble: the FIRST user message of the turn.
        if let Some((i, _, _)) = group.iter().find(|(_, k, _)| *k == EntryKind::USER_MESSAGE) {
            out.push(Row { kind: ItemKind::UserBubble, index: *i });
        }
        // assistant-prose: the turn's FINAL assistant text (deltas folded).
        if let Some((i, _, _)) = group
            .iter()
            .filter(|(_, k, t)| *k == EntryKind::ASSISTANT_TEXT && !t.is_empty())
            .next_back()
        {
            out.push(Row { kind: ItemKind::AssistantProse, index: *i });
        }
        // tool-cell × N: the turn's tool calls, in order (index = tool order).
        for (k, (i, kind, _)) in group
            .iter()
            .filter(|(_, k, _)| *k == EntryKind::TOOL_CALL)
            .enumerate()
        {
            let _ = i;
            let _ = k;
            out.push(Row { kind: ItemKind::ToolCell, index: k });
        }
        // tail: a live last turn shows the activity row; a settled turn shows
        // the worked-for disclosure + the answer actions.
        if is_last && live {
            out.push(Row { kind: ItemKind::WorkingRow, index: 0 });
        } else if !group.is_empty() {
            out.push(Row { kind: ItemKind::WorkedFor, index: 0 });
            out.push(Row { kind: ItemKind::AnswerActions, index: 0 });
        }
    }
    out
}

/// A memoised lowering: `(kind, index, values-json)` → the Splash DSL.
#[derive(Default)]
pub struct Cache {
    map: HashMap<String, String>,
}

/// Cap the cache so a long scroll cannot grow it without bound (the values of
/// the rows that scrolled away are evicted first — insertion order is dropped
/// wholesale at the cap, which is enough: rows that stay visible re-lower once).
const CACHE_CAP: usize = 512;

impl Cache {
    /// The lowered DSL for one item, from the cache or by lowering it now.
    ///
    /// `Err` names the reason the item fell back (a missing component, or a
    /// binding that does not resolve).
    pub fn lower(&mut self, bridge: &Arc<Mutex<Bridge>>, kind: ItemKind, index: usize) -> Result<String, String> {
        let copies = {
            let b = bridge.lock().unwrap();
            let ctx = crate::bindings::Ctx::new(&b.store, &b.ui);
            components::item_copies(kind, &ctx, index)?
        };
        let key = format!(
            "{}:{}:{}",
            kind.id(),
            index,
            serde_json::to_string(&copies).unwrap_or_default()
        );
        if let Some(hit) = self.map.get(&key) {
            return Ok(hit.clone());
        }
        let dsl = components::lower(kind, &index.to_string(), &copies)?;
        if self.map.len() >= CACHE_CAP {
            self.map.clear();
        }
        self.map.insert(key, dsl.clone());
        Ok(dsl)
    }

    /// How many lowerings are memoised (a test reads this).
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// The flow's UI state (so a test can build a store + UI and drive the rows).
pub fn flow_ui() -> Arc<Mutex<FlowUi>> {
    Arc::new(Mutex::new(FlowUi::default()))
}
