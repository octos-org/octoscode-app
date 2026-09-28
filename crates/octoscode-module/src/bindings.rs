//! The L0 binding table: the ONLY door between store data and cards.
//!
//! **Rule (8.8 condition 2):** cards never see Rust types. A card asks for a
//! value by a **binding id** (a stable string) and gets JSON back; it sends an
//! **action id** and the host performs it. This module owns the mapping, so a
//! card's contract is a small, declared table — not a struct it must mirror.
//!
//! Add a binding: one arm in [`query`]. Add an action: one arm in [`action`].
use std::sync::Arc;

use octoscode_store::Store;
use serde_json::{json, Value};

/// The declared bindings, as `(id, description)` — the contract a card author
/// reads. Kept in one place so the set is auditable.
pub const BINDINGS: &[(&str, &str)] = &[
    ("conn.state", "the connection state as display text, e.g. \"Live\""),
    ("conn.live", "whether the connection is live (bool)"),
    ("session.count", "the number of sessions in the list (int)"),
    ("session.list", "the session rows: [{id,title,message_count,active_turn}]"),
    ("session.active", "the active session id, or null"),
    ("caps.count", "the number of accepted capabilities (int)"),
    ("summary", "a one-line \"conn: … sessions: …\" summary"),
];

/// The declared actions, as `(id, description)`.
pub const ACTIONS: &[(&str, &str)] = &[
    ("session.refresh", "re-ask the server for the session list"),
];

/// Resolve a binding id against the store. `None` when the id is not declared.
/// Always returns JSON — a card never receives a Rust type.
pub fn query(store: &Arc<Store>, id: &str) -> Option<Value> {
    Some(match id {
        "conn.state" => json!(store.connection()),
        "conn.live" => json!(store.is_live()),
        "session.count" => json!(store.session_count()),
        "session.list" => json!(store
            .sessions()
            .into_iter()
            .map(|s| json!({
                "id": s.id,
                "title": s.title,
                "message_count": s.message_count,
                "active_turn": s.active_turn,
            }))
            .collect::<Vec<_>>()),
        "session.active" => json!(store.active_session()),
        "caps.count" => json!(store.capabilities().len()),
        "summary" => json!(store.summary()),
        _ => return None,
    })
}

/// The action a card may request, as the id it will be handled under. `None`
/// when the id is not declared. (The module calls the client for
/// `session.refresh`; see `lib.rs`.)
pub fn is_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id)
}

#[cfg(test)]
mod tests {
    //! Bindings from a store fixture: no UI, no network, no transport —
    //! the binding table is pure, so it is testable in isolation.
    use super::*;
    use octoscode_store::Session;

    fn fixture() -> Arc<Store> {
        let store = Arc::new(Store::new());
        store.set_connection("Live".into(), true);
        store.set_capabilities(vec!["auxiliary.rest_to_ws.v1".into(), "session.hydrate.v1".into()]);
        store.set_sessions(vec![
            Session {
                id: "octoscode:main".into(),
                title: Some("Main".into()),
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
        store
    }

    #[test]
    fn every_declared_binding_resolves() {
        let store = fixture();
        for (id, _desc) in BINDINGS {
            assert!(
                query(&store, id).is_some(),
                "declared binding {id:?} has no arm in query()"
            );
        }
    }

    #[test]
    fn undefined_binding_is_none_not_a_panic() {
        let store = fixture();
        assert!(query(&store, "not.a.binding").is_none());
        assert!(query(&store, "").is_none());
    }

    #[test]
    fn connection_and_count_bindings_are_typed_json() {
        let store = fixture();
        assert_eq!(query(&store, "conn.state"), Some(json!("Live")));
        assert_eq!(query(&store, "conn.live"), Some(json!(true)));
        assert_eq!(query(&store, "session.count"), Some(json!(2)));
        assert_eq!(query(&store, "caps.count"), Some(json!(2)));
        assert_eq!(
            query(&store, "summary"),
            Some(json!("conn: Live   sessions: 2"))
        );
    }

    #[test]
    fn session_list_binding_is_a_json_array_of_rows() {
        let store = fixture();
        let v = query(&store, "session.list").expect("declared");
        let rows = v.as_array().expect("an array, never a Rust type");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["id"], "octoscode:main");
        assert_eq!(rows[0]["message_count"], 3);
        assert_eq!(rows[0]["active_turn"], true);
        // A row with no title serializes as null, not a missing key.
        assert_eq!(rows[1]["title"], serde_json::Value::Null);

        assert_eq!(query(&store, "session.active"), Some(json!("octoscode:main")));
    }

    #[test]
    fn declared_actions_are_recognized_and_unknown_ones_are_not() {
        assert!(is_action("session.refresh"));
        assert!(!is_action("session.delete"));
    }
}
