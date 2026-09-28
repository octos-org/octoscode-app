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

use serde::{Deserialize, Serialize};

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::{methods, InputItem};
use octoscode_store::{EntryKind, Store};

use crate::method::Method;
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

/// `turn/state/get` — authoritative liveness for one turn (UPCR-2026-011).
///
/// Web call site: `src-web/apps/web/src/features/composer/use-turn-controller.ts`
/// (recovers an unknown-outcome start; the client `getTurnState` at
/// `src-web/packages/client/src/client.ts:491`). Params/result are the
/// octos-core types verbatim, so the wire shape cannot drift.
pub struct TurnStateGet;

impl Method for TurnStateGet {
    const NAME: &'static str = methods::TURN_STATE_GET;
    type Params = octos_core::ui_protocol::TurnStateGetParams;
    type Result = octos_core::ui_protocol::TurnStateGetResult;
}

/// `turn/steer` — steer the live turn's input buffer (AppUI extension).
///
/// Not a `methods::` const in octos-core: it is an AppUI extension method
/// (`src-web/packages/client/src/steer.ts:8`; server
/// `crates/octos-cli/src/api/ui_protocol_transport.rs:346`). Web call site:
/// `steer.ts:41`, gated on the method + `event.turn_steer_dropped.v1`
/// (`steer.ts:20-21`). The web sends
/// `{session_id, expected_turn_id, input: [{kind:"text", text}]}` and reads
/// `{turn_id, steered}`.
pub struct TurnSteer;

/// Params as the web sends them. `expected_turn_id` may be absent (steer
/// whatever turn is live); `input` reuses the core [`InputItem`] shape.
#[derive(Debug, Clone, Serialize)]
pub struct TurnSteerParams {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_turn_id: Option<String>,
    pub input: Vec<InputItem>,
}

/// `{turn_id, steered}` — the web's `TurnSteerResult` (`steer.ts:9-12`).
#[derive(Debug, Clone, Deserialize)]
pub struct TurnSteerResult {
    pub turn_id: String,
    pub steered: bool,
}

impl Method for TurnSteer {
    const NAME: &'static str = "turn/steer";
    type Params = TurnSteerParams;
    type Result = TurnSteerResult;
}

/// `thread/graph/get` — the session's thread graph (UPCR-2026-010).
///
/// Web caller: `src-web/apps/web/src/features/inspection/inspection-binding.ts:146`
/// (`thread/graph/get` with `{session_id}`; `at` absent = current head). Its
/// result is the token/thread structure the inspection surface renders.
pub struct ThreadGraphGet;

impl Method for ThreadGraphGet {
    const NAME: &'static str = methods::THREAD_GRAPH_GET;
    type Params = octos_core::ui_protocol::ThreadGraphGetParams;
    type Result = octos_core::ui_protocol::ThreadGraphGetResult;
}

/// `turn/steer_dropped` — accepted-but-undrained steer inputs returned at
/// turn end (`event.turn_steer_dropped.v1`). Sent BEFORE the turn's terminal
/// event; `reason` is `"interrupted"` or `"turn_ended"`.
///
/// Web consumer: `src-web/apps/web/src/features/composer/use-turn-controller.ts`
/// (SPEC: `docs/parity/g-composer.csv` row 53 — the steered text is restored
/// in order instead of being silently lost). The store keeps the dropped
/// inputs per turn so the UI can hand the text back.
pub struct TurnSteerDroppedHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for TurnSteerDroppedHandler {
    const METHOD: &'static str = methods::TURN_STEER_DROPPED;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::TurnSteerDropped(dropped) = notification {
            self.store.note_seen(Self::METHOD);
            let session = dropped.session_id.0.clone();
            let turn_id = dropped.turn_id.0.to_string();
            self.store.domains.turn.steer_dropped(
                &session,
                &turn_id,
                dropped.inputs.clone(),
                &dropped.reason,
            );
        }
    }
}

/// Request methods owned by this domain: `turn/state/get` and `turn/steer`
/// are implemented here; `turn/start` and `turn/interrupt` stay on the
/// transport's typed `OutboundCommand`s (they carry the lifecycle reply).
/// Notifications registered here: `turn/started`, `turn/completed`,
/// `turn/error`, `turn/steer_dropped`, `message/delta`.
pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(TurnStartedHandler { store: store.clone() });
    reg.register(TurnCompletedHandler { store: store.clone() });
    reg.register(TurnErrorHandler { store: store.clone() });
    reg.register(MessageDeltaHandler { store: store.clone() });
    reg.register(TurnSteerDroppedHandler { store });
}
