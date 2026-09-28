//! `turn/*` and the streaming text notifications.
//!
//! Implemented in this card: the notifications the store needs now —
//! `turn/started`, `turn/completed`, `turn/error`, `message/delta`. The
//! request methods (`turn/start`, `turn/steer`, `turn/interrupt`,
//! `turn/state/get`) stay on the transport's typed commands for now
//! (`StartTurn`/`InterruptTurn`), and are listed here for the fan-out lane.
//!
//! **The new store shape (card #10):** a handler writes through its own
//! domain (`store.domains.turn`) and appends to the transcript with an
//! [`EntryKind`] tag — never a shared enum. `message/delta` uses
//! `append_delta`, which folds streamed text into ONE assistant entry.
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octoscode_store::{EntryKind, Store};

use crate::registry::{NotificationHandler, Registry};

/// `turn/started` — a turn began in a session.
pub struct TurnStartedHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for TurnStartedHandler {
    const METHOD: &'static str = methods::TURN_STARTED;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::TurnStarted(started) = notification {
            self.store.note_seen(Self::METHOD);
            let session = started.session_id.0.clone();
            let turn_id = started.turn_id.0.to_string();
            self.store.domains.turn.started(&turn_id);
            self.store.domains.session.timeline.append(
                &session,
                Some(turn_id),
                EntryKind::ASSISTANT_TEXT,
                String::new(),
            );
        }
    }
}

/// `turn/completed` — a turn ended cleanly.
pub struct TurnCompletedHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for TurnCompletedHandler {
    const METHOD: &'static str = methods::TURN_COMPLETED;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::TurnCompleted(completed) = notification {
            self.store.note_seen(Self::METHOD);
            let session = completed.session_id.0.clone();
            let turn_id = completed.turn_id.0.to_string();
            self.store.domains.turn.ended(&turn_id);
            // A turn boundary closes the assistant entry it belongs to, so
            // later deltas start a new block instead of appending to a
            // finished one.
            self.store.domains.session.timeline.close_turn(&session, &turn_id);
        }
    }
}

/// `turn/error` — a turn ended with an error (carries code + message).
pub struct TurnErrorHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for TurnErrorHandler {
    const METHOD: &'static str = methods::TURN_ERROR;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::TurnError(error) = notification {
            self.store.note_seen(Self::METHOD);
            let session = error.session_id.0.clone();
            let turn_id = error.turn_id.0.to_string();
            self.store.domains.turn.ended(&turn_id);
            self.store.domains.session.timeline.close_turn(&session, &turn_id);
            // A readable system notice: a deterministic kind, the error text.
            self.store.domains.session.timeline.append_data(
                &session,
                Some(turn_id),
                EntryKind::SYSTEM_NOTICE,
                format!("{}: {}", error.code, error.message),
                serde_json::json!({"code": error.code, "message": error.message}),
            );
        }
    }
}

/// `message/delta` — streamed assistant text (the live reply).
pub struct MessageDeltaHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for MessageDeltaHandler {
    const METHOD: &'static str = methods::MESSAGE_DELTA;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::MessageDelta(delta) = notification {
            self.store.note_seen(Self::METHOD);
            let session = delta.session_id.0.clone();
            let turn_id = delta.turn_id.0.to_string();
            // Folds into the open assistant entry for this turn (or starts one).
            self.store.domains.session.timeline.append_delta(
                &session,
                Some(&turn_id),
                EntryKind::ASSISTANT_TEXT,
                &delta.text,
            );
        }
    }
}

/// Request methods owned by this domain (stubs for the fan-out lane):
/// `turn/start`, `turn/steer`, `turn/interrupt`, `turn/state/get`.
/// Notifications: `turn/started`, `turn/completed`, `turn/error`,
/// `turn/steer_dropped`, `turn/spawn_complete`, `message/delta`,
/// `message/reasoning_delta`.
pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(TurnStartedHandler { store: store.clone() });
    reg.register(TurnCompletedHandler { store: store.clone() });
    reg.register(TurnErrorHandler { store: store.clone() });
    reg.register(MessageDeltaHandler { store });
}
