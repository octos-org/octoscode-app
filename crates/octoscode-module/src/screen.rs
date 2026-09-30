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
    /// The turn this row belongs to (**card #21j**). The settled tail rows
    /// (`worked-for`, `answer-actions`) render from THEIR OWN turn's terminal,
    /// so a later turn can never relabel an earlier one.
    pub turn: Option<String>,
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
            out.push(Row { kind: ItemKind::UserBubble, index: *i, turn: turn_of(turn) });
        }
        // assistant-prose: the turn's FINAL assistant text (deltas folded).
        if let Some((i, _, _)) = group
            .iter()
            .filter(|(_, k, t)| *k == EntryKind::ASSISTANT_TEXT && !t.is_empty())
            .next_back()
        {
            out.push(Row { kind: ItemKind::AssistantProse, index: *i, turn: turn_of(turn) });
        }
        // tool-cell × N: the turn's tool calls, in order. The `index` is the
        // tool's ordinal within the turn (the `tools[]` binding's own index).
        let tool_calls = group.iter().filter(|(_, k, _)| *k == EntryKind::TOOL_CALL).count();
        for k in 0..tool_calls {
            out.push(Row { kind: ItemKind::ToolCell, index: k, turn: turn_of(turn) });
        }
        // tail: a live last turn shows the activity row; a settled turn shows
        // the worked-for disclosure + the answer actions.
        // #32i item 1: the web removes the working row when the turn
        // completes ("Worked for Ns", timeline/model.ts). The global live
        // flag can OUTLIVE the turn — `turn.active` ORs the session row's
        // server view (bindings.rs:192-201), which is sticky until the next
        // `session/list` — so the LIVE tail renders only while THIS turn has
        // no terminal yet. A settled last turn discloses, even with
        // live=true (the "Working · 0s" leftover: flow had cleared, the row
        // fell back to the authored placeholder).
        let live_tail =
            is_last && live && store.domains.turn.terminal(turn).is_none();
        if live_tail {
            out.push(Row { kind: ItemKind::WorkingRow, index: 0, turn: turn_of(turn) });
        // **Card #21j**: the settled tail discloses a turn's OWN terminal, so it
        // renders only when that turn actually settled or produced a reply. A
        // turn with neither (e.g. a prompt whose `turn/start` never came back —
        // a replay-driven capture mints such a group) has nothing to disclose;
        // before #21j it rendered a blank pill carrying another turn's label.
        } else if !group.is_empty() && turn_settled_or_replied(store, turn, group) {
            out.push(Row { kind: ItemKind::WorkedFor, index: 0, turn: turn_of(turn) });
            out.push(Row { kind: ItemKind::AnswerActions, index: 0, turn: turn_of(turn) });
        }
    }
    out
}

/// Did `turn` record a terminal, or produce a reply (answer text / a tool call)?
///
/// The settled tail is a turn's outcome disclosure (**card #21j**), so a turn
/// that neither settled nor replied has nothing to show. A lone user message is
/// not a reply — that is the group a never-answered dispatch leaves behind.
fn turn_settled_or_replied(
    store: &Arc<Store>,
    turn: &str,
    group: &[(usize, EntryKind, String)],
) -> bool {
    if !turn.is_empty() && store.domains.turn.terminal(turn).is_some() {
        return true;
    }
    group.iter().any(|(_, kind, text)| {
        (*kind == EntryKind::ASSISTANT_TEXT && !text.is_empty()) || *kind == EntryKind::TOOL_CALL
    })
}

/// A grouped turn's key is its id; `""` (an entry with no `turn_id`) maps to
/// `None`, which the bindings read as "no per-turn value" (**card #21j**).
fn turn_of(turn: &str) -> Option<String> {
    if turn.is_empty() {
        None
    } else {
        Some(turn.to_owned())
    }
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
    /// Lower one item. `turn` is the row's own turn id (**card #21j**), so a
    /// settled tail renders from its own terminal; it is part of the cache key
    /// (two rows of the same kind+index can differ only by turn).
    pub fn lower(
        &mut self,
        bridge: &Arc<Mutex<Bridge>>,
        kind: ItemKind,
        index: usize,
        turn: Option<&str>,
    ) -> Result<String, String> {
        let copies = {
            let b = bridge.lock().unwrap();
            let ctx = crate::bindings::Ctx::new(&b.store, &b.ui);
            components::item_copies(kind, &ctx, index, turn)?
        };
        let key = format!(
            "{}:{}:{}:{}",
            kind.id(),
            index,
            turn.unwrap_or(""),
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

#[cfg(test)]
mod tests {
    //! The row model + the lowering cache: pure, no window, no transport.
    use super::*;
    use octoscode_store::Session;

    /// A store with one active session and `n` timeline entries.
    fn store_with(n: usize) -> Arc<Store> {
        let store = Arc::new(Store::new());
        store.set_active(Some("s1".into()));
        store.set_sessions(vec![Session {
            id: "s1".into(),
            title: Some("T".into()),
            message_count: 0,
            updated_at: None,
            last_prompt: None,
            active_turn: false,
        }]);
        let tl = &store.domains.session.timeline;
        for i in 0..n {
            tl.append(
                "s1",
                Some("t1".into()),
                octoscode_store::EntryKind::TOOL_CALL,
                format!("tool {i}"),
            );
        }
        store
    }

    /// A `Bridge` (the widget's own holder) over a store — reachable here because
    /// this is the same crate.
    fn bridge(store: Arc<Store>) -> Arc<Mutex<crate::Bridge>> {
        Arc::new(Mutex::new(crate::Bridge {
            conv: None,
            store,
            ui: flow_ui(),
            screens: Arc::new(Mutex::new(crate::screens::connect::ConnectUi::default())),
        }))
    }

    #[test]
    fn thread_rows_are_the_session_list_with_the_active_flag() {
        let store = store_with(0);
        let rows = thread_rows(&store);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "T");
        assert_eq!(rows[0].meta, "0 messages");
        assert!(rows[0].active);
    }

    #[test]
    fn display_order_is_user_first_reasoning_folded_answer_then_tools() {
        let store = store_with(0);
        let tl = &store.domains.session.timeline;
        // Arrival order is deliberately hostile: a delta arrives BEFORE the
        // canonical user message (the real server sends it at seq 154, behind
        // 153 delta frames) — display order must still put the user first.
        tl.append_delta("s1", Some("t1"), octoscode_store::EntryKind::ASSISTANT_TEXT, "par");
        tl.append_delta("s1", Some("t1"), octoscode_store::EntryKind::REASONING, "thinking…");
        tl.append_delta("s1", Some("t1"), octoscode_store::EntryKind::ASSISTANT_TEXT, "tial answer");
        tl.append_data(
            "s1",
            Some("t1".into()),
            octoscode_store::EntryKind::TOOL_CALL,
            "read_file".into(),
            serde_json::json!({}),
        );
        tl.upsert_user_message("s1", "t1", "why 5?", serde_json::json!({}));

        let rows = timeline_rows(&store, false);
        let kinds: Vec<ItemKind> = rows.iter().map(|r| r.kind).collect();
        assert_eq!(
            kinds,
            vec![
                ItemKind::UserBubble,      // user FIRST (not its arrival slot)
                ItemKind::AssistantProse,  // the folded answer
                ItemKind::ToolCell,        // the tool call
                ItemKind::WorkedFor,       // settled tail
                ItemKind::AnswerActions,
            ],
            "reasoning is folded (no row); user first; answer before tools"
        );
        // The user row projects the folded user entry.
        let entries = store.domains.session.timeline.entries("s1");
        let user_idx = rows[0].index;
        assert_eq!(entries[user_idx].text, "why 5?");
    }

    #[test]
    fn a_settled_last_turn_discloses_even_when_the_live_flag_is_stuck() {
        // #32i item 1: "Working · 0s" lingered after the answer finished —
        // the global live flag outlived the turn, so the web's remove-on-
        // complete never happened. The row model must consult the turn's OWN
        // terminal: settled => "Worked for Ns", never the working row.
        let store = store_with(0);
        store
            .domains
            .session
            .timeline
            .upsert_user_message("s1", "t1", "hi", serde_json::json!({}));
        store.domains.turn.set_terminal("t1", "completed");
        let rows = timeline_rows(&store, true);
        assert!(
            rows.iter().any(|r| r.kind == ItemKind::WorkedFor),
            "a settled last turn shows the worked-for disclosure"
        );
        assert!(
            !rows.iter().any(|r| r.kind == ItemKind::WorkingRow),
            "no working row may linger after the terminal"
        );
    }

    #[test]
    fn a_live_last_turn_shows_the_working_row_instead_of_worked_for() {
        let store = store_with(0);
        store
            .domains
            .session
            .timeline
            .upsert_user_message("s1", "t1", "hi", serde_json::json!({}));
        let rows = timeline_rows(&store, true);
        assert!(
            rows.iter().any(|r| r.kind == ItemKind::WorkingRow),
            "a live turn shows the activity row"
        );
        assert!(!rows.iter().any(|r| r.kind == ItemKind::WorkedFor));
    }

    #[test]
    fn a_2000_entry_timeline_builds_2000_rows_quickly() {
        // The row model itself must be cheap: the virtualization (only the
        // visible ~20 items lower) is the `PortalList`'s job, proven by
        // `examples/pl_probe.rs` (40 items -> 4 `Splash` rows).
        let store = store_with(2000);
        let t = std::time::Instant::now();
        let rows = timeline_rows(&store, false);
        let elapsed = t.elapsed();
        assert_eq!(rows.len(), 2002, "2000 tool rows + worked-for + answer-actions");
        assert!(
            elapsed < std::time::Duration::from_secs(2),
            "row model took {elapsed:?} for 2000 entries"
        );
        // Only rows that need a component are `ToolCell` — 2000 of them.
        assert_eq!(
            rows.iter().filter(|r| r.kind == ItemKind::ToolCell).count(),
            2000
        );
    }

    #[test]
    fn the_cache_lowers_once_per_distinct_item() {
        let store = store_with(3);
        let b = bridge(store);
        // Give the 3 tool rows DISTINCT names, so the cache sees 3 distinct items
        // (identical items share one entry — which is the point of keying on the
        // values, not the index).
        {
            let mut bb = b.lock().unwrap();
            let mut ui = bb.ui.lock().unwrap();
            for i in 0..3 {
                ui.note_tool_started_for_test(&format!("c{i}"), &format!("tool{i}"));
            }
        }
        let mut cache = Cache::default();
        for i in 0..3 {
            assert!(cache.lower(&b, ItemKind::ToolCell, i, None).is_ok());
        }
        let after_first = cache.len();
        assert_eq!(after_first, 3, "one entry per distinct item");
        for i in 0..3 {
            assert!(cache.lower(&b, ItemKind::ToolCell, i, None).is_ok());
        }
        assert_eq!(cache.len(), after_first, "a redraw is a cache hit");
    }
}
