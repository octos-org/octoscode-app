//! The L0 binding table: the ONLY door between store data and cards.
//!
//! **Rule (8.8 condition 2):** cards never see Rust types. A card asks for a
//! value by a **binding id** (a stable string) and gets JSON back; it sends an
//! **action id** and the host performs it. This module owns the mapping, so a
//! card's contract is a small, declared table — not a struct it must mirror.
//!
//! ## The conversation bindings (card #12 §3)
//!
//! [`WEB_BINDINGS`] implements every id in
//! `design/stage-b/conversation/bindings.json` (24 ids: the 5 live-gate
//! components), on top of the 7 the module already had. Each id's doc below
//! names its JSON shape and where the value comes from — the store, or the
//! flow's UI state ([`crate::flow::FlowUi`]) for what the wire never carries.
//!
//! Add a binding: one arm in [`query`]. Add an action: one arm in
//! [`is_action`] (the module performs it against the conversation).
use std::sync::{Arc, Mutex};

use octoscode_store::Store;
use serde_json::{json, Value};

use crate::flow::FlowUi;

/// The pre-#12 bindings, as `(id, description)`.
pub const BINDINGS: &[(&str, &str)] = &[
    ("conn.state", "the connection state as display text, e.g. \"Live\""),
    ("conn.live", "whether the connection is live (bool)"),
    ("session.count", "the number of sessions in the list (int)"),
    ("session.list", "the session rows: [{id,title,message_count,active_turn}]"),
    ("session.active", "the active session id, or null"),
    ("caps.count", "the number of accepted capabilities (int)"),
    ("summary", "a one-line \"conn: … sessions: …\" summary"),
];

/// The 24 ids from `design/bindings.json` — the 5 live-gate components
/// (thread list, streaming turn, tool cells, composer states, completed
/// answer). `[].` ids project one field across their list's rows.
pub const WEB_BINDINGS: &[(&str, &str)] = &[
    // --- conversation-01: THREAD LIST -----------------------------------
    ("threads", "list: the session rows [{id,title,message_count,active_turn}]"),
    ("threads[].title", "text: each thread's title (null when unnamed)"),
    ("threads[].active", "bool: is this the active session"),
    ("threads.active", "text: the active session id, or null"),
    // --- conversation-03/08: STREAMING TURN + COMPOSER ------------------
    ("turn.active", "bool: a turn is live (STOP vs send)"),
    ("timeline.entries", "list: [{kind, text, turn_id}] for the active session"),
    ("timeline.entries[].text", "text: each entry's text"),
    ("timeline.entries[].kind", "text: each entry's kind tag"),
    ("turn.activity", "text: \"Working · 12s\" while a turn is live, else \"\""),
    ("composer.draft", "text: the composer input's current value"),
    ("composer.placeholder", "text: the idle placeholder copy"),
    // --- conversation-04: TOOL CELLS ------------------------------------
    ("tools", "list: [{name, summary, status}] for the turn's tool calls"),
    ("tools[].summary", "text: each tool row's summary"),
    ("tools[].status", "text: running | done | failed"),
    ("tools[].expanded", "bool: is this row's output disclosed (UI-local)"),
    ("tool.output", "list: the completed tools' output previews"),
    // --- conversation-09: COMPLETED ANSWER ------------------------------
    ("answer.worked_for", "text: \"Worked for 3m 4s\" for the last turn"),
    ("answer.timestamp", "text: when the last turn completed"),
    ("approval.pending", "bool: an approval/requested is outstanding"),
    ("question.pending", "bool: a user_question/requested is outstanding"),
];

/// The 4 conversation ids whose `bindings.json` `type` is `action` — the
/// module performs them; they are not `query`-resolvable data ids.
pub const WEB_ACTION_IDS: &[(&str, &str)] = &[
    ("turn.steer", "turn/steer the live turn's input buffer"),
    ("turn.interrupt", "turn/interrupt the live turn"),
    ("composer.submit", "turn/start with the current composer draft"),
    ("answer.expand", "toggle the worked-for disclosure (UI-local)"),
];

/// Every `query`-resolvable data id: the 7 pre-#12 ids + the 20 conversation
/// data ids (the 24 conversation ids minus the 4 `action` ones).
pub fn all_binding_ids() -> Vec<&'static str> {
    BINDINGS
        .iter()
        .chain(WEB_BINDINGS.iter())
        .map(|(id, _)| *id)
        .collect()
}

/// Every conversation id in `design/bindings.json` (data + action) — the
/// audit that all 24 are covered by `query` or `is_action`.
pub fn all_conversation_ids() -> Vec<&'static str> {
    WEB_BINDINGS
        .iter()
        .chain(WEB_ACTION_IDS.iter())
        .map(|(id, _)| *id)
        .collect()
}

/// The declared actions, as `(id, description)`. `session.refresh` predates
/// #12; the rest are `design/bindings.json`'s `client_action`s.
pub const ACTIONS: &[(&str, &str)] = &[
    ("session.refresh", "re-ask the server for the session list"),
    ("session.new", "start a NEW chat: mint a fresh session id and open it"),
    ("composer.submit", "turn/start with the current composer draft"),
    ("turn.interrupt", "turn/interrupt the live turn"),
    ("turn.steer", "turn/steer the live turn's input buffer"),
    ("answer.expand", "toggle the worked-for disclosure (UI-local)"),
    // Card #21 §3 — the per-item control actions the #16 components emit.
    ("thread.open", "open the clicked thread row's session (`row.id`)"),
    ("answer.copy", "copy the answer text (UI-local; the clipboard is the host's)"),
    ("tool.toggle", "toggle a tool cell's output disclosure (UI-local)"),
    // Screen actions (#29d palette, #29a/#29c workspace/connect/models) are NOT
    // declared here: each screen owns its own table (the one-owner rule,
    // #29d3) — `screens::palette::owns_action` and its siblings.
];

/// The conversation action ids the fallback view emits (its buttons map to
/// these). Kept here so the view names an id the binding table owns.
pub const ACTION_NEW_CHAT: &str = "session.new";
pub const ACTION_SUBMIT: &str = "composer.submit";
pub const ACTION_INTERRUPT: &str = "turn.interrupt";

/// The idle placeholder copy (`bindings.json` `composer.placeholder`,
/// card_source `idle_placeholder ('Ask Octos anything')`).
pub const COMPOSER_PLACEHOLDER: &str = "Ask Octos anything";

/// What a binding resolves against: the store, plus the flow's UI state for
/// the values the protocol never carries (draft, tool rows, turn timing).
pub struct Ctx<'a> {
    pub store: &'a Arc<Store>,
    pub ui: &'a Mutex<FlowUi>,
}

impl<'a> Ctx<'a> {
    pub fn new(store: &'a Arc<Store>, ui: &'a Mutex<FlowUi>) -> Self {
        Self { store, ui }
    }
}

/// Resolve a binding id. `None` when the id is not declared. Always returns
/// JSON — a card never receives a Rust type.
pub fn query(ctx: &Ctx<'_>, id: &str) -> Option<Value> {
    // Entry #29c: the stage-C screens' ids (`models.*`, `context.*`,
    // `skills.*`) resolve in their own module; conversation ids fall through.
    if let Some(v) = crate::screens::models::query_binding(ctx, id) {
        return Some(v);
    }
    let store = ctx.store;
    let ui = ctx.ui.lock().unwrap();
    Some(match id {
        // ---- pre-#12 -------------------------------------------------------
        "conn.state" => json!(store.connection()),
        "conn.live" => json!(store.is_live()),
        "session.count" => json!(store.session_count()),
        "session.list" => json!(rows_json(store)),
        "session.active" => json!(store.active_session()),
        "caps.count" => json!(store.capabilities().len()),
        "summary" => json!(store.summary()),

        // #29d — Stage C screens (board 2.8/2.11/2.12); prefix-disjoint ids.
        _ if crate::screens::palette::owns_binding(id) => {
            return crate::screens::palette::query(ctx, id);
        }

        // ---- conversation-01: THREAD LIST ---------------------------------
        "threads" => json!(rows_json(store)),
        "threads[].title" => {
            json!(store.sessions().into_iter().map(|s| s.title).collect::<Vec<_>>())
        }
        "threads[].active" => {
            let active = store.active_session();
            json!(store
                .sessions()
                .into_iter()
                .map(|s| active.as_deref() == Some(s.id.as_str()))
                .collect::<Vec<_>>())
        }
        "threads.active" => json!(store.active_session()),

        // ---- conversation-03/08: STREAMING TURN + COMPOSER -----------------
        "turn.active" => {
            // The flow knows a locally-dispatched turn before its ACK, the way
            // the web's `acceptLocalDispatch` does (`use-turn-controller.ts:413`);
            // the store's session row is the server's own view. Either ⇒ live.
            let session_active = store
                .active_session()
                .and_then(|id| store.sessions().into_iter().find(|s| s.id == id))
                .map(|s| s.active_turn)
                .unwrap_or(false);
            json!(ui.turn_active() || session_active)
        }
        "timeline.entries" => json!(timeline_json(store)),
        "timeline.entries[].text" => json!(timeline_entries(store)
            .into_iter()
            .map(|e| e.text)
            .collect::<Vec<_>>()),
        "timeline.entries[].kind" => json!(timeline_entries(store)
            .into_iter()
            .map(|e| e.kind.tag().to_owned())
            .collect::<Vec<_>>()),
        "turn.activity" => json!(ui.turn_activity()),
        "composer.draft" => json!(ui.draft()),
        "composer.placeholder" => json!(COMPOSER_PLACEHOLDER),

        // ---- conversation-04: TOOL CELLS -----------------------------------
        "tools" => json!(tools_json(&ui)),
        "tools[].summary" => json!(ui.tools().into_iter().map(|t| t.summary).collect::<Vec<_>>()),
        "tools[].status" => json!(ui.tools().into_iter().map(|t| t.status).collect::<Vec<_>>()),
        "tools[].expanded" => json!(ui
            .tools()
            .into_iter()
            .map(|t| ui.is_expanded(&t.tool_call_id))
            .collect::<Vec<_>>()),
        "tool.output" => json!(ui.tool_output()),

        // ---- conversation-09: COMPLETED ANSWER -----------------------------
        "answer.worked_for" => json!(ui.worked_for()),
        "answer.timestamp" => json!(ui.answer_timestamp()),
        "approval.pending" => json!(ui.approval_pending()),
        "question.pending" => json!(ui.question_pending()),

        // #29b: board-2 data slots answer from the screens table before the
        // conversation table declines the id.
        other => match crate::screens::workspace::query(ctx, other) {
            Some(v) => v,
            // #30d: board-3 data slots answer from the sessions table.
            None => match crate::screens::sessions::query(ctx, other) {
                Some(v) => v,
                None => return None,
            },
        },
    })
}

fn rows_json(store: &Arc<Store>) -> Vec<Value> {
    store
        .sessions()
        .into_iter()
        .map(|s| {
            json!({
                "id": s.id,
                "title": s.title,
                "message_count": s.message_count,
                "active_turn": s.active_turn,
            })
        })
        .collect()
}

fn timeline_entries(store: &Arc<Store>) -> Vec<octoscode_store::TimelineEntry> {
    match store.active_session() {
        Some(id) => store.domains.session.timeline.entries(&id),
        None => Vec::new(),
    }
}

fn timeline_json(store: &Arc<Store>) -> Vec<Value> {
    timeline_entries(store)
        .into_iter()
        .map(|e| json!({"kind": e.kind.tag(), "text": e.text, "turn_id": e.turn_id}))
        .collect()
}

fn tools_json(ui: &FlowUi) -> Vec<Value> {
    ui.tools()
        .into_iter()
        .map(|t| {
            json!({
                "name": t.name,
                "summary": t.summary,
                "status": t.status,
                "tool_call_id": t.tool_call_id,
            })
        })
        .collect()
}

/// Whether `id` is a declared action (the module performs it).
pub fn is_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id) || crate::screens::models::owns(id)
}

#[cfg(test)]
mod tests {
    //! Bindings from a store + flow fixture: no UI, no network, no transport —
    //! the binding table is pure, so it is testable in isolation.
    use super::*;
    use crate::flow::FlowUi;
    use octoscode_store::timeline::EntryKind;
    use octoscode_store::Session;

    fn fixture() -> (Arc<Store>, Mutex<FlowUi>) {
        let store = Arc::new(Store::new());
        store.set_connection("Live".into(), true);
        store.set_capabilities(vec!["auxiliary.rest_to_ws.v1".into(), "session.hydrate.v1".into()]);
        store.set_sessions(vec![
            Session {
                id: "octoscode:main".into(),
                title: Some("Fix steer queue drop".into()),
                message_count: 3,
                updated_at: None,
                last_prompt: None,
                active_turn: true,
            },
            Session {
                id: "octoscode:other".into(),
                title: None,
                message_count: 0,
                updated_at: None,
                last_prompt: None,
                active_turn: false,
            },
        ]);
        store.set_active(Some("octoscode:main".into()));
        (store, Mutex::new(FlowUi::default()))
    }

    fn q(store: &Arc<Store>, ui: &Mutex<FlowUi>, id: &str) -> Value {
        query(&Ctx::new(store, ui), id)
            .unwrap_or_else(|| panic!("binding {id:?} is not implemented"))
    }

    // --- the contract: every declared id resolves -------------------------

    #[test]
    fn every_declared_data_binding_resolves_and_actions_are_not_data() {
        let (store, ui) = fixture();
        for id in all_binding_ids() {
            assert!(
                query(&Ctx::new(&store, &ui), id).is_some(),
                "declared data binding {id:?} has no arm in query()"
            );
        }
        // 7 pre-#12 + 20 conversation data ids.
        assert_eq!(all_binding_ids().len(), 27);
        assert_eq!(WEB_BINDINGS.len(), 20);
        assert_eq!(WEB_ACTION_IDS.len(), 4);
        assert_eq!(all_conversation_ids().len(), 24, "bindings.json declares 24");

        // The 4 action ids are actions, and are NOT query-resolvable data ids.
        for id in all_conversation_ids() {
            let is_action_id = WEB_ACTION_IDS.iter().any(|(a, _)| *a == id);
            assert_eq!(
                is_action(id),
                is_action_id,
                "action-ness mismatch for {id}"
            );
            if is_action_id {
                assert!(
                    query(&Ctx::new(&store, &ui), id).is_none(),
                    "{id} is an action and must not be a data binding"
                );
            }
        }

        // The 24 conversation ids are all distinct.
        let mut ids = all_conversation_ids();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 24);
    }

    #[test]
    fn undefined_binding_is_none_not_a_panic() {
        let (store, ui) = fixture();
        assert!(query(&Ctx::new(&store, &ui), "not.a.binding").is_none());
        assert!(query(&Ctx::new(&store, &ui), "").is_none());
    }

    // --- the 7 pre-#12 ids ------------------------------------------------

    #[test]
    fn connection_and_count_bindings_are_typed_json() {
        let (store, ui) = fixture();
        assert_eq!(q(&store, &ui, "conn.state"), json!("Live"));
        assert_eq!(q(&store, &ui, "conn.live"), json!(true));
        assert_eq!(q(&store, &ui, "session.count"), json!(2));
        assert_eq!(q(&store, &ui, "caps.count"), json!(2));
        assert_eq!(q(&store, &ui, "summary"), json!("conn: Live   sessions: 2"));
        assert_eq!(q(&store, &ui, "session.active"), json!("octoscode:main"));
    }

    #[test]
    fn session_list_binding_is_a_json_array_of_rows() {
        let (store, ui) = fixture();
        let rows = q(&store, &ui, "session.list");
        let rows = rows.as_array().expect("an array, never a Rust type");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["id"], "octoscode:main");
        assert_eq!(rows[0]["message_count"], 3);
        assert_eq!(rows[0]["active_turn"], true);
        assert_eq!(rows[1]["title"], Value::Null);
    }

    // --- conversation-01: THREAD LIST (4 ids) -----------------------------

    #[test]
    fn binding_threads() {
        let (store, ui) = fixture();
        let rows = q(&store, &ui, "threads");
        let rows = rows.as_array().expect("a list");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["id"], "octoscode:main");
        assert_eq!(rows[0]["title"], "Fix steer queue drop");
        assert_eq!(rows[1]["id"], "octoscode:other");
    }

    #[test]
    fn binding_threads_title() {
        let (store, ui) = fixture();
        assert_eq!(
            q(&store, &ui, "threads[].title"),
            json!(["Fix steer queue drop", null])
        );
    }

    #[test]
    fn binding_threads_active_is_per_row() {
        let (store, ui) = fixture();
        // Only the active session's row is true.
        assert_eq!(q(&store, &ui, "threads[].active"), json!([true, false]));
    }

    #[test]
    fn binding_threads_active() {
        let (store, ui) = fixture();
        assert_eq!(q(&store, &ui, "threads.active"), json!("octoscode:main"));
    }

    // --- conversation-03/08: STREAMING TURN + COMPOSER (9 ids) ------------

    #[test]
    fn binding_turn_active_reflects_the_store_row() {
        let (store, ui) = fixture();
        // `octoscode:main` has `active_turn: true` in the fixture.
        assert_eq!(q(&store, &ui, "turn.active"), json!(true));

        // A flow-local dispatch also makes it live before any ACK.
        let (store2, ui2) = fixture();
        store2.set_sessions(vec![]);
        store2.set_active(None);
        ui2.lock().unwrap().begin_turn_now("t1");
        assert_eq!(q(&store2, &ui2, "turn.active"), json!(true));
    }

    #[test]
    fn binding_timeline_entries_and_projections() {
        let (store, ui) = fixture();
        // No entries yet: the list exists and is empty.
        assert_eq!(q(&store, &ui, "timeline.entries"), json!([]));
        assert_eq!(q(&store, &ui, "timeline.entries[].text"), json!([]));
        assert_eq!(q(&store, &ui, "timeline.entries[].kind"), json!([]));

        let tl = &store.domains.session.timeline;
        tl.append("octoscode:main", Some("t1".into()), EntryKind::USER_MESSAGE, "hello".into());
        tl.append_delta("octoscode:main", Some("t1"), EntryKind::ASSISTANT_TEXT, "hi ");
        tl.append_delta("octoscode:main", Some("t1"), EntryKind::ASSISTANT_TEXT, "there");

        let entries = q(&store, &ui, "timeline.entries");
        let entries = entries.as_array().expect("a list");
        assert_eq!(entries.len(), 2, "one user entry + one folded assistant entry");
        assert_eq!(entries[0]["kind"], "user.message");
        assert_eq!(entries[0]["turn_id"], "t1");
        assert_eq!(entries[1]["text"], "hi there");

        assert_eq!(
            q(&store, &ui, "timeline.entries[].text"),
            json!(["hello", "hi there"])
        );
        assert_eq!(
            q(&store, &ui, "timeline.entries[].kind"),
            json!(["user.message", "assistant.text"])
        );
    }

    #[test]
    fn binding_turn_activity_is_empty_until_a_turn_is_live() {
        let (store, ui) = fixture();
        assert_eq!(q(&store, &ui, "turn.activity"), json!(""));
        ui.lock().unwrap().begin_turn_now("t1");
        let v = q(&store, &ui, "turn.activity");
        assert!(v.as_str().unwrap().starts_with("Working · "), "got {v}");
    }

    #[test]
    fn binding_composer_draft_round_trips() {
        let (store, ui) = fixture();
        assert_eq!(q(&store, &ui, "composer.draft"), json!(""));
        ui.lock().unwrap().set_draft_inner("draft text");
        assert_eq!(q(&store, &ui, "composer.draft"), json!("draft text"));
    }

    #[test]
    fn binding_composer_placeholder_is_the_static_copy() {
        let (store, ui) = fixture();
        assert_eq!(q(&store, &ui, "composer.placeholder"), json!("Ask Octos anything"));
    }

    // --- conversation-04: TOOL CELLS (5 ids) ------------------------------

    #[test]
    fn bindings_tools_and_projections() {
        let (store, ui) = fixture();
        assert_eq!(q(&store, &ui, "tools"), json!([]));
        assert_eq!(q(&store, &ui, "tools[].summary"), json!([]));
        assert_eq!(q(&store, &ui, "tools[].status"), json!([]));
        assert_eq!(q(&store, &ui, "tools[].expanded"), json!([]));
        assert_eq!(q(&store, &ui, "tool.output"), json!([]));

        {
            let mut u = ui.lock().unwrap();
            u.note_tool_started_for_test("c1", "bash");
            u.note_tool_completed_for_test("c1", "bash", true, Some("exit 0\nmore"));
            u.note_tool_started_for_test("c2", "read_file");
        }

        let tools = q(&store, &ui, "tools");
        let tools = tools.as_array().expect("a list");
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "bash");
        assert_eq!(tools[0]["status"], "done");
        assert_eq!(tools[0]["summary"], "exit 0"); // first line only
        assert_eq!(tools[1]["status"], "running");

        assert_eq!(q(&store, &ui, "tools[].summary"), json!(["exit 0", "read_file"]));
        assert_eq!(q(&store, &ui, "tools[].status"), json!(["done", "running"]));
        assert_eq!(q(&store, &ui, "tool.output"), json!(["exit 0\nmore"]));
    }

    #[test]
    fn binding_tools_expanded_is_ui_local_per_row() {
        let (store, ui) = fixture();
        {
            let mut u = ui.lock().unwrap();
            u.note_tool_started_for_test("c1", "bash");
            u.note_tool_started_for_test("c2", "read_file");
            assert!(u.toggle_expanded("c1"), "first toggle opens");
        }
        assert_eq!(q(&store, &ui, "tools[].expanded"), json!([true, false]));
        ui.lock().unwrap().toggle_expanded("c1");
        assert_eq!(q(&store, &ui, "tools[].expanded"), json!([false, false]));
    }

    // --- conversation-09: COMPLETED ANSWER (5 ids) ------------------------

    #[test]
    fn binding_answer_worked_for_and_timestamp() {
        let (store, ui) = fixture();
        assert_eq!(q(&store, &ui, "answer.worked_for"), json!(""));
        {
            let mut u = ui.lock().unwrap();
            u.begin_turn_now("t1");
            u.end_turn_now(true);
        }
        let worked = q(&store, &ui, "answer.worked_for");
        assert!(
            worked.as_str().unwrap().starts_with("Worked for "),
            "got {worked}"
        );
        let ts = q(&store, &ui, "answer.timestamp");
        let ts = ts.as_str().unwrap();
        // Card #21d item 4: a just-ended turn reads `now`, never `t=<epoch>`
        // (the atlas label shape is `Sep 28, 9:41 PM`; `relative-time.ts`).
        assert_eq!(ts, "now", "a fresh turn is 'now', not the raw epoch");
    }

    #[test]
    fn action_answer_expand_toggles() {
        // `answer.expand` is an ACTION id (bindings.json type: "action"); it
        // toggles UI-local state rather than resolving a store value.
        let (_store, ui) = fixture();
        assert!(!ui.lock().unwrap().answer_expanded());
        assert!(ui.lock().unwrap().toggle_answer_expanded());
        assert!(ui.lock().unwrap().answer_expanded());
        assert!(is_action("answer.expand"));
    }

    #[test]
    fn binding_approval_and_question_pending() {
        let (store, ui) = fixture();
        assert_eq!(q(&store, &ui, "approval.pending"), json!(false));
        assert_eq!(q(&store, &ui, "question.pending"), json!(false));
        ui.lock().unwrap().set_pending_for_test(true, true);
        assert_eq!(q(&store, &ui, "approval.pending"), json!(true));
        assert_eq!(q(&store, &ui, "question.pending"), json!(true));
    }

    // --- actions ----------------------------------------------------------

    #[test]
    fn every_declared_action_is_recognized_and_unknown_ones_are_not() {
        for (id, _) in ACTIONS {
            assert!(is_action(id), "{id} must be an action");
        }
        // The four conversation actions + session.refresh + session.new
        // (card #14 defect 4: New chat mints a fresh session id) + card #21 §3's
        // three per-item controls (thread.open, answer.copy, tool.toggle).
        assert_eq!(ACTIONS.len(), 9);
        assert!(is_action("composer.submit"));
        assert!(is_action("turn.interrupt"));
        assert!(is_action("turn.steer"));
        assert!(is_action("answer.expand"));
        assert!(is_action("session.new"), "New chat is a declared action");
        assert!(!is_action("session.delete"));
    }
}
