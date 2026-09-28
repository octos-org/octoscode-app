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
use octos_core::ui_protocol::{
    methods, AttachmentOwnerV2, EnvelopeToolEndStatus, InputItem, PayloadV2, TurnTerminalOutcome,
};
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
            // Card #14 defect 2: do NOT create an entry here. A row is born on
            // the first delta / `assistant_persisted`, as on the web
            // (`timeline/model.ts:648-678` `appendText`) — creating one eagerly
            // left a stray empty `[assistant.text]` whenever a turn produced no
            // assistant text (e.g. an interrupted turn, turn 2 in `trace.jsonl`).
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

/// `projection/envelope` — the v2 projection stream.
///
/// **Owner: turn** (card #13 §2). This is the frame the server actually sends
/// once `projection.envelope.v2` is negotiated — which our feature list does
/// (`features.rs:53`) — so without this handler the whole live turn is
/// dropped (the bug the gate found).
///
/// The transport has already decoded the frame (`UiNotification::EnvelopeV2`,
/// `ui_protocol.rs:6688`); this handler unwraps the [`PayloadV2`] kinds and
/// folds them into the SAME store domains the bare notifications use, so a
/// turn looks identical on both transports. The web's fold lives at
/// `src-web/apps/web/src/features/timeline/model.ts:326-360` (payload kind →
/// entry) and `session/durable-session.ts:140-195` (ordering).
///
/// Ordering rules (`durable-session.ts:174-195`): per-thread `seq` must be
/// strictly increasing (a `seq <=` the last accepted is dropped), and the
/// canonical cursor advances to the max. Both live in `store.domains.turn`.
pub struct ProjectionEnvelopeHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for ProjectionEnvelopeHandler {
    const METHOD: &'static str = methods::PROJECTION_ENVELOPE;

    fn handle(&self, notification: &UiNotification) {
        let UiNotification::EnvelopeV2(frame) = notification else {
            return;
        };
        self.store.note_seen(Self::METHOD);
        let env = &frame.envelope;
        let session = frame.session_id.0.clone();
        let turn_id = env.turn_id.clone();

        // Ordering: drop a non-increasing per-thread seq, else advance.
        let cursor = env.cursor.as_ref().map(|c| (c.stream.as_str(), c.seq));
        if !self
            .store
            .domains
            .turn
            .accept_envelope(&env.thread_id, env.seq, cursor)
        {
            log::debug!(
                "octoscode: dropped stale projection seq {} for thread {}",
                env.seq,
                env.thread_id
            );
            return;
        }

        let timeline = &self.store.domains.session.timeline;
        match &env.payload {
            // The user's own prompt becomes a `user.message` entry.
            //
            // Card #14 defect 1: the real server sends this at `seq 154`,
            // BEHIND 153 delta frames (`trace.jsonl`), so arrival order puts it
            // last. The web splices the canonical row in before its turn's
            // first reply and dedups an optimistic row
            // (`timeline/model.ts:1019-1043`, `upsertUser`), which
            // [`Timeline::upsert_user_message`] reproduces.
            PayloadV2::UserMessage { text, files } => {
                timeline.upsert_user_message(
                    &session,
                    &turn_id,
                    text,
                    serde_json::json!({"files": files}),
                );
            }
            // Streamed assistant text folds into ONE entry, exactly like
            // `message/delta` (the web maps both to the same segment).
            PayloadV2::AssistantDelta { text, .. } => {
                timeline.append_delta(&session, Some(&turn_id), EntryKind::ASSISTANT_TEXT, text);
            }
            // Reasoning is its own entry kind, never the answer text.
            PayloadV2::ReasoningDelta { text } => {
                timeline.append_delta(&session, Some(&turn_id), EntryKind::REASONING, text);
            }
            // Finalizes the segment its deltas wrote: our `finalize_assistant`
            // keeps the streamed text and closes the entry (falling back to
            // the persisted text if the deltas never arrived).
            PayloadV2::AssistantPersisted { text, .. } => {
                timeline.finalize_assistant(&session, &turn_id, text);
            }
            PayloadV2::ToolStart {
                tool_call_id,
                name,
                arguments_preview,
            } => {
                self.store.domains.tool.call_started(
                    tool_call_id,
                    name,
                    arguments_preview.as_deref(),
                );
                timeline.append_data(
                    &session,
                    Some(turn_id.clone()),
                    EntryKind::TOOL_CALL,
                    name.clone(),
                    serde_json::json!({"tool_call_id": tool_call_id, "status": "running"}),
                );
            }
            PayloadV2::ToolProgress {
                tool_call_id,
                message,
            } => {
                self.store
                    .domains
                    .tool
                    .call_progress(tool_call_id, message);
            }
            PayloadV2::ToolEnd {
                tool_call_id,
                status,
                output_preview,
                duration_ms,
                ..
            } => {
                let wire = match status {
                    EnvelopeToolEndStatus::Complete => "complete",
                    EnvelopeToolEndStatus::Error => "error",
                    EnvelopeToolEndStatus::Skipped => "skipped",
                    EnvelopeToolEndStatus::Aborted => "aborted",
                };
                self.store.domains.tool.call_ended(
                    tool_call_id,
                    wire,
                    output_preview.as_deref(),
                    *duration_ms,
                );
            }
            PayloadV2::FileAttached {
                path,
                mime,
                size_bytes,
                attachment_owner,
            } => {
                timeline.append_data(
                    &session,
                    Some(turn_id.clone()),
                    EntryKind::ATTACHMENT,
                    path.clone(),
                    serde_json::json!({
                        "path": path, "mime": mime, "size_bytes": size_bytes,
                        "owner": owner_json(attachment_owner),
                    }),
                );
            }
            // The canonical terminal for completed/errored/interrupted/
            // rate-limited. It settles the turn exactly like `turn/completed`
            // or `turn/error` do (the web's `turn_terminal` fold).
            PayloadV2::TurnTerminal {
                outcome,
                error,
                token_usage,
            } => {
                let name = match outcome {
                    TurnTerminalOutcome::Completed => "completed",
                    TurnTerminalOutcome::Errored => "errored",
                    TurnTerminalOutcome::Interrupted => "interrupted",
                    TurnTerminalOutcome::RateLimited => "rate_limited",
                };
                self.store.domains.turn.ended(&turn_id);
                self.store.domains.turn.set_terminal(&turn_id, name);
                match outcome {
                    TurnTerminalOutcome::Completed => {
                        timeline.close_turn(&session, &turn_id);
                    }
                    TurnTerminalOutcome::Errored => {
                        let (code, message) = error
                            .as_ref()
                            .map(|e| (e.code.clone(), e.message.clone()))
                            .unwrap_or_else(|| ("error".to_owned(), String::new()));
                        timeline.close_turn(&session, &turn_id);
                        timeline.append_data(
                            &session,
                            Some(turn_id.clone()),
                            EntryKind::SYSTEM_NOTICE,
                            format!("{code}: {message}"),
                            serde_json::json!({"code": code, "message": message}),
                        );
                    }
                    TurnTerminalOutcome::Interrupted | TurnTerminalOutcome::RateLimited => {
                        // Non-clean ends still close the streamed entry, with
                        // a named notice (never a silent stop).
                        timeline.close_turn(&session, &turn_id);
                        timeline.append_data(
                            &session,
                            Some(turn_id.clone()),
                            EntryKind::SYSTEM_NOTICE,
                            name.to_owned(),
                            serde_json::json!({"outcome": name, "token_usage": token_usage}),
                        );
                    }
                }
            }
            // A background child stream's late completion. Record it as a
            // notice on this session (the child stream carries its own
            // `parent_turn_id`); never fatal.
            PayloadV2::BackgroundChildCompleted {
                parent_turn_id,
                content,
                task_id,
                ..
            } => {
                timeline.append_data(
                    &session,
                    Some(parent_turn_id.clone()),
                    EntryKind::SYSTEM_NOTICE,
                    content.clone(),
                    serde_json::json!({"task_id": task_id, "kind": "background_spawn_complete"}),
                );
            }
        }
    }
}

/// The `attachment_owner` half of a `file_attached` payload, as JSON.
fn owner_json(owner: &AttachmentOwnerV2) -> serde_json::Value {
    serde_json::json!({
        "assistant_segment_id": owner.assistant_segment_id,
        "tool_call_id": owner.tool_call_id,
    })
}

/// `message/reasoning_delta` — the model's streamed thinking (card #13 §3).
///
/// The web renders reasoning as its own timeline row, never as answer text
/// (`src-web/apps/web/src/features/timeline/model.ts`, the reasoning kind), so
/// it folds into the `assistant.reasoning` entry kind — a separate entry from
/// `assistant.text`.
pub struct ReasoningDeltaHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for ReasoningDeltaHandler {
    const METHOD: &'static str = methods::MESSAGE_REASONING_DELTA;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::ReasoningDelta(delta) = notification {
            self.store.note_seen(Self::METHOD);
            let session = delta.session_id.0.clone();
            let turn_id = delta.turn_id.0.to_string();
            self.store.domains.session.timeline.append_delta(
                &session,
                Some(&turn_id),
                EntryKind::REASONING,
                &delta.text,
            );
        }
    }
}

/// `progress/updated` — the harness's rich progress metadata (card #13 §3).
///
/// The web renders a progress bar/spinner from this
/// (`src-web/apps/web/src/features/...`, `UiProgressEvent`), so the latest
/// metadata is stored per session and cleared at a turn boundary.
pub struct ProgressUpdatedHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for ProgressUpdatedHandler {
    const METHOD: &'static str = methods::PROGRESS_UPDATED;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::ProgressUpdated(e) = notification {
            self.store.note_seen(Self::METHOD);
            let metadata = serde_json::to_value(&e.metadata).unwrap_or(serde_json::Value::Null);
            self.store.domains.turn.set_progress(&e.session_id.0, Some(metadata));
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
    reg.register(TurnSteerDroppedHandler { store: store.clone() });
    // Card #13 §2: the v2 projection stream is owned by turn (one owner —
    // the registry panics on a duplicate).
    reg.register(ProjectionEnvelopeHandler { store: store.clone() });
    reg.register(ReasoningDeltaHandler { store: store.clone() });
    reg.register(ProgressUpdatedHandler { store });
}
