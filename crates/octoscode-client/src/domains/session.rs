//! `session/*` — the session list, open/close, and session-scoped state.
//!
//! Implemented in this card: `session/list`. `session/open` stays on the
//! transport's typed `OutboundCommand::OpenSession` (it carries the replay
//! cursor bracket), so it is deliberately NOT a [`Method`] here.
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octoscode_store::{Session, Store};

use crate::method::Method;
use crate::registry::{NotificationHandler, Registry};

/// `session/list` — the legacy per-profile listing (`cwd: None`).
#[derive(Debug, Default, Serialize)]
pub struct SessionList {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// One row of `session/list` (subset of the server's `SessionInfo` used here;
/// serde ignores the rest).
#[derive(Debug, Clone, Deserialize)]
pub struct SessionListRow {
    pub id: String,
    #[serde(default)]
    pub message_count: usize,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub last_prompt: Option<String>,
    #[serde(default)]
    pub active_turn: bool,
}

#[derive(Debug, Deserialize)]
pub struct SessionListResult {
    #[serde(default)]
    pub sessions: Vec<SessionListRow>,
}

impl Method for SessionList {
    const NAME: &'static str = methods::SESSION_LIST;
    type Params = SessionListParams;
    type Result = SessionListResult;
}

/// The params actually sent: only `cwd` is ever meaningful, and we send none.
#[derive(Debug, Default, Serialize)]
pub struct SessionListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

impl From<SessionListRow> for Session {
    fn from(r: SessionListRow) -> Self {
        Session {
            id: r.id,
            title: r.title,
            message_count: r.message_count,
            updated_at: r.updated_at,
            last_prompt: r.last_prompt,
            active_turn: r.active_turn,
        }
    }
}

impl SessionListResult {
    /// Fold the server rows into the store's session list.
    pub fn into_sessions(self) -> Vec<Session> {
        self.sessions.into_iter().map(Session::from).collect()
    }
}

/// `session/opened`-class: a session became active on this connection.
pub struct SessionOpenedHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for SessionOpenedHandler {
    const METHOD: &'static str = methods::SESSION_OPEN;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::SessionOpened(opened) = notification {
            self.store.note_seen(Self::METHOD);
            self.store.set_active(Some(opened.session_id.0.clone()));
        }
    }
}

/// Register this domain's notification handlers.
pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(SessionOpenedHandler { store });
}
