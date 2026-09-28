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
